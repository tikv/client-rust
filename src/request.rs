// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::{Backoff, NoBackoff, NoJitterBackoff},
    pd::PdClient,
    transaction::{resolve_locks, HasLocks},
    BoundRange, Error, ErrorKind, Key, Result,
};
use futures::{future::BoxFuture, prelude::*, stream::BoxStream};
use std::{
    cmp::{max, min},
    sync::Arc,
};
use tikv_client_store::{HasError, HasRegionError, Request, Store};

const DEFAULT_REGION_BACKOFF: NoJitterBackoff = NoJitterBackoff::new(2, 500, 10);
pub const OPTIMISTIC_BACKOFF: NoJitterBackoff = NoJitterBackoff::new(2, 500, 10);
pub const PESSIMISTIC_BACKOFF: NoBackoff = NoBackoff;

pub trait KvRequest: Request + Clone + Sync + Send + 'static + Sized {
    type Result;
    type RpcResponse: HasError + HasLocks + Clone + Send + 'static;
    /// A single `KvRequest` can be divided into a number of RPC requests because the keys span
    /// several regions or a single RPC request is too large. Most of the fields in these requests
    /// share the same content while `KeyData`, which contains keys (and associated data if any),
    /// is the part which differs among the requests.
    type KeyData: Send;

    fn execute(
        self,
        pd_client: Arc<impl PdClient>,
        lock_backoff: impl Backoff,
    ) -> BoxFuture<'static, Result<Self::Result>> {
        Self::reduce(
            self.response_stream(pd_client, lock_backoff)
                .and_then(|mut response| match response.error() {
                    Some(e) => future::err(e),
                    None => future::ok(response),
                })
                .map_ok(Self::map_result)
                .boxed(),
        )
    }

    fn response_stream(
        self,
        pd_client: Arc<impl PdClient>,
        lock_backoff: impl Backoff,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        self.retry_response_stream(pd_client, DEFAULT_REGION_BACKOFF, lock_backoff)
    }

    fn retry_response_stream(
        mut self,
        pd_client: Arc<impl PdClient>,
        region_backoff: impl Backoff,
        lock_backoff: impl Backoff,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        let stores = self.store_stream(pd_client.clone());
        stores
            .and_then(move |(key_data, store)| {
                let request = self.make_rpc_request(key_data, &store);
                async move {
                    let request = request?;
                    let response = store.dispatch::<_, Self::RpcResponse>(&request).await?;
                    Ok((request, *response))
                }
            })
            .map_ok(move |(request, mut response)| {
                if let Some(region_error) = response.region_error() {
                    return request.on_region_error(
                        region_error,
                        pd_client.clone(),
                        region_backoff.clone(),
                        lock_backoff.clone(),
                    );
                }
                // Resolve locks
                let locks = response.take_locks();
                if !locks.is_empty() {
                    let pd_client = pd_client.clone();
                    let region_backoff = region_backoff.clone();
                    let lock_backoff = lock_backoff.clone();
                    return resolve_locks(locks, pd_client.clone())
                        .map_ok(move |resolved| {
                            if !resolved {
                                request.on_resolve_lock_failed(
                                    pd_client,
                                    region_backoff,
                                    lock_backoff,
                                )
                            } else {
                                request.response_stream(pd_client, OPTIMISTIC_BACKOFF)
                            }
                        })
                        .try_flatten_stream()
                        .boxed();
                }
                stream::once(future::ok(response)).boxed()
            })
            .try_flatten()
            .boxed()
    }

    fn on_region_error(
        self,
        region_error: Error,
        pd_client: Arc<impl PdClient>,
        mut region_backoff: impl Backoff,
        lock_backoff: impl Backoff,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        region_backoff.next_delay_duration().map_or(
            stream::once(future::err(region_error)).boxed(),
            move |delay_duration| {
                let fut = async move {
                    futures_timer::Delay::new(delay_duration).await;
                    Ok(())
                };

                fut.map_ok(move |_| {
                    self.retry_response_stream(pd_client, region_backoff, lock_backoff)
                })
                .try_flatten_stream()
                .boxed()
            },
        )
    }

    fn on_resolve_lock_failed(
        self,
        pd_client: Arc<impl PdClient>,
        region_backoff: impl Backoff,
        mut lock_backoff: impl Backoff,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        lock_backoff.next_delay_duration().map_or(
            stream::once(future::err(ErrorKind::ResolveLockError.into())).boxed(),
            move |delay_duration| {
                let fut = async move {
                    futures_timer::Delay::new(delay_duration).await;
                    Ok(())
                };
                fut.map_ok(move |_| {
                    self.retry_response_stream(pd_client, region_backoff, lock_backoff)
                })
                .try_flatten_stream()
                .boxed()
            },
        )
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store)>>;

    fn make_rpc_request(&self, key_data: Self::KeyData, store: &Store) -> Result<Self>;

    fn map_result(result: Self::RpcResponse) -> Self::Result;

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>>;

    fn request_from_store(&self, store: &Store) -> Result<Self>
    where
        Self: Default,
    {
        let mut request = Self::default();
        request.set_context(store.region.context()?);
        Ok(request)
    }
}

pub fn store_stream_for_key<KeyData, PdC>(
    key_data: KeyData,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(KeyData, Store)>>
where
    KeyData: AsRef<Key> + Send + 'static,
    PdC: PdClient,
{
    pd_client
        .store_for_key(key_data.as_ref().clone())
        .map_ok(move |store| (key_data, store))
        .into_stream()
        .boxed()
}

/// Maps keys to a stream of stores. `key_data` must be sorted in increasing order
pub fn store_stream_for_keys<KeyData, IntoKey, I, PdC>(
    key_data: I,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<KeyData>, Store)>>
where
    KeyData: AsRef<Key> + Send + Sync + 'static,
    IntoKey: Into<KeyData> + 'static,
    I: IntoIterator<Item = IntoKey>,
    I::IntoIter: Send + Sync + 'static,
    PdC: PdClient,
{
    pd_client
        .clone()
        .group_keys_by_region(key_data.into_iter().map(Into::into))
        .and_then(move |(region_id, key)| {
            pd_client
                .clone()
                .store_for_id(region_id)
                .map_ok(move |store| (key, store))
        })
        .boxed()
}

pub fn store_stream_for_range<PdC: PdClient>(
    range: BoundRange,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<((Key, Key), Store)>> {
    pd_client
        .stores_for_range(range.clone())
        .map_ok(move |store| {
            let region_range = store.region.range();
            (bound_range(region_range, range.clone()), store)
        })
        .into_stream()
        .boxed()
}

/// The range used for request should be the intersection of `region_range` and `range`.
fn bound_range(region_range: (Key, Key), range: BoundRange) -> (Key, Key) {
    let (lower, upper) = region_range;
    let (lower_bound, upper_bound) = range.into_keys();
    let up = match (upper.is_empty(), upper_bound) {
        (_, None) => upper,
        (true, Some(ub)) => ub,
        (_, Some(ub)) if ub.is_empty() => upper,
        (_, Some(ub)) => min(upper, ub),
    };
    (max(lower, lower_bound), up)
}

pub fn store_stream_for_ranges<PdC: PdClient>(
    ranges: Vec<BoundRange>,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<BoundRange>, Store)>> {
    pd_client
        .clone()
        .group_ranges_by_region(ranges)
        .and_then(move |(region_id, range)| {
            pd_client
                .clone()
                .store_for_id(region_id)
                .map_ok(move |store| (range, store))
        })
        .into_stream()
        .boxed()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::mock::{MockKvClient, MockPdClient};
    use async_trait::async_trait;
    use futures::executor;
    use grpcio::CallOption;
    use kvproto::{kvrpcpb, tikvpb::TikvClient};
    use std::{any::Any, sync::Mutex};
    use tikv_client_common::stats::{tikv_stats, RequestStats};

    #[test]
    fn test_region_retry() {
        #[derive(Clone)]
        struct MockRpcResponse;

        impl HasError for MockRpcResponse {
            fn error(&mut self) -> Option<Error> {
                unreachable!()
            }
        }

        impl HasRegionError for MockRpcResponse {
            fn region_error(&mut self) -> Option<Error> {
                Some(Error::region_not_found(1))
            }
        }

        impl HasLocks for MockRpcResponse {}

        #[derive(Clone)]
        struct MockKvRequest {
            test_invoking_count: Arc<Mutex<usize>>,
        }

        #[async_trait]
        impl Request for MockKvRequest {
            async fn dispatch(&self, _: &TikvClient, _: CallOption) -> Result<Box<dyn Any>> {
                Ok(Box::new(MockRpcResponse {}))
            }

            fn stats(&self) -> RequestStats {
                tikv_stats("mock")
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_context(&mut self, _: kvrpcpb::Context) {
                unreachable!();
            }
        }

        impl KvRequest for MockKvRequest {
            type Result = ();
            type RpcResponse = MockRpcResponse;
            type KeyData = Key;

            fn make_rpc_request(&self, _key_data: Self::KeyData, _store: &Store) -> Result<Self> {
                Ok(Self {
                    test_invoking_count: self.test_invoking_count.clone(),
                })
            }

            fn map_result(_: Self::RpcResponse) -> Self::Result {}

            fn reduce(
                _results: BoxStream<'static, Result<Self::Result>>,
            ) -> BoxFuture<'static, Result<Self::Result>> {
                unreachable!()
            }

            fn store_stream<PdC: PdClient>(
                &mut self,
                pd_client: Arc<PdC>,
            ) -> BoxStream<'static, Result<(Self::KeyData, Store)>> {
                // Increases by 1 for each call.
                let mut test_invoking_count = self.test_invoking_count.lock().unwrap();
                *test_invoking_count += 1;

                store_stream_for_key(Key::from("mock_key".to_owned()), pd_client)
            }
        }

        let invoking_count = Arc::new(Mutex::new(0));

        let request = MockKvRequest {
            test_invoking_count: invoking_count.clone(),
        };

        let pd_client = Arc::new(MockPdClient::new(MockKvClient::with_dispatch_hook(
            |_: &dyn Any| Ok(Box::new(MockRpcResponse) as Box<dyn Any>),
        )));
        let region_backoff = NoJitterBackoff::new(1, 1, 3);
        let lock_backoff = NoJitterBackoff::new(1, 1, 3);
        let stream = request.retry_response_stream(pd_client, region_backoff, lock_backoff);

        executor::block_on(async { stream.collect::<Vec<Result<MockRpcResponse>>>().await });

        // Original call plus the 3 retries
        assert_eq!(*invoking_count.lock().unwrap(), 4);
    }
}
