// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::HexRepr;
#[cfg(test)]
use proptest::{arbitrary::any_with, collection::size_range};
#[cfg(test)]
use proptest_derive::Arbitrary;
use std::{fmt, str, u8};

/// The value part of a key/value pair.
///
/// In TiKV, values are an ordered sequence of bytes. This has an advantage over choosing `String`
/// as valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
///
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`.
///
/// ```rust
/// use tikv_client::Value;
///
/// let static_str: &'static str = "TiKV";
/// let from_static_str = Value::from(static_str.to_owned());
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
#[cfg_attr(test, derive(Arbitrary))]
pub struct Value(
    #[cfg_attr(
        test,
        proptest(
            strategy = "any_with::<Vec<u8>>((size_range(crate::proptests::PROPTEST_VALUE_MAX), ()))"
        )
    )]
    pub(super) Vec<u8>,
);

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
