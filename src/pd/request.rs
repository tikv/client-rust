// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

//! A utility module for managing and retrying PD requests.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::compat::Compat01As03;
use futures::prelude::*;
use futures::ready;
use futures::task::{Context, Poll};
use std::pin::Pin;
use tokio_timer::timer::Handle;

use crate::{
    pd::client::{Cluster, RetryClient},
    util::GLOBAL_TIMER_HANDLE,
    Result,
};

const RECONNECT_INTERVAL_SEC: u64 = 1;
const MAX_REQUEST_COUNT: usize = 3;
const LEADER_CHANGE_RETRY: usize = 10;

pub(super) fn retry_request<Resp, Func, RespFuture>(
    client: Arc<RetryClient>,
    func: Func,
) -> RetryRequest<impl Future<Output = Result<Resp>>>
where
    Resp: Send + 'static,
    Func: Fn(&Cluster) -> RespFuture + Send + 'static,
    RespFuture: Future<Output = Result<Resp>> + Send + 'static,
{
    let mut req = Request::new(func, client);
    RetryRequest {
        reconnect_count: LEADER_CHANGE_RETRY,
        future: req
            .reconnect_if_needed()
            .map_err(|_| internal_err!("failed to reconnect"))
            .and_then(move |_| req.send_and_receive()),
    }
}

/// A future which will retry a request up to `reconnect_count` times or until it
/// succeeds.
pub(super) struct RetryRequest<Fut> {
    reconnect_count: usize,
    future: Fut,
}

struct Request<Func> {
    // We keep track of requests sent and after `MAX_REQUEST_COUNT` we reconnect.
    request_sent: usize,

    client: Arc<RetryClient>,
    timer: Handle,

    // A function which makes an async request.
    func: Func,
}

impl<Resp, Fut> Future for RetryRequest<Fut>
where
    Resp: Send + 'static,
    Fut: Future<Output = Result<Resp>> + Send + 'static,
{
    type Output = Result<Resp>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<Resp>> {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            if this.reconnect_count == 0 {
                return Poll::Ready(Err(internal_err!("failed to send request")));
            }

            debug!("reconnect remains: {}", this.reconnect_count);
            this.reconnect_count -= 1;
            let resp = ready!(Pin::new_unchecked(&mut this.future).poll(cx))?;
            Poll::Ready(Ok(resp))
        }
    }
}

impl<Resp, Func, RespFuture> Request<Func>
where
    Resp: Send + 'static,
    Func: Fn(&Cluster) -> RespFuture + Send + 'static,
    RespFuture: Future<Output = Result<Resp>> + Send + 'static,
{
    fn new(func: Func, client: Arc<RetryClient>) -> Self {
        Request {
            request_sent: 0,
            client,
            timer: GLOBAL_TIMER_HANDLE.clone(),
            func,
        }
    }

    fn reconnect_if_needed(&mut self) -> impl Future<Output = std::result::Result<(), ()>> + Send {
        if self.request_sent < MAX_REQUEST_COUNT {
            return future::Either::Left(future::ok(()));
        }

        // FIXME: should not block the core.
        match self.client.reconnect(RECONNECT_INTERVAL_SEC) {
            Ok(_) => {
                self.request_sent = 0;
                future::Either::Left(future::ok(()))
            }
            Err(_) => future::Either::Right(
                Compat01As03::new(
                    self.timer
                        .delay(Instant::now() + Duration::from_secs(RECONNECT_INTERVAL_SEC)),
                )
                .map(|_| Err(())),
            ),
        }
    }

    fn send_and_receive(&mut self) -> impl Future<Output = Result<Resp>> + Send {
        self.request_sent += 1;
        debug!("request sent: {}", self.request_sent);

        self.client.with_cluster(&self.func)
    }
}
