/// NotLeader is the error variant that tells a request be handle by raft leader
/// is sent to raft follower or learner.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NotLeader {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    /// Region leader of the requested region
    #[prost(message, optional, tag = "2")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
}
/// IsWitness is the error variant that tells a request be handle by witness
/// which should be forbidden and retry.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsWitness {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
/// BucketVersionNotMatch is the error variant that tells the request buckets version is not match.
/// client should update the buckets version and retry.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BucketVersionNotMatch {
    #[prost(uint64, tag = "1")]
    pub version: u64,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub keys: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiskFull {
    /// The requested store ID
    #[prost(uint64, repeated, tag = "1")]
    pub store_id: ::prost::alloc::vec::Vec<u64>,
    /// The detailed info
    #[prost(string, tag = "2")]
    pub reason: ::prost::alloc::string::String,
}
/// StoreNotMatch is the error variant that tells the request is sent to wrong store.
/// (i.e. inconsistency of the store ID that request shows and the real store ID of this server.)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreNotMatch {
    /// Store id in request
    #[prost(uint64, tag = "1")]
    pub request_store_id: u64,
    /// Actual store id
    #[prost(uint64, tag = "2")]
    pub actual_store_id: u64,
}
/// RegionNotFound is the error variant that tells there isn't any region in this TiKV
/// matches the requested region ID.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionNotFound {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
/// RegionNotInitialized is the error variant that tells there isn't any initialized peer
/// matchesthe request region ID.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionNotInitialized {
    /// The request region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
/// KeyNotInRegion is the error variant that tells the key the request requires isn't present in
/// this region.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyNotInRegion {
    /// The requested key
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    /// The requested region ID
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    /// Start key of the requested region
    #[prost(bytes = "vec", tag = "3")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    /// Snd key of the requested region
    #[prost(bytes = "vec", tag = "4")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
}
/// EpochNotMatch is the error variant that tells a region has been updated.
/// (e.g. by splitting / merging, or raft Confchange.)
/// Hence, a command is based on a stale version of a region.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EpochNotMatch {
    /// Available regions that may be siblings of the requested one.
    #[prost(message, repeated, tag = "1")]
    pub current_regions: ::prost::alloc::vec::Vec<super::metapb::Region>,
}
/// ServerIsBusy is the error variant that tells the server is too busy to response.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerIsBusy {
    #[prost(string, tag = "1")]
    pub reason: ::prost::alloc::string::String,
    /// The suggested backoff time
    #[prost(uint64, tag = "2")]
    pub backoff_ms: u64,
    #[prost(uint32, tag = "3")]
    pub estimated_wait_ms: u32,
    /// Current applied_index at the leader, may be used in replica read.
    #[prost(uint64, tag = "4")]
    pub applied_index: u64,
}
/// StaleCommand is the error variant that tells the command is stale, that is,
/// the current request term is lower than current raft term.
/// This can be retried at most time.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StaleCommand {}
/// RaftEntryTooLarge is the error variant that tells the request is too large to be serialized to a
/// reasonable small raft entry.
/// (i.e. greater than the configured value `raft_entry_max_size` in `raftstore`)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftEntryTooLarge {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    /// Size of the raft entry
    #[prost(uint64, tag = "2")]
    pub entry_size: u64,
}
/// MaxTimestampNotSynced is the error variant that tells the peer has just become a leader and
/// updating the max timestamp in the concurrency manager from PD TSO is ongoing. In this case,
/// the prewrite of an async commit transaction cannot succeed. The client can backoff and
/// resend the request.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MaxTimestampNotSynced {}
/// ReadIndexNotReady is the error variant that tells the read index request is not ready, that is,
/// the current region is in a status that not ready to serve the read index request. For example,
/// region is in splitting or merging status.
/// This can be retried at most time.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadIndexNotReady {
    /// The reason why the region is not ready to serve read index request
    #[prost(string, tag = "1")]
    pub reason: ::prost::alloc::string::String,
    /// The requested region ID
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
}
/// ProposalInMergingMode is the error variant that tells the proposal is rejected because raft is
/// in the merging mode. This may happen when BR/Lightning try to ingest SST.
/// This can be retried at most time.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProposalInMergingMode {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataIsNotReady {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub peer_id: u64,
    #[prost(uint64, tag = "3")]
    pub safe_ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoveryInProgress {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FlashbackInProgress {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub flashback_start_ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FlashbackNotPrepared {
    /// The requested region ID
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
/// MismatchPeerId is the error variant that tells the request is sent to wrong peer.
/// Client receives this error should reload the region info and retry.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MismatchPeerId {
    #[prost(uint64, tag = "1")]
    pub request_peer_id: u64,
    #[prost(uint64, tag = "2")]
    pub store_peer_id: u64,
}
/// Error wraps all region errors, indicates an error encountered by a request.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    /// The error message
    #[prost(string, tag = "1")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub not_leader: ::core::option::Option<NotLeader>,
    #[prost(message, optional, tag = "3")]
    pub region_not_found: ::core::option::Option<RegionNotFound>,
    #[prost(message, optional, tag = "4")]
    pub key_not_in_region: ::core::option::Option<KeyNotInRegion>,
    #[prost(message, optional, tag = "5")]
    pub epoch_not_match: ::core::option::Option<EpochNotMatch>,
    #[prost(message, optional, tag = "6")]
    pub server_is_busy: ::core::option::Option<ServerIsBusy>,
    #[prost(message, optional, tag = "7")]
    pub stale_command: ::core::option::Option<StaleCommand>,
    #[prost(message, optional, tag = "8")]
    pub store_not_match: ::core::option::Option<StoreNotMatch>,
    #[prost(message, optional, tag = "9")]
    pub raft_entry_too_large: ::core::option::Option<RaftEntryTooLarge>,
    #[prost(message, optional, tag = "10")]
    pub max_timestamp_not_synced: ::core::option::Option<MaxTimestampNotSynced>,
    #[prost(message, optional, tag = "11")]
    pub read_index_not_ready: ::core::option::Option<ReadIndexNotReady>,
    #[prost(message, optional, tag = "12")]
    pub proposal_in_merging_mode: ::core::option::Option<ProposalInMergingMode>,
    #[prost(message, optional, tag = "13")]
    pub data_is_not_ready: ::core::option::Option<DataIsNotReady>,
    #[prost(message, optional, tag = "14")]
    pub region_not_initialized: ::core::option::Option<RegionNotInitialized>,
    #[prost(message, optional, tag = "15")]
    pub disk_full: ::core::option::Option<DiskFull>,
    /// Online recovery is still in performing, reject writes to avoid potential issues
    #[prost(message, optional, tag = "16")]
    pub recovery_in_progress: ::core::option::Option<RecoveryInProgress>,
    /// Flashback is still in performing, reject any read or write to avoid potential issues.
    /// NOTICE: this error is non-retryable, the request should fail ASAP when it meets this error.
    #[prost(message, optional, tag = "17")]
    pub flashback_in_progress: ::core::option::Option<FlashbackInProgress>,
    /// If the second phase flashback request is sent to a region that is not prepared for the flashback,
    /// this error will be returned.
    /// NOTICE: this error is non-retryable, the client should retry the first phase flashback request when it meets this error.
    #[prost(message, optional, tag = "18")]
    pub flashback_not_prepared: ::core::option::Option<FlashbackNotPrepared>,
    /// IsWitness is the error variant that tells a request be handle by witness
    /// which should be forbidden and retry.
    #[prost(message, optional, tag = "19")]
    pub is_witness: ::core::option::Option<IsWitness>,
    #[prost(message, optional, tag = "20")]
    pub mismatch_peer_id: ::core::option::Option<MismatchPeerId>,
    /// BucketVersionNotMatch is the error variant that tells the request buckets version is not match.
    #[prost(message, optional, tag = "21")]
    pub bucket_version_not_match: ::core::option::Option<BucketVersionNotMatch>,
}
