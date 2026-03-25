use crate::generated::remote_high_frequency_gen::{
    BLOCK_SNAPSHOT_PACKET_ID, ENTITY_SNAPSHOT_PACKET_ID, HIDDEN_SNAPSHOT_PACKET_ID,
    STATE_SNAPSHOT_PACKET_ID,
};
use crate::snapshot_ingest::InboundSnapshot;
use mdt_remote::{
    CustomChannelRemoteDispatchSpec, CustomChannelRemoteFamily, CustomChannelRemoteRegistry,
    HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry, InboundRemoteDispatchSpec,
    InboundRemoteFamily, InboundRemoteRegistry, RemoteManifest, RemoteManifestError,
    RemotePacketIdFixedTable, TypedRemoteRegistries, CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT,
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
struct TypedRemoteLookup {
    high_frequency: HighFrequencyRemoteRegistry,
    inbound_remote: InboundRemoteRegistry,
    custom_channel: CustomChannelRemoteRegistry,
}

impl TypedRemoteLookup {
    fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registries = TypedRemoteRegistries::from_manifest(manifest)?;
        Ok(Self {
            high_frequency: registries.high_frequency,
            inbound_remote: registries.inbound_remote,
            custom_channel: registries.custom_channel,
        })
    }

    fn inbound_snapshot_packet_specs(
        &self,
    ) -> Result<[(u8, HighFrequencyRemoteMethod); 4], RemoteManifestError> {
        let mut resolved = Vec::with_capacity(4);
        let mut seen_packet_ids = std::collections::HashSet::with_capacity(4);

        for method in [
            HighFrequencyRemoteMethod::StateSnapshot,
            HighFrequencyRemoteMethod::EntitySnapshot,
            HighFrequencyRemoteMethod::BlockSnapshot,
            HighFrequencyRemoteMethod::HiddenSnapshot,
        ] {
            let packet_id = self.high_frequency.packet_id(method).ok_or(
                RemoteManifestError::MissingHighFrequencyPacket(method.method_name()),
            )?;
            if !seen_packet_ids.insert(packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate high-frequency server->client snapshot packet id: {packet_id}"
                )));
            }
            resolved.push((packet_id, method));
        }

        resolved.try_into().map_err(|_| {
            RemoteManifestError::InvalidPacketSequence(
                "high-frequency server->client snapshot registry length drifted".into(),
            )
        })
    }

    fn client_snapshot_packet_id(&self) -> Result<u8, RemoteManifestError> {
        self.high_frequency
            .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                HighFrequencyRemoteMethod::ClientSnapshot.method_name(),
            ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundSnapshotPacketRegistry {
    by_packet_id: [(u8, HighFrequencyRemoteMethod); 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRemotePacketRegistry {
    by_packet_id: RemotePacketIdFixedTable<InboundRemoteDispatchSpec>,
    resolved_specs: [(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomChannelPacketRegistry {
    by_packet_id: RemotePacketIdFixedTable<CustomChannelRemoteDispatchSpec>,
    resolved_specs: [(u8, CustomChannelRemoteDispatchSpec); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedPacketRegistries {
    pub inbound_snapshot: InboundSnapshotPacketRegistry,
    pub inbound_remote: InboundRemotePacketRegistry,
    pub custom_channel: CustomChannelPacketRegistry,
    pub client_snapshot_packet_id: u8,
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
        let high_frequency = HighFrequencyRemoteRegistry::from_manifest(manifest)?;
        Ok(Self {
            by_packet_id: inbound_snapshot_packet_specs_from_registry(&high_frequency)?,
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
        let registry = CustomChannelRemoteRegistry::from_manifest(manifest)?;
        Ok(Self::from_typed_registry(registry))
    }

    pub fn classify(&self, packet_id: u8) -> Option<CustomChannelRemoteFamily> {
        self.dispatch_spec(packet_id).map(|spec| spec.family)
    }

    pub fn classify_inbound(&self, packet_id: u8) -> Option<InboundRemoteFamily> {
        self.classify(packet_id)
            .and_then(CustomChannelRemoteFamily::inbound_remote_family)
    }

    pub fn dispatch_spec(&self, packet_id: u8) -> Option<CustomChannelRemoteDispatchSpec> {
        self.by_packet_id.get(packet_id)
    }

    pub fn packet_id(&self, family: CustomChannelRemoteFamily) -> Option<u8> {
        self.resolved_specs
            .iter()
            .find_map(|(packet_id, spec)| (spec.family == family).then_some(*packet_id))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id.contains_packet_id(packet_id)
    }

    pub fn len(&self) -> usize {
        self.resolved_specs.len()
    }

    fn from_typed_registry(registry: CustomChannelRemoteRegistry) -> Self {
        let resolved_specs = registry.resolved_dispatch_specs();
        Self {
            by_packet_id: registry.packet_id_fixed_table(),
            resolved_specs,
        }
    }
}

impl InboundRemotePacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = InboundRemoteRegistry::from_manifest(manifest)?;
        Ok(Self::from_typed_registry(registry))
    }
}

impl CombinedPacketRegistries {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let lookup = TypedRemoteLookup::from_remote_manifest(manifest)?;
        let client_snapshot_packet_id = lookup.client_snapshot_packet_id()?;

        Ok(Self {
            inbound_snapshot: InboundSnapshotPacketRegistry {
                by_packet_id: lookup.inbound_snapshot_packet_specs()?,
            },
            inbound_remote: InboundRemotePacketRegistry::from_typed_registry(lookup.inbound_remote),
            custom_channel: CustomChannelPacketRegistry::from_typed_registry(lookup.custom_channel),
            client_snapshot_packet_id,
        })
    }
}

impl InboundRemotePacketRegistry {
    pub fn classify(&self, packet_id: u8) -> Option<InboundRemoteFamily> {
        self.dispatch_spec(packet_id).map(|spec| spec.family)
    }

    pub fn packet_id(&self, family: InboundRemoteFamily) -> Option<u8> {
        self.resolved_specs
            .iter()
            .find_map(|(packet_id, spec)| (spec.family == family).then_some(*packet_id))
    }

    pub fn dispatch_spec(&self, packet_id: u8) -> Option<InboundRemoteDispatchSpec> {
        self.by_packet_id.get(packet_id)
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id.contains_packet_id(packet_id)
    }

    pub fn len(&self) -> usize {
        self.resolved_specs.len()
    }

    fn from_typed_registry(registry: InboundRemoteRegistry) -> Self {
        let resolved_specs = registry.resolved_dispatch_specs();
        Self {
            by_packet_id: registry.packet_id_fixed_table(),
            resolved_specs,
        }
    }
}

fn inbound_snapshot_packet_specs_from_registry(
    high_frequency: &HighFrequencyRemoteRegistry,
) -> Result<[(u8, HighFrequencyRemoteMethod); 4], RemoteManifestError> {
    let mut resolved = Vec::with_capacity(4);
    let mut seen_packet_ids = std::collections::HashSet::with_capacity(4);

    for method in [
        HighFrequencyRemoteMethod::StateSnapshot,
        HighFrequencyRemoteMethod::EntitySnapshot,
        HighFrequencyRemoteMethod::BlockSnapshot,
        HighFrequencyRemoteMethod::HiddenSnapshot,
    ] {
        let packet_id = high_frequency.packet_id(method).ok_or(
            RemoteManifestError::MissingHighFrequencyPacket(method.method_name()),
        )?;
        if !seen_packet_ids.insert(packet_id) {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "duplicate high-frequency server->client snapshot packet id: {packet_id}"
            )));
        }
        resolved.push((packet_id, method));
    }

    resolved.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "high-frequency server->client snapshot registry length drifted".into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CombinedPacketRegistries, CustomChannelPacketRegistry, InboundRemoteDispatchSpec,
        InboundRemoteFamily, InboundRemotePacketRegistry,
    };
    use mdt_remote::{
        read_remote_manifest, BasePacketEntry, CompressionFlagSpec,
        CustomChannelRemoteDispatchSpec, CustomChannelRemoteFamily, CustomChannelRemotePayloadKind,
        CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
        InboundRemoteRegistry, RemoteGeneratorInfo, RemoteManifest, RemoteManifestError,
        RemotePacketEntry, RemoteParamEntry, WireSpec,
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
        let client_snapshot_packet_id = HighFrequencyRemoteRegistry::from_manifest(&manifest)
            .unwrap()
            .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
            .unwrap();

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
        assert!(!registry.contains_packet_id(client_snapshot_packet_id));
        assert_eq!(
            registry.classify_inbound(
                registry
                    .packet_id(CustomChannelRemoteFamily::ServerPacketReliable)
                    .unwrap()
            ),
            Some(InboundRemoteFamily::ServerPacketReliable)
        );
        assert_eq!(
            registry.classify_inbound(
                registry
                    .packet_id(CustomChannelRemoteFamily::ClientPacketReliable)
                    .unwrap()
            ),
            None
        );
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
        assert_eq!(
            registry.dispatch_spec(14),
            Some(CustomChannelRemoteDispatchSpec {
                family: CustomChannelRemoteFamily::ClientLogicDataReliable,
                payload_kind: CustomChannelRemotePayloadKind::LogicData,
            })
        );
    }

    #[test]
    fn custom_channel_remote_family_registry_matches_remote_typed_dispatch_specs() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = CustomChannelPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let remote_registry = CustomChannelRemoteRegistry::from_manifest(&manifest).unwrap();

        let packet_id = remote_registry
            .packet_id(CustomChannelRemoteFamily::ServerBinaryPacketReliable)
            .unwrap();
        assert_eq!(
            registry.packet_id(CustomChannelRemoteFamily::ServerBinaryPacketReliable),
            Some(packet_id)
        );
        assert_eq!(
            registry.dispatch_spec(packet_id),
            remote_registry.dispatch_spec(packet_id)
        );
        assert_eq!(
            registry.dispatch_spec(remote_registry.resolved_dispatch_specs()[8].0),
            Some(remote_registry.resolved_dispatch_specs()[8].1)
        );
    }

    #[test]
    fn inbound_remote_family_registry_reuses_custom_channel_typed_lookup() {
        let manifest = custom_channel_remote_family_manifest_with_decoys();
        let registry = InboundRemotePacketRegistry::from_remote_manifest(&manifest).unwrap();
        let custom_registry = CustomChannelPacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(
            registry.packet_id(InboundRemoteFamily::ServerPacketReliable),
            Some(10)
        );
        assert_eq!(registry.classify(9), None);
        assert_eq!(
            registry.classify(10),
            Some(InboundRemoteFamily::ServerPacketReliable)
        );
        assert_eq!(custom_registry.classify_inbound(10), registry.classify(10));
        assert_eq!(custom_registry.classify_inbound(5), None);
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
        let remote_registry = InboundRemoteRegistry::from_manifest(&manifest).unwrap();

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
            registry.dispatch_spec(remote_registry.resolved_dispatch_specs()[4].0),
            Some(remote_registry.resolved_dispatch_specs()[4].1)
        );
    }

    #[test]
    fn inbound_snapshot_registry_reuses_high_frequency_typed_registry_packet_ids() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry =
            super::InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let remote_registry = HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();
        let state_packet_id = remote_registry
            .packet_id(HighFrequencyRemoteMethod::StateSnapshot)
            .unwrap();
        let entity_packet_id = remote_registry
            .packet_id(HighFrequencyRemoteMethod::EntitySnapshot)
            .unwrap();

        assert_eq!(
            registry
                .classify(state_packet_id, &[1])
                .map(|packet| packet.method),
            Some(HighFrequencyRemoteMethod::StateSnapshot)
        );
        assert_eq!(
            registry
                .classify(entity_packet_id, &[2])
                .map(|packet| packet.method),
            Some(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            remote_registry.packet_id(HighFrequencyRemoteMethod::StateSnapshot),
            Some(state_packet_id)
        );
        assert_eq!(
            remote_registry.packet_id(HighFrequencyRemoteMethod::EntitySnapshot),
            Some(entity_packet_id)
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

    #[test]
    fn combined_packet_registries_build_all_registry_views_from_one_lookup() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let combined = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();
        let remote_registry = HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();

        assert_eq!(
            combined.client_snapshot_packet_id,
            remote_registry
                .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
                .unwrap()
        );
        assert_eq!(combined.inbound_snapshot.len(), 4);
        assert_eq!(combined.inbound_remote.len(), 6);
        assert_eq!(combined.custom_channel.len(), 10);
        assert_eq!(
            combined
                .inbound_remote
                .packet_id(InboundRemoteFamily::ServerPacketReliable),
            Some(
                combined
                    .custom_channel
                    .packet_id(CustomChannelRemoteFamily::ServerPacketReliable)
                    .unwrap()
            )
        );
    }

    #[test]
    fn combined_packet_registries_require_snapshot_and_client_snapshot_entries() {
        let manifest = custom_channel_remote_family_manifest_with_decoys();
        let error = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap_err();

        assert!(matches!(
            error,
            RemoteManifestError::MissingHighFrequencyPacket("clientSnapshot")
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
}
