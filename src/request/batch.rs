use crate::{Error, Result};
use futures::{
    channel::{mpsc, oneshot},
    executor::block_on,
    join, pin_mut,
    prelude::*,
    task::{AtomicWaker, Context, Poll},
};
use grpcio::WriteFlags;
use log::debug;
use std::{cell::RefCell, collections::VecDeque, pin::Pin, rc::Rc, thread};
use tikv_client_common::internal_err;
use tikv_client_proto::pdpb::*;

pub struct BatchCommandRequestEntry {
    cmd: BatchCommandsRequest,
    tx: oneshot::Sender<BatchCommandsResponse>,
    // TODO
    id: u64,
}

impl BatchCommandRequestEntry {
    pub fn new(cmd: BatchCommandsRequest, tx: oneshot::Sender<BatchCommandsResponse>) -> Self {
        Self { cmd, tx, id: 0 }
    }
}

/// BatchWorker provides request in batch and return the result in batch.
pub struct BatchWorker {
    request_tx: mpsc::Sender<BatchCommandRequestEntry>,
}

impl BatchWorker {
    pub fn new(
        kv_client: &KvClient,
        max_batch_size: usize,
        max_inflight_requests: usize,
        max_deplay_duration: std::time::Duration,
        options: CallOption,
    ) -> Result<BatchWorker> {
        let (request_tx, request_rx) = mpsc::channel(max_batch_size);

        // Create rpc sender and receiver
        let (rpc_sender, rpc_receiver) = kv_client.batch_commands_opt(options)?;

        // Start a background thread to handle batch requests and responses
        thread::spawn(move || {
            block_on(run_batch_worker(
                rpc_sender.sink_err_into(),
                rpc_receiver.err_into(),
                request_rx,
                max_batch_size,
                max_inflight_requests,
            ))
        });

        Ok(BatchWorker { request_tx })
    }

    pub async fn dispatch(
        mut self,
        request: BatchCommandsRequest,
    ) -> Result<BatchCommandsResponse> {
        let (tx, rx) = oneshot::channel();
        // Generate BatchCommandRequestEntry
        let entry = BatchCommandRequestEntry::new(request, tx);
        // Send request entry to the background thread to handle the request, response will be
        // received in rx channel.
        self.request_tx
            .send(entry)
            .await
            .map_err(|_| Error::Internal("Failed to send request to batch worker".to_owned()))?;
        Ok(rx.await?)
    }
}

async fn run_batch_worker(
    mut tx: impl Sink<(BatchCommandsRequest, WriteFlags), Error = Error> + Unpin,
    mut rx: impl Stream<Item = Result<BatchCommandsResponse>> + Unpin,
    request_rx: mpsc::Receiver<BatchCommandsRequestEntry>,
    max_batch_size: usize,
    max_inflight_requests: usize,
    max_deplay_duration: std::time::Duration,
) {
    // Inflight requests which are waiting for the response from rpc server
    let mut inflight_requests =
        Rc::new(RefCell::new(VecDeque::with_capacity(max_inflight_requests)));

    let waker = Rc::new(AtomicWaker::new());

    // TODO
    pin_mut!(request_rx);

    let mut request_stream = BatchCommandsRequestStream {
        request_rx,
        inflight_requests: inflight_requests.clone(),
        self_waker: waker.clone(),
        max_batch_size,
        max_inflight_requests,
        max_deplay_duration,
    }
    .map(Ok);

    let send_requests = tx.send_all(&mut request_stream);

    let recv_handle_response = async move {
        while let Some(Ok(resp)) = rx.next().await {
            let mut inflight_requests = inflight_requests.borrow_mut();

            if inflight_requests.len == max_inflight_requests {
                waker.wake();
            }

            // TODO more handle to response

            let request_id = resp.get_header().get_request_id();
            let request = inflight_requests.pop_front().unwrap();
            debug!(
                "Received response for request_id {}: {:?}",
                request_id, resp
            );
            request.tx.send(resp).unwrap();
        }
    };

    let (tx_res, rx_res) = join!(send_requests, recv_handle_response);
    debug!("Batch sender finished: {:?}", tx_res);
    debug!("Batch receiver finished: {:?}", rx_res);
}

struct BatchCommandsRequestStream {
    request_rx: mpsc::Receiver<BatchCommandsRequestEntry>,
    inflight_requests: Rc<RefCell<VecDeque<oneshot::Sender<BatchCommandsResponse>>>>,
    self_waker: Rc<AtomicWaker>,
    max_batch_size: usize,
    max_inflight_requests: usize,
    max_deplay_duration: std::time::Duration,
}

impl Stream for BatchCommandsRequestStream {
    type Item = Result<(BatchCommandsRequest, WriteFlags)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut inflight_requests = self.inflight_requests.clone().borrow_mut();
        if inflight_requests.len() == 0 {
            self.self_waker.register(cx.waker());
        }

        // Collect user requests
        let requests = vec![];
        let latency_timer = Instant::now();
        while requests.len() < self.max_batch_size
            && inflight_requests.len() < self.max_inflight_requests
        {
            // We can not wait longger than max_deplay_duration
            if latency_timer.elapsed() > self.max_deplay_duration {
                break;
            }

            match self.request_rx.poll_next(cx) {
                Poll::Ready(Some(Ok(entry))) => {
                    inflight_requests.push_back(entry.tx);
                    requests.push(entry.cmd);
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    break;
                }
            }
        }

        if !requests.is_empty() {
            let write_flags = WriteFlags::default().buffer_hint(false);
            Poll::Ready(Some((requests, write_flags)))
        } else {
            self.self_waker.register(cx.waker());
            Poll::Pending
        }
    }
}
