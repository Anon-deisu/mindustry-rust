use crate::packet_registry::{CustomChannelPacketRegistry, InboundRemotePacketRegistry};
use mdt_remote::{
    CustomChannelRemoteFamily, CustomChannelRemotePayloadKind, InboundRemoteFamily, RemoteManifest,
    RemoteManifestError,
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

#[derive(Debug, Clone, PartialEq)]
pub enum TypedCustomChannelRemoteDispatch {
    Text {
        family: CustomChannelRemoteFamily,
        packet_type: String,
        contents: String,
    },
    Binary {
        family: CustomChannelRemoteFamily,
        packet_type: String,
        contents: Vec<u8>,
    },
    LogicData {
        family: CustomChannelRemoteFamily,
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
pub struct TypedCustomChannelRemoteDispatchError {
    pub family: CustomChannelRemoteFamily,
    pub packet_id: u8,
    pub payload_kind: CustomChannelRemotePayloadKind,
    pub reason: String,
}

impl fmt::Display for TypedCustomChannelRemoteDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to decode {:?} custom-channel remote packet {} as {:?}: {}",
            self.family, self.packet_id, self.payload_kind, self.reason
        )
    }
}

impl std::error::Error for TypedCustomChannelRemoteDispatchError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInboundRemoteDispatcher {
    registry: InboundRemotePacketRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCustomChannelRemoteDispatcher {
    registry: CustomChannelPacketRegistry,
}

impl TypedInboundRemoteDispatcher {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        Ok(Self::from_packet_registry(
            InboundRemotePacketRegistry::from_remote_manifest(manifest)?,
        ))
    }

    pub fn from_packet_registry(registry: InboundRemotePacketRegistry) -> Self {
        Self { registry }
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

        let decoded = decode_payload(spec.payload_kind, payload).map_err(|reason| {
            TypedInboundRemoteDispatchError {
                family: spec.family,
                packet_id,
                payload_kind: spec.payload_kind,
                reason,
            }
        })?;
        Ok(Some(decoded.into_inbound_dispatch(spec.family)))
    }
}

impl TypedInboundRemoteDispatch {
    pub fn payload_kind_label(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Binary { .. } => "binary",
            Self::LogicData { .. } => "logic",
        }
    }

    pub fn route_label(&self) -> String {
        match self {
            Self::Text { family, .. }
            | Self::Binary { family, .. }
            | Self::LogicData { family, .. } => {
                format!("{}/{}", family.method_name(), self.payload_kind_label())
            }
        }
    }
}

impl TypedCustomChannelRemoteDispatcher {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            registry: CustomChannelPacketRegistry::from_remote_manifest(manifest)?,
        })
    }

    pub fn from_packet_registry(registry: CustomChannelPacketRegistry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &CustomChannelPacketRegistry {
        &self.registry
    }

    pub fn dispatch(
        &self,
        packet_id: u8,
        payload: &[u8],
    ) -> Result<Option<TypedCustomChannelRemoteDispatch>, TypedCustomChannelRemoteDispatchError>
    {
        let Some(spec) = self.registry.dispatch_spec(packet_id) else {
            return Ok(None);
        };

        let decoded = decode_payload(spec.payload_kind, payload).map_err(|reason| {
            TypedCustomChannelRemoteDispatchError {
                family: spec.family,
                packet_id,
                payload_kind: spec.payload_kind,
                reason,
            }
        })?;
        Ok(Some(decoded.into_custom_channel_dispatch(spec.family)))
    }
}

impl TypedCustomChannelRemoteDispatch {
    pub fn payload_kind_label(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Binary { .. } => "binary",
            Self::LogicData { .. } => "logic",
        }
    }

    pub fn route_label(&self) -> String {
        match self {
            Self::Text { family, .. }
            | Self::Binary { family, .. }
            | Self::LogicData { family, .. } => {
                format!("{}/{}", family.method_name(), self.payload_kind_label())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DecodedRemotePayload {
    Text {
        packet_type: String,
        contents: String,
    },
    Binary {
        packet_type: String,
        contents: Vec<u8>,
    },
    LogicData {
        channel: String,
        value: TypeIoObject,
    },
}

impl DecodedRemotePayload {
    fn into_inbound_dispatch(self, family: InboundRemoteFamily) -> TypedInboundRemoteDispatch {
        match self {
            Self::Text {
                packet_type,
                contents,
            } => TypedInboundRemoteDispatch::Text {
                family,
                packet_type,
                contents,
            },
            Self::Binary {
                packet_type,
                contents,
            } => TypedInboundRemoteDispatch::Binary {
                family,
                packet_type,
                contents,
            },
            Self::LogicData { channel, value } => TypedInboundRemoteDispatch::LogicData {
                family,
                channel,
                value,
            },
        }
    }

    fn into_custom_channel_dispatch(
        self,
        family: CustomChannelRemoteFamily,
    ) -> TypedCustomChannelRemoteDispatch {
        match self {
            Self::Text {
                packet_type,
                contents,
            } => TypedCustomChannelRemoteDispatch::Text {
                family,
                packet_type,
                contents,
            },
            Self::Binary {
                packet_type,
                contents,
            } => TypedCustomChannelRemoteDispatch::Binary {
                family,
                packet_type,
                contents,
            },
            Self::LogicData { channel, value } => TypedCustomChannelRemoteDispatch::LogicData {
                family,
                channel,
                value,
            },
        }
    }
}

fn decode_payload(
    payload_kind: CustomChannelRemotePayloadKind,
    payload: &[u8],
) -> Result<DecodedRemotePayload, String> {
    match payload_kind {
        CustomChannelRemotePayloadKind::Text => {
            let (packet_type, contents) = decode_text_payload(payload)?;
            Ok(DecodedRemotePayload::Text {
                packet_type,
                contents,
            })
        }
        CustomChannelRemotePayloadKind::Binary => {
            let (packet_type, contents) = decode_binary_payload(payload)?;
            Ok(DecodedRemotePayload::Binary {
                packet_type,
                contents,
            })
        }
        CustomChannelRemotePayloadKind::LogicData => {
            let (channel, value) = decode_logic_data_payload(payload)?;
            Ok(DecodedRemotePayload::LogicData { channel, value })
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
    use super::{
        TypedCustomChannelRemoteDispatch, TypedCustomChannelRemoteDispatcher,
        TypedInboundRemoteDispatch, TypedInboundRemoteDispatcher,
    };
    use mdt_remote::{
        BasePacketEntry, CompressionFlagSpec, CustomChannelRemoteFamily,
        CustomChannelRemotePayloadKind, InboundRemoteFamily, RemoteGeneratorInfo, RemoteManifest,
        RemotePacketEntry, RemoteParamEntry, WireSpec, REMOTE_MANIFEST_SCHEMA_V1,
    };
    use mdt_typeio::{write_object, write_string, TypeIoObject};

    #[test]
    fn typed_dispatch_ignores_method_only_decoy_packet_ids() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let payload = encode_text_payload("mod.echo", "hello");

        assert_eq!(dispatcher.dispatch(9, &payload).unwrap(), None);
        assert_eq!(
            dispatcher.dispatch(10, &payload).unwrap(),
            Some(TypedInboundRemoteDispatch::Text {
                family: InboundRemoteFamily::ServerPacketReliable,
                packet_type: "mod.echo".to_string(),
                contents: "hello".to_string(),
            })
        );
        assert_eq!(
            dispatcher
                .dispatch(10, &payload)
                .unwrap()
                .unwrap()
                .route_label(),
            "serverPacketReliable/text"
        );
    }

    #[test]
    fn typed_dispatch_decodes_client_logic_data_payloads() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        let payload = encode_logic_payload("logic.alpha", &value);

        assert_eq!(
            dispatcher.dispatch(15, &payload).unwrap(),
            Some(TypedInboundRemoteDispatch::LogicData {
                family: InboundRemoteFamily::ClientLogicDataUnreliable,
                channel: "logic.alpha".to_string(),
                value,
            })
        );
        assert_eq!(
            dispatcher
                .dispatch(15, &payload)
                .unwrap()
                .unwrap()
                .route_label(),
            "clientLogicDataUnreliable/logic"
        );
    }

    #[test]
    fn typed_dispatch_reports_payload_shape_errors_with_family_context() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let error = dispatcher.dispatch(10, &[1, 0, 3, b'a']).unwrap_err();

        assert_eq!(error.family, InboundRemoteFamily::ServerPacketReliable);
        assert_eq!(error.packet_id, 10);
        assert_eq!(error.payload_kind, CustomChannelRemotePayloadKind::Text);
        assert_eq!(
            error.reason,
            "unexpected EOF at 3: need 3 bytes, only 1 remaining".to_string()
        );
    }

    #[test]
    fn typed_dispatch_reports_trailing_bytes_for_binary_payloads() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();

        let mut binary_payload = encode_binary_payload("mod.bin", &[1, 2, 3, 4]);
        binary_payload.push(0xff);
        let binary_error = dispatcher.dispatch(8, &binary_payload).unwrap_err();
        assert_eq!(
            binary_error.family,
            CustomChannelRemoteFamily::ClientBinaryPacketUnreliable
        );
        assert_eq!(binary_error.packet_id, 8);
        assert_eq!(
            binary_error.payload_kind,
            CustomChannelRemotePayloadKind::Binary
        );
        assert_eq!(
            binary_error.reason,
            format!(
                "payload has trailing bytes: consumed {}, total {}",
                binary_payload.len() - 1,
                binary_payload.len()
            )
        );

        let value = TypeIoObject::ObjectArray(vec![TypeIoObject::Int(3), TypeIoObject::Bool(false)]);
        let mut logic_payload = encode_logic_payload("logic.beta", &value);
        logic_payload.push(0xff);
        let logic_error = dispatcher.dispatch(14, &logic_payload).unwrap_err();
        assert_eq!(
            logic_error.family,
            CustomChannelRemoteFamily::ClientLogicDataReliable
        );
        assert_eq!(logic_error.packet_id, 14);
        assert_eq!(
            logic_error.payload_kind,
            CustomChannelRemotePayloadKind::LogicData
        );
        assert_eq!(
            logic_error.reason,
            format!(
                "payload has trailing bytes: consumed {}, total {}",
                logic_payload.len() - 1,
                logic_payload.len()
            )
        );
    }

    #[test]
    fn typed_dispatch_reports_trailing_bytes_for_text_payloads() {
        let manifest = custom_channel_manifest_with_decoys();
        let inbound_dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let mut inbound_payload = encode_text_payload("mod.echo", "hello");
        inbound_payload.push(0xff);
        let inbound_error = inbound_dispatcher.dispatch(10, &inbound_payload).unwrap_err();
        assert_eq!(inbound_error.family, InboundRemoteFamily::ServerPacketReliable);
        assert_eq!(inbound_error.packet_id, 10);
        assert_eq!(inbound_error.payload_kind, CustomChannelRemotePayloadKind::Text);
        assert_eq!(
            inbound_error.reason,
            format!(
                "payload has trailing bytes: consumed {}, total {}",
                inbound_payload.len() - 1,
                inbound_payload.len()
            )
        );

        let custom_dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let mut custom_payload = encode_text_payload("mod.client", "hello");
        custom_payload.push(0xff);
        let custom_error = custom_dispatcher.dispatch(5, &custom_payload).unwrap_err();
        assert_eq!(
            custom_error.family,
            CustomChannelRemoteFamily::ClientPacketReliable
        );
        assert_eq!(custom_error.packet_id, 5);
        assert_eq!(custom_error.payload_kind, CustomChannelRemotePayloadKind::Text);
        assert_eq!(
            custom_error.reason,
            format!(
                "payload has trailing bytes: consumed {}, total {}",
                custom_payload.len() - 1,
                custom_payload.len()
            )
        );
    }

    #[test]
    fn typed_dispatch_reports_binary_route_label_symmetry() {
        let manifest = custom_channel_manifest_with_decoys();
        let inbound_dispatcher = TypedInboundRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let custom_dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let payload = encode_binary_payload("mod.bin", &[1, 2, 3, 4]);

        let inbound = inbound_dispatcher.dispatch(12, &payload).unwrap().unwrap();
        assert_eq!(inbound.payload_kind_label(), "binary");
        assert_eq!(inbound.route_label(), "serverBinaryPacketReliable/binary");
        assert_eq!(
            inbound,
            TypedInboundRemoteDispatch::Binary {
                family: InboundRemoteFamily::ServerBinaryPacketReliable,
                packet_type: "mod.bin".to_string(),
                contents: vec![1, 2, 3, 4],
            }
        );

        let custom = custom_dispatcher.dispatch(8, &payload).unwrap().unwrap();
        assert_eq!(custom.payload_kind_label(), "binary");
        assert_eq!(custom.route_label(), "clientBinaryPacketUnreliable/binary");
        assert_eq!(
            custom,
            TypedCustomChannelRemoteDispatch::Binary {
                family: CustomChannelRemoteFamily::ClientBinaryPacketUnreliable,
                packet_type: "mod.bin".to_string(),
                contents: vec![1, 2, 3, 4],
            }
        );
    }

    #[test]
    fn custom_channel_typed_dispatch_ignores_method_only_decoy_packet_ids() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let payload = encode_text_payload("mod.client", "hello");

        assert_eq!(dispatcher.dispatch(4, &payload).unwrap(), None);
        assert_eq!(
            dispatcher.dispatch(5, &payload).unwrap(),
            Some(TypedCustomChannelRemoteDispatch::Text {
                family: CustomChannelRemoteFamily::ClientPacketReliable,
                packet_type: "mod.client".to_string(),
                contents: "hello".to_string(),
            })
        );
        assert_eq!(
            dispatcher
                .dispatch(5, &payload)
                .unwrap()
                .unwrap()
                .route_label(),
            "clientPacketReliable/text"
        );
    }

    #[test]
    fn custom_channel_typed_dispatch_decodes_client_binary_payloads() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let payload = encode_binary_payload("mod.bin", &[1, 2, 3, 4]);

        assert_eq!(
            dispatcher.dispatch(8, &payload).unwrap(),
            Some(TypedCustomChannelRemoteDispatch::Binary {
                family: CustomChannelRemoteFamily::ClientBinaryPacketUnreliable,
                packet_type: "mod.bin".to_string(),
                contents: vec![1, 2, 3, 4],
            })
        );
    }

    #[test]
    fn custom_channel_typed_dispatch_decodes_logic_payloads() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let value =
            TypeIoObject::ObjectArray(vec![TypeIoObject::Int(3), TypeIoObject::Bool(false)]);
        let payload = encode_logic_payload("logic.beta", &value);

        assert_eq!(
            dispatcher.dispatch(14, &payload).unwrap(),
            Some(TypedCustomChannelRemoteDispatch::LogicData {
                family: CustomChannelRemoteFamily::ClientLogicDataReliable,
                channel: "logic.beta".to_string(),
                value,
            })
        );
        assert_eq!(
            dispatcher
                .dispatch(14, &payload)
                .unwrap()
                .unwrap()
                .route_label(),
            "clientLogicDataReliable/logic"
        );
    }

    #[test]
    fn custom_channel_typed_dispatch_reports_payload_shape_errors_with_family_context() {
        let manifest = custom_channel_manifest_with_decoys();
        let dispatcher =
            TypedCustomChannelRemoteDispatcher::from_remote_manifest(&manifest).unwrap();
        let error = dispatcher.dispatch(5, &[1, 0, 3, b'a']).unwrap_err();

        assert_eq!(
            error.family,
            CustomChannelRemoteFamily::ClientPacketReliable
        );
        assert_eq!(error.packet_id, 5);
        assert_eq!(error.payload_kind, CustomChannelRemotePayloadKind::Text);
        assert_eq!(
            error.reason,
            "unexpected EOF at 3: need 3 bytes, only 1 remaining".to_string()
        );
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
                remote_packet(
                    0,
                    4,
                    "mindustry.gen.ClientPacketReliableDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketReliable",
                    "server",
                    "client",
                    false,
                    vec![
                        param("player", "Player", false, false),
                        param("contents", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    1,
                    5,
                    "mindustry.gen.ClientPacketReliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketReliable",
                    "server",
                    "client",
                    false,
                    vec![
                        param("type", "java.lang.String", true, true),
                        param("contents", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    2,
                    6,
                    "mindustry.gen.ClientPacketUnreliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientPacketUnreliable",
                    "server",
                    "client",
                    true,
                    vec![
                        param("type", "java.lang.String", true, true),
                        param("contents", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    3,
                    7,
                    "mindustry.gen.ClientBinaryPacketReliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientBinaryPacketReliable",
                    "server",
                    "client",
                    false,
                    vec![
                        param("type", "java.lang.String", true, true),
                        param("contents", "byte[]", true, true),
                    ],
                ),
                remote_packet(
                    4,
                    8,
                    "mindustry.gen.ClientBinaryPacketUnreliableCallPacket",
                    "mindustry.core.NetClient",
                    "clientBinaryPacketUnreliable",
                    "server",
                    "client",
                    true,
                    vec![
                        param("type", "java.lang.String", true, true),
                        param("contents", "byte[]", true, true),
                    ],
                ),
                remote_packet(
                    5,
                    9,
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
                    6,
                    10,
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
                    7,
                    11,
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
                    8,
                    12,
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
                    9,
                    13,
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
                    10,
                    14,
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
                remote_packet(
                    11,
                    15,
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

    fn encode_binary_payload(packet_type: &str, contents: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        write_string(&mut payload, Some(packet_type));
        payload.extend_from_slice(&(contents.len() as u16).to_be_bytes());
        payload.extend_from_slice(contents);
        payload
    }

    fn encode_logic_payload(channel: &str, value: &TypeIoObject) -> Vec<u8> {
        let mut payload = Vec::new();
        write_string(&mut payload, Some(channel));
        write_object(&mut payload, value);
        payload
    }
}
