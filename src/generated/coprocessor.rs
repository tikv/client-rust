/// \[start, end)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyRange {
    #[prost(bytes = "vec", tag = "1")]
    pub start: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub end: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(message, optional, tag = "1")]
    pub context: ::core::option::Option<super::kvrpcpb::Context>,
    #[prost(int64, tag = "2")]
    pub tp: i64,
    #[prost(bytes = "vec", tag = "3")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "7")]
    pub start_ts: u64,
    #[prost(message, repeated, tag = "4")]
    pub ranges: ::prost::alloc::vec::Vec<KeyRange>,
    /// If cache is enabled, TiKV returns cache hit instead of data if
    /// its last version matches this `cache_if_match_version`.
    #[prost(bool, tag = "5")]
    pub is_cache_enabled: bool,
    #[prost(uint64, tag = "6")]
    pub cache_if_match_version: u64,
    /// Any schema-ful storage to validate schema correctness if necessary.
    #[prost(int64, tag = "8")]
    pub schema_ver: i64,
    #[prost(bool, tag = "9")]
    pub is_trace_enabled: bool,
    /// paging_size is 0 when it's disabled, otherwise, it should be a positive number.
    #[prost(uint64, tag = "10")]
    pub paging_size: u64,
    /// tasks stores the batched coprocessor tasks sent to the same tikv store.
    #[prost(message, repeated, tag = "11")]
    pub tasks: ::prost::alloc::vec::Vec<StoreBatchTask>,
    /// This is the session id between a client and tidb
    #[prost(uint64, tag = "12")]
    pub connection_id: u64,
    /// This is the session alias between a client and tidb
    #[prost(string, tag = "13")]
    pub connection_alias: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub region_error: ::core::option::Option<super::errorpb::Error>,
    #[prost(message, optional, tag = "3")]
    pub locked: ::core::option::Option<super::kvrpcpb::LockInfo>,
    #[prost(string, tag = "4")]
    pub other_error: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "5")]
    pub range: ::core::option::Option<KeyRange>,
    /// This field is always filled for compatibility consideration. However
    /// newer TiDB should respect `exec_details_v2` field instead.
    #[prost(message, optional, tag = "6")]
    pub exec_details: ::core::option::Option<super::kvrpcpb::ExecDetails>,
    /// This field is provided in later versions, containing more detailed
    /// information.
    #[prost(message, optional, tag = "11")]
    pub exec_details_v2: ::core::option::Option<super::kvrpcpb::ExecDetailsV2>,
    #[prost(bool, tag = "7")]
    pub is_cache_hit: bool,
    #[prost(uint64, tag = "8")]
    pub cache_last_version: u64,
    #[prost(bool, tag = "9")]
    pub can_be_cached: bool,
    /// Contains the latest buckets version of the region.
    /// Clients should query PD to update buckets in cache if its is stale.
    #[prost(uint64, tag = "12")]
    pub latest_buckets_version: u64,
    /// StoreBatchTaskResponse is the collection of batch task responses.
    #[prost(message, repeated, tag = "13")]
    pub batch_responses: ::prost::alloc::vec::Vec<StoreBatchTaskResponse>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionInfo {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(message, optional, tag = "2")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(message, repeated, tag = "3")]
    pub ranges: ::prost::alloc::vec::Vec<KeyRange>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TableRegions {
    #[prost(int64, tag = "1")]
    pub physical_table_id: i64,
    #[prost(message, repeated, tag = "2")]
    pub regions: ::prost::alloc::vec::Vec<RegionInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchRequest {
    #[prost(message, optional, tag = "1")]
    pub context: ::core::option::Option<super::kvrpcpb::Context>,
    #[prost(int64, tag = "2")]
    pub tp: i64,
    #[prost(bytes = "vec", tag = "3")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, repeated, tag = "4")]
    pub regions: ::prost::alloc::vec::Vec<RegionInfo>,
    #[prost(uint64, tag = "5")]
    pub start_ts: u64,
    /// Any schema-ful storage to validate schema correctness if necessary.
    #[prost(int64, tag = "6")]
    pub schema_ver: i64,
    /// Used for partition table scan
    #[prost(message, repeated, tag = "7")]
    pub table_regions: ::prost::alloc::vec::Vec<TableRegions>,
    #[prost(string, tag = "8")]
    pub log_id: ::prost::alloc::string::String,
    /// This is the session id between a client and tidb
    #[prost(uint64, tag = "9")]
    pub connection_id: u64,
    /// This is the session alias between a client and tidb
    #[prost(string, tag = "10")]
    pub connection_alias: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, tag = "2")]
    pub other_error: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub exec_details: ::core::option::Option<super::kvrpcpb::ExecDetails>,
    #[prost(message, repeated, tag = "4")]
    pub retry_regions: ::prost::alloc::vec::Vec<super::metapb::Region>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreBatchTask {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(message, optional, tag = "2")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(message, optional, tag = "3")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, repeated, tag = "4")]
    pub ranges: ::prost::alloc::vec::Vec<KeyRange>,
    #[prost(uint64, tag = "5")]
    pub task_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreBatchTaskResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub region_error: ::core::option::Option<super::errorpb::Error>,
    #[prost(message, optional, tag = "3")]
    pub locked: ::core::option::Option<super::kvrpcpb::LockInfo>,
    #[prost(string, tag = "4")]
    pub other_error: ::prost::alloc::string::String,
    #[prost(uint64, tag = "5")]
    pub task_id: u64,
    #[prost(message, optional, tag = "6")]
    pub exec_details_v2: ::core::option::Option<super::kvrpcpb::ExecDetailsV2>,
}
