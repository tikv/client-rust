// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This module contains utility types and functions for making the transition
//! from futures 0.1 to 1.0 easier.

use futures::prelude::*;
use futures::task::{Context, Poll};
use futures::try_ready;
use std::pin::Pin;

/// The status of a `loop_fn` loop.
#[derive(Debug)]
pub(crate) enum Loop<T, S> {
    /// Indicates that the loop has completed with output `T`.
    Break(T),

    /// Indicates that the loop function should be called again with input
    /// state `S`.
    Continue(S),
}

/// A future implementing a tail-recursive loop.
///
/// Created by the `loop_fn` function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub(crate) struct LoopFn<A, F> {
    future: A,
    func: F,
}

/// Creates a new future implementing a tail-recursive loop.
pub(crate) fn loop_fn<S, T, A, F, E>(initial_state: S, mut func: F) -> LoopFn<A, F>
where
    F: FnMut(S) -> A,
    A: Future<Output = Result<Loop<T, S>, E>>,
{
    LoopFn {
        future: func(initial_state),
        func,
    }
}

impl<S, T, A, F, E> Future for LoopFn<A, F>
where
    F: FnMut(S) -> A,
    A: Future<Output = Result<Loop<T, S>, E>>,
{
    type Output = Result<T, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<T, E>> {
        loop {
            unsafe {
                let this = Pin::get_unchecked_mut(self);
                match try_ready!(Pin::new_unchecked(&mut this.future).poll(cx)) {
                    Loop::Break(x) => return Poll::Ready(Ok(x)),
                    Loop::Continue(s) => this.future = (this.func)(s),
                }
                self = Pin::new_unchecked(this);
            }
        }
    }
}

/// Emulate `send_all`/`SendAll` from futures 0.1 since the 0.3 versions don't
/// work with Tokio `Handle`s due to ownership differences.
pub(crate) trait SinkCompat<I, E> {
    fn send_all_compat<S>(self, stream: S) -> SendAllCompat<Self, S>
    where
        S: Stream<Item = I> + Unpin,
        Self: Sink<I, SinkError = E> + Sized + Unpin,
    {
        SendAllCompat::new(self, stream)
    }
}

impl<T, E, S: Sink<T, SinkError = E>> SinkCompat<T, E> for S {}

#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub(crate) struct SendAllCompat<Si, St>
where
    Si: Sink<St::Item> + Unpin,
    St: Stream + Unpin,
{
    sink: Option<Si>,
    stream: Option<stream::Fuse<St>>,
    buffered: Option<St::Item>,
}

impl<Si, St> Unpin for SendAllCompat<Si, St>
where
    Si: Sink<St::Item> + Unpin,
    St: Stream + Unpin,
{
}

impl<Si, St> SendAllCompat<Si, St>
where
    Si: Sink<St::Item> + Unpin,
    St: Stream + Unpin,
{
    pub(crate) fn new(sink: Si, stream: St) -> SendAllCompat<Si, St> {
        SendAllCompat {
            sink: Some(sink),
            stream: Some(stream.fuse()),
            buffered: None,
        }
    }

    fn sink_mut(&mut self) -> Pin<&mut Si> {
        Pin::new(
            self.sink
                .as_mut()
                .take()
                .expect("Attempted to poll SendAllCompat after completion"),
        )
    }

    fn stream_mut(&mut self) -> Pin<&mut stream::Fuse<St>> {
        Pin::new(
            self.stream
                .as_mut()
                .take()
                .expect("Attempted to poll SendAllCompat after completion"),
        )
    }

    fn take_result(&mut self) -> (Si, St) {
        let sink = self
            .sink
            .take()
            .expect("Attempted to poll SendAllCompat after completion");
        let fuse = self
            .stream
            .take()
            .expect("Attempted to poll SendAllCompat after completion");
        (sink, fuse.into_inner())
    }

    fn try_start_send(
        &mut self,
        item: St::Item,
        cx: &mut Context,
    ) -> Poll<Result<(()), Si::SinkError>> {
        debug_assert!(self.buffered.is_none());
        match self.sink_mut().poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(self.sink_mut().start_send(item)),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => {
                self.buffered = Some(item);
                Poll::Pending
            }
        }
    }
}

impl<Si, St> Future for SendAllCompat<Si, St>
where
    Si: Sink<St::Item> + Unpin,
    St: Stream + Unpin,
{
    type Output = Result<((Si, St)), Si::SinkError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<((Si, St)), Si::SinkError>> {
        if let Some(item) = self.buffered.take() {
            try_ready!(self.try_start_send(item, cx))
        }

        loop {
            match self.stream_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => try_ready!(self.try_start_send(item, cx)),
                Poll::Ready(None) => {
                    try_ready!(self.sink_mut().poll_close(cx));
                    return Poll::Ready(Ok(self.take_result()));
                }
                Poll::Pending => {
                    try_ready!(self.sink_mut().poll_flush(cx));
                    return Poll::Pending;
                }
            }
        }
    }
}
