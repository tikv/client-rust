/// The message save the metadata of a backup.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BackupMeta {
    /// ID and version of backuped cluster.
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(string, tag = "2")]
    pub cluster_version: ::prost::alloc::string::String,
    /// Save the version of BR running backup jobs.
    #[prost(string, tag = "11")]
    pub br_version: ::prost::alloc::string::String,
    /// The backupmeta scheme version.
    #[prost(int32, tag = "12")]
    pub version: i32,
    /// A set of files that compose a backup.
    /// Note: `files` is deprecated, as it bloats backupmeta. It is kept for
    /// compatibility, so new BR can restore older backups.
    #[prost(message, repeated, tag = "4")]
    pub files: ::prost::alloc::vec::Vec<File>,
    /// An index to files contains data files.
    #[prost(message, optional, tag = "13")]
    pub file_index: ::core::option::Option<MetaFile>,
    /// A pair of timestamp specifies a time range of a backup.
    /// For full backup, the start_version equals to the end_version,
    /// it means point in time.
    /// For incremental backup, the time range is specified as
    /// (start_version, end_version\].
    #[prost(uint64, tag = "5")]
    pub start_version: u64,
    #[prost(uint64, tag = "6")]
    pub end_version: u64,
    /// Table metadata describes database and table info.
    /// Note: `schemas` is deprecated, as it bloats backupmeta. It is kept for
    /// compatibility, so new BR can restore older backups.
    #[prost(message, repeated, tag = "7")]
    pub schemas: ::prost::alloc::vec::Vec<Schema>,
    /// An index to files contains Schemas.
    #[prost(message, optional, tag = "14")]
    pub schema_index: ::core::option::Option<MetaFile>,
    /// If in raw kv mode, `start_versions`, `end_versions` and `schemas` will be
    /// ignored, and the backup data's range is represented by raw_ranges.
    #[prost(bool, tag = "8")]
    pub is_raw_kv: bool,
    /// Note: `raw_ranges` is deprecated, as it bloats backupmeta. It is kept for
    /// compatibility, so new BR can restore older backups.
    #[prost(message, repeated, tag = "9")]
    pub raw_ranges: ::prost::alloc::vec::Vec<RawRange>,
    /// An index to files contains RawRanges.
    #[prost(message, optional, tag = "15")]
    pub raw_range_index: ::core::option::Option<MetaFile>,
    /// In incremental backup, DDLs which are completed in
    /// (lastBackupTS, backupTS\] will be stored here.
    /// Note: `raw_ranges` is deprecated, as it bloats backupmeta. It is kept for
    /// compatibility, so new BR can restore older backups.
    #[prost(bytes = "vec", tag = "10")]
    pub ddls: ::prost::alloc::vec::Vec<u8>,
    /// An index to files contains DDLs.
    #[prost(message, optional, tag = "16")]
    pub ddl_indexes: ::core::option::Option<MetaFile>,
    /// the backup result into `backupmeta` file
    #[prost(string, tag = "17")]
    pub backup_result: ::prost::alloc::string::String,
    /// API version implies the encode of the key and value.
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "18")]
    pub api_version: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct File {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub sha256: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "5")]
    pub start_version: u64,
    #[prost(uint64, tag = "6")]
    pub end_version: u64,
    #[prost(uint64, tag = "7")]
    pub crc64xor: u64,
    #[prost(uint64, tag = "8")]
    pub total_kvs: u64,
    #[prost(uint64, tag = "9")]
    pub total_bytes: u64,
    #[prost(string, tag = "10")]
    pub cf: ::prost::alloc::string::String,
    #[prost(uint64, tag = "11")]
    pub size: u64,
    /// cipher_iv is used for AES cipher
    #[prost(bytes = "vec", tag = "12")]
    pub cipher_iv: ::prost::alloc::vec::Vec<u8>,
}
/// MetaFile describes a multi-level index of data used in backup.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MetaFile {
    /// A set of files that contains a MetaFile.
    /// It is used as a multi-level index.
    #[prost(message, repeated, tag = "1")]
    pub meta_files: ::prost::alloc::vec::Vec<File>,
    /// A set of files that contains user data.
    #[prost(message, repeated, tag = "2")]
    pub data_files: ::prost::alloc::vec::Vec<File>,
    /// A set of files that contains Schemas.
    #[prost(message, repeated, tag = "3")]
    pub schemas: ::prost::alloc::vec::Vec<Schema>,
    /// A set of files that contains RawRanges.
    #[prost(message, repeated, tag = "4")]
    pub raw_ranges: ::prost::alloc::vec::Vec<RawRange>,
    /// A set of files that contains DDLs.
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub ddls: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Schema {
    #[prost(bytes = "vec", tag = "1")]
    pub db: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub table: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub crc64xor: u64,
    #[prost(uint64, tag = "4")]
    pub total_kvs: u64,
    #[prost(uint64, tag = "5")]
    pub total_bytes: u64,
    #[prost(uint32, tag = "6")]
    pub tiflash_replicas: u32,
    /// stats represents the dump stats for a analyzed table, which generate by DumpStatsToJSON
    /// <https://github.com/pingcap/tidb/blob/e136429d8dc5d70f43cd3f94179b0b9f47595097/statistics/handle/dump.go#L116>
    #[prost(bytes = "vec", tag = "7")]
    pub stats: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RawRange {
    #[prost(bytes = "vec", tag = "1")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, tag = "3")]
    pub cf: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterIdError {
    #[prost(uint64, tag = "1")]
    pub current: u64,
    #[prost(uint64, tag = "2")]
    pub request: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(string, tag = "1")]
    pub msg: ::prost::alloc::string::String,
    #[prost(oneof = "error::Detail", tags = "3, 4, 5")]
    pub detail: ::core::option::Option<error::Detail>,
}
/// Nested message and enum types in `Error`.
pub mod error {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Detail {
        #[prost(message, tag = "3")]
        ClusterIdError(super::ClusterIdError),
        #[prost(message, tag = "4")]
        KvError(super::super::kvrpcpb::KeyError),
        #[prost(message, tag = "5")]
        RegionError(super::super::errorpb::Error),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CipherInfo {
    #[prost(enumeration = "super::encryptionpb::EncryptionMethod", tag = "1")]
    pub cipher_type: i32,
    #[prost(bytes = "vec", tag = "2")]
    pub cipher_key: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BackupRequest {
    #[prost(uint64, tag = "1")]
    pub cluster_id: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub start_version: u64,
    #[prost(uint64, tag = "5")]
    pub end_version: u64,
    /// The I/O rate limit for backup request.
    #[prost(uint64, tag = "7")]
    pub rate_limit: u64,
    /// The concurrency for executing the backup request in every tikv node.
    #[prost(uint32, tag = "8")]
    pub concurrency: u32,
    #[prost(message, optional, tag = "9")]
    pub storage_backend: ::core::option::Option<StorageBackend>,
    /// If raw kv mode is enabled, `start_version` and `end_version` will be ignored, and `cf`
    /// specifies which cf to backup.
    #[prost(bool, tag = "10")]
    pub is_raw_kv: bool,
    #[prost(string, tag = "11")]
    pub cf: ::prost::alloc::string::String,
    /// algorithm used for compress sst files
    #[prost(enumeration = "CompressionType", tag = "12")]
    pub compression_type: i32,
    /// sst compression level, some algorithms support negative compression levels
    #[prost(int32, tag = "13")]
    pub compression_level: i32,
    /// The cipher_info is Used to encrypt sst
    #[prost(message, optional, tag = "14")]
    pub cipher_info: ::core::option::Option<CipherInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StorageBackend {
    #[prost(oneof = "storage_backend::Backend", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub backend: ::core::option::Option<storage_backend::Backend>,
}
/// Nested message and enum types in `StorageBackend`.
pub mod storage_backend {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Backend {
        #[prost(message, tag = "1")]
        Noop(super::Noop),
        #[prost(message, tag = "2")]
        Local(super::Local),
        #[prost(message, tag = "3")]
        S3(super::S3),
        #[prost(message, tag = "4")]
        Gcs(super::Gcs),
        #[prost(message, tag = "5")]
        CloudDynamic(super::CloudDynamic),
        #[prost(message, tag = "6")]
        Hdfs(super::Hdfs),
        #[prost(message, tag = "7")]
        AzureBlobStorage(super::AzureBlobStorage),
    }
}
/// Noop storage backend saves files into void.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Noop {}
/// Local storage backend saves files into local disk
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Local {
    #[prost(string, tag = "1")]
    pub path: ::prost::alloc::string::String,
}
/// S3 storage backend saves files into S3 compatible storages
/// For non-aws providers, endpoint must be provided
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct S3 {
    #[prost(string, tag = "1")]
    pub endpoint: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub region: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub bucket: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub prefix: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub storage_class: ::prost::alloc::string::String,
    /// server side encryption
    #[prost(string, tag = "6")]
    pub sse: ::prost::alloc::string::String,
    #[prost(string, tag = "7")]
    pub acl: ::prost::alloc::string::String,
    #[prost(string, tag = "8")]
    pub access_key: ::prost::alloc::string::String,
    #[prost(string, tag = "9")]
    pub secret_access_key: ::prost::alloc::string::String,
    #[prost(bool, tag = "10")]
    pub force_path_style: bool,
    #[prost(string, tag = "11")]
    pub sse_kms_key_id: ::prost::alloc::string::String,
}
/// GCS storage backend saves files into google cloud storage.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Gcs {
    #[prost(string, tag = "1")]
    pub endpoint: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub bucket: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub prefix: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub storage_class: ::prost::alloc::string::String,
    /// If not empty, applies a predefined set of access controls.
    /// See <https://cloud.google.com/storage/docs/access-control/lists#predefined-acl>
    /// for valid values.
    #[prost(string, tag = "5")]
    pub predefined_acl: ::prost::alloc::string::String,
    /// Service Account Credentials JSON blob
    /// You can get one from <https://console.cloud.google.com/apis/credentials,> and
    /// copy the content, set it as string here.
    #[prost(string, tag = "6")]
    pub credentials_blob: ::prost::alloc::string::String,
}
/// AzureBlobStorage storage backend saves files into azure blob storage.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AzureBlobStorage {
    #[prost(string, tag = "1")]
    pub endpoint: ::prost::alloc::string::String,
    /// Alias: container
    #[prost(string, tag = "2")]
    pub bucket: ::prost::alloc::string::String,
    /// Notice: prefix starts without `/`, otherwise the first directory's name is empty.
    #[prost(string, tag = "3")]
    pub prefix: ::prost::alloc::string::String,
    /// Alias: access_tier.
    /// See <https://docs.microsoft.com/en-us/azure/storage/blobs/access-tiers-overview>
    #[prost(string, tag = "4")]
    pub storage_class: ::prost::alloc::string::String,
    /// if empty, try to read account_name from the node's environment variable $AZURE_STORAGE_ACCOUNT.
    #[prost(string, tag = "5")]
    pub account_name: ::prost::alloc::string::String,
    /// Use shared key to access the azure blob
    /// If the node's environment variables($AZURE_CLIENT_ID, $AZURE_TENANT_ID, $AZURE_CLIENT_SECRET) exist,
    /// prefer to use token to access the azure blob.
    ///
    /// See <https://docs.microsoft.com/en-us/azure/storage/common/identity-library-acquire-token?toc=/azure/storage/blobs/toc.json>
    ///
    /// Otherwise, if empty, try to read shared key from the node's environment variable $AZURE_STORAGE_KEY.
    #[prost(string, tag = "6")]
    pub shared_key: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bucket {
    #[prost(string, tag = "1")]
    pub endpoint: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub region: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub bucket: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub prefix: ::prost::alloc::string::String,
    #[prost(string, tag = "6")]
    pub storage_class: ::prost::alloc::string::String,
}
/// CloudDynamic allows testing new cloud providers and new fields without changing protobuf definitions
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloudDynamic {
    #[prost(message, optional, tag = "1")]
    pub bucket: ::core::option::Option<Bucket>,
    /// s3, gcs and azureBlobStorage are supported
    #[prost(string, tag = "2")]
    pub provider_name: ::prost::alloc::string::String,
    #[prost(map = "string, string", tag = "3")]
    pub attrs: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
/// HDFS storage backend saves file into HDFS compatible storages
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Hdfs {
    /// a URL: hdfs:///some/path or hdfs://host:port/some/path
    #[prost(string, tag = "1")]
    pub remote: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BackupResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(bytes = "vec", tag = "2")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, repeated, tag = "4")]
    pub files: ::prost::alloc::vec::Vec<File>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExternalStorageRestoreRequest {
    #[prost(message, optional, tag = "1")]
    pub storage_backend: ::core::option::Option<StorageBackend>,
    #[prost(string, tag = "2")]
    pub object_name: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub restore_name: ::prost::alloc::string::String,
    #[prost(uint64, tag = "4")]
    pub content_length: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExternalStorageRestoreResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExternalStorageSaveRequest {
    #[prost(message, optional, tag = "1")]
    pub storage_backend: ::core::option::Option<StorageBackend>,
    #[prost(string, tag = "2")]
    pub object_name: ::prost::alloc::string::String,
    #[prost(uint64, tag = "3")]
    pub content_length: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExternalStorageSaveResponse {}
/// sst files compression algorithm
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CompressionType {
    Unknown = 0,
    Lz4 = 1,
    Snappy = 2,
    Zstd = 3,
}
impl CompressionType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CompressionType::Unknown => "UNKNOWN",
            CompressionType::Lz4 => "LZ4",
            CompressionType::Snappy => "SNAPPY",
            CompressionType::Zstd => "ZSTD",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "LZ4" => Some(Self::Lz4),
            "SNAPPY" => Some(Self::Snappy),
            "ZSTD" => Some(Self::Zstd),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod backup_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct BackupClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl BackupClient<tonic::transport::Channel> {
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
    impl<T> BackupClient<T>
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
        ) -> BackupClient<InterceptedService<T, F>>
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
            BackupClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn backup(
            &mut self,
            request: impl tonic::IntoRequest<super::BackupRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::BackupResponse>>,
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
            let path = http::uri::PathAndQuery::from_static("/backup.Backup/backup");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("backup.Backup", "backup"));
            self.inner.server_streaming(req, path, codec).await
        }
    }
}
/// Generated client implementations.
pub mod external_storage_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// ExternalStorage is a service for using a cloud backend from StorageBackend to store files.
    /// This can be used to backup and restore SST files.
    #[derive(Debug, Clone)]
    pub struct ExternalStorageClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ExternalStorageClient<tonic::transport::Channel> {
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
    impl<T> ExternalStorageClient<T>
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
        ) -> ExternalStorageClient<InterceptedService<T, F>>
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
            ExternalStorageClient::new(InterceptedService::new(inner, interceptor))
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
        /// Restore to a file
        pub async fn restore(
            &mut self,
            request: impl tonic::IntoRequest<super::ExternalStorageRestoreRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExternalStorageRestoreResponse>,
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
                "/backup.ExternalStorage/restore",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("backup.ExternalStorage", "restore"));
            self.inner.unary(req, path, codec).await
        }
        /// Save a file to storage
        pub async fn save(
            &mut self,
            request: impl tonic::IntoRequest<super::ExternalStorageSaveRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExternalStorageSaveResponse>,
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
                "/backup.ExternalStorage/save",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("backup.ExternalStorage", "save"));
            self.inner.unary(req, path, codec).await
        }
    }
}
