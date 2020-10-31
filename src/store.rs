use crate::{Region, Result};
use derive_new::new;
use std::any::Any;
use tikv_client_store::{KvClient, KvConnect, Request};

#[derive(new)]
pub struct Store {
    pub region: Region,
    pub client: Box<dyn KvClient + Send + Sync>,
}

impl Store {
    pub async fn dispatch<Req: Request, Resp: Any>(&self, request: &Req) -> Result<Box<Resp>> {
        Ok(self
            .client
            .dispatch(request)
            .await?
            .downcast()
            .expect("Downcast failed"))
    }
}

pub trait KvConnectStore: KvConnect {
    fn connect_to_store(&self, region: Region, address: String) -> Result<Store> {
        info!("connect to tikv endpoint: {:?}", &address);
        let client = self.connect(address.as_str())?;
        Ok(Store::new(region, Box::new(client)))
    }
}
