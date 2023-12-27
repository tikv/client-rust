#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestHeader {
    /// cluster_id is the ID of the cluster which be sent to.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    /// sender_id is the ID of the sender server, also member ID or etcd ID.
    #[prost(uint64, tag = "2")]
    pub sender_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseHeader {
    /// cluster_id is the ID of the cluster which sent the response.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(enumeration = "ErrorType", tag = "1")]
    pub r#type: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeySpace {
    #[prost(bytes = "vec", tag = "1")]
    pub space_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub gc_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListKeySpacesRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    /// set with_gc_safe_point to true to also receive gc safe point for each key space
    #[prost(bool, tag = "2")]
    pub with_gc_safe_point: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListKeySpacesResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub key_spaces: ::prost::alloc::vec::Vec<KeySpace>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMinServiceSafePointRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub space_id: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMinServiceSafePointResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub safe_point: u64,
    /// revision here is to safeguard the validity of the obtained min,
    /// preventing cases where new services register their safe points after min is obtained by gc worker
    #[prost(int64, tag = "3")]
    pub revision: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateGcSafePointRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub space_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub safe_point: u64,
    /// here client need to provide the revision obtained from GetMinServiceSafePoint,
    /// so server can check if it's still valid
    #[prost(int64, tag = "4")]
    pub revision: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateGcSafePointResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// update will be successful if revision is valid and new safepoint > old safe point
    /// if failed, previously obtained min might be incorrect, should retry from GetMinServiceSafePoint
    #[prost(bool, tag = "2")]
    pub succeeded: bool,
    #[prost(uint64, tag = "3")]
    pub new_safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateServiceSafePointRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub space_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub service_id: ::prost::alloc::vec::Vec<u8>,
    /// safe point will be set to expire on (PD Server time + TTL)
    /// pass in a ttl \< 0 to remove target safe point
    /// pass in MAX_INT64 to set a safe point that never expire
    #[prost(int64, tag = "4")]
    pub ttl: i64,
    #[prost(uint64, tag = "5")]
    pub safe_point: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateServiceSafePointResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// update will be successful if ttl \< 0 (a removal request)
    /// or if new safe point >= old safe point and new safe point >= gc safe point
    #[prost(bool, tag = "2")]
    pub succeeded: bool,
    #[prost(uint64, tag = "3")]
    pub gc_safe_point: u64,
    #[prost(uint64, tag = "4")]
    pub old_safe_point: u64,
    #[prost(uint64, tag = "5")]
    pub new_safe_point: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorType {
    Ok = 0,
    Unknown = 1,
    NotBootstrapped = 2,
    /// revision supplied does not match the current etcd revision
    RevisionMismatch = 3,
    /// if the proposed safe point is earlier than old safe point or gc safe point
    SafepointRollback = 4,
}
impl ErrorType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ErrorType::Ok => "OK",
            ErrorType::Unknown => "UNKNOWN",
            ErrorType::NotBootstrapped => "NOT_BOOTSTRAPPED",
            ErrorType::RevisionMismatch => "REVISION_MISMATCH",
            ErrorType::SafepointRollback => "SAFEPOINT_ROLLBACK",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "OK" => Some(Self::Ok),
            "UNKNOWN" => Some(Self::Unknown),
            "NOT_BOOTSTRAPPED" => Some(Self::NotBootstrapped),
            "REVISION_MISMATCH" => Some(Self::RevisionMismatch),
            "SAFEPOINT_ROLLBACK" => Some(Self::SafepointRollback),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod gc_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct GcClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl GcClient<tonic::transport::Channel> {
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
    impl<T> GcClient<T>
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
        ) -> GcClient<InterceptedService<T, F>>
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
            GcClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn list_key_spaces(
            &mut self,
            request: impl tonic::IntoRequest<super::ListKeySpacesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListKeySpacesResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/gcpb.GC/ListKeySpaces");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("gcpb.GC", "ListKeySpaces"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_min_service_safe_point(
            &mut self,
            request: impl tonic::IntoRequest<super::GetMinServiceSafePointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMinServiceSafePointResponse>,
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
                "/gcpb.GC/GetMinServiceSafePoint",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("gcpb.GC", "GetMinServiceSafePoint"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_gc_safe_point(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateGcSafePointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateGcSafePointResponse>,
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
                "/gcpb.GC/UpdateGCSafePoint",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("gcpb.GC", "UpdateGCSafePoint"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_service_safe_point(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateServiceSafePointRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateServiceSafePointResponse>,
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
                "/gcpb.GC/UpdateServiceSafePoint",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("gcpb.GC", "UpdateServiceSafePoint"));
            self.inner.unary(req, path, codec).await
        }
    }
}
