#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AutoIdRequest {
    #[prost(int64, tag = "1")]
    pub db_id: i64,
    #[prost(int64, tag = "2")]
    pub tbl_id: i64,
    #[prost(bool, tag = "3")]
    pub is_unsigned: bool,
    #[prost(uint64, tag = "4")]
    pub n: u64,
    #[prost(int64, tag = "5")]
    pub increment: i64,
    #[prost(int64, tag = "6")]
    pub offset: i64,
    #[prost(uint32, tag = "7")]
    pub keyspace_id: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AutoIdResponse {
    #[prost(int64, tag = "1")]
    pub min: i64,
    #[prost(int64, tag = "2")]
    pub max: i64,
    #[prost(bytes = "vec", tag = "3")]
    pub errmsg: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RebaseRequest {
    #[prost(int64, tag = "1")]
    pub db_id: i64,
    #[prost(int64, tag = "2")]
    pub tbl_id: i64,
    #[prost(bool, tag = "3")]
    pub is_unsigned: bool,
    #[prost(int64, tag = "4")]
    pub base: i64,
    #[prost(bool, tag = "5")]
    pub force: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RebaseResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub errmsg: ::prost::alloc::vec::Vec<u8>,
}
/// Generated client implementations.
pub mod auto_id_alloc_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct AutoIdAllocClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl AutoIdAllocClient<tonic::transport::Channel> {
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
    impl<T> AutoIdAllocClient<T>
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
        ) -> AutoIdAllocClient<InterceptedService<T, F>>
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
            AutoIdAllocClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn alloc_auto_id(
            &mut self,
            request: impl tonic::IntoRequest<super::AutoIdRequest>,
        ) -> std::result::Result<tonic::Response<super::AutoIdResponse>, tonic::Status> {
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
                "/autoid.AutoIDAlloc/AllocAutoID",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("autoid.AutoIDAlloc", "AllocAutoID"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn rebase(
            &mut self,
            request: impl tonic::IntoRequest<super::RebaseRequest>,
        ) -> std::result::Result<tonic::Response<super::RebaseResponse>, tonic::Status> {
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
                "/autoid.AutoIDAlloc/Rebase",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("autoid.AutoIDAlloc", "Rebase"));
            self.inner.unary(req, path, codec).await
        }
    }
}
