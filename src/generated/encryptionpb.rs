/// General encryption metadata for any data type.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EncryptionMeta {
    /// ID of the key used to encrypt the data.
    #[prost(uint64, tag = "1")]
    pub key_id: u64,
    /// Initialization vector (IV) of the data.
    #[prost(bytes = "vec", tag = "2")]
    pub iv: ::prost::alloc::vec::Vec<u8>,
}
/// Information about an encrypted file.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileInfo {
    /// ID of the key used to encrypt the file.
    #[prost(uint64, tag = "1")]
    pub key_id: u64,
    /// Initialization vector (IV) of the file.
    #[prost(bytes = "vec", tag = "2")]
    pub iv: ::prost::alloc::vec::Vec<u8>,
    /// Method of encryption algorithm used to encrypted the file.
    #[prost(enumeration = "EncryptionMethod", tag = "3")]
    pub method: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileDictionary {
    /// A map of file name to file info.
    #[prost(map = "string, message", tag = "1")]
    pub files: ::std::collections::HashMap<::prost::alloc::string::String, FileInfo>,
}
/// The key used to encrypt the user data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataKey {
    /// A sequence of secret bytes used to encrypt data.
    #[prost(bytes = "vec", tag = "1")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    /// Method of encryption algorithm used to encrypted data.
    #[prost(enumeration = "EncryptionMethod", tag = "2")]
    pub method: i32,
    /// Creation time of the key.
    #[prost(uint64, tag = "3")]
    pub creation_time: u64,
    /// A flag for the key have ever been exposed.
    #[prost(bool, tag = "4")]
    pub was_exposed: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyDictionary {
    /// A map of key ID to dat key.
    #[prost(map = "uint64, message", tag = "1")]
    pub keys: ::std::collections::HashMap<u64, DataKey>,
    /// ID of a key currently in use.
    #[prost(uint64, tag = "2")]
    pub current_key_id: u64,
}
/// Master key config.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MasterKey {
    #[prost(oneof = "master_key::Backend", tags = "1, 2, 3")]
    pub backend: ::core::option::Option<master_key::Backend>,
}
/// Nested message and enum types in `MasterKey`.
pub mod master_key {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Backend {
        #[prost(message, tag = "1")]
        Plaintext(super::MasterKeyPlaintext),
        #[prost(message, tag = "2")]
        File(super::MasterKeyFile),
        #[prost(message, tag = "3")]
        Kms(super::MasterKeyKms),
    }
}
/// MasterKeyPlaintext indicates content is stored as plaintext.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MasterKeyPlaintext {}
/// MasterKeyFile is a master key backed by a file containing encryption key in human-readable
/// hex format.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MasterKeyFile {
    /// Local file path.
    #[prost(string, tag = "1")]
    pub path: ::prost::alloc::string::String,
}
/// MasterKeyKms is a master key backed by KMS service that manages the encryption key,
/// and provide API to encrypt and decrypt a data key, which is used to encrypt the content.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MasterKeyKms {
    /// KMS vendor.
    #[prost(string, tag = "1")]
    pub vendor: ::prost::alloc::string::String,
    /// KMS key id.
    #[prost(string, tag = "2")]
    pub key_id: ::prost::alloc::string::String,
    /// KMS region.
    #[prost(string, tag = "3")]
    pub region: ::prost::alloc::string::String,
    /// KMS endpoint. Normally not needed.
    #[prost(string, tag = "4")]
    pub endpoint: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EncryptedContent {
    /// Metadata of the encrypted content.
    /// Eg. IV, method and KMS key ID
    /// It is preferred to define new fields for extra metadata than using this metadata map.
    #[prost(map = "string, bytes", tag = "1")]
    pub metadata: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::vec::Vec<u8>,
    >,
    /// Encrypted content.
    #[prost(bytes = "vec", tag = "2")]
    pub content: ::prost::alloc::vec::Vec<u8>,
    /// Master key used to encrypt the content.
    #[prost(message, optional, tag = "3")]
    pub master_key: ::core::option::Option<MasterKey>,
    /// Initilization vector (IV) used.
    #[prost(bytes = "vec", tag = "4")]
    pub iv: ::prost::alloc::vec::Vec<u8>,
    /// Encrypted data key generated by KMS and used to actually encrypt data.
    /// Valid only when KMS is used.
    #[prost(bytes = "vec", tag = "5")]
    pub ciphertext_key: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EncryptionMethod {
    Unknown = 0,
    Plaintext = 1,
    Aes128Ctr = 2,
    Aes192Ctr = 3,
    Aes256Ctr = 4,
    Sm4Ctr = 5,
}
impl EncryptionMethod {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            EncryptionMethod::Unknown => "UNKNOWN",
            EncryptionMethod::Plaintext => "PLAINTEXT",
            EncryptionMethod::Aes128Ctr => "AES128_CTR",
            EncryptionMethod::Aes192Ctr => "AES192_CTR",
            EncryptionMethod::Aes256Ctr => "AES256_CTR",
            EncryptionMethod::Sm4Ctr => "SM4_CTR",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "PLAINTEXT" => Some(Self::Plaintext),
            "AES128_CTR" => Some(Self::Aes128Ctr),
            "AES192_CTR" => Some(Self::Aes192Ctr),
            "AES256_CTR" => Some(Self::Aes256Ctr),
            "SM4_CTR" => Some(Self::Sm4Ctr),
            _ => None,
        }
    }
}
