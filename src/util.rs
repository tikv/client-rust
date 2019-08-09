// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{sync::mpsc, thread, time::Duration};
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

lazy_static! {
    pub static ref GLOBAL_TIMER_HANDLE: Handle = start_global_timer();
}

fn start_global_timer() -> Handle {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name(thread_name!("timer"))
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
    use futures::compat::Compat01As03;

    #[test]
    fn test_global_timer() {
        let handle = super::GLOBAL_TIMER_HANDLE.clone();
        let delay =
            handle.delay(::std::time::Instant::now() + ::std::time::Duration::from_millis(100));
        let timer = ::std::time::Instant::now();
        futures::executor::block_on(Compat01As03::new(delay)).unwrap();
        assert!(timer.elapsed() >= ::std::time::Duration::from_millis(100));
    }
}
