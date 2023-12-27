/// The entry is a type of change that needs to be applied. It contains two data fields.
/// While the fields are built into the model; their usage is determined by the entry_type.
///
/// For normal entries, the data field should contain the data change that should be applied.
/// The context field can be used for any contextual data that might be relevant to the
/// application of the data.
///
/// For configuration changes, the data will contain the ConfChange message and the
/// context will provide anything needed to assist the configuration change. The context
/// if for the user to set and use in this case.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Entry {
    #[prost(enumeration = "EntryType", tag = "1")]
    pub entry_type: i32,
    #[prost(uint64, tag = "2")]
    pub term: u64,
    #[prost(uint64, tag = "3")]
    pub index: u64,
    #[prost(bytes = "vec", tag = "4")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub context: ::prost::alloc::vec::Vec<u8>,
    /// Deprecated! It is kept for backward compatibility.
    /// TODO: remove it in the next major release.
    #[prost(bool, tag = "5")]
    pub sync_log: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotMetadata {
    /// The current `ConfState`.
    #[prost(message, optional, tag = "1")]
    pub conf_state: ::core::option::Option<ConfState>,
    /// The applied index.
    #[prost(uint64, tag = "2")]
    pub index: u64,
    /// The term of the applied index.
    #[prost(uint64, tag = "3")]
    pub term: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Snapshot {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub metadata: ::core::option::Option<SnapshotMetadata>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Message {
    #[prost(enumeration = "MessageType", tag = "1")]
    pub msg_type: i32,
    #[prost(uint64, tag = "2")]
    pub to: u64,
    #[prost(uint64, tag = "3")]
    pub from: u64,
    #[prost(uint64, tag = "4")]
    pub term: u64,
    #[prost(uint64, tag = "5")]
    pub log_term: u64,
    #[prost(uint64, tag = "6")]
    pub index: u64,
    #[prost(message, repeated, tag = "7")]
    pub entries: ::prost::alloc::vec::Vec<Entry>,
    #[prost(uint64, tag = "8")]
    pub commit: u64,
    #[prost(message, optional, tag = "9")]
    pub snapshot: ::core::option::Option<Snapshot>,
    #[prost(uint64, tag = "13")]
    pub request_snapshot: u64,
    #[prost(bool, tag = "10")]
    pub reject: bool,
    #[prost(uint64, tag = "11")]
    pub reject_hint: u64,
    #[prost(bytes = "vec", tag = "12")]
    pub context: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "14")]
    pub deprecated_priority: u64,
    /// If this new field is not set, then use the above old field; otherwise
    /// use the new field. When broadcasting request vote, both fields are
    /// set if the priority is larger than 0. This change is not a fully
    /// compatible change, but it makes minimal impact that only new priority
    /// is not recognized by the old nodes during rolling update.
    #[prost(int64, tag = "15")]
    pub priority: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HardState {
    #[prost(uint64, tag = "1")]
    pub term: u64,
    #[prost(uint64, tag = "2")]
    pub vote: u64,
    #[prost(uint64, tag = "3")]
    pub commit: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfState {
    #[prost(uint64, repeated, tag = "1")]
    pub voters: ::prost::alloc::vec::Vec<u64>,
    #[prost(uint64, repeated, tag = "2")]
    pub learners: ::prost::alloc::vec::Vec<u64>,
    /// The voters in the outgoing config. If not empty the node is in joint consensus.
    #[prost(uint64, repeated, tag = "3")]
    pub voters_outgoing: ::prost::alloc::vec::Vec<u64>,
    /// The nodes that will become learners when the outgoing config is removed.
    /// These nodes are necessarily currently in nodes_joint (or they would have
    /// been added to the incoming config right away).
    #[prost(uint64, repeated, tag = "4")]
    pub learners_next: ::prost::alloc::vec::Vec<u64>,
    /// If set, the config is joint and Raft will automatically transition into
    /// the final config (i.e. remove the outgoing config) when this is safe.
    #[prost(bool, tag = "5")]
    pub auto_leave: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfChange {
    #[prost(enumeration = "ConfChangeType", tag = "2")]
    pub change_type: i32,
    #[prost(uint64, tag = "3")]
    pub node_id: u64,
    #[prost(bytes = "vec", tag = "4")]
    pub context: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "1")]
    pub id: u64,
}
/// ConfChangeSingle is an individual configuration change operation. Multiple
/// such operations can be carried out atomically via a ConfChangeV2.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfChangeSingle {
    #[prost(enumeration = "ConfChangeType", tag = "1")]
    pub change_type: i32,
    #[prost(uint64, tag = "2")]
    pub node_id: u64,
}
/// ConfChangeV2 messages initiate configuration changes. They support both the
/// simple "one at a time" membership change protocol and full Joint Consensus
/// allowing for arbitrary changes in membership.
///
/// The supplied context is treated as an opaque payload and can be used to
/// attach an action on the state machine to the application of the config change
/// proposal. Note that contrary to Joint Consensus as outlined in the Raft
/// paper\[1\], configuration changes become active when they are *applied* to the
/// state machine (not when they are appended to the log).
///
/// The simple protocol can be used whenever only a single change is made.
///
/// Non-simple changes require the use of Joint Consensus, for which two
/// configuration changes are run. The first configuration change specifies the
/// desired changes and transitions the Raft group into the joint configuration,
/// in which quorum requires a majority of both the pre-changes and post-changes
/// configuration. Joint Consensus avoids entering fragile intermediate
/// configurations that could compromise survivability. For example, without the
/// use of Joint Consensus and running across three availability zones with a
/// replication factor of three, it is not possible to replace a voter without
/// entering an intermediate configuration that does not survive the outage of
/// one availability zone.
///
/// The provided ConfChangeTransition specifies how (and whether) Joint Consensus
/// is used, and assigns the task of leaving the joint configuration either to
/// Raft or the application. Leaving the joint configuration is accomplished by
/// proposing a ConfChangeV2 with only and optionally the Context field
/// populated.
///
/// For details on Raft membership changes, see:
///
/// \[1\]: <https://github.com/ongardie/dissertation/blob/master/online-trim.pdf>
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfChangeV2 {
    #[prost(enumeration = "ConfChangeTransition", tag = "1")]
    pub transition: i32,
    #[prost(message, repeated, tag = "2")]
    pub changes: ::prost::alloc::vec::Vec<ConfChangeSingle>,
    #[prost(bytes = "vec", tag = "3")]
    pub context: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EntryType {
    EntryNormal = 0,
    EntryConfChange = 1,
    EntryConfChangeV2 = 2,
}
impl EntryType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            EntryType::EntryNormal => "EntryNormal",
            EntryType::EntryConfChange => "EntryConfChange",
            EntryType::EntryConfChangeV2 => "EntryConfChangeV2",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "EntryNormal" => Some(Self::EntryNormal),
            "EntryConfChange" => Some(Self::EntryConfChange),
            "EntryConfChangeV2" => Some(Self::EntryConfChangeV2),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MessageType {
    MsgHup = 0,
    MsgBeat = 1,
    MsgPropose = 2,
    MsgAppend = 3,
    MsgAppendResponse = 4,
    MsgRequestVote = 5,
    MsgRequestVoteResponse = 6,
    MsgSnapshot = 7,
    MsgHeartbeat = 8,
    MsgHeartbeatResponse = 9,
    MsgUnreachable = 10,
    MsgSnapStatus = 11,
    MsgCheckQuorum = 12,
    MsgTransferLeader = 13,
    MsgTimeoutNow = 14,
    MsgReadIndex = 15,
    MsgReadIndexResp = 16,
    MsgRequestPreVote = 17,
    MsgRequestPreVoteResponse = 18,
}
impl MessageType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            MessageType::MsgHup => "MsgHup",
            MessageType::MsgBeat => "MsgBeat",
            MessageType::MsgPropose => "MsgPropose",
            MessageType::MsgAppend => "MsgAppend",
            MessageType::MsgAppendResponse => "MsgAppendResponse",
            MessageType::MsgRequestVote => "MsgRequestVote",
            MessageType::MsgRequestVoteResponse => "MsgRequestVoteResponse",
            MessageType::MsgSnapshot => "MsgSnapshot",
            MessageType::MsgHeartbeat => "MsgHeartbeat",
            MessageType::MsgHeartbeatResponse => "MsgHeartbeatResponse",
            MessageType::MsgUnreachable => "MsgUnreachable",
            MessageType::MsgSnapStatus => "MsgSnapStatus",
            MessageType::MsgCheckQuorum => "MsgCheckQuorum",
            MessageType::MsgTransferLeader => "MsgTransferLeader",
            MessageType::MsgTimeoutNow => "MsgTimeoutNow",
            MessageType::MsgReadIndex => "MsgReadIndex",
            MessageType::MsgReadIndexResp => "MsgReadIndexResp",
            MessageType::MsgRequestPreVote => "MsgRequestPreVote",
            MessageType::MsgRequestPreVoteResponse => "MsgRequestPreVoteResponse",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "MsgHup" => Some(Self::MsgHup),
            "MsgBeat" => Some(Self::MsgBeat),
            "MsgPropose" => Some(Self::MsgPropose),
            "MsgAppend" => Some(Self::MsgAppend),
            "MsgAppendResponse" => Some(Self::MsgAppendResponse),
            "MsgRequestVote" => Some(Self::MsgRequestVote),
            "MsgRequestVoteResponse" => Some(Self::MsgRequestVoteResponse),
            "MsgSnapshot" => Some(Self::MsgSnapshot),
            "MsgHeartbeat" => Some(Self::MsgHeartbeat),
            "MsgHeartbeatResponse" => Some(Self::MsgHeartbeatResponse),
            "MsgUnreachable" => Some(Self::MsgUnreachable),
            "MsgSnapStatus" => Some(Self::MsgSnapStatus),
            "MsgCheckQuorum" => Some(Self::MsgCheckQuorum),
            "MsgTransferLeader" => Some(Self::MsgTransferLeader),
            "MsgTimeoutNow" => Some(Self::MsgTimeoutNow),
            "MsgReadIndex" => Some(Self::MsgReadIndex),
            "MsgReadIndexResp" => Some(Self::MsgReadIndexResp),
            "MsgRequestPreVote" => Some(Self::MsgRequestPreVote),
            "MsgRequestPreVoteResponse" => Some(Self::MsgRequestPreVoteResponse),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ConfChangeTransition {
    /// Automatically use the simple protocol if possible, otherwise fall back
    /// to ConfChangeType::Implicit. Most applications will want to use this.
    Auto = 0,
    /// Use joint consensus unconditionally, and transition out of them
    /// automatically (by proposing a zero configuration change).
    ///
    /// This option is suitable for applications that want to minimize the time
    /// spent in the joint configuration and do not store the joint configuration
    /// in the state machine (outside of InitialState).
    Implicit = 1,
    /// Use joint consensus and remain in the joint configuration until the
    /// application proposes a no-op configuration change. This is suitable for
    /// applications that want to explicitly control the transitions, for example
    /// to use a custom payload (via the Context field).
    Explicit = 2,
}
impl ConfChangeTransition {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ConfChangeTransition::Auto => "Auto",
            ConfChangeTransition::Implicit => "Implicit",
            ConfChangeTransition::Explicit => "Explicit",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Auto" => Some(Self::Auto),
            "Implicit" => Some(Self::Implicit),
            "Explicit" => Some(Self::Explicit),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ConfChangeType {
    AddNode = 0,
    RemoveNode = 1,
    AddLearnerNode = 2,
}
impl ConfChangeType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ConfChangeType::AddNode => "AddNode",
            ConfChangeType::RemoveNode => "RemoveNode",
            ConfChangeType::AddLearnerNode => "AddLearnerNode",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AddNode" => Some(Self::AddNode),
            "RemoveNode" => Some(Self::RemoveNode),
            "AddLearnerNode" => Some(Self::AddLearnerNode),
            _ => None,
        }
    }
}
