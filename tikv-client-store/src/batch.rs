use crate::{request::from_batch_commands_resp, Error, Request, Result};
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
const BATCH_WORKER_NAME: &str = "batch-worker";

type Response = oneshot::Sender<Box<dyn Any + Send>>;
pub struct RequestEntry {
    cmd: Box<dyn Request>,
    tx: Response,
    transport_layer_load: u64,
    id: u64,
}

impl RequestEntry {
    pub fn new(cmd: Box<dyn Request>, tx: Response, transport_layer_load: u64) -> Self {
        Self {
            cmd,
            tx,
            transport_layer_load,
            id: ID_ALLOC.fetch_add(1, Ordering::Relaxed),
        }
    }
}

/// BatchWorker provides request in batch and return the result in batch.
#[derive(Clone)]
pub struct BatchWorker {
    request_tx: mpsc::Sender<RequestEntry>,
    last_transport_layer_load_report: Arc<AtomicU64>,
}

impl BatchWorker {
    pub fn new(
        kv_client: Arc<TikvClient>,
        max_batch_size: usize,
        max_inflight_requests: usize,
        max_delay_duration: u64,
        overload_threshold: u64,
        options: CallOption,
    ) -> Result<BatchWorker> {
        let (request_tx, request_rx) = mpsc::channel(max_inflight_requests);

        // Create rpc sender and receiver
        let (rpc_sender, rpc_receiver) = kv_client.batch_commands_opt(options)?;

        let last_transport_layer_load_report = Arc::new(AtomicU64::new(0));

        // Start a background thread to handle batch requests and responses
        let last_transport_layer_load_report_clone = last_transport_layer_load_report.clone();
        thread::Builder::new()
            .name(BATCH_WORKER_NAME.to_owned())
            .spawn(move || {
                block_on(run_batch_worker(
                    rpc_sender.sink_err_into(),
                    rpc_receiver.err_into(),
                    request_rx,
                    max_batch_size,
                    max_inflight_requests,
                    max_delay_duration,
                    overload_threshold,
                    last_transport_layer_load_report_clone,
                ))
            })
            .unwrap();

        Ok(BatchWorker {
            request_tx,
            last_transport_layer_load_report,
        })
    }

    pub async fn dispatch(mut self, request: Box<dyn Request>) -> Result<Box<dyn Any + Send>> {
        let (tx, rx) = oneshot::channel();
        // Generate BatchCommandRequestEntry
        let last_transport_layer_load = self
            .last_transport_layer_load_report
            .load(Ordering::Relaxed);

        // Save the load of transport layer in RequestEntry
        let entry = RequestEntry::new(request, tx, last_transport_layer_load);
        // Send request entry to the background thread to handle the request, response will be
        // received in rx channel.
        self.request_tx
            .send(entry)
            .await
            .map_err(|_| internal_err!("Failed to send request to batch worker".to_owned()))?;
        rx.await
            .map_err(|_| internal_err!("Failed to receive response from batch worker".to_owned()))
    }
}

async fn run_batch_worker(
    mut tx: impl Sink<(BatchCommandsRequest, WriteFlags), Error = Error> + Unpin,
    mut rx: impl Stream<Item = Result<BatchCommandsResponse>> + Unpin,
    request_rx: mpsc::Receiver<RequestEntry>,
    max_batch_size: usize,
    max_inflight_requests: usize,
    max_delay_duration: u64,
    overload_threshold: u64,
    last_transport_layer_load_report: Arc<AtomicU64>,
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
        overload_threshold,
    }
    .map(Ok);

    let send_requests = tx.send_all(&mut request_stream);

    let recv_handle_response = async move {
        while let Some(Ok(mut batch_resp)) = rx.next().await {
            let mut inflight_requests = inflight_requests.borrow_mut();

            if inflight_requests.len() == max_inflight_requests {
                waker.wake();
            }

            let trasport_layer_load = batch_resp.get_transport_layer_load();
            // Store the load of transport layer
            last_transport_layer_load_report.store(trasport_layer_load, Ordering::Relaxed);

            for (id, resp) in batch_resp
                .take_request_ids()
                .into_iter()
                .zip(batch_resp.take_responses())
            {
                if let Some(tx) = inflight_requests.remove(&id) {
                    let inner_resp = from_batch_commands_resp(resp);
                    debug!("Received response for request_id {}", id);
                    tx.send(inner_resp.unwrap()).unwrap();
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
    overload_threshold: u64,
}

impl Stream for BatchCommandsRequestStream<'_> {
    type Item = (BatchCommandsRequest, WriteFlags);

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

                    // Check the transport layer load received in RequestEntry
                    let load_reported = entry.transport_layer_load;
                    if load_reported > 0
                        && self.overload_threshold > 0
                        && load_reported > self.overload_threshold
                    {
                        break;
                    }
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
            let mut batch_request = BatchCommandsRequest::new_();
            batch_request.set_requests(requests);
            batch_request.set_request_ids(request_ids);
            let write_flags = WriteFlags::default().buffer_hint(false);
            Poll::Ready(Some((batch_request, write_flags)))
        } else {
            self.self_waker.register(cx.waker());
            Poll::Pending
        }
    }
}
