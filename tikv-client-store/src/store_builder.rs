use crate::Region;
use std::time::Duration;

pub struct StoreBuilder {
    pub region: Region,
    pub address: String,
    pub timeout: Duration,
}

impl StoreBuilder {
    pub fn new(region: Region, address: String, timeout: Duration) -> StoreBuilder {
        StoreBuilder {
            region,
            address,
            timeout,
        }
    }
}
