#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct S3LockResult {
    #[prost(oneof = "s3_lock_result::Error", tags = "1, 2, 3")]
    pub error: ::core::option::Option<s3_lock_result::Error>,
}
/// Nested message and enum types in `S3LockResult`.
pub mod s3_lock_result {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Error {
        #[prost(message, tag = "1")]
        Success(super::Success),
        #[prost(message, tag = "2")]
        NotOwner(super::NotOwner),
        #[prost(message, tag = "3")]
        Conflict(super::Conflict),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Success {}
/// Error caused by S3GC owner changed
/// client should retry
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NotOwner {}
/// Error caused by concurrency conflict,
/// request cancel
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Conflict {
    #[prost(string, tag = "1")]
    pub reason: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TryAddLockRequest {
    /// The data file key to add lock
    #[prost(bytes = "vec", tag = "1")]
    pub data_file_key: ::prost::alloc::vec::Vec<u8>,
    /// The lock store id
    #[prost(uint64, tag = "3")]
    pub lock_store_id: u64,
    /// The upload sequence number of lock store
    #[prost(uint64, tag = "4")]
    pub lock_seq: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TryAddLockResponse {
    #[prost(message, optional, tag = "1")]
    pub result: ::core::option::Option<S3LockResult>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TryMarkDeleteRequest {
    /// The data file key to be marked as deleted
    #[prost(bytes = "vec", tag = "1")]
    pub data_file_key: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TryMarkDeleteResponse {
    #[prost(message, optional, tag = "1")]
    pub result: ::core::option::Option<S3LockResult>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDisaggConfigRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisaggS3Config {
    #[prost(string, tag = "1")]
    pub bucket: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub root: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub endpoint: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDisaggConfigResponse {
    #[prost(message, optional, tag = "1")]
    pub s3_config: ::core::option::Option<DisaggS3Config>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisaggTaskMeta {
    /// start ts of a query
    #[prost(uint64, tag = "1")]
    pub start_ts: u64,
    /// gather_id + query_ts + server_id + local_query_id to represent a global unique query.
    ///
    /// used to distinguish different gathers in the mpp query
    #[prost(int64, tag = "9")]
    pub gather_id: i64,
    /// timestamp when start to execute query, used for TiFlash miniTSO schedule.
    #[prost(uint64, tag = "2")]
    pub query_ts: u64,
    /// TiDB server id
    #[prost(uint64, tag = "3")]
    pub server_id: u64,
    /// unique local query_id if tidb don't restart.
    #[prost(uint64, tag = "4")]
    pub local_query_id: u64,
    /// if task id is -1 , it indicates a tidb task.
    #[prost(int64, tag = "5")]
    pub task_id: i64,
    /// the exectuor id
    #[prost(string, tag = "6")]
    pub executor_id: ::prost::alloc::string::String,
    /// keyspace id of the request
    #[prost(uint32, tag = "7")]
    pub keyspace_id: u32,
    /// API version of the request
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "8")]
    pub api_version: i32,
    /// This is the session id between a client and tidb
    #[prost(uint64, tag = "10")]
    pub connection_id: u64,
    /// This is the session alias between a client and tidb
    #[prost(string, tag = "11")]
    pub connection_alias: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisaggReadError {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub msg: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EstablishDisaggTaskError {
    #[prost(oneof = "establish_disagg_task_error::Errors", tags = "1, 2, 99")]
    pub errors: ::core::option::Option<establish_disagg_task_error::Errors>,
}
/// Nested message and enum types in `EstablishDisaggTaskError`.
pub mod establish_disagg_task_error {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Errors {
        #[prost(message, tag = "1")]
        ErrorRegion(super::ErrorRegion),
        #[prost(message, tag = "2")]
        ErrorLocked(super::ErrorLocked),
        #[prost(message, tag = "99")]
        ErrorOther(super::ErrorOther),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ErrorRegion {
    #[prost(string, tag = "1")]
    pub msg: ::prost::alloc::string::String,
    /// The read node needs to update its region cache about these regions.
    #[prost(uint64, repeated, tag = "2")]
    pub region_ids: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ErrorLocked {
    #[prost(string, tag = "1")]
    pub msg: ::prost::alloc::string::String,
    /// The read node needs to resolve these locks.
    #[prost(message, repeated, tag = "2")]
    pub locked: ::prost::alloc::vec::Vec<super::kvrpcpb::LockInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ErrorOther {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub msg: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EstablishDisaggTaskRequest {
    #[prost(message, optional, tag = "1")]
    pub meta: ::core::option::Option<DisaggTaskMeta>,
    /// target address of this task.
    #[prost(string, tag = "2")]
    pub address: ::prost::alloc::string::String,
    /// The write node needs to ensure that subsequent
    /// FetchDisaggPagesRequest can be processed within timeout_s.
    /// unit: seconds
    #[prost(int64, tag = "3")]
    pub timeout_s: i64,
    /// The key ranges, Region meta that read node need to execute TableScan
    #[prost(message, repeated, tag = "4")]
    pub regions: ::prost::alloc::vec::Vec<super::coprocessor::RegionInfo>,
    #[prost(int64, tag = "5")]
    pub schema_ver: i64,
    /// Used for PartitionTableScan
    #[prost(message, repeated, tag = "6")]
    pub table_regions: ::prost::alloc::vec::Vec<super::coprocessor::TableRegions>,
    /// The encoded TableScan/PartitionTableScan + Selection.
    #[prost(bytes = "vec", tag = "7")]
    pub encoded_plan: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EstablishDisaggTaskResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<EstablishDisaggTaskError>,
    /// Write node maintains a snapshot with a lease time.
    /// Read node should read the delta pages
    /// (ColumnFileInMemory and ColumnFileTiny)
    /// along with this store_id and snapshot_id.
    ///
    /// metapb.Store.id
    #[prost(uint64, tag = "3")]
    pub store_id: u64,
    #[prost(message, optional, tag = "4")]
    pub snapshot_id: ::core::option::Option<DisaggTaskMeta>,
    /// Serialized disaggregated tasks (per physical table)
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub tables: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelDisaggTaskRequest {
    #[prost(message, optional, tag = "1")]
    pub meta: ::core::option::Option<DisaggTaskMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelDisaggTaskResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchDisaggPagesRequest {
    /// The snapshot id to fetch pages
    #[prost(message, optional, tag = "1")]
    pub snapshot_id: ::core::option::Option<DisaggTaskMeta>,
    #[prost(int64, tag = "2")]
    pub table_id: i64,
    #[prost(uint64, tag = "3")]
    pub segment_id: u64,
    /// It must be a subset of the delta pages ids returned
    /// in EstablishDisaggTaskResponse.segments
    #[prost(uint64, repeated, tag = "4")]
    pub page_ids: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PagesPacket {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<DisaggReadError>,
    /// Serialized column file data
    ///
    /// * ColumnFilePersisted alone with its schema, page data, field offsets
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub pages: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    /// * ColumnFileInMemory alone with its serialized block
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub chunks: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    /// Return tipb.SelectResponse.execution_summaries in the
    /// last packet
    #[prost(bytes = "vec", repeated, tag = "4")]
    pub summaries: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
