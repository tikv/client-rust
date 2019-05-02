// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::ops::{
    Bound, Deref, DerefMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};
use std::{fmt, str, u8};

use crate::{Error, Result};

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
    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.0
    }

    #[inline]
    pub(crate) fn push(&mut self, v: u8) {
        self.0.push(v)
    }

    #[inline]
    pub(crate) fn pop(&mut self) {
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
    pub(crate) fn into_inner(self) -> Vec<u8> {
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
                None | Some(&mut u8::MAX) => v.push(0),
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
                Some(&mut u8::MIN) => v.pop(),
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

fn convert_to_bound_key(b: Bound<impl Into<Key>>) -> Bound<Key> {
    match b {
        Bound::Included(k) => Bound::Included(k.into()),
        Bound::Excluded(k) => Bound::Excluded(k.into()),
        Bound::Unbounded => Bound::Unbounded,
    }
}
