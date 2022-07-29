use std::{future::Future, sync::Arc};

use slog::{Drain, Logger};

use crate::{
    pd::{PdRpcClient, RetryClient},
    Config, Result,
};

pub(crate) struct ClientContext<C> {
    pub logger: Logger,
    pub pd: Arc<PdRpcClient<C>>,
}

impl<C> ClientContext<C> {
    pub async fn new<S, F, Fut>(
        pd_endpoints: Vec<S>,
        config: Config,
        codec_factory: F,
        optional_logger: Option<Logger>,
    ) -> Result<Self>
    where
        S: Into<String>,
        F: Fn(Arc<RetryClient>) -> Fut,
        Fut: Future<Output = Result<C>>,
    {
        let logger = optional_logger.unwrap_or_else(|| {
            let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
            Logger::root(
                slog_term::FullFormat::new(plain)
                    .build()
                    .filter_level(slog::Level::Info)
                    .fuse(),
                o!(),
            )
        });

        let pd_endpoints: Vec<String> = pd_endpoints.into_iter().map(Into::into).collect();
        let pd = Arc::new(
            PdRpcClient::connect(&pd_endpoints, config, codec_factory, logger.clone()).await?,
        );

        Ok(ClientContext { pd, logger })
    }
}
