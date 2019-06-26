// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::HexRepr;
#[cfg(test)]
use proptest::{arbitrary::any_with, collection::size_range};
#[cfg(test)]
use proptest_derive::Arbitrary;
use std::ops::{Bound, Deref, DerefMut};
use std::{fmt, str, u8};

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
#[cfg_attr(test, derive(Arbitrary))]
pub struct Key(
    #[cfg_attr(
        test,
        proptest(
            strategy = "any_with::<Vec<u8>>((size_range(crate::proptests::PROPTEST_KEY_MAX), ()))"
        )
    )]
    Vec<u8>,
);

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
    pub(super) fn zero_terminated(&self) -> bool {
        self.0.last().map(|i| *i == 0).unwrap_or(false)
    }

    #[inline]
    pub(super) fn push_zero(&mut self) {
        self.0.push(0)
    }

    #[inline]
    pub(super) fn into_lower_bound(mut self) -> Bound<Key> {
        if self.zero_terminated() {
            self.0.pop().unwrap();
            Bound::Excluded(self)
        } else {
            Bound::Included(self)
        }
    }

    #[inline]
    pub(super) fn into_upper_bound(mut self) -> Bound<Key> {
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
