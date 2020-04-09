// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! A utility module for managing and retrying PD requests.

use std::{
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use futures::lock::{Mutex, MutexGuard, MutexLockFuture};
use futures::prelude::*;
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
    Result,
};
use failure::_core::pin::Pin;
use futures::task::{Context, Poll};

// FIXME: these numbers and how they are used are all just cargo-culted in, there
// may be more optimal values.
const RECONNECT_INTERVAL_SEC: u64 = 1;
const MAX_REQUEST_COUNT: usize = 3;
const LEADER_CHANGE_RETRY: usize = 10;

/// Client for communication with a PD cluster. Has the facility to reconnect to the cluster.
pub struct RetryClient<Cl = Cluster> {
    cluster: RwLock<Cl>,
    connection: Connection,
    timeout: Duration,
    connected: Arc<Mutex<bool>>,
}

#[cfg(test)]
impl<Cl> RetryClient<Cl> {
    pub fn new_with_cluster(
        env: Arc<Environment>,
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
        cluster: Cl,
    ) -> RetryClient<Cl> {
        let connection = Connection::new(env, security_mgr);
        RetryClient {
            cluster: RwLock::new(cluster),
            connection,
            timeout,
            connected: Arc::new(Mutex::new(true)),
        }
    }
}

impl RetryClient<Cluster> {
    pub async fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<RetryClient> {
        let connection = Connection::new(env, security_mgr);
        let cluster = RwLock::new(connection.connect_cluster(endpoints, timeout).await?);
        Ok(RetryClient {
            cluster,
            connection,
            timeout,
            connected: Arc::new(Mutex::new(true)),
        })
    }

    // These get_* functions will try multiple times to make a request, reconnecting as necessary.
    pub async fn get_region(self: Arc<Self>, key: Vec<u8>) -> Result<Region> {
        let timeout = self.timeout;
        retry_request(self, move |cluster| {
            cluster.get_region(key.clone(), timeout)
        })
        .await
    }

    pub async fn get_region_by_id(self: Arc<Self>, id: RegionId) -> Result<Region> {
        let timeout = self.timeout;
        retry_request(self, move |cluster| cluster.get_region_by_id(id, timeout)).await
    }

    pub async fn get_store(self: Arc<Self>, id: StoreId) -> Result<metapb::Store> {
        let timeout = self.timeout;
        retry_request(self, move |cluster| cluster.get_store(id, timeout)).await
    }

    #[allow(dead_code)]
    pub async fn get_all_stores(self: Arc<Self>) -> Result<Vec<metapb::Store>> {
        let timeout = self.timeout;
        retry_request(self, move |cluster| cluster.get_all_stores(timeout)).await
    }

    pub async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
        retry_request(self, move |cluster| cluster.get_timestamp()).await
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

struct ShouldReconnect<'a> {
    connected: MutexLockFuture<'a, bool>,
    first_ready: Option<bool>,
}

impl<'a> ShouldReconnect<'a> {
    fn new(connected: MutexLockFuture<'a, bool>) -> Self {
        ShouldReconnect {
            connected,
            first_ready: None,
        }
    }
}

impl<'a> Future for ShouldReconnect<'a> {
    type Output = (MutexGuard<'a, bool>, bool);
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.connected).poll(cx) {
            Poll::Ready(val) => match self.first_ready {
                None => {
                    self.first_ready = Some(true);
                    Poll::Ready((val, true))
                }
                Some(true) => panic!("should not happen"),
                Some(false) => {
                    let res = !*val;
                    Poll::Ready((val, res))
                }
            },
            Poll::Pending => {
                if self.first_ready.is_none() {
                    self.first_ready = Some(false)
                }
                Poll::Pending
            }
        }
    }
}

// A node-like thing that can be connected to.
#[async_trait]
trait Reconnect {
    type Cl;
    async fn reconnect(&self, interval: u64) -> Result<()>;
    fn with_cluster<T, F: Fn(&Self::Cl) -> T>(&self, f: F) -> T;
}

#[async_trait]
impl Reconnect for RetryClient<Cluster> {
    type Cl = Cluster;

    async fn reconnect(&self, interval: u64) -> Result<()> {
        let connected: MutexLockFuture<bool> = self.connected.lock();
        let (mut connected, should_connect) = ShouldReconnect::new(connected).await;
        //if try to reconnect while a previous reconnect is running,
        // the second reconnect will succeed in direct if the first one is succeed
        if should_connect {
            *connected = false;
            if let Some(cluster) = {
                let (id, members) = {
                    let read_guard = self.cluster.read().unwrap();
                    (read_guard.id, read_guard.members.clone())
                };

                self.connection
                    .reconnect(id, &members, interval, self.timeout)
                    .await?
            } {
                *self.cluster.write().unwrap() = cluster;
            }
            *connected = true;
            Ok(())
        } else {
            Ok(())
        }
    }

    fn with_cluster<T, F: Fn(&Cluster) -> T>(&self, f: F) -> T {
        f(&self.cluster.read().unwrap())
    }
}

async fn retry_request<Rc, Resp, Func, RespFuture>(client: Arc<Rc>, func: Func) -> Result<Resp>
where
    Rc: Reconnect,
    Resp: Send + 'static,
    Func: Fn(&Rc::Cl) -> RespFuture,
    RespFuture: Future<Output = Result<Resp>> + Send + 'static,
{
    let mut last_err = Ok(());
    for _ in 0..LEADER_CHANGE_RETRY {
        let fut = client.with_cluster(&func);
        match fut.await {
            Ok(r) => return Ok(r),
            Err(e) => last_err = Err(e),
        }

        // Reconnect.
        let mut reconnect_count = MAX_REQUEST_COUNT;
        while let Err(e) = client.reconnect(RECONNECT_INTERVAL_SEC).await {
            reconnect_count -= 1;
            if reconnect_count == 0 {
                return Err(e);
            }
            Delay::new(Duration::from_secs(RECONNECT_INTERVAL_SEC)).await;
        }
    }

    last_err?;
    unreachable!();
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Error;
    use futures::executor;
    use futures::future::ready;
    use std::sync::Mutex;

    #[test]
    fn test_reconnect() {
        struct MockClient {
            reconnect_count: Mutex<usize>,
        }

        #[async_trait]
        impl Reconnect for MockClient {
            type Cl = ();

            async fn reconnect(&self, _: u64) -> Result<()> {
                *self.reconnect_count.lock().unwrap() += 1;
                // Not actually unimplemented, we just don't care about the error.
                Err(Error::unimplemented())
            }

            fn with_cluster<T, F: Fn(&()) -> T>(&self, f: F) -> T {
                f(&())
            }
        }

        let client = Arc::new(MockClient {
            reconnect_count: Mutex::new(0),
        });

        fn ready_err(_: &()) -> impl Future<Output = Result<()>> + Send + 'static {
            ready(Err(internal_err!("whoops")))
        }

        let result = executor::block_on(retry_request(client.clone(), ready_err));
        assert!(result.is_err());
        assert_eq!(*client.reconnect_count.lock().unwrap(), MAX_REQUEST_COUNT);

        *client.reconnect_count.lock().unwrap() = 0;
        let result = executor::block_on(retry_request(client.clone(), |_| ready(Ok(()))));
        assert!(result.is_ok());
        assert_eq!(*client.reconnect_count.lock().unwrap(), 0);
    }

    #[test]
    fn test_retry() {
        struct MockClient {
            retry_count: Mutex<usize>,
        }

        #[async_trait]
        impl Reconnect for MockClient {
            type Cl = ();

            async fn reconnect(&self, _: u64) -> Result<()> {
                Ok(())
            }

            fn with_cluster<T, F: Fn(&()) -> T>(&self, f: F) -> T {
                *self.retry_count.lock().unwrap() += 1;
                f(&())
            }
        }

        let client = Arc::new(MockClient {
            retry_count: Mutex::new(0),
        });
        let max_retries = Arc::new(Mutex::new(1000));

        let result = executor::block_on(retry_request(client.clone(), |_| {
            let mut max_retries = max_retries.lock().unwrap();
            *max_retries -= 1;
            if *max_retries == 0 {
                ready(Ok(()))
            } else {
                ready(Err(internal_err!("whoops")))
            }
        }));
        assert!(result.is_err());
        assert_eq!(*client.retry_count.lock().unwrap(), LEADER_CHANGE_RETRY);

        let client = Arc::new(MockClient {
            retry_count: Mutex::new(0),
        });
        let max_retries = Arc::new(Mutex::new(2));

        let result = executor::block_on(retry_request(client.clone(), |_| {
            let mut max_retries = max_retries.lock().unwrap();
            *max_retries -= 1;
            if *max_retries == 0 {
                ready(Ok(()))
            } else {
                ready(Err(internal_err!("whoops")))
            }
        }));
        assert!(result.is_ok());
        assert_eq!(*client.retry_count.lock().unwrap(), 2);
    }
}
