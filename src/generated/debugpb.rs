#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRequest {
    #[prost(enumeration = "Db", tag = "1")]
    pub db: i32,
    #[prost(string, tag = "2")]
    pub cf: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "3")]
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
pub struct RaftLogRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub log_index: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftLogResponse {
    #[prost(message, optional, tag = "1")]
    pub entry: ::core::option::Option<super::eraftpb::Entry>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionInfoRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionInfoResponse {
    #[prost(message, optional, tag = "1")]
    pub raft_local_state: ::core::option::Option<super::raft_serverpb::RaftLocalState>,
    #[prost(message, optional, tag = "2")]
    pub raft_apply_state: ::core::option::Option<super::raft_serverpb::RaftApplyState>,
    #[prost(message, optional, tag = "3")]
    pub region_local_state: ::core::option::Option<
        super::raft_serverpb::RegionLocalState,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionSizeRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(string, repeated, tag = "2")]
    pub cfs: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionSizeResponse {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<region_size_response::Entry>,
}
/// Nested message and enum types in `RegionSizeResponse`.
pub mod region_size_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Entry {
        #[prost(string, tag = "1")]
        pub cf: ::prost::alloc::string::String,
        #[prost(uint64, tag = "2")]
        pub size: u64,
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScanMvccRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub from_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub to_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub limit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScanMvccResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub info: ::core::option::Option<super::kvrpcpb::MvccInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactRequest {
    #[prost(enumeration = "Db", tag = "1")]
    pub db: i32,
    #[prost(string, tag = "2")]
    pub cf: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "3")]
    pub from_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub to_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint32, tag = "5")]
    pub threads: u32,
    #[prost(enumeration = "BottommostLevelCompaction", tag = "6")]
    pub bottommost_level_compaction: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InjectFailPointRequest {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub actions: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InjectFailPointResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoverFailPointRequest {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoverFailPointResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListFailPointsRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListFailPointsResponse {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<list_fail_points_response::Entry>,
}
/// Nested message and enum types in `ListFailPointsResponse`.
pub mod list_fail_points_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Entry {
        #[prost(string, tag = "1")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub actions: ::prost::alloc::string::String,
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMetricsRequest {
    #[prost(bool, tag = "1")]
    pub all: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMetricsResponse {
    #[prost(string, tag = "1")]
    pub prometheus: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub rocksdb_kv: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub rocksdb_raft: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub jemalloc: ::prost::alloc::string::String,
    #[prost(uint64, tag = "5")]
    pub store_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionConsistencyCheckRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionConsistencyCheckResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ModifyTikvConfigRequest {
    #[prost(enumeration = "Module", tag = "1")]
    pub module: i32,
    #[prost(string, tag = "2")]
    pub config_name: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub config_value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ModifyTikvConfigResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Property {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRegionPropertiesRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRegionPropertiesResponse {
    #[prost(message, repeated, tag = "1")]
    pub props: ::prost::alloc::vec::Vec<Property>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStoreInfoRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStoreInfoResponse {
    #[prost(uint64, tag = "1")]
    pub store_id: u64,
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "2")]
    pub api_version: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClusterInfoRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClusterInfoResponse {
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllRegionsInStoreRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllRegionsInStoreResponse {
    #[prost(uint64, repeated, tag = "1")]
    pub regions: ::prost::alloc::vec::Vec<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Db {
    Invalid = 0,
    Kv = 1,
    Raft = 2,
}
impl Db {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Db::Invalid => "INVALID",
            Db::Kv => "KV",
            Db::Raft => "RAFT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "INVALID" => Some(Self::Invalid),
            "KV" => Some(Self::Kv),
            "RAFT" => Some(Self::Raft),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Module {
    Unused = 0,
    Kvdb = 1,
    Raftdb = 2,
    Readpool = 3,
    Server = 4,
    Storage = 5,
    Pd = 6,
    Metric = 7,
    Coprocessor = 8,
    Security = 9,
    Import = 10,
}
impl Module {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Module::Unused => "UNUSED",
            Module::Kvdb => "KVDB",
            Module::Raftdb => "RAFTDB",
            Module::Readpool => "READPOOL",
            Module::Server => "SERVER",
            Module::Storage => "STORAGE",
            Module::Pd => "PD",
            Module::Metric => "METRIC",
            Module::Coprocessor => "COPROCESSOR",
            Module::Security => "SECURITY",
            Module::Import => "IMPORT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNUSED" => Some(Self::Unused),
            "KVDB" => Some(Self::Kvdb),
            "RAFTDB" => Some(Self::Raftdb),
            "READPOOL" => Some(Self::Readpool),
            "SERVER" => Some(Self::Server),
            "STORAGE" => Some(Self::Storage),
            "PD" => Some(Self::Pd),
            "METRIC" => Some(Self::Metric),
            "COPROCESSOR" => Some(Self::Coprocessor),
            "SECURITY" => Some(Self::Security),
            "IMPORT" => Some(Self::Import),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum BottommostLevelCompaction {
    /// Skip bottommost level compaction
    Skip = 0,
    /// Force bottommost level compaction
    Force = 1,
    /// Compact bottommost level if there is a compaction filter.
    IfHaveCompactionFilter = 2,
}
impl BottommostLevelCompaction {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            BottommostLevelCompaction::Skip => "Skip",
            BottommostLevelCompaction::Force => "Force",
            BottommostLevelCompaction::IfHaveCompactionFilter => "IfHaveCompactionFilter",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Skip" => Some(Self::Skip),
            "Force" => Some(Self::Force),
            "IfHaveCompactionFilter" => Some(Self::IfHaveCompactionFilter),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod debug_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Debug service for TiKV.
    ///
    /// Errors are defined as follow:
    ///
    /// * OK: Okay, we are good!
    /// * UNKNOWN: For unknown error.
    /// * INVALID_ARGUMENT: Something goes wrong within requests.
    /// * NOT_FOUND: It is key or region not found, it's based on context, detailed
    ///  reason can be found in grpc message.
    ///  Note: It bypasses raft layer.
    #[derive(Debug, Clone)]
    pub struct DebugClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl DebugClient<tonic::transport::Channel> {
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
    impl<T> DebugClient<T>
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
        ) -> DebugClient<InterceptedService<T, F>>
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
            DebugClient::new(InterceptedService::new(inner, interceptor))
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
        /// Read a value arbitrarily for a key.
        /// Note: Server uses key directly w/o any encoding.
        pub async fn get(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRequest>,
        ) -> std::result::Result<tonic::Response<super::GetResponse>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/Get");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "Get"));
            self.inner.unary(req, path, codec).await
        }
        /// Read raft info.
        pub async fn raft_log(
            &mut self,
            request: impl tonic::IntoRequest<super::RaftLogRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RaftLogResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/RaftLog");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "RaftLog"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn region_info(
            &mut self,
            request: impl tonic::IntoRequest<super::RegionInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegionInfoResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/RegionInfo");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "RegionInfo"));
            self.inner.unary(req, path, codec).await
        }
        /// Calculate size of a region.
        /// Note: DO NOT CALL IT IN PRODUCTION, it's really expensive.
        pub async fn region_size(
            &mut self,
            request: impl tonic::IntoRequest<super::RegionSizeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegionSizeResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/RegionSize");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "RegionSize"));
            self.inner.unary(req, path, codec).await
        }
        /// Scan a specific range.
        /// Note: DO NOT CALL IT IN PRODUCTION, it's really expensive.
        /// Server uses keys directly w/o any encoding.
        pub async fn scan_mvcc(
            &mut self,
            request: impl tonic::IntoRequest<super::ScanMvccRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ScanMvccResponse>>,
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/ScanMvcc");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "ScanMvcc"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// Compact a column family in a specified range.
        /// Note: Server uses keys directly w/o any encoding.
        pub async fn compact(
            &mut self,
            request: impl tonic::IntoRequest<super::CompactRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CompactResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/Compact");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "Compact"));
            self.inner.unary(req, path, codec).await
        }
        /// Inject a fail point. Currently, it's only used in tests.
        /// Note: DO NOT CALL IT IN PRODUCTION.
        pub async fn inject_fail_point(
            &mut self,
            request: impl tonic::IntoRequest<super::InjectFailPointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::InjectFailPointResponse>,
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
                "/debugpb.Debug/InjectFailPoint",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "InjectFailPoint"));
            self.inner.unary(req, path, codec).await
        }
        /// Recover from a fail point.
        pub async fn recover_fail_point(
            &mut self,
            request: impl tonic::IntoRequest<super::RecoverFailPointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RecoverFailPointResponse>,
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
                "/debugpb.Debug/RecoverFailPoint",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "RecoverFailPoint"));
            self.inner.unary(req, path, codec).await
        }
        /// List all fail points.
        pub async fn list_fail_points(
            &mut self,
            request: impl tonic::IntoRequest<super::ListFailPointsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListFailPointsResponse>,
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
                "/debugpb.Debug/ListFailPoints",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "ListFailPoints"));
            self.inner.unary(req, path, codec).await
        }
        /// Get Metrics
        pub async fn get_metrics(
            &mut self,
            request: impl tonic::IntoRequest<super::GetMetricsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMetricsResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/debugpb.Debug/GetMetrics");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("debugpb.Debug", "GetMetrics"));
            self.inner.unary(req, path, codec).await
        }
        /// Do a consistent check for a region.
        pub async fn check_region_consistency(
            &mut self,
            request: impl tonic::IntoRequest<super::RegionConsistencyCheckRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegionConsistencyCheckResponse>,
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
                "/debugpb.Debug/CheckRegionConsistency",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "CheckRegionConsistency"));
            self.inner.unary(req, path, codec).await
        }
        /// dynamically modify tikv's config
        pub async fn modify_tikv_config(
            &mut self,
            request: impl tonic::IntoRequest<super::ModifyTikvConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ModifyTikvConfigResponse>,
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
                "/debugpb.Debug/ModifyTikvConfig",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "ModifyTikvConfig"));
            self.inner.unary(req, path, codec).await
        }
        /// Get region properties
        pub async fn get_region_properties(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRegionPropertiesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRegionPropertiesResponse>,
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
                "/debugpb.Debug/GetRegionProperties",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "GetRegionProperties"));
            self.inner.unary(req, path, codec).await
        }
        /// Get store ID
        pub async fn get_store_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetStoreInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetStoreInfoResponse>,
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
                "/debugpb.Debug/GetStoreInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "GetStoreInfo"));
            self.inner.unary(req, path, codec).await
        }
        /// Get cluster ID
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
            let path = http::uri::PathAndQuery::from_static(
                "/debugpb.Debug/GetClusterInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "GetClusterInfo"));
            self.inner.unary(req, path, codec).await
        }
        /// Get all region IDs in the store
        pub async fn get_all_regions_in_store(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAllRegionsInStoreRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetAllRegionsInStoreResponse>,
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
                "/debugpb.Debug/GetAllRegionsInStore",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("debugpb.Debug", "GetAllRegionsInStore"));
            self.inner.unary(req, path, codec).await
        }
    }
}
