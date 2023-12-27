/// request to read region meata from a store
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadRegionMetaRequest {
    #[prost(uint64, tag = "1")]
    pub store_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(string, tag = "1")]
    pub msg: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionMeta {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub peer_id: u64,
    #[prost(uint64, tag = "3")]
    pub last_log_term: u64,
    #[prost(uint64, tag = "4")]
    pub last_index: u64,
    #[prost(uint64, tag = "5")]
    pub commit_index: u64,
    #[prost(uint64, tag = "6")]
    pub version: u64,
    /// reserved, it may be used in late phase for peer check
    #[prost(bool, tag = "7")]
    pub tombstone: bool,
    #[prost(bytes = "vec", tag = "8")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
}
/// command to store for recover region
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoverRegionRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    /// force region_id as leader
    #[prost(bool, tag = "2")]
    pub as_leader: bool,
    /// set Peer to tombstoned in late phase
    #[prost(bool, tag = "3")]
    pub tombstone: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoverRegionResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(uint64, tag = "2")]
    pub store_id: u64,
}
/// wait apply to last index
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WaitApplyRequest {
    #[prost(uint64, tag = "1")]
    pub store_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WaitApplyResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
}
/// resolve data by resolved_ts
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResolveKvDataRequest {
    #[prost(uint64, tag = "1")]
    pub resolved_ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResolveKvDataResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(uint64, tag = "2")]
    pub store_id: u64,
    /// reserved for summary of restore
    #[prost(uint64, tag = "3")]
    pub resolved_key_count: u64,
    /// cursor of delete key.commit_ts, reserved for progress of restore
    /// progress is (current_commit_ts - resolved_ts) / (backup_ts - resolved_ts) x 100%
    #[prost(uint64, tag = "4")]
    pub current_commit_ts: u64,
}
/// Generated client implementations.
pub mod recover_data_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// a recovery workflow likes
    ///
    /// 1. BR read ReadRegionMeta to get all region meta
    /// 1. BR send recover region to tikv, e.g assign leader and wait leader apply to last index
    /// 1. BR wait all regions in tikv to apply to last index (no write during the recovery)
    /// 1. BR resolved kv data
    #[derive(Debug, Clone)]
    pub struct RecoverDataClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RecoverDataClient<tonic::transport::Channel> {
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
    impl<T> RecoverDataClient<T>
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
        ) -> RecoverDataClient<InterceptedService<T, F>>
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
            RecoverDataClient::new(InterceptedService::new(inner, interceptor))
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
        /// read region meta to ready region meta
        pub async fn read_region_meta(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadRegionMetaRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::RegionMeta>>,
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
                "/recover_data.RecoverData/ReadRegionMeta",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("recover_data.RecoverData", "ReadRegionMeta"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// execute the recovery command
        pub async fn recover_region(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::RecoverRegionRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::RecoverRegionResponse>,
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
                "/recover_data.RecoverData/RecoverRegion",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("recover_data.RecoverData", "RecoverRegion"));
            self.inner.client_streaming(req, path, codec).await
        }
        /// wait all region apply to last index
        pub async fn wait_apply(
            &mut self,
            request: impl tonic::IntoRequest<super::WaitApplyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::WaitApplyResponse>,
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
                "/recover_data.RecoverData/WaitApply",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("recover_data.RecoverData", "WaitApply"));
            self.inner.unary(req, path, codec).await
        }
        /// execute delete data from kv db
        pub async fn resolve_kv_data(
            &mut self,
            request: impl tonic::IntoRequest<super::ResolveKvDataRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ResolveKvDataResponse>>,
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
                "/recover_data.RecoverData/ResolveKvData",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("recover_data.RecoverData", "ResolveKvData"));
            self.inner.server_streaming(req, path, codec).await
        }
    }
}
