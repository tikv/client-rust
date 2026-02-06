use std::hint::black_box;
use std::sync::Arc;
use std::task::{Wake, Waker};
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use futures::task::AtomicWaker;

const MAX_PENDING_COUNT: usize = 1 << 16;
const FULL_EVERY: u64 = 1024;
const FULL_WINDOW: u64 = 16;

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
    fn wake_by_ref(self: &Arc<Self>) {}
}

fn response_policy_old(iterations: u64) -> Duration {
    let atomic_waker = AtomicWaker::new();
    let waker = Waker::from(Arc::new(NoopWake));
    atomic_waker.register(&waker);

    let mut pending_len = 0usize;
    let start = Instant::now();
    for i in 0..iterations {
        if i % FULL_EVERY == 0 {
            pending_len = MAX_PENDING_COUNT;
        }
        black_box(pending_len >= MAX_PENDING_COUNT);
        pending_len = pending_len.saturating_sub(1);
        atomic_waker.wake();
    }
    start.elapsed()
}

fn response_policy_new(iterations: u64) -> Duration {
    let atomic_waker = AtomicWaker::new();
    let waker = Waker::from(Arc::new(NoopWake));
    atomic_waker.register(&waker);

    let mut pending_len = 0usize;
    let start = Instant::now();
    for i in 0..iterations {
        if i % FULL_EVERY == 0 {
            pending_len = MAX_PENDING_COUNT;
        }
        let was_full = pending_len >= MAX_PENDING_COUNT;
        pending_len = pending_len.saturating_sub(1);
        let should_wake = was_full && pending_len < MAX_PENDING_COUNT;
        if black_box(should_wake) {
            atomic_waker.wake();
        }
    }
    start.elapsed()
}

fn register_policy_old(iterations: u64) -> Duration {
    let atomic_waker = AtomicWaker::new();
    let waker = Waker::from(Arc::new(NoopWake));

    let start = Instant::now();
    for i in 0..iterations {
        let pending_len = if i % FULL_EVERY < FULL_WINDOW {
            MAX_PENDING_COUNT
        } else {
            MAX_PENDING_COUNT - 1
        };
        black_box(pending_len);
        atomic_waker.register(&waker);
    }
    start.elapsed()
}

fn register_policy_new(iterations: u64) -> Duration {
    let atomic_waker = AtomicWaker::new();
    let waker = Waker::from(Arc::new(NoopWake));

    let start = Instant::now();
    for i in 0..iterations {
        let pending_len = if i % FULL_EVERY < FULL_WINDOW {
            MAX_PENDING_COUNT
        } else {
            MAX_PENDING_COUNT - 1
        };
        if black_box(pending_len >= MAX_PENDING_COUNT) {
            atomic_waker.register(&waker);
        }
    }
    start.elapsed()
}

fn bench_tso_waker_policy(c: &mut Criterion) {
    let mut group = c.benchmark_group("tso_waker_policy");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(6));

    group.bench_function(BenchmarkId::new("response", "old"), |b| {
        b.iter_custom(response_policy_old);
    });
    group.bench_function(BenchmarkId::new("response", "new"), |b| {
        b.iter_custom(response_policy_new);
    });
    group.bench_function(BenchmarkId::new("register", "old"), |b| {
        b.iter_custom(register_policy_old);
    });
    group.bench_function(BenchmarkId::new("register", "new"), |b| {
        b.iter_custom(register_policy_new);
    });

    group.finish();
}

criterion_group!(benches, bench_tso_waker_policy);
criterion_main!(benches);
