#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(enumeration = "StatusCode", tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
}
/// The version is used to tell the configuration which can be shared
/// or not apart.
/// Global version represents the version of these configuration
/// which can be shared, each kind of component only have one.
/// For local version, every component will have one to represent
/// the version of these configuration which cannot be shared.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Version {
    #[prost(uint64, tag = "1")]
    pub local: u64,
    #[prost(uint64, tag = "2")]
    pub global: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Local {
    #[prost(string, tag = "1")]
    pub component_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Global {
    #[prost(string, tag = "1")]
    pub component: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigKind {
    #[prost(oneof = "config_kind::Kind", tags = "1, 2")]
    pub kind: ::core::option::Option<config_kind::Kind>,
}
/// Nested message and enum types in `ConfigKind`.
pub mod config_kind {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Kind {
        #[prost(message, tag = "1")]
        Local(super::Local),
        #[prost(message, tag = "2")]
        Global(super::Global),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigEntry {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LocalConfig {
    #[prost(message, optional, tag = "1")]
    pub version: ::core::option::Option<Version>,
    #[prost(string, tag = "2")]
    pub component: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub component_id: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub config: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Header {
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub version: ::core::option::Option<Version>,
    #[prost(string, tag = "3")]
    pub component: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub component_id: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub config: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub status: ::core::option::Option<Status>,
    #[prost(message, optional, tag = "3")]
    pub version: ::core::option::Option<Version>,
    #[prost(string, tag = "4")]
    pub config: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub status: ::core::option::Option<Status>,
    #[prost(message, repeated, tag = "3")]
    pub local_configs: ::prost::alloc::vec::Vec<LocalConfig>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub version: ::core::option::Option<Version>,
    #[prost(string, tag = "3")]
    pub component: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub component_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub status: ::core::option::Option<Status>,
    #[prost(message, optional, tag = "3")]
    pub version: ::core::option::Option<Version>,
    #[prost(string, tag = "4")]
    pub config: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub version: ::core::option::Option<Version>,
    #[prost(message, optional, tag = "3")]
    pub kind: ::core::option::Option<ConfigKind>,
    #[prost(message, repeated, tag = "4")]
    pub entries: ::prost::alloc::vec::Vec<ConfigEntry>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub status: ::core::option::Option<Status>,
    #[prost(message, optional, tag = "3")]
    pub version: ::core::option::Option<Version>,
    #[prost(string, tag = "4")]
    pub config: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub version: ::core::option::Option<Version>,
    #[prost(message, optional, tag = "3")]
    pub kind: ::core::option::Option<ConfigKind>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(message, optional, tag = "2")]
    pub status: ::core::option::Option<Status>,
    #[prost(message, optional, tag = "3")]
    pub version: ::core::option::Option<Version>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StatusCode {
    Unknown = 0,
    Ok = 1,
    WrongVersion = 2,
    NotChange = 3,
    ComponentNotFound = 4,
    ComponentIdNotFound = 5,
}
impl StatusCode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            StatusCode::Unknown => "UNKNOWN",
            StatusCode::Ok => "OK",
            StatusCode::WrongVersion => "WRONG_VERSION",
            StatusCode::NotChange => "NOT_CHANGE",
            StatusCode::ComponentNotFound => "COMPONENT_NOT_FOUND",
            StatusCode::ComponentIdNotFound => "COMPONENT_ID_NOT_FOUND",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "OK" => Some(Self::Ok),
            "WRONG_VERSION" => Some(Self::WrongVersion),
            "NOT_CHANGE" => Some(Self::NotChange),
            "COMPONENT_NOT_FOUND" => Some(Self::ComponentNotFound),
            "COMPONENT_ID_NOT_FOUND" => Some(Self::ComponentIdNotFound),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod config_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct ConfigClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ConfigClient<tonic::transport::Channel> {
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
    impl<T> ConfigClient<T>
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
        ) -> ConfigClient<InterceptedService<T, F>>
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
            ConfigClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn create(
            &mut self,
            request: impl tonic::IntoRequest<super::CreateRequest>,
        ) -> std::result::Result<tonic::Response<super::CreateResponse>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/configpb.Config/Create");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("configpb.Config", "Create"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_all(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAllRequest>,
        ) -> std::result::Result<tonic::Response<super::GetAllResponse>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/configpb.Config/GetAll");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("configpb.Config", "GetAll"));
            self.inner.unary(req, path, codec).await
        }
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
            let path = http::uri::PathAndQuery::from_static("/configpb.Config/Get");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("configpb.Config", "Get"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateRequest>,
        ) -> std::result::Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/configpb.Config/Update");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("configpb.Config", "Update"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteRequest>,
        ) -> std::result::Result<tonic::Response<super::DeleteResponse>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/configpb.Config/Delete");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("configpb.Config", "Delete"));
            self.inner.unary(req, path, codec).await
        }
    }
}
