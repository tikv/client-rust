// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.
use std::fmt;

mod bound_range;
pub mod codec;
mod key;
mod kvpair;
mod value;

pub use bound_range::BoundRange;
pub use bound_range::IntoOwnedRange;
pub use key::Key;
pub use key::KvPairTTL;
pub use kvpair::KvPair;
pub use value::Value;

struct HexRepr<'a>(pub &'a [u8]);

impl<'a> fmt::Display for HexRepr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}
