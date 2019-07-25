// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

mod client;
mod errors;

pub use self::client::KvRpcClient;
pub use self::errors::HasError;
pub use kvproto::tikvpb::TikvClient;

use crate::pd::Region;
use crate::raw::{ColumnFamily, RawRequest};
use crate::security::SecurityManager;
use crate::Result;
use derive_new::new;
use futures::future::BoxFuture;
use grpcio::CallOption;
use grpcio::Environment;
use kvproto::kvrpcpb;
use std::sync::Arc;
use std::time::Duration;

/// A trait for connecting to TiKV stores.
pub trait KvConnect: Sized {
    type KvClient: KvClient + Clone + Send + Sync + 'static;

    fn connect(&self, address: &str) -> Result<Self::KvClient>;
}

pub type RpcFnType<Req, Resp> =
    for<'a, 'b> fn(
        &'a TikvClient,
        &'b Req,
        CallOption,
    )
        -> std::result::Result<::grpcio::ClientUnaryReceiver<Resp>, ::grpcio::Error>;

#[derive(new, Clone)]
pub struct TikvConnect {
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
}

impl KvConnect for TikvConnect {
    type KvClient = KvRpcClient;

    fn connect(&self, address: &str) -> Result<KvRpcClient> {
        self.security_mgr
            .connect(self.env.clone(), address, TikvClient::new)
            .map(|c| KvRpcClient::new(Arc::new(c)))
    }
}

pub trait KvClient {
    fn dispatch<T: RawRequest>(
        &self,
        request: &T::RpcRequest,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>>;
}

#[derive(new)]
pub struct Store<Client: KvClient> {
    pub region: Region,
    client: Client,
    timeout: Duration,
}

impl<Client: KvClient> Store<Client> {
    pub fn call_options(&self) -> CallOption {
        CallOption::default().timeout(self.timeout)
    }

    pub fn request<T: KvRawRequest>(&self) -> T {
        let mut request = T::default();
        // FIXME propagate the error instead of using `expect`
        request.set_context(
            self.region
                .context()
                .expect("Cannot create context from region"),
        );
        request
    }

    pub fn dispatch<T: RawRequest>(
        &self,
        request: &T::RpcRequest,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>> {
        self.client.dispatch::<T>(request, opt)
    }
}

pub trait KvRawRequest: Default {
    fn set_cf(&mut self, cf: String);
    fn set_context(&mut self, context: kvrpcpb::Context);

    fn maybe_set_cf(&mut self, cf: Option<ColumnFamily>) {
        if let Some(cf) = cf {
            self.set_cf(cf.to_string());
        }
    }
}

macro_rules! impl_raw_request {
    ($name: ident) => {
        impl KvRawRequest for kvrpcpb::$name {
            fn set_cf(&mut self, cf: String) {
                self.set_cf(cf);
            }
            fn set_context(&mut self, context: kvrpcpb::Context) {
                self.set_context(context);
            }
        }
    };
}

impl_raw_request!(RawGetRequest);
impl_raw_request!(RawBatchGetRequest);
impl_raw_request!(RawPutRequest);
impl_raw_request!(RawBatchPutRequest);
impl_raw_request!(RawDeleteRequest);
impl_raw_request!(RawBatchDeleteRequest);
impl_raw_request!(RawScanRequest);
impl_raw_request!(RawBatchScanRequest);
impl_raw_request!(RawDeleteRangeRequest);
