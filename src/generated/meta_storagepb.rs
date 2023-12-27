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
pub struct RequestHeader {
    /// cluster_id is the ID of the cluster which be sent to.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    /// source is the source of the request.
    #[prost(string, tag = "2")]
    pub source: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseHeader {
    /// cluster_id is the ID of the cluster which sent the response.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<Error>,
    #[prost(int64, tag = "3")]
    pub revision: i64,
}
/// copied part of <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/etcdserverpb/rpc.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub range_end: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "4")]
    pub start_revision: i64,
    #[prost(bool, tag = "5")]
    pub prev_kv: bool,
}
/// copied part of <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/etcdserverpb/rpc.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(int64, tag = "2")]
    pub compact_revision: i64,
    #[prost(message, repeated, tag = "3")]
    pub events: ::prost::alloc::vec::Vec<Event>,
}
/// copied part of <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/etcdserverpb/rpc.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub range_end: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "4")]
    pub limit: i64,
    #[prost(int64, tag = "5")]
    pub revision: i64,
}
/// copied part of <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/etcdserverpb/rpc.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, repeated, tag = "2")]
    pub kvs: ::prost::alloc::vec::Vec<KeyValue>,
    #[prost(bool, tag = "3")]
    pub more: bool,
    #[prost(int64, tag = "4")]
    pub count: i64,
}
/// copied part of <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/etcdserverpb/rpc.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutRequest {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<RequestHeader>,
    #[prost(bytes = "vec", tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub value: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag = "4")]
    pub lease: i64,
    #[prost(bool, tag = "5")]
    pub prev_kv: bool,
}
/// copied part of <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/etcdserverpb/rpc.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PutResponse {
    #[prost(message, optional, tag = "1")]
    pub header: ::core::option::Option<ResponseHeader>,
    #[prost(message, optional, tag = "2")]
    pub prev_kv: ::core::option::Option<KeyValue>,
}
/// copied from etcd <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/mvccpb/kv.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyValue {
    /// key is the key in bytes. An empty key is not allowed.
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    /// create_revision is the revision of last creation on this key.
    #[prost(int64, tag = "2")]
    pub create_revision: i64,
    /// mod_revision is the revision of last modification on this key.
    #[prost(int64, tag = "3")]
    pub mod_revision: i64,
    /// version is the version of the key. A deletion resets
    /// the version to zero and any modification of the key
    /// increases its version.
    #[prost(int64, tag = "4")]
    pub version: i64,
    /// value is the value held by the key, in bytes.
    #[prost(bytes = "vec", tag = "5")]
    pub value: ::prost::alloc::vec::Vec<u8>,
    /// lease is the ID of the lease that attached to key.
    /// When the attached lease expires, the key will be deleted.
    /// If lease is 0, then no lease is attached to the key.
    #[prost(int64, tag = "6")]
    pub lease: i64,
}
/// copied from etcd <https://github.com/etcd-io/etcd/blob/7dfd29b0cc7ce25337276dce646ca2a65aa44b4d/api/mvccpb/kv.proto>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Event {
    /// type is the kind of event. If type is a PUT, it indicates
    /// new data has been stored to the key. If type is a DELETE,
    /// it indicates the key was deleted.
    #[prost(enumeration = "event::EventType", tag = "1")]
    pub r#type: i32,
    /// kv holds the KeyValue for the event.
    /// A PUT event contains current kv pair.
    /// A PUT event with kv.Version=1 indicates the creation of a key.
    /// A DELETE/EXPIRE event contains the deleted key with
    /// its modification revision set to the revision of deletion.
    #[prost(message, optional, tag = "2")]
    pub kv: ::core::option::Option<KeyValue>,
    /// prev_kv holds the key-value pair before the event happens.
    #[prost(message, optional, tag = "3")]
    pub prev_kv: ::core::option::Option<KeyValue>,
}
/// Nested message and enum types in `Event`.
pub mod event {
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
    pub enum EventType {
        Put = 0,
        Delete = 1,
    }
    impl EventType {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                EventType::Put => "PUT",
                EventType::Delete => "DELETE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "PUT" => Some(Self::Put),
                "DELETE" => Some(Self::Delete),
                _ => None,
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorType {
    Ok = 0,
    Unknown = 1,
    /// required watch revision is smaller than current compact/min revision.
    DataCompacted = 2,
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
            ErrorType::DataCompacted => "DATA_COMPACTED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "OK" => Some(Self::Ok),
            "UNKNOWN" => Some(Self::Unknown),
            "DATA_COMPACTED" => Some(Self::DataCompacted),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod meta_storage_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// MetaStorage is the meta storage service.
    #[derive(Debug, Clone)]
    pub struct MetaStorageClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl MetaStorageClient<tonic::transport::Channel> {
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
    impl<T> MetaStorageClient<T>
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
        ) -> MetaStorageClient<InterceptedService<T, F>>
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
            MetaStorageClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn watch(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::WatchResponse>>,
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
                "/meta_storagepb.MetaStorage/Watch",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("meta_storagepb.MetaStorage", "Watch"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// Get is the same as etcd Range which might be implemented in a more common way
        /// so that we can use other storages to replace etcd in the future.
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
            let path = http::uri::PathAndQuery::from_static(
                "/meta_storagepb.MetaStorage/Get",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("meta_storagepb.MetaStorage", "Get"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn put(
            &mut self,
            request: impl tonic::IntoRequest<super::PutRequest>,
        ) -> std::result::Result<tonic::Response<super::PutResponse>, tonic::Status> {
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
                "/meta_storagepb.MetaStorage/Put",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("meta_storagepb.MetaStorage", "Put"));
            self.inner.unary(req, path, codec).await
        }
    }
}
