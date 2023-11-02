#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SearchLogRequest {
    #[prost(int64, tag = "1")]
    pub start_time: i64,
    #[prost(int64, tag = "2")]
    pub end_time: i64,
    #[prost(enumeration = "LogLevel", repeated, tag = "3")]
    pub levels: ::prost::alloc::vec::Vec<i32>,
    /// We use a string array to represent multiple CNF pattern sceniaor like:
    /// SELECT * FROM t WHERE c LIKE '%s%' and c REGEXP '.*a.*' because
    /// Golang and Rust don't support perl-like (?=re1)(?=re2)
    #[prost(string, repeated, tag = "4")]
    pub patterns: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(enumeration = "search_log_request::Target", tag = "5")]
    pub target: i32,
}
/// Nested message and enum types in `SearchLogRequest`.
pub mod search_log_request {
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
    pub enum Target {
        Normal = 0,
        Slow = 1,
    }
    impl Target {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Target::Normal => "Normal",
                Target::Slow => "Slow",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "Normal" => Some(Self::Normal),
                "Slow" => Some(Self::Slow),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SearchLogResponse {
    #[prost(message, repeated, tag = "1")]
    pub messages: ::prost::alloc::vec::Vec<LogMessage>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogMessage {
    #[prost(int64, tag = "1")]
    pub time: i64,
    #[prost(enumeration = "LogLevel", tag = "2")]
    pub level: i32,
    #[prost(string, tag = "3")]
    pub message: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerInfoRequest {
    #[prost(enumeration = "ServerInfoType", tag = "1")]
    pub tp: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerInfoPair {
    #[prost(string, tag = "1")]
    pub key: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerInfoItem {
    /// cpu, memory, disk, network ...
    #[prost(string, tag = "1")]
    pub tp: ::prost::alloc::string::String,
    /// eg. network: lo1/eth0, cpu: core1/core2, disk: sda1/sda2
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    /// all key-value pairs for specified item, e.g:
    /// ServerInfoItem {
    /// tp = "network"
    /// name = "eth0"
    /// paris = \[
    /// ServerInfoPair { key = "readbytes", value = "4k"},
    /// ServerInfoPair { key = "writebytes", value = "1k"},
    /// \]
    /// }
    #[prost(message, repeated, tag = "3")]
    pub pairs: ::prost::alloc::vec::Vec<ServerInfoPair>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerInfoResponse {
    #[prost(message, repeated, tag = "1")]
    pub items: ::prost::alloc::vec::Vec<ServerInfoItem>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum LogLevel {
    Unknown = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Trace = 4,
    Critical = 5,
    Error = 6,
}
impl LogLevel {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            LogLevel::Unknown => "UNKNOWN",
            LogLevel::Debug => "Debug",
            LogLevel::Info => "Info",
            LogLevel::Warn => "Warn",
            LogLevel::Trace => "Trace",
            LogLevel::Critical => "Critical",
            LogLevel::Error => "Error",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "Debug" => Some(Self::Debug),
            "Info" => Some(Self::Info),
            "Warn" => Some(Self::Warn),
            "Trace" => Some(Self::Trace),
            "Critical" => Some(Self::Critical),
            "Error" => Some(Self::Error),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServerInfoType {
    All = 0,
    HardwareInfo = 1,
    SystemInfo = 2,
    LoadInfo = 3,
}
impl ServerInfoType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ServerInfoType::All => "All",
            ServerInfoType::HardwareInfo => "HardwareInfo",
            ServerInfoType::SystemInfo => "SystemInfo",
            ServerInfoType::LoadInfo => "LoadInfo",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "All" => Some(Self::All),
            "HardwareInfo" => Some(Self::HardwareInfo),
            "SystemInfo" => Some(Self::SystemInfo),
            "LoadInfo" => Some(Self::LoadInfo),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod diagnostics_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Diagnostics service for TiDB cluster components.
    #[derive(Debug, Clone)]
    pub struct DiagnosticsClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl DiagnosticsClient<tonic::transport::Channel> {
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
    impl<T> DiagnosticsClient<T>
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
        ) -> DiagnosticsClient<InterceptedService<T, F>>
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
            DiagnosticsClient::new(InterceptedService::new(inner, interceptor))
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
        /// Searchs log in the target node
        pub async fn search_log(
            &mut self,
            request: impl tonic::IntoRequest<super::SearchLogRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::SearchLogResponse>>,
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
                "/diagnosticspb.Diagnostics/search_log",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("diagnosticspb.Diagnostics", "search_log"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// Retrieves server info in the target node
        pub async fn server_info(
            &mut self,
            request: impl tonic::IntoRequest<super::ServerInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ServerInfoResponse>,
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
                "/diagnosticspb.Diagnostics/server_info",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("diagnosticspb.Diagnostics", "server_info"));
            self.inner.unary(req, path, codec).await
        }
    }
}
