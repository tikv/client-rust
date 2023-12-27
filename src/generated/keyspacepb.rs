#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyspaceMeta {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    #[prost(enumeration = "KeyspaceState", tag = "3")]
    pub state: i32,
    #[prost(int64, tag = "4")]
    pub created_at: i64,
    #[prost(int64, tag = "5")]
    pub state_changed_at: i64,
    #[prost(map = "string, string", tag = "7")]
    pub config: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoadKeyspaceRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::RequestHeader>,
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoadKeyspaceResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub keyspace: ::core::option::Option<KeyspaceMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchKeyspacesRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::RequestHeader>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchKeyspacesResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub keyspaces: ::prost::alloc::vec::Vec<KeyspaceMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateKeyspaceStateRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub id: u32,
    #[prost(enumeration = "KeyspaceState", tag = "3")]
    pub state: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateKeyspaceStateResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub keyspace: ::core::option::Option<KeyspaceMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllKeyspacesRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::RequestHeader>,
    #[prost(uint32, tag = "2")]
    pub start_id: u32,
    #[prost(uint32, tag = "3")]
    pub limit: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllKeyspacesResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<super::pdpb::ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub keyspaces: ::prost::alloc::vec::Vec<KeyspaceMeta>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum KeyspaceState {
    Enabled = 0,
    Disabled = 1,
    Archived = 2,
    Tombstone = 3,
}
impl KeyspaceState {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            KeyspaceState::Enabled => "ENABLED",
            KeyspaceState::Disabled => "DISABLED",
            KeyspaceState::Archived => "ARCHIVED",
            KeyspaceState::Tombstone => "TOMBSTONE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ENABLED" => Some(Self::Enabled),
            "DISABLED" => Some(Self::Disabled),
            "ARCHIVED" => Some(Self::Archived),
            "TOMBSTONE" => Some(Self::Tombstone),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod keyspace_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Keyspace provides services to manage keyspaces.
    #[derive(Debug, Clone)]
    pub struct KeyspaceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl KeyspaceClient<tonic::transport::Channel> {
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
    impl<T> KeyspaceClient<T>
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
        ) -> KeyspaceClient<InterceptedService<T, F>>
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
            KeyspaceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn load_keyspace(
            &mut self,
            request: impl tonic::IntoRequest<super::LoadKeyspaceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::LoadKeyspaceResponse>,
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
                "/keyspacepb.Keyspace/LoadKeyspace",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("keyspacepb.Keyspace", "LoadKeyspace"));
            self.inner.unary(req, path, codec).await
        }
        /// WatchKeyspaces first return all current keyspaces' metadata as its first response.
        /// Then, it returns responses containing keyspaces that had their metadata changed.
        pub async fn watch_keyspaces(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchKeyspacesRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::WatchKeyspacesResponse>>,
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
                "/keyspacepb.Keyspace/WatchKeyspaces",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("keyspacepb.Keyspace", "WatchKeyspaces"));
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn update_keyspace_state(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateKeyspaceStateRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateKeyspaceStateResponse>,
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
                "/keyspacepb.Keyspace/UpdateKeyspaceState",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("keyspacepb.Keyspace", "UpdateKeyspaceState"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_all_keyspaces(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAllKeyspacesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetAllKeyspacesResponse>,
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
                "/keyspacepb.Keyspace/GetAllKeyspaces",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("keyspacepb.Keyspace", "GetAllKeyspaces"));
            self.inner.unary(req, path, codec).await
        }
    }
}
