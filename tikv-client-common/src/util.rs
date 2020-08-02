// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{thread, time::Duration};

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

/// make a thread name with additional tag inheriting from current thread.
#[macro_export]
macro_rules! thread_name {
    ($name:expr) => {{
        $crate::util::get_tag_from_thread_name()
            .map(|tag| format!("{}::{}", $name, tag))
            .unwrap_or_else(|| $name.to_owned())
    }};
}

pub fn get_tag_from_thread_name() -> Option<String> {
    thread::current()
        .name()
        .and_then(|name| name.split("::").skip(1).last())
        .map(From::from)
}

/// Convert Duration to seconds.
#[inline]
pub fn duration_to_sec(d: Duration) -> f64 {
    let nanos = f64::from(d.subsec_nanos());
    // In most cases, we can't have so large Duration, so here just panic if overflow now.
    d.as_secs() as f64 + (nanos / 1_000_000_000.0)
}
