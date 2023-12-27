/// TaskMeta contains meta of a mpp plan, including query's ts and task address.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskMeta {
    /// start ts of a query
    #[prost(uint64, tag = "1")]
    pub start_ts: u64,
    /// if task id is -1 , it indicates a tidb task.
    #[prost(int64, tag = "2")]
    pub task_id: i64,
    /// Only used for hash partition
    #[prost(int64, tag = "3")]
    pub partition_id: i64,
    /// target address of this task.
    #[prost(string, tag = "4")]
    pub address: ::prost::alloc::string::String,
    /// used to distinguish different gathers in the mpp query.
    #[prost(uint64, tag = "5")]
    pub gather_id: u64,
    /// timestamp when start to execute query, used for TiFlash miniTSO schedule.
    #[prost(uint64, tag = "6")]
    pub query_ts: u64,
    /// unique local query_id if tidb don't restart. So we can use gather_id + query_ts + local_query_id + server_id to represent a global unique query.
    #[prost(uint64, tag = "7")]
    pub local_query_id: u64,
    /// TiDB server id
    #[prost(uint64, tag = "8")]
    pub server_id: u64,
    /// mpp version
    #[prost(int64, tag = "9")]
    pub mpp_version: i64,
    /// keyspace id of the request
    #[prost(uint32, tag = "10")]
    pub keyspace_id: u32,
    /// coordinator_address of this query
    #[prost(string, tag = "11")]
    pub coordinator_address: ::prost::alloc::string::String,
    /// Only when coordinator_address is not empty, this flag can be true. When set to true, TiFlash only report execution summary through ReportMPPTaskStatus service, don't include summaries in MppDataPacket
    #[prost(bool, tag = "12")]
    pub report_execution_summary: bool,
    /// API version of the request
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "16")]
    pub api_version: i32,
    #[prost(string, tag = "17")]
    pub resource_group_name: ::prost::alloc::string::String,
    /// This is the session id between a client and tidb
    #[prost(uint64, tag = "18")]
    pub connection_id: u64,
    /// This is the session alias between a client and tidb
    #[prost(string, tag = "19")]
    pub connection_alias: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsAliveRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsAliveResponse {
    #[prost(bool, tag = "1")]
    pub available: bool,
    #[prost(int64, tag = "2")]
    pub mpp_version: i64,
}
/// Dipsatch the task request to different tiflash servers.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DispatchTaskRequest {
    #[prost(message, optional, tag = "1")]
    pub meta: ::core::option::Option<TaskMeta>,
    #[prost(bytes = "vec", tag = "2")]
    pub encoded_plan: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "3")]
    pub timeout: i64,
    #[prost(message, repeated, tag = "4")]
    pub regions: ::prost::alloc::vec::Vec<super::coprocessor::RegionInfo>,
    /// If this task contains table scan, we still need their region info.
    #[prost(int64, tag = "5")]
    pub schema_ver: i64,
    /// Used for partition table scan
    #[prost(message, repeated, tag = "6")]
    pub table_regions: ::prost::alloc::vec::Vec<super::coprocessor::TableRegions>,
}
/// Get response of DispatchTaskRequest.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DispatchTaskResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, repeated, tag = "2")]
    pub retry_regions: ::prost::alloc::vec::Vec<super::metapb::Region>,
}
/// CancelTaskRequest closes the execution of a task.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelTaskRequest {
    #[prost(message, optional, tag = "1")]
    pub meta: ::core::option::Option<TaskMeta>,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelTaskResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
}
/// ReportTaskStatus reports the execution status of a task.
/// when TiFlash reports status to TiDB, ReportTaskStatusRequest serialize tipb.TiFlashExecutionInfo into data;
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportTaskStatusRequest {
    #[prost(message, optional, tag = "1")]
    pub meta: ::core::option::Option<TaskMeta>,
    #[prost(bytes = "vec", tag = "2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReportTaskStatusResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
}
/// build connection between different tasks. Data is sent by the tasks that are closer to the data sources.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EstablishMppConnectionRequest {
    /// node closer to the source
    #[prost(message, optional, tag = "1")]
    pub sender_meta: ::core::option::Option<TaskMeta>,
    /// node closer to the tidb mpp gather.
    #[prost(message, optional, tag = "2")]
    pub receiver_meta: ::core::option::Option<TaskMeta>,
}
/// when TiFlash sends data to TiDB, Data packets wrap tipb.SelectResponse, i.e., serialize tipb.SelectResponse into data;
/// when TiFlash sends data to TiFlash, data blocks are serialized into chunks, and the execution_summaries in tipb.SelectResponse are serialized into data only for the last packet.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MppDataPacket {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<Error>,
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub chunks: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint64, repeated, tag = "4")]
    pub stream_ids: ::prost::alloc::vec::Vec<u64>,
    /// version of data packet format
    #[prost(int64, tag = "5")]
    pub version: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub msg: ::prost::alloc::string::String,
    #[prost(int64, tag = "3")]
    pub mpp_version: i64,
}
