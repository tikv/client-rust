// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use derive_new::new;
use std::cmp::{Eq, PartialEq};
use std::convert::TryFrom;
use std::ops::{Bound, Range, RangeFrom, RangeInclusive};
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
#[derive(new, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Key(Vec<u8>);

impl Key {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    fn zero_terminated(&self) -> bool {
        self.0.last().map(|i| *i == 0).unwrap_or(false)
    }

    #[inline]
    fn push_zero(&mut self) {
        self.0.push(0)
    }

    #[inline]
    fn into_lower_bound(mut self) -> Bound<Key> {
        if self.zero_terminated() {
            self.0.pop().unwrap();
            Bound::Excluded(self)
        } else {
            Bound::Included(self)
        }
    }

    #[inline]
    fn into_upper_bound(mut self) -> Bound<Key> {
        if self.zero_terminated() {
            self.0.pop().unwrap();
            Bound::Included(self)
        } else {
            Bound::Excluded(self)
        }
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

impl Into<Vec<u8>> for Key {
    fn into(self) -> Vec<u8> {
        self.0
    }
}

impl<'a> Into<&'a [u8]> for &'a Key {
    fn into(self) -> &'a [u8] {
        &self.0
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
#[derive(new, Default, Clone, Eq, PartialEq, Hash)]
pub struct Value(Vec<u8>);

impl Value {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

impl Into<Vec<u8>> for Value {
    fn into(self) -> Vec<u8> {
        self.0
    }
}

impl<'a> Into<&'a [u8]> for &'a Value {
    fn into(self) -> &'a [u8] {
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

impl Into<(Key, Value)> for KvPair {
    fn into(self) -> (Key, Value) {
        (self.0, self.1)
    }
}

impl fmt::Debug for KvPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let KvPair(key, value) = self;
        match str::from_utf8(&value.0) {
            Ok(s) => write!(f, "KvPair({}, {:?})", HexRepr(&key.0), s),
            Err(_) => write!(f, "KvPair({}, {})", HexRepr(&key.0), HexRepr(&value.0)),
        }
    }
}

/// A struct for expressing ranges. This type is semi-opaque and is not really meant for users to
/// deal with directly. Most functions which operate on ranges will accept any types which
/// implement `Into<BoundRange>`.
///
/// In TiKV, keys are an ordered sequence of bytes. This means we can have ranges over those
/// bytes. Eg `001` is before `010`.
///
/// `Into<BoundRange>` has implementations for common range types like `a..b`, `a..=b` where `a` and `b`
/// `impl Into<Key>`. You can implement `Into<BoundRange>` for your own types by using `try_from`.
///
/// Invariant: a range may not be unbounded below.
///
/// ```rust
/// use tikv_client::{BoundRange, Key};
/// use std::ops::{Range, RangeInclusive, RangeTo, RangeToInclusive, RangeFrom, RangeFull, Bound};
/// # use std::convert::TryInto;
///
/// let explict_range: Range<Key> = Range { start: Key::from("Rust"), end: Key::from("TiKV") };
/// let from_explict_range: BoundRange = explict_range.into();
///
/// let range: Range<&str> = "Rust".."TiKV";
/// let from_range: BoundRange = range.into();
/// assert_eq!(from_explict_range, from_range);
///
/// let range: RangeInclusive<&str> = "Rust"..="TiKV";
/// let from_range: BoundRange = range.into();
/// assert_eq!(
///     from_range,
///     (Bound::Included(Key::from("Rust")), Bound::Included(Key::from("TiKV"))),
/// );
///
/// let range_from: RangeFrom<&str> = "Rust"..;
/// let from_range_from: BoundRange = range_from.into();
/// assert_eq!(
///     from_range_from,
///     (Bound::Included(Key::from("Rust")), Bound::Unbounded),
/// );
/// ```
///
/// **But, you should not need to worry about all this:** Most functions which operate
/// on ranges will accept any types  which implement `Into<BoundRange>`.
/// which means all of the above types can be passed directly to those functions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundRange {
    from: Bound<Key>,
    to: Bound<Key>,
}

impl BoundRange {
    /// Create a new BoundRange.
    ///
    /// The caller must ensure that `from` is not `Unbounded`.
    fn new(from: Bound<Key>, to: Bound<Key>) -> BoundRange {
        // Debug assert because this function is private.
        debug_assert!(from != Bound::Unbounded);
        BoundRange { from, to }
    }

    /// Ranges used in scanning TiKV have a particularity to them.
    ///
    /// The **start** of a scan is inclusive, unless appended with an '\0', then it is exclusive.
    ///
    /// The **end** of a scan is exclusive, unless appended with an '\0', then it is inclusive.
    ///
    /// ```rust
    /// use tikv_client::{BoundRange, Key};
    /// // Exclusive
    /// let range = "a".."z";
    /// assert_eq!(
    ///     BoundRange::from(range).into_keys(),
    ///     (Key::from("a"), Some(Key::from("z"))),
    /// );
    /// // Inclusive
    /// let range = "a"..="z";
    /// assert_eq!(
    ///     BoundRange::from(range).into_keys(),
    ///     (Key::from("a"), Some(Key::from("z\0"))),
    /// );
    /// // Open
    /// let range = "a"..;
    /// assert_eq!(
    ///     BoundRange::from(range).into_keys(),
    ///     (Key::from("a"), None),
    /// );
    // ```
    pub fn into_keys(self) -> (Key, Option<Key>) {
        let start = match self.from {
            Bound::Included(v) => v,
            Bound::Excluded(mut v) => {
                v.push_zero();
                v
            }
            Bound::Unbounded => unreachable!(),
        };
        let end = match self.to {
            Bound::Included(mut v) => {
                v.push_zero();
                Some(v)
            }
            Bound::Excluded(v) => Some(v),
            Bound::Unbounded => None,
        };
        (start, end)
    }
}

impl<T: Into<Key> + Clone> PartialEq<(Bound<T>, Bound<T>)> for BoundRange {
    fn eq(&self, other: &(Bound<T>, Bound<T>)) -> bool {
        self.from == convert_to_bound_key(other.0.clone())
            && self.to == convert_to_bound_key(other.1.clone())
    }
}

impl<T: Into<Key>> From<Range<T>> for BoundRange {
    fn from(other: Range<T>) -> BoundRange {
        BoundRange::new(
            Bound::Included(other.start.into()),
            Bound::Excluded(other.end.into()),
        )
    }
}

impl<T: Into<Key>> From<RangeFrom<T>> for BoundRange {
    fn from(other: RangeFrom<T>) -> BoundRange {
        BoundRange::new(Bound::Included(other.start.into()), Bound::Unbounded)
    }
}

impl<T: Into<Key>> From<RangeInclusive<T>> for BoundRange {
    fn from(other: RangeInclusive<T>) -> BoundRange {
        let (start, end) = other.into_inner();
        BoundRange::new(Bound::Included(start.into()), Bound::Included(end.into()))
    }
}

impl<T: Into<Key>> From<(T, Option<T>)> for BoundRange {
    fn from(other: (T, Option<T>)) -> BoundRange {
        let to = match other.1 {
            None => Bound::Unbounded,
            Some(to) => to.into().into_upper_bound(),
        };

        BoundRange::new(other.0.into().into_lower_bound(), to)
    }
}

impl<T: Into<Key>> From<(T, T)> for BoundRange {
    fn from(other: (T, T)) -> BoundRange {
        BoundRange::new(
            other.0.into().into_lower_bound(),
            other.1.into().into_upper_bound(),
        )
    }
}

impl<T: Into<Key> + Eq> TryFrom<(Bound<T>, Bound<T>)> for BoundRange {
    type Error = Error;

    fn try_from(bounds: (Bound<T>, Bound<T>)) -> Result<BoundRange> {
        if bounds.0 == Bound::Unbounded {
            Err(Error::invalid_key_range())
        } else {
            Ok(BoundRange::new(
                convert_to_bound_key(bounds.0),
                convert_to_bound_key(bounds.1),
            ))
        }
    }
}

fn convert_to_bound_key<K: Into<Key>>(b: Bound<K>) -> Bound<Key> {
    match b {
        Bound::Included(k) => Bound::Included(k.into()),
        Bound::Excluded(k) => Bound::Excluded(k.into()),
        Bound::Unbounded => Bound::Unbounded,
    }
}
