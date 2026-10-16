// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This module is the low-level mechanisms for getting timestamps from a PD
//! cluster. It should be used via the `get_timestamp` API in `PdClient`.
//!
//! Once a `TimestampOracle` is created, a background task serves its requests over a TSO
//! stream. The `get_timestamp` method creates a oneshot channel whose transmitter is served
//! as a `TimestampRequest`. `TimestampRequest`s are sent to the background task through a
//! bounded multi-producer, single-consumer channel. Every time the request side of the
//! stream is polled, it tries to exhaust the channel to get as many requests as possible
//! and sends a single `TsoRequest` to the PD server. The background task receives
//! `TsoResponse`s from the PD server and allocates timestamps for the requests.
//!
//! When the stream fails (PD stalls, restarts or closes it), the requests waiting on it
//! fail with the stream's error and the background task opens a new stream, with backoff,
//! for the requests that follow. The oracle's request channel stays open for as long as
//! the oracle exists.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use futures::prelude::*;
use futures::task::AtomicWaker;
use futures::task::Context;
use futures::task::Poll;
use log::debug;
use log::info;
use log::warn;
use pin_project::pin_project;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tonic::transport::Channel;

use crate::internal_err;
use crate::proto::pdpb::pd_client::PdClient;
use crate::proto::pdpb::*;
use crate::Result;

/// It is an empirical value.
const MAX_BATCH_SIZE: usize = 64;

/// TODO: This value should be adjustable.
const MAX_PENDING_COUNT: usize = 1 << 16;

/// The first pause before reopening a failed TSO stream. It doubles up to
/// [`TSO_RECONNECT_BACKOFF_MAX`] and starts again from here after a stream that
/// delivered at least one response.
const TSO_RECONNECT_BACKOFF_MIN: Duration = Duration::from_millis(50);
const TSO_RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(1);

type TimestampRequest = oneshot::Sender<Result<Timestamp>>;

/// The timestamp oracle (TSO) which provides monotonically increasing timestamps.
#[derive(Clone)]
pub(crate) struct TimestampOracle {
    /// The transmitter of a bounded channel which transports requests of getting a single
    /// timestamp to the TSO background task. A bounded channel is used to prevent using
    /// too much memory unexpectedly.
    /// In the background task, the `TimestampRequest`, which is actually a one channel
    /// sender, is used to send back the timestamp result.
    request_tx: mpsc::Sender<TimestampRequest>,
}

impl TimestampOracle {
    pub(crate) fn new(cluster_id: u64, pd_client: &PdClient<Channel>) -> Result<TimestampOracle> {
        Ok(Self::with_connector(
            cluster_id,
            GrpcTsoConnector(pd_client.clone()),
        ))
    }

    fn with_connector(cluster_id: u64, connector: impl TsoConnector) -> TimestampOracle {
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);

        // Start a background task to handle TSO requests and responses.
        tokio::spawn(run_tso(cluster_id, connector, request_rx));

        TimestampOracle { request_tx }
    }

    pub(crate) async fn get_timestamp(self) -> Result<Timestamp> {
        debug!("getting current timestamp");
        let (request, response) = oneshot::channel();
        self.request_tx
            .send(request)
            .await
            .map_err(|_| internal_err!("TimestampRequest channel is closed"))?;
        response.await?
    }
}

/// Opens a TSO stream. In production this is PD's `Tso` RPC; the unit tests use
/// in-memory streams.
#[async_trait]
trait TsoConnector: Send + 'static {
    type Responses: Stream<Item = std::result::Result<TsoResponse, tonic::Status>>
        + Send
        + Unpin
        + 'static;

    async fn connect(&mut self, requests: TsoRequestStream) -> Result<Self::Responses>;
}

struct GrpcTsoConnector(PdClient<Channel>);

#[async_trait]
impl TsoConnector for GrpcTsoConnector {
    type Responses = tonic::Streaming<TsoResponse>;

    async fn connect(&mut self, requests: TsoRequestStream) -> Result<Self::Responses> {
        Ok(self.0.tso(requests).await?.into_inner())
    }
}

/// The request receiver, shared by the successive streams of one oracle. Only the
/// current stream takes requests from it (see [`StreamState::dead`]).
type SharedReceiver = Arc<Mutex<mpsc::Receiver<TimestampRequest>>>;

/// The state one TSO stream shares between its request side (polled by the gRPC
/// transport) and its response side (the background task).
struct StreamState {
    /// The `TimestampRequest`s which are waiting for the responses from the PD server.
    pending_requests: Mutex<VecDeque<RequestGroup>>,
    /// The request that woke the background task up to reopen a failed stream; the
    /// stream sends it first. Taken only while holding `pending_requests`.
    first_request: Mutex<Option<TimestampRequest>>,
    /// Set by the background task once the stream has failed. A dead stream takes no
    /// more requests from the shared receiver, so they wait for the next stream.
    /// Written only while holding `pending_requests`, and read under it too.
    dead: AtomicBool,
    /// Set by the request side when the request channel is closed (every
    /// `TimestampOracle` handle was dropped): the expected end of the stream.
    request_channel_closed: AtomicBool,
    /// When there are too many pending requests, the request side refuses to fetch
    /// more requests from the bounded channel. This waker is used to wake it up once
    /// the queue containing pending requests is no longer full.
    request_side_waker: AtomicWaker,
}

impl StreamState {
    fn new(first_request: Option<TimestampRequest>) -> Arc<StreamState> {
        Arc::new(StreamState {
            pending_requests: Mutex::new(VecDeque::with_capacity(MAX_BATCH_SIZE)),
            first_request: Mutex::new(first_request),
            dead: AtomicBool::new(false),
            request_channel_closed: AtomicBool::new(false),
            request_side_waker: AtomicWaker::new(),
        })
    }

    fn take_first_request(&self) -> Option<TimestampRequest> {
        self.first_request
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, VecDeque<RequestGroup>> {
        self.pending_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Marks the stream dead and fails every request still waiting on it.
    fn fail(&self, reason: &str) {
        let (failed, first) = {
            let mut pending = self.lock_pending();
            self.dead.store(true, Ordering::SeqCst);
            (std::mem::take(&mut *pending), self.take_first_request())
        };
        let requests = failed
            .into_iter()
            .flat_map(|group| group.requests)
            .chain(first);
        for request in requests {
            let _ = request.send(Err(internal_err!("TSO stream failed: {}", reason)));
        }
    }
}

async fn run_tso(
    cluster_id: u64,
    mut connector: impl TsoConnector,
    request_rx: mpsc::Receiver<TimestampRequest>,
) {
    let request_rx: SharedReceiver = Arc::new(Mutex::new(request_rx));
    let mut backoff = TSO_RECONNECT_BACKOFF_MIN;
    // The request that woke the task up to reopen a failed stream; the new stream
    // sends it first.
    let mut first = None;
    loop {
        let state = StreamState::new(first.take());
        match serve_stream(cluster_id, &mut connector, &request_rx, &state).await {
            StreamEnd::RequestChannelClosed => {
                info!("TSO stream terminated: the timestamp oracle was dropped");
                return;
            }
            StreamEnd::Failed { reason, responded } => {
                state.fail(&reason);
                if responded {
                    backoff = TSO_RECONNECT_BACKOFF_MIN;
                }
                warn!(
                    "TSO stream failed: {}; reopening it in {:?}",
                    reason, backoff
                );
            }
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(TSO_RECONNECT_BACKOFF_MAX);

        // Reopen the stream only when a timestamp is needed, so an idle oracle does not
        // keep dialling an unavailable PD.
        let next = future::poll_fn(|cx| {
            request_rx
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .poll_recv(cx)
        })
        .await;
        match next {
            Some(request) => first = Some(request),
            None => {
                info!("TSO stream terminated: the timestamp oracle was dropped");
                return;
            }
        }
    }
}

enum StreamEnd {
    /// Every `TimestampOracle` handle was dropped.
    RequestChannelClosed,
    /// The stream could not be opened, broke, or PD answered out of protocol.
    /// `responded` is whether it delivered at least one response.
    Failed { reason: String, responded: bool },
}

async fn serve_stream(
    cluster_id: u64,
    connector: &mut impl TsoConnector,
    request_rx: &SharedReceiver,
    state: &Arc<StreamState>,
) -> StreamEnd {
    let request_stream = TsoRequestStream {
        cluster_id,
        request_rx: request_rx.clone(),
        state: state.clone(),
    };

    let mut responses = match connector.connect(request_stream).await {
        Ok(responses) => responses,
        Err(e) => {
            return StreamEnd::Failed {
                reason: format!("cannot open the TSO stream: {e}"),
                responded: false,
            }
        }
    };

    let mut responded = false;
    loop {
        match responses.next().await {
            Some(Ok(resp)) => {
                let allocated = allocate_timestamps(&resp, &mut state.lock_pending());
                if let Err(e) = allocated {
                    return StreamEnd::Failed {
                        reason: e.to_string(),
                        responded,
                    };
                }
                responded = true;
                // Wake up the request side blocked by too many pending requests.
                state.request_side_waker.wake();
            }
            Some(Err(status)) => {
                return StreamEnd::Failed {
                    reason: status.to_string(),
                    responded,
                }
            }
            None if state.request_channel_closed.load(Ordering::SeqCst) => {
                return StreamEnd::RequestChannelClosed
            }
            None => {
                return StreamEnd::Failed {
                    reason: "PD closed the TSO stream".to_owned(),
                    responded,
                }
            }
        }
    }
}

struct RequestGroup {
    tso_request: TsoRequest,
    requests: Vec<TimestampRequest>,
}

#[pin_project]
struct TsoRequestStream {
    cluster_id: u64,
    request_rx: SharedReceiver,
    state: Arc<StreamState>,
}

impl Stream for TsoRequestStream {
    type Item = TsoRequest;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let state = &**this.state;

        let mut pending_requests = state.lock_pending();
        if state.dead.load(Ordering::SeqCst) {
            // The background task has given up on this stream; leave the requests in
            // the channel for the next one.
            return Poll::Ready(None);
        }

        let mut requests = Vec::new();
        if let Some(first) = state.take_first_request() {
            requests.push(first);
        }

        {
            let mut request_rx = this.request_rx.lock().unwrap_or_else(|e| e.into_inner());
            while requests.len() < MAX_BATCH_SIZE && pending_requests.len() < MAX_PENDING_COUNT {
                match request_rx.poll_recv(cx) {
                    Poll::Ready(Some(sender)) => {
                        requests.push(sender);
                    }
                    Poll::Ready(None) if requests.is_empty() => {
                        state.request_channel_closed.store(true, Ordering::SeqCst);
                        return Poll::Ready(None);
                    }
                    _ => break,
                }
            }
        }

        if !requests.is_empty() {
            let req = TsoRequest {
                header: Some(RequestHeader {
                    cluster_id: *this.cluster_id,
                    ..Default::default()
                }),
                count: requests.len() as u32,
                dc_location: String::new(),
            };

            let request_group = RequestGroup {
                tso_request: req.clone(),
                requests,
            };
            pending_requests.push_back(request_group);

            Poll::Ready(Some(req))
        } else {
            // Set the waker to the context, then the stream can be waked up after the pending queue
            // is no longer full.
            state.request_side_waker.register(cx.waker());
            Poll::Pending
        }
    }
}

fn allocate_timestamps(
    resp: &TsoResponse,
    pending_requests: &mut VecDeque<RequestGroup>,
) -> Result<()> {
    // PD returns the timestamp with the biggest logical value. We can send back timestamps
    // whose logical value is from `logical - count + 1` to `logical` using the senders
    // in `pending`.
    let tail_ts = resp
        .timestamp
        .as_ref()
        .ok_or_else(|| internal_err!("No timestamp in TsoResponse"))?;

    let mut offset = resp.count;
    if let Some(RequestGroup {
        tso_request,
        requests,
    }) = pending_requests.pop_front()
    {
        if tso_request.count != offset {
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
            let _ = request.send(Ok(ts));
        }
    } else {
        return Err(internal_err!("PD gives more TsoResponse than expected"));
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use futures::channel::mpsc as fmpsc;
    use tokio::time::timeout;

    use super::*;

    /// How one fake TSO stream behaves.
    #[derive(Clone, Copy)]
    enum Plan {
        /// Opening the stream fails.
        Refuse,
        /// Answers this many `TsoRequest`s, then fails the stream with `UNAVAILABLE`.
        AnswerThenFail(usize),
        /// Answers every request.
        Answer,
    }

    /// A fake PD: each `connect` takes the next plan (the last one repeats), and the
    /// logical clock is shared by all streams.
    #[derive(Clone)]
    struct FakePd {
        plans: Arc<Mutex<VecDeque<Plan>>>,
        connects: Arc<AtomicUsize>,
        logical: Arc<Mutex<i64>>,
        request_streams_ended: Arc<AtomicUsize>,
    }

    impl FakePd {
        fn new(plans: &[Plan]) -> FakePd {
            FakePd {
                plans: Arc::new(Mutex::new(plans.iter().copied().collect())),
                connects: Arc::new(AtomicUsize::new(0)),
                logical: Arc::new(Mutex::new(0)),
                request_streams_ended: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn next_plan(&self) -> Plan {
            let mut plans = self.plans.lock().unwrap();
            if plans.len() > 1 {
                plans.pop_front().unwrap()
            } else {
                *plans.front().unwrap()
            }
        }
    }

    #[async_trait]
    impl TsoConnector for FakePd {
        type Responses = fmpsc::UnboundedReceiver<std::result::Result<TsoResponse, tonic::Status>>;

        async fn connect(&mut self, mut requests: TsoRequestStream) -> Result<Self::Responses> {
            self.connects.fetch_add(1, Ordering::SeqCst);
            let mut answers = match self.next_plan() {
                Plan::Refuse => return Err(internal_err!("connection refused")),
                Plan::AnswerThenFail(n) => Some(n),
                Plan::Answer => None,
            };
            let (tx, rx) = fmpsc::unbounded();
            let pd = self.clone();
            tokio::spawn(async move {
                while let Some(req) = requests.next().await {
                    if answers == Some(0) {
                        let _ = tx.unbounded_send(Err(tonic::Status::unavailable("PD stalled")));
                        return;
                    }
                    answers = answers.map(|n| n - 1);
                    let logical = {
                        let mut logical = pd.logical.lock().unwrap();
                        *logical += req.count as i64;
                        *logical
                    };
                    let resp = TsoResponse {
                        count: req.count,
                        timestamp: Some(Timestamp {
                            physical: 1_000,
                            logical,
                            suffix_bits: 0,
                        }),
                        ..Default::default()
                    };
                    if tx.unbounded_send(Ok(resp)).is_err() {
                        return;
                    }
                }
                pd.request_streams_ended.fetch_add(1, Ordering::SeqCst);
            });
            Ok(rx)
        }
    }

    async fn ts(oracle: &TimestampOracle) -> Result<Timestamp> {
        timeout(Duration::from_secs(5), oracle.clone().get_timestamp())
            .await
            .expect("get_timestamp must not hang")
    }

    #[tokio::test]
    async fn requests_after_a_stream_failure_are_served_by_a_new_stream() {
        let pd = FakePd::new(&[Plan::AnswerThenFail(1), Plan::Answer]);
        let oracle = TimestampOracle::with_connector(1, pd.clone());

        let first = ts(&oracle).await.unwrap();
        // The stream fails on the second request: that request fails with the
        // stream's error instead of hanging.
        let err = ts(&oracle).await.unwrap_err();
        assert!(
            err.to_string().contains("TSO stream failed") && err.to_string().contains("PD stalled"),
            "unexpected error: {err}"
        );
        // Later requests are served by a new stream: the oracle is not dead, and it
        // never reports "TimestampRequest channel is closed".
        for _ in 0..3 {
            let next = ts(&oracle).await.unwrap();
            assert!(next.logical > first.logical);
        }
        assert_eq!(pd.connects.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_refused_stream_is_reopened_with_backoff() {
        let pd = FakePd::new(&[Plan::Refuse, Plan::Refuse, Plan::Answer]);
        let oracle = TimestampOracle::with_connector(1, pd.clone());

        // The first open is refused while nobody waits; each later attempt is made
        // for a waiting request. A refused attempt fails its request...
        let mut failures = 0;
        let ts = loop {
            match ts(&oracle).await {
                Ok(ts) => break ts,
                Err(e) => {
                    assert!(e.to_string().contains("connection refused"), "{e}");
                    failures += 1;
                    assert!(failures < 5, "the oracle never reconnected");
                }
            }
        };
        // ...and the third open serves it.
        assert_eq!(ts.logical, 1);
        assert_eq!(failures, 1);
        assert_eq!(pd.connects.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn concurrent_requests_survive_repeated_stream_failures() {
        let pd = FakePd::new(&[
            Plan::AnswerThenFail(3),
            Plan::AnswerThenFail(0),
            Plan::AnswerThenFail(5),
            Plan::Answer,
        ]);
        let oracle = TimestampOracle::with_connector(1, pd.clone());

        let tasks = (0..32).map(|_| {
            let oracle = oracle.clone();
            tokio::spawn(async move {
                // A caller retries a failed request, as `pd::RetryClient` does.
                for _ in 0..10 {
                    if let Ok(ts) = ts(&oracle).await {
                        return ts;
                    }
                }
                panic!("no timestamp after 10 attempts");
            })
        });
        let mut all = Vec::new();
        for task in tasks {
            all.push(task.await.unwrap().logical);
        }
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 32, "timestamps must be unique");
    }

    #[tokio::test]
    async fn the_background_task_ends_when_the_oracle_is_dropped() {
        let pd = FakePd::new(&[Plan::Answer]);
        let oracle = TimestampOracle::with_connector(1, pd.clone());
        ts(&oracle).await.unwrap();
        drop(oracle);
        timeout(Duration::from_secs(5), async {
            while pd.request_streams_ended.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the request stream must end once the oracle is dropped");
        assert_eq!(pd.connects.load(Ordering::SeqCst), 1);
    }
}
