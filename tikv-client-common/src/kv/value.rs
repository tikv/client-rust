// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::u8;

const _PROPTEST_VALUE_MAX: usize = 1024 * 16; // 16 KB

/// The value part of a key/value pair.
///
/// In TiKV, values are an ordered sequence of bytes. This has an advantage over choosing `String`
/// as valid `UTF-8` is not required. This means that the user is permitted to store any data they wish,
/// as long as it can be represented by bytes. (Which is to say, pretty much anything!)
///
/// This type wraps around an owned value, so it should be treated it like `String` or `Vec<u8>`.
///
/// ```rust
/// use tikv_client_common::Value;
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
/// While `.into()` is usually sufficient for obtaining the buffer itself, sometimes type inference
/// isn't able to determine the correct type. Notably in the `assert_eq!()` and `==` cases. In
/// these cases using the fully-qualified-syntax is useful:
///
/// ```rust
/// use tikv_client_common::Value;
///
/// let buf = "TiKV".as_bytes().to_owned();
/// let value = Value::from(buf.clone());
/// assert_eq!(Into::<Vec<u8>>::into(value), buf);
/// ```
///
/// Many functions which accept a `Value` accept an `Into<Value>`, which means all of the above types
/// can be passed directly to those functions.

pub type Value = Vec<u8>;
