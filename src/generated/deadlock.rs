#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WaitForEntriesRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WaitForEntriesResponse {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<WaitForEntry>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WaitForEntry {
    /// The transaction id that is waiting.
    #[prost(uint64, tag = "1")]
    pub txn: u64,
    /// The transaction id that is being waited for.
    #[prost(uint64, tag = "2")]
    pub wait_for_txn: u64,
    /// The hash value of the key is being waited for.
    #[prost(uint64, tag = "3")]
    pub key_hash: u64,
    /// The key the current txn is trying to lock.
    #[prost(bytes = "vec", tag = "4")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    /// The tag came from the lock request's context.
    #[prost(bytes = "vec", tag = "5")]
    pub resource_group_tag: ::prost::alloc::vec::Vec<u8>,
    /// Milliseconds it has been waits.
    #[prost(uint64, tag = "6")]
    pub wait_time: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeadlockRequest {
    #[prost(enumeration = "DeadlockRequestType", tag = "1")]
    pub tp: i32,
    #[prost(message, optional, tag = "2")]
    pub entry: ::core::option::Option<WaitForEntry>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeadlockResponse {
    /// The same entry sent by DeadlockRequest, identifies the sender.
    #[prost(message, optional, tag = "1")]
    pub entry: ::core::option::Option<WaitForEntry>,
    /// The key hash of the lock that is hold by the waiting transaction.
    #[prost(uint64, tag = "2")]
    pub deadlock_key_hash: u64,
    /// The other entries of the dead lock circle. The current entry is in `entry` field and  not
    /// included in this field.
    #[prost(message, repeated, tag = "3")]
    pub wait_chain: ::prost::alloc::vec::Vec<WaitForEntry>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DeadlockRequestType {
    Detect = 0,
    /// CleanUpWaitFor cleans a single entry the transaction is waiting.
    CleanUpWaitFor = 1,
    /// CleanUp cleans all entries the transaction is waiting.
    CleanUp = 2,
}
impl DeadlockRequestType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DeadlockRequestType::Detect => "Detect",
            DeadlockRequestType::CleanUpWaitFor => "CleanUpWaitFor",
            DeadlockRequestType::CleanUp => "CleanUp",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Detect" => Some(Self::Detect),
            "CleanUpWaitFor" => Some(Self::CleanUpWaitFor),
            "CleanUp" => Some(Self::CleanUp),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod deadlock_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct DeadlockClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl DeadlockClient<tonic::transport::Channel> {
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
    impl<T> DeadlockClient<T>
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
        ) -> DeadlockClient<InterceptedService<T, F>>
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
            DeadlockClient::new(InterceptedService::new(inner, interceptor))
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
        /// Get local wait for entries, should be handle by every node.
        /// The owner should sent this request to all members to build the complete wait for graph.
        pub async fn get_wait_for_entries(
            &mut self,
            request: impl tonic::IntoRequest<super::WaitForEntriesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::WaitForEntriesResponse>,
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
                "/deadlock.Deadlock/GetWaitForEntries",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("deadlock.Deadlock", "GetWaitForEntries"));
            self.inner.unary(req, path, codec).await
        }
        /// Detect should only sent to the owner. only be handled by the owner.
        /// The DeadlockResponse is sent back only if there is deadlock detected.
        /// CleanUpWaitFor and CleanUp doesn't return responses.
        pub async fn detect(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::DeadlockRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::DeadlockResponse>>,
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
            let path = http::uri::PathAndQuery::from_static("/deadlock.Deadlock/Detect");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("deadlock.Deadlock", "Detect"));
            self.inner.streaming(req, path, codec).await
        }
    }
}
