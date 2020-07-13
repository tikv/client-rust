// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#[macro_use]
extern crate log;

mod errors;

pub use self::errors::{HasError, HasRegionError};
use derive_new::new;
use futures::{compat::Compat01As03, future::BoxFuture, prelude::*};
use grpcio::{CallOption, Environment};
pub use kvproto::tikvpb::TikvClient;
use std::{sync::Arc, time::Duration};
use tikv_client_common::{security::SecurityManager, stats::tikv_stats, ErrorKind, Result};
use tikv_client_pd::{Region, StoreBuilder};

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
    fn dispatch<Resp, RpcFuture>(
        &self,
        request_name: &'static str,
        fut: ::grpcio::Result<RpcFuture>,
    ) -> BoxFuture<'static, Result<Resp>>
    where
        Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
        Resp: HasError + Sized + Clone + Send + 'static,
        RpcFuture: Send + 'static;

    fn get_rpc_client(&self) -> Arc<TikvClient>;
}

/// This client handles requests for a single TiKV node. It converts the data
/// types and abstractions of the client program into the grpc data types.
#[derive(new, Clone)]
pub struct KvRpcClient {
    pub rpc_client: Arc<TikvClient>,
}

impl KvClient for KvRpcClient {
    fn dispatch<Resp, RpcFuture>(
        &self,
        request_name: &'static str,
        fut: ::grpcio::Result<RpcFuture>,
    ) -> BoxFuture<'static, Result<Resp>>
    where
        Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
        Resp: HasError + Sized + Clone + Send + 'static,
        RpcFuture: Send + 'static,
    {
        map_errors_and_trace(request_name, fut).boxed()
    }

    fn get_rpc_client(&self) -> Arc<TikvClient> {
        self.rpc_client.clone()
    }
}

#[derive(new)]
pub struct Store<Client: KvClient> {
    pub region: Region,
    pub client: Client,
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

    pub fn dispatch<Resp, RpcFuture>(
        &self,
        request_name: &'static str,
        fut: ::grpcio::Result<RpcFuture>,
    ) -> BoxFuture<'static, Result<Resp>>
    where
        Compat01As03<RpcFuture>: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
        Resp: HasError + Sized + Clone + Send + 'static,
        RpcFuture: Send + 'static,
    {
        self.client.dispatch(request_name, fut)
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
