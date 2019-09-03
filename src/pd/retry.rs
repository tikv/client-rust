// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! A utility module for managing and retrying PD requests.

use std::{
    fmt,
    pin::Pin,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::future::BoxFuture;
use futures::prelude::*;
use futures::ready;
use futures::task::{Context, Poll};
use futures_timer::Delay;
use grpcio::Environment;
use kvproto::metapb;

use crate::{
    pd::{
        cluster::{Cluster, Connection},
        Region, RegionId, StoreId,
    },
    security::SecurityManager,
    transaction::Timestamp,
    Error, Result,
};

const RECONNECT_INTERVAL_SEC: u64 = 1;
const MAX_REQUEST_COUNT: usize = 3;
const LEADER_CHANGE_RETRY: usize = 10;

/// Client for communication with a PD cluster. Has the facility to reconnect to the cluster.
pub struct RetryClient<Cl = Cluster> {
    cluster: RwLock<Arc<Cl>>,
    connection: Connection,
    timeout: Duration,
}

impl<Cl> RetryClient<Cl> {
    #[cfg(test)]
    pub fn new_with_cluster(
        env: Arc<Environment>,
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
        cluster: Cl,
    ) -> RetryClient<Cl> {
        let connection = Connection::new(env, security_mgr);
        RetryClient {
            cluster: RwLock::new(Arc::new(cluster)),
            connection,
            timeout,
        }
    }
}

impl RetryClient<Cluster> {
    pub fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<RetryClient> {
        let connection = Connection::new(env, security_mgr);
        let cluster = RwLock::new(Arc::new(connection.connect_cluster(endpoints, timeout)?));
        Ok(RetryClient {
            cluster,
            connection,
            timeout,
        })
    }

    // These get_* functions will try multiple times to make a request, reconnecting as necessary.
    pub fn get_region(self: Arc<Self>, key: &[u8]) -> BoxFuture<'static, Result<Region>> {
        let key = key.to_owned();
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_region(key.clone(), timeout)
        }))
    }

    pub fn get_region_by_id(self: Arc<Self>, id: RegionId) -> BoxFuture<'static, Result<Region>> {
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_region_by_id(id, timeout)
        }))
    }

    pub fn get_store(self: Arc<Self>, id: StoreId) -> BoxFuture<'static, Result<metapb::Store>> {
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_store(id, timeout)
        }))
    }

    pub fn reconnect(&self, interval: u64) -> Result<()> {
        let new_cluster =
            self.connection
                .reconnect(&self.cluster.read().unwrap(), interval, self.timeout)?;
        if let Some(cluster) = new_cluster {
            *self.cluster.write().unwrap() = Arc::new(cluster);
        }
        Ok(())
    }

    pub fn with_cluster<T, F: Fn(&Cluster) -> T>(&self, f: F) -> T {
        f(&self.cluster.read().unwrap())
    }

    #[allow(dead_code)]
    pub fn get_all_stores(self: Arc<Self>) -> BoxFuture<'static, Result<Vec<metapb::Store>>> {
        let timeout = self.timeout;
        Box::pin(retry_request(self, move |cluster| {
            cluster.get_all_stores(timeout)
        }))
    }

    pub async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
        for _ in 0..LEADER_CHANGE_RETRY {
            let tso = self.cluster.read().unwrap().clone();
            match tso.get_timestamp().await {
                Ok(ts) => return Ok(ts),
                Err(e) => {
                    debug!("reconnect because of {}!", e);
                    // ATTENTION: this will block the event loop
                    if let Err(e) = self.reconnect(RECONNECT_INTERVAL_SEC) {
                        debug!("reconnect failed: {}", e);
                        Delay::new(Duration::from_secs(RECONNECT_INTERVAL_SEC)).await?;
                    }
                }
            }
        }
        Err(Error::internal_error("get_timestamp exceeds retry limit"))
    }
}

impl fmt::Debug for RetryClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("pd::RetryClient")
            .field("cluster_id", &self.cluster.read().unwrap().id)
            .field("timeout", &self.timeout)
            .finish()
    }
}

fn retry_request<Resp, Func, RespFuture>(
    client: Arc<RetryClient>,
    func: Func,
) -> RetryRequest<impl Future<Output = Result<Resp>>>
where
    Resp: Send + 'static,
    Func: Fn(&Cluster) -> RespFuture + Send + 'static,
    RespFuture: Future<Output = Result<Resp>> + Send + 'static,
{
    let mut req = Request::new(func, client);
    RetryRequest {
        reconnect_count: LEADER_CHANGE_RETRY,
        future: req
            .reconnect_if_needed()
            .map_err(|_| internal_err!("failed to reconnect"))
            .and_then(move |_| req.send_and_receive()),
    }
}

/// A future which will retry a request up to `reconnect_count` times or until it
/// succeeds.
struct RetryRequest<Fut> {
    reconnect_count: usize,
    future: Fut,
}

struct Request<Func> {
    // We keep track of requests sent and after `MAX_REQUEST_COUNT` we reconnect.
    request_sent: usize,

    client: Arc<RetryClient>,

    // A function which makes an async request.
    func: Func,
}

impl<Resp, Fut> Future for RetryRequest<Fut>
where
    Resp: Send + 'static,
    Fut: Future<Output = Result<Resp>> + Send + 'static,
{
    type Output = Result<Resp>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<Resp>> {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            if this.reconnect_count == 0 {
                return Poll::Ready(Err(internal_err!("failed to send request")));
            }

            debug!("reconnect remains: {}", this.reconnect_count);
            this.reconnect_count -= 1;
            let resp = ready!(Pin::new_unchecked(&mut this.future).poll(cx))?;
            Poll::Ready(Ok(resp))
        }
    }
}

impl<Resp, Func, RespFuture> Request<Func>
where
    Resp: Send + 'static,
    Func: Fn(&Cluster) -> RespFuture + Send + 'static,
    RespFuture: Future<Output = Result<Resp>> + Send + 'static,
{
    fn new(func: Func, client: Arc<RetryClient>) -> Self {
        Request {
            request_sent: 0,
            client,
            func,
        }
    }

    fn reconnect_if_needed(&mut self) -> impl Future<Output = std::result::Result<(), ()>> + Send {
        if self.request_sent < MAX_REQUEST_COUNT {
            return future::Either::Left(future::ok(()));
        }

        // FIXME: should not block the core.
        match self.client.reconnect(RECONNECT_INTERVAL_SEC) {
            Ok(_) => {
                self.request_sent = 0;
                future::Either::Left(future::ok(()))
            }
            Err(_) => future::Either::Right(
                Delay::new(Duration::from_secs(RECONNECT_INTERVAL_SEC)).map(|_| Err(())),
            ),
        }
    }

    fn send_and_receive(&mut self) -> impl Future<Output = Result<Resp>> + Send {
        self.request_sent += 1;
        debug!("request sent: {}", self.request_sent);

        self.client.with_cluster(&self.func)
    }
}
