use std::ops::RangeBounds;

use {Config, Key, KvFuture, KvPair, Value};

pub struct RawClient;

impl RawClient {
    pub fn new(_config: &Config) -> KvFuture<Self> {
        unimplemented!()
    }
}

impl Client for RawClient {
    fn get<K, C>(&self, _key: K, _cf: C) -> KvFuture<Value>
    where
        K: AsRef<Key>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn batch_get<K, C>(&self, _keys: K, _cf: C) -> KvFuture<Vec<KvPair>>
    where
        K: AsRef<[Key]>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn put<P, C>(&self, _pair: P, _cf: C) -> KvFuture<()>
    where
        P: Into<KvPair>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn batch_put<I, P, C>(&self, _pairs: I, _cf: C) -> KvFuture<()>
    where
        I: IntoIterator<Item = P>,
        P: Into<KvPair>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn delete<K, C>(&self, _key: K, _cf: C) -> KvFuture<()>
    where
        K: AsRef<Key>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn batch_delete<K, C>(&self, _keys: K, _cf: C) -> KvFuture<()>
    where
        K: AsRef<[Key]>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn scan<R, C>(&self, _range: R, _limit: u32, _key_only: bool, _cf: C) -> KvFuture<Vec<KvPair>>
    where
        R: RangeBounds<Key>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn batch_scan<R, B, C>(
        &self,
        _ranges: R,
        _each_limit: u32,
        _key_only: bool,
        _cf: C,
    ) -> KvFuture<Vec<KvPair>>
    where
        R: AsRef<[B]>,
        B: RangeBounds<Key>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }

    fn delete_range<R, C>(&self, _range: R, _cf: C) -> KvFuture<()>
    where
        R: RangeBounds<Key>,
        C: Into<Option<String>>,
    {
        unimplemented!()
    }
}

pub trait Client {
    fn get<K, C>(&self, key: K, cf: C) -> KvFuture<Value>
    where
        K: AsRef<Key>,
        C: Into<Option<String>>;

    fn batch_get<K, C>(&self, keys: K, cf: C) -> KvFuture<Vec<KvPair>>
    where
        K: AsRef<[Key]>,
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
        K: AsRef<Key>,
        C: Into<Option<String>>;

    fn batch_delete<K, C>(&self, keys: K, cf: C) -> KvFuture<()>
    where
        K: AsRef<[Key]>,
        C: Into<Option<String>>;

    fn scan<R, C>(&self, range: R, limit: u32, key_only: bool, cf: C) -> KvFuture<Vec<KvPair>>
    where
        R: RangeBounds<Key>,
        C: Into<Option<String>>;

    fn batch_scan<R, B, C>(
        &self,
        ranges: R,
        each_limit: u32,
        key_only: bool,
        cf: C,
    ) -> KvFuture<Vec<KvPair>>
    where
        R: AsRef<[B]>,
        B: RangeBounds<Key>,
        C: Into<Option<String>>;

    fn delete_range<R, C>(&self, range: R, cf: C) -> KvFuture<()>
    where
        R: RangeBounds<Key>,
        C: Into<Option<String>>;
}
