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
use std::time::Duration;

use futures::pin_mut;
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
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tonic::transport::Channel;

use crate::internal_err;
use crate::proto::pdpb::pd_client::PdClient;
use crate::proto::pdpb::*;
use crate::Result;

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
    /// Closed (its sender dropped) when the TSO worker exits, however it exits.
    /// `get_timestamp` watches this so its callers fail fast when the worker is
    /// gone. Relying on the worker's teardown to drop the request channel is
    /// not enough: the channel is owned by the gRPC request body inside the
    /// connection's task, and on a frozen connection that task may not run
    /// again — releasing nothing — until the connection thaws.
    worker_closed: watch::Receiver<()>,
}

impl TimestampOracle {
    /// `timeout` bounds every wait against the PD server: stream creation, and
    /// each silent window of the response stream while request batches are
    /// outstanding (see [`allocate_from_stream`]). It is the client's
    /// configured request timeout (`Config::timeout`), whose contract covers
    /// requests to PD nodes. A healthy PD answers a TSO request in
    /// milliseconds, but a stream can go silent without terminating — the
    /// peer's VM paused by its hypervisor, the process frozen under resource
    /// starvation, a middlebox silently discarding the connection — and
    /// nothing else in this path bounds the wait, so a request could otherwise
    /// hang forever (#516). client-go bounds its TSO batches the same way: the
    /// dispatcher arms a deadline watcher per batch with `defaultPDTimeout` =
    /// 3s and cancels the stream on expiry (pd/client/clients/tso/dispatcher.go),
    /// and bounds stream creation via `checkStreamTimeout`
    /// (pd/client/clients/tso/stream.go).
    pub(crate) fn new(
        cluster_id: u64,
        pd_client: &PdClient<Channel>,
        timeout: Duration,
    ) -> Result<TimestampOracle> {
        let pd_client = pd_client.clone();
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);
        let (close_on_exit, worker_closed) = watch::channel(());

        // Start a background thread to handle TSO requests and responses
        tokio::spawn(async move {
            let _close_on_exit = close_on_exit;
            run_tso(cluster_id, pd_client, request_rx, timeout).await
        });

        Ok(TimestampOracle {
            request_tx,
            worker_closed,
        })
    }

    pub(crate) async fn get_timestamp(mut self) -> Result<Timestamp> {
        debug!("getting current timestamp");
        let (request, response) = oneshot::channel();
        // The worker's death must be observed while ENQUEUEING too: a dead
        // worker's request channel can remain open (held by the frozen
        // connection's task) and, once its buffer fills, `send` would block
        // forever without ever failing.
        tokio::select! {
            biased;
            sent = self.request_tx.send(request) => {
                sent.map_err(|_| internal_err!("TimestampRequest channel is closed"))?
            }
            _ = self.worker_closed.changed() => {
                return Err(internal_err!("the TSO worker terminated"));
            }
        }
        tokio::select! {
            // Deliver a timestamp that is already there even if the worker died
            // in the same instant.
            biased;
            res = response => Ok(res?),
            _ = self.worker_closed.changed() => {
                Err(internal_err!("the TSO worker terminated"))
            }
        }
    }
}

async fn run_tso(
    cluster_id: u64,
    mut pd_client: PdClient<Channel>,
    request_rx: mpsc::Receiver<TimestampRequest>,
    deadline: Duration,
) -> Result<()> {
    // The `TimestampRequest`s which are waiting for the responses from the PD server
    let pending_requests = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));

    // When there are too many pending requests, the `send_request` future will refuse to fetch
    // more requests from the bounded channel. This waker is used to wake up the sending future
    // if the queue containing pending requests is no longer full.
    let sending_future_waker = Arc::new(AtomicWaker::new());

    let request_stream = TsoRequestStream {
        cluster_id,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
    };

    let result = async {
        // Stream creation needs the same bound as the receive loop, but ARMED
        // ONLY WHILE A BATCH IS PENDING: PD (a gRPC-go server) sends response
        // headers with its first response, so `tso(..)` legitimately waits as
        // long as the client is idle — while against a frozen peer it would
        // hang with a request owed (client-go: `checkStreamTimeout`). HTTP/2
        // lets the request stream flow before response headers arrive, so
        // batches do get dispatched (and become pending) during this await.
        let responses = tokio::select! {
            created = pd_client.tso(request_stream) => created?.into_inner(),
            stalled = stall_watchdog(&pending_requests, deadline) => return Err(stalled),
        };

        allocate_from_stream(
            responses,
            &pending_requests,
            &sending_future_waker,
            deadline,
        )
        .await
    }
    .await;

    match &result {
        // The stream ended with nothing owed — e.g. the client is shutting down.
        Ok(()) => info!("TSO stream terminated"),
        // Failed to come up, stalled, or terminated with batches outstanding.
        // The worker's exit closes the oracle's `worker_closed` watch, failing
        // every waiting and future request fast, and the retry layer reconnects.
        Err(e) => warn!("TSO stream failed: {}", e),
    }
    result
}

/// Resolve (never `Ok`) once a full `deadline` window has passed with a batch
/// pending the whole time and no progress possible — the stall signal for
/// stream creation, where no response can arrive by definition. The same
/// [1, 2]-window guarantee as [`allocate_from_stream`]: a batch dispatched
/// mid-window is granted the following full window.
async fn stall_watchdog(
    pending_requests: &Mutex<VecDeque<RequestGroup>>,
    deadline: Duration,
) -> crate::Error {
    loop {
        let pending_at_window_start = !pending_requests.lock().await.is_empty();
        tokio::time::sleep(deadline).await;
        if pending_at_window_start && !pending_requests.lock().await.is_empty() {
            return internal_err!(
                "TSO stream creation stalled: no stream within {:?} with requests pending",
                deadline
            );
        }
    }
}

/// Drive the TSO response stream, allocating timestamps to the pending request
/// batches, until the stream ends or goes silent past `deadline` with batches
/// outstanding.
///
/// The deadline is evaluated in whole windows: a batch dispatched near the end
/// of a silent window is granted one more full window before the stream is
/// declared stalled, so teardown happens only after between one and two
/// deadlines of genuine silence. (client-go stamps a deadline per batch at
/// dispatch time; windows approximate that without tracking send times.)
async fn allocate_from_stream(
    mut responses: impl Stream<Item = std::result::Result<TsoResponse, tonic::Status>> + Unpin,
    pending_requests: &Mutex<VecDeque<RequestGroup>>,
    sending_future_waker: &AtomicWaker,
    deadline: Duration,
) -> Result<()> {
    loop {
        let pending_at_window_start = !pending_requests.lock().await.is_empty();
        match timeout(deadline, responses.next()).await {
            Ok(Some(Ok(resp))) => {
                {
                    let mut pending_requests = pending_requests.lock().await;
                    allocate_timestamps(&resp, &mut pending_requests)?;
                }
                // Wake up the sending future blocked by too many pending requests or locked.
                sending_future_waker.wake();
            }
            Ok(Some(Err(status))) => return Err(status.into()),
            Ok(None) => {
                let outstanding = pending_requests.lock().await.len();
                if outstanding == 0 {
                    return Ok(());
                }
                return Err(internal_err!(
                    "TSO stream terminated with {} batches outstanding",
                    outstanding
                ));
            }
            Err(_elapsed) => {
                let outstanding = pending_requests.lock().await.len();
                if outstanding == 0 {
                    // A quiet stream with nothing owed is healthy idleness.
                    continue;
                }
                if pending_at_window_start {
                    // Owed a response for the entire silent window: stalled.
                    return Err(internal_err!(
                        "TSO stream stalled: no response within {:?} with {} batches outstanding",
                        deadline,
                        outstanding
                    ));
                }
                // The batch was dispatched mid-window; grant it one full window
                // (the next iteration starts with it pending).
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
    #[pin]
    request_rx: mpsc::Receiver<oneshot::Sender<Timestamp>>,
    pending_requests: Arc<Mutex<VecDeque<RequestGroup>>>,
    self_waker: Arc<AtomicWaker>,
}

impl Stream for TsoRequestStream {
    type Item = TsoRequest;

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
            this.self_waker.register(cx.waker());
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
            let _ = request.send(ts);
        }
    } else {
        return Err(internal_err!("PD gives more TsoResponse than expected"));
    };
    Ok(())
}

#[cfg(test)]
mod tests {

    use futures::stream;

    use super::*;

    const DEADLINE: Duration = Duration::from_secs(3);

    fn group_of_one() -> (RequestGroup, oneshot::Receiver<Timestamp>) {
        let (tx, rx) = oneshot::channel();
        let group = RequestGroup {
            tso_request: TsoRequest {
                count: 1,
                ..Default::default()
            },
            requests: vec![tx],
        };
        (group, rx)
    }

    /// The #516 hang: a stream that never yields while a batch is owed. The
    /// deadline must tear the loop down — and the paused clock proves it takes
    /// between one and two deadline windows, not forever.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_stream_with_batches_outstanding_fails_within_two_deadlines() {
        let pending = Mutex::new(VecDeque::new());
        let (group, _rx) = group_of_one();
        pending.lock().await.push_back(group);
        let waker = AtomicWaker::new();

        let t0 = tokio::time::Instant::now();
        let result = allocate_from_stream(stream::pending(), &pending, &waker, DEADLINE).await;
        let elapsed = t0.elapsed();

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("stalled"),
            "want the stall error, got {err:?}"
        );
        assert!(
            elapsed >= DEADLINE && elapsed <= DEADLINE * 2,
            "want teardown within [1, 2] deadlines, took {elapsed:?}"
        );
    }

    /// A batch dispatched mid-window must not be torn down at that window's
    /// end — it gets one full window of its own.
    #[tokio::test(start_paused = true)]
    async fn a_batch_dispatched_mid_window_gets_a_full_window() {
        let pending = Arc::new(Mutex::new(VecDeque::new()));
        let waker = Arc::new(AtomicWaker::new());

        let task = {
            let pending = pending.clone();
            let waker = waker.clone();
            tokio::spawn(async move {
                let t0 = tokio::time::Instant::now();
                let result =
                    allocate_from_stream(stream::pending(), &pending, &waker, DEADLINE).await;
                (result, t0.elapsed())
            })
        };

        // Dispatch the batch halfway through the first (idle) window.
        tokio::time::sleep(DEADLINE / 2).await;
        let (group, _rx) = group_of_one();
        pending.lock().await.push_back(group);

        let (result, elapsed) = task.await.unwrap();
        assert!(result.is_err());
        assert!(
            elapsed >= DEADLINE * 2,
            "the mid-window batch must get a full window before teardown, got {elapsed:?}"
        );
    }

    /// Healthy idleness — no batches owed — must never be torn down, no matter
    /// how long the stream stays quiet.
    #[tokio::test(start_paused = true)]
    async fn an_idle_stream_is_left_alone() {
        let pending = Mutex::new(VecDeque::new());
        let waker = AtomicWaker::new();

        let outcome = timeout(
            DEADLINE * 20,
            allocate_from_stream(stream::pending(), &pending, &waker, DEADLINE),
        )
        .await;
        assert!(
            outcome.is_err(),
            "an idle stream must keep waiting, but the loop returned {outcome:?}"
        );
    }

    /// Termination with batches owed is a failure (the callers' requests are
    /// dropped and fail fast); termination when idle is a clean end.
    #[tokio::test]
    async fn termination_is_clean_only_when_nothing_is_owed() {
        let pending = Mutex::new(VecDeque::new());
        let waker = AtomicWaker::new();
        let empty: Vec<std::result::Result<TsoResponse, tonic::Status>> = vec![];
        let result = allocate_from_stream(stream::iter(empty), &pending, &waker, DEADLINE).await;
        assert!(result.is_ok());

        let (group, _rx) = group_of_one();
        pending.lock().await.push_back(group);
        let empty: Vec<std::result::Result<TsoResponse, tonic::Status>> = vec![];
        let err = allocate_from_stream(stream::iter(empty), &pending, &waker, DEADLINE)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("terminated"),
            "want the terminated-with-outstanding error, got {err:?}"
        );
    }

    /// A status error carried by the stream surfaces as-is instead of being
    /// silently swallowed (which the old loop did).
    #[tokio::test]
    async fn a_stream_error_propagates() {
        let pending = Mutex::new(VecDeque::new());
        let waker = AtomicWaker::new();
        let items = vec![Err(tonic::Status::unavailable("pd gone"))];
        let err = allocate_from_stream(stream::iter(items), &pending, &waker, DEADLINE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, crate::Error::GrpcAPI(_)),
            "want the tonic status surfaced, got {err:?}"
        );
    }

    /// A client that is merely idle after construction must keep its worker:
    /// PD sends response headers only with its first response, so stream
    /// creation legitimately waits while nothing is owed. The watchdog must
    /// never fire with the pending queue empty.
    #[tokio::test(start_paused = true)]
    async fn an_idle_worker_is_not_torn_down_during_stream_creation() {
        let pending = Mutex::new(VecDeque::new());
        let outcome = timeout(DEADLINE * 20, stall_watchdog(&pending, DEADLINE)).await;
        assert!(
            outcome.is_err(),
            "the watchdog must keep waiting while nothing is owed, got {outcome:?}"
        );
    }

    /// With a batch owed and no stream forthcoming, the watchdog fires within
    /// the same [1, 2]-window guarantee as the receive loop.
    #[tokio::test(start_paused = true)]
    async fn the_creation_watchdog_fires_with_a_batch_pending() {
        let pending = Mutex::new(VecDeque::new());
        let (group, _rx) = group_of_one();
        pending.lock().await.push_back(group);

        let t0 = tokio::time::Instant::now();
        let err = stall_watchdog(&pending, DEADLINE).await;
        let elapsed = t0.elapsed();
        assert!(err.to_string().contains("creation stalled"));
        assert!(
            elapsed >= DEADLINE && elapsed <= DEADLINE * 2,
            "want the watchdog within [1, 2] windows, took {elapsed:?}"
        );
    }

    /// Enqueueing must observe worker death too: a dead worker's request
    /// channel can stay open (held by a frozen connection's task) with a full
    /// buffer, and `send` would otherwise block forever.
    #[tokio::test]
    async fn enqueueing_observes_worker_death() {
        let (request_tx, request_rx) = mpsc::channel(1);
        let (close_on_exit, worker_closed) = watch::channel(());
        // The buffer is full and the receiver alive-but-unpolled: the zombie.
        let (dummy, _dummy_rx) = oneshot::channel();
        request_tx.try_send(dummy).unwrap();
        // The worker is dead.
        drop(close_on_exit);

        let oracle = TimestampOracle {
            request_tx,
            worker_closed,
        };
        let outcome = timeout(Duration::from_secs(30), oracle.get_timestamp())
            .await
            .expect("a full queue on a dead worker must fail, not block");
        assert!(outcome.is_err());
        drop(request_rx);
    }

    /// The oracle's callers must fail fast when the worker is gone — the
    /// worker's teardown cannot rely on dropping the request channel (a frozen
    /// connection's task may hold it), so `get_timestamp` watches the worker's
    /// exit directly. A lazy channel to a closed port makes the worker die at
    /// stream creation.
    #[tokio::test]
    async fn get_timestamp_fails_fast_when_the_worker_dies() {
        let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
        let pd_client = PdClient::new(channel);
        let oracle = TimestampOracle::new(1, &pd_client, DEADLINE).unwrap();

        let outcome = timeout(Duration::from_secs(30), oracle.get_timestamp())
            .await
            .expect("a request against a dead worker must fail, not hang");
        assert!(outcome.is_err());
    }

    /// The happy path: a response allocates a timestamp to the pending batch,
    /// and a subsequent clean end reports Ok.
    #[tokio::test]
    async fn responses_allocate_timestamps_to_pending_batches() {
        let pending = Mutex::new(VecDeque::new());
        let (group, rx) = group_of_one();
        pending.lock().await.push_back(group);
        let waker = AtomicWaker::new();

        let items = vec![Ok(TsoResponse {
            count: 1,
            timestamp: Some(Timestamp {
                physical: 7,
                logical: 42,
                ..Default::default()
            }),
            ..Default::default()
        })];
        let result = allocate_from_stream(stream::iter(items), &pending, &waker, DEADLINE).await;
        assert!(result.is_ok(), "clean end after all batches answered");

        let ts = rx
            .await
            .expect("the pending request must receive its timestamp");
        assert_eq!((ts.physical, ts.logical), (7, 42));
    }
}
