#[macro_use]
mod leader;
mod client;

pub mod errors;
pub use self::client::PdRpcClient;
pub use self::errors::{Error, Result};

use std::ops::Deref;

use futures::Future;
use kvproto::metapb;
use prometheus::*;

pub type Key = Vec<u8>;
pub type PdFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

#[derive(Clone, Debug, PartialEq)]
pub struct RegionInfo {
    pub region: metapb::Region,
    pub leader: Option<metapb::Peer>,
}

impl RegionInfo {
    pub fn new(region: metapb::Region, leader: Option<metapb::Peer>) -> RegionInfo {
        RegionInfo { region, leader }
    }
}

impl Deref for RegionInfo {
    type Target = metapb::Region;

    fn deref(&self) -> &Self::Target {
        &self.region
    }
}

pub const INVALID_ID: u64 = 0;
const REQUEST_TIMEOUT: u64 = 2; // 2s

#[derive(Debug)]
pub struct PdTimestamp {
    pub physical: i64,
    pub logical: i64,
}

lazy_static! {
    pub static ref PD_REQUEST_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "pd_request_duration_seconds",
        "Bucketed histogram of PD requests duration",
        &["type"]
    ).unwrap();
}

pub trait PdClient: Send + Sync {
    // Return the cluster ID.
    fn get_cluster_id(&self) -> Result<u64>;

    // Get store information.
    fn get_store(&self, store_id: u64) -> PdFuture<metapb::Store>;

    // Get all stores information.
    fn get_all_stores(&self) -> PdFuture<Vec<metapb::Store>> {
        unimplemented!();
    }

    // Get cluster meta information.
    fn get_cluster_config(&self) -> PdFuture<metapb::Cluster>;

    // For route.
    // Get region which the key belong to.
    fn get_region(&self, key: &[u8]) -> PdFuture<metapb::Region>;

    // Get region info which the key belong to.
    fn get_region_info(&self, key: &[u8]) -> PdFuture<RegionInfo>;

    // Get region by region id.
    fn get_region_by_id(&self, region_id: u64) -> PdFuture<Option<metapb::Region>>;

    // Register a handler to the client, it will be invoked after reconnecting to PD.
    //
    // Please note that this method should only be called once.
    fn handle_reconnect<F: Fn() + Sync + Send + 'static>(&self, _: F) {}

    // get a timestamp from PD
    fn get_ts(&self) -> PdFuture<PdTimestamp>;
}
