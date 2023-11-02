#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SwitchModeRequest {
    #[prost(string, tag = "1")]
    pub pd_addr: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub request: ::core::option::Option<super::import_sstpb::SwitchModeRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SwitchModeResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpenEngineRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub key_prefix: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpenEngineResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteHead {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Mutation {
    #[prost(enumeration = "mutation::Op", tag = "1")]
    pub op: i32,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
/// Nested message and enum types in `Mutation`.
pub mod mutation {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Op {
        Put = 0,
    }
    impl Op {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Op::Put => "Put",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "Put" => Some(Self::Put),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteBatch {
    #[prost(uint64, tag = "1")]
    pub commit_ts: u64,
    #[prost(message, repeated, tag = "2")]
    pub mutations: ::prost::alloc::vec::Vec<Mutation>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteEngineRequest {
    #[prost(oneof = "write_engine_request::Chunk", tags = "1, 2")]
    pub chunk: ::core::option::Option<write_engine_request::Chunk>,
}
/// Nested message and enum types in `WriteEngineRequest`.
pub mod write_engine_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Chunk {
        #[prost(message, tag = "1")]
        Head(super::WriteHead),
        #[prost(message, tag = "2")]
        Batch(super::WriteBatch),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KvPair {
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteEngineV3Request {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub commit_ts: u64,
    #[prost(message, repeated, tag = "3")]
    pub pairs: ::prost::alloc::vec::Vec<KvPair>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteEngineResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseEngineRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseEngineResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ImportEngineRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, tag = "2")]
    pub pd_addr: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ImportEngineResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CleanupEngineRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CleanupEngineResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactClusterRequest {
    #[prost(string, tag = "1")]
    pub pd_addr: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub request: ::core::option::Option<super::import_sstpb::CompactRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactClusterResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVersionRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVersionResponse {
    #[prost(string, tag = "1")]
    pub version: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub commit: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMetricsRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMetricsResponse {
    #[prost(string, tag = "1")]
    pub prometheus: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    /// This can happen if the client hasn't opened the engine, or the server
    /// restarts while the client is writing or closing. An unclosed engine will
    /// be removed on server restart, so the client should not continue but
    /// restart the previous job in that case.
    #[prost(message, optional, tag = "1")]
    pub engine_not_found: ::core::option::Option<error::EngineNotFound>,
}
/// Nested message and enum types in `Error`.
pub mod error {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct EngineNotFound {
        #[prost(bytes = "vec", tag = "1")]
        pub uuid: ::prost::alloc::vec::Vec<u8>,
    }
}
/// Generated client implementations.
pub mod import_kv_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// ImportKV provides a service to import key-value pairs to TiKV.
    ///
    /// In order to import key-value pairs to TiKV, the user should:
    ///
    /// 1. Open an engine identified by an UUID.
    /// 1. Open write streams to write key-value batches to the opened engine.
    ///   Different streams/clients can write to the same engine concurrently.
    /// 1. Close the engine after all write batches have been finished. An
    ///   engine can only be closed when all write streams are closed. An
    ///   engine can only be closed once, and it can not be opened again
    ///   once it is closed.
    /// 1. Import the data in the engine to the target cluster. Note that
    ///   the import process is not atomic, it requires the data to be
    ///   idempotent on retry. An engine can only be imported after it is
    ///   closed. An engine can be imported multiple times, but can not be
    ///   imported concurrently.
    /// 1. Clean up the engine after it has been imported. Delete all data
    ///   in the engine. An engine can not be cleaned up when it is
    ///   writing or importing.
    #[derive(Debug, Clone)]
    pub struct ImportKvClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ImportKvClient<tonic::transport::Channel> {
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
    impl<T> ImportKvClient<T>
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
        ) -> ImportKvClient<InterceptedService<T, F>>
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
            ImportKvClient::new(InterceptedService::new(inner, interceptor))
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
        /// Switch the target cluster to normal/import mode.
        pub async fn switch_mode(
            &mut self,
            request: impl tonic::IntoRequest<super::SwitchModeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SwitchModeResponse>,
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
                "/import_kvpb.ImportKV/SwitchMode",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "SwitchMode"));
            self.inner.unary(req, path, codec).await
        }
        /// Open an engine.
        pub async fn open_engine(
            &mut self,
            request: impl tonic::IntoRequest<super::OpenEngineRequest>,
        ) -> std::result::Result<
            tonic::Response<super::OpenEngineResponse>,
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
                "/import_kvpb.ImportKV/OpenEngine",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "OpenEngine"));
            self.inner.unary(req, path, codec).await
        }
        /// Open a write stream to the engine.
        pub async fn write_engine(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::WriteEngineRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::WriteEngineResponse>,
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
                "/import_kvpb.ImportKV/WriteEngine",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "WriteEngine"));
            self.inner.client_streaming(req, path, codec).await
        }
        /// Write to engine, single message version
        pub async fn write_engine_v3(
            &mut self,
            request: impl tonic::IntoRequest<super::WriteEngineV3Request>,
        ) -> std::result::Result<
            tonic::Response<super::WriteEngineResponse>,
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
                "/import_kvpb.ImportKV/WriteEngineV3",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "WriteEngineV3"));
            self.inner.unary(req, path, codec).await
        }
        /// Close the engine.
        pub async fn close_engine(
            &mut self,
            request: impl tonic::IntoRequest<super::CloseEngineRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CloseEngineResponse>,
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
                "/import_kvpb.ImportKV/CloseEngine",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "CloseEngine"));
            self.inner.unary(req, path, codec).await
        }
        /// Import the engine to the target cluster.
        pub async fn import_engine(
            &mut self,
            request: impl tonic::IntoRequest<super::ImportEngineRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ImportEngineResponse>,
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
                "/import_kvpb.ImportKV/ImportEngine",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "ImportEngine"));
            self.inner.unary(req, path, codec).await
        }
        /// Clean up the engine.
        pub async fn cleanup_engine(
            &mut self,
            request: impl tonic::IntoRequest<super::CleanupEngineRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CleanupEngineResponse>,
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
                "/import_kvpb.ImportKV/CleanupEngine",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "CleanupEngine"));
            self.inner.unary(req, path, codec).await
        }
        /// Compact the target cluster for better performance.
        pub async fn compact_cluster(
            &mut self,
            request: impl tonic::IntoRequest<super::CompactClusterRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CompactClusterResponse>,
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
                "/import_kvpb.ImportKV/CompactCluster",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "CompactCluster"));
            self.inner.unary(req, path, codec).await
        }
        /// Get current version and commit hash
        pub async fn get_version(
            &mut self,
            request: impl tonic::IntoRequest<super::GetVersionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetVersionResponse>,
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
                "/import_kvpb.ImportKV/GetVersion",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "GetVersion"));
            self.inner.unary(req, path, codec).await
        }
        /// Get importer metrics
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
            let path = http::uri::PathAndQuery::from_static(
                "/import_kvpb.ImportKV/GetMetrics",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_kvpb.ImportKV", "GetMetrics"));
            self.inner.unary(req, path, codec).await
        }
    }
}
