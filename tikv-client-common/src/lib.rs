#[macro_use]
mod errors;
pub mod security;

#[macro_use]
extern crate log;

#[doc(inline)]
pub use crate::errors::{Error, ErrorKind, Result};
