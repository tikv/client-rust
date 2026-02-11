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
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use futures::prelude::*;
use futures::task::AtomicWaker;
use futures::task::Context;
use futures::task::Poll;
use log::debug;
use log::warn;
use pin_project::pin_project;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use crate::internal_err;
use crate::proto::pdpb::pd_client::PdClient;
use crate::proto::pdpb::*;
use crate::stats::observe_tso_batch;
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
}

impl TimestampOracle {
    pub(crate) fn new(cluster_id: u64, pd_client: &PdClient<Channel>) -> Result<TimestampOracle> {
        let pd_client = pd_client.clone();
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);

        // Start a background task to handle TSO requests and responses.
        // If it exits with an error, log it explicitly so root cause is preserved.
        tokio::spawn(async move {
            if let Err(err) = run_tso(cluster_id, pd_client, request_rx).await {
                warn!("TSO background task exited with error: {:?}", err);
            }
        });

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

async fn run_tso(
    cluster_id: u64,
    mut pd_client: PdClient<Channel>,
    request_rx: mpsc::Receiver<TimestampRequest>,
) -> Result<()> {
    // The `TimestampRequest`s which are waiting for the responses from the PD server
    let pending_requests = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));

    // When there are too many pending requests, the `send_request` future will refuse to fetch
    // more requests from the bounded channel. This waker is used to wake up the sending future
    // if the queue containing pending requests is no longer full.
    let sending_future_waker = Arc::new(AtomicWaker::new());
    // This flag indicates the sender stream could not acquire `pending_requests` lock in poll
    // and needs an explicit wake from the response path.
    let sender_waiting_on_lock = Arc::new(AtomicBool::new(false));

    let request_stream = TsoRequestStream {
        cluster_id,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
        sender_waiting_on_lock: sender_waiting_on_lock.clone(),
    };

    // let send_requests = rpc_sender.send_all(&mut request_stream);
    let mut responses = pd_client.tso(request_stream).await?.into_inner();

    while let Some(resp) = responses.next().await {
        let resp = resp?;
        let should_wake_sender = {
            let mut pending_requests = pending_requests.lock().await;
            let was_full = pending_requests.len() >= MAX_PENDING_COUNT;
            allocate_timestamps(&resp, &mut pending_requests)?;
            was_full && pending_requests.len() < MAX_PENDING_COUNT
        };
        let sender_blocked_by_lock = sender_waiting_on_lock.swap(false, Ordering::SeqCst);

        // Wake sender when:
        // 1. a previously full queue gains capacity, or
        // 2. sender was blocked on `pending_requests` mutex contention.
        if should_wake_sender || sender_blocked_by_lock {
            sending_future_waker.wake();
        }
    }
    let pending_count = pending_requests.lock().await.len();
    if pending_count == 0 {
        Ok(())
    } else {
        Err(internal_err!(
            "TSO stream terminated with {} pending requests",
            pending_count
        ))
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
    sender_waiting_on_lock: Arc<AtomicBool>,
}

impl Stream for TsoRequestStream {
    type Item = TsoRequest;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        let mut pending_requests = if let Ok(pending_requests) = this.pending_requests.try_lock() {
            this.sender_waiting_on_lock.store(false, Ordering::SeqCst);
            pending_requests
        } else {
            let pending_requests = register_sender_wait_for_pending_lock(
                cx,
                this.self_waker.as_ref(),
                this.sender_waiting_on_lock.as_ref(),
                || this.pending_requests.try_lock().ok(),
            );

            if let Some(pending_requests) = pending_requests {
                pending_requests
            } else {
                return Poll::Pending;
            }
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
            observe_tso_batch(requests.len());
            let req = TsoRequest {
                header: Some(RequestHeader {
                    cluster_id: *this.cluster_id,
                    sender_id: 0,
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
            // Register self waker only when blocked by a full pending queue.
            // When queue is not full, poll_recv above has already registered the receiver waker.
            if pending_requests.len() >= MAX_PENDING_COUNT {
                this.self_waker.register(cx.waker());
            }
            Poll::Pending
        }
    }
}

fn register_sender_wait_for_pending_lock<F, G>(
    cx: &mut Context<'_>,
    self_waker: &AtomicWaker,
    sender_waiting_on_lock: &AtomicBool,
    mut try_lock_after_register: F,
) -> Option<G>
where
    F: FnMut() -> Option<G>,
{
    // Register first so a wake from the response path targets the current task.
    self_waker.register(cx.waker());
    sender_waiting_on_lock.store(true, Ordering::SeqCst);

    // Retry once after advertising waiting to close the race where the response path
    // already checked/cleared the flag before this store and therefore will not wake.
    if let Some(guard) = try_lock_after_register() {
        sender_waiting_on_lock.store(false, Ordering::SeqCst);
        Some(guard)
    } else {
        None
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
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    use futures::executor::block_on;
    use futures::task::noop_waker_ref;
    use futures::task::waker;
    use futures::task::ArcWake;

    use super::*;

    struct WakeCounter {
        wakes: AtomicUsize,
    }

    impl ArcWake for WakeCounter {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn test_tso_request(count: u32) -> TsoRequest {
        TsoRequest {
            header: Some(RequestHeader {
                cluster_id: 1,
                sender_id: 0,
            }),
            count,
            dc_location: String::new(),
        }
    }

    fn test_tso_response(count: u32, logical: i64) -> TsoResponse {
        TsoResponse {
            header: None,
            count,
            timestamp: Some(Timestamp {
                physical: 123,
                logical,
                suffix_bits: 0,
            }),
        }
    }

    type TestStreamContext = (
        TsoRequestStream,
        mpsc::Sender<TimestampRequest>,
        Arc<Mutex<VecDeque<RequestGroup>>>,
        Arc<AtomicWaker>,
        Arc<AtomicBool>,
    );

    fn new_test_stream() -> TestStreamContext {
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);
        let pending_requests = Arc::new(Mutex::new(VecDeque::new()));
        let self_waker = Arc::new(AtomicWaker::new());
        let sender_waiting_on_lock = Arc::new(AtomicBool::new(false));
        let stream = TsoRequestStream {
            cluster_id: 1,
            request_rx,
            pending_requests: pending_requests.clone(),
            self_waker: self_waker.clone(),
            sender_waiting_on_lock: sender_waiting_on_lock.clone(),
        };
        (
            stream,
            request_tx,
            pending_requests,
            self_waker,
            sender_waiting_on_lock,
        )
    }

    #[test]
    fn allocate_timestamps_successfully_assigns_monotonic_timestamps() {
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();
        let mut pending_requests = VecDeque::new();
        pending_requests.push_back(RequestGroup {
            tso_request: test_tso_request(3),
            requests: vec![tx1, tx2, tx3],
        });

        allocate_timestamps(&test_tso_response(3, 100), &mut pending_requests).unwrap();
        assert!(pending_requests.is_empty());

        let ts1 = block_on(rx1).unwrap();
        let ts2 = block_on(rx2).unwrap();
        let ts3 = block_on(rx3).unwrap();
        assert_eq!(ts1.logical, 98);
        assert_eq!(ts2.logical, 99);
        assert_eq!(ts3.logical, 100);
    }

    #[test]
    fn allocate_timestamps_errors_without_timestamp() {
        let (tx, _rx) = oneshot::channel();
        let mut pending_requests = VecDeque::new();
        pending_requests.push_back(RequestGroup {
            tso_request: test_tso_request(1),
            requests: vec![tx],
        });
        let resp = TsoResponse {
            header: None,
            count: 1,
            timestamp: None,
        };

        let err = allocate_timestamps(&resp, &mut pending_requests).unwrap_err();
        assert!(format!("{err:?}").contains("No timestamp in TsoResponse"));
    }

    #[test]
    fn allocate_timestamps_errors_when_count_mismatches() {
        let (tx, _rx) = oneshot::channel();
        let mut pending_requests = VecDeque::new();
        pending_requests.push_back(RequestGroup {
            tso_request: test_tso_request(2),
            requests: vec![tx],
        });

        let err =
            allocate_timestamps(&test_tso_response(1, 10), &mut pending_requests).unwrap_err();
        assert!(format!("{err:?}").contains("different number of timestamps"));
    }

    #[test]
    fn allocate_timestamps_errors_on_extra_response() {
        let mut pending_requests = VecDeque::new();
        let err =
            allocate_timestamps(&test_tso_response(1, 10), &mut pending_requests).unwrap_err();
        assert!(format!("{err:?}").contains("more TsoResponse than expected"));
    }

    #[test]
    fn poll_next_emits_request_and_enqueues_request_group() {
        let (stream, request_tx, pending_requests, _self_waker, sender_waiting_on_lock) =
            new_test_stream();
        let (tx, _rx) = oneshot::channel();
        request_tx.try_send(tx).unwrap();

        let mut stream = Box::pin(stream);
        let mut cx = Context::from_waker(noop_waker_ref());
        let polled = stream.as_mut().poll_next(&mut cx);
        let req = match polled {
            Poll::Ready(Some(req)) => req,
            other => panic!("expected Poll::Ready(Some(_)), got {:?}", other),
        };

        assert_eq!(req.count, 1);
        assert!(!sender_waiting_on_lock.load(Ordering::SeqCst));
        let queued = block_on(async { pending_requests.lock().await.len() });
        assert_eq!(queued, 1);
    }

    #[test]
    fn poll_next_registers_self_waker_when_pending_queue_is_full() {
        let (stream, _request_tx, pending_requests, self_waker, _sender_waiting_on_lock) =
            new_test_stream();
        block_on(async {
            let mut guard = pending_requests.lock().await;
            for _ in 0..MAX_PENDING_COUNT {
                guard.push_back(RequestGroup {
                    tso_request: test_tso_request(0),
                    requests: Vec::new(),
                });
            }
        });

        let wake_counter = Arc::new(WakeCounter {
            wakes: AtomicUsize::new(0),
        });
        let test_waker = waker(wake_counter.clone());
        let mut cx = Context::from_waker(&test_waker);
        let mut stream = Box::pin(stream);

        let polled = stream.as_mut().poll_next(&mut cx);
        assert!(matches!(polled, Poll::Pending));
        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 0);

        self_waker.wake();
        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn poll_next_marks_waiting_flag_when_lock_is_contended_and_response_wakes() {
        let (stream, _request_tx, pending_requests, self_waker, sender_waiting_on_lock) =
            new_test_stream();
        let lock_guard = block_on(pending_requests.lock());

        let wake_counter = Arc::new(WakeCounter {
            wakes: AtomicUsize::new(0),
        });
        let test_waker = waker(wake_counter.clone());
        let mut cx = Context::from_waker(&test_waker);
        let mut stream = Box::pin(stream);

        let polled = stream.as_mut().poll_next(&mut cx);
        assert!(matches!(polled, Poll::Pending));
        assert!(sender_waiting_on_lock.load(Ordering::SeqCst));

        // Simulate response path: swap flag and wake.
        drop(lock_guard);
        if sender_waiting_on_lock.swap(false, Ordering::SeqCst) {
            self_waker.wake();
        }
        assert!(wake_counter.wakes.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn register_sender_wait_sets_waiting_flag_and_registers_waker_on_retry_failure() {
        let self_waker = AtomicWaker::new();
        let sender_waiting_on_lock = AtomicBool::new(false);
        let wake_counter = Arc::new(WakeCounter {
            wakes: AtomicUsize::new(0),
        });
        let test_waker = waker(wake_counter.clone());
        let mut cx = Context::from_waker(&test_waker);

        let reacquired = register_sender_wait_for_pending_lock(
            &mut cx,
            &self_waker,
            &sender_waiting_on_lock,
            || None::<()>,
        );
        assert!(reacquired.is_none());
        assert!(sender_waiting_on_lock.load(Ordering::SeqCst));

        self_waker.wake();
        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn register_sender_wait_retries_once_and_clears_waiting_flag_when_lock_reacquires() {
        let self_waker = AtomicWaker::new();
        let sender_waiting_on_lock = AtomicBool::new(false);
        let mut retry_count = 0;
        let wake_counter = Arc::new(WakeCounter {
            wakes: AtomicUsize::new(0),
        });
        let test_waker = waker(wake_counter.clone());
        let mut cx = Context::from_waker(&test_waker);

        // Simulates the lost-wake interleaving: initial lock contention, then lock
        // becomes available before any response-side wake.
        let reacquired = register_sender_wait_for_pending_lock(
            &mut cx,
            &self_waker,
            &sender_waiting_on_lock,
            || {
                retry_count += 1;
                Some(())
            },
        );
        assert_eq!(retry_count, 1);
        assert!(reacquired.is_some());
        assert!(!sender_waiting_on_lock.load(Ordering::SeqCst));
        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 0);
    }

    /// After acquiring the lock, the waiting flag must be cleared.
    #[test]
    fn poll_next_clears_waiting_flag_on_lock_acquire() {
        let (stream, request_tx, _pending_requests, _self_waker, sender_waiting_on_lock) =
            new_test_stream();
        // Pre-set the flag as if a previous poll was contended.
        sender_waiting_on_lock.store(true, Ordering::SeqCst);

        let (tx, _rx) = oneshot::channel();
        request_tx.try_send(tx).unwrap();

        let mut stream = Box::pin(stream);
        let mut cx = Context::from_waker(noop_waker_ref());
        let polled = stream.as_mut().poll_next(&mut cx);
        assert!(matches!(polled, Poll::Ready(Some(_))));
        // Flag must be cleared after successful lock acquisition.
        assert!(!sender_waiting_on_lock.load(Ordering::SeqCst));
    }

    /// When queue is not full and no requests available, poll_next should not
    /// register the self_waker (the channel waker handles this case).
    #[test]
    fn poll_next_does_not_register_self_waker_when_queue_not_full() {
        let (stream, _request_tx, _pending_requests, self_waker, _sender_waiting_on_lock) =
            new_test_stream();

        let wake_counter = Arc::new(WakeCounter {
            wakes: AtomicUsize::new(0),
        });
        let test_waker = waker(wake_counter.clone());
        let mut cx = Context::from_waker(&test_waker);
        let mut stream = Box::pin(stream);

        // No requests in channel, queue empty -> Pending.
        let polled = stream.as_mut().poll_next(&mut cx);
        assert!(matches!(polled, Poll::Pending));

        // self_waker.wake() should NOT reach our waker because the self_waker
        // was not registered (queue is not full).
        self_waker.wake();
        assert_eq!(wake_counter.wakes.load(Ordering::SeqCst), 0);
    }
}
