#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestHeader {
    /// cluster_id is the ID of the cluster which be sent to.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    /// sender_id is the ID of the sender server.
    #[prost(uint64, tag = "2")]
    pub sender_id: u64,
    /// keyspace_id is the unique id of the tenant/keyspace.
    #[prost(uint32, tag = "3")]
    pub keyspace_id: u32,
    /// keyspace_group_id is the unique id of the keyspace group to which the tenant/keyspace belongs.
    #[prost(uint32, tag = "4")]
    pub keyspace_group_id: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseHeader {
    /// cluster_id is the ID of the cluster which sent the response.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<Error>,
    /// keyspace_id is the unique id of the tenant/keyspace as the response receiver.
    #[prost(uint32, tag = "3")]
    pub keyspace_id: u32,
    /// keyspace_group_id is the unique id of the keyspace group to which the tenant/keyspace belongs.
    #[prost(uint32, tag = "4")]
    pub keyspace_group_id: u32,
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
pub struct TsoRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub count: u32,
    #[prost(string, tag = "3")]
    pub dc_location: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TsoResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint32, tag = "2")]
    pub count: u32,
    #[prost(message, optional, tag = "3")]
    pub timestamp: ::core::option::Option<super::pdpb::Timestamp>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Participant {
    /// name is the unique name of the TSO participant.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// id is the unique id of the TSO participant.
    #[prost(uint64, tag = "2")]
    pub id: u64,
    /// listen_urls is the serivce endpoint list in the url format.
    /// listen_urls\[0\] is primary service endpoint.
    #[prost(string, repeated, tag = "3")]
    pub listen_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyspaceGroupMember {
    #[prost(string, tag = "1")]
    pub address: ::prost::alloc::string::String,
    #[prost(bool, tag = "2")]
    pub is_primary: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitState {
    #[prost(uint32, tag = "1")]
    pub split_source: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyspaceGroup {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(string, tag = "2")]
    pub user_kind: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub split_state: ::core::option::Option<SplitState>,
    #[prost(message, repeated, tag = "4")]
    pub members: ::prost::alloc::vec::Vec<KeyspaceGroupMember>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindGroupByKeyspaceIdRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub keyspace_id: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindGroupByKeyspaceIdResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub keyspace_group: ::core::option::Option<KeyspaceGroup>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMinTsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(string, tag = "2")]
    pub dc_location: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMinTsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub timestamp: ::core::option::Option<super::pdpb::Timestamp>,
    /// the count of keyspace group primaries that the TSO server/pod is serving
    #[prost(uint32, tag = "3")]
    pub keyspace_groups_serving: u32,
    /// the total count of keyspace groups
    #[prost(uint32, tag = "4")]
    pub keyspace_groups_total: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorType {
    Ok = 0,
    Unknown = 1,
    NotBootstrapped = 2,
    AlreadyBootstrapped = 3,
    InvalidValue = 4,
    ClusterMismatched = 5,
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
            ErrorType::AlreadyBootstrapped => "ALREADY_BOOTSTRAPPED",
            ErrorType::InvalidValue => "INVALID_VALUE",
            ErrorType::ClusterMismatched => "CLUSTER_MISMATCHED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "OK" => Some(Self::Ok),
            "UNKNOWN" => Some(Self::Unknown),
            "NOT_BOOTSTRAPPED" => Some(Self::NotBootstrapped),
            "ALREADY_BOOTSTRAPPED" => Some(Self::AlreadyBootstrapped),
            "INVALID_VALUE" => Some(Self::InvalidValue),
            "CLUSTER_MISMATCHED" => Some(Self::ClusterMismatched),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod tso_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct TsoClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl TsoClient<tonic::transport::Channel> {
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
    impl<T> TsoClient<T>
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
        ) -> TsoClient<InterceptedService<T, F>>
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
            TsoClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn tso(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::TsoRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::TsoResponse>>,
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
            let path = http::uri::PathAndQuery::from_static("/tsopb.TSO/Tso");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tsopb.TSO", "Tso"));
            self.inner.streaming(req, path, codec).await
        }
        /// Find the keyspace group that the keyspace belongs to by keyspace id.
        pub async fn find_group_by_keyspace_id(
            &mut self,
            request: impl tonic::IntoRequest<super::FindGroupByKeyspaceIdRequest>,
        ) -> std::result::Result<
            tonic::Response<super::FindGroupByKeyspaceIdResponse>,
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
                "/tsopb.TSO/FindGroupByKeyspaceID",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("tsopb.TSO", "FindGroupByKeyspaceID"));
            self.inner.unary(req, path, codec).await
        }
        /// Get the minimum timestamp across all keyspace groups served by the TSO server who receives
        /// and handle the request. If the TSO server/pod is not serving any keyspace group, return
        /// an empty timestamp, and the client needs to skip the empty timestamps when collecting
        /// the min timestamp from all TSO servers/pods.
        pub async fn get_min_ts(
            &mut self,
            request: impl tonic::IntoRequest<super::GetMinTsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMinTsResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/tsopb.TSO/GetMinTS");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tsopb.TSO", "GetMinTS"));
            self.inner.unary(req, path, codec).await
        }
    }
}
