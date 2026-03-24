#[path = "remote_lookup.rs"]
mod remote_lookup;

use crate::generated::remote_high_frequency_gen::{
    BLOCK_SNAPSHOT_PACKET_ID, ENTITY_SNAPSHOT_PACKET_ID, HIDDEN_SNAPSHOT_PACKET_ID,
    STATE_SNAPSHOT_PACKET_ID,
};
use crate::snapshot_ingest::InboundSnapshot;
use mdt_remote::{
    HighFrequencyRemoteMethod, RemoteFlow, RemoteManifest, RemoteManifestError,
    RemotePacketRegistry, RemotePacketSelector,
};
pub use remote_lookup::{InboundRemoteFamily, InboundRemotePacketRegistry};
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

#[cfg(test)]
mod tests {
    use super::{InboundRemoteFamily, InboundRemotePacketRegistry};
    use mdt_remote::read_remote_manifest;
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
    fn inbound_remote_family_registry_matches_prior_string_lookup_for_non_snapshot_family() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundRemotePacketRegistry::from_remote_manifest(&manifest).unwrap();
        let legacy_server_packet_reliable = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "serverPacketReliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let legacy_client_logic_data_unreliable = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientLogicDataUnreliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "java.lang.Object"
            })
            .map(|entry| entry.packet_id);

        assert_eq!(
            registry.packet_id(InboundRemoteFamily::ServerPacketReliable),
            legacy_server_packet_reliable
        );
        assert_eq!(
            registry.packet_id(InboundRemoteFamily::ClientLogicDataUnreliable),
            legacy_client_logic_data_unreliable
        );
    }
}
