// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{Error, KvPair};

use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;

/// An unresolved [`Transaction::scan`](Transaction::scan) request.
///
/// Once resolved this request will result in a scanner over the given keys.
pub struct Scanner;

impl Stream for Scanner {
    type Item = Result<KvPair, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}
