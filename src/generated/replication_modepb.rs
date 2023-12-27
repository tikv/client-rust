/// The replication status sync from PD to TiKV.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReplicationStatus {
    #[prost(enumeration = "ReplicationMode", tag = "1")]
    pub mode: i32,
    #[prost(message, optional, tag = "2")]
    pub dr_auto_sync: ::core::option::Option<DrAutoSync>,
}
/// The status of dr-autosync mode.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DrAutoSync {
    /// The key of the label that used for distinguish different DC.
    #[prost(string, tag = "1")]
    pub label_key: ::prost::alloc::string::String,
    #[prost(enumeration = "DrAutoSyncState", tag = "2")]
    pub state: i32,
    /// Unique ID of the state, it increases after each state transfer.
    #[prost(uint64, tag = "3")]
    pub state_id: u64,
    /// Duration to wait before switching to SYNC by force (in seconds)
    #[prost(int32, tag = "4")]
    pub wait_sync_timeout_hint: i32,
    /// Stores should only sync messages with available stores when state is ASYNC or ASYNC_WAIT.
    #[prost(uint64, repeated, tag = "5")]
    pub available_stores: ::prost::alloc::vec::Vec<u64>,
    /// Stores should forbid region split.
    #[prost(bool, tag = "6")]
    pub pause_region_split: bool,
}
/// The replication status sync from TiKV to PD.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionReplicationStatus {
    #[prost(enumeration = "RegionReplicationState", tag = "1")]
    pub state: i32,
    /// Unique ID of the state, it increases after each state transfer.
    #[prost(uint64, tag = "2")]
    pub state_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreDrAutoSyncStatus {
    #[prost(enumeration = "DrAutoSyncState", tag = "1")]
    pub state: i32,
    #[prost(uint64, tag = "2")]
    pub state_id: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ReplicationMode {
    /// The standard mode. Replicate logs to majority peer.
    Majority = 0,
    /// DR mode. Replicate logs among 2 DCs.
    DrAutoSync = 1,
}
impl ReplicationMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ReplicationMode::Majority => "MAJORITY",
            ReplicationMode::DrAutoSync => "DR_AUTO_SYNC",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "MAJORITY" => Some(Self::Majority),
            "DR_AUTO_SYNC" => Some(Self::DrAutoSync),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DrAutoSyncState {
    /// Raft logs need to sync between different DCs
    Sync = 0,
    /// Wait for switching to ASYNC. Stop sync raft logs between DCs.
    AsyncWait = 1,
    /// Raft logs need to sync to majority peers
    Async = 2,
    /// Switching from ASYNC to SYNC mode
    SyncRecover = 3,
}
impl DrAutoSyncState {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DrAutoSyncState::Sync => "SYNC",
            DrAutoSyncState::AsyncWait => "ASYNC_WAIT",
            DrAutoSyncState::Async => "ASYNC",
            DrAutoSyncState::SyncRecover => "SYNC_RECOVER",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SYNC" => Some(Self::Sync),
            "ASYNC_WAIT" => Some(Self::AsyncWait),
            "ASYNC" => Some(Self::Async),
            "SYNC_RECOVER" => Some(Self::SyncRecover),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RegionReplicationState {
    /// The region's state is unknown
    Unknown = 0,
    /// Logs sync to majority peers
    SimpleMajority = 1,
    /// Logs sync to different DCs
    IntegrityOverLabel = 2,
}
impl RegionReplicationState {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RegionReplicationState::Unknown => "UNKNOWN",
            RegionReplicationState::SimpleMajority => "SIMPLE_MAJORITY",
            RegionReplicationState::IntegrityOverLabel => "INTEGRITY_OVER_LABEL",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "SIMPLE_MAJORITY" => Some(Self::SimpleMajority),
            "INTEGRITY_OVER_LABEL" => Some(Self::IntegrityOverLabel),
            _ => None,
        }
    }
}
