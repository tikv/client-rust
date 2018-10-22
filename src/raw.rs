use {Config, Key, KeyRange, KvFuture, KvPair, Value};

pub trait Client {
    fn new(_config: &Config) -> KvFuture<Self> {
        unimplemented!()
    }

    fn get<K, C>(&self, key: K, cf: C) -> KvFuture<Value>
    where
        K: Into<Key>,
        C: Into<Option<String>>;

    fn batch_get<I, K, C>(&self, keys: I, cf: C) -> KvFuture<Vec<KvPair>>
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
        C: Into<Option<String>>;

    fn put<P, C>(&self, pair: P, cf: C) -> KvFuture<()>
    where
        P: Into<KvPair>,
        C: Into<Option<String>>;

    fn batch_put<I, P, C>(&self, pairs: I, cf: C) -> KvFuture<()>
    where
        I: IntoIterator<Item = P>,
        P: Into<KvPair>,
        C: Into<Option<String>>;

    fn delete<K, C>(&self, key: K, cf: C) -> KvFuture<()>
    where
        K: Into<Key>,
        C: Into<Option<String>>;

    fn batch_delete<I, K, C>(&self, keys: I, cf: C) -> KvFuture<()>
    where
        I: IntoIterator<Item = K>,
        K: Into<Key>,
        C: Into<Option<String>>;

    fn scan<R, C>(&self, range: R, limit: u32, key_only: bool, cf: C) -> KvFuture<Vec<KvPair>>
    where
        R: Into<KeyRange>,
        C: Into<Option<String>>;

    fn batch_scan<I, R, C>(
        &self,
        ranges: I,
        each_limit: u32,
        key_only: bool,
        cf: C,
    ) -> KvFuture<Vec<KvPair>>
    where
        I: IntoIterator<Item = R>,
        R: Into<KeyRange>,
        C: Into<Option<String>>;

    fn delete_range<R, C>(&self, range: R, cf: C) -> KvFuture<()>
    where
        R: Into<KeyRange>,
        C: Into<Option<String>>;
}
