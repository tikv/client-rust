// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This module is the low-level mechanisms for getting timestamps from a PD
//! cluster. It should be used via the `get_timestamp` API in `PdClient`.
//!
//! Once a `TimestampOracle` is created, there will be two futures running in a background working
//! thread created automatically. The `get_timestamp` method creates a oneshot channel whose
//! transmitter is served as a `TimestampRequest`. `TimestampRequest`s are sent to the working
//! thread through a bounded multi-producer, single-consumer channel. Every time the first future
//! is polled, it tries to exhaust the channel to get as many requests as possible and sends a
//! single `TsoRequest` to the PD server. The other future receives `TsoResponse`s from the PD
//! server and allocates timestamps for the requests.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;

use futures::pin_mut;
use futures::prelude::*;
use futures::task::AtomicWaker;
use futures::task::Context;
use futures::task::Poll;
use log::debug;
use log::info;
use pin_project::pin_project;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use crate::internal_err;
use crate::proto::pdpb;
use crate::proto::pdpb::pd_client::PdClient as PdTsoClient;
use crate::proto::tsopb;
use crate::proto::tsopb::tso_client::TsoClient;
use crate::Result;
use crate::Timestamp;

/// It is an empirical value.
const MAX_BATCH_SIZE: usize = 64;

/// TODO: This value should be adjustable.
const MAX_PENDING_COUNT: usize = 1 << 16;

type TimestampRequest = oneshot::Sender<Timestamp>;

/// The timestamp oracle (TSO) which provides monotonically increasing timestamps.
#[derive(Clone)]
pub(crate) struct TimestampOracle {
    /// The transmitter of a bounded channel which transports requests of getting a single
    /// timestamp to the TSO working thread. A bounded channel is used to prevent using
    /// too much memory unexpectedly.
    /// In the working thread, the `TimestampRequest`, which is actually a one channel sender,
    /// is used to send back the timestamp result.
    request_tx: mpsc::Sender<TimestampRequest>,
}

impl TimestampOracle {
    pub(crate) fn new(
        cluster_id: u64,
        pd_client: &PdTsoClient<Channel>,
    ) -> Result<TimestampOracle> {
        let pd_client = pd_client.clone();
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);

        // Start a background thread to handle TSO requests and responses
        tokio::spawn(run_pd_tso(cluster_id, pd_client, request_rx));

        Ok(TimestampOracle { request_tx })
    }

    pub(crate) fn new_tso_service(
        cluster_id: u64,
        keyspace_id: u32,
        keyspace_group_id: u32,
        tso_client: TsoClient<Channel>,
    ) -> Result<TimestampOracle> {
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);

        tokio::spawn(run_tso_service(
            cluster_id,
            keyspace_id,
            keyspace_group_id,
            tso_client,
            request_rx,
        ));

        Ok(TimestampOracle { request_tx })
    }

    pub(crate) async fn get_timestamp(self) -> Result<Timestamp> {
        debug!("getting current timestamp");
        let (request, response) = oneshot::channel();
        self.request_tx
            .send(request)
            .await
            .map_err(|_| internal_err!("TimestampRequest channel is closed"))?;
        Ok(response.await?)
    }
}

async fn run_pd_tso(
    cluster_id: u64,
    mut pd_client: PdTsoClient<Channel>,
    request_rx: mpsc::Receiver<TimestampRequest>,
) -> Result<()> {
    // The `TimestampRequest`s which are waiting for the responses from the PD server
    let pending_requests = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));

    // When there are too many pending requests, the `send_request` future will refuse to fetch
    // more requests from the bounded channel. This waker is used to wake up the sending future
    // if the queue containing pending requests is no longer full.
    let sending_future_waker = Arc::new(AtomicWaker::new());

    let request_stream = PdTsoRequestStream {
        cluster_id,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
    };

    // let send_requests = rpc_sender.send_all(&mut request_stream);
    let mut responses = pd_client.tso(request_stream).await?.into_inner();

    while let Some(Ok(resp)) = responses.next().await {
        {
            let mut pending_requests = pending_requests.lock().await;
            allocate_timestamps(resp.count, resp.timestamp.as_ref(), &mut pending_requests)?;
        }

        // Wake up the sending future blocked by too many pending requests or locked.
        sending_future_waker.wake();
    }
    // TODO: distinguish between unexpected stream termination and expected end of test
    info!("TSO stream terminated");
    Ok(())
}

async fn run_tso_service(
    cluster_id: u64,
    keyspace_id: u32,
    keyspace_group_id: u32,
    mut tso_client: TsoClient<Channel>,
    request_rx: mpsc::Receiver<TimestampRequest>,
) -> Result<()> {
    let pending_requests = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));
    let sending_future_waker = Arc::new(AtomicWaker::new());

    let request_stream = TsoServiceRequestStream {
        cluster_id,
        keyspace_id,
        keyspace_group_id,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
    };

    let mut responses = tso_client.tso(request_stream).await?.into_inner();

    while let Some(Ok(resp)) = responses.next().await {
        if let Some(err) = resp.header.as_ref().and_then(|h| h.error.as_ref()) {
            return Err(internal_err!("TSO service returned error: {}", err.message));
        }
        {
            let mut pending_requests = pending_requests.lock().await;
            allocate_timestamps(resp.count, resp.timestamp.as_ref(), &mut pending_requests)?;
        }
        sending_future_waker.wake();
    }
    info!("TSO service stream terminated");
    Ok(())
}

struct RequestGroup {
    count: u32,
    requests: Vec<TimestampRequest>,
}

#[pin_project]
struct PdTsoRequestStream {
    cluster_id: u64,
    #[pin]
    request_rx: mpsc::Receiver<oneshot::Sender<Timestamp>>,
    pending_requests: Arc<Mutex<VecDeque<RequestGroup>>>,
    self_waker: Arc<AtomicWaker>,
}

impl Stream for PdTsoRequestStream {
    type Item = pdpb::TsoRequest;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        let pending_requests = this.pending_requests.lock();
        pin_mut!(pending_requests);
        let mut pending_requests = if let Poll::Ready(pending_requests) = pending_requests.poll(cx)
        {
            pending_requests
        } else {
            this.self_waker.register(cx.waker());
            return Poll::Pending;
        };
        let mut requests = Vec::new();

        while requests.len() < MAX_BATCH_SIZE && pending_requests.len() < MAX_PENDING_COUNT {
            match this.request_rx.poll_recv(cx) {
                Poll::Ready(Some(sender)) => {
                    requests.push(sender);
                }
                Poll::Ready(None) if requests.is_empty() => return Poll::Ready(None),
                _ => break,
            }
        }

        if !requests.is_empty() {
            let count = requests.len() as u32;
            let req = pdpb::TsoRequest {
                header: Some(pdpb::RequestHeader {
                    cluster_id: *this.cluster_id,
                    sender_id: 0,
                }),
                count,
                dc_location: String::new(),
            };

            let request_group = RequestGroup { count, requests };
            pending_requests.push_back(request_group);

            Poll::Ready(Some(req))
        } else {
            // Set the waker to the context, then the stream can be waked up after the pending queue
            // is no longer full.
            this.self_waker.register(cx.waker());
            Poll::Pending
        }
    }
}

#[pin_project]
struct TsoServiceRequestStream {
    cluster_id: u64,
    keyspace_id: u32,
    keyspace_group_id: u32,
    #[pin]
    request_rx: mpsc::Receiver<oneshot::Sender<Timestamp>>,
    pending_requests: Arc<Mutex<VecDeque<RequestGroup>>>,
    self_waker: Arc<AtomicWaker>,
}

impl Stream for TsoServiceRequestStream {
    type Item = tsopb::TsoRequest;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        let pending_requests = this.pending_requests.lock();
        pin_mut!(pending_requests);
        let mut pending_requests = if let Poll::Ready(pending_requests) = pending_requests.poll(cx)
        {
            pending_requests
        } else {
            this.self_waker.register(cx.waker());
            return Poll::Pending;
        };
        let mut requests = Vec::new();

        while requests.len() < MAX_BATCH_SIZE && pending_requests.len() < MAX_PENDING_COUNT {
            match this.request_rx.poll_recv(cx) {
                Poll::Ready(Some(sender)) => {
                    requests.push(sender);
                }
                Poll::Ready(None) if requests.is_empty() => return Poll::Ready(None),
                _ => break,
            }
        }

        if !requests.is_empty() {
            let count = requests.len() as u32;
            let req = tsopb::TsoRequest {
                header: Some(tsopb::RequestHeader {
                    cluster_id: *this.cluster_id,
                    sender_id: 0,
                    keyspace_id: *this.keyspace_id,
                    keyspace_group_id: *this.keyspace_group_id,
                }),
                count,
                dc_location: String::new(),
            };

            pending_requests.push_back(RequestGroup { count, requests });

            Poll::Ready(Some(req))
        } else {
            this.self_waker.register(cx.waker());
            Poll::Pending
        }
    }
}

fn allocate_timestamps(
    count: u32,
    timestamp: Option<&Timestamp>,
    pending_requests: &mut VecDeque<RequestGroup>,
) -> Result<()> {
    // PD returns the timestamp with the biggest logical value. We can send back timestamps
    // whose logical value is from `logical - count + 1` to `logical` using the senders
    // in `pending`.
    let tail_ts = timestamp.ok_or_else(|| internal_err!("No timestamp in TsoResponse"))?;

    let mut offset = count;
    if let Some(RequestGroup {
        count: request_count,
        requests,
    }) = pending_requests.pop_front()
    {
        if request_count != offset {
            return Err(internal_err!(
                "PD gives different number of timestamps than expected"
            ));
        }

        for request in requests {
            offset -= 1;
            let ts = Timestamp {
                physical: tail_ts.physical,
                logical: tail_ts.logical - offset as i64,
                suffix_bits: tail_ts.suffix_bits,
            };
            let _ = request.send(ts);
        }
    } else {
        return Err(internal_err!("PD gives more TsoResponse than expected"));
    };
    Ok(())
}
