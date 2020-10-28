// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{ErrorKind, HasError, Region, Result, SecurityManager};
use async_trait::async_trait;
use derive_new::new;
use grpcio::{CallOption, Environment};
use kvproto::tikvpb::TikvClient;
use std::{sync::Arc, time::Duration};
use tikv_client_common::stats::tikv_stats;

/// A trait for connecting to TiKV stores.
pub trait KvConnect: Sized + Send + Sync + 'static {
    type KvClient: KvClient + Clone + Send + Sync + 'static;

    fn connect(&self, address: &str) -> Result<Self::KvClient>;

    fn connect_to_store(&self, region: Region, address: String) -> Result<Store<Self::KvClient>> {
        info!("connect to tikv endpoint: {:?}", &address);
        let client = self.connect(address.as_str())?;
        Ok(Store::new(region, client))
    }
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
    timeout: Duration,
}

impl KvConnect for TikvConnect {
    type KvClient = impl KvClient + Clone + Send + Sync + 'static;

    fn connect(&self, address: &str) -> Result<Self::KvClient> {
        self.security_mgr
            .connect(self.env.clone(), address, TikvClient::new)
            .map(|c| KvRpcClient::new(Arc::new(c), self.timeout))
    }
}

#[async_trait]
pub trait KvClient {
    async fn dispatch<Req, Resp>(&self, fun: RpcFnType<Req, Resp>, request: Req) -> Result<Resp>
    where
        Req: Send + Sync + 'static,
        Resp: HasError + Sized + Clone + Send + 'static;
}

/// This client handles requests for a single TiKV node. It converts the data
/// types and abstractions of the client program into the grpc data types.
#[derive(new, Clone)]
struct KvRpcClient {
    rpc_client: Arc<TikvClient>,
    timeout: Duration,
}

#[async_trait]
impl KvClient for KvRpcClient {
    async fn dispatch<Req, Resp>(&self, fun: RpcFnType<Req, Resp>, request: Req) -> Result<Resp>
    where
        Req: Send + Sync + 'static,
        Resp: HasError + Sized + Clone + Send + 'static,
    {
        fun(
            &self.rpc_client,
            &request,
            CallOption::default().timeout(self.timeout),
        )?
        .await
        .map_err(|e| ErrorKind::Grpc(e).into())
    }
}

#[derive(new)]
pub struct Store<Client: KvClient> {
    pub region: Region,
    pub client: Client,
}

impl<Client: KvClient> Store<Client> {
    pub async fn dispatch<Req, Resp>(
        &self,
        request_name: &'static str,
        fun: RpcFnType<Req, Resp>,
        request: Req,
    ) -> Result<Resp>
    where
        Req: Send + Sync + 'static,
        Resp: HasError + Sized + Clone + Send + 'static,
    {
        let result = self.client.dispatch(fun, request).await;

        tikv_stats(request_name).done(result)
    }
}
