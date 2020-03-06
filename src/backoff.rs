// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

// https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/

use rand::{thread_rng, Rng};

pub trait Backoff: Clone + Copy + Send + 'static {
    fn next_delay_ms(&mut self) -> Option<u64>;
}

#[derive(Clone, Copy)]
pub struct NoJitterBackoff {
    current_attempts: u32,
    max_attempts: u32,
    current_delay_ms: u64,
    max_delay_ms: u64,
}

impl NoJitterBackoff {
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_attempts: u32) -> Self {
        Self {
            current_attempts: 0,
            max_attempts,
            current_delay_ms: base_delay_ms,
            max_delay_ms,
        }
    }
}

impl Backoff for NoJitterBackoff {
    fn next_delay_ms(&mut self) -> Option<u64> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let delay_ms = self.max_delay_ms.min(self.current_delay_ms);

        self.current_attempts += 1;
        self.current_delay_ms <<= 1;

        Some(delay_ms)
    }
}

#[derive(Clone, Copy)]
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
    fn next_delay_ms(&mut self) -> Option<u64> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let delay_ms = self.max_delay_ms.min(self.current_delay_ms);

        let mut rng = thread_rng();
        let delay_ms: u64 = rng.gen_range(0, delay_ms);

        self.current_attempts += 1;
        self.current_delay_ms <<= 1;

        Some(delay_ms)
    }
}

#[derive(Clone, Copy)]
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
    fn next_delay_ms(&mut self) -> Option<u64> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let delay_ms = self.max_delay_ms.min(self.current_delay_ms);
        let half_delay_ms = delay_ms >> 1;

        let mut rng = thread_rng();
        let delay_ms: u64 = rng.gen_range(0, half_delay_ms) + half_delay_ms;

        self.current_attempts += 1;
        self.current_delay_ms <<= 1;

        Some(delay_ms)
    }
}

#[derive(Clone, Copy)]
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
    fn next_delay_ms(&mut self) -> Option<u64> {
        if self.current_attempts >= self.max_attempts {
            return None;
        }

        let mut rng = thread_rng();
        let delay_ms: u64 =
            rng.gen_range(0, self.current_delay_ms * 3 - self.base_delay_ms) + self.base_delay_ms;

        let delay_ms = delay_ms.min(self.max_delay_ms);

        self.current_attempts += 1;
        self.current_delay_ms = delay_ms;

        Some(delay_ms)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_no_jitter_backoff() {
        // Tests for zero attempts.
        let mut backoff = NoJitterBackoff::new(0, 0, 0);
        assert_eq!(backoff.next_delay_ms(), None);

        let mut backoff = NoJitterBackoff::new(2, 7, 3);
        assert_eq!(backoff.next_delay_ms(), Some(2));
        assert_eq!(backoff.next_delay_ms(), Some(4));
        assert_eq!(backoff.next_delay_ms(), Some(7));
        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_full_jitter_backoff() {
        let mut backoff = FullJitterBackoff::new(2, 7, 3);
        assert!(backoff.next_delay_ms().unwrap() <= 2);
        assert!(backoff.next_delay_ms().unwrap() <= 4);
        assert!(backoff.next_delay_ms().unwrap() <= 7);
        assert_eq!(backoff.next_delay_ms(), None);
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

        let first_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(first_delay_ms >= 1 && first_delay_ms <= 2);

        let second_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(second_delay_ms >= 2 && second_delay_ms <= 4);

        let third_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(third_delay_ms >= 3 && third_delay_ms <= 6);

        assert_eq!(backoff.next_delay_ms(), None);
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

        let first_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(first_delay_ms >= 2 && first_delay_ms <= 6);

        let second_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(second_delay_ms >= 2 && second_delay_ms <= 7u64.min(first_delay_ms * 3));

        let third_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(third_delay_ms >= 2 && third_delay_ms <= 7u64.min(second_delay_ms * 3));

        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    #[should_panic(expected = "base_delay_ms must be positive")]
    fn test_decorrelated_jitter_backoff_with_invalid_base_delay_ms() {
        DecorrelatedJitterBackoff::new(0, 7, 3);
    }
}
