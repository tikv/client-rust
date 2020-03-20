// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    backoff::{Backoff, NoJitterBackoff},
    kv_client::{HasError, HasRegionError, KvClient, RpcFnType, Store},
    pd::PdClient,
    transaction::{resolve_locks, HasLocks},
    BoundRange, Error, Key, Result,
};

use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use grpcio::CallOption;
use kvproto::kvrpcpb;
use std::sync::Arc;
use std::time::Duration;

const LOCK_RETRY_DELAY_MS: u64 = 10;
const DEFAULT_REGION_BACKOFF: NoJitterBackoff = NoJitterBackoff::new(2, 500, 10);

pub trait KvRequest: Sync + Send + 'static + Sized {
    type Result;
    type RpcResponse: HasError + HasLocks + Clone + Send + 'static;
    /// A single `KvRequest` can be divided into a number of RPC requests because the keys span
    /// several regions or a single RPC request is too large. Most of the fields in these requests
    /// share the same content while `KeyData`, which contains keys (and associated data if any),
    /// is the part which differs among the requests.
    type KeyData;
    const REQUEST_NAME: &'static str;
    const RPC_FN: RpcFnType<Self, Self::RpcResponse>;

    fn execute(self, pd_client: Arc<impl PdClient>) -> BoxFuture<'static, Result<Self::Result>> {
        Self::reduce(
            self.response_stream(pd_client)
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
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        self.retry_response_stream(pd_client, DEFAULT_REGION_BACKOFF)
    }

    fn retry_response_stream(
        mut self,
        pd_client: Arc<impl PdClient>,
        backoff: impl Backoff,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        let stores = self.store_stream(pd_client.clone());
        stores
            .and_then(move |(key_data, store)| {
                let request = self.make_rpc_request(key_data, &store);
                self.dispatch_hook(store.call_options())
                    .unwrap_or_else(|| store.dispatch::<Self>(&request, store.call_options()))
                    .map_ok(move |response| (request, response))
            })
            .map_ok(move |(request, mut response)| {
                if let Some(region_error) = response.region_error() {
                    return request.on_region_error(
                        region_error,
                        pd_client.clone(),
                        backoff.clone(),
                    );
                }
                // Resolve locks
                let locks = response.take_locks();
                if !locks.is_empty() {
                    let pd_client = pd_client.clone();
                    return resolve_locks(locks, pd_client.clone())
                        .map_ok(|resolved| {
                            // TODO: backoff
                            let delay_ms = if resolved { 0 } else { LOCK_RETRY_DELAY_MS };
                            futures_timer::Delay::new(Duration::from_millis(delay_ms))
                        })
                        .map_ok(move |_| request.response_stream(pd_client))
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
        mut backoff: impl Backoff,
    ) -> BoxStream<'static, Result<Self::RpcResponse>> {
        backoff.next_delay_duration().map_or(
            stream::once(future::err(region_error)).boxed(),
            move |delay_duration| {
                let fut = async move {
                    futures_timer::Delay::new(delay_duration).await;
                    Ok(())
                };

                fut.map_ok(move |_| self.retry_response_stream(pd_client, backoff))
                    .try_flatten_stream()
                    .boxed()
            },
        )
    }

    fn store_stream<PdC: PdClient>(
        &mut self,
        pd_client: Arc<PdC>,
    ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>>;

    fn make_rpc_request<KvC: KvClient>(&self, key_data: Self::KeyData, store: &Store<KvC>) -> Self;

    fn map_result(result: Self::RpcResponse) -> Self::Result;

    fn reduce(
        results: BoxStream<'static, Result<Self::Result>>,
    ) -> BoxFuture<'static, Result<Self::Result>>;
}

pub fn store_stream_for_key<KeyData, PdC>(
    key_data: KeyData,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(KeyData, Store<PdC::KvClient>)>>
where
    KeyData: AsRef<Key> + Send + 'static,
    PdC: PdClient,
{
    pd_client
        .store_for_key(key_data.as_ref())
        .map_ok(move |store| (key_data, store))
        .into_stream()
        .boxed()
}

pub fn store_stream_for_keys<KeyData, IntoKey, I, PdC>(
    key_data: I,
    pd_client: Arc<PdC>,
) -> BoxStream<'static, Result<(Vec<KeyData>, Store<PdC::KvClient>)>>
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
) -> BoxStream<'static, Result<((Key, Key), Store<PdC::KvClient>)>> {
    pd_client
        .stores_for_range(range)
        .map_ok(move |store| {
            // FIXME should be bounded by self.range
            let range = store.region.range();
            (range, store)
        })
        .into_stream()
        .boxed()
}

/// Permits easy mocking of rpc calls.
pub trait DispatchHook: KvRequest {
    fn dispatch_hook(
        &self,
        _opt: CallOption,
    ) -> Option<BoxFuture<'static, Result<Self::RpcResponse>>> {
        None
    }
}

impl<T: KvRequest> DispatchHook for T {
    #[cfg(test)]
    default fn dispatch_hook(
        &self,
        _opt: CallOption,
    ) -> Option<BoxFuture<'static, Result<Self::RpcResponse>>> {
        None
    }
}

pub trait KvRpcRequest: Default {
    fn set_context(&mut self, context: kvrpcpb::Context);
}

macro_rules! impl_kv_rpc_request {
    ($name: ident) => {
        impl KvRpcRequest for kvrpcpb::$name {
            fn set_context(&mut self, context: kvrpcpb::Context) {
                self.set_context(context);
            }
        }
    };
}

impl_kv_rpc_request!(RawGetRequest);
impl_kv_rpc_request!(RawBatchGetRequest);
impl_kv_rpc_request!(RawPutRequest);
impl_kv_rpc_request!(RawBatchPutRequest);
impl_kv_rpc_request!(RawDeleteRequest);
impl_kv_rpc_request!(RawBatchDeleteRequest);
impl_kv_rpc_request!(RawScanRequest);
impl_kv_rpc_request!(RawBatchScanRequest);
impl_kv_rpc_request!(RawDeleteRangeRequest);
impl_kv_rpc_request!(GetRequest);
impl_kv_rpc_request!(ScanRequest);
impl_kv_rpc_request!(PrewriteRequest);
impl_kv_rpc_request!(CommitRequest);
impl_kv_rpc_request!(CleanupRequest);
impl_kv_rpc_request!(BatchGetRequest);
impl_kv_rpc_request!(BatchRollbackRequest);
impl_kv_rpc_request!(ResolveLockRequest);

#[cfg(test)]
mod test {
    use super::*;
    use crate::mock::MockPdClient;
    use futures::executor;
    use kvproto::tikvpb::TikvClient;
    use std::sync::Mutex;

    #[test]
    fn test_region_retry() {
        #[derive(Clone)]
        struct MockRpcResponse {}

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

        struct MockKvRequest {
            test_invoking_count: Arc<Mutex<usize>>,
        }

        fn mock_async_opt(
            _client: &TikvClient,
            _req: &MockKvRequest,
            _opt: CallOption,
        ) -> std::result::Result<::grpcio::ClientUnaryReceiver<MockRpcResponse>, ::grpcio::Error>
        {
            unreachable!()
        }

        impl DispatchHook for MockKvRequest {
            fn dispatch_hook(
                &self,
                _opt: CallOption,
            ) -> Option<BoxFuture<'static, Result<Self::RpcResponse>>> {
                Some(future::ok(MockRpcResponse {}).boxed())
            }
        }

        impl KvRequest for MockKvRequest {
            type Result = ();
            type RpcResponse = MockRpcResponse;
            type KeyData = Key;
            const REQUEST_NAME: &'static str = "mock";
            const RPC_FN: RpcFnType<Self, Self::RpcResponse> = mock_async_opt;

            fn make_rpc_request<KvC: KvClient>(
                &self,
                _key_data: Self::KeyData,
                _store: &Store<KvC>,
            ) -> Self {
                Self {
                    test_invoking_count: self.test_invoking_count.clone(),
                }
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
            ) -> BoxStream<'static, Result<(Self::KeyData, Store<PdC::KvClient>)>> {
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

        let pd_client = Arc::new(MockPdClient);
        let backoff = NoJitterBackoff::new(1, 1, 3);
        let stream = request.retry_response_stream(pd_client, backoff);

        executor::block_on(async { stream.collect::<Vec<Result<MockRpcResponse>>>().await });

        // Original call plus the 3 retries
        assert_eq!(*invoking_count.lock().unwrap(), 4);
    }
}
