use crate::{Error, Request, Result};
use core::any::Any;
use futures::{
    channel::{mpsc, oneshot},
    executor::block_on,
    join, pin_mut,
    prelude::*,
    task::{AtomicWaker, Context, Poll},
};
use grpcio::{CallOption, WriteFlags};
use log::debug;
use std::{
    cell::RefCell,
    collections::HashMap,
    pin::Pin,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tikv_client_common::internal_err;
use tikv_client_proto::tikvpb::{BatchCommandsRequest, BatchCommandsResponse, TikvClient};

static ID_ALLOC: AtomicU64 = AtomicU64::new(0);

type Response = oneshot::Sender<Result<Box<dyn Any + Send>>>;
pub struct RequestEntry {
    cmd: Box<dyn Request>,
    tx: Response,
    id: u64,
}

impl RequestEntry {
    pub fn new(cmd: Box<dyn Request>, tx: Response) -> Self {
        Self {
            cmd,
            tx,
            id: ID_ALLOC.fetch_add(1, Ordering::Relaxed),
        }
    }
}

/// BatchWorker provides request in batch and return the result in batch.
#[derive(Clone)]
pub struct BatchWorker {
    request_tx: mpsc::Sender<RequestEntry>,
}

impl BatchWorker {
    pub fn new(
        kv_client: Arc<TikvClient>,
        max_batch_size: usize,
        max_inflight_requests: usize,
        max_delay_duration: u64,
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
                max_delay_duration,
            ))
        });

        Ok(BatchWorker { request_tx })
    }

    pub async fn dispatch(mut self, request: Box<dyn Request>) -> Result<Box<dyn Any>> {
        let (tx, rx) = oneshot::channel();
        // Generate BatchCommandRequestEntry
        let entry = RequestEntry::new(request, tx);
        // Send request entry to the background thread to handle the request, response will be
        // received in rx channel.
        self.request_tx
            .send(entry)
            .await
            .map_err(|_| internal_err!("Failed to send request to batch worker".to_owned()))?;
        Ok(Box::new(rx.await?))
    }
}

async fn run_batch_worker(
    mut tx: impl Sink<(BatchCommandsRequest, WriteFlags), Error = Error> + Unpin,
    mut rx: impl Stream<Item = Result<BatchCommandsResponse>> + Unpin,
    request_rx: mpsc::Receiver<RequestEntry>,
    max_batch_size: usize,
    max_inflight_requests: usize,
    max_delay_duration: u64,
) {
    // Inflight requests which are waiting for the response from rpc server
    let inflight_requests = Rc::new(RefCell::new(HashMap::new()));

    let waker = Rc::new(AtomicWaker::new());

    pin_mut!(request_rx);
    let mut request_stream = BatchCommandsRequestStream {
        request_rx,
        inflight_requests: inflight_requests.clone(),
        self_waker: waker.clone(),
        max_batch_size,
        max_inflight_requests,
        max_delay_duration,
    };

    let send_requests = tx.send_all(&mut request_stream);

    let recv_handle_response = async move {
        while let Some(Ok(mut batch_resp)) = rx.next().await {
            // TODO resp is BatchCommandsResponse, split it into responses
            let mut inflight_requests = inflight_requests.borrow_mut();

            if inflight_requests.len() == max_inflight_requests {
                waker.wake();
            }

            // TODO more handle to response
            for (id, resp) in batch_resp
                .take_request_ids()
                .into_iter()
                .zip(batch_resp.take_responses())
            {
                if let Some(tx) = inflight_requests.remove(&id) {
                    debug!("Received response for request_id {}: {:?}", id, resp.cmd);
                    tx.send(Ok(Box::new(resp.cmd))).unwrap();
                }
            }
        }
    };

    let (tx_res, rx_res) = join!(send_requests, recv_handle_response);
    debug!("Batch sender finished: {:?}", tx_res);
    debug!("Batch receiver finished: {:?}", rx_res);
}

struct BatchCommandsRequestStream<'a> {
    request_rx: Pin<&'a mut mpsc::Receiver<RequestEntry>>,
    inflight_requests: Rc<RefCell<HashMap<u64, Response>>>,
    self_waker: Rc<AtomicWaker>,
    max_batch_size: usize,
    max_inflight_requests: usize,
    max_delay_duration: u64,
}

impl Stream for BatchCommandsRequestStream<'_> {
    type Item = Result<(BatchCommandsRequest, WriteFlags)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inflight_requests = self.inflight_requests.clone();
        let mut inflight_requests = inflight_requests.borrow_mut();
        if inflight_requests.len() == 0 {
            self.self_waker.register(cx.waker());
        }

        // Collect user requests
        let mut requests = vec![];
        let mut request_ids = vec![];
        let latency_timer = Instant::now();
        while requests.len() < self.max_batch_size
            && inflight_requests.len() < self.max_inflight_requests
        {
            // We can not wait longger than max_deplay_duration
            if latency_timer.elapsed() > Duration::from_millis(self.max_delay_duration) {
                break;
            }

            match self.request_rx.as_mut().poll_next(cx) {
                Poll::Ready(Some(entry)) => {
                    inflight_requests.insert(entry.id, entry.tx);
                    requests.push(entry.cmd.to_batch_request());
                    request_ids.push(entry.id);
                }
                Poll::Ready(None) => {
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    break;
                }
            }
        }

        // The requests is the commands will be convert to a batch request
        if !requests.is_empty() {
            // TODO generate BatchCommandsRequest from requests
            let mut batch_request = BatchCommandsRequest::new_();
            batch_request.set_requests(requests);
            // TODO request id
            batch_request.set_request_ids(request_ids);
            let write_flags = WriteFlags::default().buffer_hint(false);
            Poll::Ready(Some(Ok((batch_request, write_flags))))
        } else {
            self.self_waker.register(cx.waker());
            Poll::Pending
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
