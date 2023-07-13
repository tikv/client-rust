#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DiskUsage {
    Normal = 0,
    AlmostFull = 1,
    AlreadyFull = 2,
}
impl DiskUsage {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DiskUsage::Normal => "Normal",
            DiskUsage::AlmostFull => "AlmostFull",
            DiskUsage::AlreadyFull => "AlreadyFull",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Normal" => Some(Self::Normal),
            "AlmostFull" => Some(Self::AlmostFull),
            "AlreadyFull" => Some(Self::AlreadyFull),
            _ => None,
        }
    }
}
