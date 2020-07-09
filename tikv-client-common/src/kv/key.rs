// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::HexRepr;
#[cfg(test)]
use proptest::{arbitrary::any_with, collection::size_range};
#[cfg(test)]
use proptest_derive::Arbitrary;
use std::ops::Bound;
use std::{fmt, u8};

/// The key part of a key/value pair.
///
/// In TiKV, keys are an ordered sequence of bytes. This has an advantage over choosing `String` as
/// valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
///
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`.
///
/// ```rust
/// use tikv_client::Key;
///
/// let static_str: &'static str = "TiKV";
/// let from_static_str = Key::from(static_str.to_owned());
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
/// While `.into()` is usually sufficient for obtaining the buffer itself, sometimes type inference
/// isn't able to determine the correct type. Notably in the `assert_eq!()` and `==` cases. In
/// these cases using the fully-qualified-syntax is useful:
///
/// ```rust
/// use tikv_client::Key;
///
/// let buf = "TiKV".as_bytes().to_owned();
/// let key = Key::from(buf.clone());
/// assert_eq!(Into::<Vec<u8>>::into(key), buf);
/// ```
///
/// Many functions which accept a `Key` accept an `Into<Key>`, which means all of the above types
/// can be passed directly to those functions.
#[derive(Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
#[repr(transparent)]
pub struct Key(
    #[cfg_attr(
        test,
        proptest(
            strategy = "any_with::<Vec<u8>>((size_range(crate::proptests::PROPTEST_KEY_MAX), ()))"
        )
    )]
    pub(super) Vec<u8>,
);

impl Key {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

impl AsRef<Key> for Key {
    fn as_ref(&self) -> &Key {
        self
    }
}

impl AsRef<Key> for Vec<u8> {
    fn as_ref(&self) -> &Key {
        unsafe { &*(self as *const Vec<u8> as *const Key) }
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Key({})", HexRepr(&self.0))
    }
}
