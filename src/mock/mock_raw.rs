// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::MockRpcPdClient;
use crate::{raw::requests, request::KvRequest, ColumnFamily};
use grpcio::Environment;
pub use mock_tikv::{start_server, MockTikv, PORT};
use std::{sync::Arc, u32};
use tikv_client_common::{
    security::SecurityManager, BoundRange, Config, Error, Key, KvPair, Result, Value,
};
use tikv_client_store::{KvConnect, TikvConnect};

const MAX_RAW_KV_SCAN_LIMIT: u32 = 10240;

// TODO: remove this struct later.
// should modify client so that it is compatible with and type of pd clients and use that for test.
#[derive(Clone)]
pub struct MockRawClient {
    rpc: Arc<MockRpcPdClient>,
    cf: Option<ColumnFamily>,
    key_only: bool,
}

impl MockRawClient {
    pub async fn new(_config: Config) -> Result<MockRawClient> {
        let rpc_client = TikvConnect::new(
            Arc::new(Environment::new(1)),
            Arc::new(SecurityManager::default()),
        )
        .connect(format!("localhost:{}", PORT).as_str())
        .unwrap();
        let rpc = Arc::new(MockRpcPdClient::new(rpc_client));
        Ok(MockRawClient {
            rpc,
            cf: None,
            key_only: false,
        })
    }

    pub fn with_cf(&self, cf: ColumnFamily) -> MockRawClient {
        MockRawClient {
            rpc: self.rpc.clone(),
            cf: Some(cf),
            key_only: self.key_only,
        }
    }

    pub fn with_key_only(&self, key_only: bool) -> MockRawClient {
        MockRawClient {
            rpc: self.rpc.clone(),
            cf: self.cf.clone(),
            key_only,
        }
    }

    pub async fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        requests::new_raw_get_request(key, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn batch_get(
        &self,
        keys: impl IntoIterator<Item = impl Into<Key>>,
    ) -> Result<Vec<KvPair>> {
        requests::new_raw_batch_get_request(keys, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        requests::new_raw_put_request(key, value, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn batch_put(
        &self,
        pairs: impl IntoIterator<Item = impl Into<KvPair>>,
    ) -> Result<()> {
        requests::new_raw_batch_put_request(pairs, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn delete(&self, key: impl Into<Key>) -> Result<()> {
        requests::new_raw_delete_request(key, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn batch_delete(&self, keys: impl IntoIterator<Item = impl Into<Key>>) -> Result<()> {
        requests::new_raw_batch_delete_request(keys, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn delete_range(&self, range: impl Into<BoundRange>) -> Result<()> {
        requests::new_raw_delete_range_request(range, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn scan(&self, range: impl Into<BoundRange>, limit: u32) -> Result<Vec<KvPair>> {
        if limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::max_scan_limit_exceeded(limit, MAX_RAW_KV_SCAN_LIMIT));
        }

        requests::new_raw_scan_request(range, limit, self.key_only, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }

    pub async fn batch_scan(
        &self,
        ranges: impl IntoIterator<Item = impl Into<BoundRange>>,
        each_limit: u32,
    ) -> Result<Vec<KvPair>> {
        if each_limit > MAX_RAW_KV_SCAN_LIMIT {
            return Err(Error::max_scan_limit_exceeded(
                each_limit,
                MAX_RAW_KV_SCAN_LIMIT,
            ));
        }

        requests::new_raw_batch_scan_request(ranges, each_limit, self.key_only, self.cf.clone())
            .execute(self.rpc.clone())
            .await
    }
}

#[cfg(test)]
mod test {

    use super::MockRawClient;
    use crate::{mock::MockRpcPdClient, pd::PdClient, request::KvRequest};
    use grpcio::redirect_log;

    use simple_logger::SimpleLogger;
        use mock_tikv::start_server;
use tikv_client_common::{Config, KvPair};

    #[tokio::test]
    async fn test_raw_put_get() {
        SimpleLogger::new().init().unwrap();
        redirect_log();

        let mut server = start_server();
        let mock_client = MockRawClient::new(Config::default()).await.unwrap();

        // empty; get non-existent key
        let res = mock_client.get("k1".to_owned()).await;
        assert_eq!(res.unwrap().unwrap(), vec![]);

        // empty; put then batch_get
        let _ = mock_client
            .put("k1".to_owned(), "v1".to_owned())
            .await
            .unwrap();
        let _ = mock_client
            .put("k2".to_owned(), "v2".to_owned())
            .await
            .unwrap();

        let res = mock_client
            .batch_get(vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()])
            .await
            .unwrap();
        assert_eq!(res[0].1, "v1".as_bytes());
        assert_eq!(res[1].1, "v2".as_bytes());
        assert_eq!(res[2].1, "".as_bytes());

        // k1,k2; batch_put then batch_get
        let _ = mock_client
            .batch_put(vec![
                KvPair::new("k3".to_owned(), "v3".to_owned()),
                KvPair::new("k4".to_owned(), "v4".to_owned()),
            ])
            .await
            .unwrap();

        let res = mock_client
            .batch_get(vec!["k4".to_owned(), "k3".to_owned()])
            .await
            .unwrap();
        assert_eq!(res[0].1, "v4".as_bytes());
        assert_eq!(res[1].1, "v3".as_bytes());

        // k1,k2,k3,k4; delete then get
        let res = mock_client.delete("k3".to_owned()).await;
        assert!(res.is_ok());

        let res = mock_client.delete("key-not-exist".to_owned()).await;
        assert!(res.is_err());

        let res = mock_client.get("k3".to_owned()).await;
        assert_eq!(res.unwrap().unwrap(), "".as_bytes());

        // k1,k2,k4; batch_delete then batch_get
        let res = mock_client
            .batch_delete(vec![
                "k1".to_owned(),
                "k2".to_owned(),
                "k3".to_owned(),
                "k4".to_owned(),
            ])
            .await;
        assert!(res.is_err());

        let res = mock_client
            .batch_delete(vec!["k1".to_owned(), "k2".to_owned(), "k4".to_owned()])
            .await;
        assert!(res.is_ok());

        let res = mock_client
            .batch_get(vec![
                "k1".to_owned(),
                "k2".to_owned(),
                "k3".to_owned(),
                "k4".to_owned(),
            ])
            .await
            .unwrap();
        for i in 0..3 {
            assert_eq!(res[i].1, "".as_bytes());
        }

        let _ = server.shutdown().await;
    }
}
