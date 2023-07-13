#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRequest {
    #[prost(string, tag = "1")]
    pub cf: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutRequest {
    #[prost(string, tag = "1")]
    pub cf: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRequest {
    #[prost(string, tag = "1")]
    pub cf: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRangeRequest {
    #[prost(string, tag = "1")]
    pub cf: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag = "4")]
    pub notify_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRangeResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapResponse {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrewriteRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub lock: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrewriteResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngestSstRequest {
    #[prost(message, optional, tag = "1")]
    pub sst: ::core::option::Option<super::import_sstpb::SstMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngestSstResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadIndexRequest {
    /// In replica read, leader uses start_ts and key_ranges to check memory locks.
    #[prost(uint64, tag = "1")]
    pub start_ts: u64,
    #[prost(message, repeated, tag = "2")]
    pub key_ranges: ::prost::alloc::vec::Vec<super::kvrpcpb::KeyRange>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadIndexResponse {
    #[prost(uint64, tag = "1")]
    pub read_index: u64,
    /// The memory lock blocking this read at the leader
    #[prost(message, optional, tag = "2")]
    pub locked: ::core::option::Option<super::kvrpcpb::LockInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(enumeration = "CmdType", tag = "1")]
    pub cmd_type: i32,
    #[prost(message, optional, tag = "2")]
    pub get: ::core::option::Option<GetRequest>,
    #[prost(message, optional, tag = "4")]
    pub put: ::core::option::Option<PutRequest>,
    #[prost(message, optional, tag = "5")]
    pub delete: ::core::option::Option<DeleteRequest>,
    #[prost(message, optional, tag = "6")]
    pub snap: ::core::option::Option<SnapRequest>,
    #[prost(message, optional, tag = "7")]
    pub prewrite: ::core::option::Option<PrewriteRequest>,
    #[prost(message, optional, tag = "8")]
    pub delete_range: ::core::option::Option<DeleteRangeRequest>,
    #[prost(message, optional, tag = "9")]
    pub ingest_sst: ::core::option::Option<IngestSstRequest>,
    #[prost(message, optional, tag = "10")]
    pub read_index: ::core::option::Option<ReadIndexRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(enumeration = "CmdType", tag = "1")]
    pub cmd_type: i32,
    #[prost(message, optional, tag = "2")]
    pub get: ::core::option::Option<GetResponse>,
    #[prost(message, optional, tag = "4")]
    pub put: ::core::option::Option<PutResponse>,
    #[prost(message, optional, tag = "5")]
    pub delete: ::core::option::Option<DeleteResponse>,
    #[prost(message, optional, tag = "6")]
    pub snap: ::core::option::Option<SnapResponse>,
    #[prost(message, optional, tag = "7")]
    pub prewrite: ::core::option::Option<PrewriteResponse>,
    #[prost(message, optional, tag = "8")]
    pub delte_range: ::core::option::Option<DeleteRangeResponse>,
    #[prost(message, optional, tag = "9")]
    pub ingest_sst: ::core::option::Option<IngestSstResponse>,
    #[prost(message, optional, tag = "10")]
    pub read_index: ::core::option::Option<ReadIndexResponse>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangePeerRequest {
    /// This can be only called in internal RaftStore now.
    #[prost(enumeration = "super::eraftpb::ConfChangeType", tag = "1")]
    pub change_type: i32,
    #[prost(message, optional, tag = "2")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangePeerResponse {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangePeerV2Request {
    #[prost(message, repeated, tag = "1")]
    pub changes: ::prost::alloc::vec::Vec<ChangePeerRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangePeerV2Response {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitRequest {
    /// This can be only called in internal RaftStore now.
    /// The split_key must be in the been splitting region.
    #[prost(bytes = "vec", tag = "1")]
    pub split_key: ::prost::alloc::vec::Vec<u8>,
    /// We split the region into two, first uses the origin
    /// parent region id, and the second uses the new_region_id.
    /// We must guarantee that the new_region_id is global unique.
    #[prost(uint64, tag = "2")]
    pub new_region_id: u64,
    /// The peer ids for the new split region.
    #[prost(uint64, repeated, tag = "3")]
    pub new_peer_ids: ::prost::alloc::vec::Vec<u64>,
    /// If true, right region derive the origin region_id,
    /// left region use new_region_id.
    /// Will be ignored in batch split, use `BatchSplitRequest::right_derive` instead.
    #[deprecated]
    #[prost(bool, tag = "4")]
    pub right_derive: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitResponse {
    #[prost(message, optional, tag = "1")]
    pub left: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "2")]
    pub right: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchSplitRequest {
    #[prost(message, repeated, tag = "1")]
    pub requests: ::prost::alloc::vec::Vec<SplitRequest>,
    /// If true, the last region derive the origin region_id,
    /// other regions use new ids.
    #[prost(bool, tag = "2")]
    pub right_derive: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchSplitResponse {
    #[prost(message, repeated, tag = "1")]
    pub regions: ::prost::alloc::vec::Vec<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactLogRequest {
    #[prost(uint64, tag = "1")]
    pub compact_index: u64,
    #[prost(uint64, tag = "2")]
    pub compact_term: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactLogResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransferLeaderRequest {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, repeated, tag = "2")]
    pub peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransferLeaderResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ComputeHashRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub context: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyHashRequest {
    #[prost(uint64, tag = "1")]
    pub index: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub context: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyHashResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrepareMergeRequest {
    #[prost(uint64, tag = "1")]
    pub min_index: u64,
    #[prost(message, optional, tag = "2")]
    pub target: ::core::option::Option<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrepareMergeResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommitMergeRequest {
    #[prost(message, optional, tag = "1")]
    pub source: ::core::option::Option<super::metapb::Region>,
    #[prost(uint64, tag = "2")]
    pub commit: u64,
    #[prost(message, repeated, tag = "3")]
    pub entries: ::prost::alloc::vec::Vec<super::eraftpb::Entry>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommitMergeResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RollbackMergeRequest {
    #[prost(uint64, tag = "1")]
    pub commit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RollbackMergeResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AdminRequest {
    #[prost(enumeration = "AdminCmdType", tag = "1")]
    pub cmd_type: i32,
    #[prost(message, optional, tag = "2")]
    pub change_peer: ::core::option::Option<ChangePeerRequest>,
    #[deprecated]
    #[prost(message, optional, tag = "3")]
    pub split: ::core::option::Option<SplitRequest>,
    #[prost(message, optional, tag = "4")]
    pub compact_log: ::core::option::Option<CompactLogRequest>,
    #[prost(message, optional, tag = "5")]
    pub transfer_leader: ::core::option::Option<TransferLeaderRequest>,
    #[prost(message, optional, tag = "6")]
    pub verify_hash: ::core::option::Option<VerifyHashRequest>,
    #[prost(message, optional, tag = "7")]
    pub prepare_merge: ::core::option::Option<PrepareMergeRequest>,
    #[prost(message, optional, tag = "8")]
    pub commit_merge: ::core::option::Option<CommitMergeRequest>,
    #[prost(message, optional, tag = "9")]
    pub rollback_merge: ::core::option::Option<RollbackMergeRequest>,
    #[prost(message, optional, tag = "10")]
    pub splits: ::core::option::Option<BatchSplitRequest>,
    #[prost(message, optional, tag = "11")]
    pub change_peer_v2: ::core::option::Option<ChangePeerV2Request>,
    #[prost(message, optional, tag = "12")]
    pub compute_hash: ::core::option::Option<ComputeHashRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AdminResponse {
    #[prost(enumeration = "AdminCmdType", tag = "1")]
    pub cmd_type: i32,
    #[prost(message, optional, tag = "2")]
    pub change_peer: ::core::option::Option<ChangePeerResponse>,
    #[deprecated]
    #[prost(message, optional, tag = "3")]
    pub split: ::core::option::Option<SplitResponse>,
    #[prost(message, optional, tag = "4")]
    pub compact_log: ::core::option::Option<CompactLogResponse>,
    #[prost(message, optional, tag = "5")]
    pub transfer_leader: ::core::option::Option<TransferLeaderResponse>,
    #[prost(message, optional, tag = "6")]
    pub verify_hash: ::core::option::Option<VerifyHashResponse>,
    #[prost(message, optional, tag = "7")]
    pub prepare_merge: ::core::option::Option<PrepareMergeResponse>,
    #[prost(message, optional, tag = "8")]
    pub commit_merge: ::core::option::Option<CommitMergeResponse>,
    #[prost(message, optional, tag = "9")]
    pub rollback_merge: ::core::option::Option<RollbackMergeResponse>,
    #[prost(message, optional, tag = "10")]
    pub splits: ::core::option::Option<BatchSplitResponse>,
    #[prost(message, optional, tag = "11")]
    pub change_peer_v2: ::core::option::Option<ChangePeerV2Response>,
}
/// For get the leader of the region.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionLeaderRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionLeaderResponse {
    #[prost(message, optional, tag = "1")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
}
/// For getting more information of the region.
/// We add some admin operations (ChangePeer, Split...) into the pb job list,
/// then pd server will peek the first one, handle it and then pop it from the job lib.
/// But sometimes, the pd server may crash before popping. When another pd server
/// starts and finds the job is running but not finished, it will first check whether
/// the raft server already has handled this job.
/// E,g, for ChangePeer, if we add Peer10 into region1 and find region1 has already had
/// Peer10, we can think this ChangePeer is finished, and can pop this job from job list
/// directly.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionDetailRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionDetailResponse {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "2")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StatusRequest {
    #[prost(enumeration = "StatusCmdType", tag = "1")]
    pub cmd_type: i32,
    #[prost(message, optional, tag = "2")]
    pub region_leader: ::core::option::Option<RegionLeaderRequest>,
    #[prost(message, optional, tag = "3")]
    pub region_detail: ::core::option::Option<RegionDetailRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StatusResponse {
    #[prost(enumeration = "StatusCmdType", tag = "1")]
    pub cmd_type: i32,
    #[prost(message, optional, tag = "2")]
    pub region_leader: ::core::option::Option<RegionLeaderResponse>,
    #[prost(message, optional, tag = "3")]
    pub region_detail: ::core::option::Option<RegionDetailResponse>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftRequestHeader {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(message, optional, tag = "2")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    /// true for read linearization
    #[prost(bool, tag = "3")]
    pub read_quorum: bool,
    /// 16 bytes, to distinguish request.
    #[prost(bytes = "vec", tag = "4")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "5")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(uint64, tag = "6")]
    pub term: u64,
    #[prost(bool, tag = "7")]
    pub sync_log: bool,
    #[prost(bool, tag = "8")]
    pub replica_read: bool,
    /// Read requests can be responsed directly after the Raft applys to `applied_index`.
    #[prost(uint64, tag = "9")]
    pub applied_index: u64,
    /// Custom flags for this raft request.
    #[prost(uint64, tag = "10")]
    pub flags: u64,
    #[prost(bytes = "vec", tag = "11")]
    pub flag_data: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftResponseHeader {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<super::errorpb::Error>,
    #[prost(bytes = "vec", tag = "2")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub current_term: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftCmdRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RaftRequestHeader>,
    /// We can't enclose normal requests and administrator request
    /// at same time.
    #[prost(message, repeated, tag = "2")]
    pub requests: ::prost::alloc::vec::Vec<Request>,
    #[prost(message, optional, tag = "3")]
    pub admin_request: ::core::option::Option<AdminRequest>,
    #[prost(message, optional, tag = "4")]
    pub status_request: ::core::option::Option<StatusRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftCmdResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RaftResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub responses: ::prost::alloc::vec::Vec<Response>,
    #[prost(message, optional, tag = "3")]
    pub admin_response: ::core::option::Option<AdminResponse>,
    #[prost(message, optional, tag = "4")]
    pub status_response: ::core::option::Option<StatusResponse>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CmdType {
    Invalid = 0,
    Get = 1,
    Put = 3,
    Delete = 4,
    Snap = 5,
    Prewrite = 6,
    DeleteRange = 7,
    IngestSst = 8,
    ReadIndex = 9,
}
impl CmdType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CmdType::Invalid => "Invalid",
            CmdType::Get => "Get",
            CmdType::Put => "Put",
            CmdType::Delete => "Delete",
            CmdType::Snap => "Snap",
            CmdType::Prewrite => "Prewrite",
            CmdType::DeleteRange => "DeleteRange",
            CmdType::IngestSst => "IngestSST",
            CmdType::ReadIndex => "ReadIndex",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Invalid" => Some(Self::Invalid),
            "Get" => Some(Self::Get),
            "Put" => Some(Self::Put),
            "Delete" => Some(Self::Delete),
            "Snap" => Some(Self::Snap),
            "Prewrite" => Some(Self::Prewrite),
            "DeleteRange" => Some(Self::DeleteRange),
            "IngestSST" => Some(Self::IngestSst),
            "ReadIndex" => Some(Self::ReadIndex),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AdminCmdType {
    InvalidAdmin = 0,
    ChangePeer = 1,
    /// Use `BatchSplit` instead.
    Split = 2,
    CompactLog = 3,
    TransferLeader = 4,
    ComputeHash = 5,
    VerifyHash = 6,
    PrepareMerge = 7,
    CommitMerge = 8,
    RollbackMerge = 9,
    BatchSplit = 10,
    ChangePeerV2 = 11,
}
impl AdminCmdType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            AdminCmdType::InvalidAdmin => "InvalidAdmin",
            AdminCmdType::ChangePeer => "ChangePeer",
            AdminCmdType::Split => "Split",
            AdminCmdType::CompactLog => "CompactLog",
            AdminCmdType::TransferLeader => "TransferLeader",
            AdminCmdType::ComputeHash => "ComputeHash",
            AdminCmdType::VerifyHash => "VerifyHash",
            AdminCmdType::PrepareMerge => "PrepareMerge",
            AdminCmdType::CommitMerge => "CommitMerge",
            AdminCmdType::RollbackMerge => "RollbackMerge",
            AdminCmdType::BatchSplit => "BatchSplit",
            AdminCmdType::ChangePeerV2 => "ChangePeerV2",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "InvalidAdmin" => Some(Self::InvalidAdmin),
            "ChangePeer" => Some(Self::ChangePeer),
            "Split" => Some(Self::Split),
            "CompactLog" => Some(Self::CompactLog),
            "TransferLeader" => Some(Self::TransferLeader),
            "ComputeHash" => Some(Self::ComputeHash),
            "VerifyHash" => Some(Self::VerifyHash),
            "PrepareMerge" => Some(Self::PrepareMerge),
            "CommitMerge" => Some(Self::CommitMerge),
            "RollbackMerge" => Some(Self::RollbackMerge),
            "BatchSplit" => Some(Self::BatchSplit),
            "ChangePeerV2" => Some(Self::ChangePeerV2),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StatusCmdType {
    InvalidStatus = 0,
    RegionLeader = 1,
    RegionDetail = 2,
}
impl StatusCmdType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            StatusCmdType::InvalidStatus => "InvalidStatus",
            StatusCmdType::RegionLeader => "RegionLeader",
            StatusCmdType::RegionDetail => "RegionDetail",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "InvalidStatus" => Some(Self::InvalidStatus),
            "RegionLeader" => Some(Self::RegionLeader),
            "RegionDetail" => Some(Self::RegionDetail),
            _ => None,
        }
    }
}
