use std::ops::RangeBounds;

use futures::{Future, Poll};

use {Config, Key, KvFuture, KvPair, Value};

#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ColumnFamily(String);

impl Into<ColumnFamily> for String {
    fn into(self) -> ColumnFamily {
        ColumnFamily(self)
    }
}

impl<'a> Into<ColumnFamily> for &'a str {
    fn into(self) -> ColumnFamily {
        ColumnFamily(self.to_owned())
    }
}

pub struct Get<'a, AClient: Client + 'a> {
    client: &'a AClient,
    key: Key,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> Get<'a, AClient> {
    fn new(client: &'a AClient, key: Key) -> Self {
        Get {
            client,
            key,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for Get<'a, AClient> {
    type Item = Value;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchGet<'a, AClient: Client + 'a> {
    client: &'a AClient,
    keys: Vec<Key>,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> BatchGet<'a, AClient> {
    fn new(client: &'a AClient, keys: Vec<Key>) -> Self {
        BatchGet {
            client,
            keys,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for BatchGet<'a, AClient> {
    type Item = Vec<KvPair>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Put<'a, AClient: Client + 'a> {
    client: &'a AClient,
    pair: KvPair,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> Put<'a, AClient> {
    fn new(client: &'a AClient, pair: KvPair) -> Self {
        Put {
            client,
            pair,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for Put<'a, AClient> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.pair;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchPut<'a, AClient: Client + 'a> {
    client: &'a AClient,
    pairs: Vec<KvPair>,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> BatchPut<'a, AClient> {
    fn new(client: &'a AClient, pairs: Vec<KvPair>) -> Self {
        BatchPut {
            client,
            pairs,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for BatchPut<'a, AClient> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.pairs;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Delete<'a, AClient: Client + 'a> {
    client: &'a AClient,
    key: Key,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> Delete<'a, AClient> {
    fn new(client: &'a AClient, key: Key) -> Self {
        Delete {
            client,
            key,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for Delete<'a, AClient> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchDelete<'a, AClient: Client + 'a> {
    client: &'a AClient,
    keys: Vec<Key>,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> BatchDelete<'a, AClient> {
    fn new(client: &'a AClient, keys: Vec<Key>) -> Self {
        BatchDelete {
            client,
            keys,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for BatchDelete<'a, AClient> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Scan<'a, AClient: Client + 'a> {
    client: &'a AClient,
    range: (Key, Key),
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> Scan<'a, AClient> {
    fn new(client: &'a AClient, range: (Key, Key), limit: u32) -> Self {
        Scan {
            client,
            range,
            limit,
            key_only: false,
            cf: None,
        }
    }

    pub fn key_only(mut self) -> Self {
        self.key_only = true;
        self
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for Scan<'a, AClient> {
    type Item = Vec<KvPair>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.range;
        let _ = &self.limit;
        let _ = &self.key_only;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchScan<'a, AClient: Client + 'a> {
    client: &'a AClient,
    ranges: Vec<(Key, Key)>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> BatchScan<'a, AClient> {
    fn new(client: &'a AClient, ranges: Vec<(Key, Key)>, each_limit: u32) -> Self {
        BatchScan {
            client,
            ranges,
            each_limit,
            key_only: false,
            cf: None,
        }
    }

    pub fn key_only(mut self) -> Self {
        self.key_only = true;
        self
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for BatchScan<'a, AClient> {
    type Item = Vec<KvPair>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.ranges;
        let _ = &self.each_limit;
        let _ = &self.key_only;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct DeleteRange<'a, AClient: Client + 'a> {
    client: &'a AClient,
    range: (Key, Key),
    cf: Option<ColumnFamily>,
}

impl<'a, AClient: Client + 'a> DeleteRange<'a, AClient> {
    fn new(client: &'a AClient, range: (Key, Key)) -> Self {
        DeleteRange {
            client,
            range,
            cf: None,
        }
    }

    pub fn cf(mut self, cf: impl Into<ColumnFamily>) -> Self {
        self.cf = Some(cf.into());
        self
    }
}

impl<'a, AClient: Client + 'a> Future for DeleteRange<'a, AClient> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.range;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub trait Client {
    type AClient: Client;

    fn get(&self, key: impl AsRef<Key>) -> Get<Self::AClient>;

    fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet<Self::AClient>;

    fn put(&self, pair: impl Into<KvPair>) -> Put<Self::AClient>;

    fn batch_put(
        &self,
        pairs: impl IntoIterator<Item = impl Into<KvPair>>,
    ) -> BatchPut<Self::AClient>;

    fn delete(&self, key: impl AsRef<Key>) -> Delete<Self::AClient>;

    fn batch_delete(&self, keys: impl AsRef<[Key]>) -> BatchDelete<Self::AClient>;

    fn scan(&self, range: impl RangeBounds<Key>, limit: u32) -> Scan<Self::AClient>;

    fn batch_scan<Ranges, Bounds>(
        &self,
        ranges: Ranges,
        each_limit: u32,
    ) -> BatchScan<Self::AClient>
    where
        Ranges: AsRef<[Bounds]>,
        Bounds: RangeBounds<Key>;

    fn delete_range(&self, range: impl RangeBounds<Key>) -> DeleteRange<Self::AClient>;
}

pub struct RawClient;

impl RawClient {
    pub fn new(_config: &Config) -> KvFuture<Self> {
        unimplemented!()
    }

    fn extract_range(_range: &impl RangeBounds<Key>) -> (Key, Key) {
        unimplemented!()
    }
}

impl Client for RawClient {
    type AClient = Self;

    fn get(&self, key: impl AsRef<Key>) -> Get<Self> {
        Get::new(self, key.as_ref().clone())
    }

    fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet<Self> {
        BatchGet::new(self, keys.as_ref().to_vec())
    }

    fn put(&self, pair: impl Into<KvPair>) -> Put<Self> {
        Put::new(self, pair.into())
    }

    fn batch_put(&self, pairs: impl IntoIterator<Item = impl Into<KvPair>>) -> BatchPut<Self> {
        BatchPut::new(self, pairs.into_iter().map(Into::into).collect())
    }

    fn delete(&self, key: impl AsRef<Key>) -> Delete<Self> {
        Delete::new(self, key.as_ref().clone())
    }

    fn batch_delete(&self, keys: impl AsRef<[Key]>) -> BatchDelete<Self> {
        BatchDelete::new(self, keys.as_ref().to_vec())
    }

    fn scan(&self, range: impl RangeBounds<Key>, limit: u32) -> Scan<Self> {
        Scan::new(self, Self::extract_range(&range), limit)
    }

    fn batch_scan<Ranges, Bounds>(&self, ranges: Ranges, each_limit: u32) -> BatchScan<Self>
    where
        Ranges: AsRef<[Bounds]>,
        Bounds: RangeBounds<Key>,
    {
        BatchScan::new(
            self,
            ranges.as_ref().iter().map(Self::extract_range).collect(),
            each_limit,
        )
    }

    fn delete_range(&self, range: impl RangeBounds<Key>) -> DeleteRange<Self> {
        DeleteRange::new(self, Self::extract_range(&range))
    }
}
