#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SwitchModeRequest {
    #[prost(enumeration = "SwitchMode", tag = "1")]
    pub mode: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SwitchModeResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Range {
    #[prost(bytes = "vec", tag = "1")]
    pub start: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub end: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SstMeta {
    #[prost(bytes = "vec", tag = "1")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub range: ::core::option::Option<Range>,
    #[prost(uint32, tag = "3")]
    pub crc32: u32,
    #[prost(uint64, tag = "4")]
    pub length: u64,
    #[prost(string, tag = "5")]
    pub cf_name: ::prost::alloc::string::String,
    #[prost(uint64, tag = "6")]
    pub region_id: u64,
    #[prost(message, optional, tag = "7")]
    pub region_epoch: ::core::option::Option<super::metapb::RegionEpoch>,
    #[prost(bool, tag = "8")]
    pub end_key_exclusive: bool,
    /// total_kvs and total_bytes is equivalent to PD's approximate_keys and approximate_size
    /// set these values can save time from tikv upload keys and size to PD through Heartbeat.
    #[prost(uint64, tag = "9")]
    pub total_kvs: u64,
    #[prost(uint64, tag = "10")]
    pub total_bytes: u64,
    /// API version implies the encode of the key and value.
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "11")]
    pub api_version: i32,
    /// cipher_iv is used to encrypt/decrypt sst
    #[prost(bytes = "vec", tag = "12")]
    pub cipher_iv: ::prost::alloc::vec::Vec<u8>,
}
/// A rewrite rule is applied on the *encoded* keys (the internal storage
/// representation).
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RewriteRule {
    #[prost(bytes = "vec", tag = "1")]
    pub old_key_prefix: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub new_key_prefix: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub new_timestamp: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadRequest {
    #[prost(oneof = "upload_request::Chunk", tags = "1, 2")]
    pub chunk: ::core::option::Option<upload_request::Chunk>,
}
/// Nested message and enum types in `UploadRequest`.
pub mod upload_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Chunk {
        #[prost(message, tag = "1")]
        Meta(super::SstMeta),
        #[prost(bytes, tag = "2")]
        Data(::prost::alloc::vec::Vec<u8>),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngestRequest {
    #[prost(message, optional, tag = "1")]
    pub context: ::core::option::Option<super::kvrpcpb::Context>,
    #[prost(message, optional, tag = "2")]
    pub sst: ::core::option::Option<SstMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MultiIngestRequest {
    #[prost(message, optional, tag = "1")]
    pub context: ::core::option::Option<super::kvrpcpb::Context>,
    #[prost(message, repeated, tag = "2")]
    pub ssts: ::prost::alloc::vec::Vec<SstMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngestResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<super::errorpb::Error>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactRequest {
    /// Compact files in the range and above the output level.
    /// Compact all files if the range is not specified.
    /// Compact all files to the bottommost level if the output level is -1.
    #[prost(message, optional, tag = "1")]
    pub range: ::core::option::Option<Range>,
    #[prost(int32, tag = "2")]
    pub output_level: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompactResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DownloadRequest {
    /// The SST meta used to identify the downloaded file.
    /// Must be the same among all nodes in the same Raft group.
    /// Note: the "crc32" and "cf_name" fields are ignored in this request,
    /// and the "range" field represents the closed key range after rewrite
    /// (as origin keys in encoded representation).
    #[prost(message, optional, tag = "2")]
    pub sst: ::core::option::Option<SstMeta>,
    /// The file name of the SST file.
    #[prost(string, tag = "9")]
    pub name: ::prost::alloc::string::String,
    /// Performs a key prefix rewrite after downloading the SST file.
    /// All keys in the SST will be rewritten as:
    ///
    ///   new_key = new_key_prefix + old_key\[len(old_key_prefix)..\]
    ///
    /// When used for TiDB, rewriting the prefix changes the table ID. Please
    /// note that key-rewrite is applied on the origin keys in encoded
    /// representation (the SST itself should still use data keys in encoded
    /// representation).
    ///
    /// You need to ensure that the keys before and after rewriting are in the
    /// same order, otherwise the RPC request will fail.
    #[prost(message, optional, tag = "13")]
    pub rewrite_rule: ::core::option::Option<RewriteRule>,
    #[prost(message, optional, tag = "14")]
    pub storage_backend: ::core::option::Option<super::backup::StorageBackend>,
    #[prost(bool, tag = "15")]
    pub is_raw_kv: bool,
    /// cipher_info is used to decrypt sst when download sst
    #[prost(message, optional, tag = "16")]
    pub cipher_info: ::core::option::Option<super::backup::CipherInfo>,
}
/// For now it is just used for distinguishing the error of the request with the error
/// of gRPC, add more concrete types if it is necessary later.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(string, tag = "1")]
    pub message: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DownloadResponse {
    /// The actual key range (after rewrite) of the downloaded SST. The range is
    /// inclusive in both ends.
    #[prost(message, optional, tag = "1")]
    pub range: ::core::option::Option<Range>,
    /// Whether the SST is empty. An empty SST is prohibited in TiKV, do not
    /// ingest if this field is true.
    /// (Deprecated, should be replaced by checking `length == 0` in the future)
    #[prost(bool, tag = "2")]
    pub is_empty: bool,
    #[prost(message, optional, tag = "3")]
    pub error: ::core::option::Option<Error>,
    /// The CRC32 checksum of the rewritten SST file (implementation can return
    /// zero, indicating the CRC32 was not calculated).
    #[prost(uint32, tag = "4")]
    pub crc32: u32,
    /// The actual length of the rewritten SST file.
    #[prost(uint64, tag = "5")]
    pub length: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetDownloadSpeedLimitRequest {
    /// The download speed limit (bytes/second). Set to 0 for unlimited speed.
    #[prost(uint64, tag = "1")]
    pub speed_limit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetDownloadSpeedLimitResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Pair {
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration = "pair::Op", tag = "3")]
    pub op: i32,
}
/// Nested message and enum types in `Pair`.
pub mod pair {
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
    pub enum Op {
        Put = 0,
        Delete = 1,
    }
    impl Op {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Op::Put => "Put",
                Op::Delete => "Delete",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "Put" => Some(Self::Put),
                "Delete" => Some(Self::Delete),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteBatch {
    #[prost(uint64, tag = "1")]
    pub commit_ts: u64,
    #[prost(message, repeated, tag = "2")]
    pub pairs: ::prost::alloc::vec::Vec<Pair>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteRequest {
    #[prost(oneof = "write_request::Chunk", tags = "1, 2")]
    pub chunk: ::core::option::Option<write_request::Chunk>,
}
/// Nested message and enum types in `WriteRequest`.
pub mod write_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Chunk {
        #[prost(message, tag = "1")]
        Meta(super::SstMeta),
        #[prost(message, tag = "2")]
        Batch(super::WriteBatch),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, repeated, tag = "2")]
    pub metas: ::prost::alloc::vec::Vec<SstMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RawWriteBatch {
    #[prost(uint64, tag = "1")]
    pub ttl: u64,
    #[prost(message, repeated, tag = "2")]
    pub pairs: ::prost::alloc::vec::Vec<Pair>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RawWriteRequest {
    #[prost(oneof = "raw_write_request::Chunk", tags = "1, 2")]
    pub chunk: ::core::option::Option<raw_write_request::Chunk>,
}
/// Nested message and enum types in `RawWriteRequest`.
pub mod raw_write_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Chunk {
        #[prost(message, tag = "1")]
        Meta(super::SstMeta),
        #[prost(message, tag = "2")]
        Batch(super::RawWriteBatch),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RawWriteResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, repeated, tag = "2")]
    pub metas: ::prost::alloc::vec::Vec<SstMeta>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DuplicateDetectRequest {
    #[prost(message, optional, tag = "1")]
    pub context: ::core::option::Option<super::kvrpcpb::Context>,
    #[prost(bytes = "vec", tag = "2")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    /// Return only the keys found by scanning, not their values.
    #[prost(bool, tag = "4")]
    pub key_only: bool,
    /// We only check the data whose timestamp is larger than `min_commit_ts`. `min_commit_ts` is exclueded.
    #[prost(uint64, tag = "5")]
    pub min_commit_ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KvPair {
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub commit_ts: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DuplicateDetectResponse {
    #[prost(message, optional, tag = "1")]
    pub region_error: ::core::option::Option<super::errorpb::Error>,
    #[prost(message, optional, tag = "2")]
    pub key_error: ::core::option::Option<Error>,
    /// The these keys will be in asc order (but commit time is in desc order),
    ///   and the content is just like following:
    /// [
    ///    {key: "key1", value: "value11", commit_ts: 1005},
    ///    {key: "key1", value: "value12", commit_ts: 1004},
    ///    {key: "key1", value: "value13", commit_ts: 1001},
    ///    {key: "key2", value: "value21", commit_ts: 1004},
    ///    {key: "key2", value: "value22", commit_ts: 1002},
    ///    ...
    /// ]
    #[prost(message, repeated, tag = "3")]
    pub pairs: ::prost::alloc::vec::Vec<KvPair>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SwitchMode {
    Normal = 0,
    Import = 1,
}
impl SwitchMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            SwitchMode::Normal => "Normal",
            SwitchMode::Import => "Import",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Normal" => Some(Self::Normal),
            "Import" => Some(Self::Import),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod import_sst_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// ImportSST provides a service to import a generated SST file to a region in TiKV.
    ///
    /// In order to import an SST file to a region, the user should:
    /// 1. Retrieve the meta of the region according to the SST file's range.
    /// 2. Upload the SST file to the servers where the region's peers locate in.
    /// 3. Issue an ingest request to the region's leader with the SST file's metadata.
    ///
    /// It's the user's responsibility to make sure that the SST file is uploaded to
    /// the servers where the region's peers locate in, before issue the ingest
    /// request to the region's leader. However, the region can be scheduled (so the
    /// location of the region's peers will be changed) or split/merged (so the range
    /// of the region will be changed), after the SST file is uploaded, but before
    /// the SST file is ingested. So, the region's epoch is provided in the SST
    /// file's metadata, to guarantee that the region's epoch must be the same
    /// between the SST file is uploaded and ingested later.
    #[derive(Debug, Clone)]
    pub struct ImportSstClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ImportSstClient<tonic::transport::Channel> {
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
    impl<T> ImportSstClient<T>
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
        ) -> ImportSstClient<InterceptedService<T, F>>
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
            ImportSstClient::new(InterceptedService::new(inner, interceptor))
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
        /// Switch to normal/import mode.
        pub async fn switch_mode(
            &mut self,
            request: impl tonic::IntoRequest<super::SwitchModeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SwitchModeResponse>,
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
                "/import_sstpb.ImportSST/SwitchMode",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "SwitchMode"));
            self.inner.unary(req, path, codec).await
        }
        /// Upload an SST file to a server.
        pub async fn upload(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::UploadRequest>,
        ) -> std::result::Result<tonic::Response<super::UploadResponse>, tonic::Status> {
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
                "/import_sstpb.ImportSST/Upload",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "Upload"));
            self.inner.client_streaming(req, path, codec).await
        }
        /// Ingest an uploaded SST file to a region.
        pub async fn ingest(
            &mut self,
            request: impl tonic::IntoRequest<super::IngestRequest>,
        ) -> std::result::Result<tonic::Response<super::IngestResponse>, tonic::Status> {
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
                "/import_sstpb.ImportSST/Ingest",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "Ingest"));
            self.inner.unary(req, path, codec).await
        }
        /// Compact the specific range for better performance.
        pub async fn compact(
            &mut self,
            request: impl tonic::IntoRequest<super::CompactRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CompactResponse>,
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
                "/import_sstpb.ImportSST/Compact",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "Compact"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn set_download_speed_limit(
            &mut self,
            request: impl tonic::IntoRequest<super::SetDownloadSpeedLimitRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SetDownloadSpeedLimitResponse>,
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
                "/import_sstpb.ImportSST/SetDownloadSpeedLimit",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("import_sstpb.ImportSST", "SetDownloadSpeedLimit"),
                );
            self.inner.unary(req, path, codec).await
        }
        /// Download an SST file from an external storage, and performs key-rewrite
        /// after downloading.
        pub async fn download(
            &mut self,
            request: impl tonic::IntoRequest<super::DownloadRequest>,
        ) -> std::result::Result<
            tonic::Response<super::DownloadResponse>,
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
                "/import_sstpb.ImportSST/Download",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "Download"));
            self.inner.unary(req, path, codec).await
        }
        /// Open a write stream to generate sst files
        pub async fn write(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::WriteRequest>,
        ) -> std::result::Result<tonic::Response<super::WriteResponse>, tonic::Status> {
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
                "/import_sstpb.ImportSST/Write",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "Write"));
            self.inner.client_streaming(req, path, codec).await
        }
        pub async fn raw_write(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::RawWriteRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RawWriteResponse>,
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
                "/import_sstpb.ImportSST/RawWrite",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "RawWrite"));
            self.inner.client_streaming(req, path, codec).await
        }
        /// Ingest Multiple files in one request
        pub async fn multi_ingest(
            &mut self,
            request: impl tonic::IntoRequest<super::MultiIngestRequest>,
        ) -> std::result::Result<tonic::Response<super::IngestResponse>, tonic::Status> {
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
                "/import_sstpb.ImportSST/MultiIngest",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "MultiIngest"));
            self.inner.unary(req, path, codec).await
        }
        /// Collect duplicate data from TiKV.
        pub async fn duplicate_detect(
            &mut self,
            request: impl tonic::IntoRequest<super::DuplicateDetectRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::DuplicateDetectResponse>>,
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
                "/import_sstpb.ImportSST/DuplicateDetect",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("import_sstpb.ImportSST", "DuplicateDetect"));
            self.inner.server_streaming(req, path, codec).await
        }
    }
}
