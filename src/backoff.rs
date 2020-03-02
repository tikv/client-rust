// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

// https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/

use rand::{thread_rng, Rng};

pub trait Backoff: Clone + Copy + Send + Sync + 'static {
    fn next_delay_ms(&mut self) -> Option<u64>;

    fn exponential_delay_ms(base_delay_ms: u64, max_delay_ms: u64, attempts: u32) -> u64 {
        max_delay_ms.min(2u64.pow(attempts) * base_delay_ms)
    }
}

#[derive(Clone, Copy)]
pub struct BackoffState {
    current_attempts: u32,
    max_attempts: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
    last_delay_ms: u64,
}

impl BackoffState {
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, attempts: u32) -> Self {
        // Both argument `base_delay_ms` and `max_delay_ms` cannot be less than
        // 2 to prevent panic on `rng.gen_range`.

        let base_delay_ms = match base_delay_ms {
            0 | 1 => 2,
            v => v,
        };

        let max_delay_ms = match max_delay_ms {
            0 | 1 => 2,
            v => v,
        };

        Self {
            current_attempts: 0,
            max_attempts: attempts,
            base_delay_ms,
            max_delay_ms,
            last_delay_ms: base_delay_ms,
        }
    }
}

#[derive(Clone, Copy)]
pub struct NoJitterBackoff {
    pub state: BackoffState,
}

impl Backoff for NoJitterBackoff {
    fn next_delay_ms(&mut self) -> Option<u64> {
        let state = &mut self.state;
        if state.current_attempts >= state.max_attempts {
            return None;
        }

        let delay_ms = Self::exponential_delay_ms(
            state.base_delay_ms,
            state.max_delay_ms,
            state.current_attempts,
        );

        state.current_attempts += 1;
        state.last_delay_ms = delay_ms;

        Some(delay_ms)
    }
}

#[derive(Clone, Copy)]
pub struct FullJitterBackoff {
    pub state: BackoffState,
}

impl Backoff for FullJitterBackoff {
    fn next_delay_ms(&mut self) -> Option<u64> {
        let state = &mut self.state;
        if state.current_attempts >= state.max_attempts {
            return None;
        }

        let delay_ms = Self::exponential_delay_ms(
            state.base_delay_ms,
            state.max_delay_ms,
            state.current_attempts,
        );

        let mut rng = thread_rng();
        let delay_ms: u64 = rng.gen_range(0, delay_ms);

        state.current_attempts += 1;
        state.last_delay_ms = delay_ms;

        Some(delay_ms)
    }
}

#[derive(Clone, Copy)]
pub struct EqualJitterBackoff {
    pub state: BackoffState,
}

impl Backoff for EqualJitterBackoff {
    fn next_delay_ms(&mut self) -> Option<u64> {
        let state = &mut self.state;
        if state.current_attempts >= state.max_attempts {
            return None;
        }

        let delay_ms = Self::exponential_delay_ms(
            state.base_delay_ms,
            state.max_delay_ms,
            state.current_attempts,
        );

        let half_delay_ms = delay_ms / 2;

        let mut rng = thread_rng();
        let delay_ms: u64 = rng.gen_range(0, half_delay_ms) + half_delay_ms;

        state.current_attempts += 1;
        state.last_delay_ms = delay_ms;

        Some(delay_ms)
    }
}

#[derive(Clone, Copy)]
pub struct DecorrelatedJitterBackoff {
    pub state: BackoffState,
}

impl Backoff for DecorrelatedJitterBackoff {
    fn next_delay_ms(&mut self) -> Option<u64> {
        let state = &mut self.state;
        if state.current_attempts >= state.max_attempts {
            return None;
        }

        let mut rng = thread_rng();
        let delay_ms: u64 =
            rng.gen_range(0, state.last_delay_ms * 3 - state.base_delay_ms) + state.base_delay_ms;

        let delay_ms = delay_ms.min(state.max_delay_ms);

        state.current_attempts += 1;
        state.last_delay_ms = delay_ms;

        Some(delay_ms)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_no_jitter_backoff() {
        // Tests for zero attempts.
        let backoff_state = BackoffState::new(0, 0, 0);
        let mut backoff = NoJitterBackoff {
            state: backoff_state,
        };

        assert_eq!(backoff.next_delay_ms(), None);

        let backoff_state = BackoffState::new(2, 7, 3);
        let mut backoff = NoJitterBackoff {
            state: backoff_state,
        };

        assert_eq!(backoff.next_delay_ms(), Some(2));
        assert_eq!(backoff.next_delay_ms(), Some(4));
        assert_eq!(backoff.next_delay_ms(), Some(7));
        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_full_jitter_backoff() {
        let backoff_state = BackoffState::new(2, 7, 3);
        let mut backoff = FullJitterBackoff {
            state: backoff_state,
        };

        assert!(backoff.next_delay_ms().unwrap() <= 2);
        assert!(backoff.next_delay_ms().unwrap() <= 4);
        assert!(backoff.next_delay_ms().unwrap() <= 7);
        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_equal_jitter_backoff() {
        // Tests for no panic when passing zero to both argument `base_delay_ms`
        // and `max_delay_ms`.
        let backoff_state = BackoffState::new(0, 0, 1);
        let mut backoff = EqualJitterBackoff {
            state: backoff_state,
        };

        assert_eq!(backoff.next_delay_ms().is_some(), true);
        assert_eq!(backoff.next_delay_ms(), None);

        let backoff_state = BackoffState::new(2, 7, 3);
        let mut backoff = EqualJitterBackoff {
            state: backoff_state,
        };

        let first_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(first_delay_ms >= 1 && first_delay_ms <= 2);

        let second_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(second_delay_ms >= 2 && second_delay_ms <= 4);

        let third_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(third_delay_ms >= 3 && third_delay_ms <= 6);

        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_decorrelated_jitter_backoff() {
        let backoff_state = BackoffState::new(2, 7, 3);
        let mut backoff = DecorrelatedJitterBackoff {
            state: backoff_state,
        };

        let first_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(first_delay_ms >= 2 && first_delay_ms <= 6);

        let second_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(second_delay_ms >= 2 && second_delay_ms <= 7u64.min(first_delay_ms * 3));

        let third_delay_ms = backoff.next_delay_ms().unwrap();
        assert!(third_delay_ms >= 2 && third_delay_ms <= 7u64.min(second_delay_ms * 3));

        assert_eq!(backoff.next_delay_ms(), None);
    }
}
