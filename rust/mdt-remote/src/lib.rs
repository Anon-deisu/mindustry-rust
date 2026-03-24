use serde::{Deserialize, Serialize};
use std::{fmt, fs, path::Path};

pub const REMOTE_MANIFEST_SCHEMA_V1: &str = "mdt.remote.manifest.v1";
pub const HIGH_FREQUENCY_REMOTE_METHOD_COUNT: usize = 5;
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
pub struct RemoteGeneratorInfo {
    pub source: String,
    #[serde(rename = "callClass")]
    pub call_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasePacketEntry {
    pub id: u8,
    #[serde(rename = "class")]
    pub class_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub forward: bool,
    pub unreliable: bool,
    pub priority: String,
    pub params: Vec<RemoteParamEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

pub fn read_remote_manifest(path: impl AsRef<Path>) -> Result<RemoteManifest, RemoteManifestError> {
    let text = fs::read_to_string(path)?;
    parse_remote_manifest(&text)
}

pub fn parse_remote_manifest(text: &str) -> Result<RemoteManifest, RemoteManifestError> {
    let manifest: RemoteManifest = serde_json::from_str(text)?;
    validate_remote_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_remote_manifest(manifest: &RemoteManifest) -> Result<(), RemoteManifestError> {
    if manifest.schema != REMOTE_MANIFEST_SCHEMA_V1 {
        return Err(RemoteManifestError::UnsupportedSchema(
            manifest.schema.clone(),
        ));
    }

    validate_wire_spec(&manifest.wire)?;

    for (index, packet) in manifest.base_packets.iter().enumerate() {
        if packet.id != index as u8 {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "base packet {} has id {}, expected {}",
                packet.class_name, packet.id, index
            )));
        }
    }

    let remote_id_offset = manifest.base_packets.len() as u8;
    for (index, packet) in manifest.remote_packets.iter().enumerate() {
        if packet.remote_index != index {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "remote packet {} has remoteIndex {}, expected {}",
                packet.packet_class, packet.remote_index, index
            )));
        }

        let expected_packet_id = remote_id_offset + index as u8;
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
        if packet.variants.trim().is_empty() {
            return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                "remote packet {} has empty variants",
                packet.packet_class
            )));
        }

        remote_flow_from_targets(&packet.targets)?;
        remote_priority_from_str(&packet.priority)?;

        for param in &packet.params {
            if param.name.trim().is_empty() {
                return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "remote packet {} has param with empty name",
                    packet.packet_class
                )));
            }
            if param.java_type.trim().is_empty() {
                return Err(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "remote packet {} param {} has empty javaType",
                    packet.packet_class, param.name
                )));
            }
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

pub fn high_frequency_remote_packets(
    manifest: &RemoteManifest,
) -> Result<Vec<TypedRemotePacketSpec<'_>>, RemoteManifestError> {
    let registry = RemotePacketRegistry::from_manifest(manifest)?;
    let mut packets = Vec::with_capacity(HIGH_FREQUENCY_REMOTE_METHOD_COUNT);
    for method in HighFrequencyRemoteMethod::ordered() {
        let entry = registry
            .first_matching(RemotePacketSelector::high_frequency(method))
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                method.method_name(),
            ))?;

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

pub fn generate_rust_registry(manifest: &RemoteManifest) -> String {
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
    out.finish()
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

pub fn remote_packet_const_name(packet_class: &str) -> String {
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

fn param_is_wire_included_client_server(
    network_included_when_caller_is_client: bool,
    network_included_when_caller_is_server: bool,
    flow: RemoteFlow,
) -> bool {
    match flow {
        RemoteFlow::ClientToServer => network_included_when_caller_is_client,
        RemoteFlow::ServerToClient => network_included_when_caller_is_server,
        RemoteFlow::Bidirectional => {
            network_included_when_caller_is_client || network_included_when_caller_is_server
        }
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

    pub fn into_packets(self) -> Vec<TypedRemotePacketMetadata<'a>> {
        self.packets
    }
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

fn typed_remote_packet_metadata(
    entry: &RemotePacketEntry,
) -> Result<TypedRemotePacketMetadata<'_>, RemoteManifestError> {
    let flow = remote_flow_from_targets(&entry.targets)?;
    let priority = remote_priority_from_str(&entry.priority)?;
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
                flow,
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
    fn generates_rust_registry_from_manifest_sample() {
        let manifest = parse_remote_manifest(SAMPLE_MANIFEST).unwrap();
        let registry = generate_rust_registry(&manifest);
        assert!(registry.contains("pub const TEST_CALL_PACKET_ID: u8 = 4;"));
        assert!(registry.contains("pub const REMOTE_PACKET_SPECS: &[RemotePacketSpec] = &["));
        assert!(registry.contains("priority: \"high\""));
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
