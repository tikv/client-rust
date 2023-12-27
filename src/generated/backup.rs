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
    /// the placement policy info in backup cluster. we assume the policy won't be too much for one cluster.
    #[prost(message, repeated, tag = "19")]
    pub policies: ::prost::alloc::vec::Vec<PlacementPolicy>,
    /// new_collations_enabled specifies the config `new_collations_enabled_on_first_bootstrap` in tidb.
    #[prost(string, tag = "20")]
    pub new_collations_enabled: ::prost::alloc::string::String,
    /// If in txn kv mode, `schemas` will be ignored, the backup data's range is as same as normal backup.
    #[prost(bool, tag = "21")]
    pub is_txn_kv: bool,
    /// maintain the id mapping from upstream cluster to downstream cluster.
    #[prost(message, repeated, tag = "22")]
    pub db_maps: ::prost::alloc::vec::Vec<PitrDbMap>,
    #[prost(enumeration = "BackupMode", tag = "23")]
    pub mode: i32,
    /// record the backup range and the correspond SST files when using file-copy backup.
    #[prost(message, repeated, tag = "24")]
    pub ranges: ::prost::alloc::vec::Vec<BackupRange>,
    /// record the size of the backup data files and meta files
    #[prost(uint64, tag = "25")]
    pub backup_size: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BackupRange {
    #[prost(bytes = "vec", tag = "1")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, repeated, tag = "3")]
    pub files: ::prost::alloc::vec::Vec<File>,
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
    /// A set of files that contains BackupRanges.
    #[prost(message, repeated, tag = "6")]
    pub backup_ranges: ::prost::alloc::vec::Vec<BackupRange>,
    /// A set of files that contains DDLs.
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub ddls: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlacementPolicy {
    #[prost(bytes = "vec", tag = "1")]
    pub info: ::prost::alloc::vec::Vec<u8>,
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
pub struct IdMap {
    #[prost(int64, tag = "1")]
    pub upstream_id: i64,
    #[prost(int64, tag = "2")]
    pub downstream_id: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PitrTableMap {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub id_map: ::core::option::Option<IdMap>,
    #[prost(message, repeated, tag = "3")]
    pub partitions: ::prost::alloc::vec::Vec<IdMap>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PitrDbMap {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub id_map: ::core::option::Option<IdMap>,
    #[prost(message, repeated, tag = "3")]
    pub tables: ::prost::alloc::vec::Vec<PitrTableMap>,
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
    /// dst_api_version indicates the key-value encoding version used by the
    /// generated SST file. Accepted values:
    ///
    /// 1. "v1": the generated SST files are encoded with api-v1, can be restored
    ///    to TiKV clusters whose api version is set to v1.
    /// 1. "v2": the generated SST files are encoded with api-v2, can be restored
    ///    to TiKV clusters whose api version is set to v2.
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "15")]
    pub dst_api_version: i32,
    /// with checkpoint, some subintervals of the range have been backed up and recorded.
    /// only the remaining sub ranges of the range need to be backed up this time.
    #[prost(message, repeated, tag = "16")]
    pub sub_ranges: ::prost::alloc::vec::Vec<super::kvrpcpb::KeyRange>,
    /// replica_read indicates whether to use replica read for backup.
    /// If it is false, the backup will only read data from leader.
    /// If it is true, the backup will read data from both leader and follower.
    #[prost(bool, tag = "17")]
    pub replica_read: bool,
    #[prost(enumeration = "BackupMode", tag = "18")]
    pub mode: i32,
    /// unique_id represents the handle of this backup. after we implement file-copy backup.
    /// we need generate some internal states during the whole backup precedure.
    /// this unique id is help to find the state effictively.
    #[prost(string, tag = "19")]
    pub unique_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "20")]
    pub context: ::core::option::Option<super::kvrpcpb::Context>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamBackupTaskInfo {
    /// The storage for backup, parsed by BR.
    #[prost(message, optional, tag = "1")]
    pub storage: ::core::option::Option<StorageBackend>,
    /// The time range for backing up.
    #[prost(uint64, tag = "2")]
    pub start_ts: u64,
    #[prost(uint64, tag = "3")]
    pub end_ts: u64,
    /// Misc meta datas.
    /// The name of the task, also the ID of the task.
    #[prost(string, tag = "4")]
    pub name: ::prost::alloc::string::String,
    /// The table filter of the task.
    /// Only for displaying the task info.
    #[prost(string, repeated, tag = "5")]
    pub table_filter: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// compression type
    #[prost(enumeration = "CompressionType", tag = "6")]
    pub compression_type: i32,
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
    #[prost(string, tag = "12")]
    pub role_arn: ::prost::alloc::string::String,
    #[prost(string, tag = "13")]
    pub external_id: ::prost::alloc::string::String,
    #[prost(bool, tag = "14")]
    pub object_lock_enabled: bool,
    #[prost(string, tag = "15")]
    pub session_token: ::prost::alloc::string::String,
    #[prost(string, tag = "16")]
    pub provider: ::prost::alloc::string::String,
    #[prost(string, tag = "17")]
    pub profile: ::prost::alloc::string::String,
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
/// The encryption algorithm must be AES256.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AzureCustomerKey {
    /// A Base64-encoded AES-256 encryption key value.
    #[prost(string, tag = "1")]
    pub encryption_key: ::prost::alloc::string::String,
    /// The Base64-encoded SHA256 of the encryption key.
    #[prost(string, tag = "2")]
    pub encryption_key_sha256: ::prost::alloc::string::String,
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
    /// There are 3 kinds of credentials, and the priority order is
    /// `SAS > Shared key > Azure AD (env) > Shared key (env)`.
    ///
    /// 1. Use shared key to access the azure blob
    ///    If the node's environment variables($AZURE_CLIENT_ID, $AZURE_TENANT_ID, $AZURE_CLIENT_SECRET) exist,
    ///    prefer to use token to access the azure blob.
    ///
    /// See <https://learn.microsoft.com/en-us/rest/api/storageservices/authorize-with-shared-key>
    ///
    /// Otherwise, if empty, try to read shared key from the node's environment variable $AZURE_STORAGE_KEY.
    #[prost(string, tag = "6")]
    pub shared_key: ::prost::alloc::string::String,
    /// 2. Use Azure AD (Azure Active Directory) to access the azure blob
    ///
    /// See <https://learn.microsoft.com/en-us/rest/api/storageservices/authorize-with-azure-active-directory>
    ///
    /// The Azure AD would generate the token, which tasks some time.
    /// So it is not recommanded to generate the token in each request.
    /// // AzureActiveDirectory azure_ad = #;
    ///
    /// 3. Use SAS (shared access signature)
    ///
    /// See <https://learn.microsoft.com/en-us/rest/api/storageservices/delegate-access-with-shared-access-signature>
    #[prost(string, tag = "8")]
    pub access_sig: ::prost::alloc::string::String,
    /// Server Side Encryption, 2 types in total:
    ///
    /// 1. Specify an encryption scope for uploaded blobs.
    ///
    /// See <https://learn.microsoft.com/en-us/azure/storage/blobs/encryption-scope-manage?tabs=powershell#upload-a-blob-with-an-encryption-scope>
    #[prost(string, tag = "9")]
    pub encryption_scope: ::prost::alloc::string::String,
    /// 2. Provide an encryption key on a request to blob storage.
    ///
    /// See <https://learn.microsoft.com/en-us/azure/storage/blobs/encryption-customer-provided-keys>
    #[prost(message, optional, tag = "10")]
    pub encryption_key: ::core::option::Option<AzureCustomerKey>,
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
    /// API version implies the encode of the key and value.
    #[prost(enumeration = "super::kvrpcpb::ApiVersion", tag = "5")]
    pub api_version: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CleanupRequest {
    /// unique_id represents the unique handle of the whole backup predecure.
    /// it generated in prepare request and corrosponed to one specific backup.
    #[prost(string, tag = "1")]
    pub unique_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CleanupResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(bool, tag = "2")]
    pub success: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrepareRequest {
    /// whether save state to the storage.
    #[prost(bool, tag = "1")]
    pub save_to_storage: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrepareResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    /// unique_id represents the unique handle of the whole backup predecure.
    /// if unique_id = 0 means prepare failed.
    /// if unique_id > 0 means prepare success and all states saved with this unique info.
    #[prost(string, tag = "2")]
    pub unique_id: ::prost::alloc::string::String,
    #[prost(uint64, tag = "3")]
    pub collect_file_count: u64,
    #[prost(uint64, tag = "4")]
    pub collect_file_size: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckAdminRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckAdminResponse {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<Error>,
    #[prost(message, optional, tag = "2")]
    pub region: ::core::option::Option<super::metapb::Region>,
    #[prost(bool, tag = "3")]
    pub has_pending_admin: bool,
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
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Metadata {
    /// deprecated after(in) v6.3.0 TiKV cluster
    #[prost(message, repeated, tag = "1")]
    pub files: ::prost::alloc::vec::Vec<DataFileInfo>,
    #[prost(message, repeated, tag = "6")]
    pub file_groups: ::prost::alloc::vec::Vec<DataFileGroup>,
    #[prost(int64, tag = "2")]
    pub store_id: i64,
    #[prost(uint64, tag = "3")]
    pub resolved_ts: u64,
    #[prost(uint64, tag = "4")]
    pub max_ts: u64,
    #[prost(uint64, tag = "5")]
    pub min_ts: u64,
    #[prost(enumeration = "MetaVersion", tag = "7")]
    pub meta_version: i32,
}
/// DataFileGroup is the merged file info in log-backup
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataFileGroup {
    /// Path of the file.
    #[prost(string, tag = "1")]
    pub path: ::prost::alloc::string::String,
    /// Partitions of the file.
    #[prost(message, repeated, tag = "2")]
    pub data_files_info: ::prost::alloc::vec::Vec<DataFileInfo>,
    /// / Below are extra information of the file, for better filtering files.
    /// The min ts of the keys in the file.
    #[prost(uint64, tag = "3")]
    pub min_ts: u64,
    /// The max ts of the keys in the file.
    #[prost(uint64, tag = "4")]
    pub max_ts: u64,
    /// The resolved ts of the region when saving the file.
    #[prost(uint64, tag = "5")]
    pub min_resolved_ts: u64,
    /// The file length after compressed.
    #[prost(uint64, tag = "6")]
    pub length: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataFileInfo {
    /// SHA256 of the file.
    #[prost(bytes = "vec", tag = "1")]
    pub sha256: ::prost::alloc::vec::Vec<u8>,
    /// Path of the file.
    #[prost(string, tag = "2")]
    pub path: ::prost::alloc::string::String,
    #[prost(int64, tag = "3")]
    pub number_of_entries: i64,
    /// / Below are extra information of the file, for better filtering files.
    /// The min ts of the keys in the file.
    #[prost(uint64, tag = "4")]
    pub min_ts: u64,
    /// The max ts of the keys in the file.
    #[prost(uint64, tag = "5")]
    pub max_ts: u64,
    /// The resolved ts of the region when saving the file.
    #[prost(uint64, tag = "6")]
    pub resolved_ts: u64,
    /// The region of the file.
    #[prost(int64, tag = "7")]
    pub region_id: i64,
    /// The key range of the file.
    /// Encoded and starts with 'z'(internal key).
    #[prost(bytes = "vec", tag = "8")]
    pub start_key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub end_key: ::prost::alloc::vec::Vec<u8>,
    /// The column family of the file.
    #[prost(string, tag = "10")]
    pub cf: ::prost::alloc::string::String,
    /// The operation type of the file.
    #[prost(enumeration = "FileType", tag = "11")]
    pub r#type: i32,
    /// Whether the data file contains meta keys(m prefixed keys) only.
    #[prost(bool, tag = "12")]
    pub is_meta: bool,
    /// The table ID of the file contains, when `is_meta` is true, would be ignored.
    #[prost(int64, tag = "13")]
    pub table_id: i64,
    /// The file length.
    #[prost(uint64, tag = "14")]
    pub length: u64,
    /// The minimal begin ts in default cf if this file is write cf.
    #[prost(uint64, tag = "15")]
    pub min_begin_ts_in_default_cf: u64,
    /// Offset of the partition. compatible with V1 and V2.
    #[prost(uint64, tag = "16")]
    pub range_offset: u64,
    /// The range length of the merged file, if it exists.
    #[prost(uint64, tag = "17")]
    pub range_length: u64,
    /// The compression type for the file.
    #[prost(enumeration = "CompressionType", tag = "18")]
    pub compression_type: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamBackupError {
    /// the unix epoch time (in millisecs) of the time the error reported.
    #[prost(uint64, tag = "1")]
    pub happen_at: u64,
    /// the unified error code of the error.
    #[prost(string, tag = "2")]
    pub error_code: ::prost::alloc::string::String,
    /// the user-friendly error message.
    #[prost(string, tag = "3")]
    pub error_message: ::prost::alloc::string::String,
    /// the store id of who issues the error.
    #[prost(uint64, tag = "4")]
    pub store_id: u64,
}
/// sst files or log files compression algorithm
/// for log files, unknown means not use compression algorithm
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
/// BackupMpde represents the mode of this whole backup request to the cluster.
/// and we need to store it in `backupmeta`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum BackupMode {
    /// scan and generate precise SST file of a backup range.
    /// then we don't need to do anything in future restore.
    /// Note: this mode will cost the CPU resource of TiKV.
    Scan = 0,
    /// check and upload the coarse overlap SST files of a backup range.
    /// then we need to use a merge iterator to filter unexpected kv in future restore.
    /// Note: this mode will save the CPU resource of TiKV.
    File = 1,
}
impl BackupMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            BackupMode::Scan => "SCAN",
            BackupMode::File => "FILE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SCAN" => Some(Self::Scan),
            "FILE" => Some(Self::File),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MetaVersion {
    V1 = 0,
    V2 = 1,
}
impl MetaVersion {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            MetaVersion::V1 => "V1",
            MetaVersion::V2 => "V2",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "V1" => Some(Self::V1),
            "V2" => Some(Self::V2),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum FileType {
    Delete = 0,
    Put = 1,
}
impl FileType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            FileType::Delete => "Delete",
            FileType::Put => "Put",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Delete" => Some(Self::Delete),
            "Put" => Some(Self::Put),
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
        /// CheckPendingAdminOp used for snapshot backup. before we start snapshot for a TiKV.
        /// we need stop all schedule first and make sure all in-flight schedule has finished.
        /// this rpc check all pending conf change for leader.
        pub async fn check_pending_admin_op(
            &mut self,
            request: impl tonic::IntoRequest<super::CheckAdminRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::CheckAdminResponse>>,
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
                "/backup.Backup/CheckPendingAdminOp",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("backup.Backup", "CheckPendingAdminOp"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// prepare is used for file-copy backup. before we start the backup for a TiKV.
        /// we need invoke this function to generate the SST files map. or we get nothing to backup.
        pub async fn prepare(
            &mut self,
            request: impl tonic::IntoRequest<super::PrepareRequest>,
        ) -> std::result::Result<
            tonic::Response<super::PrepareResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/backup.Backup/prepare");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("backup.Backup", "prepare"));
            self.inner.unary(req, path, codec).await
        }
        /// cleanup used for file-copy backup. after we finish the backup for a TiKV.
        /// we need clean some internel state. e.g. checkpoint, SST File maps
        pub async fn cleanup(
            &mut self,
            request: impl tonic::IntoRequest<super::CleanupRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CleanupResponse>,
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
            let path = http::uri::PathAndQuery::from_static("/backup.Backup/cleanup");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("backup.Backup", "cleanup"));
            self.inner.unary(req, path, codec).await
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
