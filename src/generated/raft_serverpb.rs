#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftMessage {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(message, optional, tag = "2")]
    pub from_peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, optional, tag = "3")]
    pub to_peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, optional, tag = "4")]
    pub message: ::core::option::Option<super::eraftpb::Message>,
    #[prost(message, optional, tag = "5")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    /// true means to_peer is a tombstone peer and it should remove itself.
    #[prost(bool, tag = "6")]
    pub is_tombstone: bool,
    /// Region key range \[start_key, end_key).
    #[prost(bytes = "vec", tag = "7")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    /// If it has value, to_peer should be removed if merge is never going to complete.
    #[prost(message, optional, tag = "9")]
    pub merge_target: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "10")]
    pub extra_msg: ::core::option::Option<ExtraMessage>,
    #[prost(bytes = "vec", tag = "11")]
    pub extra_ctx: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration = "super::disk_usage::DiskUsage", tag = "12")]
    pub disk_usage: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftTruncatedState {
    #[prost(uint64, tag = "1")]
    pub index: u64,
    #[prost(uint64, tag = "2")]
    pub term: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotCfFile {
    #[prost(string, tag = "1")]
    pub cf: ::prost::alloc::string::String,
    #[prost(uint64, tag = "2")]
    pub size: u64,
    #[prost(uint32, tag = "3")]
    pub checksum: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotMeta {
    #[prost(message, repeated, tag = "1")]
    pub cf_files: ::prost::alloc::vec::Vec<SnapshotCfFile>,
    /// true means this snapshot is triggered for load balance
    #[prost(bool, tag = "2")]
    pub for_balance: bool,
    /// true means this is an empty snapshot for witness
    #[prost(bool, tag = "3")]
    pub for_witness: bool,
    /// the timestamp second to generate snapshot
    #[prost(uint64, tag = "4")]
    pub start: u64,
    /// the duration of generating snapshot
    #[prost(uint64, tag = "5")]
    pub generate_duration_sec: u64,
    /// the path of the tablet snapshot, it should only be used for v1 to receive
    /// snapshot from v2
    #[prost(string, tag = "6")]
    pub tablet_snap_path: ::prost::alloc::string::String,
    /// A hint of the latest commit index on leader when sending snapshot.
    /// It should only be used for v2 to send snapshot to v1.
    /// See <https://github.com/pingcap/tiflash/issues/7568>
    #[prost(uint64, tag = "7")]
    pub commit_index_hint: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotChunk {
    #[prost(message, optional, tag = "1")]
    pub message: ::core::option::Option<RaftMessage>,
    #[prost(bytes = "vec", tag = "2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Done {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotFileMeta {
    #[prost(uint64, tag = "1")]
    pub file_size: u64,
    #[prost(string, tag = "2")]
    pub file_name: ::prost::alloc::string::String,
    /// Some block data. Unencrypted.
    #[prost(bytes = "vec", tag = "3")]
    pub head_chunk: ::prost::alloc::vec::Vec<u8>,
    /// trailing data including checksum. Unencrypted.
    #[prost(bytes = "vec", tag = "4")]
    pub trailing_chunk: ::prost::alloc::vec::Vec<u8>,
}
/// Snapshot preview for server to decide whether skip some files.
/// Server should send back an `AcceptedSnapshotFile` to let client
/// keep sending specified files. Only SST files can be skipped, all
/// other files should always be sent.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotPreview {
    #[prost(message, repeated, tag = "1")]
    pub metas: ::prost::alloc::vec::Vec<TabletSnapshotFileMeta>,
    /// There may be too many metas, use a flag to indicate all metas
    /// are sent.
    #[prost(bool, tag = "2")]
    pub end: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotFileChunk {
    #[prost(uint64, tag = "1")]
    pub file_size: u64,
    #[prost(string, tag = "2")]
    pub file_name: ::prost::alloc::string::String,
    /// Encrypted.
    #[prost(bytes = "vec", tag = "3")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    /// Initial vector if encryption is enabled.
    #[prost(bytes = "vec", tag = "4")]
    pub iv: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "5")]
    pub key: ::core::option::Option<super::encryptionpb::DataKey>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotHead {
    #[prost(message, optional, tag = "1")]
    pub message: ::core::option::Option<RaftMessage>,
    #[prost(bool, tag = "2")]
    pub use_cache: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotEnd {
    /// Checksum of all data sent in `TabletSnapshotFileChunk.data` and
    /// `TabletSnapshotFileChunk.file_name`.
    #[prost(uint64, tag = "1")]
    pub checksum: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotRequest {
    #[prost(oneof = "tablet_snapshot_request::Payload", tags = "1, 2, 3, 4")]
    pub payload: ::core::option::Option<tablet_snapshot_request::Payload>,
}
/// Nested message and enum types in `TabletSnapshotRequest`.
pub mod tablet_snapshot_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Payload {
        #[prost(message, tag = "1")]
        Head(super::TabletSnapshotHead),
        #[prost(message, tag = "2")]
        Preview(super::TabletSnapshotPreview),
        #[prost(message, tag = "3")]
        Chunk(super::TabletSnapshotFileChunk),
        #[prost(message, tag = "4")]
        End(super::TabletSnapshotEnd),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AcceptedSnapshotFiles {
    #[prost(string, repeated, tag = "1")]
    pub file_name: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabletSnapshotResponse {
    #[prost(message, optional, tag = "1")]
    pub files: ::core::option::Option<AcceptedSnapshotFiles>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyValue {
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftSnapshotData {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(uint64, tag = "2")]
    pub file_size: u64,
    #[prost(message, repeated, tag = "3")]
    pub data: ::prost::alloc::vec::Vec<KeyValue>,
    #[prost(uint64, tag = "4")]
    pub version: u64,
    #[prost(message, optional, tag = "5")]
    pub meta: ::core::option::Option<SnapshotMeta>,
    #[prost(message, repeated, tag = "6")]
    pub removed_records: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    #[prost(message, repeated, tag = "7")]
    pub merged_records: ::prost::alloc::vec::Vec<MergedRecord>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreIdent {
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(uint64, tag = "2")]
    pub store_id: u64,
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "3")]
    pub api_version: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreRecoverState {
    /// Used for TiKV start recovery when WAL of KVDB was disabled.
    /// TiKV may read all relations between seqno and raft log index, and replay
    /// all raft logs which corresponding seqno smaller than the seqno here.
    /// After TiKV replays all raft logs and flushed KV data, the seqno here must
    /// be updated.
    #[prost(uint64, tag = "1")]
    pub seqno: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftLocalState {
    #[prost(message, optional, tag = "1")]
    pub hard_state: ::core::option::Option<super::eraftpb::HardState>,
    #[prost(uint64, tag = "2")]
    pub last_index: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftApplyState {
    #[prost(uint64, tag = "1")]
    pub applied_index: u64,
    #[prost(uint64, tag = "3")]
    pub last_commit_index: u64,
    #[prost(uint64, tag = "4")]
    pub commit_index: u64,
    #[prost(uint64, tag = "5")]
    pub commit_term: u64,
    #[prost(message, optional, tag = "2")]
    pub truncated_state: ::core::option::Option<RaftTruncatedState>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MergeState {
    #[prost(uint64, tag = "1")]
    pub min_index: u64,
    #[prost(message, optional, tag = "2")]
    pub target: ::core::option::Option<super::metapb::Region>,
    #[prost(uint64, tag = "3")]
    pub commit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MergedRecord {
    #[prost(uint64, tag = "1")]
    pub source_region_id: u64,
    #[prost(message, optional, tag = "2")]
    pub source_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    /// Peers of source region when merge is committed.
    #[prost(message, repeated, tag = "3")]
    pub source_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// Removed peers (by confchange) of source region when merge is committed.
    #[prost(message, repeated, tag = "9")]
    pub source_removed_records: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    #[prost(uint64, tag = "4")]
    pub target_region_id: u64,
    #[prost(message, optional, tag = "5")]
    pub target_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(message, repeated, tag = "6")]
    pub target_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// Commit merge index.
    #[prost(uint64, tag = "7")]
    pub index: u64,
    /// Prepare merge index.
    #[prost(uint64, tag = "8")]
    pub source_index: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionLocalState {
    #[prost(enumeration = "PeerState", tag = "1")]
    pub state: i32,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "3")]
    pub merge_state: ::core::option::Option<MergeState>,
    /// The apply index corresponding to the storage when it's initialized.
    #[prost(uint64, tag = "4")]
    pub tablet_index: u64,
    /// Raft doesn't guarantee peer will be removed in the end. In v1, peer finds
    /// out its destiny by logs or broadcast; in v2, leader is responsible to
    /// ensure removed peers are destroyed.
    /// Note: only peers who has been part of this region can be in this list.
    #[prost(message, repeated, tag = "5")]
    pub removed_records: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// Merged peer can't be deleted like gc peers. Instead, leader needs to
    /// query target peer to decide whether source peer can be destroyed.
    #[prost(message, repeated, tag = "6")]
    pub merged_records: ::prost::alloc::vec::Vec<MergedRecord>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionSequenceNumberRelation {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub sequence_number: u64,
    #[prost(message, optional, tag = "3")]
    pub apply_state: ::core::option::Option<RaftApplyState>,
    #[prost(message, optional, tag = "4")]
    pub region_state: ::core::option::Option<RegionLocalState>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AvailabilityContext {
    #[prost(uint64, tag = "1")]
    pub from_region_id: u64,
    #[prost(message, optional, tag = "2")]
    pub from_region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(bool, tag = "3")]
    pub unavailable: bool,
    #[prost(bool, tag = "4")]
    pub trimmed: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FlushMemtable {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RefreshBuckets {
    #[prost(uint64, tag = "1")]
    pub version: u64,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub keys: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint64, repeated, tag = "3")]
    pub sizes: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckGcPeer {
    /// The region ID who triggers the check and wait for report. It should be
    /// the ID of RaftMessage.from.
    #[prost(uint64, tag = "1")]
    pub from_region_id: u64,
    /// The region ID to be checked if should be destroyed.
    #[prost(uint64, tag = "2")]
    pub check_region_id: u64,
    /// The epoch of the region to be checked.
    #[prost(message, optional, tag = "3")]
    pub check_region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    /// The peer to be checked.
    #[prost(message, optional, tag = "4")]
    pub check_peer: ::core::option::Option<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExtraMessage {
    #[prost(enumeration = "ExtraMessageType", tag = "1")]
    pub r#type: i32,
    /// It's merge related index. In `WantRollbackMerge`, it's prepare merge index. In
    /// `MsgGcPeerRequest`, it's the commit merge index. In `MsgVoterReplicatedIndexRequest`
    /// it's the voter_replicated_index.
    #[prost(uint64, tag = "2")]
    pub index: u64,
    /// In `MsgCheckStalePeerResponse`, it's the peers that receiver can continue to query.
    #[prost(message, repeated, tag = "3")]
    pub check_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    #[prost(bool, tag = "4")]
    pub wait_data: bool,
    /// Flag for forcely wake up hibernate regions if true.
    #[prost(bool, tag = "5")]
    pub forcely_awaken: bool,
    #[prost(message, optional, tag = "6")]
    pub check_gc_peer: ::core::option::Option<CheckGcPeer>,
    #[prost(message, optional, tag = "7")]
    pub flush_memtable: ::core::option::Option<FlushMemtable>,
    /// Used by `MsgAvailabilityRequest` and `MsgAvailabilityResponse` in v2.
    #[prost(message, optional, tag = "8")]
    pub availability_context: ::core::option::Option<AvailabilityContext>,
    /// notice the peer to refresh buckets version
    #[prost(message, optional, tag = "9")]
    pub refresh_buckets: ::core::option::Option<RefreshBuckets>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PeerState {
    Normal = 0,
    Applying = 1,
    Tombstone = 2,
    Merging = 3,
    /// Currently used for witness to non-witness conversion: When a witness
    /// has just become a non-witness, we need to set and persist this state,
    /// so that when the service restarts before applying snapshot, we can
    /// actively request snapshot when initializing this peer.
    Unavailable = 4,
}
impl PeerState {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            PeerState::Normal => "Normal",
            PeerState::Applying => "Applying",
            PeerState::Tombstone => "Tombstone",
            PeerState::Merging => "Merging",
            PeerState::Unavailable => "Unavailable",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Normal" => Some(Self::Normal),
            "Applying" => Some(Self::Applying),
            "Tombstone" => Some(Self::Tombstone),
            "Merging" => Some(Self::Merging),
            "Unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ExtraMessageType {
    MsgRegionWakeUp = 0,
    MsgWantRollbackMerge = 1,
    MsgCheckStalePeer = 2,
    MsgCheckStalePeerResponse = 3,
    /// If leader is going to sleep, it will send requests to all its followers
    /// to make sure they all agree to sleep.
    MsgHibernateRequest = 4,
    MsgHibernateResponse = 5,
    MsgRejectRaftLogCausedByMemoryUsage = 6,
    MsgAvailabilityRequest = 7,
    MsgAvailabilityResponse = 8,
    MsgVoterReplicatedIndexRequest = 9,
    MsgVoterReplicatedIndexResponse = 10,
    /// Message means that `from` is tombstone. Leader can then update removed_records.
    MsgGcPeerRequest = 11,
    MsgGcPeerResponse = 12,
    MsgFlushMemtable = 13,
    MsgRefreshBuckets = 14,
}
impl ExtraMessageType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ExtraMessageType::MsgRegionWakeUp => "MsgRegionWakeUp",
            ExtraMessageType::MsgWantRollbackMerge => "MsgWantRollbackMerge",
            ExtraMessageType::MsgCheckStalePeer => "MsgCheckStalePeer",
            ExtraMessageType::MsgCheckStalePeerResponse => "MsgCheckStalePeerResponse",
            ExtraMessageType::MsgHibernateRequest => "MsgHibernateRequest",
            ExtraMessageType::MsgHibernateResponse => "MsgHibernateResponse",
            ExtraMessageType::MsgRejectRaftLogCausedByMemoryUsage => {
                "MsgRejectRaftLogCausedByMemoryUsage"
            }
            ExtraMessageType::MsgAvailabilityRequest => "MsgAvailabilityRequest",
            ExtraMessageType::MsgAvailabilityResponse => "MsgAvailabilityResponse",
            ExtraMessageType::MsgVoterReplicatedIndexRequest => {
                "MsgVoterReplicatedIndexRequest"
            }
            ExtraMessageType::MsgVoterReplicatedIndexResponse => {
                "MsgVoterReplicatedIndexResponse"
            }
            ExtraMessageType::MsgGcPeerRequest => "MsgGcPeerRequest",
            ExtraMessageType::MsgGcPeerResponse => "MsgGcPeerResponse",
            ExtraMessageType::MsgFlushMemtable => "MsgFlushMemtable",
            ExtraMessageType::MsgRefreshBuckets => "MsgRefreshBuckets",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "MsgRegionWakeUp" => Some(Self::MsgRegionWakeUp),
            "MsgWantRollbackMerge" => Some(Self::MsgWantRollbackMerge),
            "MsgCheckStalePeer" => Some(Self::MsgCheckStalePeer),
            "MsgCheckStalePeerResponse" => Some(Self::MsgCheckStalePeerResponse),
            "MsgHibernateRequest" => Some(Self::MsgHibernateRequest),
            "MsgHibernateResponse" => Some(Self::MsgHibernateResponse),
            "MsgRejectRaftLogCausedByMemoryUsage" => {
                Some(Self::MsgRejectRaftLogCausedByMemoryUsage)
            }
            "MsgAvailabilityRequest" => Some(Self::MsgAvailabilityRequest),
            "MsgAvailabilityResponse" => Some(Self::MsgAvailabilityResponse),
            "MsgVoterReplicatedIndexRequest" => {
                Some(Self::MsgVoterReplicatedIndexRequest)
            }
            "MsgVoterReplicatedIndexResponse" => {
                Some(Self::MsgVoterReplicatedIndexResponse)
            }
            "MsgGcPeerRequest" => Some(Self::MsgGcPeerRequest),
            "MsgGcPeerResponse" => Some(Self::MsgGcPeerResponse),
            "MsgFlushMemtable" => Some(Self::MsgFlushMemtable),
            "MsgRefreshBuckets" => Some(Self::MsgRefreshBuckets),
            _ => None,
        }
    }
}
