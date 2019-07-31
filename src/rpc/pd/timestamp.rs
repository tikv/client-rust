// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This module is the low-level mechanisms for getting timestamps from a PD
//! cluster. It should be used via the `get_timestamp` API in `PdClient`.

use super::Timestamp;
use crate::{Error, Result};

use futures::{
    channel::{mpsc, oneshot},
    compat::*,
    executor::block_on,
    join, pin_mut,
    prelude::*,
    task::{Context, Poll, Waker},
};
use grpcio::{ClientDuplexReceiver, ClientDuplexSender, WriteFlags};
use kvproto::pdpb::{PdClient, *};
use std::{cell::RefCell, collections::VecDeque, pin::Pin, rc::Rc, thread};

const MAX_PENDING_COUNT: usize = 64;

/// The timestamp oracle which provides monotonically increasing timestamps.
#[derive(Clone)]
pub struct Tso {
    /// The transmitter of a bounded channel which transports the sender of an oneshot channel to
    /// the TSO working thread.
    /// In the working thread, the `oneshot::Sender` is used to send back timestamp results.
    result_sender_tx: mpsc::Sender<oneshot::Sender<Timestamp>>,
}

impl Tso {
    pub fn new(cluster_id: u64, pd_client: &PdClient) -> Result<Tso> {
        let (result_sender_tx, result_sender_rx) = mpsc::channel(MAX_PENDING_COUNT);

        // Start a background thread to handle TSO requests and responses
        let worker = TsoWorker::new(cluster_id, pd_client, result_sender_rx)?;
        thread::spawn(move || block_on(worker.run()));

        Ok(Tso { result_sender_tx })
    }

    pub async fn get_timestamp(mut self) -> Result<Timestamp> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.result_sender_tx
            .send(result_sender)
            .await
            .map_err(|_| Error::internal_error("Result sender channel is closed"))?;
        Ok(result_receiver.await?)
    }
}

struct TsoWorker {
    cluster_id: u64,
    result_sender_rx: mpsc::Receiver<oneshot::Sender<Timestamp>>,
    rpc_sender: Compat01As03Sink<ClientDuplexSender<TsoRequest>, (TsoRequest, WriteFlags)>,
    rpc_receiver: Compat01As03<ClientDuplexReceiver<TsoResponse>>,
}

impl TsoWorker {
    fn new(
        cluster_id: u64,
        pd_client: &PdClient,
        result_sender_rx: mpsc::Receiver<oneshot::Sender<Timestamp>>,
    ) -> Result<Self> {
        // FIXME: use tso_opt
        let (rpc_sender, rpc_receiver) = pd_client.tso()?;
        Ok(TsoWorker {
            cluster_id,
            result_sender_rx,
            rpc_sender: rpc_sender.sink_compat(),
            rpc_receiver: rpc_receiver.compat(),
        })
    }

    async fn run(mut self) {
        let ctx = Rc::new(RefCell::new(TsoContext {
            cluster_id: self.cluster_id,
            pending_queue: VecDeque::with_capacity(MAX_PENDING_COUNT),
            waker: None,
        }));

        let result_sender_rx = self.result_sender_rx;
        pin_mut!(result_sender_rx);
        let mut request_stream = TsoRequestStream {
            result_sender_rx,
            ctx: ctx.clone(),
        };

        let send_requests = self.rpc_sender.send_all(&mut request_stream);

        let mut rpc_receiver = self.rpc_receiver;
        let receive_and_handle_responses = async move {
            while let Some(Ok(resp)) = rpc_receiver.next().await {
                let mut ctx = ctx.borrow_mut();
                ctx.allocate_timestamps(&resp)?;
                if let Some(waker) = &ctx.waker {
                    waker.wake_by_ref();
                }
            }
            Err(Error::internal_error("TSO stream terminated"))
        };

        let _: (_, Result<()>) = join!(send_requests, receive_and_handle_responses);
    }
}

struct TsoRequestStream<'a> {
    result_sender_rx: Pin<&'a mut mpsc::Receiver<oneshot::Sender<Timestamp>>>,
    ctx: Rc<RefCell<TsoContext>>,
}

impl<'a> Stream for TsoRequestStream<'a> {
    type Item = (TsoRequest, WriteFlags);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let ctx = self.ctx.clone();
        let mut ctx = ctx.borrow_mut();

        // Set the waker to the context, then the stream can be waked up after the pending queue
        // is no longer full.
        if ctx.waker.is_none() {
            ctx.waker = Some(cx.waker().clone());
        }

        let mut count = 0;
        while ctx.pending_queue.len() < MAX_PENDING_COUNT {
            match self.result_sender_rx.as_mut().poll_next(cx) {
                Poll::Ready(Some(sender)) => {
                    ctx.pending_queue.push_back(sender);
                    count += 1;
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => break,
            }
        }

        if count > 0 {
            let req = TsoRequest {
                header: Some(RequestHeader {
                    cluster_id: ctx.cluster_id,
                }),
                count,
            };
            let write_flags = WriteFlags::default().buffer_hint(false);
            Poll::Ready(Some((req, write_flags)))
        } else {
            Poll::Pending
        }
    }
}

struct TsoContext {
    cluster_id: u64,
    pending_queue: VecDeque<oneshot::Sender<Timestamp>>,
    waker: Option<Waker>,
}

impl TsoContext {
    fn allocate_timestamps(&mut self, resp: &TsoResponse) -> Result<()> {
        // PD returns the timestamp with the biggest logical value. We can send back timestamps
        // whose logical value is from `logical - count + 1` to `logical` using the senders
        // in `pending`.
        let tail_ts = resp
            .timestamp
            .as_ref()
            .ok_or_else(|| Error::internal_error("No timestamp in TsoResponse"))?;
        let mut offset = resp.count as i64;
        while offset > 0 {
            offset -= 1;
            if let Some(sender) = self.pending_queue.pop_front() {
                let ts = Timestamp {
                    physical: tail_ts.physical,
                    logical: tail_ts.logical - offset,
                };

                // It doesn't matter if the receiver of the channel is dropped.
                let _ = sender.send(ts);
            } else {
                Err(Error::internal_error(
                    "PD gives more timestamps than expected",
                ))?
            }
        }
        Ok(())
    }
}
