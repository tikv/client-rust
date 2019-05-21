// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use futures::{
    sync::{
        mpsc::{channel, unbounded, Receiver, Sender, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    Future, Sink, Stream,
};
use fxhash::FxHashSet as HashSet;
use grpcio::{CallOption, Environment, WriteFlags};
use kvproto::pdpb;
use log::*;
use tokio_core::reactor::{Core, Handle as OtherHandle};

use crate::{
    rpc::{
        pd::{
            context::{observe_tso_batch, request_context},
            PdTimestamp,
        },
        security::SecurityManager,
        util::HandyRwLock,
    },
    Error, Result,
};

macro_rules! pd_request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::default();
        let mut header = ::kvproto::pdpb::RequestHeader::default();
        header.set_cluster_id($cluster_id);
        request.set_header(header);
        request
    }};
}

type TsoChannel = oneshot::Sender<PdTimestamp>;

enum PdTask {
    Init,
    Request,
    Response(Vec<oneshot::Sender<PdTimestamp>>, pdpb::TsoResponse),
}

struct PdReactor {
    task_tx: Option<UnboundedSender<Option<PdTask>>>,
    tso_tx: Sender<pdpb::TsoRequest>,
    tso_rx: Option<Receiver<pdpb::TsoRequest>>,

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
    fn new() -> Self {
        let (tso_tx, tso_rx) = channel(1);
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

    fn start(&mut self, client: Arc<RwLock<LeaderClient>>) {
        if self.handle.is_none() {
            info!("starting pd reactor thread");
            let (task_tx, task_rx) = unbounded();
            task_tx.unbounded_send(Some(PdTask::Init)).unwrap();
            self.task_tx = Some(task_tx);
            self.handle = Some(
                thread::Builder::new()
                    .name("dispatcher thread".to_owned())
                    .spawn(move || Self::poll(&client, task_rx))
                    .unwrap(),
            )
        } else {
            warn!("tso sender and receiver are stale, refreshing...");
            let (tso_tx, tso_rx) = channel(1);
            self.tso_tx = tso_tx;
            self.tso_rx = Some(tso_rx);
            self.schedule(PdTask::Init);
        }
    }

    fn schedule(&self, task: PdTask) {
        self.task_tx
            .as_ref()
            .unwrap()
            .unbounded_send(Some(task))
            .expect("unbounded send should never fail");
    }

    fn poll(client: &Arc<RwLock<LeaderClient>>, rx: UnboundedReceiver<Option<PdTask>>) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        {
            let f = rx.take_while(|t| Ok(t.is_some())).for_each(|t| {
                Self::dispatch(&client, t.unwrap(), &handle);
                Ok(())
            });
            core.run(f).unwrap();
        }
    }

    fn init(client: &Arc<RwLock<LeaderClient>>, handle: &OtherHandle) {
        let client = Arc::clone(client);
        let (tx, rx) = client.wl().client.tso().unwrap();
        let tso_rx = client.wl().reactor.tso_rx.take().unwrap();
        handle.spawn(
            tx.sink_map_err(Into::into)
                .send_all(tso_rx.then(|r| match r {
                    Ok(r) => Ok((r, WriteFlags::default())),
                    Err(()) => Err(internal_err!("failed to recv tso requests")),
                }))
                .then(|r: Result<_>| match r {
                    Ok((mut sender, _)) => {
                        sender.get_mut().cancel();
                        Ok(())
                    }
                    Err(e) => {
                        error!("failed to send tso requests: {:?}", e);
                        Err(())
                    }
                }),
        );
        handle.spawn(
            rx.for_each(move |resp| {
                let mut client = client.wl();
                let reactor = &mut client.reactor;
                let tso_pending = reactor.tso_pending.take().unwrap();
                reactor.schedule(PdTask::Response(tso_pending, resp));
                if !reactor.tso_batch.is_empty() {
                    // Schedule another tso_batch of request
                    reactor.schedule(PdTask::Request);
                }
                Ok(())
            })
            .map_err(|e| panic!("unexpected error: {:?}", e)),
        );
    }

    fn tso_request(client: &Arc<RwLock<LeaderClient>>) {
        let mut client = client.wl();
        let cluster_id = client.cluster_id;
        let reactor = &mut client.reactor;
        let mut tso_batch = reactor.tso_buffer.take().unwrap();
        tso_batch.extend(reactor.tso_batch.drain(..));
        let mut request = pd_request!(cluster_id, pdpb::TsoRequest);
        let batch_size = observe_tso_batch(tso_batch.len());
        request.set_count(batch_size);
        reactor.tso_pending = Some(tso_batch);
        reactor
            .tso_tx
            .try_send(request)
            .expect("channel can never be full");
    }

    fn tso_response(
        client: &Arc<RwLock<LeaderClient>>,
        mut requests: Vec<TsoChannel>,
        response: &pdpb::TsoResponse,
    ) {
        let timestamp = response.get_timestamp();
        for (offset, request) in requests.drain(..).enumerate() {
            request
                .send(PdTimestamp {
                    physical: timestamp.physical,
                    logical: timestamp.logical + offset as i64,
                })
                .unwrap();
        }
        client.wl().reactor.tso_buffer = Some(requests);
    }

    fn dispatch(client: &Arc<RwLock<LeaderClient>>, task: PdTask, handle: &OtherHandle) {
        match task {
            PdTask::Request => Self::tso_request(client),
            PdTask::Response(requests, response) => Self::tso_response(client, requests, &response),
            PdTask::Init => Self::init(client, handle),
        }
    }

    fn get_ts(&mut self) -> impl Future<Item = PdTimestamp, Error = Error> {
        let context = request_context("get_ts", ());
        let (tx, rx) = oneshot::channel::<PdTimestamp>();
        self.tso_batch.push(tx);
        if self.tso_pending.is_none() {
            // Schedule tso request to run.
            self.schedule(PdTask::Request);
        }
        rx.map_err(Into::into).then(move |r| context.done(r))
    }
}

pub struct LeaderClient {
    pub client: pdpb::PdClient,
    pub members: pdpb::GetMembersResponse,

    env: Arc<Environment>,
    cluster_id: u64,
    security_mgr: Arc<SecurityManager>,
    last_update: Instant,
    reactor: PdReactor,
    timeout: Duration,
}

impl LeaderClient {
    pub fn cluster_id(&self) -> u64 {
        self.cluster_id
    }

    pub fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<Arc<RwLock<LeaderClient>>> {
        let (client, members) = validate_endpoints(&env, endpoints, &security_mgr, timeout)?;
        let cluster_id = members.get_header().get_cluster_id();
        let client = Arc::new(RwLock::new(LeaderClient {
            env,
            client,
            members,
            security_mgr,
            last_update: Instant::now(),
            reactor: PdReactor::new(),
            cluster_id,
            timeout,
        }));

        client.wl().reactor.start(Arc::clone(&client));
        Ok(client)
    }

    pub fn get_ts(&mut self) -> impl Future<Item = PdTimestamp, Error = Error> {
        self.reactor.get_ts()
    }

    // Re-establish connection with PD leader in synchronized fashion.
    pub fn reconnect(leader: &Arc<RwLock<LeaderClient>>, interval: u64) -> Result<()> {
        warn!("updating pd client, blocking the tokio core");
        let ((client, members), start) = {
            let leader = leader.rl();
            if leader.last_update.elapsed() < Duration::from_secs(interval) {
                // Avoid unnecessary updating.
                return Ok(());
            }

            let start = Instant::now();
            let timeout = leader.timeout;
            (
                try_connect_leader(&leader.env, &leader.security_mgr, &leader.members, timeout)?,
                start,
            )
        };

        {
            let leader_clone = Arc::clone(leader);
            let mut leader = leader.wl();
            leader.client = client;
            leader.members = members;
            leader.last_update = Instant::now();
            leader.reactor.start(leader_clone);
        }
        warn!("updating PD client done, spent {:?}", start.elapsed());
        Ok(())
    }
}

pub fn validate_endpoints(
    env: &Arc<Environment>,
    endpoints: &[String],
    security_mgr: &SecurityManager,
    timeout: Duration,
) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
    let len = endpoints.len();
    let mut endpoints_set = HashSet::with_capacity_and_hasher(len, Default::default());

    let mut members = None;
    let mut cluster_id = None;
    for ep in endpoints {
        if !endpoints_set.insert(ep) {
            return Err(internal_err!("duplicated PD endpoint {}", ep));
        }

        let (_, resp) = match connect(Arc::clone(&env), security_mgr, ep, timeout) {
            Ok(resp) => resp,
            // Ignore failed PD node.
            Err(e) => {
                error!("PD endpoint {} failed to respond: {:?}", ep, e);
                continue;
            }
        };

        // Check cluster ID.
        let cid = resp.get_header().get_cluster_id();
        if let Some(sample) = cluster_id {
            if sample != cid {
                return Err(internal_err!(
                    "PD response cluster_id mismatch, want {}, got {}",
                    sample,
                    cid
                ));
            }
        } else {
            cluster_id = Some(cid);
        }
        // TODO: check all fields later?

        if members.is_none() {
            members = Some(resp);
        }
    }

    match members {
        Some(members) => {
            let (client, members) = try_connect_leader(&env, security_mgr, &members, timeout)?;
            info!("All PD endpoints are consistent: {:?}", endpoints);
            Ok((client, members))
        }
        _ => Err(internal_err!("PD cluster failed to respond")),
    }
}

fn connect(
    env: Arc<Environment>,
    security_mgr: &SecurityManager,
    addr: &str,
    timeout: Duration,
) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
    let client = security_mgr.connect(env, addr, pdpb::PdClient::new)?;
    let option = CallOption::default().timeout(timeout);
    let resp = client
        .get_members_opt(&pdpb::GetMembersRequest::default(), option)
        .map_err(Error::from)?;
    Ok((client, resp))
}

fn try_connect(
    env: &Arc<Environment>,
    security_mgr: &SecurityManager,
    addr: &str,
    cluster_id: u64,
    timeout: Duration,
) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
    let (client, r) = connect(Arc::clone(&env), security_mgr, addr, timeout)?;
    let new_cluster_id = r.get_header().get_cluster_id();
    if new_cluster_id != cluster_id {
        Err(internal_err!(
            "{} no longer belongs to cluster {}, it is in {}",
            addr,
            cluster_id,
            new_cluster_id
        ))
    } else {
        Ok((client, r))
    }
}

pub fn try_connect_leader(
    env: &Arc<Environment>,
    security_mgr: &SecurityManager,
    previous: &pdpb::GetMembersResponse,
    timeout: Duration,
) -> Result<(pdpb::PdClient, pdpb::GetMembersResponse)> {
    let previous_leader = previous.get_leader();
    let members = previous.get_members();
    let cluster_id = previous.get_header().get_cluster_id();
    let mut resp = None;
    // Try to connect to other members, then the previous leader.
    'outer: for m in members
        .iter()
        .filter(|m| *m != previous_leader)
        .chain(&[previous_leader.clone()])
    {
        for ep in m.get_client_urls() {
            match try_connect(&env, security_mgr, ep.as_str(), cluster_id, timeout) {
                Ok((_, r)) => {
                    resp = Some(r);
                    break 'outer;
                }
                Err(e) => {
                    error!("failed to connect to {}, {:?}", ep, e);
                    continue;
                }
            }
        }
    }

    // Then try to connect the PD cluster leader.
    if let Some(resp) = resp {
        let leader = resp.get_leader().clone();
        for ep in leader.get_client_urls() {
            let r = try_connect(&env, security_mgr, ep.as_str(), cluster_id, timeout);
            if r.is_ok() {
                return r;
            }
        }
    }

    Err(internal_err!("failed to connect to {:?}", members))
}
