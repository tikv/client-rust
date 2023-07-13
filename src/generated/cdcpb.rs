#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Header {
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(string, tag = "2")]
    pub ticdc_version: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DuplicateRequest {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Compatibility {
    #[prost(string, tag = "1")]
    pub required_version: ::prost::alloc::string::String,
}
/// ClusterIDMismatch is an error variable that
/// tells people that the cluster ID of the request does not match the TiKV cluster ID.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterIdMismatch {
    /// The current tikv cluster ID.
    #[prost(uint64, tag = "1")]
    pub current: u64,
    /// The cluster ID of the TiCDC request.
    #[prost(uint64, tag = "2")]
    pub request: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(message, optional, tag = "1")]
    pub not_leader: ::core::option::Option<super::errorpb::NotLeader>,
    #[prost(message, optional, tag = "2")]
    pub region_not_found: ::core::option::Option<super::errorpb::RegionNotFound>,
    #[prost(message, optional, tag = "3")]
    pub epoch_not_match: ::core::option::Option<super::errorpb::EpochNotMatch>,
    #[prost(message, optional, tag = "4")]
    pub duplicate_request: ::core::option::Option<DuplicateRequest>,
    #[prost(message, optional, tag = "5")]
    pub compatibility: ::core::option::Option<Compatibility>,
    #[prost(message, optional, tag = "6")]
    pub cluster_id_mismatch: ::core::option::Option<ClusterIdMismatch>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TxnInfo {
    #[prost(uint64, tag = "1")]
    pub start_ts: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub primary: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TxnStatus {
    #[prost(uint64, tag = "1")]
    pub start_ts: u64,
    #[prost(uint64, tag = "2")]
    pub min_commit_ts: u64,
    #[prost(uint64, tag = "3")]
    pub commit_ts: u64,
    #[prost(bool, tag = "4")]
    pub is_rolled_back: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Event {
    #[prost(uint64, tag = "1")]
    pub region_id: u64,
    #[prost(uint64, tag = "2")]
    pub index: u64,
    #[prost(uint64, tag = "7")]
    pub request_id: u64,
    #[prost(oneof = "event::Event", tags = "3, 4, 5, 6, 8")]
    pub event: ::core::option::Option<event::Event>,
}
/// Nested message and enum types in `Event`.
pub mod event {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Row {
        #[prost(uint64, tag = "1")]
        pub start_ts: u64,
        #[prost(uint64, tag = "2")]
        pub commit_ts: u64,
        #[prost(enumeration = "LogType", tag = "3")]
        pub r#type: i32,
        #[prost(enumeration = "row::OpType", tag = "4")]
        pub op_type: i32,
        #[prost(bytes = "vec", tag = "5")]
        pub key: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "6")]
        pub value: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "7")]
        pub old_value: ::prost::alloc::vec::Vec<u8>,
    }
    /// Nested message and enum types in `Row`.
    pub mod row {
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
        pub enum OpType {
            Unknown = 0,
            Put = 1,
            Delete = 2,
        }
        impl OpType {
            /// String value of the enum field names used in the ProtoBuf definition.
            ///
            /// The values are not transformed in any way and thus are considered stable
            /// (if the ProtoBuf definition does not change) and safe for programmatic use.
            pub fn as_str_name(&self) -> &'static str {
                match self {
                    OpType::Unknown => "UNKNOWN",
                    OpType::Put => "PUT",
                    OpType::Delete => "DELETE",
                }
            }
            /// Creates an enum from field names used in the ProtoBuf definition.
            pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
                match value {
                    "UNKNOWN" => Some(Self::Unknown),
                    "PUT" => Some(Self::Put),
                    "DELETE" => Some(Self::Delete),
                    _ => None,
                }
            }
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Entries {
        #[prost(message, repeated, tag = "1")]
        pub entries: ::prost::alloc::vec::Vec<Row>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Admin {
        #[prost(message, optional, tag = "1")]
        pub admin_request: ::core::option::Option<
            super::super::raft_cmdpb::AdminRequest,
        >,
        #[prost(message, optional, tag = "2")]
        pub admin_response: ::core::option::Option<
            super::super::raft_cmdpb::AdminResponse,
        >,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct LongTxn {
        #[prost(message, repeated, tag = "1")]
        pub txn_info: ::prost::alloc::vec::Vec<super::TxnInfo>,
    }
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
    pub enum LogType {
        Unknown = 0,
        Prewrite = 1,
        Commit = 2,
        Rollback = 3,
        Committed = 4,
        Initialized = 5,
    }
    impl LogType {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                LogType::Unknown => "UNKNOWN",
                LogType::Prewrite => "PREWRITE",
                LogType::Commit => "COMMIT",
                LogType::Rollback => "ROLLBACK",
                LogType::Committed => "COMMITTED",
                LogType::Initialized => "INITIALIZED",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN" => Some(Self::Unknown),
                "PREWRITE" => Some(Self::Prewrite),
                "COMMIT" => Some(Self::Commit),
                "ROLLBACK" => Some(Self::Rollback),
                "COMMITTED" => Some(Self::Committed),
                "INITIALIZED" => Some(Self::Initialized),
                _ => None,
            }
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "3")]
        Entries(Entries),
        #[prost(message, tag = "4")]
        Admin(Admin),
        #[prost(message, tag = "5")]
        Error(super::Error),
        #[prost(uint64, tag = "6")]
        ResolvedTs(u64),
        /// Note that field 7 is taken by request_id.
        ///
        /// More region level events ...
        #[prost(message, tag = "8")]
        LongTxn(LongTxn),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangeDataEvent {
    #[prost(message, repeated, tag = "1")]
    pub events: ::prost::alloc::vec::Vec<Event>,
    /// More store level events ...
    #[prost(message, optional, tag = "2")]
    pub resolved_ts: ::core::option::Option<ResolvedTs>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResolvedTs {
    #[prost(uint64, repeated, tag = "1")]
    pub regions: ::prost::alloc::vec::Vec<u64>,
    #[prost(uint64, tag = "2")]
    pub ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangeDataRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<Header>,
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    #[prost(message, optional, tag = "3")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(uint64, tag = "4")]
    pub checkpoint_ts: u64,
    #[prost(bytes = "vec", tag = "5")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    /// Used for CDC to identify events corresponding to different requests.
    #[prost(uint64, tag = "7")]
    pub request_id: u64,
    #[prost(enumeration = "super::kvrpcpb::ExtraOp", tag = "8")]
    pub extra_op: i32,
    #[prost(oneof = "change_data_request::Request", tags = "9, 10")]
    pub request: ::core::option::Option<change_data_request::Request>,
}
/// Nested message and enum types in `ChangeDataRequest`.
pub mod change_data_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Register {}
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct NotifyTxnStatus {
        #[prost(message, repeated, tag = "1")]
        pub txn_status: ::prost::alloc::vec::Vec<super::TxnStatus>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        /// A normal request that trying to register change data feed on a region.
        #[prost(message, tag = "9")]
        Register(Register),
        /// Notify the region that some of the running transactions on the region has a pushed
        /// min_commit_ts so that the resolved_ts can be advanced.
        #[prost(message, tag = "10")]
        NotifyTxnStatus(NotifyTxnStatus),
    }
}
/// Generated client implementations.
pub mod change_data_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct ChangeDataClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ChangeDataClient<tonic::transport::Channel> {
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
    impl<T> ChangeDataClient<T>
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
        ) -> ChangeDataClient<InterceptedService<T, F>>
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
            ChangeDataClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn event_feed(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::ChangeDataRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ChangeDataEvent>>,
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
                "/cdcpb.ChangeData/EventFeed",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("cdcpb.ChangeData", "EventFeed"));
            self.inner.streaming(req, path, codec).await
        }
    }
}
