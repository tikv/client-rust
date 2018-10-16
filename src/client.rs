use raw::Raw;
use txn::{Oracle, Snapshot, Timestamp, Transaction, Txn};
use {Key, KeyRange, KvFuture, KvPair, Value};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub pd_endpoints: Vec<String>,
    pub ca_path: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl Config {
    pub fn new<E>(pd_endpoints: E) -> Self
    where
        E: IntoIterator<Item = String>,
    {
        Config {
            pd_endpoints: pd_endpoints.into_iter().collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
        }
    }

    pub fn security(mut self, ca_path: String, cert_path: String, key_path: String) -> Self {
        self.ca_path = Some(ca_path);
        self.cert_path = Some(cert_path);
        self.key_path = Some(key_path);
        self
    }
}

impl Into<Config> for String {
    fn into(self) -> Config {
        Config::new(Some(self))
    }
}

impl<'a> Into<Config> for Vec<String> {
    fn into(self) -> Config {
        Config::new(self)
    }
}

pub struct Client;

impl Client {
    pub fn new<C>(config: C) -> KvFuture<Self>
    where
        C: Into<Config>,
    {
        drop(config);
        unimplemented!()
    }
}

impl Raw for Client {
    fn get<K, C>(&self, key: K, cf: C) -> KvFuture<Value>
    where
        K: Into<Key>,
        C: Into<Option<String>>,
    {
        drop(key);
        drop(cf);
        unimplemented!()
    }
    fn batch_get<I, K, C>(&self, keys: I, cf: C) -> KvFuture<Vec<KvPair>>
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
        C: Into<Option<String>>,
    {
        drop(keys);
        drop(cf);
        unimplemented!()
    }
    fn put<P, C>(&self, pair: P, cf: C) -> KvFuture<()>
    where
        P: Into<KvPair>,
        C: Into<Option<String>>,
    {
        drop(pair);
        drop(cf);
        unimplemented!()
    }
    fn batch_put<I, P, C>(&self, pairs: I, cf: C) -> KvFuture<()>
    where
        I: IntoIterator<Item = P>,
        P: Into<KvPair>,
        C: Into<Option<String>>,
    {
        drop(pairs);
        drop(cf);
        unimplemented!()
    }
    fn delete<K, C>(&self, key: K, cf: C) -> KvFuture<()>
    where
        K: Into<Key>,
        C: Into<Option<String>>,
    {
        drop(key);
        drop(cf);
        unimplemented!()
    }
    fn batch_delete<I, K, C>(&self, keys: I, cf: C) -> KvFuture<()>
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
        C: Into<Option<String>>,
    {
        drop(keys);
        drop(cf);
        unimplemented!()
    }
    fn scan<R, C>(&self, range: R, _limit: u32, _key_only: bool, cf: C) -> KvFuture<Vec<KvPair>>
    where
        R: Into<KeyRange>,
        C: Into<Option<String>>,
    {
        drop(range);
        drop(cf);
        unimplemented!()
    }
    fn batch_scan<I, R, C>(
        &self,
        ranges: I,
        _each_limit: u32,
        _key_only: bool,
        cf: C,
    ) -> KvFuture<Vec<KvPair>>
    where
        I: IntoIterator<Item = R>,
        R: Into<KeyRange>,
        C: Into<Option<String>>,
    {
        drop(ranges);
        drop(cf);
        unimplemented!()
    }
    fn delete_range<R, C>(&self, range: R, cf: C) -> KvFuture<()>
    where
        R: Into<KeyRange>,
        C: Into<Option<String>>,
    {
        drop(range);
        drop(cf);
        unimplemented!()
    }
}

impl Txn for Client {
    fn begin(&self) -> KvFuture<Transaction> {
        unimplemented!()
    }

    fn begin_with_timestamp(&self, _timestamp: Timestamp) -> KvFuture<Transaction> {
        unimplemented!()
    }

    fn snapshot(&self) -> KvFuture<Snapshot> {
        unimplemented!()
    }

    fn current_timestamp(&self) -> Timestamp {
        unimplemented!()
    }

    fn oracle(&self) -> Oracle {
        unimplemented!()
    }
}
