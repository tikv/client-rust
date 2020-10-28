#[macro_use]
mod errors;
mod kv;
pub mod security;
pub mod stats;
mod timestamp;

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate prometheus;

#[doc(inline)]
pub use crate::errors::{Error, ErrorKind, Result};
#[doc(inline)]
pub use crate::kv::{codec, BoundRange, Key, KvPair, ToOwnedRange, Value};
#[doc(inline)]
pub use crate::timestamp::{Timestamp, TimestampExt};
