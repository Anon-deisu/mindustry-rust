use crate::packet_registry::InboundRemotePacketRegistry;
use mdt_remote::{
    CustomChannelRemotePayloadKind, InboundRemoteFamily, RemoteManifest, RemoteManifestError,
};
use mdt_typeio::{read_object_prefix, read_string_prefix, TypeIoObject, TypeIoReadError};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TypedInboundRemoteDispatch {
    Text {
        family: InboundRemoteFamily,
        packet_type: String,
        contents: String,
    },
    Binary {
        family: InboundRemoteFamily,
        packet_type: String,
        contents: Vec<u8>,
    },
    LogicData {
        family: InboundRemoteFamily,
        channel: String,
        value: TypeIoObject,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInboundRemoteDispatchError {
    pub family: InboundRemoteFamily,
    pub packet_id: u8,
    pub payload_kind: CustomChannelRemotePayloadKind,
    pub reason: String,
}

impl fmt::Display for TypedInboundRemoteDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to decode {:?} inbound remote packet {} as {:?}: {}",
            self.family, self.packet_id, self.payload_kind, self.reason
        )
    }
}

impl std::error::Error for TypedInboundRemoteDispatchError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInboundRemoteDispatcher {
    registry: InboundRemotePacketRegistry,
}

impl TypedInboundRemoteDispatcher {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            registry: InboundRemotePacketRegistry::from_remote_manifest(manifest)?,
        })
    }

    pub fn registry(&self) -> &InboundRemotePacketRegistry {
        &self.registry
    }

    pub fn dispatch(
        &self,
        packet_id: u8,
        payload: &[u8],
    ) -> Result<Option<TypedInboundRemoteDispatch>, TypedInboundRemoteDispatchError> {
        let Some(spec) = self.registry.dispatch_spec(packet_id) else {
            return Ok(None);
        };

        match spec.payload_kind {
            CustomChannelRemotePayloadKind::Text => {
                let (packet_type, contents) = decode_text_payload(payload).map_err(|reason| {
                    TypedInboundRemoteDispatchError {
                        family: spec.family,
                        packet_id,
                        payload_kind: spec.payload_kind,
                        reason,
                    }
                })?;
                Ok(Some(TypedInboundRemoteDispatch::Text {
                    family: spec.family,
                    packet_type,
                    contents,
                }))
            }
            CustomChannelRemotePayloadKind::Binary => {
                let (packet_type, contents) = decode_binary_payload(payload).map_err(|reason| {
                    TypedInboundRemoteDispatchError {
                        family: spec.family,
                        packet_id,
                        payload_kind: spec.payload_kind,
                        reason,
                    }
                })?;
                Ok(Some(TypedInboundRemoteDispatch::Binary {
                    family: spec.family,
                    packet_type,
                    contents,
                }))
            }
            CustomChannelRemotePayloadKind::LogicData => {
                let (channel, value) = decode_logic_data_payload(payload).map_err(|reason| {
                    TypedInboundRemoteDispatchError {
                        family: spec.family,
                        packet_id,
                        payload_kind: spec.payload_kind,
                        reason,
                    }
                })?;
                Ok(Some(TypedInboundRemoteDispatch::LogicData {
                    family: spec.family,
                    channel,
                    value,
                }))
            }
        }
    }
}

fn decode_text_payload(payload: &[u8]) -> Result<(String, String), String> {
    let (packet_type, consumed_type) = decode_non_null_string_prefix(payload, "packet_type")?;
    let (contents, consumed_contents) =
        decode_non_null_string_prefix(&payload[consumed_type..], "contents")?;
    ensure_consumed(consumed_type + consumed_contents, payload.len())?;
    Ok((packet_type, contents))
}

fn decode_binary_payload(payload: &[u8]) -> Result<(String, Vec<u8>), String> {
    let (packet_type, consumed_type) = decode_non_null_string_prefix(payload, "packet_type")?;
    let (contents, consumed_contents) = read_typeio_bytes_prefix(&payload[consumed_type..])?;
    ensure_consumed(consumed_type + consumed_contents, payload.len())?;
    Ok((packet_type, contents))
}

fn decode_logic_data_payload(payload: &[u8]) -> Result<(String, TypeIoObject), String> {
    let (channel, consumed_channel) = decode_non_null_string_prefix(payload, "channel")?;
    let (value, consumed_value) =
        read_object_prefix(&payload[consumed_channel..]).map_err(typeio_error_to_string)?;
    ensure_consumed(consumed_channel + consumed_value, payload.len())?;
    Ok((channel, value))
}

fn decode_non_null_string_prefix(payload: &[u8], field: &str) -> Result<(String, usize), String> {
    let (value, consumed) = read_string_prefix(payload).map_err(typeio_error_to_string)?;
    let value = value.ok_or_else(|| format!("{field} string is null"))?;
    Ok((value, consumed))
}

fn read_typeio_bytes_prefix(payload: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let len_bytes = payload
        .get(..2)
        .ok_or_else(|| "bytes payload missing length prefix".to_string())?;
    let len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
    let bytes = payload
        .get(2..2 + len)
        .ok_or_else(|| format!("bytes payload truncated: expected {len} bytes"))?;
    Ok((bytes.to_vec(), 2 + len))
}

fn ensure_consumed(consumed: usize, total: usize) -> Result<(), String> {
    if consumed == total {
        Ok(())
    } else {
        Err(format!(
            "payload has trailing bytes: consumed {consumed}, total {total}"
        ))
    }
}

fn typeio_error_to_string(error: TypeIoReadError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{TypedInboundRemoteDispatch, TypedInboundRemoteDispatcher};
    use mdt_remote::{
        BasePacketEntry, CompressionFlagSpec, CustomChannelRemotePayloadKind, InboundRemoteFamily,
        RemoteGeneratorInfo, RemoteManifest, RemotePacketEntry, RemoteParamEntry, WireSpec,
        REMOTE_MANIFEST_SCHEMA_V1,
    };
    use mdt_typeio::{write_object, write_string, TypeIoObject};

    #[test]
    fn typed_dispatch_ignores_method_only_decoy_packet_ids() {
        let manifest = inbound_remote_manifest_with_decoy_text_family();
        let dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let payload = encode_text_payload("mod.echo", "hello");

        assert_eq!(dispatcher.dispatch(4, &payload).unwrap(), None);
        assert_eq!(
            dispatcher.dispatch(5, &payload).unwrap(),
            Some(TypedInboundRemoteDispatch::Text {
                family: InboundRemoteFamily::ServerPacketReliable,
                packet_type: "mod.echo".to_string(),
                contents: "hello".to_string(),
            })
        );
    }

    #[test]
    fn typed_dispatch_decodes_client_logic_data_payloads() {
        let manifest = inbound_remote_manifest_with_decoy_text_family();
        let dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        let payload = encode_logic_payload("logic.alpha", &value);

        assert_eq!(
            dispatcher.dispatch(6, &payload).unwrap(),
            Some(TypedInboundRemoteDispatch::LogicData {
                family: InboundRemoteFamily::ClientLogicDataUnreliable,
                channel: "logic.alpha".to_string(),
                value,
            })
        );
    }

    #[test]
    fn typed_dispatch_reports_payload_shape_errors_with_family_context() {
        let manifest = inbound_remote_manifest_with_decoy_text_family();
        let dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let error = dispatcher.dispatch(5, &[1, 0, 3, b'a']).unwrap_err();

        assert_eq!(error.family, InboundRemoteFamily::ServerPacketReliable);
        assert_eq!(error.packet_id, 5);
        assert_eq!(error.payload_kind, CustomChannelRemotePayloadKind::Text);
        assert_eq!(
            error.reason,
            "unexpected EOF at 3: need 3 bytes, only 1 remaining".to_string()
        );
    }

    fn inbound_remote_manifest_with_decoy_text_family() -> RemoteManifest {
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
                remote_packet(
                    0,
                    4,
                    "mindustry.gen.ServerPacketReliableDecoyCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketReliable",
                    "client",
                    "server",
                    false,
                    vec![
                        param("tile", "mindustry.world.Tile", false, false),
                        param("type", "java.lang.String", true, true),
                        param("contents", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    1,
                    5,
                    "mindustry.gen.ServerPacketReliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketReliable",
                    "client",
                    "server",
                    false,
                    vec![
                        param("player", "Player", false, false),
                        param("type", "java.lang.String", true, true),
                        param("contents", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    2,
                    6,
                    "mindustry.gen.ClientLogicDataUnreliableCallPacket",
                    "mindustry.core.NetServer",
                    "clientLogicDataUnreliable",
                    "client",
                    "server",
                    true,
                    vec![
                        param("player", "Player", false, false),
                        param("channel", "java.lang.String", true, true),
                        param("value", "java.lang.Object", true, true),
                    ],
                ),
                remote_packet(
                    3,
                    7,
                    "mindustry.gen.ServerPacketUnreliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverPacketUnreliable",
                    "client",
                    "server",
                    true,
                    vec![
                        param("player", "Player", false, false),
                        param("type", "java.lang.String", true, true),
                        param("contents", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    4,
                    8,
                    "mindustry.gen.ServerBinaryPacketReliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverBinaryPacketReliable",
                    "client",
                    "server",
                    false,
                    vec![
                        param("player", "Player", false, false),
                        param("type", "java.lang.String", true, true),
                        param("contents", "byte[]", true, true),
                    ],
                ),
                remote_packet(
                    5,
                    9,
                    "mindustry.gen.ServerBinaryPacketUnreliableCallPacket",
                    "mindustry.core.NetServer",
                    "serverBinaryPacketUnreliable",
                    "client",
                    "server",
                    true,
                    vec![
                        param("player", "Player", false, false),
                        param("type", "java.lang.String", true, true),
                        param("contents", "byte[]", true, true),
                    ],
                ),
                remote_packet(
                    6,
                    10,
                    "mindustry.gen.ClientLogicDataReliableCallPacket",
                    "mindustry.core.NetServer",
                    "clientLogicDataReliable",
                    "client",
                    "server",
                    false,
                    vec![
                        param("player", "Player", false, false),
                        param("channel", "java.lang.String", true, true),
                        param("value", "java.lang.Object", true, true),
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

    fn remote_packet(
        remote_index: usize,
        packet_id: u8,
        packet_class: &str,
        declaring_type: &str,
        method: &str,
        targets: &str,
        called: &str,
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
            called: called.into(),
            variants: "all".into(),
            allow_on_client: None,
            allow_on_server: None,
            forward: false,
            unreliable,
            priority: "normal".into(),
            params,
        }
    }

    fn param(name: &str, java_type: &str, client: bool, server: bool) -> RemoteParamEntry {
        RemoteParamEntry {
            name: name.into(),
            java_type: java_type.into(),
            network_included_when_caller_is_client: client,
            network_included_when_caller_is_server: server,
        }
    }

    fn encode_text_payload(packet_type: &str, contents: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        write_string(&mut payload, Some(packet_type));
        write_string(&mut payload, Some(contents));
        payload
    }

    fn encode_logic_payload(channel: &str, value: &TypeIoObject) -> Vec<u8> {
        let mut payload = Vec::new();
        write_string(&mut payload, Some(channel));
        write_object(&mut payload, value);
        payload
    }
}
