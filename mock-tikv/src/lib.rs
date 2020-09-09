// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.
mod pd;
mod server;
mod store;

pub use pd::{start_mock_pd_server, MockPd, MOCK_PD_PORT};
pub use server::{start_mock_tikv_server, MockTikv, MOCK_TIKV_PORT};
pub use store::KvStore;

#[macro_export]
macro_rules! spawn_unary_success {
    ($ctx:ident, $req:ident, $resp:ident, $sink:ident) => {
        let f = $sink
            .success($resp)
            .map_err(move |e| panic!("failed to reply {:?}: {:?}", $req, e))
            .map(|_| ());
        $ctx.spawn(f);
    };
}
