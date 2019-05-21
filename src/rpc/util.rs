// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    sync::{mpsc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    thread,
    time::Duration,
};

use lazy_static::*;
use tokio_timer::{self, timer::Handle};

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
macro_rules! thd_name {
    ($name:expr) => {{
        $crate::rpc::util::get_tag_from_thread_name()
            .map(|tag| format!("{}::{}", $name, tag))
            .unwrap_or_else(|| $name.to_owned())
    }};
}

/// A handy shortcut to replace `RwLock` write/read().unwrap() pattern to
/// shortcut wl and rl.
pub trait HandyRwLock<T> {
    fn wl(&self) -> RwLockWriteGuard<T>;
    fn rl(&self) -> RwLockReadGuard<T>;
}

impl<T> HandyRwLock<T> for RwLock<T> {
    fn wl(&self) -> RwLockWriteGuard<T> {
        self.write().unwrap()
    }

    fn rl(&self) -> RwLockReadGuard<T> {
        self.read().unwrap()
    }
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

lazy_static! {
    pub static ref GLOBAL_TIMER_HANDLE: Handle = start_global_timer();
}

fn start_global_timer() -> Handle {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name(thd_name!("timer"))
        .spawn(move || {
            let mut timer = tokio_timer::Timer::default();
            tx.send(timer.handle()).unwrap();
            loop {
                timer.turn(None).unwrap();
            }
        })
        .unwrap();
    rx.recv().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::Future;
    use std::*;

    #[test]
    fn test_rwlock_deadlock() {
        // If the test runs over 60s, then there is a deadlock.
        let mu = RwLock::new(Some(1));
        {
            let _clone = foo(&mu.rl());
            let mut data = mu.wl();
            assert!(data.is_some());
            *data = None;
        }

        {
            match foo(&mu.rl()) {
                Some(_) | None => {
                    let res = mu.try_write();
                    assert!(res.is_err());
                }
            }
        }

        #[cfg_attr(feature = "cargo-clippy", allow(clippy::clone_on_copy))]
        fn foo(a: &Option<usize>) -> Option<usize> {
            a.clone()
        }
    }

    #[test]
    fn test_internal_error() {
        let file_name = file!();
        let line_number = line!();
        let e = internal_err!("{}", "hi");
        assert_eq!(
            format!("{}", e),
            format!("[{}:{}]: hi", file_name, line_number + 1)
        );
    }

    #[test]
    fn test_global_timer() {
        let handle = super::GLOBAL_TIMER_HANDLE.clone();
        let delay =
            handle.delay(::std::time::Instant::now() + ::std::time::Duration::from_millis(100));
        let timer = ::std::time::Instant::now();
        delay.wait().unwrap();
        assert!(timer.elapsed() >= ::std::time::Duration::from_millis(100));
    }
}
