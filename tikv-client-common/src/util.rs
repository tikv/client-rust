// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

#[macro_export]
macro_rules! internal_err {
    ($e:expr) => ({
        let kind = $crate::Error::internal_error(
            format!("[{}:{}]: {}", file!(), line!(),  $e)
        );
        $crate::Error::from(kind)
    });
    ($f:tt, $($arg:expr),+) => ({
        internal_err!(format!($f, $($arg),+))
    });
}

/// Convert Duration to seconds.
#[inline]
pub fn duration_to_sec(d: Duration) -> f64 {
    let nanos = f64::from(d.subsec_nanos());
    // In most cases, we can't have so large Duration, so here just panic if overflow now.
    d.as_secs() as f64 + (nanos / 1_000_000_000.0)
}
