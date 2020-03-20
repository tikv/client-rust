// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

// https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/

use rand::{thread_rng, Rng};
use std::time::Duration;

pub trait Backoff: Clone + Send + 'static {
    // Returns the delay period for next retry. If the maximum retry count is hit returns None.
    fn next_delay_duration(&mut self) -> Option<Duration>;
}

// Exponential backoff means that the retry delay should multiply a constant
// after each attempt, up to a maximum value. After each attempt, the new retry
// delay should be:
//
// new_delay = min(max_delay, base_delay * 2 ** attempts)
#[derive(Clone)]
pub struct NoJitterBackoff {
    current_attempts: u32,
    max_attempts: u32,
    current_delay_ms: u64,
    max_delay_ms: u64,
}

impl NoJitterBackoff {
    pub const fn new(base_delay_ms: u64, max_delay_ms: u64, max_attempts: u32) -> Self {
        Self {
            current_attempts: 0,
            max_attempts,
            current_delay_ms: base_delay_ms,
            max_delay_ms,
        }
    }
}

impl Backoff for NoJitterBackoff {
    fn next_delay_duration(&mut self) -> Option<Duration> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let delay_ms = self.max_delay_ms.min(self.current_delay_ms);

        self.current_attempts += 1;
        self.current_delay_ms <<= 1;

        Some(Duration::from_millis(delay_ms))
    }
}

// Adds Jitter to the basic exponential backoff. Returns a random value between
// zero and the calculated exponential backoff:
//
// temp = min(max_delay, base_delay * 2 ** attempts)
// new_delay = random_between(0, temp)
#[derive(Clone)]
pub struct FullJitterBackoff {
    current_attempts: u32,
    max_attempts: u32,
    current_delay_ms: u64,
    max_delay_ms: u64,
}

impl FullJitterBackoff {
    #[allow(dead_code)]
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_attempts: u32) -> Self {
        if base_delay_ms == 0 || max_delay_ms == 0 {
            panic!("Both base_delay_ms and max_delay_ms must be positive");
        }

        Self {
            current_attempts: 0,
            max_attempts,
            current_delay_ms: base_delay_ms,
            max_delay_ms,
        }
    }
}

impl Backoff for FullJitterBackoff {
    fn next_delay_duration(&mut self) -> Option<Duration> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let delay_ms = self.max_delay_ms.min(self.current_delay_ms);

        let mut rng = thread_rng();
        let delay_ms: u64 = rng.gen_range(0, delay_ms);

        self.current_attempts += 1;
        self.current_delay_ms <<= 1;

        Some(Duration::from_millis(delay_ms))
    }
}

// Equal Jitter limits the random value should be equal or greater than half of
// the calculated exponential backoff:
//
// temp = min(max_delay, base_delay * 2 ** attempts)
// new_delay = random_between(temp / 2, temp)
#[derive(Clone)]
pub struct EqualJitterBackoff {
    current_attempts: u32,
    max_attempts: u32,
    current_delay_ms: u64,
    max_delay_ms: u64,
}

impl EqualJitterBackoff {
    #[allow(dead_code)]
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_attempts: u32) -> Self {
        if base_delay_ms < 2 || max_delay_ms < 2 {
            panic!("Both base_delay_ms and max_delay_ms must be greater than 1");
        }

        Self {
            current_attempts: 0,
            max_attempts,
            current_delay_ms: base_delay_ms,
            max_delay_ms,
        }
    }
}

impl Backoff for EqualJitterBackoff {
    fn next_delay_duration(&mut self) -> Option<Duration> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let delay_ms = self.max_delay_ms.min(self.current_delay_ms);
        let half_delay_ms = delay_ms >> 1;

        let mut rng = thread_rng();
        let delay_ms: u64 = rng.gen_range(0, half_delay_ms) + half_delay_ms;

        self.current_attempts += 1;
        self.current_delay_ms <<= 1;

        Some(Duration::from_millis(delay_ms))
    }
}

// Decorrelated Jitter is always calculated with the previous backoff
// (the initial value is base_delay):
//
// temp = random_between(base_delay, previous_delay * 3)
// new_delay = min(max_delay, temp)
#[derive(Clone)]
pub struct DecorrelatedJitterBackoff {
    current_attempts: u32,
    max_attempts: u32,
    base_delay_ms: u64,
    current_delay_ms: u64,
    max_delay_ms: u64,
}

impl DecorrelatedJitterBackoff {
    #[allow(dead_code)]
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_attempts: u32) -> Self {
        if base_delay_ms == 0 {
            panic!("base_delay_ms must be positive");
        }

        Self {
            current_attempts: 0,
            max_attempts,
            base_delay_ms,
            current_delay_ms: base_delay_ms,
            max_delay_ms,
        }
    }
}

impl Backoff for DecorrelatedJitterBackoff {
    fn next_delay_duration(&mut self) -> Option<Duration> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let mut rng = thread_rng();
        let delay_ms: u64 =
            rng.gen_range(0, self.current_delay_ms * 3 - self.base_delay_ms) + self.base_delay_ms;

        let delay_ms = delay_ms.min(self.max_delay_ms);

        self.current_attempts += 1;
        self.current_delay_ms = delay_ms;

        Some(Duration::from_millis(delay_ms))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn test_no_jitter_backoff() {
        // Tests for zero attempts.
        let mut backoff = NoJitterBackoff {
            current_attempts: 0,
            max_attempts: 0,
            current_delay_ms: 0,
            max_delay_ms: 0,
        };

        assert_eq!(backoff.next_delay_duration(), None);

        let mut backoff = NoJitterBackoff {
            current_attempts: 0,
            max_attempts: 3,
            current_delay_ms: 2,
            max_delay_ms: 7,
        };

        assert_eq!(
            backoff.next_delay_duration(),
            Some(Duration::from_millis(2))
        );
        assert_eq!(
            backoff.next_delay_duration(),
            Some(Duration::from_millis(4))
        );
        assert_eq!(
            backoff.next_delay_duration(),
            Some(Duration::from_millis(7))
        );
        assert_eq!(backoff.next_delay_duration(), None);
    }

    #[test]
    fn test_full_jitter_backoff() {
        let mut backoff = FullJitterBackoff::new(2, 7, 3);
        assert!(backoff.next_delay_duration().unwrap() <= Duration::from_millis(2));
        assert!(backoff.next_delay_duration().unwrap() <= Duration::from_millis(4));
        assert!(backoff.next_delay_duration().unwrap() <= Duration::from_millis(7));
        assert_eq!(backoff.next_delay_duration(), None);
    }

    #[test]
    #[should_panic(expected = "Both base_delay_ms and max_delay_ms must be positive")]
    fn test_full_jitter_backoff_with_invalid_base_delay_ms() {
        FullJitterBackoff::new(0, 7, 3);
    }

    #[test]
    #[should_panic(expected = "Both base_delay_ms and max_delay_ms must be positive")]
    fn test_full_jitter_backoff_with_invalid_max_delay_ms() {
        FullJitterBackoff::new(2, 0, 3);
    }

    #[test]
    fn test_equal_jitter_backoff() {
        let mut backoff = EqualJitterBackoff::new(2, 7, 3);

        let first_delay_dur = backoff.next_delay_duration().unwrap();
        assert!(first_delay_dur >= Duration::from_millis(1));
        assert!(first_delay_dur <= Duration::from_millis(2));

        let second_delay_dur = backoff.next_delay_duration().unwrap();
        assert!(second_delay_dur >= Duration::from_millis(2));
        assert!(second_delay_dur <= Duration::from_millis(4));

        let third_delay_dur = backoff.next_delay_duration().unwrap();
        assert!(third_delay_dur >= Duration::from_millis(3));
        assert!(third_delay_dur <= Duration::from_millis(6));

        assert_eq!(backoff.next_delay_duration(), None);
    }

    #[test]
    #[should_panic(expected = "Both base_delay_ms and max_delay_ms must be greater than 1")]
    fn test_equal_jitter_backoff_with_invalid_base_delay_ms() {
        EqualJitterBackoff::new(1, 7, 3);
    }

    #[test]
    #[should_panic(expected = "Both base_delay_ms and max_delay_ms must be greater than 1")]
    fn test_equal_jitter_backoff_with_invalid_max_delay_ms() {
        EqualJitterBackoff::new(2, 1, 3);
    }

    #[test]
    fn test_decorrelated_jitter_backoff() {
        let mut backoff = DecorrelatedJitterBackoff::new(2, 7, 3);

        let first_delay_dur = backoff.next_delay_duration().unwrap();
        assert!(first_delay_dur >= Duration::from_millis(2));
        assert!(first_delay_dur <= Duration::from_millis(6));

        let second_delay_dur = backoff.next_delay_duration().unwrap();
        assert!(second_delay_dur >= Duration::from_millis(2));
        let cap_ms = 7u64.min((first_delay_dur.as_millis() * 3).try_into().unwrap());
        assert!(second_delay_dur <= Duration::from_millis(cap_ms));

        let third_delay_dur = backoff.next_delay_duration().unwrap();
        assert!(third_delay_dur >= Duration::from_millis(2));
        let cap_ms = 7u64.min((second_delay_dur.as_millis() * 3).try_into().unwrap());
        assert!(second_delay_dur <= Duration::from_millis(cap_ms));

        assert_eq!(backoff.next_delay_duration(), None);
    }

    #[test]
    #[should_panic(expected = "base_delay_ms must be positive")]
    fn test_decorrelated_jitter_backoff_with_invalid_base_delay_ms() {
        DecorrelatedJitterBackoff::new(0, 7, 3);
    }
}
