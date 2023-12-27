#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestHeader {
    /// cluster_id is the ID of the cluster which be sent to.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    /// sender_id is the ID of the sender server.
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
pub struct Participant {
    /// name is the unique name of the scheduling participant.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// id is the unique id of the scheduling participant.
    #[prost(uint64, tag = "2")]
    pub id: u64,
    /// listen_urls is the serivce endpoint list in the url format.
    /// listen_urls\[0\] is primary service endpoint.
    #[prost(string, repeated, tag = "3")]
    pub listen_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreHeartbeatRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub stats: ::core::option::Option<super::pdpb::StoreStats>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreHeartbeatResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(string, tag = "2")]
    pub cluster_version: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionHeartbeatRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    /// Leader Peer sending the heartbeat.
    #[prost(message, optional, tag = "3")]
    pub leader: ::core::option::Option<super::metapb::Peer>,
    /// Term is the term of raft group.
    #[prost(uint64, tag = "4")]
    pub term: u64,
    /// Leader considers that these peers are down.
    #[prost(message, repeated, tag = "5")]
    pub down_peers: ::prost::alloc::vec::Vec<super::pdpb::PeerStats>,
    /// Pending peers are the peers that the leader can't consider as
    /// working followers.
    #[prost(message, repeated, tag = "6")]
    pub pending_peers: ::prost::alloc::vec::Vec<super::metapb::Peer>,
    /// Bytes read/written during this period.
    #[prost(uint64, tag = "7")]
    pub bytes_written: u64,
    #[prost(uint64, tag = "8")]
    pub bytes_read: u64,
    /// Keys read/written during this period.
    #[prost(uint64, tag = "9")]
    pub keys_written: u64,
    #[prost(uint64, tag = "10")]
    pub keys_read: u64,
    /// Approximate region size.
    #[prost(uint64, tag = "11")]
    pub approximate_size: u64,
    /// Approximate number of keys.
    #[prost(uint64, tag = "12")]
    pub approximate_keys: u64,
    /// QueryStats reported write query stats, and there are read query stats in store heartbeat
    #[prost(message, optional, tag = "13")]
    pub query_stats: ::core::option::Option<super::pdpb::QueryStats>,
    /// Actually reported time interval
    #[prost(message, optional, tag = "14")]
    pub interval: ::core::option::Option<super::pdpb::TimeInterval>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegionHeartbeatResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    /// ID of the region
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    #[prost(message, optional, tag = "3")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    /// Leader of the region at the moment of the corresponding request was made.
    #[prost(message, optional, tag = "4")]
    pub target_peer: ::core::option::Option<super::metapb::Peer>,
    /// Notice, Pd only allows handling reported epoch >= current pd's.
    /// Leader peer reports region status with RegionHeartbeatRequest
    /// to pd regularly, pd will determine whether this region
    /// should do ChangePeer or not.
    /// E,g, max peer number is 3, region A, first only peer 1 in A.
    ///
    /// 1. Pd region state -> Peers (1), ConfVer (1).
    /// 1. Leader peer 1 reports region state to pd, pd finds the
    ///    peer number is \< 3, so first changes its current region
    ///    state -> Peers (1, 2), ConfVer (1), and returns ChangePeer Adding 2.
    /// 1. Leader does ChangePeer, then reports Peers (1, 2), ConfVer (2),
    ///    pd updates its state -> Peers (1, 2), ConfVer (2).
    /// 1. Leader may report old Peers (1), ConfVer (1) to pd before ConfChange
    ///    finished, pd stills responses ChangePeer Adding 2, of course, we must
    ///    guarantee the second ChangePeer can't be applied in TiKV.
    #[prost(message, optional, tag = "5")]
    pub change_peer: ::core::option::Option<super::pdpb::ChangePeer>,
    /// Pd can return transfer_leader to let TiKV does leader transfer itself.
    #[prost(message, optional, tag = "6")]
    pub transfer_leader: ::core::option::Option<super::pdpb::TransferLeader>,
    #[prost(message, optional, tag = "7")]
    pub merge: ::core::option::Option<super::pdpb::Merge>,
    /// PD sends split_region to let TiKV split a region into two regions.
    #[prost(message, optional, tag = "8")]
    pub split_region: ::core::option::Option<super::pdpb::SplitRegion>,
    /// Multiple change peer operations atomically.
    /// Note: PD can use both ChangePeer and ChangePeerV2 at the same time
    /// (not in the same RegionHeartbeatResponse).
    /// Now, PD use ChangePeerV2 in following scenarios:
    /// 1. replacing peers
    /// 2. demoting voter directly
    #[prost(message, optional, tag = "9")]
    pub change_peer_v2: ::core::option::Option<super::pdpb::ChangePeerV2>,
    #[prost(message, optional, tag = "10")]
    pub switch_witnesses: ::core::option::Option<super::pdpb::BatchSwitchWitness>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScatterRegionsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    /// If group is defined, the regions with the same group would be scattered as a whole group.
    /// If not defined, the regions would be scattered in a cluster level.
    #[prost(string, tag = "2")]
    pub group: ::prost::alloc::string::String,
    /// If regions_id is defined, the region_id would be ignored.
    #[prost(uint64, repeated, tag = "3")]
    pub regions_id: ::prost::alloc::vec::Vec<u64>,
    #[prost(uint64, tag = "4")]
    pub retry_limit: u64,
    #[prost(bool, tag = "5")]
    pub skip_store_limit: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScatterRegionsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub finished_percentage: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitRegionsRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub split_keys: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint64, tag = "3")]
    pub retry_limit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SplitRegionsResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub finished_percentage: u64,
    #[prost(uint64, repeated, tag = "3")]
    pub regions_id: ::prost::alloc::vec::Vec<u64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperatorRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperatorResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(uint64, tag = "2")]
    pub region_id: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub desc: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration = "super::pdpb::OperatorStatus", tag = "4")]
    pub status: i32,
    #[prost(bytes = "vec", tag = "5")]
    pub kind: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AskBatchSplitRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(uint32, tag = "3")]
    pub split_count: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AskBatchSplitResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub ids: ::prost::alloc::vec::Vec<super::pdpb::SplitId>,
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
pub mod scheduling_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct SchedulingClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl SchedulingClient<tonic::transport::Channel> {
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
    impl<T> SchedulingClient<T>
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
        ) -> SchedulingClient<InterceptedService<T, F>>
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
            SchedulingClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn store_heartbeat(
            &mut self,
            request: impl tonic::IntoRequest<super::StoreHeartbeatRequest>,
        ) -> std::result::Result<
            tonic::Response<super::StoreHeartbeatResponse>,
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
                "/schedulingpb.Scheduling/StoreHeartbeat",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("schedulingpb.Scheduling", "StoreHeartbeat"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn region_heartbeat(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::RegionHeartbeatRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::RegionHeartbeatResponse>>,
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
                "/schedulingpb.Scheduling/RegionHeartbeat",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("schedulingpb.Scheduling", "RegionHeartbeat"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn split_regions(
            &mut self,
            request: impl tonic::IntoRequest<super::SplitRegionsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SplitRegionsResponse>,
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
                "/schedulingpb.Scheduling/SplitRegions",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("schedulingpb.Scheduling", "SplitRegions"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn scatter_regions(
            &mut self,
            request: impl tonic::IntoRequest<super::ScatterRegionsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ScatterRegionsResponse>,
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
                "/schedulingpb.Scheduling/ScatterRegions",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("schedulingpb.Scheduling", "ScatterRegions"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_operator(
            &mut self,
            request: impl tonic::IntoRequest<super::GetOperatorRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetOperatorResponse>,
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
                "/schedulingpb.Scheduling/GetOperator",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("schedulingpb.Scheduling", "GetOperator"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn ask_batch_split(
            &mut self,
            request: impl tonic::IntoRequest<super::AskBatchSplitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AskBatchSplitResponse>,
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
                "/schedulingpb.Scheduling/AskBatchSplit",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("schedulingpb.Scheduling", "AskBatchSplit"));
            self.inner.unary(req, path, codec).await
        }
    }
}
