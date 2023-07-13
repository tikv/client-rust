#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Cluster {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// max peer count for a region.
    /// pd will do the auto-balance if region peer count mismatches.
    ///
    /// more attributes......
    #[prost(uint32, tag = "2")]
    pub max_peer_count: u32,
}
/// Case insensitive key/value for replica constraints.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreLabel {
    #[prost(string, tag = "1")]
    pub key: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Store {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// Address to handle client requests (kv, cop, etc.)
    #[prost(string, tag = "2")]
    pub address: ::prost::alloc::string::String,
    #[prost(enumeration = "StoreState", tag = "3")]
    pub state: i32,
    #[prost(message, repeated, tag = "4")]
    pub labels: ::prost::alloc::vec::Vec<StoreLabel>,
    #[prost(string, tag = "5")]
    pub version: ::prost::alloc::string::String,
    /// Address to handle peer requests (raft messages from other store).
    /// Empty means same as address.
    #[prost(string, tag = "6")]
    pub peer_address: ::prost::alloc::string::String,
    /// Status address provides the HTTP service for external components
    #[prost(string, tag = "7")]
    pub status_address: ::prost::alloc::string::String,
    #[prost(string, tag = "8")]
    pub git_hash: ::prost::alloc::string::String,
    /// The start timestamp of the current store
    #[prost(int64, tag = "9")]
    pub start_timestamp: i64,
    #[prost(string, tag = "10")]
    pub deploy_path: ::prost::alloc::string::String,
    /// The last heartbeat timestamp of the store.
    #[prost(int64, tag = "11")]
    pub last_heartbeat: i64,
    /// If the store is physically destroyed, which means it can never up again.
    #[prost(bool, tag = "12")]
    pub physically_destroyed: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionEpoch {
    /// Conf change version, auto increment when add or remove peer
    #[prost(uint64, tag = "1")]
    pub conf_ver: u64,
    /// Region version, auto increment when split or merge
    #[prost(uint64, tag = "2")]
    pub version: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Region {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// Region key range [start_key, end_key).
    #[prost(bytes = "vec", tag = "2")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "4")]
    pub region_epoch: ::core::option::Option<RegionEpoch>,
    #[prost(message, repeated, tag = "5")]
    pub peers: ::prost::alloc::vec::Vec<Peer>,
    /// Encryption metadata for start_key and end_key. encryption_meta.iv is IV for start_key.
    /// IV for end_key is calculated from (encryption_meta.iv + len(start_key)).
    /// The field is only used by PD and should be ignored otherwise.
    /// If encryption_meta is empty (i.e. nil), it means start_key and end_key are unencrypted.
    #[prost(message, optional, tag = "6")]
    pub encryption_meta: ::core::option::Option<super::encryptionpb::EncryptionMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Peer {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(uint64, tag = "2")]
    pub store_id: u64,
    #[prost(enumeration = "PeerRole", tag = "3")]
    pub role: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StoreState {
    Up = 0,
    Offline = 1,
    Tombstone = 2,
}
impl StoreState {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            StoreState::Up => "Up",
            StoreState::Offline => "Offline",
            StoreState::Tombstone => "Tombstone",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Up" => Some(Self::Up),
            "Offline" => Some(Self::Offline),
            "Tombstone" => Some(Self::Tombstone),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PeerRole {
    /// Voter -> Voter
    Voter = 0,
    /// Learner/None -> Learner
    Learner = 1,
    /// Learner/None -> Voter
    IncomingVoter = 2,
    /// Voter -> Learner
    ///
    /// We forbid Voter -> None, it can introduce unavailability as discussed in
    /// etcd-io/etcd#7625
    /// Learner -> None can be apply directly, doesn't need to be stored as
    /// joint state.
    DemotingVoter = 3,
}
impl PeerRole {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            PeerRole::Voter => "Voter",
            PeerRole::Learner => "Learner",
            PeerRole::IncomingVoter => "IncomingVoter",
            PeerRole::DemotingVoter => "DemotingVoter",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Voter" => Some(Self::Voter),
            "Learner" => Some(Self::Learner),
            "IncomingVoter" => Some(Self::IncomingVoter),
            "DemotingVoter" => Some(Self::DemotingVoter),
            _ => None,
        }
    }
}
