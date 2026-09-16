// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Low-level mechanisms for obtaining timestamps from PD or the TSO
//! microservice.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
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
use crate::proto::apipb;
use crate::proto::pdpb;
use crate::proto::pdpb::pd_client::PdClient;
use crate::proto::tsopb;
use crate::Result;
use crate::SecurityManager;
use crate::Timestamp;

/// It is an empirical value.
const MAX_BATCH_SIZE: usize = 64;

/// TODO: This value should be adjustable.
const MAX_PENDING_COUNT: usize = 1 << 16;

struct TimestampRequest {
    sender: oneshot::Sender<Timestamp>,
}

#[derive(Clone)]
struct ApiV3TimestampOracle {
    request_tx: mpsc::Sender<TimestampRequest>,
}

struct ApiV3TimestampOracles {
    cluster_id: u64,
    pd_client: PdClient<Channel>,
    security_mgr: Arc<SecurityManager>,
    oracles: Mutex<HashMap<(u32, u32), ApiV3TimestampOracle>>,
}

/// The timestamp oracle (TSO) which provides monotonically increasing timestamps.
#[derive(Clone)]
pub(crate) struct TimestampOracle {
    /// Legacy TSO requests continue to use the PD service. API V3 requests are
    /// routed to the TSO microservice because current kvproto intentionally no
    /// longer carries a keyspace identity in `pdpb.TsoRequest`.
    legacy_request_tx: mpsc::Sender<TimestampRequest>,
    api_v3_oracles: Arc<ApiV3TimestampOracles>,
}

impl TimestampOracle {
    pub(crate) fn new(
        cluster_id: u64,
        pd_client: &PdClient<Channel>,
        security_mgr: Arc<SecurityManager>,
    ) -> Result<TimestampOracle> {
        let (legacy_request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);

        // Start a background thread for legacy PD TSO requests.
        tokio::spawn(run_legacy_tso(cluster_id, pd_client.clone(), request_rx));

        Ok(TimestampOracle {
            legacy_request_tx,
            api_v3_oracles: Arc::new(ApiV3TimestampOracles {
                cluster_id,
                pd_client: pd_client.clone(),
                security_mgr,
                oracles: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub(crate) async fn get_timestamp(self) -> Result<Timestamp> {
        self.get_timestamp_with_identity(None).await
    }

    pub(crate) async fn get_timestamp_with_identity(
        self,
        identity: Option<apipb::KeyspaceIdentity>,
    ) -> Result<Timestamp> {
        debug!("getting current timestamp");
        let request_tx = match identity {
            Some(identity) => {
                self.api_v3_oracles
                    .get_or_create(identity)
                    .await?
                    .request_tx
            }
            None => self.legacy_request_tx,
        };
        let (sender, response) = oneshot::channel();
        request_tx
            .send(TimestampRequest { sender })
            .await
            .map_err(|_| internal_err!("TimestampRequest channel is closed"))?;
        Ok(response.await?)
    }
}

impl ApiV3TimestampOracles {
    async fn get_or_create(
        &self,
        identity: apipb::KeyspaceIdentity,
    ) -> Result<ApiV3TimestampOracle> {
        let key = (identity.namespace_id, identity.keyspace_id);
        if let Some(oracle) = self.oracles.lock().await.get(&key).cloned() {
            return Ok(oracle);
        }

        let oracle = self.connect(identity).await?;
        let mut oracles = self.oracles.lock().await;
        Ok(match oracles.entry(key) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => entry.insert(oracle).clone(),
        })
    }

    async fn connect(&self, identity: apipb::KeyspaceIdentity) -> Result<ApiV3TimestampOracle> {
        let mut pd_client = self.pd_client.clone();
        let cluster_info = pd_client
            .get_cluster_info(pdpb::GetClusterInfoRequest {
                header: Some(pdpb::ResponseHeader {
                    cluster_id: self.cluster_id,
                    ..Default::default()
                }),
            })
            .await?
            .into_inner();
        if let Some(error) = cluster_info
            .header
            .as_ref()
            .and_then(|header| header.error.as_ref())
        {
            return Err(internal_err!(
                "failed to discover TSO services: {}",
                error.message
            ));
        }
        if cluster_info.tso_urls.is_empty() {
            return Err(internal_err!(
                "PD did not return any TSO service addresses for API V3"
            ));
        }

        let mut last_error = None;
        for tso_url in cluster_info.tso_urls {
            match self.find_primary(&tso_url, &identity).await {
                Ok((primary_url, keyspace_group_id)) => {
                    let tso_client = self
                        .security_mgr
                        .connect(&primary_url, tsopb::tso_client::TsoClient::<Channel>::new)
                        .await?;
                    let callee_id = callee_id(&primary_url);
                    let (request_tx, request_rx) = mpsc::channel(MAX_BATCH_SIZE);
                    tokio::spawn(run_api_v3_tso(
                        self.cluster_id,
                        keyspace_group_id,
                        callee_id,
                        identity,
                        tso_client,
                        request_rx,
                    ));
                    return Ok(ApiV3TimestampOracle { request_tx });
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.unwrap_or_else(|| internal_err!("failed to discover TSO primary")))
    }

    async fn find_primary(
        &self,
        tso_url: &str,
        identity: &apipb::KeyspaceIdentity,
    ) -> Result<(String, u32)> {
        let mut client = self
            .security_mgr
            .connect(tso_url, tsopb::tso_client::TsoClient::<Channel>::new)
            .await?;
        let header = tsopb::RequestHeader {
            cluster_id: self.cluster_id,
            keyspace: Some(tsopb::request_header::Keyspace::KeyspaceIdentity(
                identity.clone(),
            )),
            ..Default::default()
        };
        let response = client
            .find_group_by_keyspace_id(tsopb::FindGroupByKeyspaceIdRequest {
                header: Some(header),
                keyspace: Some(
                    tsopb::find_group_by_keyspace_id_request::Keyspace::KeyspaceIdentity(
                        identity.clone(),
                    ),
                ),
                mod_revision: 0,
            })
            .await?
            .into_inner();
        if let Some(error) = response
            .header
            .as_ref()
            .and_then(|header| header.error.as_ref())
        {
            if error.r#type != tsopb::ErrorType::Ok as i32 {
                return Err(internal_err!(
                    "failed to find API V3 TSO keyspace group: {}",
                    error.message
                ));
            }
        }
        let group = response
            .keyspace_group
            .ok_or_else(|| internal_err!("TSO service returned no keyspace group"))?;
        let primary = group
            .members
            .iter()
            .find(|member| member.is_primary)
            .ok_or_else(|| internal_err!("TSO keyspace group {} has no primary", group.id))?;
        Ok((primary.address.clone(), group.id))
    }
}

fn callee_id(address: &str) -> String {
    address
        .strip_prefix("http://")
        .or_else(|| address.strip_prefix("https://"))
        .unwrap_or(address)
        .trim_end_matches('/')
        .to_owned()
}

async fn run_legacy_tso(
    cluster_id: u64,
    mut pd_client: PdClient<Channel>,
    request_rx: mpsc::Receiver<TimestampRequest>,
) -> Result<()> {
    let pending_requests = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));
    let sending_future_waker = Arc::new(AtomicWaker::new());
    let request_stream = LegacyTsoRequestStream {
        cluster_id,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
    };
    let mut responses = pd_client.tso(request_stream).await?.into_inner();

    while let Some(Ok(response)) = responses.next().await {
        let mut pending_requests = pending_requests.lock().await;
        allocate_timestamps(
            response.timestamp.as_ref(),
            response.count,
            &mut pending_requests,
        )?;
        sending_future_waker.wake();
    }
    info!("legacy PD TSO stream terminated");
    Ok(())
}

async fn run_api_v3_tso(
    cluster_id: u64,
    keyspace_group_id: u32,
    callee_id: String,
    identity: apipb::KeyspaceIdentity,
    mut tso_client: tsopb::tso_client::TsoClient<Channel>,
    request_rx: mpsc::Receiver<TimestampRequest>,
) -> Result<()> {
    let pending_requests = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PENDING_COUNT)));
    let sending_future_waker = Arc::new(AtomicWaker::new());
    let request_stream = ApiV3TsoRequestStream {
        cluster_id,
        keyspace_group_id,
        callee_id,
        identity,
        request_rx,
        pending_requests: pending_requests.clone(),
        self_waker: sending_future_waker.clone(),
    };
    let mut responses = tso_client.tso(request_stream).await?.into_inner();

    while let Some(Ok(response)) = responses.next().await {
        if let Some(error) = response
            .header
            .as_ref()
            .and_then(|header| header.error.as_ref())
        {
            if error.r#type != tsopb::ErrorType::Ok as i32 {
                return Err(internal_err!(
                    "API V3 TSO request failed: {}",
                    error.message
                ));
            }
        }
        let mut pending_requests = pending_requests.lock().await;
        allocate_timestamps(
            response.timestamp.as_ref(),
            response.count,
            &mut pending_requests,
        )?;
        sending_future_waker.wake();
    }
    info!("API V3 TSO stream terminated");
    Ok(())
}

struct RequestGroup {
    count: u32,
    requests: Vec<oneshot::Sender<Timestamp>>,
}

#[pin_project]
struct LegacyTsoRequestStream {
    cluster_id: u64,
    #[pin]
    request_rx: mpsc::Receiver<TimestampRequest>,
    pending_requests: Arc<Mutex<VecDeque<RequestGroup>>>,
    self_waker: Arc<AtomicWaker>,
}

impl Stream for LegacyTsoRequestStream {
    type Item = pdpb::TsoRequest;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.project();
        poll_request_batch(
            this.request_rx,
            this.pending_requests,
            this.self_waker,
            |count| pdpb::TsoRequest {
                header: Some(pdpb::RequestHeader {
                    cluster_id: *this.cluster_id,
                    sender_id: 0,
                    ..Default::default()
                }),
                count,
                dc_location: String::new(),
            },
            cx,
        )
    }
}

#[pin_project]
struct ApiV3TsoRequestStream {
    cluster_id: u64,
    keyspace_group_id: u32,
    callee_id: String,
    identity: apipb::KeyspaceIdentity,
    #[pin]
    request_rx: mpsc::Receiver<TimestampRequest>,
    pending_requests: Arc<Mutex<VecDeque<RequestGroup>>>,
    self_waker: Arc<AtomicWaker>,
}

impl Stream for ApiV3TsoRequestStream {
    type Item = tsopb::TsoRequest;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.project();
        poll_request_batch(
            this.request_rx,
            this.pending_requests,
            this.self_waker,
            |count| tsopb::TsoRequest {
                header: Some(tsopb::RequestHeader {
                    cluster_id: *this.cluster_id,
                    keyspace_group_id: *this.keyspace_group_id,
                    callee_id: this.callee_id.clone(),
                    keyspace: Some(tsopb::request_header::Keyspace::KeyspaceIdentity(
                        this.identity.clone(),
                    )),
                    ..Default::default()
                }),
                count,
                dc_location: String::new(),
            },
            cx,
        )
    }
}

fn poll_request_batch<T>(
    mut request_rx: Pin<&mut mpsc::Receiver<TimestampRequest>>,
    pending_requests: &Arc<Mutex<VecDeque<RequestGroup>>>,
    self_waker: &Arc<AtomicWaker>,
    make_request: impl FnOnce(u32) -> T,
    cx: &mut Context,
) -> Poll<Option<T>> {
    let pending = pending_requests.lock();
    pin_mut!(pending);
    let mut pending = if let Poll::Ready(pending) = pending.poll(cx) {
        pending
    } else {
        self_waker.register(cx.waker());
        return Poll::Pending;
    };
    if pending.len() >= MAX_PENDING_COUNT {
        self_waker.register(cx.waker());
        return Poll::Pending;
    }

    let first = match request_rx.poll_recv(cx) {
        Poll::Ready(Some(request)) => request,
        Poll::Ready(None) => return Poll::Ready(None),
        Poll::Pending => {
            self_waker.register(cx.waker());
            return Poll::Pending;
        }
    };
    let mut requests = vec![first.sender];
    while requests.len() < MAX_BATCH_SIZE {
        match request_rx.poll_recv(cx) {
            Poll::Ready(Some(request)) => requests.push(request.sender),
            Poll::Ready(None) | Poll::Pending => break,
        }
    }
    let count = requests.len() as u32;
    pending.push_back(RequestGroup { count, requests });
    Poll::Ready(Some(make_request(count)))
}

fn allocate_timestamps(
    tail_ts: Option<&pdpb::Timestamp>,
    count: u32,
    pending_requests: &mut VecDeque<RequestGroup>,
) -> Result<()> {
    let tail_ts = tail_ts.ok_or_else(|| internal_err!("No timestamp in TSO response"))?;
    let mut offset = count;
    let group = pending_requests
        .pop_front()
        .ok_or_else(|| internal_err!("TSO gives more responses than expected"))?;
    if group.count != offset {
        return Err(internal_err!(
            "TSO gives different number of timestamps than expected"
        ));
    }
    for request in group.requests {
        offset -= 1;
        let timestamp = Timestamp {
            physical: tail_ts.physical,
            logical: tail_ts.logical - offset as i64,
            suffix_bits: tail_ts.suffix_bits,
        };
        let _ = request.send(timestamp);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn api_v3_tso_request_stream_batches_and_includes_identity() {
        let (request_tx, request_rx) = mpsc::channel(2);
        let (sender1, _response1) = oneshot::channel();
        let (sender2, _response2) = oneshot::channel();
        request_tx
            .send(TimestampRequest { sender: sender1 })
            .await
            .unwrap();
        request_tx
            .send(TimestampRequest { sender: sender2 })
            .await
            .unwrap();
        let identity = apipb::KeyspaceIdentity {
            namespace_id: 3,
            keyspace_id: 7,
        };
        let mut stream = ApiV3TsoRequestStream {
            cluster_id: 42,
            keyspace_group_id: 9,
            callee_id: "tso-0:3379".to_owned(),
            identity: identity.clone(),
            request_rx,
            pending_requests: Arc::new(Mutex::new(VecDeque::new())),
            self_waker: Arc::new(AtomicWaker::new()),
        };

        let request = stream.next().await.unwrap();
        let header = request.header.unwrap();
        assert_eq!(request.count, 2);
        assert_eq!(header.cluster_id, 42);
        assert_eq!(header.keyspace_group_id, 9);
        assert_eq!(header.callee_id, "tso-0:3379");
        assert_eq!(
            header.keyspace,
            Some(tsopb::request_header::Keyspace::KeyspaceIdentity(identity))
        );
    }

    #[test]
    fn strips_tso_address_scheme_for_callee_id() {
        assert_eq!(callee_id("https://tso-0:3379/"), "tso-0:3379");
        assert_eq!(callee_id("tso-0:3379"), "tso-0:3379");
    }
}
