// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! This module contains utility types and functions for making the transition
//! from futures 0.1 to 1.0 easier.

use futures::prelude::*;
use futures::ready;
use futures::task::{Context, Poll};
use std::pin::Pin;

/// A future implementing a tail-recursive loop.
///
/// Created by the `loop_fn` function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub(crate) struct LoopFn<A, F> {
    future: A,
    func: F,
}

pub(crate) fn stream_fn<S, T, A, F, E>(initial_state: S, mut func: F) -> LoopFn<A, F>
where
    F: FnMut(S) -> A,
    A: Future<Output = Result<Option<(S, T)>, E>>,
{
    LoopFn {
        future: func(initial_state),
        func,
    }
}

impl<S, T, A, F, E> Stream for LoopFn<A, F>
where
    F: FnMut(S) -> A,
    A: Future<Output = Result<Option<(S, T)>, E>>,
{
    type Item = Result<T, E>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            match ready!(Pin::new_unchecked(&mut this.future).poll(cx)) {
                Err(e) => Poll::Ready(Some(Err(e))),
                Ok(None) => Poll::Ready(None),
                Ok(Some((s, t))) => {
                    this.future = (this.func)(s);
                    Poll::Ready(Some(Ok(t)))
                }
            }
        }
    }
}

/// A future created by the `ok_and_then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub(crate) struct OkAndThen<A, F> {
    future: A,
    func: F,
}

impl<U, T, A, F, E> Future for OkAndThen<A, F>
where
    F: FnMut(U) -> Result<T, E>,
    A: Future<Output = Result<U, E>>,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<T, E>> {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            let result = ready!(Pin::new_unchecked(&mut this.future).poll(cx))?;
            Poll::Ready((this.func)(result))
        }
    }
}

/// An extension crate to make using our combinator functions more ergonomic.
pub(crate) trait ClientFutureExt {
    /// This function is similar to `map_ok` combinator. Provide a function which
    /// is applied after the `self` future is resolved, only if that future
    /// resolves to `Ok`. Similar to `Result::and_then`, the supplied function
    /// must return a Result (c.f., `map_ok`, which returns the underlying type,
    /// `T`).
    ///
    /// Note that unlike `and_then`, the supplied function returns a resolved
    /// value, not a closure.
    fn ok_and_then<U, T, F, E>(self, func: F) -> OkAndThen<Self, F>
    where
        F: FnMut(U) -> Result<T, E>,
        Self: Future<Output = Result<U, E>> + Sized,
    {
        OkAndThen { future: self, func }
    }
}

impl<T: TryFuture> ClientFutureExt for T {}
