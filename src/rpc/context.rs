// Copyright 2018 The TiKV Project Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

use std::time::Instant;

use prometheus::{HistogramVec, IntCounterVec};

use crate::{rpc::util::duration_to_sec, Result};

pub struct RequestContext<Executor> {
    start: Instant,
    cmd: &'static str,
    duration: &'static HistogramVec,
    failed_duration: &'static HistogramVec,
    failed_counter: &'static IntCounterVec,
    executor: Option<Executor>,
}

impl<Executor> RequestContext<Executor> {
    pub fn new(
        cmd: &'static str,
        duration: &'static HistogramVec,
        counter: &'static IntCounterVec,
        failed_duration: &'static HistogramVec,
        failed_counter: &'static IntCounterVec,
        executor: Executor,
    ) -> Self {
        counter.with_label_values(&[cmd]).inc();
        RequestContext {
            start: Instant::now(),
            cmd,
            duration,
            failed_duration,
            failed_counter,
            executor: Some(executor),
        }
    }

    pub fn executor(&mut self) -> Executor {
        self.executor
            .take()
            .expect("executor can only be take once")
    }

    pub fn done<R>(&self, r: Result<R>) -> Result<R> {
        if r.is_ok() {
            self.duration
                .with_label_values(&[self.cmd])
                .observe(duration_to_sec(self.start.elapsed()));
        } else {
            self.failed_duration
                .with_label_values(&[self.cmd])
                .observe(duration_to_sec(self.start.elapsed()));
            self.failed_counter.with_label_values(&[self.cmd]).inc();
        }
        r
    }
}
