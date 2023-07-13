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
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsAliveRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsAliveResponse {
    #[prost(bool, tag = "1")]
    pub available: bool,
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
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub msg: ::prost::alloc::string::String,
}
