#[macro_use]
mod errors;
pub mod security;
mod timestamp;

#[macro_use]
extern crate log;

#[doc(inline)]
pub use crate::errors::{Error, ErrorKind, Result};
#[doc(inline)]
pub use crate::timestamp::{Timestamp, TimestampExt};
