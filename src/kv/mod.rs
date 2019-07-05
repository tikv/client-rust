// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.
use std::{fmt, u8};

mod key;
pub use key::Key;
mod value;
pub use value::Value;
mod kvpair;
pub use kvpair::KvPair;
mod bound_range;
pub use bound_range::BoundRange;

struct HexRepr<'a>(pub &'a [u8]);

impl<'a> fmt::Display for HexRepr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}
