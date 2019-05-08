// Copyright 2018 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

// Long and nested future chains can quickly result in large generic types.
#![type_length_limit = "16777216"]
#![allow(clippy::redundant_closure)]

//! TiKV Client for Rust.
//!
//! > Currently this crate is experimental and some portions (eg the Transactional API) are still
//! > in active development. You're encouraged to use this library for testing and help us find
//! > problems!
//!
//! This crate provides a clean, ready to use client for [TiKV](https://github.com/tikv/tikv), a
//! distributed transactional Key-Value database written in Rust.
//!
//! With this crate you can easily connect to any TiKV deployment, interact with it, and mutate the
//! data it contains.
//!
//! This is an open source (Apache 2) project hosted by the Cloud Native Computing Foundation
//! (CNCF) and maintained by the TiKV Authors. *We'd love it if you joined us in improving this
//! project.*
//!
//! ## Install
//!
//! There are no special requirements to use this. It is a Rust 2018 edition crate supporting
//! stable and nightly.
//!
//! To use this crate in your project, add it as a dependency in the `Cargo.toml` of your Rust project:
//!
//! ```toml
//! [dependencies]
//! # ...Your other dependencies...
//! tikv-client = "~0.1"
//! futures = "0.1" # You'll need this later.
//! ```
//!
//! Then run a `cargo build --package tikv-client` command to test building the crate.
//!
//! Next, you need to choose the API appropriate for your needs.
//!
//! ## Choosing an API
//!
//! This crate offers both [**raw**](raw/index.html) and
//! [**transactional**](transaction/index.html) APIs. You should choose just one for your system.
//!
//! The *consequence* of supporting transactions is increased overhead of coordination with the
//! placement driver for timestamp acquisition. This is approximately 1 RTT.
//!
//! *While it is possible to use both APIs at the same time, doing so is unsafe and unsupported.*
//!
//! Choose the one that suites your needs as described below, then add the import statement to your
//! file where you need to use the library.
//!
//! ### Transactional
//!
//! The [transactional](transaction/index.html) API supports **transactions** via Multi-Version
//! Concurrency Control (MVCC).
//!
//! **Best when you mostly do** complex sets of actions, actions which may require a rollback,
//! operations affecting multiple keys or values, or operations that depend on strong ordering.
//!
//! ```rust
//! use tikv_client::{*, transaction::*};
//! ```
//!
//! ### Raw
//!
//! The [raw](raw/index.html) API has **reduced coordination overhead**, but lacks any
//! transactional abilities.
//!
//! **Best when you mostly do** single row changes, and have very limited cross-row (eg. foreign
//! key) requirements. You will not be able to use transactions with this API.
//!
//! ```rust
//! use tikv_client::{*, raw::*};
//! ```
//!
//! ## Connect
//!
//! Regardless of which API you choose, you'll need to connect your client
//! ([raw](raw/struct.Client.html), [transactional](transaction/struct.Client.html)).
//!
//! ```rust
//! # use tikv_client::{*, raw::*};
//! use futures::Future;
//!
//! // Configure endpoints and optional TLS.
//! let config = Config::new(vec![ // A list of PD endpoints.
//!     "192.168.0.100:2379",
//!     "192.168.0.101:2379",
//! ]).with_security("root.ca", "internal.cert", "internal.key");
//!
//! // Get an unresolved connection.
//! let connect = Client::new(config);
//!
//! // Resolve the connection into a client.
//! let client = connect.wait();
//! ```
//!
//! At this point, you should seek the documentation in the related API modules.
//!

use futures::Future;
use serde_derive::*;
use std::{
    fmt,
    ops::{
        Bound, Deref, DerefMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo,
        RangeToInclusive,
    },
    path::PathBuf,
    str,
    time::Duration,
    u8::{MAX as U8_MAX, MIN as U8_MIN},
};

mod errors;
pub mod raw;
mod rpc;
pub mod transaction;

#[doc(inline)]
pub use crate::errors::Error;
#[doc(inline)]
pub use crate::errors::ErrorKind;
#[doc(inline)]
pub use crate::errors::Result;

struct HexRepr<'a>(pub &'a [u8]);

impl<'a> fmt::Display for HexRepr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}

/// The key part of a key/value pair.
///
/// In TiKV, keys are an ordered sequence of bytes. This has an advantage over choosing `String` as
/// valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
///
/// This is a *wrapper type* that implements `Deref<Target=[u8]>` so it can be used like one transparently.
///
/// This type also implements `From` for many types. With one exception, these are all done without
/// reallocation. Using a `&'static str`, like many examples do for simplicity, has an internal
/// allocation cost.
///
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`
/// over a `&str` or `&[u8]`.
///
/// ```rust
/// use tikv_client::Key;
///
/// let static_str: &'static str = "TiKV";
/// let from_static_str = Key::from(static_str);
///
/// let string: String = String::from(static_str);
/// let from_string = Key::from(string);
/// assert_eq!(from_static_str, from_string);
///
/// let vec: Vec<u8> = static_str.as_bytes().to_vec();
/// let from_vec = Key::from(vec);
/// assert_eq!(from_static_str, from_vec);
///
/// let bytes = static_str.as_bytes().to_vec();
/// let from_bytes = Key::from(bytes);
/// assert_eq!(from_static_str, from_bytes);
/// ```
///
/// **But, you should not need to worry about all this:** Many functions which accept a `Key`
/// accept an `Into<Key>`, which means all of the above types can be passed directly to those
/// functions.
#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Key(Vec<u8>);

impl Key {
    #[inline]
    pub fn new(value: Vec<u8>) -> Self {
        Key(value)
    }
    #[inline]
    fn into_inner(self) -> Vec<u8> {
        self.0
    }

    #[inline]
    fn push(&mut self, v: u8) {
        self.0.push(v)
    }

    #[inline]
    fn pop(&mut self) {
        self.0.pop();
    }
}

impl From<Vec<u8>> for Key {
    fn from(v: Vec<u8>) -> Self {
        Key(v)
    }
}

impl From<String> for Key {
    fn from(v: String) -> Key {
        Key(v.into_bytes())
    }
}

impl From<&'static str> for Key {
    fn from(v: &'static str) -> Key {
        Key(v.as_bytes().to_vec())
    }
}

impl AsRef<Key> for Key {
    fn as_ref(&self) -> &Key {
        self
    }
}

impl AsMut<Key> for Key {
    fn as_mut(&mut self) -> &mut Key {
        self
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for Key {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl Deref for Key {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Key {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Key({})", HexRepr(&self.0))
    }
}

/// The value part of a key/value pair.
///
/// In TiKV, values are an ordered sequence of bytes. This has an advantage over choosing `String`
/// as valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
///
/// This is a *wrapper type* that implements `Deref<Target=[u8]>` so it can be used like one transparently.
///
/// This type also implements `From` for many types. With one exception, these are all done without
/// reallocation. Using a `&'static str`, like many examples do for simplicity, has an internal
/// allocation cost.
///
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`
/// over a `&str` or `&[u8]`.
///
/// ```rust
/// use tikv_client::Value;
///
/// let static_str: &'static str = "TiKV";
/// let from_static_str = Value::from(static_str);
///
/// let string: String = String::from(static_str);
/// let from_string = Value::from(string);
/// assert_eq!(from_static_str, from_string);
///
/// let vec: Vec<u8> = static_str.as_bytes().to_vec();
/// let from_vec = Value::from(vec);
/// assert_eq!(from_static_str, from_vec);
///
/// let bytes = static_str.as_bytes().to_vec();
/// let from_bytes = Value::from(bytes);
/// assert_eq!(from_static_str, from_bytes);
/// ```
///
/// **But, you should not need to worry about all this:** Many functions which accept a `Value`
/// accept an `Into<Value>`, which means all of the above types can be passed directly to those
/// functions.
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Value(Vec<u8>);

impl Value {
    #[inline]
    pub fn new(value: Vec<u8>) -> Self {
        Value(value)
    }

    #[inline]
    fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Value {
        Value(v.into_bytes())
    }
}

impl From<&'static str> for Value {
    fn from(v: &'static str) -> Value {
        Value(v.as_bytes().to_vec())
    }
}

impl Deref for Value {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match str::from_utf8(&self.0) {
            Ok(s) => write!(f, "Value({:?})", s),
            Err(_) => write!(f, "Value({})", HexRepr(&self.0)),
        }
    }
}

/// A key/value pair.
///
/// ```rust
/// # use tikv_client::{Key, Value, KvPair};
/// let key = "key";
/// let value = "value";
/// let constructed = KvPair::new(key, value);
/// let from_tuple = KvPair::from((key, value));
/// assert_eq!(constructed, from_tuple);
/// ```
///
/// Many functions which accept a `KvPair` accept an `Into<KvPair>`, which means all of the above
/// types (Like a `(Key, Value)`) can be passed directly to those functions.
#[derive(Default, Clone, Eq, PartialEq)]
pub struct KvPair(Key, Value);

impl KvPair {
    /// Create a new `KvPair`.
    #[inline]
    pub fn new(key: impl Into<Key>, value: impl Into<Value>) -> Self {
        KvPair(key.into(), value.into())
    }

    /// Immutably borrow the `Key` part of the `KvPair`.
    #[inline]
    pub fn key(&self) -> &Key {
        &self.0
    }

    /// Immutably borrow the `Value` part of the `KvPair`.
    #[inline]
    pub fn value(&self) -> &Value {
        &self.1
    }

    #[inline]
    pub fn into_inner(self) -> (Key, Value) {
        (self.0, self.1)
    }

    #[inline]
    pub fn into_key(self) -> Key {
        self.0
    }

    #[inline]
    pub fn into_value(self) -> Value {
        self.1
    }

    /// Mutably borrow the `Key` part of the `KvPair`.
    #[inline]
    pub fn key_mut(&mut self) -> &mut Key {
        &mut self.0
    }

    /// Mutably borrow the `Value` part of the `KvPair`.
    #[inline]
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.1
    }

    /// Set the `Key` part of the `KvPair`.
    #[inline]
    pub fn set_key(&mut self, k: impl Into<Key>) {
        self.0 = k.into();
    }

    /// Set the `Value` part of the `KvPair`.
    #[inline]
    pub fn set_value(&mut self, v: impl Into<Value>) {
        self.1 = v.into();
    }
}

impl<K, V> From<(K, V)> for KvPair
where
    K: Into<Key>,
    V: Into<Value>,
{
    fn from((k, v): (K, V)) -> Self {
        KvPair(k.into(), v.into())
    }
}

impl fmt::Debug for KvPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let KvPair(key, value) = self;
        match str::from_utf8(&value) {
            Ok(s) => write!(f, "KvPair({}, {:?})", HexRepr(&key), s),
            Err(_) => write!(f, "KvPair({}, {})", HexRepr(&key), HexRepr(&value)),
        }
    }
}

/// The configuration for either a [`raw::Client`](raw/struct.Client.html) or a
/// [`transaction::Client`](transaction/struct.Client.html).
///
/// Because TiKV is managed by a [PD](https://github.com/pingcap/pd/) cluster, the endpoints for PD
/// must be provided, **not** the TiKV nodes.
///
/// It's important to **include more than one PD endpoint** (include all, if possible!)
/// This helps avoid having a *single point of failure*.
///
/// By default, this client will use an insecure connection over instead of one protected by
/// Transport Layer Security (TLS). Your deployment may have chosen to rely on security measures
/// such as a private network, or a VPN layer to provid secure transmission.
///
/// To use a TLS secured connection, use the `with_security` function to set the required
/// parameters.
///
/// TiKV does not currently offer encrypted storage (or encryption-at-rest).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pd_endpoints: Vec<String>,
    ca_path: Option<PathBuf>,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
    timeout: Duration,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

impl Config {
    /// Create a new [`Config`](struct.Config.html) which coordinates with the given PD endpoints.
    ///
    /// It's important to **include more than one PD endpoint** (include all, if possible!)
    /// This helps avoid having a *single point of failure*.
    ///
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::new(vec!["192.168.0.100:2379", "192.168.0.101:2379"]);
    /// ```
    pub fn new(pd_endpoints: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Config {
            pd_endpoints: pd_endpoints.into_iter().map(Into::into).collect(),
            ca_path: None,
            cert_path: None,
            key_path: None,
            timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Set the certificate authority, certificate, and key locations for the
    /// [`Config`](struct.Config.html).
    ///
    /// By default, TiKV connections do not utilize transport layer security. Enable it by setting
    /// these values.
    ///
    /// ```rust
    /// # use tikv_client::Config;
    /// let config = Config::new(vec!["192.168.0.100:2379", "192.168.0.101:2379"])
    ///     .with_security("root.ca", "internal.cert", "internal.key");
    /// ```
    pub fn with_security(
        mut self,
        ca_path: impl Into<PathBuf>,
        cert_path: impl Into<PathBuf>,
        key_path: impl Into<PathBuf>,
    ) -> Self {
        self.ca_path = Some(ca_path.into());
        self.cert_path = Some(cert_path.into());
        self.key_path = Some(key_path.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

pub type KvFuture<Resp> = Box<dyn Future<Item = Resp, Error = Error> + Send>;

/// A convenience trait for expressing ranges.
///
/// In TiKV, keys are an ordered sequence of bytes. This means we can have ranges over those
/// bytes. Eg `001` is before `010`.
///
/// This trait has implementations for common range types like `a..b`, `a..=b` where `a` and `b`
/// `impl Into<Key>`. You could implement this for your own types.
///
/// ```rust
/// use tikv_client::{KeyRange, Key};
/// use std::ops::{Range, RangeInclusive, RangeTo, RangeToInclusive, RangeFrom, RangeFull, Bound};
///
/// let explict_range: Range<Key> = Range { start: Key::from("Rust"), end: Key::from("TiKV") };
/// let from_explict_range = explict_range.into_bounds();
///
/// let range: Range<&str> = "Rust".."TiKV";
/// let from_range = range.into_bounds();
/// assert_eq!(from_explict_range, from_range);
///
/// let range: RangeInclusive<&str> = "Rust"..="TiKV";
/// let from_range = range.into_bounds();
/// assert_eq!(
///     (Bound::Included(Key::from("Rust")), Bound::Included(Key::from("TiKV"))),
///     from_range
/// );
///
/// let range_from: RangeFrom<&str> = "Rust"..;
/// let from_range_from = range_from.into_bounds();
/// assert_eq!(
///     (Bound::Included(Key::from("Rust")), Bound::Unbounded),
///     from_range_from,
/// );
///
/// let range_to: RangeTo<&str> = .."TiKV";
/// let from_range_to = range_to.into_bounds();
/// assert_eq!(
///     (Bound::Unbounded, Bound::Excluded(Key::from("TiKV"))),
///     from_range_to,
/// );
///
/// let range_to_inclusive: RangeToInclusive<&str> = ..="TiKV";
/// let from_range_to_inclusive = range_to_inclusive.into_bounds();
/// assert_eq!(
///     (Bound::Unbounded, Bound::Included(Key::from("TiKV"))),
///     from_range_to_inclusive,
/// );
///
/// let range_full: RangeFull = ..;
/// let from_range_full = range_full.into_bounds();
/// assert_eq!(
///     (Bound::Unbounded, Bound::Unbounded),
///     from_range_full
/// );
/// ```
///
/// **But, you should not need to worry about all this:** Many functions accept a `impl KeyRange`
/// which means all of the above types can be passed directly to those functions.
pub trait KeyRange: Sized {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>);
    fn into_keys(self) -> Result<(Key, Option<Key>)> {
        range_to_keys(self.into_bounds())
    }
}

fn range_to_keys(range: (Bound<Key>, Bound<Key>)) -> Result<(Key, Option<Key>)> {
    let start = match range.0 {
        Bound::Included(v) => v,
        Bound::Excluded(mut v) => {
            match v.last_mut() {
                None | Some(&mut U8_MAX) => v.push(0),
                Some(v) => *v += 1,
            }
            v
        }
        Bound::Unbounded => Err(Error::invalid_key_range())?,
    };
    let end = match range.1 {
        Bound::Included(v) => Some(v),
        Bound::Excluded(mut v) => Some({
            match v.last_mut() {
                None => (),
                Some(&mut U8_MIN) => v.pop(),
                Some(v) => *v -= 1,
            }
            v
        }),
        Bound::Unbounded => None,
    };
    Ok((start, end))
}

impl<T: Into<Key>> KeyRange for Range<T> {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        (
            Bound::Included(self.start.into()),
            Bound::Excluded(self.end.into()),
        )
    }
}

impl<T: Into<Key>> KeyRange for RangeFrom<T> {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        (Bound::Included(self.start.into()), Bound::Unbounded)
    }
}

impl KeyRange for RangeFull {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        (Bound::Unbounded, Bound::Unbounded)
    }
}

impl<T: Into<Key>> KeyRange for RangeInclusive<T> {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        let (start, end) = self.into_inner();
        (Bound::Included(start.into()), Bound::Included(end.into()))
    }
}

impl<T: Into<Key>> KeyRange for RangeTo<T> {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        (Bound::Unbounded, Bound::Excluded(self.end.into()))
    }
}

impl<T: Into<Key>> KeyRange for RangeToInclusive<T> {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        (Bound::Unbounded, Bound::Included(self.end.into()))
    }
}

impl<T: Into<Key>> KeyRange for (Bound<T>, Bound<T>) {
    fn into_bounds(self) -> (Bound<Key>, Bound<Key>) {
        (convert_to_bound_key(self.0), convert_to_bound_key(self.1))
    }
}

fn convert_to_bound_key<K>(b: Bound<K>) -> Bound<Key>
where
    K: Into<Key>,
{
    use std::ops::Bound::*;
    match b {
        Included(k) => Included(k.into()),
        Excluded(k) => Excluded(k.into()),
        Unbounded => Unbounded,
    }
}
