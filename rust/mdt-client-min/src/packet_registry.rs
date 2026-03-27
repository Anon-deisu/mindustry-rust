use crate::generated::remote_high_frequency_gen::{
    BLOCK_SNAPSHOT_PACKET_ID, ENTITY_SNAPSHOT_PACKET_ID, HIDDEN_SNAPSHOT_PACKET_ID,
    STATE_SNAPSHOT_PACKET_ID,
};
use crate::snapshot_ingest::InboundSnapshot;
#[path = "packet_registry_typed_remote_glue.rs"]
mod typed_remote_glue;
use mdt_remote::{
    CustomChannelRemoteDispatchSpec, CustomChannelRemoteFamily, CustomChannelRemotePayloadKind,
    CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, InboundRemoteDispatchSpec,
    InboundRemoteFamily, InboundRemoteRegistry, RemoteManifest, RemoteManifestError,
    RemotePacketIdFixedTable, WellKnownRemoteMethod, WellKnownRemoteRegistry,
    CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT, INBOUND_REMOTE_FAMILY_COUNT,
    WELL_KNOWN_REMOTE_METHOD_COUNT,
};
use typed_remote_glue::PacketRegistryTypedRemoteGlue;

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
    by_packet_id: RemotePacketIdFixedTable<HighFrequencyRemoteMethod>,
    resolved_packet_ids: [(u8, HighFrequencyRemoteMethod); 4],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellKnownRemotePacketIds {
    pub ping_packet_id: Option<u8>,
    pub client_plan_snapshot_packet_id: Option<u8>,
    pub client_plan_snapshot_received_packet_id: Option<u8>,
    pub ping_response_packet_id: Option<u8>,
    pub ping_location_packet_id: Option<u8>,
    pub debug_status_client_unreliable_packet_id: Option<u8>,
    pub trace_info_packet_id: Option<u8>,
    pub set_rules_packet_id: Option<u8>,
    pub set_objectives_packet_id: Option<u8>,
    pub set_rule_packet_id: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedPacketRegistries {
    pub inbound_snapshot: InboundSnapshotPacketRegistry,
    pub inbound_remote: InboundRemotePacketRegistry,
    pub custom_channel: CustomChannelPacketRegistry,
    pub client_snapshot_packet_id: u8,
    pub well_known_remote: WellKnownRemotePacketIds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePacketClassification {
    HighFrequency {
        method: HighFrequencyRemoteMethod,
    },
    CustomChannel {
        family: CustomChannelRemoteFamily,
        payload_kind: CustomChannelRemotePayloadKind,
    },
    InboundRemote {
        family: InboundRemoteFamily,
        payload_kind: CustomChannelRemotePayloadKind,
    },
    WellKnown {
        method: WellKnownRemoteMethod,
    },
}

impl Default for InboundSnapshotPacketRegistry {
    fn default() -> Self {
        Self {
            by_packet_id: RemotePacketIdFixedTable::from_entries(&INBOUND_SNAPSHOT_PACKET_SPECS),
            resolved_packet_ids: INBOUND_SNAPSHOT_PACKET_SPECS,
        }
    }
}

impl InboundSnapshotPacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let glue = PacketRegistryTypedRemoteGlue::from_remote_manifest(manifest)?;
        let resolved_packet_ids = glue.inbound_snapshot_packet_specs()?;
        Ok(Self {
            by_packet_id: RemotePacketIdFixedTable::from_entries(&resolved_packet_ids),
            resolved_packet_ids,
        })
    }

    pub fn classify<'a>(&self, packet_id: u8, payload: &'a [u8]) -> Option<InboundSnapshot<'a>> {
        self.by_packet_id
            .get(packet_id)
            .map(|method| InboundSnapshot::new(method, packet_id, payload))
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.by_packet_id.contains_packet_id(packet_id)
    }

    pub fn len(&self) -> usize {
        self.resolved_packet_ids.len()
    }

    pub fn method(&self, packet_id: u8) -> Option<HighFrequencyRemoteMethod> {
        self.by_packet_id.get(packet_id)
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

impl RemotePacketClassification {
    pub fn route_label(self) -> String {
        match self {
            Self::HighFrequency { method } => {
                format!("high_frequency/{}", method.method_name())
            }
            Self::CustomChannel {
                family,
                payload_kind,
            } => format!(
                "custom_channel/{}:{}",
                family.method_name(),
                payload_kind.label()
            ),
            Self::InboundRemote {
                family,
                payload_kind,
            } => format!(
                "inbound_remote/{}:{}",
                family.method_name(),
                payload_kind.label()
            ),
            Self::WellKnown { method } => format!("well_known/{}", method.method_name()),
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
        let glue = PacketRegistryTypedRemoteGlue::from_remote_manifest(manifest)?;
        let client_snapshot_packet_id = glue.client_snapshot_packet_id()?;
        let inbound_snapshot_packet_ids = glue.inbound_snapshot_packet_specs()?;
        let inbound_remote = glue.inbound_remote_registry()?;
        let custom_channel = glue.custom_channel_registry()?;
        let well_known = glue.well_known_registry()?;

        Ok(Self {
            inbound_snapshot: InboundSnapshotPacketRegistry {
                by_packet_id: RemotePacketIdFixedTable::from_entries(&inbound_snapshot_packet_ids),
                resolved_packet_ids: inbound_snapshot_packet_ids,
            },
            inbound_remote: InboundRemotePacketRegistry::from_typed_registry(inbound_remote),
            custom_channel: CustomChannelPacketRegistry::from_typed_registry(custom_channel),
            client_snapshot_packet_id,
            well_known_remote: WellKnownRemotePacketIds::from_typed_registry(well_known),
        })
    }

    pub fn classify_packet_id(&self, packet_id: u8) -> Option<RemotePacketClassification> {
        if packet_id == self.client_snapshot_packet_id {
            return Some(RemotePacketClassification::HighFrequency {
                method: HighFrequencyRemoteMethod::ClientSnapshot,
            });
        }
        if let Some(method) = self.inbound_snapshot.method(packet_id) {
            return Some(RemotePacketClassification::HighFrequency { method });
        }
        if let Some(spec) = self.custom_channel.dispatch_spec(packet_id) {
            if spec.payload_kind == CustomChannelRemotePayloadKind::LogicData {
                if let Some(family) = spec.family.inbound_remote_family() {
                    return Some(RemotePacketClassification::InboundRemote {
                        family,
                        payload_kind: spec.payload_kind,
                    });
                }
            }
            return Some(RemotePacketClassification::CustomChannel {
                family: spec.family,
                payload_kind: spec.payload_kind,
            });
        }
        if let Some(spec) = self.inbound_remote.dispatch_spec(packet_id) {
            return Some(RemotePacketClassification::InboundRemote {
                family: spec.family,
                payload_kind: spec.payload_kind,
            });
        }
        self.well_known_remote
            .method(packet_id)
            .map(|method| RemotePacketClassification::WellKnown { method })
    }
}

impl WellKnownRemotePacketIds {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let glue = PacketRegistryTypedRemoteGlue::from_remote_manifest(manifest)?;
        Ok(Self::from_typed_registry(glue.well_known_registry()?))
    }

    fn from_typed_registry(registry: WellKnownRemoteRegistry) -> Self {
        Self::from_resolved_packet_ids(registry.resolved_packet_ids())
    }

    fn from_resolved_packet_ids(
        resolved_packet_ids: [(WellKnownRemoteMethod, Option<u8>); WELL_KNOWN_REMOTE_METHOD_COUNT],
    ) -> Self {
        let packet_id = |method| {
            resolved_packet_ids
                .iter()
                .find_map(|(resolved_method, packet_id)| {
                    (*resolved_method == method).then_some(*packet_id)
                })
                .flatten()
        };

        Self {
            ping_packet_id: packet_id(WellKnownRemoteMethod::Ping),
            client_plan_snapshot_packet_id: packet_id(WellKnownRemoteMethod::ClientPlanSnapshot),
            client_plan_snapshot_received_packet_id: packet_id(
                WellKnownRemoteMethod::ClientPlanSnapshotReceived,
            ),
            ping_response_packet_id: packet_id(WellKnownRemoteMethod::PingResponse),
            ping_location_packet_id: packet_id(WellKnownRemoteMethod::PingLocation),
            debug_status_client_unreliable_packet_id: packet_id(
                WellKnownRemoteMethod::DebugStatusClientUnreliable,
            ),
            trace_info_packet_id: packet_id(WellKnownRemoteMethod::TraceInfo),
            set_rules_packet_id: packet_id(WellKnownRemoteMethod::SetRules),
            set_objectives_packet_id: packet_id(WellKnownRemoteMethod::SetObjectives),
            set_rule_packet_id: packet_id(WellKnownRemoteMethod::SetRule),
        }
    }

    pub fn packet_id(&self, method: WellKnownRemoteMethod) -> Option<u8> {
        match method {
            WellKnownRemoteMethod::Ping => self.ping_packet_id,
            WellKnownRemoteMethod::ClientPlanSnapshot => self.client_plan_snapshot_packet_id,
            WellKnownRemoteMethod::ClientPlanSnapshotReceived => {
                self.client_plan_snapshot_received_packet_id
            }
            WellKnownRemoteMethod::PingResponse => self.ping_response_packet_id,
            WellKnownRemoteMethod::PingLocation => self.ping_location_packet_id,
            WellKnownRemoteMethod::DebugStatusClientUnreliable => {
                self.debug_status_client_unreliable_packet_id
            }
            WellKnownRemoteMethod::TraceInfo => self.trace_info_packet_id,
            WellKnownRemoteMethod::SetRules => self.set_rules_packet_id,
            WellKnownRemoteMethod::SetObjectives => self.set_objectives_packet_id,
            WellKnownRemoteMethod::SetRule => self.set_rule_packet_id,
        }
    }

    pub fn contains_packet_id(&self, packet_id: u8) -> bool {
        self.method(packet_id).is_some()
    }

    pub fn resolved_packet_ids(
        &self,
    ) -> [(WellKnownRemoteMethod, Option<u8>); WELL_KNOWN_REMOTE_METHOD_COUNT] {
        WellKnownRemoteMethod::ordered().map(|method| (method, self.packet_id(method)))
    }

    pub fn method(&self, packet_id: u8) -> Option<WellKnownRemoteMethod> {
        self.resolved_packet_ids()
            .into_iter()
            .find_map(|(method, resolved_packet_id)| {
                (resolved_packet_id == Some(packet_id)).then_some(method)
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

#[cfg(test)]
mod tests {
    use super::{
        CombinedPacketRegistries, CustomChannelPacketRegistry, InboundRemoteDispatchSpec,
        InboundRemoteFamily, InboundRemotePacketRegistry, RemotePacketClassification,
        WellKnownRemotePacketIds,
    };
    use mdt_remote::{
        read_remote_manifest, BasePacketEntry, CompressionFlagSpec,
        CustomChannelRemoteDispatchSpec, CustomChannelRemoteFamily, CustomChannelRemotePayloadKind,
        CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
        InboundRemoteRegistry, RemoteGeneratorInfo, RemoteManifest, RemoteManifestError,
        RemotePacketEntry, RemoteParamEntry, WellKnownRemoteMethod, WellKnownRemoteRegistry,
        WireSpec,
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
    fn combined_packet_registries_classify_packet_ids_into_business_routes() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registries = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();
        let high_frequency_registry =
            HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();
        let well_known_registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();

        let state_snapshot_packet_id = high_frequency_registry
            .packet_id(HighFrequencyRemoteMethod::StateSnapshot)
            .unwrap();
        let server_packet_reliable_packet_id = registries
            .custom_channel
            .packet_id(CustomChannelRemoteFamily::ServerPacketReliable)
            .unwrap();
        let client_logic_data_packet_id = registries
            .inbound_remote
            .packet_id(InboundRemoteFamily::ClientLogicDataReliable)
            .unwrap();
        let set_rules_packet_id = well_known_registry
            .packet_id(WellKnownRemoteMethod::SetRules)
            .unwrap();

        assert_eq!(
            registries.classify_packet_id(state_snapshot_packet_id),
            Some(RemotePacketClassification::HighFrequency {
                method: HighFrequencyRemoteMethod::StateSnapshot,
            })
        );
        assert_eq!(
            registries.classify_packet_id(server_packet_reliable_packet_id),
            Some(RemotePacketClassification::CustomChannel {
                family: CustomChannelRemoteFamily::ServerPacketReliable,
                payload_kind: CustomChannelRemotePayloadKind::Text,
            })
        );
        assert_eq!(
            registries.classify_packet_id(client_logic_data_packet_id),
            Some(RemotePacketClassification::InboundRemote {
                family: InboundRemoteFamily::ClientLogicDataReliable,
                payload_kind: CustomChannelRemotePayloadKind::LogicData,
            })
        );
        assert_eq!(
            registries.classify_packet_id(set_rules_packet_id),
            Some(RemotePacketClassification::WellKnown {
                method: WellKnownRemoteMethod::SetRules,
            })
        );
        assert_eq!(
            registries
                .classify_packet_id(set_rules_packet_id)
                .unwrap()
                .route_label(),
            "well_known/setRules"
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
    fn standalone_inbound_snapshot_registry_matches_combined_view() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let standalone =
            super::InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let combined = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();

        assert_eq!(standalone, combined.inbound_snapshot);
    }

    #[test]
    fn combined_packet_registries_build_all_registry_views_from_one_lookup() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let combined = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();
        let remote_registry = HighFrequencyRemoteRegistry::from_manifest(&manifest).unwrap();
        let well_known_registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();

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
        assert_eq!(
            combined.well_known_remote.ping_packet_id,
            well_known_registry.packet_id(WellKnownRemoteMethod::Ping)
        );
        let expected = [
            (
                WellKnownRemoteMethod::Ping,
                combined.well_known_remote.ping_packet_id,
            ),
            (
                WellKnownRemoteMethod::ClientPlanSnapshot,
                combined.well_known_remote.client_plan_snapshot_packet_id,
            ),
            (
                WellKnownRemoteMethod::ClientPlanSnapshotReceived,
                combined
                    .well_known_remote
                    .client_plan_snapshot_received_packet_id,
            ),
            (
                WellKnownRemoteMethod::PingResponse,
                combined.well_known_remote.ping_response_packet_id,
            ),
            (
                WellKnownRemoteMethod::PingLocation,
                combined.well_known_remote.ping_location_packet_id,
            ),
            (
                WellKnownRemoteMethod::DebugStatusClientUnreliable,
                combined
                    .well_known_remote
                    .debug_status_client_unreliable_packet_id,
            ),
            (
                WellKnownRemoteMethod::TraceInfo,
                combined.well_known_remote.trace_info_packet_id,
            ),
            (
                WellKnownRemoteMethod::SetRules,
                combined.well_known_remote.set_rules_packet_id,
            ),
            (
                WellKnownRemoteMethod::SetObjectives,
                combined.well_known_remote.set_objectives_packet_id,
            ),
            (
                WellKnownRemoteMethod::SetRule,
                combined.well_known_remote.set_rule_packet_id,
            ),
        ];
        for (method, packet_id) in expected {
            assert_eq!(
                packet_id,
                well_known_registry.packet_id(method),
                "well-known packet id mismatch for {}",
                method.method_name()
            );
        }
    }

    #[test]
    fn combined_packet_registries_classify_packet_ids_by_priority_over_overlapping_registries() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let combined = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();

        let inbound_logic_data_packet_id = combined
            .inbound_remote
            .packet_id(InboundRemoteFamily::ClientLogicDataReliable)
            .unwrap();
        let custom_text_packet_id = combined
            .custom_channel
            .packet_id(CustomChannelRemoteFamily::ServerPacketReliable)
            .unwrap();

        let mut high_frequency_preferred = combined.clone();
        high_frequency_preferred.client_snapshot_packet_id = inbound_logic_data_packet_id;
        assert_eq!(
            high_frequency_preferred.classify_packet_id(inbound_logic_data_packet_id),
            Some(RemotePacketClassification::HighFrequency {
                method: HighFrequencyRemoteMethod::ClientSnapshot,
            })
        );

        let mut custom_preferred = combined.clone();
        custom_preferred.well_known_remote.ping_packet_id = Some(custom_text_packet_id);
        assert_eq!(
            custom_preferred.classify_packet_id(custom_text_packet_id),
            Some(RemotePacketClassification::CustomChannel {
                family: CustomChannelRemoteFamily::ServerPacketReliable,
                payload_kind: CustomChannelRemotePayloadKind::Text,
            })
        );

        let mut inbound_preferred = combined;
        inbound_preferred.well_known_remote.ping_packet_id = Some(inbound_logic_data_packet_id);
        assert_eq!(
            inbound_preferred.classify_packet_id(inbound_logic_data_packet_id),
            Some(RemotePacketClassification::InboundRemote {
                family: InboundRemoteFamily::ClientLogicDataReliable,
                payload_kind: CustomChannelRemotePayloadKind::LogicData,
            })
        );
    }

    #[test]
    fn generated_remote_registry_constants_match_manifest_and_combined_views() {
        use crate::generated::remote_high_frequency_gen::CLIENT_SNAPSHOT_PACKET_ID;
        use crate::generated::remote_registry_gen::{
            CLIENT_SNAPSHOT_CALL_PACKET_ID, PING_CALL_PACKET_ID, REMOTE_PACKET_SPECS,
            TILE_CONFIG_CALL_PACKET_ID, WORLD_DATA_BEGIN_CALL_PACKET_ID,
        };

        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let combined = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();

        let has_generated_spec = |packet_id, method| {
            REMOTE_PACKET_SPECS
                .iter()
                .any(|spec| spec.packet_id == packet_id && spec.method == method)
        };

        assert!(has_generated_spec(
            CLIENT_SNAPSHOT_CALL_PACKET_ID,
            "clientSnapshot"
        ));
        assert!(has_generated_spec(PING_CALL_PACKET_ID, "ping"));
        assert!(has_generated_spec(TILE_CONFIG_CALL_PACKET_ID, "tileConfig"));
        assert!(has_generated_spec(
            WORLD_DATA_BEGIN_CALL_PACKET_ID,
            "worldDataBegin"
        ));
        assert_eq!(
            combined.client_snapshot_packet_id,
            CLIENT_SNAPSHOT_PACKET_ID
        );
        assert_eq!(
            combined.well_known_remote.ping_packet_id,
            Some(PING_CALL_PACKET_ID)
        );
        assert_eq!(
            manifest
                .remote_packets
                .iter()
                .find(|entry| entry.method == "tileConfig")
                .map(|entry| entry.packet_id),
            Some(TILE_CONFIG_CALL_PACKET_ID)
        );
        assert_eq!(
            manifest
                .remote_packets
                .iter()
                .find(|entry| entry.method == "worldDataBegin")
                .map(|entry| entry.packet_id),
            Some(WORLD_DATA_BEGIN_CALL_PACKET_ID)
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

    #[test]
    fn well_known_remote_packet_ids_reject_method_name_decoys() {
        let manifest = well_known_remote_manifest_with_decoys();
        let well_known = WellKnownRemotePacketIds::from_remote_manifest(&manifest).unwrap();
        let typed_fixed_table = WellKnownRemoteRegistry::from_manifest(&manifest)
            .unwrap()
            .packet_id_fixed_table();

        assert_eq!(well_known.ping_packet_id, Some(5));
        assert_eq!(well_known.client_plan_snapshot_packet_id, Some(7));
        assert_eq!(well_known.client_plan_snapshot_received_packet_id, Some(8));
        assert_eq!(well_known.ping_response_packet_id, Some(10));
        assert_eq!(well_known.ping_location_packet_id, Some(11));
        assert_eq!(
            well_known.debug_status_client_unreliable_packet_id,
            Some(13)
        );
        assert_eq!(well_known.trace_info_packet_id, Some(15));
        assert_eq!(well_known.set_rules_packet_id, Some(16));
        assert_eq!(well_known.set_objectives_packet_id, Some(17));
        assert_eq!(well_known.set_rule_packet_id, Some(19));
        assert_eq!(
            well_known.method(5),
            Some(WellKnownRemoteMethod::Ping)
        );
        assert_eq!(well_known.method(6), None);
        assert!(well_known.contains_packet_id(5));
        assert!(!well_known.contains_packet_id(6));
        assert_eq!(well_known.method(5), typed_fixed_table.get(5));
        assert_eq!(well_known.method(6), typed_fixed_table.get(6));
        assert_eq!(
            well_known.contains_packet_id(5),
            typed_fixed_table.contains_packet_id(5)
        );
        assert_eq!(
            well_known.contains_packet_id(6),
            typed_fixed_table.contains_packet_id(6)
        );
    }

    #[test]
    fn well_known_remote_packet_ids_match_typed_registry() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let well_known = WellKnownRemotePacketIds::from_remote_manifest(&manifest).unwrap();
        let typed_registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();

        let expected = [
            (WellKnownRemoteMethod::Ping, well_known.ping_packet_id),
            (
                WellKnownRemoteMethod::ClientPlanSnapshot,
                well_known.client_plan_snapshot_packet_id,
            ),
            (
                WellKnownRemoteMethod::ClientPlanSnapshotReceived,
                well_known.client_plan_snapshot_received_packet_id,
            ),
            (
                WellKnownRemoteMethod::PingResponse,
                well_known.ping_response_packet_id,
            ),
            (
                WellKnownRemoteMethod::PingLocation,
                well_known.ping_location_packet_id,
            ),
            (
                WellKnownRemoteMethod::DebugStatusClientUnreliable,
                well_known.debug_status_client_unreliable_packet_id,
            ),
            (
                WellKnownRemoteMethod::TraceInfo,
                well_known.trace_info_packet_id,
            ),
            (
                WellKnownRemoteMethod::SetRules,
                well_known.set_rules_packet_id,
            ),
            (
                WellKnownRemoteMethod::SetObjectives,
                well_known.set_objectives_packet_id,
            ),
            (
                WellKnownRemoteMethod::SetRule,
                well_known.set_rule_packet_id,
            ),
        ];

        for (method, packet_id) in expected {
            assert_eq!(
                packet_id,
                typed_registry.packet_id(method),
                "typed well-known packet id mismatch for {}",
                method.method_name()
            );
        }
        assert_eq!(well_known.resolved_packet_ids(), typed_registry.resolved_packet_ids());
        let typed_fixed_table = typed_registry.packet_id_fixed_table();
        for packet_id in 0..=u8::MAX {
            assert_eq!(
                well_known.method(packet_id),
                typed_fixed_table.get(packet_id),
                "typed well-known classification mismatch for packet_id={packet_id}"
            );
            assert_eq!(
                well_known.contains_packet_id(packet_id),
                typed_fixed_table.contains_packet_id(packet_id),
                "typed well-known containment mismatch for packet_id={packet_id}"
            );
        }
    }

    #[test]
    fn standalone_well_known_remote_packet_ids_match_combined_view() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let standalone = WellKnownRemotePacketIds::from_remote_manifest(&manifest).unwrap();
        let combined = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap();

        assert_eq!(standalone, combined.well_known_remote);
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

    fn well_known_remote_manifest_with_decoys() -> RemoteManifest {
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
                    "mindustry.gen.PingDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "ping",
                    "client",
                    "none",
                    true,
                    vec![param("time", "long", true, true)],
                ),
                remote_packet(
                    1,
                    5,
                    "mindustry.gen.PingCallPacket",
                    "mindustry.core.NetClient",
                    "ping",
                    "client",
                    "none",
                    false,
                    vec![
                        param("player", "Player", false, false),
                        param("time", "long", true, true),
                    ],
                ),
                remote_packet(
                    2,
                    6,
                    "mindustry.gen.ClientPlanSnapshotDecoyCallPacket",
                    "mindustry.core.NetServer",
                    "clientPlanSnapshot",
                    "client",
                    "none",
                    true,
                    vec![
                        param("player", "Player", false, false),
                        param("groupId", "int", true, true),
                    ],
                ),
                remote_packet(
                    3,
                    7,
                    "mindustry.gen.ClientPlanSnapshotCallPacket",
                    "mindustry.core.NetServer",
                    "clientPlanSnapshot",
                    "client",
                    "none",
                    true,
                    vec![
                        param("player", "Player", false, false),
                        param("groupId", "int", true, true),
                        param(
                            "plans",
                            "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
                            true,
                            true,
                        ),
                    ],
                ),
                remote_packet(
                    4,
                    8,
                    "mindustry.gen.ClientPlanSnapshotReceivedCallPacket",
                    "mindustry.core.NetClient",
                    "clientPlanSnapshotReceived",
                    "server",
                    "none",
                    true,
                    vec![
                        param("player", "Player", true, true),
                        param("groupId", "int", true, true),
                        param(
                            "plans",
                            "arc.struct.Queue<mindustry.entities.units.BuildPlan>",
                            true,
                            true,
                        ),
                    ],
                ),
                remote_packet(
                    5,
                    9,
                    "mindustry.gen.PingResponseDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "pingResponse",
                    "server",
                    "none",
                    false,
                    vec![param("time", "int", true, true)],
                ),
                remote_packet(
                    6,
                    10,
                    "mindustry.gen.PingResponseCallPacket",
                    "mindustry.core.NetClient",
                    "pingResponse",
                    "server",
                    "none",
                    false,
                    vec![param("time", "long", true, true)],
                ),
                remote_packet(
                    7,
                    11,
                    "mindustry.gen.PingLocationCallPacket",
                    "mindustry.core.NetClient",
                    "pingLocation",
                    "both",
                    "server",
                    false,
                    vec![
                        param("player", "Player", false, true),
                        param("x", "float", true, true),
                        param("y", "float", true, true),
                        param("text", "java.lang.String", true, true),
                    ],
                ),
                remote_packet(
                    8,
                    12,
                    "mindustry.gen.DebugStatusClientUnreliableDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "debugStatusClientUnreliable",
                    "server",
                    "none",
                    false,
                    vec![
                        param("value", "int", true, true),
                        param("lastClientSnapshot", "int", true, true),
                        param("snapshotsSent", "int", true, true),
                    ],
                ),
                remote_packet(
                    9,
                    13,
                    "mindustry.gen.DebugStatusClientUnreliableCallPacket",
                    "mindustry.core.NetClient",
                    "debugStatusClientUnreliable",
                    "server",
                    "none",
                    true,
                    vec![
                        param("value", "int", true, true),
                        param("lastClientSnapshot", "int", true, true),
                        param("snapshotsSent", "int", true, true),
                    ],
                ),
                remote_packet(
                    10,
                    14,
                    "mindustry.gen.TraceInfoDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "traceInfo",
                    "server",
                    "none",
                    false,
                    vec![param(
                        "info",
                        "mindustry.net.Administration.TraceInfo",
                        true,
                        true,
                    )],
                ),
                remote_packet(
                    11,
                    15,
                    "mindustry.gen.TraceInfoCallPacket",
                    "mindustry.core.NetClient",
                    "traceInfo",
                    "server",
                    "none",
                    false,
                    vec![
                        param("player", "Player", true, true),
                        param("info", "mindustry.net.Administration.TraceInfo", true, true),
                    ],
                ),
                remote_packet(
                    12,
                    16,
                    "mindustry.gen.SetRulesCallPacket",
                    "mindustry.core.NetClient",
                    "setRules",
                    "server",
                    "none",
                    false,
                    vec![param("rules", "mindustry.game.Rules", true, true)],
                ),
                remote_packet(
                    13,
                    17,
                    "mindustry.gen.SetObjectivesCallPacket",
                    "mindustry.core.NetClient",
                    "setObjectives",
                    "server",
                    "none",
                    false,
                    vec![param(
                        "executor",
                        "mindustry.game.MapObjectives",
                        true,
                        true,
                    )],
                ),
                remote_packet(
                    14,
                    18,
                    "mindustry.gen.SetRuleDecoyCallPacket",
                    "mindustry.core.NetClient",
                    "setRule",
                    "server",
                    "none",
                    false,
                    vec![param("rule", "java.lang.String", true, true)],
                ),
                remote_packet(
                    15,
                    19,
                    "mindustry.gen.SetRuleCallPacket",
                    "mindustry.core.NetClient",
                    "setRule",
                    "server",
                    "none",
                    false,
                    vec![
                        param("rule", "java.lang.String", true, true),
                        param("jsonData", "java.lang.String", true, true),
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
