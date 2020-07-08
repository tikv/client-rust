// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

mod errors;

pub use self::errors::{HasError, HasRegionError};
pub use kvproto::tikvpb::TikvClient;

use crate::{
    pd::Region,
    request::{KvRequest, KvRpcRequest},
    security::SecurityManager,
    stats::tikv_stats,
    ErrorKind, Result,
};

use crate::pd::StoreBuilder;
use derive_new::new;
use futures::compat::Compat01As03;
use futures::future::BoxFuture;
use futures::prelude::*;
use grpcio::CallOption;
use grpcio::Environment;
use std::sync::Arc;
use std::time::Duration;

/// A trait for connecting to TiKV stores.
pub trait KvConnect: Sized + Send + Sync + 'static {
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
    fn dispatch<T: KvRequest>(
        &self,
        request: &T,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>>;
}

/// This client handles requests for a single TiKV node. It converts the data
/// types and abstractions of the client program into the grpc data types.
#[derive(new, Clone)]
pub struct KvRpcClient {
    rpc_client: Arc<TikvClient>,
}

impl KvClient for KvRpcClient {
    fn dispatch<T: KvRequest>(
        &self,
        request: &T,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>> {
        map_errors_and_trace(T::REQUEST_NAME, T::RPC_FN(&self.rpc_client, request, opt)).boxed()
    }
}

async fn map_errors_and_trace<Resp, RpcFuture>(
    request_name: &'static str,
    fut: ::grpcio::Result<RpcFuture>,
) -> Result<Resp>
where
    Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
    Resp: HasError + Sized + Clone + Send + 'static,
{
    let res = match fut {
        Ok(f) => Compat01As03::new(f).await,
        Err(e) => Err(e),
    };

    let context = tikv_stats(request_name);
    context.done(res.map_err(|e| ErrorKind::Grpc(e).into()))
}

#[derive(new)]
pub struct Store<Client: KvClient> {
    pub region: Region,
    client: Client,
    timeout: Duration,
}

impl<Client: KvClient> Store<Client> {
    pub fn from_builder<T>(builder: StoreBuilder, connect: Arc<T>) -> Result<Store<Client>>
    where
        Client: KvClient + Clone + Send + Sync + 'static,
        T: KvConnect<KvClient = Client>,
    {
        info!("connect to tikv endpoint: {:?}", &builder.address);
        let client = connect.connect(builder.address.as_str())?;
        Ok(Store::new(builder.region, client, builder.timeout))
    }

    pub fn call_options(&self) -> CallOption {
        CallOption::default().timeout(self.timeout)
    }

    pub fn request<T: KvRpcRequest>(&self) -> T {
        let mut request = T::default();
        // FIXME propagate the error instead of using `expect`
        request.set_context(
            self.region
                .context()
                .expect("Cannot create context from region"),
        );
        request
    }

    pub fn dispatch<T: KvRequest>(
        &self,
        request: &T,
        opt: CallOption,
    ) -> BoxFuture<'static, Result<T::RpcResponse>> {
        self.client.dispatch::<T>(request, opt)
    }
}
