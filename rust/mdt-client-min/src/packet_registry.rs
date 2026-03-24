use crate::generated::remote_high_frequency_gen::{
    BLOCK_SNAPSHOT_PACKET_ID, ENTITY_SNAPSHOT_PACKET_ID, HIDDEN_SNAPSHOT_PACKET_ID,
    STATE_SNAPSHOT_PACKET_ID,
};
use crate::snapshot_ingest::InboundSnapshot;
use mdt_remote::{
    CustomChannelRemoteFamily, HighFrequencyRemoteMethod, InboundRemoteFamily, RemoteFlow,
    RemoteManifest, RemoteManifestError, RemotePacketRegistry, RemotePacketSelector,
    CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT, INBOUND_REMOTE_FAMILY_COUNT,
};
use std::collections::HashSet;

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
    by_packet_id: [(u8, InboundRemoteFamily); INBOUND_REMOTE_FAMILY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomChannelPacketRegistry {
    by_packet_id: [(u8, CustomChannelRemoteFamily); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT],
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
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        let mut resolved_entries = Vec::with_capacity(INBOUND_SNAPSHOT_PACKET_SPECS.len());
        let mut seen_packet_ids = HashSet::with_capacity(INBOUND_SNAPSHOT_PACKET_SPECS.len());

        for (_, method) in INBOUND_SNAPSHOT_PACKET_SPECS {
            let entry = registry
                .first_matching(
                    RemotePacketSelector::high_frequency(method)
                        .with_flow(RemoteFlow::ServerToClient)
                        .with_unreliable(true),
                )
                .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                    method.method_name(),
                ))?;
            if !seen_packet_ids.insert(entry.packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate high-frequency server->client snapshot packet id: {}",
                    entry.packet_id
                )));
            }
            resolved_entries.push((method, entry.packet_id));
        }

        for ((expected_packet_id, method), (_, actual_packet_id)) in INBOUND_SNAPSHOT_PACKET_SPECS
            .iter()
            .zip(resolved_entries.iter())
        {
            if actual_packet_id != expected_packet_id {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "generated high-frequency snapshot packet id mismatch for {}: manifest={}, generated={expected_packet_id}",
                    method.method_name(),
                    actual_packet_id,
                )));
            }
        }

        Ok(Self::default())
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
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        let mut resolved_entries = Vec::with_capacity(CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);
        let mut seen_packet_ids = HashSet::with_capacity(CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT);

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
            resolved_entries.push((entry.packet_id, family));
        }

        let by_packet_id = resolved_entries
            .try_into()
            .expect("custom-channel remote family registry length should stay fixed");
        Ok(Self { by_packet_id })
    }

    pub fn classify(&self, packet_id: u8) -> Option<CustomChannelRemoteFamily> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, family)| {
                (*known_packet_id == packet_id).then_some(*family)
            })
    }

    pub fn packet_id(&self, family: CustomChannelRemoteFamily) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, known_family)| (*known_family == family).then_some(*packet_id))
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

impl InboundRemotePacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = CustomChannelPacketRegistry::from_remote_manifest(manifest)?;
        let mut resolved_entries = Vec::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);
        let mut seen_packet_ids = HashSet::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);

        for family in InboundRemoteFamily::ordered() {
            let packet_id = registry.packet_id(family.custom_channel_family()).ok_or(
                RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "missing inbound remote family packet in manifest: {}",
                    family.method_name(),
                )),
            )?;
            if !seen_packet_ids.insert(packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate inbound remote family packet id: {packet_id}",
                )));
            }
            resolved_entries.push((packet_id, family));
        }

        let by_packet_id = resolved_entries
            .try_into()
            .expect("inbound remote family registry length should stay fixed");
        Ok(Self { by_packet_id })
    }

    pub fn classify(&self, packet_id: u8) -> Option<InboundRemoteFamily> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, family)| {
                (*known_packet_id == packet_id).then_some(*family)
            })
    }

    pub fn packet_id(&self, family: InboundRemoteFamily) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, known_family)| (*known_family == family).then_some(*packet_id))
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{CustomChannelPacketRegistry, InboundRemoteFamily, InboundRemotePacketRegistry};
    use mdt_remote::{
        read_remote_manifest, BasePacketEntry, CompressionFlagSpec, CustomChannelRemoteFamily,
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
