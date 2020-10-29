#[macro_use]
mod errors;
mod kv;
pub mod security;
mod timestamp;

#[macro_use]
extern crate log;

#[doc(inline)]
pub use crate::errors::{Error, ErrorKind, Result};
#[doc(inline)]
pub use crate::kv::{codec, BoundRange, Key, KvPair, ToOwnedRange, Value};
#[doc(inline)]
pub use crate::timestamp::{Timestamp, TimestampExt};
