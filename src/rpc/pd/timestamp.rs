// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This module is the low-level mechanisms for getting timestamps from a PD
//! cluster. It should be used via the `get_timestamp` API in `PdClient`.

use std::{
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
};

use futures::channel::{
    mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use futures::compat::{Compat01As03, Compat01As03Sink};
use futures::future::TryFutureExt;
use futures::prelude::*;
use grpcio::WriteFlags;
use kvproto::pdpb;
use tokio_core::reactor::{Core, Handle as TokioHandle};

use crate::{
    compat::SinkCompat,
    rpc::pd::{
        client::Cluster,
        context::{observe_tso_batch, request_context},
        Timestamp,
    },
    Result,
};

type TsoChannel = oneshot::Sender<Timestamp>;

enum Task {
    Init,
    Request,
    Response(Vec<oneshot::Sender<Timestamp>>, pdpb::TsoResponse),
}

impl Task {
    fn dispatch(
        self,
        reactor: Arc<RwLock<PdReactor>>,
        cluster_id: u64,
        pd_client: &pdpb::PdClient,
        handle: &TokioHandle,
    ) {
        match self {
            Task::Request => reactor.write().unwrap().tso_request(cluster_id),
            Task::Response(requests, response) => {
                reactor.write().unwrap().tso_response(requests, response)
            }
            Task::Init => PdReactor::init(reactor, pd_client, handle),
        }
    }
}

/// A special-purpose event loop for asynchronously requesting timestamps. This is
/// more complex than just sending a request since requests are batched on the client.
pub(super) struct PdReactor {
    // These fields are for communicating internally within the reactor to initialize
    // it and send communicate between threads.
    task_tx: Option<UnboundedSender<Task>>,
    tso_tx: Sender<pdpb::TsoRequest>,
    tso_rx: Option<Receiver<pdpb::TsoRequest>>,

    // These fields are for communicating with the PD cluster to get batches of timestamps.
    handle: Option<JoinHandle<()>>,
    tso_pending: Option<Vec<TsoChannel>>,
    tso_buffer: Option<Vec<TsoChannel>>,
    tso_batch: Vec<TsoChannel>,
}

impl Drop for PdReactor {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl PdReactor {
    pub(super) fn new() -> Self {
        let (tso_tx, tso_rx) = mpsc::channel(1);
        PdReactor {
            task_tx: None,
            tso_tx,
            tso_rx: Some(tso_rx),
            handle: None,
            tso_buffer: Some(Vec::with_capacity(8)),
            tso_batch: Vec::with_capacity(8),
            tso_pending: None,
        }
    }

    /// Startup the reactor, including the communication thread if necessary.
    pub(super) fn start(r: Arc<RwLock<Self>>, cluster: &Cluster) {
        let reactor = &mut r.write().unwrap();
        if reactor.handle.is_none() {
            info!("starting pd reactor thread");
            let (task_tx, task_rx) = mpsc::unbounded();
            task_tx.unbounded_send(Task::Init).unwrap();
            let pd_client = cluster.client.clone();
            let cluster_id = cluster.id;
            reactor.task_tx = Some(task_tx);
            let r2 = r.clone();
            reactor.handle = Some(
                thread::Builder::new()
                    .name("dispatcher thread".to_owned())
                    .spawn(move || PdReactor::poll(r2, cluster_id, pd_client, task_rx))
                    .unwrap(),
            )
        } else {
            warn!("tso sender and receiver are stale, refreshing...");
            let (tso_tx, tso_rx) = mpsc::channel(1);
            reactor.tso_tx = tso_tx;
            reactor.tso_rx = Some(tso_rx);
            reactor.schedule(Task::Init);
        }
    }

    fn poll(
        r: Arc<RwLock<Self>>,
        cluster_id: u64,
        pd_client: pdpb::PdClient,
        task_rx: UnboundedReceiver<Task>,
    ) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let f = task_rx
            .map(|t| {
                t.dispatch(r.clone(), cluster_id, &pd_client, &handle);
            })
            .collect::<Vec<()>>();
        core.run(TryFutureExt::compat(f.unit_error())).unwrap();
    }

    // Initialise client for communicating with the PD cluster.
    fn init(r: Arc<RwLock<Self>>, pd_client: &pdpb::PdClient, handle: &TokioHandle) {
        let (tx, rx) = pd_client.tso().unwrap();
        let tx = Compat01As03Sink::new(tx);
        let rx = Compat01As03::new(rx);
        let tso_rx = r.write().unwrap().tso_rx.take().unwrap(); // Receiver<TsoRequest>: Stream

        handle.spawn(
            tx.sink_map_err(Into::into)
                .send_all_compat(tso_rx.map(|r| (r, WriteFlags::default())))
                .map(|r: Result<_>| match r {
                    Ok((_sender, _)) => {
                        // FIXME(#54) the previous code doesn't work because we can't get mutable
                        // access to the underlying StreamingCallSink to call `cancel`. But I think
                        // that is OK because it will be canceled when it is dropped.
                        //
                        // _sender.get_mut().get_ref().cancel();
                        Ok(())
                    }
                    Err(e) => {
                        error!("failed to send tso requests: {:?}", e);
                        Err(())
                    }
                })
                .compat(),
        );

        handle.spawn(
            rx.try_for_each(move |resp| {
                let reactor = &mut r.write().unwrap();
                let tso_pending = reactor.tso_pending.take().unwrap();
                reactor.schedule(Task::Response(tso_pending, resp));
                if !reactor.tso_batch.is_empty() {
                    // Schedule another tso_batch of request
                    reactor.schedule(Task::Request);
                }
                future::ready(Ok(()))
            })
            .map_err(|e| panic!("unexpected error: {:?}", e))
            .compat(),
        );
    }

    fn tso_request(&mut self, cluster_id: u64) {
        let mut tso_batch = self.tso_buffer.take().unwrap();
        tso_batch.extend(self.tso_batch.drain(..));
        let mut request = pd_request!(cluster_id, pdpb::TsoRequest);
        let batch_size = tso_batch.len();
        observe_tso_batch(batch_size);
        request.count = batch_size as u32;
        self.tso_pending = Some(tso_batch);
        self.tso_tx
            .try_send(request)
            .expect("channel can never be full");
    }

    fn tso_response(&mut self, mut requests: Vec<TsoChannel>, response: pdpb::TsoResponse) {
        let timestamp = response.get_timestamp();
        for (offset, request) in requests.drain(..).enumerate() {
            request
                .send(Timestamp {
                    physical: timestamp.physical,
                    logical: timestamp.logical + offset as i64,
                })
                .unwrap();
        }
        self.tso_buffer = Some(requests);
    }

    fn schedule(&self, task: Task) {
        self.task_tx
            .as_ref()
            .unwrap()
            .unbounded_send(task)
            .expect("unbounded send should never fail");
    }

    pub fn get_timestamp(&mut self) -> impl Future<Output = Result<Timestamp>> {
        let context = request_context("get_ts");
        let (tx, rx) = oneshot::channel::<Timestamp>();
        self.tso_batch.push(tx);
        if self.tso_pending.is_none() {
            // Schedule tso request to run.
            self.schedule(Task::Request);
        }
        rx.map_err(Into::into)
            .into_future()
            .map(move |r| context.done(r))
    }
}
