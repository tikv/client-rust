// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{batch::BatchWorker, request::Request, Result, SecurityManager};
use async_trait::async_trait;
use derive_new::new;
use grpcio::{CallOption, Environment};
use serde_derive::{Deserialize, Serialize};
use std::{any::Any, sync::Arc, time::Duration};
use tikv_client_proto::tikvpb::TikvClient;

const DEFAULT_REQUEST_TIMEOUT: u64 = 2000;
const DEFAULT_GRPC_KEEPALIVE_TIME: u64 = 10000;
const DEFAULT_GRPC_KEEPALIVE_TIMEOUT: u64 = 3000;
const DEFAULT_GRPC_COMPLETION_QUEUE_SIZE: usize = 1;
const DEFAULT_MAX_BATCH_WAIT_TIME: u64 = 10;
const DEFAULT_MAX_BATCH_SIZE: usize = 100;
const DEFAULT_OVERLOAD_THRESHOLD: usize = 200;
/// A trait for connecting to TiKV stores.
pub trait KvConnect: Sized + Send + Sync + 'static {
    type KvClient: KvClient + Clone + Send + Sync + 'static;

    fn connect(&self, address: &str, kv_config: KvClientConfig) -> Result<Self::KvClient>;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct KvClientConfig {
    pub request_timeout: u64,
    pub completion_queue_size: usize,
    pub grpc_keepalive_time: u64,
    pub grpc_keepalive_timeout: u64,
    pub allow_batch: bool,
    pub overload_threshold: usize,
    pub max_batch_wait_time: u64,
    pub max_batch_size: usize,
}

impl Default for KvClientConfig {
    fn default() -> Self {
        Self {
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            completion_queue_size: DEFAULT_GRPC_COMPLETION_QUEUE_SIZE,
            grpc_keepalive_time: DEFAULT_GRPC_KEEPALIVE_TIME,
            grpc_keepalive_timeout: DEFAULT_GRPC_KEEPALIVE_TIMEOUT,
            allow_batch: false,
            overload_threshold: DEFAULT_OVERLOAD_THRESHOLD,
            max_batch_wait_time: DEFAULT_MAX_BATCH_WAIT_TIME,
            max_batch_size: DEFAULT_MAX_BATCH_SIZE,
        }
    }
}

#[derive(new, Clone)]
pub struct TikvConnect {
    env: Arc<Environment>,
    security_mgr: Arc<SecurityManager>,
    timeout: Duration,
}

impl KvConnect for TikvConnect {
    type KvClient = KvRpcClient;

    fn connect(&self, address: &str, kv_config: KvClientConfig) -> Result<KvRpcClient> {
        self.security_mgr
            .connect(
                self.env.clone(),
                address,
                kv_config.grpc_keepalive_time,
                kv_config.grpc_keepalive_timeout,
                TikvClient::new,
            )
            .map(|c| {
                // Create batch worker if needed
                let c = Arc::new(c);
                let batch_worker = if kv_config.allow_batch {
                    Some(
                        BatchWorker::new(
                            c.clone(),
                            kv_config.max_batch_size,
                            kv_config.max_batch_size,
                            kv_config.max_batch_wait_time,
                            CallOption::default(),
                        )
                        .unwrap(),
                    )
                } else {
                    None
                };
                KvRpcClient::new(c.clone(), self.timeout, batch_worker)
            })
    }
}

#[async_trait]
pub trait KvClient {
    async fn dispatch(&self, req: Box<dyn Request>) -> Result<Box<dyn Any + Send>>;
}

/// This client handles requests for a single TiKV node. It converts the data
/// types and abstractions of the client program into the grpc data types.
#[derive(new, Clone)]
pub struct KvRpcClient {
    rpc_client: Arc<TikvClient>,
    timeout: Duration,
    batch_worker: Option<BatchWorker>,
}

#[async_trait]
impl KvClient for KvRpcClient {
    async fn dispatch(&self, request: Box<dyn Request>) -> Result<Box<dyn Any + Send>> {
        if let Some(batch_worker) = self.batch_worker.clone() {
            batch_worker.dispatch(request).await
        } else {
            // Batch no needed if not batch enabled
            request
                .dispatch(
                    &self.rpc_client,
                    CallOption::default().timeout(self.timeout),
                )
                .await
        }
    }
}
