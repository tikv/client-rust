//! A timestamp returned from the timestamp oracle.
//!
//! The version used in transactions can be converted from a timestamp.
//! The lower 18 (PHYSICAL_SHIFT_BITS) bits are the logical part of the timestamp.
//! The higher bits of the version are the physical part of the timestamp.

use std::convert::TryInto;
pub use tikv_client_proto::pdpb::Timestamp;

const PHYSICAL_SHIFT_BITS: i64 = 18;
const LOGICAL_MASK: i64 = (1 << PHYSICAL_SHIFT_BITS) - 1;

/// A helper trait to convert between [`Timestamp`](Timestamp) and an u64.
///
/// A `Timestamp` (64 bits) contains a physical part (first 46 bits) and a logical part (last 18 bits).
pub trait TimestampExt {
    /// Combine physical and logical parts to get a single `Timestamp` (version).
    fn version(&self) -> u64;
    /// Decompose the `Timestamp` (version) into physical and logical parts.
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
