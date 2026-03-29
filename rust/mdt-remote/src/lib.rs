use serde::{de, Deserialize, Serialize};
use std::{collections::HashSet, fmt, fs, path::Path};

pub const REMOTE_MANIFEST_SCHEMA_V1: &str = "mdt.remote.manifest.v1";
pub const CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT: usize = 10;
pub const HIGH_FREQUENCY_REMOTE_METHOD_COUNT: usize = 5;
pub const INBOUND_REMOTE_FAMILY_COUNT: usize = 6;
pub const WELL_KNOWN_REMOTE_METHOD_COUNT: usize = 19;
pub const REMOTE_PACKET_ID_SPACE: usize = u8::MAX as usize + 1;
pub const REMOTE_WIRE_PACKET_ID_BYTE_U8: &str = "u8";
pub const REMOTE_WIRE_LENGTH_FIELD_U16BE: &str = "u16be";
pub const REMOTE_WIRE_COMPRESSION_NONE: &str = "none";
pub const REMOTE_WIRE_COMPRESSION_LZ4: &str = "lz4";
pub const REMOTE_WIRE_COMPRESSION_THRESHOLD: u16 = 36;

#[derive(Debug)]
pub enum RemoteManifestError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnsupportedSchema(String),
    InvalidPacketSequence(String),
    InvalidWireSpec(String),
    InvalidRemotePacketMetadata(String),
    MissingHighFrequencyPacket(&'static str),
}

impl fmt::Display for RemoteManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read remote manifest: {error}"),
            Self::Json(error) => write!(f, "failed to parse remote manifest JSON: {error}"),
            Self::UnsupportedSchema(schema) => {
                write!(f, "unsupported remote manifest schema: {schema}")
            }
            Self::InvalidPacketSequence(message) => write!(f, "{message}"),
            Self::InvalidWireSpec(message) => write!(f, "{message}"),
            Self::InvalidRemotePacketMetadata(message) => write!(f, "{message}"),
            Self::MissingHighFrequencyPacket(method) => {
                write!(
                    f,
                    "missing high-frequency remote packet in manifest: {method}"
                )
            }
        }
    }
}

impl std::error::Error for RemoteManifestError {}

impl From<std::io::Error> for RemoteManifestError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for RemoteManifestError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteManifest {
    pub schema: String,
    pub generator: RemoteGeneratorInfo,
    #[serde(rename = "basePackets")]
    pub base_packets: Vec<BasePacketEntry>,
    #[serde(rename = "remotePackets")]
    pub remote_packets: Vec<RemotePacketEntry>,
    pub wire: WireSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteGeneratorInfo {
    pub source: String,
    #[serde(rename = "callClass")]
    pub call_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasePacketEntry {
    pub id: u8,
    #[serde(rename = "class")]
    pub class_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemotePacketEntry {
    #[serde(rename = "remoteIndex")]
    pub remote_index: usize,
    #[serde(rename = "packetId")]
    pub packet_id: u8,
    #[serde(rename = "packetClass")]
    pub packet_class: String,
    #[serde(rename = "declaringType")]
    pub declaring_type: String,
    pub method: String,
    pub targets: String,
    pub called: String,
    pub variants: String,
    #[serde(default, rename = "allowOnClient")]
    pub allow_on_client: Option<bool>,
    #[serde(default, rename = "allowOnServer")]
    pub allow_on_server: Option<bool>,
    pub forward: bool,
    pub unreliable: bool,
    pub priority: String,
    pub params: Vec<RemoteParamEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteParamEntry {
    pub name: String,
    #[serde(rename = "javaType")]
    pub java_type: String,
    #[serde(rename = "networkIncludedWhenCallerIsClient")]
    pub network_included_when_caller_is_client: bool,
    #[serde(rename = "networkIncludedWhenCallerIsServer")]
    pub network_included_when_caller_is_server: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireSpec {
    #[serde(rename = "packetIdByte")]
    pub packet_id_byte: String,
    #[serde(rename = "lengthField")]
    pub length_field: String,
    #[serde(rename = "compressionFlag")]
    pub compression_flag: CompressionFlagSpec,
    #[serde(rename = "compressionThreshold")]
    pub compression_threshold: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompressionFlagSpec {
    #[serde(rename = "0")]
    pub none: String,
    #[serde(rename = "1")]
    pub lz4: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteFlow {
    ClientToServer,
    ServerToClient,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePriority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighFrequencyRemoteMethod {
    ClientSnapshot,
    StateSnapshot,
    EntitySnapshot,
    BlockSnapshot,
    HiddenSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellKnownRemoteMethod {
    Ping,
    ClientPlanSnapshot,
    ClientPlanSnapshotReceived,
    PingResponse,
    PingLocation,
    DebugStatusClientUnreliable,
    TraceInfo,
    ConnectRedirect,
    ConnectConfirm,
    PlayerSpawn,
    SetRules,
    SetObjectives,
    SetRule,
    WorldDataBegin,
    KickString,
    KickReason,
    SendChatMessage,
    SendMessage,
    SendMessageWithSender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomChannelRemoteFamily {
    ClientPacketReliable,
    ClientPacketUnreliable,
    ClientBinaryPacketReliable,
    ClientBinaryPacketUnreliable,
    ServerPacketReliable,
    ServerPacketUnreliable,
    ServerBinaryPacketReliable,
    ServerBinaryPacketUnreliable,
    ClientLogicDataReliable,
    ClientLogicDataUnreliable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomChannelRemotePayloadKind {
    Text,
    Binary,
    LogicData,
}

impl CustomChannelRemotePayloadKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Binary => "binary",
            Self::LogicData => "logic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundRemoteFamily {
    ServerPacketReliable,
    ServerPacketUnreliable,
    ServerBinaryPacketReliable,
    ServerBinaryPacketUnreliable,
    ClientLogicDataReliable,
    ClientLogicDataUnreliable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteParamKind {
    Bool,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Bytes,
    TileRef,
    BlockRef,
    BuildPlanQueue,
    IntSeq,
    Opaque,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRemoteParamSpec<'a> {
    pub name: &'a str,
    pub java_type: &'a str,
    pub kind: RemoteParamKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRemoteParamMetadata<'a> {
    pub name: &'a str,
    pub java_type: &'a str,
    pub kind: RemoteParamKind,
    pub network_included_when_caller_is_client: bool,
    pub network_included_when_caller_is_server: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRemotePacketSpec<'a> {
    pub method: HighFrequencyRemoteMethod,
    pub packet_id: u8,
    pub packet_class: &'a str,
    pub declaring_type: &'a str,
    pub flow: RemoteFlow,
    pub unreliable: bool,
    pub priority: &'a str,
    pub wire_params: Vec<TypedRemoteParamSpec<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCustomChannelRemotePacketSpec<'a> {
    pub family: CustomChannelRemoteFamily,
    pub packet_id: u8,
    pub packet_class: &'a str,
    pub declaring_type: &'a str,
    pub method: &'a str,
    pub flow: RemoteFlow,
    pub unreliable: bool,
    pub payload_kind: CustomChannelRemotePayloadKind,
    pub wire_params: Vec<TypedRemoteParamSpec<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInboundRemotePacketSpec<'a> {
    pub family: InboundRemoteFamily,
    pub packet_id: u8,
    pub packet_class: &'a str,
    pub declaring_type: &'a str,
    pub method: &'a str,
    pub flow: RemoteFlow,
    pub unreliable: bool,
    pub payload_kind: CustomChannelRemotePayloadKind,
    pub wire_params: Vec<TypedRemoteParamSpec<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRemotePacketMetadata<'a> {
    pub remote_index: usize,
    pub packet_id: u8,
    pub packet_class: &'a str,
    pub declaring_type: &'a str,
    pub method: &'a str,
    pub called: &'a str,
    pub variants: &'a str,
    pub flow: RemoteFlow,
    pub forward: bool,
    pub unreliable: bool,
    pub priority: RemotePriority,
    pub params: Vec<TypedRemoteParamMetadata<'a>>,
    pub wire_params: Vec<TypedRemoteParamSpec<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomChannelRemoteDispatchSpec {
    pub family: CustomChannelRemoteFamily,
    pub payload_kind: CustomChannelRemotePayloadKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboundRemoteDispatchSpec {
    pub family: InboundRemoteFamily,
    pub payload_kind: CustomChannelRemotePayloadKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteMethodSelector<'a> {
    Name(&'a str),
    HighFrequency(HighFrequencyRemoteMethod),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemotePacketSelector<'a> {
    pub method: RemoteMethodSelector<'a>,
    pub flow: Option<RemoteFlow>,
    pub unreliable: Option<bool>,
    pub param_java_types: &'a [&'a str],
    pub wire_param_kinds: &'a [RemoteParamKind],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePacketRegistry<'a> {
    packets: Vec<TypedRemotePacketMetadata<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighFrequencyRemoteRegistry {
    by_packet_id: [(u8, HighFrequencyRemoteMethod); HIGH_FREQUENCY_REMOTE_METHOD_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomChannelRemoteRegistry {
    by_packet_id: [(u8, CustomChannelRemoteDispatchSpec); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRemoteRegistry {
    by_packet_id: [(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WellKnownRemoteRegistry {
    by_packet_id: RemotePacketIdFixedTable<WellKnownRemoteMethod>,
    by_method: [(WellKnownRemoteMethod, Option<u8>); WELL_KNOWN_REMOTE_METHOD_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePacketIdFixedTable<T: Copy> {
    by_packet_id: [Option<T>; REMOTE_PACKET_ID_SPACE],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRemoteRegistries {
    pub high_frequency: HighFrequencyRemoteRegistry,
    pub custom_channel: CustomChannelRemoteRegistry,
    pub inbound_remote: InboundRemoteRegistry,
    pub well_known: WellKnownRemoteRegistry,
}

const SERVER_PACKET_TEXT_PARAM_JAVA_TYPES: [&str; 3] =
    ["Player", "java.lang.String", "java.lang.String"];
const SERVER_PACKET_BINARY_PARAM_JAVA_TYPES: [&str; 3] = ["Player", "java.lang.String", "byte[]"];
const CLIENT_LOGIC_DATA_PARAM_JAVA_TYPES: [&str; 3] =
    ["Player", "java.lang.String", "java.lang.Object"];
const CLIENT_PACKET_TEXT_PARAM_JAVA_TYPES: [&str; 2] = ["java.lang.String", "java.lang.String"];
const CLIENT_PACKET_BINARY_PARAM_JAVA_TYPES: [&str; 2] = ["java.lang.String", "byte[]"];
const CLIENT_PACKET_TEXT_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Opaque];
const CLIENT_PACKET_BINARY_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Bytes];
const SERVER_PACKET_TEXT_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Opaque];
const SERVER_PACKET_BINARY_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Bytes];
const CLIENT_LOGIC_DATA_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Opaque];
const PING_PARAM_JAVA_TYPES: [&str; 2] = ["Player", "long"];
const PING_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Long];
const CLIENT_PLAN_SNAPSHOT_PARAM_JAVA_TYPES: [&str; 3] = [
    "Player",
    "int",
    "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
];
const CLIENT_PLAN_SNAPSHOT_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Int, RemoteParamKind::BuildPlanQueue];
const CLIENT_PLAN_SNAPSHOT_RECEIVED_PARAM_JAVA_TYPES: [&str; 3] = [
    "Player",
    "int",
    "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
];
const CLIENT_PLAN_SNAPSHOT_RECEIVED_WIRE_PARAM_KINDS: [RemoteParamKind; 3] = [
    RemoteParamKind::Opaque,
    RemoteParamKind::Int,
    RemoteParamKind::BuildPlanQueue,
];
const PING_RESPONSE_PARAM_JAVA_TYPES: [&str; 1] = ["long"];
const PING_RESPONSE_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Long];
const PING_LOCATION_PARAM_JAVA_TYPES: [&str; 4] = ["Player", "float", "float", "java.lang.String"];
const PING_LOCATION_WIRE_PARAM_KINDS: [RemoteParamKind; 4] = [
    RemoteParamKind::Opaque,
    RemoteParamKind::Float,
    RemoteParamKind::Float,
    RemoteParamKind::Opaque,
];
const DEBUG_STATUS_CLIENT_UNRELIABLE_PARAM_JAVA_TYPES: [&str; 3] = ["int", "int", "int"];
const DEBUG_STATUS_CLIENT_UNRELIABLE_WIRE_PARAM_KINDS: [RemoteParamKind; 3] = [
    RemoteParamKind::Int,
    RemoteParamKind::Int,
    RemoteParamKind::Int,
];
const TRACE_INFO_PARAM_JAVA_TYPES: [&str; 2] = ["Player", "mindustry.net.Administration.TraceInfo"];
const TRACE_INFO_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Opaque];
const CONNECT_REDIRECT_PARAM_JAVA_TYPES: [&str; 2] = ["java.lang.String", "int"];
const CONNECT_REDIRECT_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Int];
const CONNECT_CONFIRM_PARAM_JAVA_TYPES: [&str; 1] = ["Player"];
const CONNECT_CONFIRM_WIRE_PARAM_KINDS: [RemoteParamKind; 0] = [];
const PLAYER_SPAWN_PARAM_JAVA_TYPES: [&str; 2] = ["mindustry.world.Tile", "Player"];
const PLAYER_SPAWN_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::TileRef, RemoteParamKind::Opaque];
const KICK_STRING_PARAM_JAVA_TYPES: [&str; 1] = ["java.lang.String"];
const KICK_STRING_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Opaque];
const KICK_REASON_PARAM_JAVA_TYPES: [&str; 1] = ["mindustry.net.Packets.KickReason"];
const KICK_REASON_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Opaque];
const SEND_CHAT_MESSAGE_PARAM_JAVA_TYPES: [&str; 2] = ["Player", "java.lang.String"];
const SEND_CHAT_MESSAGE_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Opaque];
const SEND_MESSAGE_PARAM_JAVA_TYPES: [&str; 1] = ["java.lang.String"];
const SEND_MESSAGE_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Opaque];
const SEND_MESSAGE_WITH_SENDER_PARAM_JAVA_TYPES: [&str; 3] =
    ["java.lang.String", "java.lang.String", "Player"];
const SEND_MESSAGE_WITH_SENDER_WIRE_PARAM_KINDS: [RemoteParamKind; 3] = [
    RemoteParamKind::Opaque,
    RemoteParamKind::Opaque,
    RemoteParamKind::Opaque,
];
const SET_RULES_PARAM_JAVA_TYPES: [&str; 1] = ["mindustry.game.Rules"];
const SET_RULES_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Opaque];
const SET_OBJECTIVES_PARAM_JAVA_TYPES: [&str; 1] = ["mindustry.game.MapObjectives"];
const SET_OBJECTIVES_WIRE_PARAM_KINDS: [RemoteParamKind; 1] = [RemoteParamKind::Opaque];
const SET_RULE_PARAM_JAVA_TYPES: [&str; 2] = ["java.lang.String", "java.lang.String"];
const SET_RULE_WIRE_PARAM_KINDS: [RemoteParamKind; 2] =
    [RemoteParamKind::Opaque, RemoteParamKind::Opaque];
const WORLD_DATA_BEGIN_PARAM_JAVA_TYPES: [&str; 0] = [];
const WORLD_DATA_BEGIN_WIRE_PARAM_KINDS: [RemoteParamKind; 0] = [];

impl<'a> RemoteMethodSelector<'a> {
    fn matches(self, method: &str) -> bool {
        match self {
            Self::Name(name) => method == name,
            Self::HighFrequency(high_frequency) => method == high_frequency.method_name(),
        }
    }
}

impl<'a> RemotePacketSelector<'a> {
    pub fn method(method: &'a str) -> Self {
        Self {
            method: RemoteMethodSelector::Name(method),
            flow: None,
            unreliable: None,
            param_java_types: &[],
            wire_param_kinds: &[],
        }
    }

    pub fn high_frequency(method: HighFrequencyRemoteMethod) -> Self {
        Self {
            method: RemoteMethodSelector::HighFrequency(method),
            flow: None,
            unreliable: None,
            param_java_types: &[],
            wire_param_kinds: &[],
        }
    }

    pub fn with_flow(mut self, flow: RemoteFlow) -> Self {
        self.flow = Some(flow);
        self
    }

    pub fn with_unreliable(mut self, unreliable: bool) -> Self {
        self.unreliable = Some(unreliable);
        self
    }

    pub fn with_param_java_types(mut self, param_java_types: &'a [&'a str]) -> Self {
        self.param_java_types = param_java_types;
        self
    }

    pub fn with_wire_param_kinds(mut self, wire_param_kinds: &'a [RemoteParamKind]) -> Self {
        self.wire_param_kinds = wire_param_kinds;
        self
    }
}

impl WellKnownRemoteMethod {
    pub fn ordered() -> [Self; WELL_KNOWN_REMOTE_METHOD_COUNT] {
        [
            Self::Ping,
            Self::ClientPlanSnapshot,
            Self::ClientPlanSnapshotReceived,
            Self::PingResponse,
            Self::PingLocation,
            Self::DebugStatusClientUnreliable,
            Self::TraceInfo,
            Self::ConnectRedirect,
            Self::ConnectConfirm,
            Self::PlayerSpawn,
            Self::SetRules,
            Self::SetObjectives,
            Self::SetRule,
            Self::WorldDataBegin,
            Self::KickString,
            Self::KickReason,
            Self::SendChatMessage,
            Self::SendMessage,
            Self::SendMessageWithSender,
        ]
    }

    pub fn method_name(self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::ClientPlanSnapshot => "clientPlanSnapshot",
            Self::ClientPlanSnapshotReceived => "clientPlanSnapshotReceived",
            Self::PingResponse => "pingResponse",
            Self::PingLocation => "pingLocation",
            Self::DebugStatusClientUnreliable => "debugStatusClientUnreliable",
            Self::TraceInfo => "traceInfo",
            Self::ConnectRedirect => "connect",
            Self::ConnectConfirm => "connectConfirm",
            Self::PlayerSpawn => "playerSpawn",
            Self::SetRules => "setRules",
            Self::SetObjectives => "setObjectives",
            Self::SetRule => "setRule",
            Self::WorldDataBegin => "worldDataBegin",
            Self::KickString | Self::KickReason => "kick",
            Self::SendChatMessage => "sendChatMessage",
            Self::SendMessage | Self::SendMessageWithSender => "sendMessage",
        }
    }

    pub fn flow(self) -> RemoteFlow {
        match self {
            Self::Ping | Self::ClientPlanSnapshot => RemoteFlow::ClientToServer,
            Self::PingLocation => RemoteFlow::Bidirectional,
            Self::ClientPlanSnapshotReceived
            | Self::PingResponse
            | Self::DebugStatusClientUnreliable
            | Self::TraceInfo
            | Self::ConnectRedirect
            | Self::PlayerSpawn
            | Self::SetRules
            | Self::SetObjectives
            | Self::SetRule
            | Self::WorldDataBegin
            | Self::KickString
            | Self::KickReason
            | Self::SendMessage
            | Self::SendMessageWithSender => RemoteFlow::ServerToClient,
            Self::ConnectConfirm | Self::SendChatMessage => RemoteFlow::ClientToServer,
        }
    }

    pub fn unreliable(self) -> bool {
        matches!(
            self,
            Self::ClientPlanSnapshot
                | Self::ClientPlanSnapshotReceived
                | Self::DebugStatusClientUnreliable
        )
    }

    pub fn param_java_types(self) -> &'static [&'static str] {
        match self {
            Self::Ping => &PING_PARAM_JAVA_TYPES,
            Self::ClientPlanSnapshot => &CLIENT_PLAN_SNAPSHOT_PARAM_JAVA_TYPES,
            Self::ClientPlanSnapshotReceived => &CLIENT_PLAN_SNAPSHOT_RECEIVED_PARAM_JAVA_TYPES,
            Self::PingResponse => &PING_RESPONSE_PARAM_JAVA_TYPES,
            Self::PingLocation => &PING_LOCATION_PARAM_JAVA_TYPES,
            Self::DebugStatusClientUnreliable => &DEBUG_STATUS_CLIENT_UNRELIABLE_PARAM_JAVA_TYPES,
            Self::TraceInfo => &TRACE_INFO_PARAM_JAVA_TYPES,
            Self::ConnectRedirect => &CONNECT_REDIRECT_PARAM_JAVA_TYPES,
            Self::ConnectConfirm => &CONNECT_CONFIRM_PARAM_JAVA_TYPES,
            Self::PlayerSpawn => &PLAYER_SPAWN_PARAM_JAVA_TYPES,
            Self::SetRules => &SET_RULES_PARAM_JAVA_TYPES,
            Self::SetObjectives => &SET_OBJECTIVES_PARAM_JAVA_TYPES,
            Self::SetRule => &SET_RULE_PARAM_JAVA_TYPES,
            Self::WorldDataBegin => &WORLD_DATA_BEGIN_PARAM_JAVA_TYPES,
            Self::KickString => &KICK_STRING_PARAM_JAVA_TYPES,
            Self::KickReason => &KICK_REASON_PARAM_JAVA_TYPES,
            Self::SendChatMessage => &SEND_CHAT_MESSAGE_PARAM_JAVA_TYPES,
            Self::SendMessage => &SEND_MESSAGE_PARAM_JAVA_TYPES,
            Self::SendMessageWithSender => &SEND_MESSAGE_WITH_SENDER_PARAM_JAVA_TYPES,
        }
    }

    pub fn wire_param_kinds(self) -> &'static [RemoteParamKind] {
        match self {
            Self::Ping => &PING_WIRE_PARAM_KINDS,
            Self::ClientPlanSnapshot => &CLIENT_PLAN_SNAPSHOT_WIRE_PARAM_KINDS,
            Self::ClientPlanSnapshotReceived => &CLIENT_PLAN_SNAPSHOT_RECEIVED_WIRE_PARAM_KINDS,
            Self::PingResponse => &PING_RESPONSE_WIRE_PARAM_KINDS,
            Self::PingLocation => &PING_LOCATION_WIRE_PARAM_KINDS,
            Self::DebugStatusClientUnreliable => &DEBUG_STATUS_CLIENT_UNRELIABLE_WIRE_PARAM_KINDS,
            Self::TraceInfo => &TRACE_INFO_WIRE_PARAM_KINDS,
            Self::ConnectRedirect => &CONNECT_REDIRECT_WIRE_PARAM_KINDS,
            Self::ConnectConfirm => &CONNECT_CONFIRM_WIRE_PARAM_KINDS,
            Self::PlayerSpawn => &PLAYER_SPAWN_WIRE_PARAM_KINDS,
            Self::SetRules => &SET_RULES_WIRE_PARAM_KINDS,
            Self::SetObjectives => &SET_OBJECTIVES_WIRE_PARAM_KINDS,
            Self::SetRule => &SET_RULE_WIRE_PARAM_KINDS,
            Self::WorldDataBegin => &WORLD_DATA_BEGIN_WIRE_PARAM_KINDS,
            Self::KickString => &KICK_STRING_WIRE_PARAM_KINDS,
            Self::KickReason => &KICK_REASON_WIRE_PARAM_KINDS,
            Self::SendChatMessage => &SEND_CHAT_MESSAGE_WIRE_PARAM_KINDS,
            Self::SendMessage => &SEND_MESSAGE_WIRE_PARAM_KINDS,
            Self::SendMessageWithSender => &SEND_MESSAGE_WITH_SENDER_WIRE_PARAM_KINDS,
        }
    }

    pub fn selector(self) -> RemotePacketSelector<'static> {
        RemotePacketSelector::method(self.method_name())
            .with_flow(self.flow())
            .with_unreliable(self.unreliable())
            .with_param_java_types(self.param_java_types())
            .with_wire_param_kinds(self.wire_param_kinds())
    }
}

impl CustomChannelRemoteFamily {
    pub fn ordered() -> [Self; CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT] {
        [
            Self::ClientPacketReliable,
            Self::ClientPacketUnreliable,
            Self::ClientBinaryPacketReliable,
            Self::ClientBinaryPacketUnreliable,
            Self::ServerPacketReliable,
            Self::ServerPacketUnreliable,
            Self::ServerBinaryPacketReliable,
            Self::ServerBinaryPacketUnreliable,
            Self::ClientLogicDataReliable,
            Self::ClientLogicDataUnreliable,
        ]
    }

    pub fn method_name(self) -> &'static str {
        match self {
            Self::ClientPacketReliable => "clientPacketReliable",
            Self::ClientPacketUnreliable => "clientPacketUnreliable",
            Self::ClientBinaryPacketReliable => "clientBinaryPacketReliable",
            Self::ClientBinaryPacketUnreliable => "clientBinaryPacketUnreliable",
            Self::ServerPacketReliable => "serverPacketReliable",
            Self::ServerPacketUnreliable => "serverPacketUnreliable",
            Self::ServerBinaryPacketReliable => "serverBinaryPacketReliable",
            Self::ServerBinaryPacketUnreliable => "serverBinaryPacketUnreliable",
            Self::ClientLogicDataReliable => "clientLogicDataReliable",
            Self::ClientLogicDataUnreliable => "clientLogicDataUnreliable",
        }
    }

    fn variant_name(self) -> &'static str {
        match self {
            Self::ClientPacketReliable => "ClientPacketReliable",
            Self::ClientPacketUnreliable => "ClientPacketUnreliable",
            Self::ClientBinaryPacketReliable => "ClientBinaryPacketReliable",
            Self::ClientBinaryPacketUnreliable => "ClientBinaryPacketUnreliable",
            Self::ServerPacketReliable => "ServerPacketReliable",
            Self::ServerPacketUnreliable => "ServerPacketUnreliable",
            Self::ServerBinaryPacketReliable => "ServerBinaryPacketReliable",
            Self::ServerBinaryPacketUnreliable => "ServerBinaryPacketUnreliable",
            Self::ClientLogicDataReliable => "ClientLogicDataReliable",
            Self::ClientLogicDataUnreliable => "ClientLogicDataUnreliable",
        }
    }

    fn const_prefix(self) -> &'static str {
        match self {
            Self::ClientPacketReliable => "CLIENT_PACKET_RELIABLE",
            Self::ClientPacketUnreliable => "CLIENT_PACKET_UNRELIABLE",
            Self::ClientBinaryPacketReliable => "CLIENT_BINARY_PACKET_RELIABLE",
            Self::ClientBinaryPacketUnreliable => "CLIENT_BINARY_PACKET_UNRELIABLE",
            Self::ServerPacketReliable => "SERVER_PACKET_RELIABLE",
            Self::ServerPacketUnreliable => "SERVER_PACKET_UNRELIABLE",
            Self::ServerBinaryPacketReliable => "SERVER_BINARY_PACKET_RELIABLE",
            Self::ServerBinaryPacketUnreliable => "SERVER_BINARY_PACKET_UNRELIABLE",
            Self::ClientLogicDataReliable => "CLIENT_LOGIC_DATA_RELIABLE",
            Self::ClientLogicDataUnreliable => "CLIENT_LOGIC_DATA_UNRELIABLE",
        }
    }

    pub fn unreliable(self) -> bool {
        matches!(
            self,
            Self::ClientPacketUnreliable
                | Self::ClientBinaryPacketUnreliable
                | Self::ServerPacketUnreliable
                | Self::ServerBinaryPacketUnreliable
                | Self::ClientLogicDataUnreliable
        )
    }

    pub fn payload_kind(self) -> CustomChannelRemotePayloadKind {
        match self {
            Self::ClientPacketReliable
            | Self::ClientPacketUnreliable
            | Self::ServerPacketReliable
            | Self::ServerPacketUnreliable => CustomChannelRemotePayloadKind::Text,
            Self::ClientBinaryPacketReliable
            | Self::ClientBinaryPacketUnreliable
            | Self::ServerBinaryPacketReliable
            | Self::ServerBinaryPacketUnreliable => CustomChannelRemotePayloadKind::Binary,
            Self::ClientLogicDataReliable | Self::ClientLogicDataUnreliable => {
                CustomChannelRemotePayloadKind::LogicData
            }
        }
    }

    pub fn dispatch_spec(self) -> CustomChannelRemoteDispatchSpec {
        CustomChannelRemoteDispatchSpec {
            family: self,
            payload_kind: self.payload_kind(),
        }
    }

    pub fn param_java_types(self) -> &'static [&'static str] {
        match self {
            Self::ClientPacketReliable | Self::ClientPacketUnreliable => {
                &CLIENT_PACKET_TEXT_PARAM_JAVA_TYPES
            }
            Self::ClientBinaryPacketReliable | Self::ClientBinaryPacketUnreliable => {
                &CLIENT_PACKET_BINARY_PARAM_JAVA_TYPES
            }
            Self::ServerPacketReliable | Self::ServerPacketUnreliable => {
                &SERVER_PACKET_TEXT_PARAM_JAVA_TYPES
            }
            Self::ServerBinaryPacketReliable | Self::ServerBinaryPacketUnreliable => {
                &SERVER_PACKET_BINARY_PARAM_JAVA_TYPES
            }
            Self::ClientLogicDataReliable | Self::ClientLogicDataUnreliable => {
                &CLIENT_LOGIC_DATA_PARAM_JAVA_TYPES
            }
        }
    }

    pub fn wire_param_kinds(self) -> &'static [RemoteParamKind] {
        match self {
            Self::ClientPacketReliable | Self::ClientPacketUnreliable => {
                &CLIENT_PACKET_TEXT_WIRE_PARAM_KINDS
            }
            Self::ClientBinaryPacketReliable | Self::ClientBinaryPacketUnreliable => {
                &CLIENT_PACKET_BINARY_WIRE_PARAM_KINDS
            }
            Self::ServerPacketReliable | Self::ServerPacketUnreliable => {
                &SERVER_PACKET_TEXT_WIRE_PARAM_KINDS
            }
            Self::ServerBinaryPacketReliable | Self::ServerBinaryPacketUnreliable => {
                &SERVER_PACKET_BINARY_WIRE_PARAM_KINDS
            }
            Self::ClientLogicDataReliable | Self::ClientLogicDataUnreliable => {
                &CLIENT_LOGIC_DATA_WIRE_PARAM_KINDS
            }
        }
    }

    pub fn selector(self) -> RemotePacketSelector<'static> {
        RemotePacketSelector::method(self.method_name())
            .with_flow(self.selector_flow())
            .with_unreliable(self.unreliable())
            .with_param_java_types(self.param_java_types())
            .with_wire_param_kinds(self.wire_param_kinds())
    }

    pub fn inbound_remote_family(self) -> Option<InboundRemoteFamily> {
        match self {
            Self::ServerPacketReliable => Some(InboundRemoteFamily::ServerPacketReliable),
            Self::ServerPacketUnreliable => Some(InboundRemoteFamily::ServerPacketUnreliable),
            Self::ServerBinaryPacketReliable => {
                Some(InboundRemoteFamily::ServerBinaryPacketReliable)
            }
            Self::ServerBinaryPacketUnreliable => {
                Some(InboundRemoteFamily::ServerBinaryPacketUnreliable)
            }
            Self::ClientLogicDataReliable => Some(InboundRemoteFamily::ClientLogicDataReliable),
            Self::ClientLogicDataUnreliable => Some(InboundRemoteFamily::ClientLogicDataUnreliable),
            Self::ClientPacketReliable
            | Self::ClientPacketUnreliable
            | Self::ClientBinaryPacketReliable
            | Self::ClientBinaryPacketUnreliable => None,
        }
    }

    fn selector_flow(self) -> RemoteFlow {
        // Manifest `targets=client` packets currently normalize to
        // `RemoteFlow::ClientToServer`, while `targets=server` packets normalize to
        // `RemoteFlow::ServerToClient`. Keep that normalization local here so
        // downstream registry code can consume typed families instead of
        // restating target/flow quirks at each call site.
        match self {
            Self::ClientPacketReliable
            | Self::ClientPacketUnreliable
            | Self::ClientBinaryPacketReliable
            | Self::ClientBinaryPacketUnreliable => RemoteFlow::ServerToClient,
            Self::ServerPacketReliable
            | Self::ServerPacketUnreliable
            | Self::ServerBinaryPacketReliable
            | Self::ServerBinaryPacketUnreliable
            | Self::ClientLogicDataReliable
            | Self::ClientLogicDataUnreliable => RemoteFlow::ClientToServer,
        }
    }
}

impl InboundRemoteFamily {
    pub fn ordered() -> [Self; INBOUND_REMOTE_FAMILY_COUNT] {
        [
            Self::ServerPacketReliable,
            Self::ServerPacketUnreliable,
            Self::ServerBinaryPacketReliable,
            Self::ServerBinaryPacketUnreliable,
            Self::ClientLogicDataReliable,
            Self::ClientLogicDataUnreliable,
        ]
    }

    pub fn method_name(self) -> &'static str {
        self.custom_channel_family().method_name()
    }

    fn variant_name(self) -> &'static str {
        match self {
            Self::ServerPacketReliable => "ServerPacketReliable",
            Self::ServerPacketUnreliable => "ServerPacketUnreliable",
            Self::ServerBinaryPacketReliable => "ServerBinaryPacketReliable",
            Self::ServerBinaryPacketUnreliable => "ServerBinaryPacketUnreliable",
            Self::ClientLogicDataReliable => "ClientLogicDataReliable",
            Self::ClientLogicDataUnreliable => "ClientLogicDataUnreliable",
        }
    }

    fn const_prefix(self) -> &'static str {
        match self {
            Self::ServerPacketReliable => "SERVER_PACKET_RELIABLE",
            Self::ServerPacketUnreliable => "SERVER_PACKET_UNRELIABLE",
            Self::ServerBinaryPacketReliable => "SERVER_BINARY_PACKET_RELIABLE",
            Self::ServerBinaryPacketUnreliable => "SERVER_BINARY_PACKET_UNRELIABLE",
            Self::ClientLogicDataReliable => "CLIENT_LOGIC_DATA_RELIABLE",
            Self::ClientLogicDataUnreliable => "CLIENT_LOGIC_DATA_UNRELIABLE",
        }
    }

    pub fn unreliable(self) -> bool {
        self.custom_channel_family().unreliable()
    }

    pub fn payload_kind(self) -> CustomChannelRemotePayloadKind {
        self.custom_channel_family().payload_kind()
    }

    pub fn dispatch_spec(self) -> InboundRemoteDispatchSpec {
        InboundRemoteDispatchSpec {
            family: self,
            payload_kind: self.payload_kind(),
        }
    }

    pub fn param_java_types(self) -> &'static [&'static str] {
        self.custom_channel_family().param_java_types()
    }

    pub fn wire_param_kinds(self) -> &'static [RemoteParamKind] {
        self.custom_channel_family().wire_param_kinds()
    }

    pub fn selector(self) -> RemotePacketSelector<'static> {
        self.custom_channel_family().selector()
    }

    pub fn custom_channel_family(self) -> CustomChannelRemoteFamily {
        match self {
            Self::ServerPacketReliable => CustomChannelRemoteFamily::ServerPacketReliable,
            Self::ServerPacketUnreliable => CustomChannelRemoteFamily::ServerPacketUnreliable,
            Self::ServerBinaryPacketReliable => {
                CustomChannelRemoteFamily::ServerBinaryPacketReliable
            }
            Self::ServerBinaryPacketUnreliable => {
                CustomChannelRemoteFamily::ServerBinaryPacketUnreliable
            }
            Self::ClientLogicDataReliable => CustomChannelRemoteFamily::ClientLogicDataReliable,
            Self::ClientLogicDataUnreliable => CustomChannelRemoteFamily::ClientLogicDataUnreliable,
        }
    }
}

pub fn read_remote_manifest(path: impl AsRef<Path>) -> Result<RemoteManifest, RemoteManifestError> {
    let text = fs::read_to_string(path)?;
    parse_remote_manifest(&text)
}

pub fn parse_remote_manifest(text: &str) -> Result<RemoteManifest, RemoteManifestError> {
    let manifest_value = serde_json::from_str::<StrictJsonValue>(text)?.0;
    let manifest: RemoteManifest = serde_json::from_value(manifest_value)?;
    validate_remote_manifest(&manifest)?;
    Ok(manifest)
}

#[derive(Debug)]
struct StrictJsonValue(serde_json::Value);

impl<'de> Deserialize<'de> for StrictJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer
            .deserialize_any(StrictJsonValueVisitor)
            .map(Self)
    }
}

struct StrictJsonValueVisitor;

impl<'de> de::Visitor<'de> for StrictJsonValueVisitor {
    type Value = serde_json::Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| de::Error::custom("invalid JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(serde_json::Value::String(value.to_owned()))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(serde_json::Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(serde_json::Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element::<StrictJsonValue>()? {
            values.push(value.0);
        }
        Ok(serde_json::Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut object = serde_json::Map::new();
        let mut keys = HashSet::new();

        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate JSON key: {key}")));
            }

            let value = map.next_value::<StrictJsonValue>()?;
            object.insert(key, value.0);
        }

        Ok(serde_json::Value::Object(object))
    }
}

pub fn validate_remote_manifest(manifest: &RemoteManifest) -> Result<(), RemoteManifestError> {
    if manifest.schema != REMOTE_MANIFEST_SCHEMA_V1 {
        return Err(RemoteManifestError::UnsupportedSchema(
            manifest.schema.clone(),
        ));
    }

    validate_remote_generator_info(&manifest.generator)?;
    validate_wire_spec(&manifest.wire)?;

    if manifest.base_packets.len() > u8::MAX as usize {
        return Err(RemoteManifestError::InvalidPacketSequence(format!(
            "base packet count exceeds u8 packet id range: {}",
            manifest.base_packets.len()
        )));
    }

    let mut seen_base_packet_classes = HashSet::with_capacity(manifest.base_packets.len());
    for (index, packet) in manifest.base_packets.iter().enumerate() {
        if packet.id != index as u8 {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "base packet {} has id {}, expected {}",
                packet.class_name, packet.id, index
            )));
        }
        if packet.class_name.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "base packet {} has empty class_name",
                packet.id
            )));
        }
        if !seen_base_packet_classes.insert(packet.class_name.as_str()) {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "duplicate base packet class_name: {}",
                packet.class_name
            )));
        }
    }

    let remote_id_offset = manifest.base_packets.len();
    let mut seen_remote_packet_definitions =
        std::collections::HashSet::with_capacity(manifest.remote_packets.len());
    let mut seen_remote_packet_classes =
        std::collections::HashSet::with_capacity(manifest.remote_packets.len());
    let mut seen_remote_packet_ids =
        std::collections::HashSet::with_capacity(manifest.remote_packets.len());
    let mut seen_remote_packet_const_names =
        std::collections::HashSet::with_capacity(manifest.remote_packets.len());
    for (index, packet) in manifest.remote_packets.iter().enumerate() {
        if packet.remote_index != index {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "remote packet {} has remoteIndex {}, expected {}",
                packet.packet_class, packet.remote_index, index
            )));
        }

        if !seen_remote_packet_ids.insert(packet.packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate remote packetId: {}",
                packet.packet_id
            )));
        }

        let expected_packet_id = remote_id_offset + index;
        if expected_packet_id > u8::MAX as usize {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "remote packet {} has packetId {}, expected packet id exceeds u8 range",
                packet.packet_class, packet.packet_id
            )));
        }

        let expected_packet_id = expected_packet_id as u8;
        if packet.packet_id != expected_packet_id {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "remote packet {} has packetId {}, expected {}",
                packet.packet_class, packet.packet_id, expected_packet_id
            )));
        }

        if packet.packet_class.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has empty packetClass",
                packet.packet_id
            )));
        }
        if !seen_remote_packet_classes.insert(packet.packet_class.as_str()) {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "duplicate remote packetClass: {}",
                packet.packet_class
            )));
        }
        let packet_const_name = remote_packet_const_name_raw(&packet.packet_class);
        if packet_const_name.is_empty()
            || packet_const_name
                .as_bytes()
                .first()
                .is_some_and(|byte| byte.is_ascii_digit())
        {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has packetClass that would generate invalid Rust const name: {}",
                packet.packet_class, packet_const_name
            )));
        }
        let packet_const_name = remote_packet_const_name(&packet.packet_class);
        if !seen_remote_packet_const_names.insert(packet_const_name.clone()) {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "duplicate generated remote packet const name: {}",
                packet_const_name
            )));
        }
        if packet.declaring_type.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has empty declaringType",
                packet.packet_class
            )));
        }
        if packet.method.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has empty method",
                packet.packet_class
            )));
        }
        if packet.called.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has empty called",
                packet.packet_class
            )));
        }
        remote_called_from_str(&packet.called)?;
        if packet.variants.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has empty variants",
                packet.packet_class
            )));
        }
        remote_variants_from_str(&packet.variants)?;

        let flow = remote_flow_from_targets(&packet.targets)?;
        validate_remote_allow_flags(packet, flow)?;
        remote_priority_from_str(&packet.priority)?;
        let mut seen_param_names = HashSet::with_capacity(packet.params.len());

        for param in &packet.params {
            if param.name.trim().is_empty() {
                return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "remote packet {} has param with empty name",
                    packet.packet_class
                )));
            }
            if !seen_param_names.insert(param.name.as_str()) {
                return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "remote packet {} has duplicate param name: {}",
                    packet.packet_class, param.name
                )));
            }
            if param.java_type.trim().is_empty() {
                return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "remote packet {} param {} has empty javaType",
                    packet.packet_class, param.name
                )));
            }
        }

        if !seen_remote_packet_definitions.insert(remote_packet_definition_key(packet, flow)) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate remote packet definition: {}",
                packet.packet_class
            )));
        }
    }

    Ok(())
}

fn validate_wire_spec(wire: &WireSpec) -> Result<(), RemoteManifestError> {
    if wire.packet_id_byte != REMOTE_WIRE_PACKET_ID_BYTE_U8 {
        return Err(RemoteManifestError::InvalidWireSpec(format!(
            "unsupported wire packetIdByte: {}, expected {}",
            wire.packet_id_byte, REMOTE_WIRE_PACKET_ID_BYTE_U8
        )));
    }

    if wire.length_field != REMOTE_WIRE_LENGTH_FIELD_U16BE {
        return Err(RemoteManifestError::InvalidWireSpec(format!(
            "unsupported wire lengthField: {}, expected {}",
            wire.length_field, REMOTE_WIRE_LENGTH_FIELD_U16BE
        )));
    }

    if wire.compression_flag.none != REMOTE_WIRE_COMPRESSION_NONE {
        return Err(RemoteManifestError::InvalidWireSpec(format!(
            "unsupported wire compressionFlag[0]: {}, expected {}",
            wire.compression_flag.none, REMOTE_WIRE_COMPRESSION_NONE
        )));
    }

    if wire.compression_flag.lz4 != REMOTE_WIRE_COMPRESSION_LZ4 {
        return Err(RemoteManifestError::InvalidWireSpec(format!(
            "unsupported wire compressionFlag[1]: {}, expected {}",
            wire.compression_flag.lz4, REMOTE_WIRE_COMPRESSION_LZ4
        )));
    }

    if wire.compression_threshold != REMOTE_WIRE_COMPRESSION_THRESHOLD {
        return Err(RemoteManifestError::InvalidWireSpec(format!(
            "unsupported wire compressionThreshold: {}, expected {}",
            wire.compression_threshold, REMOTE_WIRE_COMPRESSION_THRESHOLD
        )));
    }

    Ok(())
}

fn remote_packet_definition_key(packet: &RemotePacketEntry, flow: RemoteFlow) -> String {
    let wire_flow = match flow {
        RemoteFlow::Bidirectional => bidirectional_wire_flow(packet),
        _ => flow,
    };
    let param_java_types = packet
        .params
        .iter()
        .map(|param| param.java_type.as_str())
        .collect::<Vec<_>>();
    let wire_param_kinds = packet
        .params
        .iter()
        .filter(|param| {
            param_is_wire_included_client_server(
                param.network_included_when_caller_is_client,
                param.network_included_when_caller_is_server,
                wire_flow,
            )
        })
        .map(|param| remote_param_kind(&param.java_type))
        .collect::<Vec<_>>();

    format!(
        "{}|{:?}|{}|{:?}|{:?}",
        packet.method, flow, packet.unreliable, param_java_types, wire_param_kinds
    )
}

pub fn high_frequency_remote_packets(
    manifest: &RemoteManifest,
) -> Result<Vec<TypedRemotePacketSpec<'_>>, RemoteManifestError> {
    let registry = RemotePacketRegistry::from_manifest(manifest)?;
    let mut packets = Vec::with_capacity(HIGH_FREQUENCY_REMOTE_METHOD_COUNT);
    for method in HighFrequencyRemoteMethod::ordered() {
        let entry = registry.first_high_frequency_method(method).ok_or(
            RemoteManifestError::MissingHighFrequencyPacket(method.method_name()),
        )?;

        packets.push(TypedRemotePacketSpec {
            method,
            packet_id: entry.packet_id,
            packet_class: entry.packet_class,
            declaring_type: entry.declaring_type,
            flow: entry.flow,
            unreliable: entry.unreliable,
            priority: entry.priority.as_str(),
            wire_params: entry.wire_params.clone(),
        });
    }

    Ok(packets)
}

pub fn typed_remote_packets(
    manifest: &RemoteManifest,
) -> Result<Vec<TypedRemotePacketMetadata<'_>>, RemoteManifestError> {
    Ok(RemotePacketRegistry::from_manifest(manifest)?.into_packets())
}

pub fn custom_channel_remote_packets(
    manifest: &RemoteManifest,
) -> Result<Vec<TypedCustomChannelRemotePacketSpec<'_>>, RemoteManifestError> {
    let registry = RemotePacketRegistry::from_manifest(manifest)?;
    let mut packets = Vec::with_capacity(CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);
    let mut seen_packet_ids =
        std::collections::HashSet::with_capacity(CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);

    for family in CustomChannelRemoteFamily::ordered() {
        let entry = registry.first_custom_channel_remote_family(family).ok_or(
            RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "missing custom-channel remote family packet in manifest: {}",
                family.method_name(),
            )),
        )?;
        if !seen_packet_ids.insert(entry.packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate custom-channel remote family packet id: {}",
                entry.packet_id
            )));
        }
        packets.push(TypedCustomChannelRemotePacketSpec {
            family,
            packet_id: entry.packet_id,
            packet_class: entry.packet_class,
            declaring_type: entry.declaring_type,
            method: entry.method,
            flow: entry.flow,
            unreliable: entry.unreliable,
            payload_kind: family.payload_kind(),
            wire_params: entry.wire_params.clone(),
        });
    }

    Ok(packets)
}

pub fn inbound_remote_packets(
    manifest: &RemoteManifest,
) -> Result<Vec<TypedInboundRemotePacketSpec<'_>>, RemoteManifestError> {
    let registry = RemotePacketRegistry::from_manifest(manifest)?;
    let mut packets = Vec::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);
    let mut seen_packet_ids = std::collections::HashSet::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);

    for family in InboundRemoteFamily::ordered() {
        let entry = registry.first_inbound_remote_family(family).ok_or(
            RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "missing inbound remote family packet in manifest: {}",
                family.method_name(),
            )),
        )?;
        if !seen_packet_ids.insert(entry.packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate inbound remote family packet id: {}",
                entry.packet_id
            )));
        }
        packets.push(TypedInboundRemotePacketSpec {
            family,
            packet_id: entry.packet_id,
            packet_class: entry.packet_class,
            declaring_type: entry.declaring_type,
            method: entry.method,
            flow: entry.flow,
            unreliable: entry.unreliable,
            payload_kind: family.payload_kind(),
            wire_params: entry.wire_params.clone(),
        });
    }

    Ok(packets)
}

pub fn typed_custom_channel_remote_dispatch_specs(
    manifest: &RemoteManifest,
) -> Result<
    [(u8, CustomChannelRemoteDispatchSpec); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT],
    RemoteManifestError,
> {
    let packets = custom_channel_remote_packets(manifest)?;
    let resolved_entries = packets
        .into_iter()
        .map(|packet| (packet.packet_id, packet.family.dispatch_spec()))
        .collect::<Vec<_>>();

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "custom-channel remote dispatch registry length drifted".into(),
        )
    })
}

pub fn typed_inbound_remote_dispatch_specs(
    manifest: &RemoteManifest,
) -> Result<[(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT], RemoteManifestError> {
    let packets = inbound_remote_packets(manifest)?;
    let resolved_entries = packets
        .into_iter()
        .map(|packet| (packet.packet_id, packet.family.dispatch_spec()))
        .collect::<Vec<_>>();

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "inbound remote dispatch registry length drifted".into(),
        )
    })
}

fn validate_remote_packet_id_space(manifest: &RemoteManifest) -> Result<(), RemoteManifestError> {
    let packet_id_space = manifest
        .base_packets
        .len()
        .checked_add(manifest.remote_packets.len())
        .ok_or_else(|| {
            RemoteManifestError::InvalidPacketSequence(
                "remote packet id space exceeds u8 packet id range".into(),
            )
        })?;
    if packet_id_space > REMOTE_PACKET_ID_SPACE {
        return Err(RemoteManifestError::InvalidPacketSequence(format!(
            "remote packet id space exceeds u8 packet id range: {}",
            packet_id_space
        )));
    }
    Ok(())
}

pub fn generate_rust_registry(manifest: &RemoteManifest) -> Result<String, RemoteManifestError> {
    validate_remote_packet_id_space(manifest)?;
    validate_remote_manifest(manifest)?;
    let mut out = StringBuilder::new();
    out.push_line("// @generated by mdt-remote from remote-manifest-v1.json");
    out.push_line(&format!(
        "pub const REMOTE_MANIFEST_SCHEMA: &str = {:?};",
        manifest.schema
    ));
    out.push_line(&format!(
        "pub const REMOTE_BASE_PACKET_COUNT: usize = {};",
        manifest.base_packets.len()
    ));
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct RemotePacketSpec {");
    out.push_line("    pub packet_id: u8,");
    out.push_line("    pub packet_class: &'static str,");
    out.push_line("    pub declaring_type: &'static str,");
    out.push_line("    pub method: &'static str,");
    out.push_line("    pub targets: &'static str,");
    out.push_line("    pub called: &'static str,");
    out.push_line("    pub variants: &'static str,");
    out.push_line("    pub unreliable: bool,");
    out.push_line("    pub forward: bool,");
    out.push_line("    pub priority: &'static str,");
    out.push_line("    pub param_count: usize,");
    out.push_line("}");
    out.push_line("");
    for packet in &manifest.remote_packets {
        out.push_line(&format!(
            "pub const {}_ID: u8 = {};",
            remote_packet_const_name(&packet.packet_class),
            packet.packet_id
        ));
    }
    out.push_line("");
    out.push_line("pub const REMOTE_PACKET_SPECS: &[RemotePacketSpec] = &[");
    for packet in &manifest.remote_packets {
        out.push_line("    RemotePacketSpec {");
        out.push_line(&format!("        packet_id: {},", packet.packet_id));
        out.push_line(&format!("        packet_class: {:?},", packet.packet_class));
        out.push_line(&format!(
            "        declaring_type: {:?},",
            packet.declaring_type
        ));
        out.push_line(&format!("        method: {:?},", packet.method));
        out.push_line(&format!("        targets: {:?},", packet.targets));
        out.push_line(&format!("        called: {:?},", packet.called));
        out.push_line(&format!("        variants: {:?},", packet.variants));
        out.push_line(&format!("        unreliable: {},", packet.unreliable));
        out.push_line(&format!("        forward: {},", packet.forward));
        out.push_line(&format!("        priority: {:?},", packet.priority));
        out.push_line(&format!("        param_count: {},", packet.params.len()));
        out.push_line("    },");
    }
    out.push_line("];");
    Ok(out.finish())
}

pub fn generate_high_frequency_rust_module(
    manifest: &RemoteManifest,
) -> Result<String, RemoteManifestError> {
    let packets = high_frequency_remote_packets(manifest)?;
    let mut out = StringBuilder::new();
    out.push_line("// @generated by mdt-remote from remote-manifest-v1.json");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum RemoteFlow {");
    out.push_line("    ClientToServer,");
    out.push_line("    ServerToClient,");
    out.push_line("    Bidirectional,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum HighFrequencyRemoteMethod {");
    for method in HighFrequencyRemoteMethod::ordered() {
        out.push_line(&format!("    {},", method.variant_name()));
    }
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum RemoteParamKind {");
    out.push_line("    Bool,");
    out.push_line("    Byte,");
    out.push_line("    Short,");
    out.push_line("    Int,");
    out.push_line("    Long,");
    out.push_line("    Float,");
    out.push_line("    Bytes,");
    out.push_line("    TileRef,");
    out.push_line("    BlockRef,");
    out.push_line("    BuildPlanQueue,");
    out.push_line("    IntSeq,");
    out.push_line("    Opaque,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct HighFrequencyRemoteParamSpec {");
    out.push_line("    pub name: &'static str,");
    out.push_line("    pub java_type: &'static str,");
    out.push_line("    pub kind: RemoteParamKind,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct HighFrequencyRemotePacketSpec {");
    out.push_line("    pub method: HighFrequencyRemoteMethod,");
    out.push_line("    pub packet_id: u8,");
    out.push_line("    pub packet_class: &'static str,");
    out.push_line("    pub declaring_type: &'static str,");
    out.push_line("    pub flow: RemoteFlow,");
    out.push_line("    pub unreliable: bool,");
    out.push_line("    pub priority: &'static str,");
    out.push_line("    pub wire_params: &'static [HighFrequencyRemoteParamSpec],");
    out.push_line("}");
    out.push_line("");

    for packet in &packets {
        let const_prefix = packet.method.const_prefix();
        out.push_line(&format!(
            "pub const {const_prefix}_PACKET_ID: u8 = {};",
            packet.packet_id
        ));
    }
    out.push_line("");

    for packet in &packets {
        let const_prefix = packet.method.const_prefix();
        out.push_line(&format!(
            "pub const {const_prefix}_WIRE_PARAMS: &[HighFrequencyRemoteParamSpec] = &["
        ));
        for param in &packet.wire_params {
            out.push_line("    HighFrequencyRemoteParamSpec {");
            out.push_line(&format!("        name: {:?},", param.name));
            out.push_line(&format!("        java_type: {:?},", param.java_type));
            out.push_line(&format!(
                "        kind: RemoteParamKind::{},",
                remote_param_kind_name(param.kind)
            ));
            out.push_line("    },");
        }
        out.push_line("];");
        out.push_line("");
    }

    out.push_line(
        "pub const HIGH_FREQUENCY_REMOTE_PACKET_SPECS: &[HighFrequencyRemotePacketSpec] = &[",
    );
    for packet in &packets {
        let const_prefix = packet.method.const_prefix();
        out.push_line("    HighFrequencyRemotePacketSpec {");
        out.push_line(&format!(
            "        method: HighFrequencyRemoteMethod::{},",
            packet.method.variant_name()
        ));
        out.push_line(&format!("        packet_id: {},", packet.packet_id));
        out.push_line(&format!("        packet_class: {:?},", packet.packet_class));
        out.push_line(&format!(
            "        declaring_type: {:?},",
            packet.declaring_type
        ));
        out.push_line(&format!(
            "        flow: RemoteFlow::{},",
            remote_flow_name(packet.flow)
        ));
        out.push_line(&format!("        unreliable: {},", packet.unreliable));
        out.push_line(&format!("        priority: {:?},", packet.priority));
        out.push_line(&format!("        wire_params: {const_prefix}_WIRE_PARAMS,"));
        out.push_line("    },");
    }
    out.push_line("];");
    Ok(out.finish())
}

pub fn generate_inbound_dispatch_rust_module(
    manifest: &RemoteManifest,
) -> Result<String, RemoteManifestError> {
    let custom_packets = custom_channel_remote_packets(manifest)?;
    let inbound_packets = inbound_remote_packets(manifest)?;
    let mut out = StringBuilder::new();
    out.push_line("// @generated by mdt-remote from remote-manifest-v1.json");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum RemoteFlow {");
    out.push_line("    ClientToServer,");
    out.push_line("    ServerToClient,");
    out.push_line("    Bidirectional,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum CustomChannelRemotePayloadKind {");
    out.push_line("    Text,");
    out.push_line("    Binary,");
    out.push_line("    LogicData,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum CustomChannelRemoteFamily {");
    for family in CustomChannelRemoteFamily::ordered() {
        out.push_line(&format!("    {},", family.variant_name()));
    }
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub enum InboundRemoteFamily {");
    for family in InboundRemoteFamily::ordered() {
        out.push_line(&format!("    {},", family.variant_name()));
    }
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct CustomChannelRemoteDispatchSpec {");
    out.push_line("    pub family: CustomChannelRemoteFamily,");
    out.push_line("    pub payload_kind: CustomChannelRemotePayloadKind,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct InboundRemoteDispatchSpec {");
    out.push_line("    pub family: InboundRemoteFamily,");
    out.push_line("    pub payload_kind: CustomChannelRemotePayloadKind,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct CustomChannelRemotePacketSpec {");
    out.push_line("    pub family: CustomChannelRemoteFamily,");
    out.push_line("    pub packet_id: u8,");
    out.push_line("    pub packet_class: &'static str,");
    out.push_line("    pub declaring_type: &'static str,");
    out.push_line("    pub method: &'static str,");
    out.push_line("    pub flow: RemoteFlow,");
    out.push_line("    pub unreliable: bool,");
    out.push_line("    pub payload_kind: CustomChannelRemotePayloadKind,");
    out.push_line("}");
    out.push_line("");
    out.push_line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    out.push_line("pub struct InboundRemotePacketSpec {");
    out.push_line("    pub family: InboundRemoteFamily,");
    out.push_line("    pub packet_id: u8,");
    out.push_line("    pub packet_class: &'static str,");
    out.push_line("    pub declaring_type: &'static str,");
    out.push_line("    pub method: &'static str,");
    out.push_line("    pub flow: RemoteFlow,");
    out.push_line("    pub unreliable: bool,");
    out.push_line("    pub payload_kind: CustomChannelRemotePayloadKind,");
    out.push_line("}");
    out.push_line("");

    for packet in &custom_packets {
        out.push_line(&format!(
            "pub const CUSTOM_CHANNEL_{}_PACKET_ID: u8 = {};",
            packet.family.const_prefix(),
            packet.packet_id
        ));
    }
    out.push_line("");
    for packet in &inbound_packets {
        out.push_line(&format!(
            "pub const INBOUND_{}_PACKET_ID: u8 = {};",
            packet.family.const_prefix(),
            packet.packet_id
        ));
    }
    out.push_line("");
    out.push_line(
        "pub const CUSTOM_CHANNEL_REMOTE_PACKET_SPECS: &[CustomChannelRemotePacketSpec] = &[",
    );
    for packet in &custom_packets {
        out.push_line("    CustomChannelRemotePacketSpec {");
        out.push_line(&format!(
            "        family: CustomChannelRemoteFamily::{},",
            packet.family.variant_name()
        ));
        out.push_line(&format!("        packet_id: {},", packet.packet_id));
        out.push_line(&format!("        packet_class: {:?},", packet.packet_class));
        out.push_line(&format!(
            "        declaring_type: {:?},",
            packet.declaring_type
        ));
        out.push_line(&format!("        method: {:?},", packet.method));
        out.push_line(&format!(
            "        flow: RemoteFlow::{},",
            remote_flow_name(packet.flow)
        ));
        out.push_line(&format!("        unreliable: {},", packet.unreliable));
        out.push_line(&format!(
            "        payload_kind: CustomChannelRemotePayloadKind::{},",
            packet.payload_kind.variant_name()
        ));
        out.push_line("    },");
    }
    out.push_line("];");
    out.push_line("");
    out.push_line("pub const INBOUND_REMOTE_PACKET_SPECS: &[InboundRemotePacketSpec] = &[");
    for packet in &inbound_packets {
        out.push_line("    InboundRemotePacketSpec {");
        out.push_line(&format!(
            "        family: InboundRemoteFamily::{},",
            packet.family.variant_name()
        ));
        out.push_line(&format!("        packet_id: {},", packet.packet_id));
        out.push_line(&format!("        packet_class: {:?},", packet.packet_class));
        out.push_line(&format!(
            "        declaring_type: {:?},",
            packet.declaring_type
        ));
        out.push_line(&format!("        method: {:?},", packet.method));
        out.push_line(&format!(
            "        flow: RemoteFlow::{},",
            remote_flow_name(packet.flow)
        ));
        out.push_line(&format!("        unreliable: {},", packet.unreliable));
        out.push_line(&format!(
            "        payload_kind: CustomChannelRemotePayloadKind::{},",
            packet.payload_kind.variant_name()
        ));
        out.push_line("    },");
    }
    out.push_line("];");
    out.push_line("");
    out.push_line(
        "pub const fn custom_channel_remote_dispatch_spec(packet_id: u8) -> Option<CustomChannelRemoteDispatchSpec> {",
    );
    out.push_line("    match packet_id {");
    for packet in &custom_packets {
        out.push_line(&format!(
            "        {} => Some(CustomChannelRemoteDispatchSpec {{ family: CustomChannelRemoteFamily::{}, payload_kind: CustomChannelRemotePayloadKind::{} }}),",
            packet.packet_id,
            packet.family.variant_name(),
            packet.payload_kind.variant_name()
        ));
    }
    out.push_line("        _ => None,");
    out.push_line("    }");
    out.push_line("}");
    out.push_line("");
    out.push_line(
        "pub const fn inbound_remote_dispatch_spec(packet_id: u8) -> Option<InboundRemoteDispatchSpec> {",
    );
    out.push_line("    match packet_id {");
    for packet in &inbound_packets {
        out.push_line(&format!(
            "        {} => Some(InboundRemoteDispatchSpec {{ family: InboundRemoteFamily::{}, payload_kind: CustomChannelRemotePayloadKind::{} }}),",
            packet.packet_id,
            packet.family.variant_name(),
            packet.payload_kind.variant_name()
        ));
    }
    out.push_line("        _ => None,");
    out.push_line("    }");
    out.push_line("}");
    Ok(out.finish())
}

pub fn remote_packet_const_name(packet_class: &str) -> String {
    let raw_name = remote_packet_const_name_raw(packet_class);
    if raw_name.is_empty() {
        return "PACKET".into();
    }
    if raw_name
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        return format!("_{}", raw_name);
    }
    raw_name
}

fn remote_packet_const_name_raw(packet_class: &str) -> String {
    let simple_name = packet_class
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(packet_class);
    let mut out = String::with_capacity(simple_name.len() + 8);
    let mut previous_is_lower_or_digit = false;
    for ch in simple_name.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_is_lower_or_digit && !out.ends_with('_') {
                out.push('_');
            }
            out.push(ch.to_ascii_uppercase());
            previous_is_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            previous_is_lower_or_digit = false;
        }
    }
    out.trim_end_matches('_').to_string()
}

fn remote_flow_from_targets(targets: &str) -> Result<RemoteFlow, RemoteManifestError> {
    match targets {
        "client" => Ok(RemoteFlow::ClientToServer),
        "server" => Ok(RemoteFlow::ServerToClient),
        "both" => Ok(RemoteFlow::Bidirectional),
        _ => Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
            "unsupported remote targets: {targets}"
        ))),
    }
}

fn validate_remote_allow_flags(
    packet: &RemotePacketEntry,
    flow: RemoteFlow,
) -> Result<(), RemoteManifestError> {
    let expected_allow_on_client =
        matches!(flow, RemoteFlow::ServerToClient | RemoteFlow::Bidirectional);
    let expected_allow_on_server =
        matches!(flow, RemoteFlow::ClientToServer | RemoteFlow::Bidirectional);

    match (packet.allow_on_client, packet.allow_on_server) {
        (None, None) => Ok(()),
        (Some(allow_on_client), Some(allow_on_server))
            if allow_on_client == expected_allow_on_client
                && allow_on_server == expected_allow_on_server =>
        {
            Ok(())
        }
        (Some(allow_on_client), Some(allow_on_server)) => {
            Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has allowOnClient/allowOnServer drift for targets {}: expected allowOnClient={}, allowOnServer={}, found allowOnClient={}, allowOnServer={}",
                packet.packet_class,
                packet.targets,
                expected_allow_on_client,
                expected_allow_on_server,
                allow_on_client,
                allow_on_server
            )))
        }
        _ => Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
            "remote packet {} must set allowOnClient and allowOnServer together",
            packet.packet_class
        ))),
    }
}

fn remote_called_from_str(called: &str) -> Result<(), RemoteManifestError> {
    match called {
        "server" | "client" | "both" | "none" => Ok(()),
        _ => Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
            "unsupported remote called: {called}"
        ))),
    }
}

fn remote_variants_from_str(variants: &str) -> Result<(), RemoteManifestError> {
    match variants {
        "all" | "one" | "both" => Ok(()),
        _ => Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
            "unsupported remote variants: {variants}"
        ))),
    }
}

fn validate_remote_generator_info(
    generator: &RemoteGeneratorInfo,
) -> Result<(), RemoteManifestError> {
    if generator.source.trim().is_empty() {
        return Err(RemoteManifestError::InvalidRemotePacketMetadata(
            "remote manifest generator source must not be empty".to_string(),
        ));
    }
    if generator.call_class.trim().is_empty() {
        return Err(RemoteManifestError::InvalidRemotePacketMetadata(
            "remote manifest generator callClass must not be empty".to_string(),
        ));
    }

    Ok(())
}

fn remote_priority_from_str(priority: &str) -> Result<RemotePriority, RemoteManifestError> {
    match priority {
        "low" => Ok(RemotePriority::Low),
        "normal" => Ok(RemotePriority::Normal),
        "high" => Ok(RemotePriority::High),
        _ => Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
            "unsupported remote priority: {priority}"
        ))),
    }
}

fn bidirectional_wire_flow(packet: &RemotePacketEntry) -> RemoteFlow {
    let mut has_client_only = false;
    let mut has_server_only = false;

    for param in &packet.params {
        match (
            param.network_included_when_caller_is_client,
            param.network_included_when_caller_is_server,
        ) {
            (true, false) => has_client_only = true,
            (false, true) => has_server_only = true,
            _ => {}
        }
    }

    if has_client_only {
        RemoteFlow::ClientToServer
    } else if has_server_only {
        RemoteFlow::ServerToClient
    } else {
        RemoteFlow::ClientToServer
    }
}

fn param_is_wire_included_client_server(
    network_included_when_caller_is_client: bool,
    network_included_when_caller_is_server: bool,
    flow: RemoteFlow,
) -> bool {
    match flow {
        RemoteFlow::ClientToServer => network_included_when_caller_is_client,
        RemoteFlow::ServerToClient => network_included_when_caller_is_server,
        RemoteFlow::Bidirectional => unreachable!("bidirectional flow should be normalized first"),
    }
}

fn remote_param_kind(java_type: &str) -> RemoteParamKind {
    match java_type {
        "boolean" => RemoteParamKind::Bool,
        "byte" => RemoteParamKind::Byte,
        "short" => RemoteParamKind::Short,
        "int" => RemoteParamKind::Int,
        "long" => RemoteParamKind::Long,
        "float" => RemoteParamKind::Float,
        "byte[]" => RemoteParamKind::Bytes,
        "mindustry.world.Tile" => RemoteParamKind::TileRef,
        "mindustry.world.Block" => RemoteParamKind::BlockRef,
        "arc.struct.Queue<mindustry.entities.units.BuildPlan>" => RemoteParamKind::BuildPlanQueue,
        "arc.struct.IntSeq" => RemoteParamKind::IntSeq,
        _ => RemoteParamKind::Opaque,
    }
}

fn remote_flow_name(flow: RemoteFlow) -> &'static str {
    match flow {
        RemoteFlow::ClientToServer => "ClientToServer",
        RemoteFlow::ServerToClient => "ServerToClient",
        RemoteFlow::Bidirectional => "Bidirectional",
    }
}

fn remote_param_kind_name(kind: RemoteParamKind) -> &'static str {
    match kind {
        RemoteParamKind::Bool => "Bool",
        RemoteParamKind::Byte => "Byte",
        RemoteParamKind::Short => "Short",
        RemoteParamKind::Int => "Int",
        RemoteParamKind::Long => "Long",
        RemoteParamKind::Float => "Float",
        RemoteParamKind::Bytes => "Bytes",
        RemoteParamKind::TileRef => "TileRef",
        RemoteParamKind::BlockRef => "BlockRef",
        RemoteParamKind::BuildPlanQueue => "BuildPlanQueue",
        RemoteParamKind::IntSeq => "IntSeq",
        RemoteParamKind::Opaque => "Opaque",
    }
}

impl RemotePriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

impl<'a> RemotePacketRegistry<'a> {
    pub fn from_manifest(manifest: &'a RemoteManifest) -> Result<Self, RemoteManifestError> {
        let packets = manifest
            .remote_packets
            .iter()
            .map(typed_remote_packet_metadata)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { packets })
    }

    pub fn packets(&self) -> &[TypedRemotePacketMetadata<'a>] {
        &self.packets
    }

    pub fn get_by_packet_id(&self, packet_id: u8) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.packets
            .iter()
            .find(|packet| packet.packet_id == packet_id)
    }

    pub fn get_by_packet_class(
        &self,
        packet_class: &str,
    ) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.packets
            .iter()
            .find(|packet| packet.packet_class == packet_class)
    }

    pub fn packets_for_method(&self, method: &str) -> Vec<&TypedRemotePacketMetadata<'a>> {
        self.packets
            .iter()
            .filter(|packet| packet.method == method)
            .collect()
    }

    pub fn packets_matching(
        &self,
        selector: RemotePacketSelector<'_>,
    ) -> Vec<&TypedRemotePacketMetadata<'a>> {
        self.packets
            .iter()
            .filter(|packet| packet.matches_selector(&selector))
            .collect()
    }

    pub fn first_matching(
        &self,
        selector: RemotePacketSelector<'_>,
    ) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.packets_matching(selector).into_iter().next()
    }

    pub fn first_high_frequency_method(
        &self,
        method: HighFrequencyRemoteMethod,
    ) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.first_matching(method.selector())
    }

    pub fn first_inbound_remote_family(
        &self,
        family: InboundRemoteFamily,
    ) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.first_matching(family.selector())
    }

    pub fn first_custom_channel_remote_family(
        &self,
        family: CustomChannelRemoteFamily,
    ) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.first_matching(family.selector())
    }

    pub fn first_well_known_method(
        &self,
        method: WellKnownRemoteMethod,
    ) -> Option<&TypedRemotePacketMetadata<'a>> {
        self.first_matching(method.selector())
    }

    pub fn into_packets(self) -> Vec<TypedRemotePacketMetadata<'a>> {
        self.packets
    }
}

impl TypedRemoteRegistries {
    pub fn from_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        Self::from_remote_registry(&registry)
    }

    pub fn from_remote_registry(
        registry: &RemotePacketRegistry<'_>,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            high_frequency: HighFrequencyRemoteRegistry::from_remote_registry(registry)?,
            custom_channel: CustomChannelRemoteRegistry::from_remote_registry(registry)?,
            inbound_remote: InboundRemoteRegistry::from_remote_registry(registry)?,
            well_known: WellKnownRemoteRegistry::from_remote_registry(registry)?,
        })
    }
}

impl<T: Copy> RemotePacketIdFixedTable<T> {
    pub fn from_entries<const N: usize>(entries: &[(u8, T); N]) -> Self {
        Self::from_iter(entries.iter().copied())
    }

    fn from_iter(entries: impl IntoIterator<Item = (u8, T)>) -> Self {
        let mut by_packet_id = [None; REMOTE_PACKET_ID_SPACE];
        for (packet_id, value) in entries {
            assert!(
                by_packet_id[packet_id as usize].is_none(),
                "duplicate packet id in fixed table: {packet_id}"
            );
            by_packet_id[packet_id as usize] = Some(value);
        }
        Self { by_packet_id }
    }

    pub fn get(&self, packet_id: u8) -> Option<T> {
        self.by_packet_id[packet_id as usize]
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.get(packet_id).is_some()
    }
}

impl HighFrequencyRemoteRegistry {
    pub fn from_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        Self::from_remote_registry(&registry)
    }

    pub fn from_remote_registry(
        registry: &RemotePacketRegistry<'_>,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            by_packet_id: resolve_high_frequency_remote_registry_entries(registry)?,
        })
    }

    pub fn classify(&self, packet_id: u8) -> Option<HighFrequencyRemoteMethod> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, method)| {
                (*known_packet_id == packet_id).then_some(*method)
            })
    }

    pub fn packet_id(&self, method: HighFrequencyRemoteMethod) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, known_method)| (*known_method == method).then_some(*packet_id))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id
            .iter()
            .any(|(known_packet_id, _)| *known_packet_id == packet_id)
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }

    pub fn resolved_packet_ids(
        &self,
    ) -> [(u8, HighFrequencyRemoteMethod); HIGH_FREQUENCY_REMOTE_METHOD_COUNT] {
        self.by_packet_id
    }

    pub fn packet_id_fixed_table(&self) -> RemotePacketIdFixedTable<HighFrequencyRemoteMethod> {
        RemotePacketIdFixedTable::from_entries(&self.by_packet_id)
    }
}

impl CustomChannelRemoteRegistry {
    pub fn from_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        Self::from_remote_registry(&registry)
    }

    pub fn from_remote_registry(
        registry: &RemotePacketRegistry<'_>,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            by_packet_id: resolve_custom_channel_remote_dispatch_entries(registry)?,
        })
    }

    pub fn classify(&self, packet_id: u8) -> Option<CustomChannelRemoteFamily> {
        self.dispatch_spec(packet_id).map(|spec| spec.family)
    }

    pub fn dispatch_spec(&self, packet_id: u8) -> Option<CustomChannelRemoteDispatchSpec> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, spec)| (*known_packet_id == packet_id).then_some(*spec))
    }

    pub fn packet_id(&self, family: CustomChannelRemoteFamily) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, spec)| (spec.family == family).then_some(*packet_id))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id
            .iter()
            .any(|(known_packet_id, _)| *known_packet_id == packet_id)
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }

    pub fn resolved_dispatch_specs(
        &self,
    ) -> [(u8, CustomChannelRemoteDispatchSpec); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT] {
        self.by_packet_id
    }

    pub fn packet_id_fixed_table(
        &self,
    ) -> RemotePacketIdFixedTable<CustomChannelRemoteDispatchSpec> {
        RemotePacketIdFixedTable::from_entries(&self.by_packet_id)
    }
}

impl InboundRemoteRegistry {
    pub fn from_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        Self::from_remote_registry(&registry)
    }

    pub fn from_remote_registry(
        registry: &RemotePacketRegistry<'_>,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            by_packet_id: resolve_inbound_remote_dispatch_entries(registry)?,
        })
    }

    pub fn classify(&self, packet_id: u8) -> Option<InboundRemoteFamily> {
        self.dispatch_spec(packet_id).map(|spec| spec.family)
    }

    pub fn dispatch_spec(&self, packet_id: u8) -> Option<InboundRemoteDispatchSpec> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, spec)| (*known_packet_id == packet_id).then_some(*spec))
    }

    pub fn packet_id(&self, family: InboundRemoteFamily) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, spec)| (spec.family == family).then_some(*packet_id))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id
            .iter()
            .any(|(known_packet_id, _)| *known_packet_id == packet_id)
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }

    pub fn resolved_dispatch_specs(
        &self,
    ) -> [(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT] {
        self.by_packet_id
    }

    pub fn packet_id_fixed_table(&self) -> RemotePacketIdFixedTable<InboundRemoteDispatchSpec> {
        RemotePacketIdFixedTable::from_entries(&self.by_packet_id)
    }
}

impl WellKnownRemoteRegistry {
    pub fn from_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        Self::from_remote_registry(&registry)
    }

    pub fn classify(&self, packet_id: u8) -> Option<WellKnownRemoteMethod> {
        self.by_packet_id.get(packet_id)
    }

    pub fn packet_id(&self, method: WellKnownRemoteMethod) -> Option<u8> {
        self.by_method
            .iter()
            .find_map(|(known_method, packet_id)| (*known_method == method).then_some(*packet_id))
            .flatten()
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id.contains_packet_id(packet_id)
    }

    pub fn len(&self) -> usize {
        self.by_method
            .iter()
            .filter(|(_, packet_id)| packet_id.is_some())
            .count()
    }

    pub fn resolved_packet_ids(
        &self,
    ) -> [(WellKnownRemoteMethod, Option<u8>); WELL_KNOWN_REMOTE_METHOD_COUNT] {
        self.by_method
    }

    pub fn packet_id_fixed_table(&self) -> RemotePacketIdFixedTable<WellKnownRemoteMethod> {
        self.by_packet_id.clone()
    }

    pub fn from_remote_registry(
        registry: &RemotePacketRegistry<'_>,
    ) -> Result<Self, RemoteManifestError> {
        let by_method = resolve_well_known_remote_registry_entries(registry)?;
        let by_packet_id = RemotePacketIdFixedTable::from_iter(
            by_method
                .iter()
                .filter_map(|(method, packet_id)| packet_id.map(|packet_id| (packet_id, *method))),
        );
        Ok(Self {
            by_packet_id,
            by_method,
        })
    }
}

fn resolve_high_frequency_remote_registry_entries(
    registry: &RemotePacketRegistry<'_>,
) -> Result<
    [(u8, HighFrequencyRemoteMethod); HIGH_FREQUENCY_REMOTE_METHOD_COUNT],
    RemoteManifestError,
> {
    let mut resolved_entries = Vec::with_capacity(HIGH_FREQUENCY_REMOTE_METHOD_COUNT);
    let mut seen_packet_ids =
        std::collections::HashSet::with_capacity(HIGH_FREQUENCY_REMOTE_METHOD_COUNT);

    for method in HighFrequencyRemoteMethod::ordered() {
        let packet_id = registry
            .first_high_frequency_method(method)
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                method.method_name(),
            ))?
            .packet_id;
        if !seen_packet_ids.insert(packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate high-frequency remote packet id: {packet_id}",
            )));
        }
        resolved_entries.push((packet_id, method));
    }

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "high-frequency remote registry length drifted".into(),
        )
    })
}

fn resolve_custom_channel_remote_dispatch_entries(
    registry: &RemotePacketRegistry<'_>,
) -> Result<
    [(u8, CustomChannelRemoteDispatchSpec); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT],
    RemoteManifestError,
> {
    let mut resolved_entries = Vec::with_capacity(CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);
    let mut seen_packet_ids =
        std::collections::HashSet::with_capacity(CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);

    for family in CustomChannelRemoteFamily::ordered() {
        let packet_id = registry
            .first_custom_channel_remote_family(family)
            .ok_or(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "missing custom-channel remote family packet in manifest: {}",
                family.method_name(),
            )))?
            .packet_id;
        if !seen_packet_ids.insert(packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate custom-channel remote family packet id: {packet_id}",
            )));
        }
        resolved_entries.push((packet_id, family.dispatch_spec()));
    }

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "custom-channel remote dispatch registry length drifted".into(),
        )
    })
}

fn resolve_inbound_remote_dispatch_entries(
    registry: &RemotePacketRegistry<'_>,
) -> Result<[(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT], RemoteManifestError> {
    let mut resolved_entries = Vec::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);
    let mut seen_packet_ids = std::collections::HashSet::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);

    for family in InboundRemoteFamily::ordered() {
        let packet_id = registry
            .first_inbound_remote_family(family)
            .ok_or(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "missing inbound remote family packet in manifest: {}",
                family.method_name(),
            )))?
            .packet_id;
        if !seen_packet_ids.insert(packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate inbound remote family packet id: {packet_id}",
            )));
        }
        resolved_entries.push((packet_id, family.dispatch_spec()));
    }

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "inbound remote dispatch registry length drifted".into(),
        )
    })
}

fn resolve_well_known_remote_registry_entries(
    registry: &RemotePacketRegistry<'_>,
) -> Result<
    [(WellKnownRemoteMethod, Option<u8>); WELL_KNOWN_REMOTE_METHOD_COUNT],
    RemoteManifestError,
> {
    let mut resolved_entries = Vec::with_capacity(WELL_KNOWN_REMOTE_METHOD_COUNT);
    let mut seen_packet_ids =
        std::collections::HashSet::with_capacity(WELL_KNOWN_REMOTE_METHOD_COUNT);

    for method in WellKnownRemoteMethod::ordered() {
        let packet_id = registry
            .first_well_known_method(method)
            .map(|packet| packet.packet_id);
        if let Some(packet_id) = packet_id {
            if !seen_packet_ids.insert(packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate well-known remote packet id: {packet_id}",
                )));
            }
        }
        resolved_entries.push((method, packet_id));
    }

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "well-known remote registry length drifted".into(),
        )
    })
}

impl TypedRemotePacketMetadata<'_> {
    pub fn matches_selector(&self, selector: &RemotePacketSelector<'_>) -> bool {
        selector.method.matches(self.method)
            && selector.flow.is_none_or(|flow| self.flow == flow)
            && selector
                .unreliable
                .is_none_or(|unreliable| self.unreliable == unreliable)
            && (selector.param_java_types.is_empty()
                || self.params.len() == selector.param_java_types.len()
                    && self
                        .params
                        .iter()
                        .zip(selector.param_java_types.iter())
                        .all(|(param, expected_java_type)| param.java_type == *expected_java_type))
            && (selector.wire_param_kinds.is_empty()
                || self.wire_params.len() == selector.wire_param_kinds.len()
                    && self
                        .wire_params
                        .iter()
                        .zip(selector.wire_param_kinds.iter())
                        .all(|(param, expected_kind)| param.kind == *expected_kind))
    }
}

impl HighFrequencyRemoteMethod {
    pub fn ordered() -> [Self; HIGH_FREQUENCY_REMOTE_METHOD_COUNT] {
        [
            Self::ClientSnapshot,
            Self::StateSnapshot,
            Self::EntitySnapshot,
            Self::BlockSnapshot,
            Self::HiddenSnapshot,
        ]
    }

    pub fn method_name(self) -> &'static str {
        match self {
            Self::ClientSnapshot => "clientSnapshot",
            Self::StateSnapshot => "stateSnapshot",
            Self::EntitySnapshot => "entitySnapshot",
            Self::BlockSnapshot => "blockSnapshot",
            Self::HiddenSnapshot => "hiddenSnapshot",
        }
    }

    pub fn flow(self) -> RemoteFlow {
        match self {
            Self::ClientSnapshot => RemoteFlow::ClientToServer,
            Self::StateSnapshot
            | Self::EntitySnapshot
            | Self::BlockSnapshot
            | Self::HiddenSnapshot => RemoteFlow::ServerToClient,
        }
    }

    pub fn unreliable(self) -> bool {
        true
    }

    pub fn selector(self) -> RemotePacketSelector<'static> {
        RemotePacketSelector::high_frequency(self)
            .with_flow(self.flow())
            .with_unreliable(self.unreliable())
    }

    fn variant_name(self) -> &'static str {
        match self {
            Self::ClientSnapshot => "ClientSnapshot",
            Self::StateSnapshot => "StateSnapshot",
            Self::EntitySnapshot => "EntitySnapshot",
            Self::BlockSnapshot => "BlockSnapshot",
            Self::HiddenSnapshot => "HiddenSnapshot",
        }
    }

    fn const_prefix(self) -> &'static str {
        match self {
            Self::ClientSnapshot => "CLIENT_SNAPSHOT",
            Self::StateSnapshot => "STATE_SNAPSHOT",
            Self::EntitySnapshot => "ENTITY_SNAPSHOT",
            Self::BlockSnapshot => "BLOCK_SNAPSHOT",
            Self::HiddenSnapshot => "HIDDEN_SNAPSHOT",
        }
    }
}

impl CustomChannelRemotePayloadKind {
    fn variant_name(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Binary => "Binary",
            Self::LogicData => "LogicData",
        }
    }
}

fn typed_remote_packet_metadata(
    entry: &RemotePacketEntry,
) -> Result<TypedRemotePacketMetadata<'_>, RemoteManifestError> {
    let flow = remote_flow_from_targets(&entry.targets)?;
    let priority = remote_priority_from_str(&entry.priority)?;
    let wire_flow = match flow {
        RemoteFlow::Bidirectional => bidirectional_wire_flow(entry),
        _ => flow,
    };
    let params = entry
        .params
        .iter()
        .map(|param| TypedRemoteParamMetadata {
            name: param.name.as_str(),
            java_type: param.java_type.as_str(),
            kind: remote_param_kind(&param.java_type),
            network_included_when_caller_is_client: param.network_included_when_caller_is_client,
            network_included_when_caller_is_server: param.network_included_when_caller_is_server,
        })
        .collect::<Vec<_>>();
    let wire_params = params
        .iter()
        .filter(|param| {
            param_is_wire_included_client_server(
                param.network_included_when_caller_is_client,
                param.network_included_when_caller_is_server,
                wire_flow,
            )
        })
        .map(|param| TypedRemoteParamSpec {
            name: param.name,
            java_type: param.java_type,
            kind: param.kind,
        })
        .collect::<Vec<_>>();

    Ok(TypedRemotePacketMetadata {
        remote_index: entry.remote_index,
        packet_id: entry.packet_id,
        packet_class: entry.packet_class.as_str(),
        declaring_type: entry.declaring_type.as_str(),
        method: entry.method.as_str(),
        called: entry.called.as_str(),
        variants: entry.variants.as_str(),
        flow,
        forward: entry.forward,
        unreliable: entry.unreliable,
        priority,
        params,
        wire_params,
    })
}

struct StringBuilder {
    inner: String,
}

impl StringBuilder {
    fn new() -> Self {
        Self {
            inner: String::with_capacity(8192),
        }
    }

    fn push_line(&mut self, line: &str) {
        self.inner.push_str(line);
        self.inner.push('\n');
    }

    fn finish(self) -> String {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/remote/remote-manifest-v1.json")
    }

    const SAMPLE_MANIFEST: &str = r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [
    {"id": 0, "class": "mindustry.net.Packets$StreamBegin"},
    {"id": 1, "class": "mindustry.net.Packets$StreamChunk"},
    {"id": 2, "class": "mindustry.net.Packets$WorldStream"},
    {"id": 3, "class": "mindustry.net.Packets$ConnectPacket"}
  ],
  "remotePackets": [
    {
      "remoteIndex": 0,
      "packetId": 4,
      "packetClass": "mindustry.gen.TestCallPacket",
      "declaringType": "mindustry.core.NetServer",
      "method": "test",
      "targets": "client",
      "called": "server",
      "variants": "all",
      "forward": false,
      "unreliable": true,
      "priority": "high",
      "params": [
        {"name": "player", "javaType": "Player", "networkIncludedWhenCallerIsClient": false, "networkIncludedWhenCallerIsServer": false},
        {"name": "value", "javaType": "int", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}
      ]
    }
  ],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  }
}"#;

    #[test]
    fn parses_remote_manifest_sample() {
        let manifest = parse_remote_manifest(SAMPLE_MANIFEST).unwrap();
        assert_eq!(manifest.schema, REMOTE_MANIFEST_SCHEMA_V1);
        assert_eq!(manifest.base_packets.len(), 4);
        assert_eq!(manifest.remote_packets[0].packet_id, 4);
        assert_eq!(manifest.remote_packets[0].params.len(), 2);
    }

    #[test]
    fn parse_remote_manifest_rejects_nested_duplicate_keys() {
        let manifest = r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "source": "overridden",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [],
  "remotePackets": [],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  }
}"#;

        let error = parse_remote_manifest(manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::Json(_)));
        assert!(error.to_string().contains("duplicate JSON key: source"));
    }

    #[test]
    fn parse_remote_manifest_rejects_unknown_manifest_fields() {
        let manifest = r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [],
  "remotePackets": [],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  },
  "unexpectedField": true
}"#;

        let error = parse_remote_manifest(manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::Json(_)));
        assert!(error
            .to_string()
            .contains("unknown field `unexpectedField`"));

        let wire_error = serde_json::from_str::<WireSpec>(
            r#"{
  "packetIdByte": "u8",
  "lengthField": "u16be",
  "compressionFlag": {"0": "none", "1": "lz4"},
  "compressionThreshold": 36,
  "unexpectedField": true
}"#,
        )
        .unwrap_err();
        assert!(wire_error
            .to_string()
            .contains("unknown field `unexpectedField`"));
    }

    #[test]
    fn rejects_packet_id_overflow_in_base_and_remote_sequences() {
        let make_manifest = |base_packets: usize, remote_packet_ids: &[u8]| RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: (0..base_packets)
                .map(|id| BasePacketEntry {
                    id: id as u8,
                    class_name: format!("mindustry.net.Packets$Base{id}"),
                })
                .collect(),
            remote_packets: remote_packet_ids
                .iter()
                .enumerate()
                .map(|(remote_index, &packet_id)| RemotePacketEntry {
                    remote_index,
                    packet_id,
                    packet_class: format!("mindustry.gen.RemotePacket{remote_index}"),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "test".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                })
                .collect(),
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let base_overflow = make_manifest(256, &[]);
        let error = validate_remote_manifest(&base_overflow).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
        assert_eq!(
            error.to_string(),
            "base packet count exceeds u8 packet id range: 256"
        );

        let remote_overflow = make_manifest(255, &[255, 0]);
        let error = validate_remote_manifest(&remote_overflow).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote packet mindustry.gen.RemotePacket1 has packetId 0, expected packet id exceeds u8 range"
        );
    }

    #[test]
    fn validate_remote_manifest_rejects_empty_generator_metadata() {
        let mut manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![],
            remote_packets: vec![],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        manifest.generator.source = "   ".into();
        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote manifest generator source must not be empty"
        );

        manifest.generator.source = "test".into();
        manifest.generator.call_class = "\t".into();
        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote manifest generator callClass must not be empty"
        );
    }

    #[test]
    fn validate_remote_manifest_rejects_base_packet_id_sequence_drift() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
            ],
            remote_packets: vec![],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
        assert_eq!(
            error.to_string(),
            "base packet mindustry.net.Packets$StreamChunk has id 2, expected 1"
        );
    }

    #[test]
    fn validate_remote_manifest_rejects_duplicate_packet_definition() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![],
            remote_packets: vec![
                RemotePacketEntry {
                    remote_index: 0,
                    packet_id: 0,
                    packet_class: "mindustry.gen.DuplicateRemotePacketA".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicateRemotePacket".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
                RemotePacketEntry {
                    remote_index: 1,
                    packet_id: 1,
                    packet_class: "mindustry.gen.DuplicateRemotePacketB".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicateRemotePacket".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
            ],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
        assert_eq!(
            error.to_string(),
            "duplicate remote packet definition: mindustry.gen.DuplicateRemotePacketB"
        );
    }

    #[test]
    fn validate_remote_manifest_rejects_duplicate_remote_packet_id() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![],
            remote_packets: vec![
                RemotePacketEntry {
                    remote_index: 0,
                    packet_id: 0,
                    packet_class: "mindustry.gen.DuplicatePacketIdA".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicatePacketIdA".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
                RemotePacketEntry {
                    remote_index: 1,
                    packet_id: 0,
                    packet_class: "mindustry.gen.DuplicatePacketIdB".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicatePacketIdB".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
            ],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
        assert_eq!(error.to_string(), "duplicate remote packetId: 0");
    }

    #[test]
    fn validate_remote_manifest_rejects_allow_flag_drift() {
        let mut manifest = parse_remote_manifest(SAMPLE_MANIFEST).unwrap();
        manifest.remote_packets[0].allow_on_client = Some(true);
        manifest.remote_packets[0].allow_on_server = Some(true);

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote packet mindustry.gen.TestCallPacket has allowOnClient/allowOnServer drift for targets client: expected allowOnClient=false, allowOnServer=true, found allowOnClient=true, allowOnServer=true"
        );
    }

    #[test]
    fn validate_remote_manifest_rejects_duplicate_packet_class() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![],
            remote_packets: vec![
                RemotePacketEntry {
                    remote_index: 0,
                    packet_id: 0,
                    packet_class: "mindustry.gen.DuplicateRemotePacket".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicateRemotePacketA".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
                RemotePacketEntry {
                    remote_index: 1,
                    packet_id: 1,
                    packet_class: "mindustry.gen.DuplicateRemotePacket".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicateRemotePacketB".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
            ],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "duplicate remote packetClass: mindustry.gen.DuplicateRemotePacket"
        );
    }

    #[test]
    fn validate_remote_manifest_rejects_empty_base_packet_class_name() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![BasePacketEntry {
                id: 0,
                class_name: "   ".into(),
            }],
            remote_packets: vec![],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(error.to_string(), "base packet 0 has empty class_name");
    }

    #[test]
    fn validate_remote_manifest_rejects_duplicate_base_packet_class_name() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
            ],
            remote_packets: vec![],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = validate_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "duplicate base packet class_name: mindustry.net.Packets$StreamBegin"
        );
    }

    #[test]
    fn generate_rust_registry_rejects_duplicate_packet_const_names() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![],
            remote_packets: vec![
                RemotePacketEntry {
                    remote_index: 0,
                    packet_id: 0,
                    packet_class: "mindustry.gen.DuplicateRemotePacketA".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicateRemotePacketA".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
                RemotePacketEntry {
                    remote_index: 1,
                    packet_id: 1,
                    packet_class: "other.gen.DuplicateRemotePacketA".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "duplicateRemotePacketB".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
            ],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = generate_rust_registry(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "duplicate generated remote packet const name: DUPLICATE_REMOTE_PACKET_A"
        );
    }

    #[test]
    fn rejects_wire_packet_id_byte_drift() {
        let manifest =
            SAMPLE_MANIFEST.replace("\"packetIdByte\": \"u8\"", "\"packetIdByte\": \"u16\"");
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::InvalidWireSpec(_)));
        assert_eq!(
            error.to_string(),
            "unsupported wire packetIdByte: u16, expected u8"
        );
    }

    #[test]
    fn rejects_manifest_schema_drift() {
        let manifest = SAMPLE_MANIFEST.replace(
            "\"schema\": \"mdt.remote.manifest.v1\"",
            "\"schema\": \"mdt.remote.manifest.v2\"",
        );
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::UnsupportedSchema(_)));
        assert_eq!(
            error.to_string(),
            "unsupported remote manifest schema: mdt.remote.manifest.v2"
        );
    }

    #[test]
    fn rejects_wire_length_field_drift() {
        let manifest =
            SAMPLE_MANIFEST.replace("\"lengthField\": \"u16be\"", "\"lengthField\": \"u32be\"");
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::InvalidWireSpec(_)));
        assert_eq!(
            error.to_string(),
            "unsupported wire lengthField: u32be, expected u16be"
        );
    }

    #[test]
    fn rejects_wire_compression_flag_drift() {
        let manifest = SAMPLE_MANIFEST.replace(
            "\"compressionFlag\": {\"0\": \"none\", \"1\": \"lz4\"}",
            "\"compressionFlag\": {\"0\": \"raw\", \"1\": \"lz4\"}",
        );
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::InvalidWireSpec(_)));
        assert_eq!(
            error.to_string(),
            "unsupported wire compressionFlag[0]: raw, expected none"
        );
    }

    #[test]
    fn rejects_wire_compression_threshold_drift() {
        let manifest = SAMPLE_MANIFEST.replace(
            "\"compressionThreshold\": 36",
            "\"compressionThreshold\": 35",
        );
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(error, RemoteManifestError::InvalidWireSpec(_)));
        assert_eq!(
            error.to_string(),
            "unsupported wire compressionThreshold: 35, expected 36"
        );
    }

    #[test]
    fn rejects_remote_packet_with_empty_metadata_field() {
        let cases = [
            (
                "\"packetClass\": \"mindustry.gen.TestCallPacket\"",
                "\"packetClass\": \"   \"",
                "remote packet 4 has empty packetClass",
            ),
            (
                "\"declaringType\": \"mindustry.core.NetServer\"",
                "\"declaringType\": \"   \"",
                "remote packet mindustry.gen.TestCallPacket has empty declaringType",
            ),
            (
                "\"method\": \"test\"",
                "\"method\": \"   \"",
                "remote packet mindustry.gen.TestCallPacket has empty method",
            ),
            (
                "\"called\": \"server\"",
                "\"called\": \"   \"",
                "remote packet mindustry.gen.TestCallPacket has empty called",
            ),
            (
                "\"variants\": \"all\"",
                "\"variants\": \"   \"",
                "remote packet mindustry.gen.TestCallPacket has empty variants",
            ),
        ];

        for (needle, replacement, expected_message) in cases {
            let manifest = SAMPLE_MANIFEST.replace(needle, replacement);
            let error = parse_remote_manifest(&manifest).unwrap_err();
            assert!(matches!(
                error,
                RemoteManifestError::InvalidRemotePacketMetadata(_)
            ));
            assert_eq!(error.to_string(), expected_message);
        }
    }

    #[test]
    fn generate_rust_registry_rejects_non_identifier_packet_class() {
        let manifest = SAMPLE_MANIFEST.replace(
            "\"packetClass\": \"mindustry.gen.TestCallPacket\"",
            "\"packetClass\": \"mindustry.gen.1TestCallPacket\"",
        );
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote packet mindustry.gen.1TestCallPacket has packetClass that would generate invalid Rust const name: 1_TEST_CALL_PACKET"
        );
    }

    #[test]
    fn rejects_remote_targets_drift() {
        let manifest = SAMPLE_MANIFEST.replace("\"targets\": \"client\"", "\"targets\": \"all\"");
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(error.to_string(), "unsupported remote targets: all");
    }

    #[test]
    fn rejects_remote_priority_drift() {
        let manifest =
            SAMPLE_MANIFEST.replace("\"priority\": \"high\"", "\"priority\": \"urgent\"");
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(error.to_string(), "unsupported remote priority: urgent");
    }

    #[test]
    fn rejects_remote_called_drift() {
        let manifest = SAMPLE_MANIFEST.replace("\"called\": \"server\"", "\"called\": \"relay\"");
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(error.to_string(), "unsupported remote called: relay");
    }

    #[test]
    fn rejects_remote_variants_drift() {
        let manifest = SAMPLE_MANIFEST.replace("\"variants\": \"all\"", "\"variants\": \"many\"");
        let error = parse_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(error.to_string(), "unsupported remote variants: many");
    }

    #[test]
    fn validate_remote_manifest_rejects_duplicate_param_names() {
        let mut manifest = parse_remote_manifest(SAMPLE_MANIFEST).unwrap();
        manifest.remote_packets[0].params[1].name = "player".into();

        let error = validate_remote_manifest(&manifest).unwrap_err();

        assert!(matches!(
            error,
            RemoteManifestError::InvalidRemotePacketMetadata(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote packet mindustry.gen.TestCallPacket has duplicate param name: player"
        );
    }

    #[test]
    fn generates_rust_registry_from_manifest_sample() {
        let manifest = parse_remote_manifest(SAMPLE_MANIFEST).unwrap();
        let registry = generate_rust_registry(&manifest).unwrap();
        assert!(registry.contains("pub const TEST_CALL_PACKET_ID: u8 = 4;"));
        assert!(registry.contains("pub const REMOTE_PACKET_SPECS: &[RemotePacketSpec] = &["));
        assert!(registry.contains("priority: \"high\""));
    }

    #[test]
    fn rejects_remote_packet_id_space_overflow() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.into(),
            generator: RemoteGeneratorInfo {
                source: "test".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: (0..255)
                .map(|id| BasePacketEntry {
                    id: id as u8,
                    class_name: format!("mindustry.net.Packets$Base{id}"),
                })
                .collect(),
            remote_packets: vec![
                RemotePacketEntry {
                    remote_index: 0,
                    packet_id: 255,
                    packet_class: "mindustry.gen.RemotePacket0".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "test".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
                RemotePacketEntry {
                    remote_index: 1,
                    packet_id: 0,
                    packet_class: "mindustry.gen.RemotePacket1".into(),
                    declaring_type: "mindustry.core.NetServer".into(),
                    method: "test".into(),
                    targets: "client".into(),
                    called: "server".into(),
                    variants: "all".into(),
                    allow_on_client: None,
                    allow_on_server: None,
                    forward: false,
                    unreliable: true,
                    priority: "high".into(),
                    params: vec![],
                },
            ],
            wire: WireSpec {
                packet_id_byte: REMOTE_WIRE_PACKET_ID_BYTE_U8.into(),
                length_field: REMOTE_WIRE_LENGTH_FIELD_U16BE.into(),
                compression_flag: CompressionFlagSpec {
                    none: REMOTE_WIRE_COMPRESSION_NONE.into(),
                    lz4: REMOTE_WIRE_COMPRESSION_LZ4.into(),
                },
                compression_threshold: REMOTE_WIRE_COMPRESSION_THRESHOLD,
            },
        };

        let error = generate_rust_registry(&manifest).unwrap_err();
        assert!(matches!(
            error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
        assert_eq!(
            error.to_string(),
            "remote packet id space exceeds u8 packet id range: 257"
        );
    }

    #[test]
    fn bidirectional_wire_params_preserve_directional_shape() {
        let manifest = parse_remote_manifest(
            r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [
    {"id": 0, "class": "mindustry.net.Packets$StreamBegin"},
    {"id": 1, "class": "mindustry.net.Packets$StreamChunk"},
    {"id": 2, "class": "mindustry.net.Packets$WorldStream"},
    {"id": 3, "class": "mindustry.net.Packets$ConnectPacket"}
  ],
  "remotePackets": [
    {
      "remoteIndex": 0,
      "packetId": 4,
      "packetClass": "mindustry.gen.BidirectionalShapeCallPacket",
      "declaringType": "mindustry.core.NetClient",
      "method": "bidirectionalShape",
      "targets": "both",
      "called": "both",
      "variants": "both",
      "forward": false,
      "unreliable": false,
      "priority": "normal",
      "params": [
        {"name": "shared", "javaType": "java.lang.String", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true},
        {"name": "clientOnly", "javaType": "int", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": false},
        {"name": "serverOnly", "javaType": "float", "networkIncludedWhenCallerIsClient": false, "networkIncludedWhenCallerIsServer": true}
      ]
    }
  ],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  }
}"#,
        )
        .unwrap();

        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        let packet = registry.get_by_packet_id(4).unwrap();

        assert_eq!(packet.flow, RemoteFlow::Bidirectional);
        assert_eq!(packet.params.len(), 3);
        assert_eq!(packet.wire_params.len(), 2);
        assert_eq!(packet.wire_params[0].name, "shared");
        assert_eq!(packet.wire_params[1].name, "clientOnly");
        assert_eq!(packet.params[1].name, "clientOnly");
        assert_eq!(packet.params[2].name, "serverOnly");
    }

    #[test]
    fn bidirectional_wire_flow_all_shared_falls_back_to_client_to_server() {
        let manifest = parse_remote_manifest(
            r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [
    {"id": 0, "class": "mindustry.net.Packets$StreamBegin"},
    {"id": 1, "class": "mindustry.net.Packets$StreamChunk"},
    {"id": 2, "class": "mindustry.net.Packets$WorldStream"},
    {"id": 3, "class": "mindustry.net.Packets$ConnectPacket"}
  ],
  "remotePackets": [
    {
      "remoteIndex": 0,
      "packetId": 4,
      "packetClass": "mindustry.gen.BidirectionalSharedCallPacket",
      "declaringType": "mindustry.core.NetClient",
      "method": "bidirectionalShared",
      "targets": "both",
      "called": "both",
      "variants": "both",
      "forward": false,
      "unreliable": false,
      "priority": "normal",
      "params": [
        {"name": "sharedA", "javaType": "java.lang.String", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true},
        {"name": "sharedB", "javaType": "int", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}
      ]
    }
  ],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  }
}"#,
        )
        .unwrap();

        let packet = &manifest.remote_packets[0];
        assert_eq!(bidirectional_wire_flow(packet), RemoteFlow::ClientToServer);

        let metadata = typed_remote_packet_metadata(packet).unwrap();
        assert_eq!(metadata.flow, RemoteFlow::Bidirectional);
        assert_eq!(metadata.wire_params.len(), 2);
        assert_eq!(metadata.wire_params[0].name, "sharedA");
        assert_eq!(metadata.wire_params[1].name, "sharedB");
    }

    #[test]
    fn builds_full_remote_packet_registry() {
        let manifest = parse_remote_manifest(
            r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [
    {"id": 0, "class": "mindustry.net.Packets$StreamBegin"},
    {"id": 1, "class": "mindustry.net.Packets$StreamChunk"},
    {"id": 2, "class": "mindustry.net.Packets$WorldStream"},
    {"id": 3, "class": "mindustry.net.Packets$ConnectPacket"}
  ],
  "remotePackets": [
    {
      "remoteIndex": 0,
      "packetId": 4,
      "packetClass": "mindustry.gen.SetMessageBlockTextCallPacket",
      "declaringType": "mindustry.world.blocks.logic.MessageBlock",
      "method": "setMessageBlockText",
      "targets": "client",
      "called": "server",
      "variants": "all",
      "forward": false,
      "unreliable": false,
      "priority": "normal",
      "params": [
        {"name": "player", "javaType": "Player", "networkIncludedWhenCallerIsClient": false, "networkIncludedWhenCallerIsServer": false},
        {"name": "tile", "javaType": "mindustry.world.Tile", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": false},
        {"name": "text", "javaType": "java.lang.String", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": false}
      ]
    },
    {
      "remoteIndex": 1,
      "packetId": 5,
      "packetClass": "mindustry.gen.InfoPopupCallPacket",
      "declaringType": "mindustry.core.NetClient",
      "method": "infoPopup",
      "targets": "server",
      "called": "client",
      "variants": "one",
      "forward": false,
      "unreliable": true,
      "priority": "high",
      "params": [
        {"name": "message", "javaType": "java.lang.String", "networkIncludedWhenCallerIsClient": false, "networkIncludedWhenCallerIsServer": true},
        {"name": "duration", "javaType": "float", "networkIncludedWhenCallerIsClient": false, "networkIncludedWhenCallerIsServer": true}
      ]
    },
    {
      "remoteIndex": 2,
      "packetId": 6,
      "packetClass": "mindustry.gen.InfoPopupReliableCallPacket",
      "declaringType": "mindustry.core.NetClient",
      "method": "infoPopup",
      "targets": "both",
      "called": "both",
      "variants": "both",
      "forward": true,
      "unreliable": false,
      "priority": "low",
      "params": [
        {"name": "message", "javaType": "java.lang.String", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true},
        {"name": "id", "javaType": "int", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": false}
      ]
    }
  ],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  }
}"#,
        )
        .unwrap();

        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        assert_eq!(registry.packets().len(), 3);

        let by_id = registry.get_by_packet_id(4).unwrap();
        assert_eq!(by_id.flow, RemoteFlow::ClientToServer);
        assert_eq!(by_id.priority, RemotePriority::Normal);
        assert_eq!(by_id.params.len(), 3);
        assert_eq!(by_id.params[1].kind, RemoteParamKind::TileRef);
        assert_eq!(by_id.wire_params.len(), 2);
        assert_eq!(by_id.wire_params[0].name, "tile");
        assert_eq!(by_id.wire_params[1].kind, RemoteParamKind::Opaque);

        let overloads = registry.packets_for_method("infoPopup");
        assert_eq!(overloads.len(), 2);
        assert_eq!(overloads[0].flow, RemoteFlow::ServerToClient);
        assert_eq!(overloads[0].priority, RemotePriority::High);
        assert_eq!(overloads[1].flow, RemoteFlow::Bidirectional);
        assert_eq!(overloads[1].wire_params.len(), 2);
        assert_eq!(overloads[1].wire_params[1].kind, RemoteParamKind::Int);

        let selected = registry
            .first_matching(
                RemotePacketSelector::method("infoPopup")
                    .with_flow(RemoteFlow::ServerToClient)
                    .with_unreliable(true)
                    .with_param_java_types(&["java.lang.String", "float"]),
            )
            .unwrap();
        assert_eq!(selected.packet_id, 5);
        assert_eq!(selected.packet_class, "mindustry.gen.InfoPopupCallPacket");
        assert!(selected.matches_selector(
            &RemotePacketSelector::method("infoPopup")
                .with_flow(RemoteFlow::ServerToClient)
                .with_unreliable(true)
                .with_wire_param_kinds(&[RemoteParamKind::Opaque, RemoteParamKind::Float]),
        ));

        let reliable_bidirectional = registry.packets_matching(
            RemotePacketSelector::method("infoPopup")
                .with_flow(RemoteFlow::Bidirectional)
                .with_unreliable(false)
                .with_param_java_types(&["java.lang.String", "int"]),
        );
        assert_eq!(reliable_bidirectional.len(), 1);
        assert_eq!(reliable_bidirectional[0].packet_id, 6);

        let typed_packets = typed_remote_packets(&manifest).unwrap();
        assert_eq!(typed_packets.len(), 3);
        assert_eq!(
            typed_packets[2].packet_class,
            "mindustry.gen.InfoPopupReliableCallPacket"
        );
    }

    #[test]
    fn typed_inbound_remote_family_lookup_rejects_method_only_decoys() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.ServerPacketReliableDecoyCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketReliable",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("tile", "mindustry.world.Tile", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.ServerPacketReliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketReliable",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        };

        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        let packet = registry
            .first_inbound_remote_family(InboundRemoteFamily::ServerPacketReliable)
            .unwrap();

        assert_eq!(packet.packet_id, 5);
        assert_eq!(
            packet.packet_class,
            "mindustry.gen.ServerPacketReliableCallPacket"
        );
    }

    #[test]
    fn typed_custom_channel_remote_family_lookup_rejects_method_only_decoys() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.ClientPacketReliableDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketReliable",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.ClientPacketReliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketReliable",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        };

        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        let packet = registry
            .first_custom_channel_remote_family(CustomChannelRemoteFamily::ClientPacketReliable)
            .unwrap();

        assert_eq!(packet.packet_id, 5);
        assert_eq!(
            packet.packet_class,
            "mindustry.gen.ClientPacketReliableCallPacket"
        );
        assert_eq!(packet.flow, RemoteFlow::ServerToClient);
        assert_eq!(
            CustomChannelRemoteFamily::ClientPacketReliable.payload_kind(),
            CustomChannelRemotePayloadKind::Text
        );
        assert_eq!(CustomChannelRemotePayloadKind::Text.label(), "text");
        assert_eq!(
            CustomChannelRemoteFamily::ServerBinaryPacketReliable.payload_kind(),
            CustomChannelRemotePayloadKind::Binary
        );
        assert_eq!(CustomChannelRemotePayloadKind::Binary.label(), "binary");
        assert_eq!(
            CustomChannelRemoteFamily::ClientLogicDataReliable.payload_kind(),
            CustomChannelRemotePayloadKind::LogicData
        );
        assert_eq!(CustomChannelRemotePayloadKind::LogicData.label(), "logic");
        assert_eq!(
            InboundRemoteFamily::ClientLogicDataUnreliable.payload_kind(),
            CustomChannelRemotePayloadKind::LogicData
        );
        assert_eq!(
            CustomChannelRemoteFamily::ServerPacketReliable.inbound_remote_family(),
            Some(InboundRemoteFamily::ServerPacketReliable)
        );
        assert_eq!(
            CustomChannelRemoteFamily::ClientLogicDataReliable.inbound_remote_family(),
            Some(InboundRemoteFamily::ClientLogicDataReliable)
        );
        assert_eq!(
            CustomChannelRemoteFamily::ClientPacketReliable.inbound_remote_family(),
            None
        );
        assert_eq!(
            InboundRemoteFamily::ServerBinaryPacketUnreliable.custom_channel_family(),
            CustomChannelRemoteFamily::ServerBinaryPacketUnreliable
        );
    }

    #[test]
    fn typed_high_frequency_lookup_rejects_flow_only_decoys() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.StateSnapshotDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "stateSnapshot",
                    "client",
                    "low",
                    true,
                    vec![test_param("coreData", "byte[]", true, true)],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.StateSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "stateSnapshot",
                    "server",
                    "low",
                    true,
                    vec![test_param("coreData", "byte[]", true, true)],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        };

        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        let packet = registry
            .first_high_frequency_method(HighFrequencyRemoteMethod::StateSnapshot)
            .unwrap();

        assert_eq!(packet.packet_id, 5);
        assert_eq!(packet.flow, RemoteFlow::ServerToClient);
        assert_eq!(packet.packet_class, "mindustry.gen.StateSnapshotCallPacket");
    }

    #[test]
    fn custom_channel_remote_registry_uses_typed_family_signatures() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();

        let registry = CustomChannelRemoteRegistry::from_manifest(&manifest).unwrap();

        assert_eq!(
            registry.packet_id(CustomChannelRemoteFamily::ClientPacketReliable),
            Some(22)
        );
        assert_eq!(
            registry.dispatch_spec(22),
            Some(CustomChannelRemoteDispatchSpec {
                family: CustomChannelRemoteFamily::ClientPacketReliable,
                payload_kind: CustomChannelRemotePayloadKind::Text,
            })
        );
        assert_eq!(
            registry.classify(20),
            Some(CustomChannelRemoteFamily::ClientLogicDataReliable)
        );
        assert_eq!(registry.classify(24), None);
    }

    #[test]
    fn high_frequency_remote_registry_uses_typed_method_signatures() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();

        let registry = HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();
        let state_packet_id = registry
            .packet_id(HighFrequencyRemoteMethod::StateSnapshot)
            .unwrap();
        let entity_packet_id = registry
            .packet_id(HighFrequencyRemoteMethod::EntitySnapshot)
            .unwrap();
        let block_packet_id = registry
            .packet_id(HighFrequencyRemoteMethod::BlockSnapshot)
            .unwrap();

        assert_eq!(state_packet_id, 125);
        assert_eq!(entity_packet_id, 46);
        assert_eq!(
            registry.classify(block_packet_id),
            Some(HighFrequencyRemoteMethod::BlockSnapshot)
        );
        assert_eq!(registry.classify(250), None);
    }

    #[test]
    fn inbound_remote_registry_reuses_custom_channel_registry_packet_ids() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();

        let registry = InboundRemoteRegistry::from_manifest(&manifest).unwrap();

        assert_eq!(
            registry.packet_id(InboundRemoteFamily::ServerPacketReliable),
            Some(94)
        );
        assert_eq!(registry.classify(24), None);
        assert_eq!(
            registry.dispatch_spec(21),
            Some(InboundRemoteDispatchSpec {
                family: InboundRemoteFamily::ClientLogicDataUnreliable,
                payload_kind: CustomChannelRemotePayloadKind::LogicData,
            })
        );
    }

    #[test]
    fn typed_remote_registries_bundle_reuses_typed_dispatch_entries() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let bundle = TypedRemoteRegistries::from_manifest(&manifest).unwrap();
        let high_frequency_registry =
            HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();
        let custom_registry = CustomChannelRemoteRegistry::from_manifest(&manifest).unwrap();
        let inbound_registry = InboundRemoteRegistry::from_manifest(&manifest).unwrap();
        let well_known_registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();

        assert_eq!(
            bundle.high_frequency.resolved_packet_ids(),
            high_frequency_registry.resolved_packet_ids()
        );
        assert_eq!(
            bundle.custom_channel.resolved_dispatch_specs(),
            custom_registry.resolved_dispatch_specs()
        );
        assert_eq!(
            bundle.inbound_remote.resolved_dispatch_specs(),
            inbound_registry.resolved_dispatch_specs()
        );
        assert_eq!(
            bundle.well_known.resolved_packet_ids(),
            well_known_registry.resolved_packet_ids()
        );
        assert_eq!(
            bundle
                .custom_channel
                .packet_id(CustomChannelRemoteFamily::ServerBinaryPacketReliable),
            Some(92)
        );
        assert_eq!(
            bundle
                .inbound_remote
                .packet_id(InboundRemoteFamily::ClientLogicDataUnreliable),
            Some(21)
        );
        assert_eq!(
            bundle
                .high_frequency
                .packet_id(HighFrequencyRemoteMethod::ClientSnapshot),
            Some(26)
        );
        assert_eq!(
            bundle.well_known.packet_id(WellKnownRemoteMethod::Ping),
            well_known_registry.packet_id(WellKnownRemoteMethod::Ping)
        );
    }

    #[test]
    fn typed_remote_registries_bundle_can_reuse_single_parsed_remote_registry() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let remote_registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        let bundle = TypedRemoteRegistries::from_remote_registry(&remote_registry).unwrap();
        let from_manifest = TypedRemoteRegistries::from_manifest(&manifest).unwrap();

        assert_eq!(
            bundle.high_frequency.resolved_packet_ids(),
            from_manifest.high_frequency.resolved_packet_ids()
        );
        assert_eq!(
            bundle.custom_channel.resolved_dispatch_specs(),
            from_manifest.custom_channel.resolved_dispatch_specs()
        );
        assert_eq!(
            bundle.inbound_remote.resolved_dispatch_specs(),
            from_manifest.inbound_remote.resolved_dispatch_specs()
        );
        assert_eq!(
            bundle.well_known.resolved_packet_ids(),
            from_manifest.well_known.resolved_packet_ids()
        );
    }

    #[test]
    fn typed_remote_registries_bundle_preserves_well_known_remote_decoy_lookup_tables() {
        let baseline_manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut manifest = baseline_manifest.clone();
        let decoy_packets = well_known_manifest_with_decoys()
            .remote_packets
            .into_iter()
            .filter(|packet| packet.packet_class.contains("Decoy"))
            .collect::<Vec<_>>();
        manifest.remote_packets.splice(0..0, decoy_packets);
        let baseline = TypedRemoteRegistries::from_manifest(&baseline_manifest).unwrap();
        let bundle = TypedRemoteRegistries::from_manifest(&manifest).unwrap();

        assert_eq!(
            bundle.well_known.resolved_packet_ids(),
            baseline.well_known.resolved_packet_ids()
        );
        assert_eq!(
            bundle.well_known.packet_id_fixed_table(),
            baseline.well_known.packet_id_fixed_table()
        );

        assert_eq!(
            bundle.well_known.packet_id(WellKnownRemoteMethod::Ping),
            baseline.well_known.packet_id(WellKnownRemoteMethod::Ping)
        );
        assert_eq!(
            bundle
                .well_known
                .packet_id(WellKnownRemoteMethod::ClientPlanSnapshot),
            baseline
                .well_known
                .packet_id(WellKnownRemoteMethod::ClientPlanSnapshot)
        );
        assert_eq!(
            bundle
                .well_known
                .packet_id(WellKnownRemoteMethod::PingResponse),
            baseline
                .well_known
                .packet_id(WellKnownRemoteMethod::PingResponse)
        );
        assert_eq!(
            bundle
                .well_known
                .packet_id(WellKnownRemoteMethod::DebugStatusClientUnreliable),
            baseline
                .well_known
                .packet_id(WellKnownRemoteMethod::DebugStatusClientUnreliable)
        );
        assert_eq!(
            bundle.well_known.packet_id(WellKnownRemoteMethod::SetRule),
            baseline
                .well_known
                .packet_id(WellKnownRemoteMethod::SetRule)
        );

        let fixed_table = bundle.well_known.packet_id_fixed_table();
        let baseline_fixed_table = baseline.well_known.packet_id_fixed_table();
        assert_eq!(fixed_table.get(5), baseline_fixed_table.get(5));
        assert_eq!(fixed_table.get(19), baseline_fixed_table.get(19));
        assert_eq!(fixed_table.get(4), baseline_fixed_table.get(4));
        assert_eq!(fixed_table.get(18), baseline_fixed_table.get(18));
    }

    #[test]
    fn well_known_remote_registry_exposes_expected_lookup_tables() {
        let manifest = well_known_manifest_with_decoys();
        let registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();

        let expected = [
            (WellKnownRemoteMethod::Ping, Some(5)),
            (WellKnownRemoteMethod::ClientPlanSnapshot, Some(7)),
            (WellKnownRemoteMethod::ClientPlanSnapshotReceived, Some(8)),
            (WellKnownRemoteMethod::PingResponse, Some(10)),
            (WellKnownRemoteMethod::PingLocation, Some(11)),
            (WellKnownRemoteMethod::DebugStatusClientUnreliable, Some(13)),
            (WellKnownRemoteMethod::TraceInfo, Some(15)),
            (WellKnownRemoteMethod::ConnectRedirect, Some(35)),
            (WellKnownRemoteMethod::ConnectConfirm, Some(21)),
            (WellKnownRemoteMethod::PlayerSpawn, Some(37)),
            (WellKnownRemoteMethod::SetRules, Some(16)),
            (WellKnownRemoteMethod::SetObjectives, Some(17)),
            (WellKnownRemoteMethod::SetRule, Some(19)),
            (WellKnownRemoteMethod::WorldDataBegin, Some(23)),
            (WellKnownRemoteMethod::KickString, Some(25)),
            (WellKnownRemoteMethod::KickReason, Some(27)),
            (WellKnownRemoteMethod::SendChatMessage, Some(29)),
            (WellKnownRemoteMethod::SendMessage, Some(31)),
            (WellKnownRemoteMethod::SendMessageWithSender, Some(33)),
        ];

        assert_eq!(registry.len(), expected.len());
        assert_eq!(registry.resolved_packet_ids(), expected);

        for (method, packet_id) in expected {
            let packet_id = packet_id.expect("decoy fixture should resolve every method");
            assert_eq!(registry.packet_id(method), Some(packet_id));
            assert_eq!(registry.classify(packet_id), Some(method));
            assert!(registry.contains_packet_id(packet_id));
        }

        assert_eq!(registry.classify(4), None);
        assert_eq!(registry.classify(18), None);
        assert!(!registry.contains_packet_id(4));
        assert!(!registry.contains_packet_id(250));
    }

    #[test]
    fn well_known_remote_registry_exposes_packet_id_fixed_table() {
        let manifest = well_known_manifest_with_decoys();
        let registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();
        let fixed_table = registry.packet_id_fixed_table();

        assert_eq!(fixed_table.get(5), Some(WellKnownRemoteMethod::Ping));
        assert_eq!(
            fixed_table.get(17),
            Some(WellKnownRemoteMethod::SetObjectives)
        );
        assert_eq!(fixed_table.get(35), Some(WellKnownRemoteMethod::ConnectRedirect));
        assert_eq!(fixed_table.get(21), Some(WellKnownRemoteMethod::ConnectConfirm));
        assert_eq!(fixed_table.get(37), Some(WellKnownRemoteMethod::PlayerSpawn));
        assert_eq!(fixed_table.get(23), Some(WellKnownRemoteMethod::WorldDataBegin));
        assert_eq!(fixed_table.get(25), Some(WellKnownRemoteMethod::KickString));
        assert_eq!(fixed_table.get(27), Some(WellKnownRemoteMethod::KickReason));
        assert_eq!(fixed_table.get(29), Some(WellKnownRemoteMethod::SendChatMessage));
        assert_eq!(fixed_table.get(31), Some(WellKnownRemoteMethod::SendMessage));
        assert_eq!(
            fixed_table.get(33),
            Some(WellKnownRemoteMethod::SendMessageWithSender)
        );
        assert_eq!(fixed_table.get(18), None);
        assert!(fixed_table.contains_packet_id(19));
        assert!(fixed_table.contains_packet_id(23));
        assert!(fixed_table.contains_packet_id(27));
        assert!(fixed_table.contains_packet_id(37));
        assert!(fixed_table.contains_packet_id(33));
        assert!(!fixed_table.contains_packet_id(250));
    }

    #[test]
    fn custom_channel_remote_registry_exposes_packet_id_fixed_table() {
        let manifest = custom_channel_manifest_with_decoys();
        let registry = CustomChannelRemoteRegistry::from_manifest(&manifest).unwrap();
        let fixed_table = registry.packet_id_fixed_table();

        assert_eq!(
            fixed_table.get(5),
            Some(CustomChannelRemoteDispatchSpec {
                family: CustomChannelRemoteFamily::ClientPacketReliable,
                payload_kind: CustomChannelRemotePayloadKind::Text,
            })
        );
        assert_eq!(fixed_table.get(4), None);
        assert!(fixed_table.contains_packet_id(14));
        assert!(!fixed_table.contains_packet_id(250));
    }

    #[test]
    fn inbound_remote_registry_exposes_packet_id_fixed_table() {
        let manifest = custom_channel_manifest_with_decoys();
        let registry = InboundRemoteRegistry::from_manifest(&manifest).unwrap();
        let fixed_table = registry.packet_id_fixed_table();

        assert_eq!(
            fixed_table.get(10),
            Some(InboundRemoteDispatchSpec {
                family: InboundRemoteFamily::ServerPacketReliable,
                payload_kind: CustomChannelRemotePayloadKind::Text,
            })
        );
        assert_eq!(fixed_table.get(9), None);
        assert!(fixed_table.contains_packet_id(15));
        assert!(!fixed_table.contains_packet_id(250));
    }

    #[test]
    fn remote_packet_id_fixed_table_rejects_duplicate_packet_ids() {
        let panic = std::panic::catch_unwind(|| {
            RemotePacketIdFixedTable::from_entries(&[
                (5, WellKnownRemoteMethod::Ping),
                (5, WellKnownRemoteMethod::SetRules),
            ])
        })
        .unwrap_err();
        let message = if let Some(message) = panic.downcast_ref::<&'static str>() {
            (*message).to_string()
        } else if let Some(message) = panic.downcast_ref::<String>() {
            message.clone()
        } else {
            String::new()
        };

        assert!(message.contains("duplicate packet id in fixed table: 5"));
    }

    #[test]
    fn extracts_custom_channel_remote_subset_with_decoy_rejection() {
        let manifest = custom_channel_manifest_with_decoys();

        let packets = custom_channel_remote_packets(&manifest).unwrap();

        assert_eq!(packets.len(), CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);
        assert_eq!(
            packets[0].family,
            CustomChannelRemoteFamily::ClientPacketReliable
        );
        assert_eq!(packets[0].packet_id, 5);
        assert_eq!(
            packets[0].payload_kind,
            CustomChannelRemotePayloadKind::Text
        );
        assert_eq!(
            packets[4].family,
            CustomChannelRemoteFamily::ServerPacketReliable
        );
        assert_eq!(packets[4].packet_id, 10);
        assert_eq!(
            packets[4].wire_params,
            vec![
                TypedRemoteParamSpec {
                    name: "type",
                    java_type: "java.lang.String",
                    kind: RemoteParamKind::Opaque,
                },
                TypedRemoteParamSpec {
                    name: "contents",
                    java_type: "java.lang.String",
                    kind: RemoteParamKind::Opaque,
                },
            ]
        );
    }

    #[test]
    fn extracts_inbound_remote_subset_with_typed_payload_kinds() {
        let manifest = custom_channel_manifest_with_decoys();

        let packets = inbound_remote_packets(&manifest).unwrap();

        assert_eq!(packets.len(), INBOUND_REMOTE_FAMILY_COUNT);
        assert_eq!(packets[0].family, InboundRemoteFamily::ServerPacketReliable);
        assert_eq!(packets[0].packet_id, 10);
        assert_eq!(
            packets[0].payload_kind,
            CustomChannelRemotePayloadKind::Text
        );
        assert_eq!(
            packets[4].family,
            InboundRemoteFamily::ClientLogicDataReliable
        );
        assert_eq!(packets[4].packet_id, 14);
        assert_eq!(
            packets[4].payload_kind,
            CustomChannelRemotePayloadKind::LogicData
        );
    }

    #[test]
    fn generates_inbound_dispatch_rust_module() {
        let manifest = custom_channel_manifest_with_decoys();

        let generated = generate_inbound_dispatch_rust_module(&manifest).unwrap();

        assert!(generated
            .contains("pub const CUSTOM_CHANNEL_CLIENT_PACKET_RELIABLE_PACKET_ID: u8 = 5;"));
        assert!(generated.contains("pub const INBOUND_SERVER_PACKET_RELIABLE_PACKET_ID: u8 = 10;"));
        assert!(generated
            .contains("pub const INBOUND_REMOTE_PACKET_SPECS: &[InboundRemotePacketSpec] = &["));
        assert!(generated.contains(
            "pub const fn inbound_remote_dispatch_spec(packet_id: u8) -> Option<InboundRemoteDispatchSpec> {"
        ));
        assert!(generated.contains("payload_kind: CustomChannelRemotePayloadKind::LogicData"));
        assert!(!generated.contains("ServerPacketReliableDecoyCallPacket"));
    }

    #[test]
    fn resolves_typed_inbound_remote_dispatch_specs_with_decoy_rejection() {
        let manifest = custom_channel_manifest_with_decoys();

        let specs = typed_inbound_remote_dispatch_specs(&manifest).unwrap();

        assert_eq!(specs.len(), INBOUND_REMOTE_FAMILY_COUNT);
        assert_eq!(
            specs[0],
            (
                10,
                InboundRemoteDispatchSpec {
                    family: InboundRemoteFamily::ServerPacketReliable,
                    payload_kind: CustomChannelRemotePayloadKind::Text,
                },
            )
        );
        assert_eq!(
            specs[4],
            (
                14,
                InboundRemoteDispatchSpec {
                    family: InboundRemoteFamily::ClientLogicDataReliable,
                    payload_kind: CustomChannelRemotePayloadKind::LogicData,
                },
            )
        );
    }

    #[test]
    fn resolves_typed_custom_channel_remote_dispatch_specs_with_decoy_rejection() {
        let manifest = custom_channel_manifest_with_decoys();

        let specs = typed_custom_channel_remote_dispatch_specs(&manifest).unwrap();

        assert_eq!(specs.len(), CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);
        assert_eq!(
            specs[0],
            (
                5,
                CustomChannelRemoteDispatchSpec {
                    family: CustomChannelRemoteFamily::ClientPacketReliable,
                    payload_kind: CustomChannelRemotePayloadKind::Text,
                },
            )
        );
        assert_eq!(
            specs[8],
            (
                14,
                CustomChannelRemoteDispatchSpec {
                    family: CustomChannelRemoteFamily::ClientLogicDataReliable,
                    payload_kind: CustomChannelRemotePayloadKind::LogicData,
                },
            )
        );
    }

    #[test]
    fn extracts_high_frequency_remote_subset_from_manifest() {
        let manifest = RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.ClientSnapshotCallPacket",
                    "mindustry.core.NetServer",
                    "clientSnapshot",
                    "client",
                    "high",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("snapshotID", "int", true, true),
                        test_param(
                            "plans",
                            "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
                            true,
                            true,
                        ),
                    ],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.StateSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "stateSnapshot",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("waveTime", "float", true, true),
                        test_param("coreData", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    2,
                    6,
                    "mindustry.gen.EntitySnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "entitySnapshot",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("amount", "short", true, true),
                        test_param("data", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    3,
                    7,
                    "mindustry.gen.BlockSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "blockSnapshot",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("amount", "short", true, true),
                        test_param("data", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    4,
                    8,
                    "mindustry.gen.HiddenSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "hiddenSnapshot",
                    "server",
                    "low",
                    true,
                    vec![test_param("ids", "arc.struct.IntSeq", true, true)],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        };

        let packets = high_frequency_remote_packets(&manifest).unwrap();
        assert_eq!(packets.len(), 5);
        assert_eq!(packets[0].method, HighFrequencyRemoteMethod::ClientSnapshot);
        assert_eq!(packets[0].flow, RemoteFlow::ClientToServer);
        assert_eq!(packets[0].wire_params.len(), 2);
        assert_eq!(packets[0].wire_params[0].name, "snapshotID");
        assert_eq!(
            packets[0].wire_params[1].kind,
            RemoteParamKind::BuildPlanQueue
        );
        assert_eq!(packets[4].wire_params[0].kind, RemoteParamKind::IntSeq);

        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();
        let block_snapshot = registry
            .first_matching(
                RemotePacketSelector::high_frequency(HighFrequencyRemoteMethod::BlockSnapshot)
                    .with_flow(RemoteFlow::ServerToClient)
                    .with_unreliable(true)
                    .with_wire_param_kinds(&[RemoteParamKind::Short, RemoteParamKind::Bytes]),
            )
            .unwrap();
        assert_eq!(block_snapshot.packet_id, 7);
    }

    #[test]
    fn high_frequency_remote_packets_reject_missing_expected_family() {
        let mut manifest = high_frequency_manifest_with_decoys();
        manifest
            .remote_packets
            .retain(|packet| packet.method != "hiddenSnapshot");

        let error = high_frequency_remote_packets(&manifest).unwrap_err();
        match error {
            RemoteManifestError::MissingHighFrequencyPacket(method) => {
                assert_eq!(method, "hiddenSnapshot");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn well_known_remote_lookup_rejects_method_name_decoys() {
        let manifest = well_known_manifest_with_decoys();
        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();

        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::Ping)
                .map(|packet| packet.packet_id),
            Some(5)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::ClientPlanSnapshot)
                .map(|packet| packet.packet_id),
            Some(7)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::PingResponse)
                .map(|packet| packet.packet_id),
            Some(10)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::DebugStatusClientUnreliable)
                .map(|packet| packet.packet_id),
            Some(13)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::SetRule)
                .map(|packet| packet.packet_id),
            Some(19)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::ConnectRedirect)
                .map(|packet| packet.packet_id),
            Some(35)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::ConnectConfirm)
                .map(|packet| packet.packet_id),
            Some(21)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::PlayerSpawn)
                .map(|packet| packet.packet_id),
            Some(37)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::WorldDataBegin)
                .map(|packet| packet.packet_id),
            Some(23)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::KickString)
                .map(|packet| packet.packet_id),
            Some(25)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::KickReason)
                .map(|packet| packet.packet_id),
            Some(27)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::SendChatMessage)
                .map(|packet| packet.packet_id),
            Some(29)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::SendMessage)
                .map(|packet| packet.packet_id),
            Some(31)
        );
        assert_eq!(
            registry
                .first_well_known_method(WellKnownRemoteMethod::SendMessageWithSender)
                .map(|packet| packet.packet_id),
            Some(33)
        );
    }

    #[test]
    fn custom_channel_remote_packets_reject_missing_expected_family() {
        let mut manifest = custom_channel_manifest_with_decoys();
        manifest
            .remote_packets
            .retain(|packet| packet.method != "clientLogicDataUnreliable");

        let error = custom_channel_remote_packets(&manifest).unwrap_err();
        match error {
            RemoteManifestError::InvalidRemotePacketMetadata(message) => assert_eq!(
                message,
                "missing custom-channel remote family packet in manifest: clientLogicDataUnreliable"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn inbound_remote_packets_reject_missing_expected_family() {
        let mut manifest = custom_channel_manifest_with_decoys();
        manifest
            .remote_packets
            .retain(|packet| packet.method != "clientLogicDataUnreliable");

        let error = inbound_remote_packets(&manifest).unwrap_err();
        match error {
            RemoteManifestError::InvalidRemotePacketMetadata(message) => assert_eq!(
                message,
                "missing inbound remote family packet in manifest: clientLogicDataUnreliable"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn well_known_remote_lookup_matches_real_manifest_signatures() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = RemotePacketRegistry::from_manifest(&manifest).unwrap();

        for method in WellKnownRemoteMethod::ordered() {
            assert!(
                registry.first_well_known_method(method).is_some(),
                "missing well-known packet lookup for {}",
                method.method_name()
            );
        }
    }

    #[test]
    fn generates_high_frequency_rust_module() {
        let manifest = parse_remote_manifest(
            r#"{
  "schema": "mdt.remote.manifest.v1",
  "generator": {
    "source": "mindustry.annotations.remote",
    "callClass": "mindustry.gen.Call"
  },
  "basePackets": [
    {"id": 0, "class": "mindustry.net.Packets$StreamBegin"},
    {"id": 1, "class": "mindustry.net.Packets$StreamChunk"},
    {"id": 2, "class": "mindustry.net.Packets$WorldStream"},
    {"id": 3, "class": "mindustry.net.Packets$ConnectPacket"}
  ],
  "remotePackets": [
    {"remoteIndex": 0, "packetId": 4, "packetClass": "mindustry.gen.ClientSnapshotCallPacket", "declaringType": "mindustry.core.NetServer", "method": "clientSnapshot", "targets": "client", "called": "none", "variants": "all", "forward": false, "unreliable": true, "priority": "high", "params": [{"name": "player", "javaType": "Player", "networkIncludedWhenCallerIsClient": false, "networkIncludedWhenCallerIsServer": false}, {"name": "snapshotID", "javaType": "int", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}]},
    {"remoteIndex": 1, "packetId": 5, "packetClass": "mindustry.gen.StateSnapshotCallPacket", "declaringType": "mindustry.core.NetClient", "method": "stateSnapshot", "targets": "server", "called": "none", "variants": "one", "forward": false, "unreliable": true, "priority": "low", "params": [{"name": "coreData", "javaType": "byte[]", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}]},
    {"remoteIndex": 2, "packetId": 6, "packetClass": "mindustry.gen.EntitySnapshotCallPacket", "declaringType": "mindustry.core.NetClient", "method": "entitySnapshot", "targets": "server", "called": "none", "variants": "one", "forward": false, "unreliable": true, "priority": "low", "params": [{"name": "amount", "javaType": "short", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}]},
    {"remoteIndex": 3, "packetId": 7, "packetClass": "mindustry.gen.BlockSnapshotCallPacket", "declaringType": "mindustry.core.NetClient", "method": "blockSnapshot", "targets": "server", "called": "none", "variants": "both", "forward": false, "unreliable": true, "priority": "low", "params": [{"name": "data", "javaType": "byte[]", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}]},
    {"remoteIndex": 4, "packetId": 8, "packetClass": "mindustry.gen.HiddenSnapshotCallPacket", "declaringType": "mindustry.core.NetClient", "method": "hiddenSnapshot", "targets": "server", "called": "none", "variants": "one", "forward": false, "unreliable": true, "priority": "low", "params": [{"name": "ids", "javaType": "arc.struct.IntSeq", "networkIncludedWhenCallerIsClient": true, "networkIncludedWhenCallerIsServer": true}]}
  ],
  "wire": {
    "packetIdByte": "u8",
    "lengthField": "u16be",
    "compressionFlag": {"0": "none", "1": "lz4"},
    "compressionThreshold": 36
  }
}"#,
        )
        .unwrap();

        let generated = generate_high_frequency_rust_module(&manifest).unwrap();
        assert!(generated.contains("pub const CLIENT_SNAPSHOT_PACKET_ID: u8 = 4;"));
        assert!(generated.contains(
            "pub const HIGH_FREQUENCY_REMOTE_PACKET_SPECS: &[HighFrequencyRemotePacketSpec] = &["
        ));
        assert!(generated.contains("kind: RemoteParamKind::IntSeq"));
        assert!(generated.contains("name: \"snapshotID\""));
        assert!(!generated.contains("name: \"player\""));
        assert!(!generated.contains("wire_included"));
    }

    fn high_frequency_manifest_with_decoys() -> RemoteManifest {
        RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.ClientSnapshotCallPacket",
                    "mindustry.core.NetServer",
                    "clientSnapshot",
                    "client",
                    "high",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("snapshotID", "int", true, true),
                        test_param(
                            "plans",
                            "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
                            true,
                            true,
                        ),
                    ],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.StateSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "stateSnapshot",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("waveTime", "float", true, true),
                        test_param("coreData", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    2,
                    6,
                    "mindustry.gen.EntitySnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "entitySnapshot",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("amount", "short", true, true),
                        test_param("data", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    3,
                    7,
                    "mindustry.gen.BlockSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "blockSnapshot",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("amount", "short", true, true),
                        test_param("data", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    4,
                    8,
                    "mindustry.gen.HiddenSnapshotCallPacket",
                    "mindustry.core.NetClient",
                    "hiddenSnapshot",
                    "server",
                    "low",
                    true,
                    vec![test_param("ids", "arc.struct.IntSeq", true, true)],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        }
    }

    fn custom_channel_manifest_with_decoys() -> RemoteManifest {
        RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.ClientPacketReliableDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketReliable",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.ClientPacketReliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketReliable",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    2,
                    6,
                    "mindustry.gen.ClientPacketUnreliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketUnreliable",
                    "server",
                    "normal",
                    true,
                    vec![
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    3,
                    7,
                    "mindustry.gen.ClientBinaryPacketReliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientBinaryPacketReliable",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    4,
                    8,
                    "mindustry.gen.ClientBinaryPacketUnreliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientBinaryPacketUnreliable",
                    "server",
                    "normal",
                    true,
                    vec![
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    5,
                    9,
                    "mindustry.gen.ServerPacketReliableDecoyCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketReliable",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("tile", "mindustry.world.Tile", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    6,
                    10,
                    "mindustry.gen.ServerPacketReliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketReliable",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    7,
                    11,
                    "mindustry.gen.ServerPacketUnreliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketUnreliable",
                    "client",
                    "normal",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    8,
                    12,
                    "mindustry.gen.ServerBinaryPacketReliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverBinaryPacketReliable",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    9,
                    13,
                    "mindustry.gen.ServerBinaryPacketUnreliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverBinaryPacketUnreliable",
                    "client",
                    "normal",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("type", "java.lang.String", true, true),
                        test_param("contents", "byte[]", true, true),
                    ],
                ),
                test_remote_packet(
                    10,
                    14,
                    "mindustry.gen.ClientLogicDataReliableCallPacket",
                    "mindustry.core.NetServer",
                    "clientLogicDataReliable",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("channel", "java.lang.String", true, true),
                        test_param("value", "java.lang.Object", true, true),
                    ],
                ),
                test_remote_packet(
                    11,
                    15,
                    "mindustry.gen.ClientLogicDataUnreliableCallPacket",
                    "mindustry.core.NetServer",
                    "clientLogicDataUnreliable",
                    "client",
                    "normal",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("channel", "java.lang.String", true, true),
                        test_param("value", "java.lang.Object", true, true),
                    ],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        }
    }

    fn well_known_manifest_with_decoys() -> RemoteManifest {
        RemoteManifest {
            schema: REMOTE_MANIFEST_SCHEMA_V1.to_string(),
            generator: RemoteGeneratorInfo {
                source: "mindustry.annotations.remote".into(),
                call_class: "mindustry.gen.Call".into(),
            },
            base_packets: vec![
                BasePacketEntry {
                    id: 0,
                    class_name: "mindustry.net.Packets$StreamBegin".into(),
                },
                BasePacketEntry {
                    id: 1,
                    class_name: "mindustry.net.Packets$StreamChunk".into(),
                },
                BasePacketEntry {
                    id: 2,
                    class_name: "mindustry.net.Packets$WorldStream".into(),
                },
                BasePacketEntry {
                    id: 3,
                    class_name: "mindustry.net.Packets$ConnectPacket".into(),
                },
            ],
            remote_packets: vec![
                test_remote_packet(
                    0,
                    4,
                    "mindustry.gen.PingDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "ping",
                    "client",
                    "high",
                    true,
                    vec![test_param("time", "long", true, true)],
                ),
                test_remote_packet(
                    1,
                    5,
                    "mindustry.gen.PingCallPacket",
                    "mindustry.core.NetClient",
                    "ping",
                    "client",
                    "high",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("time", "long", true, true),
                    ],
                ),
                test_remote_packet(
                    2,
                    6,
                    "mindustry.gen.ClientPlanSnapshotDecoyCallPacket",
                    "mindustry.core.NetServer",
                    "clientPlanSnapshot",
                    "client",
                    "low",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("groupId", "int", true, true),
                    ],
                ),
                test_remote_packet(
                    3,
                    7,
                    "mindustry.gen.ClientPlanSnapshotCallPacket",
                    "mindustry.core.NetServer",
                    "clientPlanSnapshot",
                    "client",
                    "low",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("groupId", "int", true, true),
                        test_param(
                            "plans",
                            "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
                            true,
                            true,
                        ),
                    ],
                ),
                test_remote_packet(
                    4,
                    8,
                    "mindustry.gen.ClientPlanSnapshotReceivedCallPacket",
                    "mindustry.core.NetClient",
                    "clientPlanSnapshotReceived",
                    "server",
                    "low",
                    true,
                    vec![
                        test_param("player", "Player", true, true),
                        test_param("groupId", "int", true, true),
                        test_param(
                            "plans",
                            "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
                            true,
                            true,
                        ),
                    ],
                ),
                test_remote_packet(
                    5,
                    9,
                    "mindustry.gen.PingResponseDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "pingResponse",
                    "server",
                    "normal",
                    false,
                    vec![test_param("time", "int", true, true)],
                ),
                test_remote_packet(
                    6,
                    10,
                    "mindustry.gen.PingResponseCallPacket",
                    "mindustry.core.NetClient",
                    "pingResponse",
                    "server",
                    "normal",
                    false,
                    vec![test_param("time", "long", true, true)],
                ),
                test_remote_packet(
                    7,
                    11,
                    "mindustry.gen.PingLocationCallPacket",
                    "mindustry.core.NetClient",
                    "pingLocation",
                    "both",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, true),
                        test_param("x", "float", true, true),
                        test_param("y", "float", true, true),
                        test_param("text", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    8,
                    12,
                    "mindustry.gen.DebugStatusClientUnreliableDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "debugStatusClientUnreliable",
                    "server",
                    "high",
                    false,
                    vec![
                        test_param("value", "int", true, true),
                        test_param("lastClientSnapshot", "int", true, true),
                        test_param("snapshotsSent", "int", true, true),
                    ],
                ),
                test_remote_packet(
                    9,
                    13,
                    "mindustry.gen.DebugStatusClientUnreliableCallPacket",
                    "mindustry.core.NetClient",
                    "debugStatusClientUnreliable",
                    "server",
                    "high",
                    true,
                    vec![
                        test_param("value", "int", true, true),
                        test_param("lastClientSnapshot", "int", true, true),
                        test_param("snapshotsSent", "int", true, true),
                    ],
                ),
                test_remote_packet(
                    10,
                    14,
                    "mindustry.gen.TraceInfoDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "traceInfo",
                    "server",
                    "normal",
                    false,
                    vec![test_param(
                        "info",
                        "mindustry.net.Administration.TraceInfo",
                        true,
                        true,
                    )],
                ),
                test_remote_packet(
                    11,
                    15,
                    "mindustry.gen.TraceInfoCallPacket",
                    "mindustry.core.NetClient",
                    "traceInfo",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", true, true),
                        test_param("info", "mindustry.net.Administration.TraceInfo", true, true),
                    ],
                ),
                test_remote_packet(
                    12,
                    16,
                    "mindustry.gen.SetRulesCallPacket",
                    "mindustry.core.NetClient",
                    "setRules",
                    "server",
                    "normal",
                    false,
                    vec![test_param("rules", "mindustry.game.Rules", true, true)],
                ),
                test_remote_packet(
                    13,
                    17,
                    "mindustry.gen.SetObjectivesCallPacket",
                    "mindustry.core.NetClient",
                    "setObjectives",
                    "server",
                    "normal",
                    false,
                    vec![test_param(
                        "executor",
                        "mindustry.game.MapObjectives",
                        true,
                        true,
                    )],
                ),
                test_remote_packet(
                    14,
                    18,
                    "mindustry.gen.SetRuleDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "setRule",
                    "server",
                    "normal",
                    false,
                    vec![test_param("rule", "java.lang.String", true, true)],
                ),
                test_remote_packet(
                    15,
                    19,
                    "mindustry.gen.SetRuleCallPacket",
                    "mindustry.core.NetClient",
                    "setRule",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("rule", "java.lang.String", true, true),
                        test_param("jsonData", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    16,
                    20,
                    "mindustry.gen.ConnectConfirmDecoyCallPacket",
                    "mindustry.core.NetServer",
                    "connectConfirm",
                    "client",
                    "high",
                    false,
                    vec![],
                ),
                test_remote_packet(
                    17,
                    21,
                    "mindustry.gen.ConnectConfirmCallPacket",
                    "mindustry.core.NetServer",
                    "connectConfirm",
                    "client",
                    "high",
                    false,
                    vec![test_param("player", "Player", false, false)],
                ),
                test_remote_packet(
                    18,
                    22,
                    "mindustry.gen.WorldDataBeginDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "worldDataBegin",
                    "server",
                    "normal",
                    true,
                    vec![],
                ),
                test_remote_packet(
                    19,
                    23,
                    "mindustry.gen.WorldDataBeginCallPacket",
                    "mindustry.core.NetClient",
                    "worldDataBegin",
                    "server",
                    "normal",
                    false,
                    vec![],
                ),
                test_remote_packet(
                    20,
                    24,
                    "mindustry.gen.KickDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "kick",
                    "server",
                    "high",
                    true,
                    vec![test_param("reason", "java.lang.String", true, true)],
                ),
                test_remote_packet(
                    21,
                    25,
                    "mindustry.gen.KickCallPacket",
                    "mindustry.core.NetClient",
                    "kick",
                    "server",
                    "high",
                    false,
                    vec![test_param("reason", "java.lang.String", true, true)],
                ),
                test_remote_packet(
                    22,
                    26,
                    "mindustry.gen.KickDecoyCallPacket2",
                    "mindustry.core.NetClient",
                    "kick",
                    "server",
                    "high",
                    true,
                    vec![test_param(
                        "reason",
                        "mindustry.net.Packets.KickReason",
                        true,
                        true,
                    )],
                ),
                test_remote_packet(
                    23,
                    27,
                    "mindustry.gen.KickCallPacket2",
                    "mindustry.core.NetClient",
                    "kick",
                    "server",
                    "high",
                    false,
                    vec![test_param(
                        "reason",
                        "mindustry.net.Packets.KickReason",
                        true,
                        true,
                    )],
                ),
                test_remote_packet(
                    24,
                    28,
                    "mindustry.gen.SendChatMessageDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "sendChatMessage",
                    "client",
                    "normal",
                    true,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("message", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    25,
                    29,
                    "mindustry.gen.SendChatMessageCallPacket",
                    "mindustry.core.NetClient",
                    "sendChatMessage",
                    "client",
                    "normal",
                    false,
                    vec![
                        test_param("player", "Player", false, false),
                        test_param("message", "java.lang.String", true, true),
                    ],
                ),
                test_remote_packet(
                    26,
                    30,
                    "mindustry.gen.SendMessageDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "sendMessage",
                    "server",
                    "normal",
                    true,
                    vec![test_param("message", "java.lang.String", true, true)],
                ),
                test_remote_packet(
                    27,
                    31,
                    "mindustry.gen.SendMessageCallPacket",
                    "mindustry.core.NetClient",
                    "sendMessage",
                    "server",
                    "normal",
                    false,
                    vec![test_param("message", "java.lang.String", true, true)],
                ),
                test_remote_packet(
                    28,
                    32,
                    "mindustry.gen.SendMessageDecoyCallPacket2",
                    "mindustry.core.NetClient",
                    "sendMessage",
                    "server",
                    "normal",
                    true,
                    vec![
                        test_param("message", "java.lang.String", true, true),
                        test_param("unformatted", "java.lang.String", true, true),
                        test_param("playersender", "Player", true, true),
                    ],
                ),
                test_remote_packet(
                    29,
                    33,
                    "mindustry.gen.SendMessageCallPacket2",
                    "mindustry.core.NetClient",
                    "sendMessage",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("message", "java.lang.String", true, true),
                        test_param("unformatted", "java.lang.String", true, true),
                        test_param("playersender", "Player", true, true),
                    ],
                ),
                test_remote_packet(
                    30,
                    34,
                    "mindustry.gen.ConnectDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "connect",
                    "server",
                    "normal",
                    true,
                    vec![
                        test_param("ip", "java.lang.String", true, true),
                        test_param("port", "int", true, true),
                    ],
                ),
                test_remote_packet(
                    31,
                    35,
                    "mindustry.gen.ConnectCallPacket",
                    "mindustry.core.NetClient",
                    "connect",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("ip", "java.lang.String", true, true),
                        test_param("port", "int", true, true),
                    ],
                ),
                test_remote_packet(
                    32,
                    36,
                    "mindustry.gen.PlayerSpawnDecoyCallPacket",
                    "mindustry.world.blocks.storage.CoreBlock",
                    "playerSpawn",
                    "server",
                    "normal",
                    true,
                    vec![
                        test_param("tile", "mindustry.world.Tile", true, true),
                        test_param("player", "Player", true, true),
                    ],
                ),
                test_remote_packet(
                    33,
                    37,
                    "mindustry.gen.PlayerSpawnCallPacket",
                    "mindustry.world.blocks.storage.CoreBlock",
                    "playerSpawn",
                    "server",
                    "normal",
                    false,
                    vec![
                        test_param("tile", "mindustry.world.Tile", true, true),
                        test_param("player", "Player", true, true),
                    ],
                ),
            ],
            wire: WireSpec {
                packet_id_byte: "u8".into(),
                length_field: "u16be".into(),
                compression_flag: CompressionFlagSpec {
                    none: "none".into(),
                    lz4: "lz4".into(),
                },
                compression_threshold: 36,
            },
        }
    }

    fn test_remote_packet(
        remote_index: usize,
        packet_id: u8,
        packet_class: &str,
        declaring_type: &str,
        method: &str,
        targets: &str,
        priority: &str,
        unreliable: bool,
        params: Vec<RemoteParamEntry>,
    ) -> RemotePacketEntry {
        RemotePacketEntry {
            remote_index,
            packet_id,
            packet_class: packet_class.into(),
            declaring_type: declaring_type.into(),
            method: method.into(),
            targets: targets.into(),
            called: "none".into(),
            variants: "all".into(),
            allow_on_client: None,
            allow_on_server: None,
            forward: false,
            unreliable,
            priority: priority.into(),
            params,
        }
    }

    fn test_param(name: &str, java_type: &str, client: bool, server: bool) -> RemoteParamEntry {
        RemoteParamEntry {
            name: name.into(),
            java_type: java_type.into(),
            network_included_when_caller_is_client: client,
            network_included_when_caller_is_server: server,
        }
    }
}
