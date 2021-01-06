// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! A utility module for managing and retrying PD requests.

use crate::{stats::pd_stats, Error, Region, RegionId, Result, SecurityManager, StoreId};
use async_trait::async_trait;
use futures_timer::Delay;
use grpcio::Environment;
use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};
use tikv_client_pd::{Cluster, Connection};
use tikv_client_proto::{
    metapb,
    pdpb::{self, Timestamp},
};
use tokio::sync::RwLock;

// FIXME: these numbers and how they are used are all just cargo-culted in, there
// may be more optimal values.
const RECONNECT_INTERVAL_SEC: u64 = 1;
const MAX_REQUEST_COUNT: usize = 3;
const LEADER_CHANGE_RETRY: usize = 10;

/// Client for communication with a PD cluster. Has the facility to reconnect to the cluster.
pub struct RetryClient<Cl = Cluster> {
    // Tuple is the cluster and the time of the cluster's last reconnect.
    cluster: RwLock<(Cl, Instant)>,
    connection: Connection,
    timeout: Duration,
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
            cluster: RwLock::new((cluster, Instant::now())),
            connection,
            timeout,
        }
    }
}

macro_rules! retry {
    ($self: ident, $tag: literal, |$cluster: ident| $call: expr) => {{
        let stats = pd_stats($tag);
        let mut last_err = Ok(());
        for _ in 0..LEADER_CHANGE_RETRY {
            let res = {
                let $cluster = &$self.cluster.read().await.0;
                $call.await
            };

            match stats.done(res) {
                Ok(r) => return Ok(r),
                Err(e) => last_err = Err(e),
            }

            let mut reconnect_count = MAX_REQUEST_COUNT;
            while let Err(e) = $self.reconnect(RECONNECT_INTERVAL_SEC).await {
                reconnect_count -= 1;
                if reconnect_count == 0 {
                    return Err(e);
                }
                Delay::new(Duration::from_secs(RECONNECT_INTERVAL_SEC)).await;
            }
        }

        last_err?;
        unreachable!();
    }};
}

impl RetryClient<Cluster> {
    pub async fn connect(
        env: Arc<Environment>,
        endpoints: &[String],
        security_mgr: Arc<SecurityManager>,
        timeout: Duration,
    ) -> Result<RetryClient> {
        let connection = Connection::new(env, security_mgr);
        let cluster = RwLock::new((
            connection.connect_cluster(endpoints, timeout).await?,
            Instant::now(),
        ));
        Ok(RetryClient {
            cluster,
            connection,
            timeout,
        })
    }

    // These get_* functions will try multiple times to make a request, reconnecting as necessary.
    // It does not know about encoding. Caller should take care of it.
    pub async fn get_region(self: Arc<Self>, key: Vec<u8>) -> Result<Region> {
        retry!(self, "get_region", |cluster| {
            let key = key.clone();
            async {
                cluster
                    .get_region(key.clone(), self.timeout)
                    .await
                    .and_then(|resp| {
                        region_from_response(resp, || Error::RegionForKeyNotFound { key })
                    })
            }
        })
    }

    pub async fn get_region_by_id(self: Arc<Self>, region_id: RegionId) -> Result<Region> {
        retry!(self, "get_region_by_id", |cluster| async {
            cluster
                .get_region_by_id(region_id, self.timeout)
                .await
                .and_then(|resp| region_from_response(resp, || Error::RegionNotFound { region_id }))
        })
    }

    pub async fn get_store(self: Arc<Self>, id: StoreId) -> Result<metapb::Store> {
        retry!(self, "get_store", |cluster| async {
            cluster
                .get_store(id, self.timeout)
                .await
                .map(|mut resp| resp.take_store())
        })
    }

    #[allow(dead_code)]
    pub async fn get_all_stores(self: Arc<Self>) -> Result<Vec<metapb::Store>> {
        retry!(self, "get_all_stores", |cluster| async {
            cluster
                .get_all_stores(self.timeout)
                .await
                .map(|mut resp| resp.take_stores().into_iter().map(Into::into).collect())
        })
    }

    pub async fn get_timestamp(self: Arc<Self>) -> Result<Timestamp> {
        retry!(self, "get_timestamp", |cluster| cluster.get_timestamp())
    }

    pub async fn update_safepoint(self: Arc<Self>, safepoint: u64) -> Result<bool> {
        retry!(self, "update_gc_safepoint", |cluster| async {
            cluster
                .update_safepoint(safepoint, self.timeout)
                .await
                .map(|resp| resp.get_new_safe_point() == safepoint)
        })
    }
}

impl fmt::Debug for RetryClient {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("pd::RetryClient")
            .field("timeout", &self.timeout)
            .finish()
    }
}

fn region_from_response(
    resp: pdpb::GetRegionResponse,
    err: impl FnOnce() -> Error,
) -> Result<Region> {
    let region = resp.region.ok_or_else(err)?;
    Ok(Region::new(region, resp.leader))
}

// A node-like thing that can be connected to.
#[async_trait]
trait Reconnect {
    type Cl;
    async fn reconnect(&self, interval_sec: u64) -> Result<()>;
}

#[async_trait]
impl Reconnect for RetryClient<Cluster> {
    type Cl = Cluster;

    async fn reconnect(&self, interval_sec: u64) -> Result<()> {
        let reconnect_begin = Instant::now();
        let mut lock = self.cluster.write().await;
        let (cluster, last_connected) = &mut *lock;
        // If `last_connected + interval_sec` is larger or equal than reconnect_begin,
        // a concurrent reconnect is just succeed when this thread trying to get write lock
        let should_connect = reconnect_begin > *last_connected + Duration::from_secs(interval_sec);
        if should_connect {
            self.connection.reconnect(cluster, self.timeout).await?;
            *last_connected = Instant::now();
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::{executor, future::ready};
    use std::sync::Mutex;
    use tikv_client_common::internal_err;

    #[test]
    fn test_reconnect() {
        struct MockClient {
            reconnect_count: Mutex<usize>,
            cluster: RwLock<((), Instant)>,
        }

        #[async_trait]
        impl Reconnect for MockClient {
            type Cl = ();

            async fn reconnect(&self, _: u64) -> Result<()> {
                *self.reconnect_count.lock().unwrap() += 1;
                // Not actually unimplemented, we just don't care about the error.
                Err(Error::Unimplemented)
            }
        }

        async fn retry_err(client: Arc<MockClient>) -> Result<()> {
            retry!(client, "test", |_c| ready(Err(internal_err!("whoops"))))
        }

        async fn retry_ok(client: Arc<MockClient>) -> Result<()> {
            retry!(client, "test", |_c| ready(Ok::<_, Error>(())))
        }

        executor::block_on(async {
            let client = Arc::new(MockClient {
                reconnect_count: Mutex::new(0),
                cluster: RwLock::new(((), Instant::now())),
            });

            assert!(retry_err(client.clone()).await.is_err());
            assert_eq!(*client.reconnect_count.lock().unwrap(), MAX_REQUEST_COUNT);

            *client.reconnect_count.lock().unwrap() = 0;
            assert!(retry_ok(client.clone()).await.is_ok());
            assert_eq!(*client.reconnect_count.lock().unwrap(), 0);
        })
    }

    #[test]
    fn test_retry() {
        struct MockClient {
            cluster: RwLock<(Mutex<usize>, Instant)>,
        }

        #[async_trait]
        impl Reconnect for MockClient {
            type Cl = Mutex<usize>;

            async fn reconnect(&self, _: u64) -> Result<()> {
                Ok(())
            }
        }

        async fn retry_max_err(
            client: Arc<MockClient>,
            max_retries: Arc<Mutex<usize>>,
        ) -> Result<()> {
            retry!(client, "test", |c| {
                let mut c = c.lock().unwrap();
                *c += 1;

                let mut max_retries = max_retries.lock().unwrap();
                *max_retries -= 1;
                if *max_retries == 0 {
                    ready(Ok(()))
                } else {
                    ready(Err(internal_err!("whoops")))
                }
            })
        }

        async fn retry_max_ok(
            client: Arc<MockClient>,
            max_retries: Arc<Mutex<usize>>,
        ) -> Result<()> {
            retry!(client, "test", |c| {
                let mut c = c.lock().unwrap();
                *c += 1;

                let mut max_retries = max_retries.lock().unwrap();
                *max_retries -= 1;
                if *max_retries == 0 {
                    ready(Ok(()))
                } else {
                    ready(Err(internal_err!("whoops")))
                }
            })
        }

        executor::block_on(async {
            let client = Arc::new(MockClient {
                cluster: RwLock::new((Mutex::new(0), Instant::now())),
            });
            let max_retries = Arc::new(Mutex::new(1000));

            assert!(retry_max_err(client.clone(), max_retries).await.is_err());
            assert_eq!(
                *client.cluster.read().await.0.lock().unwrap(),
                LEADER_CHANGE_RETRY
            );

            let client = Arc::new(MockClient {
                cluster: RwLock::new((Mutex::new(0), Instant::now())),
            });
            let max_retries = Arc::new(Mutex::new(2));

            assert!(retry_max_ok(client.clone(), max_retries).await.is_ok());
            assert_eq!(*client.cluster.read().await.0.lock().unwrap(), 2);
        })
    }
}
