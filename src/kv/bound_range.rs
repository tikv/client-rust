// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use super::Key;
use kvproto::kvrpcpb;
#[cfg(test)]
use proptest_derive::Arbitrary;
use std::borrow::Borrow;
use std::cmp::{Eq, PartialEq};
use std::convert::TryFrom;
use std::ops::{Bound, Range, RangeFrom, RangeInclusive};

use crate::{Error, Result};

/// A struct for expressing ranges. This type is semi-opaque and is not really meant for users to
/// deal with directly. Most functions which operate on ranges will accept any types which
/// implement `Into<BoundRange>`.
///
/// In TiKV, keys are an ordered sequence of bytes. This means we can have ranges over those
/// bytes. Eg `001` is before `010`.
///
/// `Into<BoundRange>` has implementations for common range types like `a..b`, `a..=b` where `a` and `b`
/// `impl Into<Key>`. You can implement `Into<BoundRange>` for your own types by using `try_from`.
///
/// Invariant: a range may not be unbounded below.
///
/// ```rust
/// use tikv_client::{BoundRange, Key};
/// use std::ops::{Range, RangeInclusive, RangeTo, RangeToInclusive, RangeFrom, RangeFull, Bound};
/// # use std::convert::TryInto;
///
/// let explict_range: Range<Key> = Range { start: Key::from("Rust".to_owned()), end: Key::from("TiKV".to_owned()) };
/// let from_explict_range: BoundRange = explict_range.into();
///
/// let range: Range<String> = "Rust".to_owned().."TiKV".to_owned();
/// let from_range: BoundRange = range.into();
/// assert_eq!(from_explict_range, from_range);
///
/// let range: RangeInclusive<String> = "Rust".to_owned()..="TiKV".to_owned();
/// let from_range: BoundRange = range.into();
/// assert_eq!(
///     from_range,
///     (Bound::Included(Key::from("Rust".to_owned())), Bound::Included(Key::from("TiKV".to_owned()))),
/// );
///
/// let range_from: RangeFrom<String> = "Rust".to_owned()..;
/// let from_range_from: BoundRange = range_from.into();
/// assert_eq!(
///     from_range_from,
///     (Bound::Included(Key::from("Rust".to_owned())), Bound::Unbounded),
/// );
/// ```
///
/// **But, you should not need to worry about all this:** Most functions which operate
/// on ranges will accept any types  which implement `Into<BoundRange>`.
/// which means all of the above types can be passed directly to those functions.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct BoundRange {
    from: Bound<Key>,
    to: Bound<Key>,
}

impl BoundRange {
    /// Create a new BoundRange.
    ///
    /// The caller must ensure that `from` is not `Unbounded`.
    fn new(from: Bound<Key>, to: Bound<Key>) -> BoundRange {
        // Debug assert because this function is private.
        debug_assert!(from != Bound::Unbounded);
        BoundRange { from, to }
    }

    /// Ranges used in scanning TiKV have a particularity to them.
    ///
    /// The **start** of a scan is inclusive, unless appended with an '\0', then it is exclusive.
    ///
    /// The **end** of a scan is exclusive, unless appended with an '\0', then it is inclusive.
    ///
    /// ```rust
    /// use tikv_client::{BoundRange, Key, ToOwnedRange};
    /// // Exclusive
    /// let range = "a".."z";
    /// assert_eq!(
    ///     BoundRange::from(range.to_owned()).into_keys(),
    ///     (Key::from("a".to_owned()), Some(Key::from("z".to_owned()))),
    /// );
    /// // Inclusive
    /// let range = "a"..="z";
    /// assert_eq!(
    ///     BoundRange::from(range.to_owned()).into_keys(),
    ///     (Key::from("a".to_owned()), Some(Key::from("z\0".to_owned()))),
    /// );
    /// // Open
    /// let range = "a".to_owned()..;
    /// assert_eq!(
    ///     BoundRange::from(range).into_keys(),
    ///     (Key::from("a".to_owned()), None),
    /// );
    // ```
    pub fn into_keys(self) -> (Key, Option<Key>) {
        let start = match self.from {
            Bound::Included(v) => v,
            Bound::Excluded(mut v) => {
                v.push_zero();
                v
            }
            Bound::Unbounded => unreachable!(),
        };
        let end = match self.to {
            Bound::Included(mut v) => {
                v.push_zero();
                Some(v)
            }
            Bound::Excluded(v) => Some(v),
            Bound::Unbounded => None,
        };
        (start, end)
    }
}

// FIXME `==` should not `clone`
impl<T: Into<Key> + Clone> PartialEq<(Bound<T>, Bound<T>)> for BoundRange {
    fn eq(&self, other: &(Bound<T>, Bound<T>)) -> bool {
        self.from == convert_to_bound_key(other.0.clone())
            && self.to == convert_to_bound_key(other.1.clone())
    }
}

impl<T: Into<Key>> From<Range<T>> for BoundRange {
    fn from(other: Range<T>) -> BoundRange {
        BoundRange::new(
            Bound::Included(other.start.into()),
            Bound::Excluded(other.end.into()),
        )
    }
}

impl<T: Into<Key>> From<RangeFrom<T>> for BoundRange {
    fn from(other: RangeFrom<T>) -> BoundRange {
        BoundRange::new(Bound::Included(other.start.into()), Bound::Unbounded)
    }
}

impl<T: Into<Key>> From<RangeInclusive<T>> for BoundRange {
    fn from(other: RangeInclusive<T>) -> BoundRange {
        let (start, end) = other.into_inner();
        BoundRange::new(Bound::Included(start.into()), Bound::Included(end.into()))
    }
}

impl<T: Into<Key>> From<(T, Option<T>)> for BoundRange {
    fn from(other: (T, Option<T>)) -> BoundRange {
        let to = match other.1 {
            None => Bound::Unbounded,
            Some(to) => to.into().into_upper_bound(),
        };

        BoundRange::new(other.0.into().into_lower_bound(), to)
    }
}

impl<T: Into<Key>> From<(T, T)> for BoundRange {
    fn from(other: (T, T)) -> BoundRange {
        BoundRange::new(
            other.0.into().into_lower_bound(),
            other.1.into().into_upper_bound(),
        )
    }
}

impl<T: Into<Key> + Eq> TryFrom<(Bound<T>, Bound<T>)> for BoundRange {
    type Error = Error;

    fn try_from(bounds: (Bound<T>, Bound<T>)) -> Result<BoundRange> {
        if bounds.0 == Bound::Unbounded {
            Err(Error::invalid_key_range())
        } else {
            Ok(BoundRange::new(
                convert_to_bound_key(bounds.0),
                convert_to_bound_key(bounds.1),
            ))
        }
    }
}

impl Into<kvrpcpb::KeyRange> for BoundRange {
    fn into(self) -> kvrpcpb::KeyRange {
        let (start, end) = self.into_keys();
        let mut range = kvrpcpb::KeyRange::default();
        range.set_start_key(start.into());
        // FIXME handle end = None rather than unwrapping
        end.map(|k| range.set_end_key(k.into())).unwrap();
        range
    }
}

/// A convenience trait for converting ranges of borrowed types into a `BoundRange`.
pub trait ToOwnedRange {
    /// Transform a borrowed range of some form into an owned `BoundRange`.
    fn to_owned(self) -> BoundRange;
}

impl<T: Into<Key> + Borrow<U>, U: ToOwned<Owned = T> + ?Sized> ToOwnedRange for Range<&U> {
    fn to_owned(self) -> BoundRange {
        From::from(Range {
            start: self.start.to_owned(),
            end: self.end.to_owned(),
        })
    }
}

impl<T: Into<Key> + Borrow<U>, U: ToOwned<Owned = T> + ?Sized> ToOwnedRange for RangeFrom<&U> {
    fn to_owned(self) -> BoundRange {
        From::from(RangeFrom {
            start: self.start.to_owned(),
        })
    }
}

impl<T: Into<Key> + Borrow<U>, U: ToOwned<Owned = T> + ?Sized> ToOwnedRange for RangeInclusive<&U> {
    fn to_owned(self) -> BoundRange {
        let (from, to) = self.into_inner();
        From::from(RangeInclusive::new(from.to_owned(), to.to_owned()))
    }
}

impl<T: Into<Key> + Borrow<U>, U: ToOwned<Owned = T> + ?Sized> ToOwnedRange for (&U, Option<&U>) {
    fn to_owned(self) -> BoundRange {
        From::from((self.0.to_owned(), self.1.map(|u| u.to_owned())))
    }
}

impl<T: Into<Key> + Borrow<U>, U: ToOwned<Owned = T> + ?Sized> ToOwnedRange for (&U, &U) {
    fn to_owned(self) -> BoundRange {
        From::from((self.0.to_owned(), self.1.to_owned()))
    }
}

fn convert_to_bound_key<K: Into<Key>>(b: Bound<K>) -> Bound<Key> {
    match b {
        Bound::Included(k) => Bound::Included(k.into()),
        Bound::Excluded(k) => Bound::Excluded(k.into()),
        Bound::Unbounded => Bound::Unbounded,
    }
}
