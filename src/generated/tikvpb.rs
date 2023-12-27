#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchCommandsRequest {
    #[prost(message, repeated, tag = "1")]
    pub requests: ::prost::alloc::vec::Vec<batch_commands_request::Request>,
    #[prost(uint64, repeated, tag = "2")]
    pub request_ids: ::prost::alloc::vec::Vec<u64>,
}
/// Nested message and enum types in `BatchCommandsRequest`.
pub mod batch_commands_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Request {
        #[prost(
            oneof = "request::Cmd",
            tags = "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 33, 34, 35, 36, 255"
        )]
        pub cmd: ::core::option::Option<request::Cmd>,
    }
    /// Nested message and enum types in `Request`.
    pub mod request {
        #[allow(clippy::derive_partial_eq_without_eq)]
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum Cmd {
            #[prost(message, tag = "1")]
            Get(super::super::super::kvrpcpb::GetRequest),
            #[prost(message, tag = "2")]
            Scan(super::super::super::kvrpcpb::ScanRequest),
            #[prost(message, tag = "3")]
            Prewrite(super::super::super::kvrpcpb::PrewriteRequest),
            #[prost(message, tag = "4")]
            Commit(super::super::super::kvrpcpb::CommitRequest),
            #[prost(message, tag = "5")]
            Import(super::super::super::kvrpcpb::ImportRequest),
            #[prost(message, tag = "6")]
            Cleanup(super::super::super::kvrpcpb::CleanupRequest),
            #[prost(message, tag = "7")]
            BatchGet(super::super::super::kvrpcpb::BatchGetRequest),
            #[prost(message, tag = "8")]
            BatchRollback(super::super::super::kvrpcpb::BatchRollbackRequest),
            #[prost(message, tag = "9")]
            ScanLock(super::super::super::kvrpcpb::ScanLockRequest),
            #[prost(message, tag = "10")]
            ResolveLock(super::super::super::kvrpcpb::ResolveLockRequest),
            #[prost(message, tag = "11")]
            Gc(super::super::super::kvrpcpb::GcRequest),
            #[prost(message, tag = "12")]
            DeleteRange(super::super::super::kvrpcpb::DeleteRangeRequest),
            #[prost(message, tag = "13")]
            RawGet(super::super::super::kvrpcpb::RawGetRequest),
            #[prost(message, tag = "14")]
            RawBatchGet(super::super::super::kvrpcpb::RawBatchGetRequest),
            #[prost(message, tag = "15")]
            RawPut(super::super::super::kvrpcpb::RawPutRequest),
            #[prost(message, tag = "16")]
            RawBatchPut(super::super::super::kvrpcpb::RawBatchPutRequest),
            #[prost(message, tag = "17")]
            RawDelete(super::super::super::kvrpcpb::RawDeleteRequest),
            #[prost(message, tag = "18")]
            RawBatchDelete(super::super::super::kvrpcpb::RawBatchDeleteRequest),
            #[prost(message, tag = "19")]
            RawScan(super::super::super::kvrpcpb::RawScanRequest),
            #[prost(message, tag = "20")]
            RawDeleteRange(super::super::super::kvrpcpb::RawDeleteRangeRequest),
            #[prost(message, tag = "21")]
            RawBatchScan(super::super::super::kvrpcpb::RawBatchScanRequest),
            #[prost(message, tag = "22")]
            Coprocessor(super::super::super::coprocessor::Request),
            #[prost(message, tag = "23")]
            PessimisticLock(super::super::super::kvrpcpb::PessimisticLockRequest),
            #[prost(message, tag = "24")]
            PessimisticRollback(
                super::super::super::kvrpcpb::PessimisticRollbackRequest,
            ),
            #[prost(message, tag = "25")]
            CheckTxnStatus(super::super::super::kvrpcpb::CheckTxnStatusRequest),
            #[prost(message, tag = "26")]
            TxnHeartBeat(super::super::super::kvrpcpb::TxnHeartBeatRequest),
            #[prost(message, tag = "33")]
            CheckSecondaryLocks(
                super::super::super::kvrpcpb::CheckSecondaryLocksRequest,
            ),
            #[prost(message, tag = "34")]
            RawCoprocessor(super::super::super::kvrpcpb::RawCoprocessorRequest),
            #[prost(message, tag = "35")]
            FlashbackToVersion(super::super::super::kvrpcpb::FlashbackToVersionRequest),
            #[prost(message, tag = "36")]
            PrepareFlashbackToVersion(
                super::super::super::kvrpcpb::PrepareFlashbackToVersionRequest,
            ),
            /// For some test cases.
            #[prost(message, tag = "255")]
            Empty(super::super::BatchCommandsEmptyRequest),
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchCommandsResponse {
    #[prost(message, repeated, tag = "1")]
    pub responses: ::prost::alloc::vec::Vec<batch_commands_response::Response>,
    #[prost(uint64, repeated, tag = "2")]
    pub request_ids: ::prost::alloc::vec::Vec<u64>,
    /// 280 means TiKV gRPC cpu usage is 280%.
    #[prost(uint64, tag = "3")]
    pub transport_layer_load: u64,
}
/// Nested message and enum types in `BatchCommandsResponse`.
pub mod batch_commands_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Response {
        #[prost(
            oneof = "response::Cmd",
            tags = "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 33, 34, 35, 36, 255"
        )]
        pub cmd: ::core::option::Option<response::Cmd>,
    }
    /// Nested message and enum types in `Response`.
    pub mod response {
        #[allow(clippy::derive_partial_eq_without_eq)]
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum Cmd {
            #[prost(message, tag = "1")]
            Get(super::super::super::kvrpcpb::GetResponse),
            #[prost(message, tag = "2")]
            Scan(super::super::super::kvrpcpb::ScanResponse),
            #[prost(message, tag = "3")]
            Prewrite(super::super::super::kvrpcpb::PrewriteResponse),
            #[prost(message, tag = "4")]
            Commit(super::super::super::kvrpcpb::CommitResponse),
            #[prost(message, tag = "5")]
            Import(super::super::super::kvrpcpb::ImportResponse),
            #[prost(message, tag = "6")]
            Cleanup(super::super::super::kvrpcpb::CleanupResponse),
            #[prost(message, tag = "7")]
            BatchGet(super::super::super::kvrpcpb::BatchGetResponse),
            #[prost(message, tag = "8")]
            BatchRollback(super::super::super::kvrpcpb::BatchRollbackResponse),
            #[prost(message, tag = "9")]
            ScanLock(super::super::super::kvrpcpb::ScanLockResponse),
            #[prost(message, tag = "10")]
            ResolveLock(super::super::super::kvrpcpb::ResolveLockResponse),
            #[prost(message, tag = "11")]
            Gc(super::super::super::kvrpcpb::GcResponse),
            #[prost(message, tag = "12")]
            DeleteRange(super::super::super::kvrpcpb::DeleteRangeResponse),
            #[prost(message, tag = "13")]
            RawGet(super::super::super::kvrpcpb::RawGetResponse),
            #[prost(message, tag = "14")]
            RawBatchGet(super::super::super::kvrpcpb::RawBatchGetResponse),
            #[prost(message, tag = "15")]
            RawPut(super::super::super::kvrpcpb::RawPutResponse),
            #[prost(message, tag = "16")]
            RawBatchPut(super::super::super::kvrpcpb::RawBatchPutResponse),
            #[prost(message, tag = "17")]
            RawDelete(super::super::super::kvrpcpb::RawDeleteResponse),
            #[prost(message, tag = "18")]
            RawBatchDelete(super::super::super::kvrpcpb::RawBatchDeleteResponse),
            #[prost(message, tag = "19")]
            RawScan(super::super::super::kvrpcpb::RawScanResponse),
            #[prost(message, tag = "20")]
            RawDeleteRange(super::super::super::kvrpcpb::RawDeleteRangeResponse),
            #[prost(message, tag = "21")]
            RawBatchScan(super::super::super::kvrpcpb::RawBatchScanResponse),
            #[prost(message, tag = "22")]
            Coprocessor(super::super::super::coprocessor::Response),
            #[prost(message, tag = "23")]
            PessimisticLock(super::super::super::kvrpcpb::PessimisticLockResponse),
            #[prost(message, tag = "24")]
            PessimisticRollback(
                super::super::super::kvrpcpb::PessimisticRollbackResponse,
            ),
            #[prost(message, tag = "25")]
            CheckTxnStatus(super::super::super::kvrpcpb::CheckTxnStatusResponse),
            #[prost(message, tag = "26")]
            TxnHeartBeat(super::super::super::kvrpcpb::TxnHeartBeatResponse),
            #[prost(message, tag = "33")]
            CheckSecondaryLocks(
                super::super::super::kvrpcpb::CheckSecondaryLocksResponse,
            ),
            #[prost(message, tag = "34")]
            RawCoprocessor(super::super::super::kvrpcpb::RawCoprocessorResponse),
            #[prost(message, tag = "35")]
            FlashbackToVersion(super::super::super::kvrpcpb::FlashbackToVersionResponse),
            #[prost(message, tag = "36")]
            PrepareFlashbackToVersion(
                super::super::super::kvrpcpb::PrepareFlashbackToVersionResponse,
            ),
            /// For some test cases.
            #[prost(message, tag = "255")]
            Empty(super::super::BatchCommandsEmptyResponse),
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchRaftMessage {
    #[prost(message, repeated, tag = "1")]
    pub msgs: ::prost::alloc::vec::Vec<super::raft_serverpb::RaftMessage>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchCommandsEmptyRequest {
    /// ID of the test request.
    #[prost(uint64, tag = "1")]
    pub test_id: u64,
    /// TiKV needs to delay at least such a time to response the client.
    #[prost(uint64, tag = "2")]
    pub delay_time: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchCommandsEmptyResponse {
    /// ID of the test request.
    #[prost(uint64, tag = "1")]
    pub test_id: u64,
}
/// Generated client implementations.
pub mod tikv_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Key/value store API for TiKV.
    #[derive(Debug, Clone)]
    pub struct TikvClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl TikvClient<tonic::transport::Channel> {
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
    impl<T> TikvClient<T>
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
        ) -> TikvClient<InterceptedService<T, F>>
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
            TikvClient::new(InterceptedService::new(inner, interceptor))
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
        /// Commands using a transactional interface.
        pub async fn kv_get(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::GetRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::GetResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvGet");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvGet"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_scan(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::ScanRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::ScanResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvScan");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvScan"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_prewrite(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::PrewriteRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::PrewriteResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvPrewrite");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvPrewrite"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_pessimistic_lock(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::PessimisticLockRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::PessimisticLockResponse>,
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
                "/tikvpb.Tikv/KvPessimisticLock",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvPessimisticLock"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_pessimistic_rollback(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::PessimisticRollbackRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::PessimisticRollbackResponse>,
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
                "/tikvpb.Tikv/KVPessimisticRollback",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KVPessimisticRollback"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_txn_heart_beat(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::TxnHeartBeatRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::TxnHeartBeatResponse>,
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
                "/tikvpb.Tikv/KvTxnHeartBeat",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvTxnHeartBeat"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_check_txn_status(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::CheckTxnStatusRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CheckTxnStatusResponse>,
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
                "/tikvpb.Tikv/KvCheckTxnStatus",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvCheckTxnStatus"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_check_secondary_locks(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::CheckSecondaryLocksRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CheckSecondaryLocksResponse>,
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
                "/tikvpb.Tikv/KvCheckSecondaryLocks",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvCheckSecondaryLocks"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_commit(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::CommitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CommitResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvCommit");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvCommit"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_import(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::ImportRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::ImportResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvImport");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvImport"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_cleanup(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::CleanupRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CleanupResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvCleanup");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvCleanup"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_batch_get(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::BatchGetRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::BatchGetResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvBatchGet");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvBatchGet"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_batch_rollback(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::BatchRollbackRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::BatchRollbackResponse>,
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
                "/tikvpb.Tikv/KvBatchRollback",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvBatchRollback"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_scan_lock(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::ScanLockRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::ScanLockResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvScanLock");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvScanLock"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_resolve_lock(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::ResolveLockRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::ResolveLockResponse>,
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
                "/tikvpb.Tikv/KvResolveLock",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvResolveLock"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_gc(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::GcRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::GcResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/KvGC");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvGC"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_delete_range(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::DeleteRangeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::DeleteRangeResponse>,
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
                "/tikvpb.Tikv/KvDeleteRange",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "KvDeleteRange"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_prepare_flashback_to_version(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::PrepareFlashbackToVersionRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::PrepareFlashbackToVersionResponse>,
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
                "/tikvpb.Tikv/KvPrepareFlashbackToVersion",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvPrepareFlashbackToVersion"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn kv_flashback_to_version(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::FlashbackToVersionRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::FlashbackToVersionResponse>,
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
                "/tikvpb.Tikv/KvFlashbackToVersion",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "KvFlashbackToVersion"));
            self.inner.unary(req, path, codec).await
        }
        /// Raw commands; no transaction support.
        pub async fn raw_get(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawGetRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawGetResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawGet");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawGet"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_batch_get(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawBatchGetRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawBatchGetResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawBatchGet");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawBatchGet"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_put(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawPutRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawPutResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawPut");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawPut"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_batch_put(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawBatchPutRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawBatchPutResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawBatchPut");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawBatchPut"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_delete(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawDeleteRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawDeleteResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawDelete");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawDelete"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_batch_delete(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::RawBatchDeleteRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawBatchDeleteResponse>,
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
                "/tikvpb.Tikv/RawBatchDelete",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "RawBatchDelete"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_scan(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawScanRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawScanResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawScan");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawScan"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_delete_range(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::RawDeleteRangeRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawDeleteRangeResponse>,
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
                "/tikvpb.Tikv/RawDeleteRange",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "RawDeleteRange"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_batch_scan(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawBatchScanRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawBatchScanResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawBatchScan");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawBatchScan"));
            self.inner.unary(req, path, codec).await
        }
        /// Get TTL of the key. Returns 0 if TTL is not set for the key.
        pub async fn raw_get_key_ttl(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawGetKeyTtlRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawGetKeyTtlResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawGetKeyTTL");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawGetKeyTTL"));
            self.inner.unary(req, path, codec).await
        }
        /// Compare if the value in database equals to `RawCASRequest.previous_value` before putting the new value. If not, this request will have no effect and the value in the database will be returned.
        pub async fn raw_compare_and_swap(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawCasRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawCasResponse>,
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
                "/tikvpb.Tikv/RawCompareAndSwap",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "RawCompareAndSwap"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn raw_checksum(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::RawChecksumRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawChecksumResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/RawChecksum");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "RawChecksum"));
            self.inner.unary(req, path, codec).await
        }
        /// Store commands (sent to a each TiKV node in a cluster, rather than a certain region).
        pub async fn unsafe_destroy_range(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::UnsafeDestroyRangeRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::UnsafeDestroyRangeResponse>,
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
                "/tikvpb.Tikv/UnsafeDestroyRange",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "UnsafeDestroyRange"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn register_lock_observer(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::RegisterLockObserverRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RegisterLockObserverResponse>,
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
                "/tikvpb.Tikv/RegisterLockObserver",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "RegisterLockObserver"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn check_lock_observer(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::CheckLockObserverRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CheckLockObserverResponse>,
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
                "/tikvpb.Tikv/CheckLockObserver",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "CheckLockObserver"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn remove_lock_observer(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::RemoveLockObserverRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RemoveLockObserverResponse>,
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
                "/tikvpb.Tikv/RemoveLockObserver",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "RemoveLockObserver"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn physical_scan_lock(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::PhysicalScanLockRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::PhysicalScanLockResponse>,
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
                "/tikvpb.Tikv/PhysicalScanLock",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "PhysicalScanLock"));
            self.inner.unary(req, path, codec).await
        }
        /// Commands for executing SQL in the TiKV coprocessor (i.e., 'pushed down' to TiKV rather than
        /// executed in TiDB).
        pub async fn coprocessor(
            &mut self,
            request: impl tonic::IntoRequest<super::super::coprocessor::Request>,
        ) -> std::result::Result<
            tonic::Response<super::super::coprocessor::Response>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/Coprocessor");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "Coprocessor"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn coprocessor_stream(
            &mut self,
            request: impl tonic::IntoRequest<super::super::coprocessor::Request>,
        ) -> std::result::Result<
            tonic::Response<
                tonic::codec::Streaming<super::super::coprocessor::Response>,
            >,
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
                "/tikvpb.Tikv/CoprocessorStream",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "CoprocessorStream"));
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn batch_coprocessor(
            &mut self,
            request: impl tonic::IntoRequest<super::super::coprocessor::BatchRequest>,
        ) -> std::result::Result<
            tonic::Response<
                tonic::codec::Streaming<super::super::coprocessor::BatchResponse>,
            >,
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
                "/tikvpb.Tikv/BatchCoprocessor",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "BatchCoprocessor"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// Command for executing custom user requests in TiKV coprocessor_v2.
        pub async fn raw_coprocessor(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::RawCoprocessorRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::RawCoprocessorResponse>,
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
                "/tikvpb.Tikv/RawCoprocessor",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "RawCoprocessor"));
            self.inner.unary(req, path, codec).await
        }
        /// Raft commands (sent between TiKV nodes).
        pub async fn raft(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::super::raft_serverpb::RaftMessage,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::raft_serverpb::Done>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/Raft");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "Raft"));
            self.inner.client_streaming(req, path, codec).await
        }
        pub async fn batch_raft(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::BatchRaftMessage>,
        ) -> std::result::Result<
            tonic::Response<super::super::raft_serverpb::Done>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/BatchRaft");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "BatchRaft"));
            self.inner.client_streaming(req, path, codec).await
        }
        pub async fn snapshot(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::super::raft_serverpb::SnapshotChunk,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::raft_serverpb::Done>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/Snapshot");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "Snapshot"));
            self.inner.client_streaming(req, path, codec).await
        }
        pub async fn tablet_snapshot(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::super::raft_serverpb::TabletSnapshotRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<
                tonic::codec::Streaming<
                    super::super::raft_serverpb::TabletSnapshotResponse,
                >,
            >,
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
                "/tikvpb.Tikv/TabletSnapshot",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "TabletSnapshot"));
            self.inner.streaming(req, path, codec).await
        }
        /// Sent from PD or TiDB to a TiKV node.
        pub async fn split_region(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::SplitRegionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::SplitRegionResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/SplitRegion");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "SplitRegion"));
            self.inner.unary(req, path, codec).await
        }
        /// Sent from TiFlash or TiKV to a TiKV node.
        pub async fn read_index(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::ReadIndexRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::ReadIndexResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/ReadIndex");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "ReadIndex"));
            self.inner.unary(req, path, codec).await
        }
        /// Commands for debugging transactions.
        pub async fn mvcc_get_by_key(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::MvccGetByKeyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::MvccGetByKeyResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/MvccGetByKey");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "MvccGetByKey"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn mvcc_get_by_start_ts(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::MvccGetByStartTsRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::MvccGetByStartTsResponse>,
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
                "/tikvpb.Tikv/MvccGetByStartTs",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "MvccGetByStartTs"));
            self.inner.unary(req, path, codec).await
        }
        /// Batched commands.
        pub async fn batch_commands(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::BatchCommandsRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::BatchCommandsResponse>>,
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
                "/tikvpb.Tikv/BatchCommands",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "BatchCommands"));
            self.inner.streaming(req, path, codec).await
        }
        /// These are for mpp execution.
        pub async fn dispatch_mpp_task(
            &mut self,
            request: impl tonic::IntoRequest<super::super::mpp::DispatchTaskRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::mpp::DispatchTaskResponse>,
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
                "/tikvpb.Tikv/DispatchMPPTask",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "DispatchMPPTask"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn cancel_mpp_task(
            &mut self,
            request: impl tonic::IntoRequest<super::super::mpp::CancelTaskRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::mpp::CancelTaskResponse>,
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
                "/tikvpb.Tikv/CancelMPPTask",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "CancelMPPTask"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn establish_mpp_connection(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::mpp::EstablishMppConnectionRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::super::mpp::MppDataPacket>>,
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
                "/tikvpb.Tikv/EstablishMPPConnection",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "EstablishMPPConnection"));
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn is_alive(
            &mut self,
            request: impl tonic::IntoRequest<super::super::mpp::IsAliveRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::mpp::IsAliveResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/IsAlive");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "IsAlive"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn report_mpp_task_status(
            &mut self,
            request: impl tonic::IntoRequest<super::super::mpp::ReportTaskStatusRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::mpp::ReportTaskStatusResponse>,
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
                "/tikvpb.Tikv/ReportMPPTaskStatus",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "ReportMPPTaskStatus"));
            self.inner.unary(req, path, codec).await
        }
        /// / CheckLeader sends all information (includes region term and epoch) to other stores.
        /// / Once a store receives a request, it checks term and epoch for each region, and sends the regions whose
        /// / term and epoch match with local information in the store.
        /// / After the client collected all responses from all stores, it checks if got a quorum of responses from
        /// / other stores for every region, and decides to advance resolved ts from these regions.
        pub async fn check_leader(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::CheckLeaderRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CheckLeaderResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/CheckLeader");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "CheckLeader"));
            self.inner.unary(req, path, codec).await
        }
        /// / Get the minimal `safe_ts` from regions at the store
        pub async fn get_store_safe_ts(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::StoreSafeTsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::StoreSafeTsResponse>,
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
                "/tikvpb.Tikv/GetStoreSafeTS",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "GetStoreSafeTS"));
            self.inner.unary(req, path, codec).await
        }
        /// / Get the information about lock waiting from TiKV.
        pub async fn get_lock_wait_info(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::GetLockWaitInfoRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::GetLockWaitInfoResponse>,
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
                "/tikvpb.Tikv/GetLockWaitInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "GetLockWaitInfo"));
            self.inner.unary(req, path, codec).await
        }
        /// / Compact a specified key range. This request is not restricted to raft leaders and will not be replicated.
        /// / It only compacts data on this node.
        /// / TODO: Currently this RPC is designed to be only compatible with TiFlash.
        /// / Shall be move out in https://github.com/pingcap/kvproto/issues/912
        pub async fn compact(
            &mut self,
            request: impl tonic::IntoRequest<super::super::kvrpcpb::CompactRequest>,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::CompactResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/Compact");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "Compact"));
            self.inner.unary(req, path, codec).await
        }
        /// / Get the information about history lock waiting from TiKV.
        pub async fn get_lock_wait_history(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::GetLockWaitHistoryRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::GetLockWaitHistoryResponse>,
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
                "/tikvpb.Tikv/GetLockWaitHistory",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "GetLockWaitHistory"));
            self.inner.unary(req, path, codec).await
        }
        /// / Get system table from TiFlash
        pub async fn get_ti_flash_system_table(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::kvrpcpb::TiFlashSystemTableRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::kvrpcpb::TiFlashSystemTableResponse>,
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
                "/tikvpb.Tikv/GetTiFlashSystemTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "GetTiFlashSystemTable"));
            self.inner.unary(req, path, codec).await
        }
        /// These are for TiFlash disaggregated architecture
        /// / Try to lock a S3 object, atomically
        pub async fn try_add_lock(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::disaggregated::TryAddLockRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::disaggregated::TryAddLockResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tikvpb.Tikv/tryAddLock");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "tryAddLock"));
            self.inner.unary(req, path, codec).await
        }
        /// / Try to delete a S3 object, atomically
        pub async fn try_mark_delete(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::disaggregated::TryMarkDeleteRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::disaggregated::TryMarkDeleteResponse>,
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
                "/tikvpb.Tikv/tryMarkDelete",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tikvpb.Tikv", "tryMarkDelete"));
            self.inner.unary(req, path, codec).await
        }
        /// / Build the disaggregated task on TiFlash write node
        pub async fn establish_disagg_task(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::disaggregated::EstablishDisaggTaskRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::disaggregated::EstablishDisaggTaskResponse>,
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
                "/tikvpb.Tikv/EstablishDisaggTask",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "EstablishDisaggTask"));
            self.inner.unary(req, path, codec).await
        }
        /// / Cancel the disaggregated task on TiFlash write node
        pub async fn cancel_disagg_task(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::disaggregated::CancelDisaggTaskRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::disaggregated::CancelDisaggTaskResponse>,
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
                "/tikvpb.Tikv/CancelDisaggTask",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "CancelDisaggTask"));
            self.inner.unary(req, path, codec).await
        }
        /// / Exchange page data between TiFlash write node and compute node
        pub async fn fetch_disagg_pages(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::disaggregated::FetchDisaggPagesRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<
                tonic::codec::Streaming<super::super::disaggregated::PagesPacket>,
            >,
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
                "/tikvpb.Tikv/FetchDisaggPages",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "FetchDisaggPages"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// / Compute node get configuration from Write node
        pub async fn get_disagg_config(
            &mut self,
            request: impl tonic::IntoRequest<
                super::super::disaggregated::GetDisaggConfigRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::super::disaggregated::GetDisaggConfigResponse>,
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
                "/tikvpb.Tikv/GetDisaggConfig",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tikvpb.Tikv", "GetDisaggConfig"));
            self.inner.unary(req, path, codec).await
        }
    }
}
