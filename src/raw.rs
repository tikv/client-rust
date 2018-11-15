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

pub struct Get<'a, Impl: Client + 'a> {
    client: &'a Impl,
    key: Key,
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> Get<'a, Impl> {
    fn new(client: &'a Impl, key: Key) -> Self {
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

impl<'a, Impl: Client + 'a> Future for Get<'a, Impl> {
    type Item = Value;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchGet<'a, Impl: Client + 'a> {
    client: &'a Impl,
    keys: Vec<Key>,
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> BatchGet<'a, Impl> {
    fn new(client: &'a Impl, keys: Vec<Key>) -> Self {
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

impl<'a, Impl: Client + 'a> Future for BatchGet<'a, Impl> {
    type Item = Vec<KvPair>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Put<'a, Impl: Client + 'a> {
    client: &'a Impl,
    pair: KvPair,
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> Put<'a, Impl> {
    fn new(client: &'a Impl, pair: KvPair) -> Self {
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

impl<'a, Impl: Client + 'a> Future for Put<'a, Impl> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.pair;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchPut<'a, Impl: Client + 'a> {
    client: &'a Impl,
    pairs: Vec<KvPair>,
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> BatchPut<'a, Impl> {
    fn new(client: &'a Impl, pairs: Vec<KvPair>) -> Self {
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

impl<'a, Impl: Client + 'a> Future for BatchPut<'a, Impl> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.pairs;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Delete<'a, Impl: Client + 'a> {
    client: &'a Impl,
    key: Key,
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> Delete<'a, Impl> {
    fn new(client: &'a Impl, key: Key) -> Self {
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

impl<'a, Impl: Client + 'a> Future for Delete<'a, Impl> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.key;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct BatchDelete<'a, Impl: Client + 'a> {
    client: &'a Impl,
    keys: Vec<Key>,
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> BatchDelete<'a, Impl> {
    fn new(client: &'a Impl, keys: Vec<Key>) -> Self {
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

impl<'a, Impl: Client + 'a> Future for BatchDelete<'a, Impl> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let _ = &self.client;
        let _ = &self.keys;
        let _ = &self.cf;
        unimplemented!()
    }
}

pub struct Scan<'a, Impl: Client + 'a> {
    client: &'a Impl,
    range: (Key, Key),
    limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
    reverse: bool,
}

impl<'a, Impl: Client + 'a> Scan<'a, Impl> {
    fn new(client: &'a Impl, range: (Key, Key), limit: u32) -> Self {
        Scan {
            client,
            range,
            limit,
            key_only: false,
            cf: None,
            reverse: false,
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

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl<'a, Impl: Client + 'a> Future for Scan<'a, Impl> {
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

pub struct BatchScan<'a, Impl: Client + 'a> {
    client: &'a Impl,
    ranges: Vec<(Key, Key)>,
    each_limit: u32,
    key_only: bool,
    cf: Option<ColumnFamily>,
    reverse: bool,
}

impl<'a, Impl: Client + 'a> BatchScan<'a, Impl> {
    fn new(client: &'a Impl, ranges: Vec<(Key, Key)>, each_limit: u32) -> Self {
        BatchScan {
            client,
            ranges,
            each_limit,
            key_only: false,
            cf: None,
            reverse: false,
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

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl<'a, Impl: Client + 'a> Future for BatchScan<'a, Impl> {
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

pub struct DeleteRange<'a, Impl: Client + 'a> {
    client: &'a Impl,
    range: (Key, Key),
    cf: Option<ColumnFamily>,
}

impl<'a, Impl: Client + 'a> DeleteRange<'a, Impl> {
    fn new(client: &'a Impl, range: (Key, Key)) -> Self {
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

impl<'a, Impl: Client + 'a> Future for DeleteRange<'a, Impl> {
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
    type Impl: Client;

    fn get(&self, key: impl AsRef<Key>) -> Get<Self::Impl>;

    fn batch_get(&self, keys: impl AsRef<[Key]>) -> BatchGet<Self::Impl>;

    fn put(&self, pair: impl Into<KvPair>) -> Put<Self::Impl>;

    fn batch_put(&self, pairs: impl IntoIterator<Item = impl Into<KvPair>>)
        -> BatchPut<Self::Impl>;

    fn delete(&self, key: impl AsRef<Key>) -> Delete<Self::Impl>;

    fn batch_delete(&self, keys: impl AsRef<[Key]>) -> BatchDelete<Self::Impl>;

    fn scan(&self, range: impl RangeBounds<Key>, limit: u32) -> Scan<Self::Impl>;

    fn batch_scan<Ranges, Bounds>(&self, ranges: Ranges, each_limit: u32) -> BatchScan<Self::Impl>
    where
        Ranges: AsRef<[Bounds]>,
        Bounds: RangeBounds<Key>;

    fn delete_range(&self, range: impl RangeBounds<Key>) -> DeleteRange<Self::Impl>;
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
    type Impl = Self;

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
