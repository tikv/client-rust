#[macro_use]
mod util;
mod stats;
mod errors;
mod kv;
mod timestamp;

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;

#[doc(inline)]
pub use crate::errors::Error;
#[doc(inline)]
pub use crate::errors::ErrorKind;
#[doc(inline)]
pub use crate::errors::Result;
#[doc(inline)]
pub use crate::kv::{BoundRange, Key, KvPair, ToOwnedRange, Value};