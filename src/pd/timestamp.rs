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

use crate::{transaction::Timestamp, Error, Result};

use futures::{
    channel::{mpsc, oneshot},
    compat::*,
    executor::block_on,
    join, pin_mut,
    prelude::*,
    task::{AtomicWaker, Context, Poll},
};
use grpcio::WriteFlags;
use kvproto::pdpb::*;
use std::{cell::RefCell, collections::VecDeque, pin::Pin, rc::Rc, thread};

/// It is an empirical value.
const MAX_BATCH_SIZE: usize = 64;

/// TODO: This value should be adjustable.
const MAX_PENDING_COUNT: usize = 1 << 16;

type TimestampRequest = oneshot::Sender<Timestamp>;

/// The timestamp oracle (TSO) which provides monotonically increasing timestamps.
#[derive(Clone)]
pub struct TimestampOracle {
    /// The transmitter of a bounded channel which transports requests of getting a single
    /// timestamp to the TSO working thread. A bounded channel is used to prevent using
    /// too much memory unexpectedly.
    /// In the working thread, the `TimestampRequest`, which is actually a one channel sender,
    /// is used to send back the timestamp result.
    request_tx: mpsc::Sender<TimestampRequest>,
}

impl TimestampOracle {
    pub fn new(cluster_id: u64, pd_client: &PdClient) -> Result<TimestampOracle> {
        let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);
        // FIXME: use tso_opt
        let (rpc_sender, rpc_receiver) = pd_client.tso()?;

        // Start a background thread to handle TSO requests and responses
        thread::spawn(move || {
            block_on(run_tso(
                cluster_id,
                rpc_sender.sink_compat().sink_err_into(),
                rpc_receiver.compat().err_into(),
                request_rx,
            ))
        });

        Ok(TimestampOracle { request_tx })
    }

    pub async fn get_timestamp(mut self) -> Result<Timestamp> {
        let (request, response) = oneshot::channel();
        self.request_tx
            .send(request)
            .await
            .map_err(|_| Error::internal_error("TimestampRequest channel is closed"))?;
        Ok(response.await?)
    }
}

async fn run_tso(
    cluster_id: u64,
    mut rpc_sender: impl Sink<(TsoRequest, WriteFlags), Error = Error> + Unpin,
    mut rpc_receiver: impl Stream<Item = Result<TsoResponse>> + Unpin,
    request_rx: mpsc::Receiver<TimestampRequest>,
) {
    // The `TimestampRequest`s which are waiting for the responses from the PD server
    let pending_requests = Rc::new(RefCell::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));

    // When there are too many pending requests, the `send_request` future will refuse to fetch
    // more requests from the bounded channel. This waker is used to wake up the sending future
    // if the queue containing pending requests is no longer full.
    let sending_future_waker = Rc::new(AtomicWaker::new());

    pin_mut!(request_rx);
    let mut request_stream = TsoRequestStream {
        cluster_id,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
    };

    let send_requests = rpc_sender.send_all(&mut request_stream);

    let receive_and_handle_responses = async move {
        while let Some(Ok(resp)) = rpc_receiver.next().await {
            let mut pending_requests = pending_requests.borrow_mut();

            // Wake up the sending future blocked by too many pending requests as we are consuming
            // some of them here.
            if pending_requests.len() == MAX_PENDING_COUNT {
                sending_future_waker.wake();
            }

            allocate_timestamps(&resp, &mut pending_requests)?;
        }
        Err(Error::internal_error("TSO stream terminated"))
    };

    let (send_res, recv_res): (_, Result<()>) = join!(send_requests, receive_and_handle_responses);
    error!("TSO send error: {:?}", send_res);
    error!("TSO receive error: {:?}", recv_res);
}

struct TsoRequestStream<'a> {
    cluster_id: u64,
    request_rx: Pin<&'a mut mpsc::Receiver<oneshot::Sender<Timestamp>>>,
    pending_requests: Rc<RefCell<VecDeque<TimestampRequest>>>,
    self_waker: Rc<AtomicWaker>,
}

impl<'a> Stream for TsoRequestStream<'a> {
    type Item = (TsoRequest, WriteFlags);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let pending_requests = self.pending_requests.clone();
        let mut pending_requests = pending_requests.borrow_mut();
        let mut count = 0;
        while count < MAX_BATCH_SIZE && pending_requests.len() < MAX_PENDING_COUNT {
            match self.request_rx.as_mut().poll_next(cx) {
                Poll::Ready(Some(sender)) => {
                    pending_requests.push_back(sender);
                    count += 1;
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => break,
            }
        }

        if count > 0 {
            let req = TsoRequest {
                header: Some(RequestHeader {
                    cluster_id: self.cluster_id,
                }),
                count: count as u32,
            };
            let write_flags = WriteFlags::default().buffer_hint(false);
            Poll::Ready(Some((req, write_flags)))
        } else {
            // Set the waker to the context, then the stream can be waked up after the pending queue
            // is no longer full.
            self.self_waker.register(cx.waker());
            Poll::Pending
        }
    }
}

fn allocate_timestamps(
    resp: &TsoResponse,
    pending_requests: &mut VecDeque<TimestampRequest>,
) -> Result<()> {
    // PD returns the timestamp with the biggest logical value. We can send back timestamps
    // whose logical value is from `logical - count + 1` to `logical` using the senders
    // in `pending`.
    let tail_ts = resp
        .timestamp
        .as_ref()
        .ok_or_else(|| Error::internal_error("No timestamp in TsoResponse"))?;

    let mut offset = i64::from(resp.count);
    while offset > 0 {
        offset -= 1;
        if let Some(sender) = pending_requests.pop_front() {
            let ts = Timestamp {
                physical: tail_ts.physical,
                logical: tail_ts.logical - offset,
            };

            // It doesn't matter if the receiver of the channel is dropped.
            let _ = sender.send(ts);
        } else {
            return Err(Error::internal_error(
                "PD gives more timestamps than expected",
            ));
        }
    }
    Ok(())
}
