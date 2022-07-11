use core::intrinsics::copy;
use std::{
    borrow::Cow,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use tikv_client_common::Error;
use tikv_client_proto::{errorpb, kvrpcpb, metapb::Region};

use crate::{kv::codec::decode_bytes_in_place, request::KvRequest, Key, Result};

type Prefix = [u8; KEYSPACE_PREFIX_LEN];

const KEYSPACE_PREFIX_LEN: usize = 4;

const MAX_KEYSPACE_ID: KeySpaceId = KeySpaceId([0xff, 0xff, 0xff]);

pub trait RequestCodec: Sized + Clone + Sync + Send + 'static {
    fn encode_request<'a, R: KvRequest<Self>>(&self, req: &'a R) -> Cow<'a, R> {
        Cow::Borrowed(req)
    }

    fn encode_key(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }

    fn decode_key(&self, _key: &mut Vec<u8>) -> Result<()> {
        Ok(())
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        (start, end)
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        key
    }

    fn decode_region(&self, _region: &mut Region) -> Result<()> {
        Ok(())
    }

    fn decode_response<R: KvRequest<Self>>(
        &self,
        _req: &R,
        resp: R::Response,
    ) -> Result<R::Response> {
        Ok(resp)
    }
}

pub trait RequestCodecExt: RequestCodec {
    fn encode_primary_lock(&self, lock: Vec<u8>) -> Vec<u8> {
        self.encode_key(lock)
    }

    fn encode_primary_key(&self, key: Vec<u8>) -> Vec<u8> {
        self.encode_key(key)
    }

    fn encode_mutations(&self, mutations: Vec<kvrpcpb::Mutation>) -> Vec<kvrpcpb::Mutation> {
        mutations
            .into_iter()
            .map(|mut m| {
                let key = m.take_key();
                m.set_key(self.encode_key(key));
                m
            })
            .collect()
    }

    fn encode_keys(&self, keys: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        keys.into_iter().map(|key| self.encode_key(key)).collect()
    }

    fn encode_pairs(&self, mut pairs: Vec<kvrpcpb::KvPair>) -> Vec<kvrpcpb::KvPair> {
        for pair in pairs.iter_mut() {
            *pair.mut_key() = self.encode_key(pair.take_key());
        }

        pairs
    }

    fn encode_ranges(&self, mut ranges: Vec<kvrpcpb::KeyRange>) -> Vec<kvrpcpb::KeyRange> {
        for range in ranges.iter_mut() {
            let (start, end) = self.encode_range(range.take_start_key(), range.take_end_key());
            *range.mut_start_key() = start;
            *range.mut_end_key() = end;
        }

        ranges
    }

    fn decode_keys(&self, keys: &mut [Vec<u8>]) -> Result<()> {
        for key in keys.iter_mut() {
            self.decode_key(key)?;
        }

        Ok(())
    }

    fn decode_error(&self, err: &mut kvrpcpb::KeyError) -> Result<()> {
        if err.has_locked() {
            let locked = err.mut_locked();
            self.decode_lock_info(locked)?;
        }

        if err.has_conflict() {
            let conflict = err.mut_conflict();
            self.decode_key(conflict.mut_key())?;
            self.decode_key(conflict.mut_primary())?;
        }

        if err.has_already_exist() {
            let already_exist = err.mut_already_exist();
            self.decode_key(already_exist.mut_key())?;
        }

        // We do not decode key in `Deadlock` since there is no use for the key right now in client side.
        // All we need is the key hash to detect deadlock.
        // TODO: while we check the keys against the deadlock key hash, we need to encode the key.

        if err.has_commit_ts_expired() {
            let commit_ts_expired = err.mut_commit_ts_expired();
            self.decode_key(commit_ts_expired.mut_key())?;
        }

        if err.has_txn_not_found() {
            let txn_not_found = err.mut_txn_not_found();
            self.decode_key(txn_not_found.mut_primary_key())?;
        }

        if err.has_assertion_failed() {
            let assertion_failed = err.mut_assertion_failed();
            self.decode_key(assertion_failed.mut_key())?;
        }

        Ok(())
    }

    fn decode_errors(&self, errors: &mut [kvrpcpb::KeyError]) -> Result<()> {
        for err in errors.iter_mut() {
            self.decode_error(err)?;
        }

        Ok(())
    }

    fn decode_lock_info(&self, lock: &mut kvrpcpb::LockInfo) -> Result<()> {
        self.decode_key(lock.mut_primary_lock())?;
        self.decode_key(lock.mut_key())?;
        self.decode_keys(lock.mut_secondaries())
    }

    fn decode_locks(&self, locks: &mut [kvrpcpb::LockInfo]) -> Result<()> {
        for lock in locks.iter_mut() {
            self.decode_lock_info(lock)?;
        }

        Ok(())
    }

    fn decode_pairs(&self, pairs: &mut [kvrpcpb::KvPair]) -> Result<()> {
        for pair in pairs.iter_mut() {
            self.decode_key(pair.mut_key())?;
        }

        Ok(())
    }

    fn decode_kvs(&self, kvs: &mut [kvrpcpb::KvPair]) -> Result<()> {
        self.decode_pairs(kvs)
    }

    fn decode_regions(&self, regions: &mut [Region]) -> Result<()> {
        for region in regions.iter_mut() {
            self.decode_region(region)?;
        }
        Ok(())
    }

    fn decode_region_error(&self, err: &mut errorpb::Error) -> Result<()> {
        if err.has_epoch_not_match() {
            self.decode_regions(err.mut_epoch_not_match().mut_current_regions())?;
        }
        Ok(())
    }
}

impl<T: RequestCodec> RequestCodecExt for T {}

pub trait RawCodec: RequestCodec {}

pub trait TxnCodec: RequestCodec {}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct KeySpaceId([u8; 3]);

impl Deref for KeySpaceId {
    type Target = [u8; 3];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KeySpaceId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait Mode: Clone + Copy + Sync + Send + 'static {
    const PREFIX: u8;
    const MIN_KEY: &'static [u8] = &[Self::PREFIX, 0, 0, 0];
    const MAX_KEY: &'static [u8] = &[Self::PREFIX + 1, 0, 0, 0];
}

#[derive(Clone, Copy)]
pub struct RawMode;

#[derive(Clone, Copy)]
pub struct TxnMode;

impl Mode for RawMode {
    const PREFIX: u8 = b'r';
}

impl Mode for TxnMode {
    const PREFIX: u8 = b't';
}

#[derive(Clone)]
pub struct ApiV1<M: Mode> {
    _phantom: PhantomData<M>,
}

impl<M: Mode> Default for ApiV1<M> {
    fn default() -> Self {
        ApiV1 {
            _phantom: PhantomData,
        }
    }
}

impl RequestCodec for ApiV1<RawMode> {}

impl RawCodec for ApiV1<RawMode> {}

impl RequestCodec for ApiV1<TxnMode> {
    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        Key::from(key).to_encoded().into()
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;

        Ok(())
    }
}

impl TxnCodec for ApiV1<TxnMode> {}

#[derive(Clone, Copy, Default)]
pub struct KeySpace<M: Mode> {
    id: KeySpaceId,
    _phantom: PhantomData<M>,
}

impl<M: Mode> From<KeySpace<M>> for Prefix {
    fn from(s: KeySpace<M>) -> Self {
        [M::PREFIX, s.id[0], s.id[1], s.id[2]]
    }
}

#[derive(Default, Clone)]
pub struct ApiV2<M: Mode> {
    keyspace: KeySpace<M>,
}

impl<M: Mode> RequestCodec for ApiV2<M> {
    fn encode_request<'a, R: KvRequest<Self>>(&self, req: &'a R) -> Cow<'a, R> {
        let mut req = req.clone();
        req.mut_context().set_api_version(kvrpcpb::ApiVersion::V2);
        Cow::Owned(req.encode_request(self))
    }

    fn encode_key(&self, mut key: Vec<u8>) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(key.len() + KEYSPACE_PREFIX_LEN);
        let prefix: Prefix = self.keyspace.into();

        encoded.extend_from_slice(&prefix);
        encoded.append(&mut key);
        encoded
    }

    fn decode_key(&self, key: &mut Vec<u8>) -> Result<()> {
        let prefix: Prefix = self.keyspace.into();

        if !key.starts_with(&prefix) {
            return Err(Error::CorruptedKeyspace {
                expected: prefix.to_vec(),
                actual: key[..KEYSPACE_PREFIX_LEN].to_vec(),
                key: key.to_vec(),
            });
        }

        unsafe {
            let trimmed_len = key.len() - KEYSPACE_PREFIX_LEN;
            let ptr = key.as_mut_ptr();
            let trimmed = key[KEYSPACE_PREFIX_LEN..].as_mut_ptr();

            copy(trimmed, ptr, trimmed_len);

            key.set_len(trimmed_len);
        }
        Ok(())
    }

    fn encode_range(&self, start: Vec<u8>, end: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        if self.keyspace.id == MAX_KEYSPACE_ID {
            (self.encode_key(start), M::MAX_KEY.to_vec())
        } else {
            (self.encode_key(start), self.encode_key(end))
        }
    }

    fn encode_pd_query(&self, key: Vec<u8>) -> Vec<u8> {
        Key::from(self.encode_key(key)).to_encoded().into()
    }

    fn decode_region(&self, region: &mut Region) -> Result<()> {
        decode_bytes_in_place(region.mut_start_key(), false)?;
        decode_bytes_in_place(region.mut_end_key(), false)?;

        // Map the region's start key to the keyspace start key.
        if region.get_start_key() < M::MIN_KEY {
            *region.mut_start_key() = vec![];
        } else {
            self.decode_key(region.mut_start_key())?;
        }

        // Map the region's end key to the keyspace end key.
        if region.get_end_key().is_empty() || region.get_end_key() > M::MAX_KEY {
            *region.mut_end_key() = vec![];
        } else {
            self.decode_key(region.mut_end_key())?;
        }

        Ok(())
    }

    fn decode_response<R: KvRequest<Self>>(
        &self,
        req: &R,
        resp: R::Response,
    ) -> Result<R::Response> {
        req.decode_response(self, resp)
    }
}
