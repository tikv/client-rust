// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{
    pd::{PdClient, PdRpcClient, RetryClient},
    request::DispatchHook,
    Config, Error, Key, Result, Timestamp,
};

use futures::future::{ready, BoxFuture, FutureExt};

use kvproto::tikvpb::TikvClient;
use std::{future::Future, sync::Arc};
use tikv_client_store::{HasError, KvClient, KvConnect};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MockKvClient {
    pub addr: String,
}

pub struct MockKvConnect;

impl KvClient for MockKvClient {
    fn dispatch<Resp, RpcFuture>(
        &self,
        _request_name: &'static str,
        _fut: grpcio::Result<RpcFuture>,
    ) -> BoxFuture<'static, Result<Resp>>
    where
        RpcFuture: Future<Output = std::result::Result<Resp, ::grpcio::Error>>,
        Resp: HasError + Sized + Clone + Send + 'static,
        RpcFuture: Send + 'static,
    {
        unimplemented!()
    }

    fn get_rpc_client(&self) -> Arc<TikvClient> {
        unimplemented!()
    }
}

impl KvConnect for MockKvConnect {
    type KvClient = MockKvClient;

    fn connect(&self, address: &str) -> Result<Self::KvClient> {
        Ok(MockKvClient {
            addr: address.to_owned(),
        })
    }
}
