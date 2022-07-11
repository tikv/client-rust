use crate::request::codec::{self, RawMode};

pub type ApiV1 = codec::ApiV1<RawMode>;
pub type ApiV2 = codec::ApiV2<RawMode>;
