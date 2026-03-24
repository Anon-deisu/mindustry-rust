#[path = "inbound_remote_registry_glue.rs"]
mod inbound_remote_registry_glue;
#[path = "snapshot_registry_glue.rs"]
mod snapshot_registry_glue;

use crate::generated::remote_high_frequency_gen::{
    BLOCK_SNAPSHOT_PACKET_ID, ENTITY_SNAPSHOT_PACKET_ID, HIDDEN_SNAPSHOT_PACKET_ID,
    STATE_SNAPSHOT_PACKET_ID,
};
use crate::snapshot_ingest::InboundSnapshot;
use mdt_remote::{
    CustomChannelRemoteFamily, CustomChannelRemoteRegistry, HighFrequencyRemoteMethod,
    InboundRemoteDispatchSpec, InboundRemoteFamily, RemoteManifest, RemoteManifestError,
    INBOUND_REMOTE_FAMILY_COUNT,
};

const INBOUND_SNAPSHOT_PACKET_SPECS: [(u8, HighFrequencyRemoteMethod); 4] = [
    (
        STATE_SNAPSHOT_PACKET_ID,
        HighFrequencyRemoteMethod::StateSnapshot,
    ),
    (
        ENTITY_SNAPSHOT_PACKET_ID,
        HighFrequencyRemoteMethod::EntitySnapshot,
    ),
    (
        BLOCK_SNAPSHOT_PACKET_ID,
        HighFrequencyRemoteMethod::BlockSnapshot,
    ),
    (
        HIDDEN_SNAPSHOT_PACKET_ID,
        HighFrequencyRemoteMethod::HiddenSnapshot,
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundSnapshotPacketRegistry {
    by_packet_id: [(u8, HighFrequencyRemoteMethod); 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRemotePacketRegistry {
    by_packet_id: [(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomChannelPacketRegistry {
    registry: CustomChannelRemoteRegistry,
}

impl Default for InboundSnapshotPacketRegistry {
    fn default() -> Self {
        Self {
            by_packet_id: INBOUND_SNAPSHOT_PACKET_SPECS,
        }
    }
}

impl InboundSnapshotPacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            by_packet_id: snapshot_registry_glue::typed_inbound_snapshot_packet_specs(manifest)?,
        })
    }

    pub fn classify<'a>(&self, packet_id: u8, payload: &'a [u8]) -> Option<InboundSnapshot<'a>> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, method)| {
                (*known_packet_id == packet_id).then_some(*method)
            })
            .map(|method| InboundSnapshot::new(method, packet_id, payload))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id
            .iter()
            .any(|(known_packet_id, _)| *known_packet_id == packet_id)
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }
}

impl CustomChannelPacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            registry: CustomChannelRemoteRegistry::from_manifest(manifest)?,
        })
    }

    pub fn classify(&self, packet_id: u8) -> Option<CustomChannelRemoteFamily> {
        self.registry.classify(packet_id)
    }

    pub fn packet_id(&self, family: CustomChannelRemoteFamily) -> Option<u8> {
        self.registry.packet_id(family)
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.registry.contains_packet_id(packet_id)
    }

    pub fn len(&self) -> usize {
        self.registry.len()
    }
}

impl InboundRemotePacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            by_packet_id: inbound_remote_registry_glue::typed_inbound_remote_packet_specs(
                manifest,
            )?,
        })
    }

    pub fn classify(&self, packet_id: u8) -> Option<InboundRemoteFamily> {
        self.dispatch_spec(packet_id).map(|spec| spec.family)
    }

    pub fn packet_id(&self, family: InboundRemoteFamily) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, spec)| (spec.family == family).then_some(*packet_id))
    }

    pub fn dispatch_spec(&self, packet_id: u8) -> Option<InboundRemoteDispatchSpec> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, spec)| (*known_packet_id == packet_id).then_some(*spec))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id
            .iter()
            .any(|(known_packet_id, _)| *known_packet_id == packet_id)
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CustomChannelPacketRegistry, InboundRemoteDispatchSpec, InboundRemoteFamily,
        InboundRemotePacketRegistry,
    };
    use mdt_remote::{
        read_remote_manifest, BasePacketEntry, CompressionFlagSpec, CustomChannelRemoteFamily,
        CustomChannelRemotePayloadKind, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
        RemoteGeneratorInfo, RemoteManifest, RemotePacketEntry, RemoteParamEntry, WireSpec,
    };
    use std::path::PathBuf;

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn builds_inbound_remote_family_registry_from_real_manifest() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundRemotePacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(registry.len(), 6);
        assert_eq!(
            registry.classify(
                registry
                    .packet_id(InboundRemoteFamily::ServerPacketReliable)
                    .unwrap()
            ),
            Some(InboundRemoteFamily::ServerPacketReliable)
        );
        assert_eq!(
            registry.classify(
                registry
                    .packet_id(InboundRemoteFamily::ClientLogicDataUnreliable)
                    .unwrap()
            ),
            Some(InboundRemoteFamily::ClientLogicDataUnreliable)
        );
    }

    #[test]
    fn builds_custom_channel_remote_family_registry_from_real_manifest() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = CustomChannelPacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(registry.len(), 10);
        assert_eq!(
            registry.classify(
                registry
                    .packet_id(CustomChannelRemoteFamily::ClientPacketReliable)
                    .unwrap()
            ),
            Some(CustomChannelRemoteFamily::ClientPacketReliable)
        );
        assert_eq!(
            registry.classify(
                registry
                    .packet_id(CustomChannelRemoteFamily::ServerPacketUnreliable)
                    .unwrap()
            ),
            Some(CustomChannelRemoteFamily::ServerPacketUnreliable)
        );
        assert_eq!(
            registry.classify(
                registry
                    .packet_id(CustomChannelRemoteFamily::ClientLogicDataReliable)
                    .unwrap()
            ),
            Some(CustomChannelRemoteFamily::ClientLogicDataReliable)
        );
        assert!(!registry.contains_packet_id(24));
    }

    #[test]
    fn custom_channel_remote_family_registry_prefers_typed_signature_over_method_name_only() {
        let manifest = custom_channel_remote_family_manifest_with_decoys();
        let registry = CustomChannelPacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(
            registry.packet_id(CustomChannelRemoteFamily::ClientPacketReliable),
            Some(5)
        );
        assert_eq!(registry.classify(4), None);
        assert_eq!(
            registry.packet_id(CustomChannelRemoteFamily::ServerPacketReliable),
            Some(10)
        );
        assert_eq!(registry.classify(9), None);
        assert_eq!(
            registry.classify(10),
            Some(CustomChannelRemoteFamily::ServerPacketReliable)
        );
    }

    #[test]
    fn inbound_remote_family_registry_reuses_custom_channel_typed_lookup() {
        let manifest = custom_channel_remote_family_manifest_with_decoys();
        let registry = InboundRemotePacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(
            registry.packet_id(InboundRemoteFamily::ServerPacketReliable),
            Some(10)
        );
        assert_eq!(registry.classify(9), None);
        assert_eq!(
            registry.classify(10),
            Some(InboundRemoteFamily::ServerPacketReliable)
        );
    }

    #[test]
    fn inbound_remote_family_registry_exposes_typed_dispatch_specs() {
        let manifest = custom_channel_remote_family_manifest_with_decoys();
        let registry = InboundRemotePacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(registry.contains_packet_id(10), true);
        assert_eq!(registry.contains_packet_id(9), false);
        assert_eq!(
            registry.dispatch_spec(10),
            Some(InboundRemoteDispatchSpec {
                family: InboundRemoteFamily::ServerPacketReliable,
                payload_kind: CustomChannelRemotePayloadKind::Text,
            })
        );
        assert_eq!(
            registry.dispatch_spec(15),
            Some(InboundRemoteDispatchSpec {
                family: InboundRemoteFamily::ClientLogicDataUnreliable,
                payload_kind: CustomChannelRemotePayloadKind::LogicData,
            })
        );
        assert_eq!(registry.dispatch_spec(9), None);
    }

    #[test]
    fn inbound_remote_family_registry_matches_remote_typed_registry() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundRemotePacketRegistry::from_remote_manifest(&manifest).unwrap();
        let remote_registry = mdt_remote::InboundRemoteRegistry::from_manifest(&manifest).unwrap();
        let remote_specs = mdt_remote::typed_inbound_remote_dispatch_specs(&manifest).unwrap();

        let packet_id = remote_registry
            .packet_id(InboundRemoteFamily::ServerPacketReliable)
            .unwrap();
        assert_eq!(
            registry.packet_id(InboundRemoteFamily::ServerPacketReliable),
            Some(packet_id)
        );
        assert_eq!(
            registry.dispatch_spec(packet_id),
            remote_registry.dispatch_spec(packet_id)
        );
        assert_eq!(
            registry.dispatch_spec(remote_specs[4].0),
            Some(remote_specs[4].1)
        );
    }

    #[test]
    fn inbound_snapshot_registry_reuses_high_frequency_typed_registry_packet_ids() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry =
            super::InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let remote_registry = HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();

        assert_eq!(
            registry.classify(122, &[1]).map(|packet| packet.method),
            Some(HighFrequencyRemoteMethod::StateSnapshot)
        );
        assert_eq!(
            registry.classify(44, &[2]).map(|packet| packet.method),
            Some(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            remote_registry.packet_id(HighFrequencyRemoteMethod::StateSnapshot),
            Some(122)
        );
        assert_eq!(
            remote_registry.packet_id(HighFrequencyRemoteMethod::EntitySnapshot),
            Some(44)
        );
        assert!(registry.contains_packet_id(
            remote_registry
                .packet_id(HighFrequencyRemoteMethod::BlockSnapshot)
                .unwrap()
        ));
        assert!(!registry.contains_packet_id(
            remote_registry
                .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
                .unwrap()
        ));
    }

    fn custom_channel_remote_family_manifest_with_decoys() -> RemoteManifest {
        RemoteManifest {
            schema: "mdt.remote.manifest.v1".into(),
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
}
