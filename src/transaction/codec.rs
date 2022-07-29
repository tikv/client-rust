use crate::request::codec::{self, TxnMode};

pub type ApiV1 = codec::ApiV1<TxnMode>;
pub type ApiV2 = codec::ApiV2<TxnMode>;
