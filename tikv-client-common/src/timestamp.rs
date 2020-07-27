//! A timestamp returned from the timestamp oracle.
//!
//! The version used in transactions can be converted from a timestamp.
//! The lower 18 (PHYSICAL_SHIFT_BITS) bits are the logical part of the timestamp.
//! The higher bits of the version are the physical part of the timestamp.

pub use kvproto::pdpb::Timestamp;
use std::convert::TryInto;

const PHYSICAL_SHIFT_BITS: i64 = 18;
const LOGICAL_MASK: i64 = (1 << PHYSICAL_SHIFT_BITS) - 1;

pub trait TimestampExt {
    fn version(&self) -> u64;
    fn from_version(version: u64) -> Self;
}

impl TimestampExt for Timestamp {
    fn version(&self) -> u64 {
        ((self.physical << PHYSICAL_SHIFT_BITS) + self.logical)
            .try_into()
            .expect("Overflow converting timestamp to version")
    }

    fn from_version(version: u64) -> Self {
        let version = version as i64;
        Self {
            physical: version >> PHYSICAL_SHIFT_BITS,
            logical: version & LOGICAL_MASK,
        }
    }
}
