#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandRequestHeader {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub index: u64,
    #[prost(uint64, tag = "3")]
    pub term: u64,
    /// Flush in-memory data to disk.
    #[prost(bool, tag = "4")]
    pub sync_log: bool,
    /// Destroy the region.
    #[prost(bool, tag = "5")]
    pub destroy: bool,
    /// Additional information for the request.
    #[prost(bytes = "vec", tag = "6")]
    pub context: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<CommandRequestHeader>,
    /// kv put / delete
    #[prost(message, repeated, tag = "2")]
    pub requests: ::prost::alloc::vec::Vec<super::raft_cmdpb::Request>,
    /// region metadata manipulation command.
    #[prost(message, optional, tag = "3")]
    pub admin_request: ::core::option::Option<super::raft_cmdpb::AdminRequest>,
    /// region metadata manipulation result.
    #[prost(message, optional, tag = "4")]
    pub admin_response: ::core::option::Option<super::raft_cmdpb::AdminResponse>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandRequestBatch {
    #[prost(message, repeated, tag = "1")]
    pub requests: ::prost::alloc::vec::Vec<CommandRequest>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandResponseHeader {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    /// Region is destroyed.
    #[prost(bool, tag = "2")]
    pub destroyed: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<CommandResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub apply_state: ::core::option::Option<super::raft_serverpb::RaftApplyState>,
    #[prost(uint64, tag = "3")]
    pub applied_term: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommandResponseBatch {
    #[prost(message, repeated, tag = "1")]
    pub responses: ::prost::alloc::vec::Vec<CommandResponse>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotState {
    #[prost(message, optional, tag = "1")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(message, optional, tag = "2")]
    pub peer: ::core::option::Option<super::metapb::Peer>,
    #[prost(message, optional, tag = "3")]
    pub apply_state: ::core::option::Option<super::raft_serverpb::RaftApplyState>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotData {
    #[prost(string, tag = "1")]
    pub cf: ::prost::alloc::string::String,
    #[prost(uint32, tag = "2")]
    pub checksum: u32,
    #[prost(message, repeated, tag = "3")]
    pub data: ::prost::alloc::vec::Vec<super::raft_serverpb::KeyValue>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotRequest {
    #[prost(oneof = "snapshot_request::Chunk", tags = "1, 2")]
    pub chunk: ::core::option::Option<snapshot_request::Chunk>,
}
/// Nested message and enum types in `SnapshotRequest`.
pub mod snapshot_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Chunk {
        /// The first message for snapshots.
        /// It contains the latest region information after applied snapshot.
        #[prost(message, tag = "1")]
        State(super::SnapshotState),
        /// Following messages are always data.
        #[prost(message, tag = "2")]
        Data(super::SnapshotData),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotDone {}
/// Generated client implementations.
pub mod engine_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct EngineClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl EngineClient<tonic::transport::Channel> {
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
    impl<T> EngineClient<T>
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
        ) -> EngineClient<InterceptedService<T, F>>
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
            EngineClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn apply_command_batch(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::CommandRequestBatch,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::CommandResponseBatch>>,
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
                "/enginepb.Engine/ApplyCommandBatch",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("enginepb.Engine", "ApplyCommandBatch"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn apply_snapshot(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::SnapshotRequest>,
        ) -> std::result::Result<tonic::Response<super::SnapshotDone>, tonic::Status> {
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
                "/enginepb.Engine/ApplySnapshot",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("enginepb.Engine", "ApplySnapshot"));
            self.inner.client_streaming(req, path, codec).await
        }
    }
}
