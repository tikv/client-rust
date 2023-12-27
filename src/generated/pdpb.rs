#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchGlobalConfigRequest {
    #[prost(string, tag = "1")]
    pub config_path: ::prost::alloc::string::String,
    #[prost(int64, tag = "2")]
    pub revision: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchGlobalConfigResponse {
    #[prost(message, repeated, tag = "1")]
    pub changes: ::prost::alloc::vec::Vec<GlobalConfigItem>,
    #[prost(int64, tag = "2")]
    pub revision: i64,
    #[prost(message, optional, tag = "3")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreGlobalConfigRequest {
    #[prost(message, repeated, tag = "1")]
    pub changes: ::prost::alloc::vec::Vec<GlobalConfigItem>,
    #[prost(string, tag = "2")]
    pub config_path: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreGlobalConfigResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoadGlobalConfigRequest {
    #[prost(string, repeated, tag = "1")]
    pub names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag = "2")]
    pub config_path: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoadGlobalConfigResponse {
    #[prost(message, repeated, tag = "1")]
    pub items: ::prost::alloc::vec::Vec<GlobalConfigItem>,
    #[prost(int64, tag = "2")]
    pub revision: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GlobalConfigItem {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// this field 'value' is replaced by the field 'payload'.
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub error: ::core::option::Option<Error>,
    #[prost(enumeration = "EventType", tag = "4")]
    pub kind: i32,
    /// Since item value needs to support marshal of different struct types,
    /// it should be set to bytes instead of string.
    #[prost(bytes = "vec", tag = "5")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestHeader {
    /// cluster_id is the ID of the cluster which be sent to.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    /// sender_id is the ID of the sender server, also member ID or etcd ID.
    #[prost(uint64, tag = "2")]
    pub sender_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseHeader {
    /// cluster_id is the ID of the cluster which sent the response.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(enumeration = "ErrorType", tag = "1")]
    pub r#type: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TsoRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub count: u32,
    #[prost(string, tag = "3")]
    pub dc_location: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timestamp {
    #[prost(int64, tag = "1")]
    pub physical: i64,
    #[prost(int64, tag = "2")]
    pub logical: i64,
    /// Number of suffix bits used for global distinction,
    /// PD client will use this to compute a TSO's logical part.
    #[prost(uint32, tag = "3")]
    pub suffix_bits: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TsoResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint32, tag = "2")]
    pub count: u32,
    #[prost(message, optional, tag = "3")]
    pub timestamp: ::core::option::Option<Timestamp>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BootstrapRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub store: ::core::option::Option<super::metapb::Store>,
    #[prost(message, optional, tag = "3")]
    pub region: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BootstrapResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub replication_status: ::core::option::Option<
        super::replication_modepb::ReplicationStatus,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsBootstrappedRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsBootstrappedResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(bool, tag = "2")]
    pub bootstrapped: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AllocIdRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AllocIdResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsSnapshotRecoveringRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsSnapshotRecoveringResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(bool, tag = "2")]
    pub marked: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStoreRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub store_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStoreResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub store: ::core::option::Option<super::metapb::Store>,
    #[prost(message, optional, tag = "3")]
    pub stats: ::core::option::Option<StoreStats>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutStoreRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub store: ::core::option::Option<super::metapb::Store>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutStoreResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub replication_status: ::core::option::Option<
        super::replication_modepb::ReplicationStatus,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllStoresRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    /// Do NOT return tombstone stores if set to true.
    #[prost(bool, tag = "2")]
    pub exclude_tombstone_stores: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllStoresResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub stores: ::prost::alloc::vec::Vec<super::metapb::Store>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRegionRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub region_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag = "3")]
    pub need_buckets: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRegionResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "3")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
    /// Leader considers that these peers are down.
    #[prost(message, repeated, tag = "5")]
    pub down_peers: ::prost::alloc::vec::Vec<PeerStats>,
    /// Pending peers are the peers that the leader can't consider as
    /// working followers.
    #[prost(message, repeated, tag = "6")]
    pub pending_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// buckets isn't nil if GetRegion.\* requests set need_buckets.
    #[prost(message, optional, tag = "7")]
    pub buckets: ::core::option::Option<super::metapb::Buckets>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRegionByIdRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    #[prost(bool, tag = "3")]
    pub need_buckets: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScanRegionsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    /// no limit when limit \<= 0.
    #[prost(int32, tag = "3")]
    pub limit: i32,
    /// end_key is +inf when it is empty.
    #[prost(bytes = "vec", tag = "4")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Region {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "2")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
    /// Leader considers that these peers are down.
    #[prost(message, repeated, tag = "3")]
    pub down_peers: ::prost::alloc::vec::Vec<PeerStats>,
    /// Pending peers are the peers that the leader can't consider as
    /// working followers.
    #[prost(message, repeated, tag = "4")]
    pub pending_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScanRegionsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// Keep for backword compatibability.
    #[prost(message, repeated, tag = "2")]
    pub region_metas: ::prost::alloc::vec::Vec<super::metapb::Region>,
    #[prost(message, repeated, tag = "3")]
    pub leaders: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// Extended region info with down/pending peers.
    #[prost(message, repeated, tag = "4")]
    pub regions: ::prost::alloc::vec::Vec<Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClusterConfigRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClusterConfigResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub cluster: ::core::option::Option<super::metapb::Cluster>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutClusterConfigRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub cluster: ::core::option::Option<super::metapb::Cluster>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutClusterConfigResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Member {
    /// name is the name of the PD member.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// member_id is the unique id of the PD member.
    #[prost(uint64, tag = "2")]
    pub member_id: u64,
    #[prost(string, repeated, tag = "3")]
    pub peer_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "4")]
    pub client_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(int32, tag = "5")]
    pub leader_priority: i32,
    #[prost(string, tag = "6")]
    pub deploy_path: ::prost::alloc::string::String,
    #[prost(string, tag = "7")]
    pub binary_version: ::prost::alloc::string::String,
    #[prost(string, tag = "8")]
    pub git_hash: ::prost::alloc::string::String,
    #[prost(string, tag = "9")]
    pub dc_location: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMembersRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMembersResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub members: ::prost::alloc::vec::Vec<Member>,
    #[prost(message, optional, tag = "3")]
    pub leader: ::core::option::Option<Member>,
    #[prost(message, optional, tag = "4")]
    pub etcd_leader: ::core::option::Option<Member>,
    #[prost(map = "string, message", tag = "5")]
    pub tso_allocator_leaders: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        Member,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClusterInfoRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClusterInfoResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(enumeration = "ServiceMode", repeated, tag = "2")]
    pub service_modes: ::prost::alloc::vec::Vec<i32>,
    /// If service mode is API_SVC_MODE, this field will be set to the
    /// registered tso service addresses.
    #[prost(string, repeated, tag = "3")]
    pub tso_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerStats {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(uint64, tag = "2")]
    pub down_seconds: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionHeartbeatRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    /// Leader Peer sending the heartbeat.
    #[prost(message, optional, tag = "3")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
    /// Leader considers that these peers are down.
    #[prost(message, repeated, tag = "4")]
    pub down_peers: ::prost::alloc::vec::Vec<PeerStats>,
    /// Pending peers are the peers that the leader can't consider as
    /// working followers.
    #[prost(message, repeated, tag = "5")]
    pub pending_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// Bytes read/written during this period.
    #[prost(uint64, tag = "6")]
    pub bytes_written: u64,
    #[prost(uint64, tag = "7")]
    pub bytes_read: u64,
    /// Keys read/written during this period.
    #[prost(uint64, tag = "8")]
    pub keys_written: u64,
    #[prost(uint64, tag = "9")]
    pub keys_read: u64,
    /// Approximate region size.
    #[prost(uint64, tag = "10")]
    pub approximate_size: u64,
    /// Actually reported time interval
    #[prost(message, optional, tag = "12")]
    pub interval: ::core::option::Option<TimeInterval>,
    /// Approximate number of keys.
    #[prost(uint64, tag = "13")]
    pub approximate_keys: u64,
    /// Term is the term of raft group.
    #[prost(uint64, tag = "14")]
    pub term: u64,
    #[prost(message, optional, tag = "15")]
    pub replication_status: ::core::option::Option<
        super::replication_modepb::RegionReplicationStatus,
    >,
    /// QueryStats reported write query stats, and there are read query stats in store heartbeat
    #[prost(message, optional, tag = "16")]
    pub query_stats: ::core::option::Option<QueryStats>,
    /// cpu_usage is the CPU time usage of the leader region since the last heartbeat,
    /// which is calculated by cpu_time_delta/heartbeat_reported_interval.
    #[prost(uint64, tag = "17")]
    pub cpu_usage: u64,
    /// (Serverless) Approximate size of key-value pairs for billing.
    /// It's counted on size of user key & value (excluding metadata fields), before compression, and latest versions only.
    #[prost(uint64, tag = "18")]
    pub approximate_kv_size: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangePeer {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(enumeration = "super::eraftpb::ConfChangeType", tag = "2")]
    pub change_type: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangePeerV2 {
    /// If changes is empty, it means that to exit joint state.
    #[prost(message, repeated, tag = "1")]
    pub changes: ::prost::alloc::vec::Vec<ChangePeer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransferLeader {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, repeated, tag = "2")]
    pub peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Merge {
    #[prost(message, optional, tag = "1")]
    pub target: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitRegion {
    #[prost(enumeration = "CheckPolicy", tag = "1")]
    pub policy: i32,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub keys: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SwitchWitness {
    #[prost(uint64, tag = "1")]
    pub peer_id: u64,
    #[prost(bool, tag = "2")]
    pub is_witness: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchSwitchWitness {
    #[prost(message, repeated, tag = "1")]
    pub switch_witnesses: ::prost::alloc::vec::Vec<SwitchWitness>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionHeartbeatResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// Notice, Pd only allows handling reported epoch >= current pd's.
    /// Leader peer reports region status with RegionHeartbeatRequest
    /// to pd regularly, pd will determine whether this region
    /// should do ChangePeer or not.
    /// E,g, max peer number is 3, region A, first only peer 1 in A.
    ///
    /// 1. Pd region state -> Peers (1), ConfVer (1).
    /// 1. Leader peer 1 reports region state to pd, pd finds the
    ///    peer number is \< 3, so first changes its current region
    ///    state -> Peers (1, 2), ConfVer (1), and returns ChangePeer Adding 2.
    /// 1. Leader does ChangePeer, then reports Peers (1, 2), ConfVer (2),
    ///    pd updates its state -> Peers (1, 2), ConfVer (2).
    /// 1. Leader may report old Peers (1), ConfVer (1) to pd before ConfChange
    ///    finished, pd stills responses ChangePeer Adding 2, of course, we must
    ///    guarantee the second ChangePeer can't be applied in TiKV.
    #[prost(message, optional, tag = "2")]
    pub change_peer: ::core::option::Option<ChangePeer>,
    /// Pd can return transfer_leader to let TiKV does leader transfer itself.
    #[prost(message, optional, tag = "3")]
    pub transfer_leader: ::core::option::Option<TransferLeader>,
    /// ID of the region
    #[prost(uint64, tag = "4")]
    pub region_id: u64,
    #[prost(message, optional, tag = "5")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    /// Leader of the region at the moment of the corresponding request was made.
    #[prost(message, optional, tag = "6")]
    pub target_peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, optional, tag = "7")]
    pub merge: ::core::option::Option<Merge>,
    /// PD sends split_region to let TiKV split a region into two regions.
    #[prost(message, optional, tag = "8")]
    pub split_region: ::core::option::Option<SplitRegion>,
    /// Multiple change peer operations atomically.
    /// Note: PD can use both ChangePeer and ChangePeerV2 at the same time
    /// (not in the same RegionHeartbeatResponse).
    /// Now, PD use ChangePeerV2 in following scenarios:
    /// 1. replacing peers
    /// 2. demoting voter directly
    #[prost(message, optional, tag = "9")]
    pub change_peer_v2: ::core::option::Option<ChangePeerV2>,
    #[prost(message, optional, tag = "10")]
    pub switch_witnesses: ::core::option::Option<BatchSwitchWitness>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AskSplitRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AskSplitResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// We split the region into two, first uses the origin
    /// parent region id, and the second uses the new_region_id.
    /// We must guarantee that the new_region_id is global unique.
    #[prost(uint64, tag = "2")]
    pub new_region_id: u64,
    /// The peer ids for the new split region.
    #[prost(uint64, repeated, tag = "3")]
    pub new_peer_ids: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportSplitRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub left: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "3")]
    pub right: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportSplitResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AskBatchSplitRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(uint32, tag = "3")]
    pub split_count: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitId {
    #[prost(uint64, tag = "1")]
    pub new_region_id: u64,
    #[prost(uint64, repeated, tag = "2")]
    pub new_peer_ids: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AskBatchSplitResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub ids: ::prost::alloc::vec::Vec<SplitId>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportBatchSplitRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, repeated, tag = "2")]
    pub regions: ::prost::alloc::vec::Vec<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportBatchSplitResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimeInterval {
    /// The unix timestamp in seconds of the start of this period.
    #[prost(uint64, tag = "1")]
    pub start_timestamp: u64,
    /// The unix timestamp in seconds of the end of this period.
    #[prost(uint64, tag = "2")]
    pub end_timestamp: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecordPair {
    #[prost(string, tag = "1")]
    pub key: ::prost::alloc::string::String,
    #[prost(uint64, tag = "2")]
    pub value: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerStat {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub read_keys: u64,
    #[prost(uint64, tag = "3")]
    pub read_bytes: u64,
    #[prost(message, optional, tag = "4")]
    pub query_stats: ::core::option::Option<QueryStats>,
    #[prost(uint64, tag = "5")]
    pub written_keys: u64,
    #[prost(uint64, tag = "6")]
    pub written_bytes: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreStats {
    #[prost(uint64, tag = "1")]
    pub store_id: u64,
    /// Capacity for the store.
    #[prost(uint64, tag = "2")]
    pub capacity: u64,
    /// Available size for the store.
    #[prost(uint64, tag = "3")]
    pub available: u64,
    /// Total region count in this store.
    #[prost(uint32, tag = "4")]
    pub region_count: u32,
    /// Current sending snapshot count.
    #[prost(uint32, tag = "5")]
    pub sending_snap_count: u32,
    /// Current receiving snapshot count.
    #[prost(uint32, tag = "6")]
    pub receiving_snap_count: u32,
    /// When the store is started (unix timestamp in seconds).
    #[prost(uint32, tag = "7")]
    pub start_time: u32,
    /// How many region is applying snapshot.
    #[prost(uint32, tag = "8")]
    pub applying_snap_count: u32,
    /// If the store is busy
    #[prost(bool, tag = "9")]
    pub is_busy: bool,
    /// Actually used space by db
    #[prost(uint64, tag = "10")]
    pub used_size: u64,
    /// Bytes written for the store during this period.
    #[prost(uint64, tag = "11")]
    pub bytes_written: u64,
    /// Keys written for the store during this period.
    #[prost(uint64, tag = "12")]
    pub keys_written: u64,
    /// Bytes read for the store during this period.
    #[prost(uint64, tag = "13")]
    pub bytes_read: u64,
    /// Keys read for the store during this period.
    #[prost(uint64, tag = "14")]
    pub keys_read: u64,
    /// Actually reported time interval
    #[prost(message, optional, tag = "15")]
    pub interval: ::core::option::Option<TimeInterval>,
    /// Threads' CPU usages in the store
    #[prost(message, repeated, tag = "16")]
    pub cpu_usages: ::prost::alloc::vec::Vec<RecordPair>,
    /// Threads' read disk I/O rates in the store
    #[prost(message, repeated, tag = "17")]
    pub read_io_rates: ::prost::alloc::vec::Vec<RecordPair>,
    /// Threads' write disk I/O rates in the store
    #[prost(message, repeated, tag = "18")]
    pub write_io_rates: ::prost::alloc::vec::Vec<RecordPair>,
    /// Operations' latencies in the store
    #[prost(message, repeated, tag = "19")]
    pub op_latencies: ::prost::alloc::vec::Vec<RecordPair>,
    /// Hot peer stat in the store
    #[prost(message, repeated, tag = "20")]
    pub peer_stats: ::prost::alloc::vec::Vec<PeerStat>,
    /// Store query stats
    #[prost(message, optional, tag = "21")]
    pub query_stats: ::core::option::Option<QueryStats>,
    /// Score that represents the speed of the store, ranges in \[1, 100\], lower is better.
    #[prost(uint64, tag = "22")]
    pub slow_score: u64,
    /// Damaged regions on the store that need to be removed by PD.
    #[prost(uint64, repeated, tag = "23")]
    pub damaged_regions_id: ::prost::alloc::vec::Vec<u64>,
    /// If the apply worker is busy, namely high apply wait duration
    #[prost(bool, tag = "24")]
    pub is_apply_busy: bool,
    /// Snapshot stats in the store
    #[prost(message, repeated, tag = "25")]
    pub snapshot_stats: ::prost::alloc::vec::Vec<SnapshotStat>,
    #[prost(message, optional, tag = "26")]
    pub slow_trend: ::core::option::Option<SlowTrend>,
    /// If the grpc server is paused.
    #[prost(bool, tag = "27")]
    pub is_grpc_paused: bool,
    /// Total memory of the store in bytes.
    #[prost(uint64, tag = "28")]
    pub total_memory: u64,
    /// Used memory of the store in bytes.
    #[prost(uint64, tag = "29")]
    pub used_memory: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SlowTrend {
    #[prost(double, tag = "1")]
    pub cause_value: f64,
    #[prost(double, tag = "2")]
    pub cause_rate: f64,
    #[prost(double, tag = "3")]
    pub result_value: f64,
    #[prost(double, tag = "4")]
    pub result_rate: f64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotStat {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    /// Generate snapshot duration
    #[prost(uint64, tag = "2")]
    pub generate_duration_sec: u64,
    /// Send snapshot duration
    #[prost(uint64, tag = "3")]
    pub send_duration_sec: u64,
    /// \|-- waiting --|-- generate --｜-- send --|
    /// \|-----------total duration---------------|
    /// Total duration include waiting and executing duration
    #[prost(uint64, tag = "4")]
    pub total_duration_sec: u64,
    /// Size is the transport data size
    #[prost(uint64, tag = "5")]
    pub transport_size: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerReport {
    #[prost(message, optional, tag = "1")]
    pub raft_state: ::core::option::Option<super::raft_serverpb::RaftLocalState>,
    #[prost(message, optional, tag = "2")]
    pub region_state: ::core::option::Option<super::raft_serverpb::RegionLocalState>,
    #[prost(bool, tag = "3")]
    pub is_force_leader: bool,
    /// The peer has proposed but uncommitted commit merge.
    #[prost(bool, tag = "4")]
    pub has_commit_merge: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreReport {
    #[prost(message, repeated, tag = "1")]
    pub peer_reports: ::prost::alloc::vec::Vec<PeerReport>,
    #[prost(uint64, tag = "2")]
    pub step: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreHeartbeatRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub stats: ::core::option::Option<StoreStats>,
    /// Detailed store report that is only filled up on PD's demand for online unsafe recovery.
    #[prost(message, optional, tag = "3")]
    pub store_report: ::core::option::Option<StoreReport>,
    #[prost(message, optional, tag = "4")]
    pub dr_autosync_status: ::core::option::Option<
        super::replication_modepb::StoreDrAutoSyncStatus,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DemoteFailedVoters {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(message, repeated, tag = "2")]
    pub failed_voters: ::prost::alloc::vec::Vec<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ForceLeader {
    /// The store ids of the failed stores, TiKV uses it to decide if a peer is alive.
    #[prost(uint64, repeated, tag = "1")]
    pub failed_stores: ::prost::alloc::vec::Vec<u64>,
    /// The region ids of the peer which is to be force leader.
    #[prost(uint64, repeated, tag = "2")]
    pub enter_force_leaders: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoveryPlan {
    /// Create empty regions to fill the key range hole.
    #[prost(message, repeated, tag = "1")]
    pub creates: ::prost::alloc::vec::Vec<super::metapb::Region>,
    /// Update the meta of the regions, including peer lists, epoch and key range.
    #[deprecated]
    #[prost(message, repeated, tag = "2")]
    pub updates: ::prost::alloc::vec::Vec<super::metapb::Region>,
    /// Tombstone the peers on the store locally.
    #[prost(uint64, repeated, tag = "3")]
    pub tombstones: ::prost::alloc::vec::Vec<u64>,
    /// Issue conf change that demote voters on failed stores to learners on the regions.
    #[prost(message, repeated, tag = "4")]
    pub demotes: ::prost::alloc::vec::Vec<DemoteFailedVoters>,
    /// Make the peers to be force leaders.
    #[prost(message, optional, tag = "5")]
    pub force_leader: ::core::option::Option<ForceLeader>,
    /// Step is an increasing number to note the round of recovery,
    /// It should be filled in the corresponding store report.
    #[prost(uint64, tag = "6")]
    pub step: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AwakenRegions {
    /// Awake all regions if abnormal_stores is empty.
    #[prost(uint64, repeated, tag = "1")]
    pub abnormal_stores: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ControlGrpc {
    #[prost(enumeration = "ControlGrpcEvent", tag = "1")]
    pub ctrl_event: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreHeartbeatResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub replication_status: ::core::option::Option<
        super::replication_modepb::ReplicationStatus,
    >,
    #[prost(string, tag = "3")]
    pub cluster_version: ::prost::alloc::string::String,
    /// Used by online unsafe recovery to request store report.
    /// Now it's substituted by reusing recovery_plan field. PD will send a empty
    /// recovery plan instead to request store report.
    #[deprecated]
    #[prost(bool, tag = "4")]
    pub require_detailed_report: bool,
    /// Operations of recovery. After the plan is executed, TiKV should attach the
    /// store report in store heartbeat.
    #[prost(message, optional, tag = "5")]
    pub recovery_plan: ::core::option::Option<RecoveryPlan>,
    /// Pd can return awaken_regions to let TiKV awaken hibernated regions itself.
    #[prost(message, optional, tag = "6")]
    pub awaken_regions: ::core::option::Option<AwakenRegions>,
    /// Pd can return operations to let TiKV forcely PAUSE | RESUME grpc server.
    #[prost(message, optional, tag = "7")]
    pub control_grpc: ::core::option::Option<ControlGrpc>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScatterRegionRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[deprecated]
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    /// PD will use these region information if it can't find the region.
    /// For example, the region is just split and hasn't report to PD yet.
    #[prost(message, optional, tag = "3")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "4")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
    /// If group is defined, the regions with the same group would be scattered as a whole group.
    /// If not defined, the regions would be scattered in a cluster level.
    #[prost(string, tag = "5")]
    pub group: ::prost::alloc::string::String,
    /// If regions_id is defined, the region_id would be ignored.
    #[prost(uint64, repeated, tag = "6")]
    pub regions_id: ::prost::alloc::vec::Vec<u64>,
    #[prost(uint64, tag = "7")]
    pub retry_limit: u64,
    #[prost(bool, tag = "8")]
    pub skip_store_limit: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScatterRegionResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub finished_percentage: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetGcSafePointRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetGcSafePointResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateGcSafePointRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateGcSafePointResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub new_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateServiceGcSafePointRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub service_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "3")]
    pub ttl: i64,
    #[prost(uint64, tag = "4")]
    pub safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateServiceGcSafePointResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub service_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "3")]
    pub ttl: i64,
    #[prost(uint64, tag = "4")]
    pub min_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetGcSafePointV2Request {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub keyspace_id: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetGcSafePointV2Response {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchGcSafePointV2Request {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(int64, tag = "2")]
    pub revision: i64,
}
/// SafePointEvent is for the rpc WatchGCSafePointV2.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SafePointEvent {
    #[prost(uint32, tag = "1")]
    pub keyspace_id: u32,
    #[prost(uint64, tag = "2")]
    pub safe_point: u64,
    #[prost(enumeration = "EventType", tag = "3")]
    pub r#type: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchGcSafePointV2Response {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub events: ::prost::alloc::vec::Vec<SafePointEvent>,
    #[prost(int64, tag = "3")]
    pub revision: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateGcSafePointV2Request {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub keyspace_id: u32,
    #[prost(uint64, tag = "3")]
    pub safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateGcSafePointV2Response {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub new_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateServiceSafePointV2Request {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub keyspace_id: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub service_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub safe_point: u64,
    /// Safe point will be set to expire on (PD Server time + TTL),
    /// pass in a ttl \< 0 to remove target safe point;
    /// pass in MAX_INT64 to set a safe point that never expire.
    /// This should be set by component that may crash unexpectedly so that it doesn't block
    /// cluster garbage collection.
    #[prost(int64, tag = "5")]
    pub ttl: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateServiceSafePointV2Response {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub service_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "3")]
    pub ttl: i64,
    #[prost(uint64, tag = "4")]
    pub min_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllGcSafePointV2Request {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GcSafePointV2 {
    #[prost(uint32, tag = "1")]
    pub keyspace_id: u32,
    #[prost(uint64, tag = "2")]
    pub gc_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllGcSafePointV2Response {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub gc_safe_points: ::prost::alloc::vec::Vec<GcSafePointV2>,
    #[prost(int64, tag = "3")]
    pub revision: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionStat {
    /// Bytes read/written during this period.
    #[prost(uint64, tag = "1")]
    pub bytes_written: u64,
    #[prost(uint64, tag = "2")]
    pub bytes_read: u64,
    /// Keys read/written during this period.
    #[prost(uint64, tag = "3")]
    pub keys_written: u64,
    #[prost(uint64, tag = "4")]
    pub keys_read: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncRegionRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub member: ::core::option::Option<Member>,
    /// the follower PD will use the start index to locate historical changes
    /// that require synchronization.
    #[prost(uint64, tag = "3")]
    pub start_index: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeersStats {
    #[prost(message, repeated, tag = "1")]
    pub peers: ::prost::alloc::vec::Vec<PeerStats>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Peers {
    #[prost(message, repeated, tag = "1")]
    pub peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncRegionResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// the leader PD will send the repsonds include
    /// changed regions records and the index of the first record.
    #[prost(message, repeated, tag = "2")]
    pub regions: ::prost::alloc::vec::Vec<super::metapb::Region>,
    #[prost(uint64, tag = "3")]
    pub start_index: u64,
    #[prost(message, repeated, tag = "4")]
    pub region_stats: ::prost::alloc::vec::Vec<RegionStat>,
    #[prost(message, repeated, tag = "5")]
    pub region_leaders: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// the buckets informations without stats.
    #[prost(message, repeated, tag = "6")]
    pub buckets: ::prost::alloc::vec::Vec<super::metapb::Buckets>,
    #[prost(message, repeated, tag = "16")]
    pub down_peers: ::prost::alloc::vec::Vec<PeersStats>,
    #[prost(message, repeated, tag = "17")]
    pub pending_peers: ::prost::alloc::vec::Vec<Peers>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperatorRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperatorResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub desc: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration = "OperatorStatus", tag = "4")]
    pub status: i32,
    #[prost(bytes = "vec", tag = "5")]
    pub kind: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncMaxTsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub max_ts: ::core::option::Option<Timestamp>,
    /// If skip_check is true, the sync will try to write the max_ts without checking whether it's bigger.
    #[prost(bool, tag = "3")]
    pub skip_check: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncMaxTsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub max_local_ts: ::core::option::Option<Timestamp>,
    #[prost(string, repeated, tag = "3")]
    pub synced_dcs: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitRegionsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub split_keys: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint64, tag = "3")]
    pub retry_limit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitRegionsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub finished_percentage: u64,
    #[prost(uint64, repeated, tag = "3")]
    pub regions_id: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitAndScatterRegionsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub split_keys: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(string, tag = "3")]
    pub group: ::prost::alloc::string::String,
    #[prost(uint64, tag = "4")]
    pub retry_limit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitAndScatterRegionsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub split_finished_percentage: u64,
    #[prost(uint64, tag = "3")]
    pub scatter_finished_percentage: u64,
    #[prost(uint64, repeated, tag = "4")]
    pub regions_id: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDcLocationInfoRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(string, tag = "2")]
    pub dc_location: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDcLocationInfoResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// suffix sign
    #[prost(int32, tag = "2")]
    pub suffix: i32,
    /// max_ts will be included into this response if PD leader think the receiver needs,
    /// which it's set when the number of the max suffix bits changes.
    #[prost(message, optional, tag = "3")]
    pub max_ts: ::core::option::Option<Timestamp>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryStats {
    #[prost(uint64, tag = "1")]
    pub gc: u64,
    #[prost(uint64, tag = "2")]
    pub get: u64,
    #[prost(uint64, tag = "3")]
    pub scan: u64,
    #[prost(uint64, tag = "4")]
    pub coprocessor: u64,
    #[prost(uint64, tag = "5")]
    pub delete: u64,
    #[prost(uint64, tag = "6")]
    pub delete_range: u64,
    #[prost(uint64, tag = "7")]
    pub put: u64,
    #[prost(uint64, tag = "8")]
    pub prewrite: u64,
    #[prost(uint64, tag = "9")]
    pub acquire_pessimistic_lock: u64,
    #[prost(uint64, tag = "10")]
    pub commit: u64,
    #[prost(uint64, tag = "11")]
    pub rollback: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportBucketsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(message, optional, tag = "3")]
    pub buckets: ::core::option::Option<super::metapb::Buckets>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportBucketsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportMinResolvedTsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub store_id: u64,
    #[prost(uint64, tag = "3")]
    pub min_resolved_ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportMinResolvedTsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetExternalTimestampRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub timestamp: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetExternalTimestampResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetExternalTimestampRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetExternalTimestampResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub timestamp: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMinTsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMinTsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub timestamp: ::core::option::Option<Timestamp>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EventType {
    Put = 0,
    Delete = 1,
}
impl EventType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            EventType::Put => "PUT",
            EventType::Delete => "DELETE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PUT" => Some(Self::Put),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorType {
    Ok = 0,
    Unknown = 1,
    NotBootstrapped = 2,
    StoreTombstone = 3,
    AlreadyBootstrapped = 4,
    IncompatibleVersion = 5,
    RegionNotFound = 6,
    GlobalConfigNotFound = 7,
    DuplicatedEntry = 8,
    EntryNotFound = 9,
    InvalidValue = 10,
    /// required watch revision is smaller than current compact/min revision.
    DataCompacted = 11,
}
impl ErrorType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ErrorType::Ok => "OK",
            ErrorType::Unknown => "UNKNOWN",
            ErrorType::NotBootstrapped => "NOT_BOOTSTRAPPED",
            ErrorType::StoreTombstone => "STORE_TOMBSTONE",
            ErrorType::AlreadyBootstrapped => "ALREADY_BOOTSTRAPPED",
            ErrorType::IncompatibleVersion => "INCOMPATIBLE_VERSION",
            ErrorType::RegionNotFound => "REGION_NOT_FOUND",
            ErrorType::GlobalConfigNotFound => "GLOBAL_CONFIG_NOT_FOUND",
            ErrorType::DuplicatedEntry => "DUPLICATED_ENTRY",
            ErrorType::EntryNotFound => "ENTRY_NOT_FOUND",
            ErrorType::InvalidValue => "INVALID_VALUE",
            ErrorType::DataCompacted => "DATA_COMPACTED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "OK" => Some(Self::Ok),
            "UNKNOWN" => Some(Self::Unknown),
            "NOT_BOOTSTRAPPED" => Some(Self::NotBootstrapped),
            "STORE_TOMBSTONE" => Some(Self::StoreTombstone),
            "ALREADY_BOOTSTRAPPED" => Some(Self::AlreadyBootstrapped),
            "INCOMPATIBLE_VERSION" => Some(Self::IncompatibleVersion),
            "REGION_NOT_FOUND" => Some(Self::RegionNotFound),
            "GLOBAL_CONFIG_NOT_FOUND" => Some(Self::GlobalConfigNotFound),
            "DUPLICATED_ENTRY" => Some(Self::DuplicatedEntry),
            "ENTRY_NOT_FOUND" => Some(Self::EntryNotFound),
            "INVALID_VALUE" => Some(Self::InvalidValue),
            "DATA_COMPACTED" => Some(Self::DataCompacted),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServiceMode {
    UnknownSvcMode = 0,
    PdSvcMode = 1,
    ApiSvcMode = 2,
}
impl ServiceMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ServiceMode::UnknownSvcMode => "UNKNOWN_SVC_MODE",
            ServiceMode::PdSvcMode => "PD_SVC_MODE",
            ServiceMode::ApiSvcMode => "API_SVC_MODE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN_SVC_MODE" => Some(Self::UnknownSvcMode),
            "PD_SVC_MODE" => Some(Self::PdSvcMode),
            "API_SVC_MODE" => Some(Self::ApiSvcMode),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CheckPolicy {
    Scan = 0,
    Approximate = 1,
    Usekey = 2,
}
impl CheckPolicy {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CheckPolicy::Scan => "SCAN",
            CheckPolicy::Approximate => "APPROXIMATE",
            CheckPolicy::Usekey => "USEKEY",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SCAN" => Some(Self::Scan),
            "APPROXIMATE" => Some(Self::Approximate),
            "USEKEY" => Some(Self::Usekey),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ControlGrpcEvent {
    /// Pause TiKV grpc server.
    Pause = 0,
    /// Resume TiKV grpc server.
    Resume = 1,
}
impl ControlGrpcEvent {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ControlGrpcEvent::Pause => "PAUSE",
            ControlGrpcEvent::Resume => "RESUME",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PAUSE" => Some(Self::Pause),
            "RESUME" => Some(Self::Resume),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OperatorStatus {
    Success = 0,
    Timeout = 1,
    Cancel = 2,
    Replace = 3,
    Running = 4,
}
impl OperatorStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            OperatorStatus::Success => "SUCCESS",
            OperatorStatus::Timeout => "TIMEOUT",
            OperatorStatus::Cancel => "CANCEL",
            OperatorStatus::Replace => "REPLACE",
            OperatorStatus::Running => "RUNNING",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SUCCESS" => Some(Self::Success),
            "TIMEOUT" => Some(Self::Timeout),
            "CANCEL" => Some(Self::Cancel),
            "REPLACE" => Some(Self::Replace),
            "RUNNING" => Some(Self::Running),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum QueryKind {
    Others = 0,
    Gc = 1,
    Get = 2,
    Scan = 3,
    Coprocessor = 4,
    Delete = 5,
    DeleteRange = 6,
    Put = 7,
    Prewrite = 8,
    AcquirePessimisticLock = 9,
    Commit = 10,
    Rollback = 11,
}
impl QueryKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            QueryKind::Others => "Others",
            QueryKind::Gc => "GC",
            QueryKind::Get => "Get",
            QueryKind::Scan => "Scan",
            QueryKind::Coprocessor => "Coprocessor",
            QueryKind::Delete => "Delete",
            QueryKind::DeleteRange => "DeleteRange",
            QueryKind::Put => "Put",
            QueryKind::Prewrite => "Prewrite",
            QueryKind::AcquirePessimisticLock => "AcquirePessimisticLock",
            QueryKind::Commit => "Commit",
            QueryKind::Rollback => "Rollback",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Others" => Some(Self::Others),
            "GC" => Some(Self::Gc),
            "Get" => Some(Self::Get),
            "Scan" => Some(Self::Scan),
            "Coprocessor" => Some(Self::Coprocessor),
            "Delete" => Some(Self::Delete),
            "DeleteRange" => Some(Self::DeleteRange),
            "Put" => Some(Self::Put),
            "Prewrite" => Some(Self::Prewrite),
            "AcquirePessimisticLock" => Some(Self::AcquirePessimisticLock),
            "Commit" => Some(Self::Commit),
            "Rollback" => Some(Self::Rollback),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod pd_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct PdClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl PdClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> PdClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> PdClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            PdClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        /// GetClusterInfo get the information of this cluster. It does not require
        /// the cluster_id in request matchs the id of this cluster.
        pub async fn get_cluster_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetClusterInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetClusterInfoResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetClusterInfo");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetClusterInfo"));
            self.inner.unary(req, path, codec).await
        }
        /// GetMembers get the member list of this cluster. It does not require
        /// the cluster_id in request matchs the id of this cluster.
        pub async fn get_members(
            &mut self,
            request: impl tonic::IntoRequest<super::GetMembersRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMembersResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetMembers");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetMembers"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn tso(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::TsoRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::TsoResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/Tso");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "Tso"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn bootstrap(
            &mut self,
            request: impl tonic::IntoRequest<super::BootstrapRequest>,
        ) -> std::result::Result<
            tonic::Response<super::BootstrapResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/Bootstrap");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "Bootstrap"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn is_bootstrapped(
            &mut self,
            request: impl tonic::IntoRequest<super::IsBootstrappedRequest>,
        ) -> std::result::Result<
            tonic::Response<super::IsBootstrappedResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/IsBootstrapped");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "IsBootstrapped"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn alloc_id(
            &mut self,
            request: impl tonic::IntoRequest<super::AllocIdRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AllocIdResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/AllocID");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "AllocID"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn is_snapshot_recovering(
            &mut self,
            request: impl tonic::IntoRequest<super::IsSnapshotRecoveringRequest>,
        ) -> std::result::Result<
            tonic::Response<super::IsSnapshotRecoveringResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/IsSnapshotRecovering",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "IsSnapshotRecovering"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_store(
            &mut self,
            request: impl tonic::IntoRequest<super::GetStoreRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetStoreResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetStore");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetStore"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn put_store(
            &mut self,
            request: impl tonic::IntoRequest<super::PutStoreRequest>,
        ) -> std::result::Result<
            tonic::Response<super::PutStoreResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/PutStore");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "PutStore"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_all_stores(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAllStoresRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetAllStoresResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetAllStores");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetAllStores"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn store_heartbeat(
            &mut self,
            request: impl tonic::IntoRequest<super::StoreHeartbeatRequest>,
        ) -> std::result::Result<
            tonic::Response<super::StoreHeartbeatResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/StoreHeartbeat");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "StoreHeartbeat"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn region_heartbeat(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::RegionHeartbeatRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::RegionHeartbeatResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/RegionHeartbeat");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "RegionHeartbeat"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn get_region(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRegionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRegionResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetRegion");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetRegion"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_prev_region(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRegionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRegionResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetPrevRegion");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetPrevRegion"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_region_by_id(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRegionByIdRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRegionResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetRegionByID");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetRegionByID"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn scan_regions(
            &mut self,
            request: impl tonic::IntoRequest<super::ScanRegionsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ScanRegionsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/ScanRegions");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "ScanRegions"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn ask_split(
            &mut self,
            request: impl tonic::IntoRequest<super::AskSplitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AskSplitResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/AskSplit");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "AskSplit"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn report_split(
            &mut self,
            request: impl tonic::IntoRequest<super::ReportSplitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ReportSplitResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/ReportSplit");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "ReportSplit"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn ask_batch_split(
            &mut self,
            request: impl tonic::IntoRequest<super::AskBatchSplitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AskBatchSplitResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/AskBatchSplit");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "AskBatchSplit"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn report_batch_split(
            &mut self,
            request: impl tonic::IntoRequest<super::ReportBatchSplitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ReportBatchSplitResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/ReportBatchSplit");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "ReportBatchSplit"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_cluster_config(
            &mut self,
            request: impl tonic::IntoRequest<super::GetClusterConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetClusterConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetClusterConfig");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetClusterConfig"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn put_cluster_config(
            &mut self,
            request: impl tonic::IntoRequest<super::PutClusterConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::PutClusterConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/PutClusterConfig");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "PutClusterConfig"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn scatter_region(
            &mut self,
            request: impl tonic::IntoRequest<super::ScatterRegionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ScatterRegionResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/ScatterRegion");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "ScatterRegion"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_gc_safe_point(
            &mut self,
            request: impl tonic::IntoRequest<super::GetGcSafePointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetGcSafePointResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetGCSafePoint");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetGCSafePoint"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_gc_safe_point(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateGcSafePointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateGcSafePointResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/UpdateGCSafePoint",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "UpdateGCSafePoint"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_service_gc_safe_point(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateServiceGcSafePointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateServiceGcSafePointResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/UpdateServiceGCSafePoint",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "UpdateServiceGCSafePoint"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_gc_safe_point_v2(
            &mut self,
            request: impl tonic::IntoRequest<super::GetGcSafePointV2Request>,
        ) -> std::result::Result<
            tonic::Response<super::GetGcSafePointV2Response>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetGCSafePointV2");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetGCSafePointV2"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn watch_gc_safe_point_v2(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchGcSafePointV2Request>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::WatchGcSafePointV2Response>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/WatchGCSafePointV2",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "WatchGCSafePointV2"));
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn update_gc_safe_point_v2(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateGcSafePointV2Request>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateGcSafePointV2Response>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/UpdateGCSafePointV2",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "UpdateGCSafePointV2"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_service_safe_point_v2(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateServiceSafePointV2Request>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateServiceSafePointV2Response>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/UpdateServiceSafePointV2",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "UpdateServiceSafePointV2"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_all_gc_safe_point_v2(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAllGcSafePointV2Request>,
        ) -> std::result::Result<
            tonic::Response<super::GetAllGcSafePointV2Response>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/GetAllGCSafePointV2",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "GetAllGCSafePointV2"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn sync_regions(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::SyncRegionRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::SyncRegionResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/SyncRegions");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "SyncRegions"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn get_operator(
            &mut self,
            request: impl tonic::IntoRequest<super::GetOperatorRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetOperatorResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetOperator");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetOperator"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn sync_max_ts(
            &mut self,
            request: impl tonic::IntoRequest<super::SyncMaxTsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SyncMaxTsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/SyncMaxTS");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "SyncMaxTS"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn split_regions(
            &mut self,
            request: impl tonic::IntoRequest<super::SplitRegionsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SplitRegionsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/SplitRegions");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "SplitRegions"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn split_and_scatter_regions(
            &mut self,
            request: impl tonic::IntoRequest<super::SplitAndScatterRegionsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SplitAndScatterRegionsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/SplitAndScatterRegions",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "SplitAndScatterRegions"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_dc_location_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetDcLocationInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetDcLocationInfoResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/GetDCLocationInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetDCLocationInfo"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn store_global_config(
            &mut self,
            request: impl tonic::IntoRequest<super::StoreGlobalConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::StoreGlobalConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/StoreGlobalConfig",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "StoreGlobalConfig"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn load_global_config(
            &mut self,
            request: impl tonic::IntoRequest<super::LoadGlobalConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::LoadGlobalConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/LoadGlobalConfig");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "LoadGlobalConfig"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn watch_global_config(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchGlobalConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::WatchGlobalConfigResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/WatchGlobalConfig",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "WatchGlobalConfig"));
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn report_buckets(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::ReportBucketsRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::ReportBucketsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/ReportBuckets");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "ReportBuckets"));
            self.inner.client_streaming(req, path, codec).await
        }
        pub async fn report_min_resolved_ts(
            &mut self,
            request: impl tonic::IntoRequest<super::ReportMinResolvedTsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ReportMinResolvedTsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/ReportMinResolvedTS",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "ReportMinResolvedTS"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn set_external_timestamp(
            &mut self,
            request: impl tonic::IntoRequest<super::SetExternalTimestampRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SetExternalTimestampResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/SetExternalTimestamp",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "SetExternalTimestamp"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_external_timestamp(
            &mut self,
            request: impl tonic::IntoRequest<super::GetExternalTimestampRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetExternalTimestampResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/pdpb.PD/GetExternalTimestamp",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("pdpb.PD", "GetExternalTimestamp"));
            self.inner.unary(req, path, codec).await
        }
        /// Get the minimum timestamp across all keyspace groups from API server
        /// TODO: Currently, we need to ask API server to get the minimum timestamp.
        /// Once we support service discovery, we can remove it.
        pub async fn get_min_ts(
            &mut self,
            request: impl tonic::IntoRequest<super::GetMinTsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMinTsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/pdpb.PD/GetMinTS");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("pdpb.PD", "GetMinTS"));
            self.inner.unary(req, path, codec).await
        }
    }
}
