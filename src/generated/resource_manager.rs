#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListResourceGroupsRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListResourceGroupsResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, repeated, tag = "2")]
    pub groups: ::prost::alloc::vec::Vec<ResourceGroup>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetResourceGroupRequest {
    #[prost(string, tag = "1")]
    pub resource_group_name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetResourceGroupResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, optional, tag = "2")]
    pub group: ::core::option::Option<ResourceGroup>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteResourceGroupRequest {
    #[prost(string, tag = "1")]
    pub resource_group_name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteResourceGroupResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(string, tag = "2")]
    pub body: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutResourceGroupRequest {
    #[prost(message, optional, tag = "1")]
    pub group: ::core::option::Option<ResourceGroup>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutResourceGroupResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(string, tag = "2")]
    pub body: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenBucketsRequest {
    #[prost(message, repeated, tag = "1")]
    pub requests: ::prost::alloc::vec::Vec<TokenBucketRequest>,
    #[prost(uint64, tag = "2")]
    pub target_request_period_ms: u64,
    #[prost(uint64, tag = "3")]
    pub client_unique_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenBucketRequest {
    #[prost(string, tag = "1")]
    pub resource_group_name: ::prost::alloc::string::String,
    /// Aggregate statistics in group level.
    #[prost(message, optional, tag = "4")]
    pub consumption_since_last_request: ::core::option::Option<Consumption>,
    /// label background request.
    #[prost(bool, tag = "5")]
    pub is_background: bool,
    #[prost(bool, tag = "6")]
    pub is_tiflash: bool,
    #[prost(oneof = "token_bucket_request::Request", tags = "2, 3")]
    pub request: ::core::option::Option<token_bucket_request::Request>,
}
/// Nested message and enum types in `TokenBucketRequest`.
pub mod token_bucket_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RequestRu {
        #[prost(message, repeated, tag = "1")]
        pub request_r_u: ::prost::alloc::vec::Vec<super::RequestUnitItem>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RequestRawResource {
        #[prost(message, repeated, tag = "1")]
        pub request_raw_resource: ::prost::alloc::vec::Vec<super::RawResourceItem>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        /// RU mode, group settings with WRU/RRU etc resource abstract unit.
        #[prost(message, tag = "2")]
        RuItems(RequestRu),
        /// Raw mode, group settings with CPU/IO etc resource unit.
        #[prost(message, tag = "3")]
        RawResourceItems(RequestRawResource),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenBucketsResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, repeated, tag = "2")]
    pub responses: ::prost::alloc::vec::Vec<TokenBucketResponse>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenBucketResponse {
    #[prost(string, tag = "1")]
    pub resource_group_name: ::prost::alloc::string::String,
    /// RU mode
    #[prost(message, repeated, tag = "2")]
    pub granted_r_u_tokens: ::prost::alloc::vec::Vec<GrantedRuTokenBucket>,
    /// Raw mode
    #[prost(message, repeated, tag = "3")]
    pub granted_resource_tokens: ::prost::alloc::vec::Vec<GrantedRawResourceTokenBucket>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GrantedRuTokenBucket {
    #[prost(enumeration = "RequestUnitType", tag = "1")]
    pub r#type: i32,
    #[prost(message, optional, tag = "2")]
    pub granted_tokens: ::core::option::Option<TokenBucket>,
    #[prost(int64, tag = "3")]
    pub trickle_time_ms: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GrantedRawResourceTokenBucket {
    #[prost(enumeration = "RawResourceType", tag = "1")]
    pub r#type: i32,
    #[prost(message, optional, tag = "2")]
    pub granted_tokens: ::core::option::Option<TokenBucket>,
    #[prost(int64, tag = "3")]
    pub trickle_time_ms: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Consumption {
    #[prost(double, tag = "1")]
    pub r_r_u: f64,
    #[prost(double, tag = "2")]
    pub w_r_u: f64,
    #[prost(double, tag = "3")]
    pub read_bytes: f64,
    #[prost(double, tag = "4")]
    pub write_bytes: f64,
    #[prost(double, tag = "5")]
    pub total_cpu_time_ms: f64,
    #[prost(double, tag = "6")]
    pub sql_layer_cpu_time_ms: f64,
    #[prost(double, tag = "7")]
    pub kv_read_rpc_count: f64,
    #[prost(double, tag = "8")]
    pub kv_write_rpc_count: f64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestUnitItem {
    #[prost(enumeration = "RequestUnitType", tag = "1")]
    pub r#type: i32,
    #[prost(double, tag = "2")]
    pub value: f64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RawResourceItem {
    #[prost(enumeration = "RawResourceType", tag = "1")]
    pub r#type: i32,
    #[prost(double, tag = "2")]
    pub value: f64,
}
/// ResourceGroup the settings definitions.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceGroup {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(enumeration = "GroupMode", tag = "2")]
    pub mode: i32,
    /// Used in RU mode, group settings with WRU/RRU etc resource abstract unit.
    #[prost(message, optional, tag = "3")]
    pub r_u_settings: ::core::option::Option<GroupRequestUnitSettings>,
    /// Used in Raw mode, group settings with CPU/IO etc resource unit.
    #[prost(message, optional, tag = "4")]
    pub raw_resource_settings: ::core::option::Option<GroupRawResourceSettings>,
    /// The task scheduling priority
    #[prost(uint32, tag = "5")]
    pub priority: u32,
    /// Runaway queries settings
    #[prost(message, optional, tag = "6")]
    pub runaway_settings: ::core::option::Option<RunawaySettings>,
    #[prost(message, optional, tag = "7")]
    pub background_settings: ::core::option::Option<BackgroundSettings>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GroupRequestUnitSettings {
    #[prost(message, optional, tag = "1")]
    pub r_u: ::core::option::Option<TokenBucket>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GroupRawResourceSettings {
    #[prost(message, optional, tag = "1")]
    pub cpu: ::core::option::Option<TokenBucket>,
    #[prost(message, optional, tag = "2")]
    pub io_read: ::core::option::Option<TokenBucket>,
    #[prost(message, optional, tag = "3")]
    pub io_write: ::core::option::Option<TokenBucket>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenBucket {
    #[prost(message, optional, tag = "1")]
    pub settings: ::core::option::Option<TokenLimitSettings>,
    /// Once used to reconfigure, the tokens is delta tokens.
    #[prost(double, tag = "2")]
    pub tokens: f64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenLimitSettings {
    #[prost(uint64, tag = "1")]
    pub fill_rate: u64,
    #[prost(int64, tag = "2")]
    pub burst_limit: i64,
    #[prost(double, tag = "3")]
    pub max_tokens: f64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(string, tag = "1")]
    pub message: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RunawayRule {
    #[prost(uint64, tag = "1")]
    pub exec_elapsed_time_ms: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RunawayWatch {
    /// how long would the watch last
    #[prost(int64, tag = "1")]
    pub lasting_duration_ms: i64,
    #[prost(enumeration = "RunawayWatchType", tag = "2")]
    pub r#type: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RunawaySettings {
    #[prost(message, optional, tag = "1")]
    pub rule: ::core::option::Option<RunawayRule>,
    #[prost(enumeration = "RunawayAction", tag = "2")]
    pub action: i32,
    #[prost(message, optional, tag = "3")]
    pub watch: ::core::option::Option<RunawayWatch>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BackgroundSettings {
    #[prost(string, repeated, tag = "1")]
    pub job_types: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Participant {
    /// name is the unique name of the resource manager participant.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// id is the unique id of the resource manager participant.
    #[prost(uint64, tag = "2")]
    pub id: u64,
    /// listen_urls is the serivce endpoint list in the url format.
    /// listen_urls\[0\] is primary service endpoint.
    #[prost(string, repeated, tag = "3")]
    pub listen_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RequestUnitType {
    Ru = 0,
}
impl RequestUnitType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RequestUnitType::Ru => "RU",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "RU" => Some(Self::Ru),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RawResourceType {
    Cpu = 0,
    IoReadFlow = 1,
    IoWriteFlow = 2,
}
impl RawResourceType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RawResourceType::Cpu => "CPU",
            RawResourceType::IoReadFlow => "IOReadFlow",
            RawResourceType::IoWriteFlow => "IOWriteFlow",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "CPU" => Some(Self::Cpu),
            "IOReadFlow" => Some(Self::IoReadFlow),
            "IOWriteFlow" => Some(Self::IoWriteFlow),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum GroupMode {
    Unknown = 0,
    RuMode = 1,
    RawMode = 2,
}
impl GroupMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            GroupMode::Unknown => "Unknown",
            GroupMode::RuMode => "RUMode",
            GroupMode::RawMode => "RawMode",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Unknown" => Some(Self::Unknown),
            "RUMode" => Some(Self::RuMode),
            "RawMode" => Some(Self::RawMode),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RunawayAction {
    NoneAction = 0,
    /// do nothing
    DryRun = 1,
    /// deprioritize the task
    CoolDown = 2,
    /// kill the task
    Kill = 3,
}
impl RunawayAction {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RunawayAction::NoneAction => "NoneAction",
            RunawayAction::DryRun => "DryRun",
            RunawayAction::CoolDown => "CoolDown",
            RunawayAction::Kill => "Kill",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "NoneAction" => Some(Self::NoneAction),
            "DryRun" => Some(Self::DryRun),
            "CoolDown" => Some(Self::CoolDown),
            "Kill" => Some(Self::Kill),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RunawayWatchType {
    NoneWatch = 0,
    Exact = 1,
    Similar = 2,
    Plan = 3,
}
impl RunawayWatchType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RunawayWatchType::NoneWatch => "NoneWatch",
            RunawayWatchType::Exact => "Exact",
            RunawayWatchType::Similar => "Similar",
            RunawayWatchType::Plan => "Plan",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "NoneWatch" => Some(Self::NoneWatch),
            "Exact" => Some(Self::Exact),
            "Similar" => Some(Self::Similar),
            "Plan" => Some(Self::Plan),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod resource_manager_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct ResourceManagerClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ResourceManagerClient<tonic::transport::Channel> {
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
    impl<T> ResourceManagerClient<T>
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
        ) -> ResourceManagerClient<InterceptedService<T, F>>
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
            ResourceManagerClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn list_resource_groups(
            &mut self,
            request: impl tonic::IntoRequest<super::ListResourceGroupsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListResourceGroupsResponse>,
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
                "/resource_manager.ResourceManager/ListResourceGroups",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "resource_manager.ResourceManager",
                        "ListResourceGroups",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_resource_group(
            &mut self,
            request: impl tonic::IntoRequest<super::GetResourceGroupRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetResourceGroupResponse>,
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
                "/resource_manager.ResourceManager/GetResourceGroup",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "resource_manager.ResourceManager",
                        "GetResourceGroup",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn add_resource_group(
            &mut self,
            request: impl tonic::IntoRequest<super::PutResourceGroupRequest>,
        ) -> std::result::Result<
            tonic::Response<super::PutResourceGroupResponse>,
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
                "/resource_manager.ResourceManager/AddResourceGroup",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "resource_manager.ResourceManager",
                        "AddResourceGroup",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn modify_resource_group(
            &mut self,
            request: impl tonic::IntoRequest<super::PutResourceGroupRequest>,
        ) -> std::result::Result<
            tonic::Response<super::PutResourceGroupResponse>,
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
                "/resource_manager.ResourceManager/ModifyResourceGroup",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "resource_manager.ResourceManager",
                        "ModifyResourceGroup",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete_resource_group(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteResourceGroupRequest>,
        ) -> std::result::Result<
            tonic::Response<super::DeleteResourceGroupResponse>,
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
                "/resource_manager.ResourceManager/DeleteResourceGroup",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "resource_manager.ResourceManager",
                        "DeleteResourceGroup",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn acquire_token_buckets(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::TokenBucketsRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::TokenBucketsResponse>>,
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
                "/resource_manager.ResourceManager/AcquireTokenBuckets",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "resource_manager.ResourceManager",
                        "AcquireTokenBuckets",
                    ),
                );
            self.inner.streaming(req, path, codec).await
        }
    }
}
