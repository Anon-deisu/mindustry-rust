pub mod arcnet_loop;
pub mod bootstrap_flow;
pub mod client_session;
pub mod connect_packet;
pub mod custom_packet_runtime;
pub mod custom_packet_runtime_bridge;
pub mod custom_packet_runtime_relay;
pub mod custom_packet_runtime_logic;
pub mod custom_packet_runtime_surface;
pub mod effect_data_runtime;
pub mod effect_runtime;
pub mod entity_snapshot_families;
pub mod event_summary;
pub mod generated;
pub mod net_loop;
pub mod packet_registry;
pub mod render_runtime;
pub mod rules_objectives_semantics;
pub mod runtime_custom_packet_business;
pub mod session_state;
pub mod snapshot_ingest;
pub mod state_snapshot_semantics;
pub mod typed_remote_dispatch;
pub mod udp_loop;

#[cfg(test)]
mod tests {
    use super::net_loop::{ingest_inbound_packet, NetLoopStats};
    use super::packet_registry::InboundSnapshotPacketRegistry;
    use super::session_state::SessionState;
    use mdt_protocol::encode_packet;
    use mdt_remote::{read_remote_manifest, HighFrequencyRemoteMethod, RemoteManifestError};
    use std::path::PathBuf;

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn builds_inbound_snapshot_registry_from_real_manifest() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();

        assert_eq!(registry.len(), 4);
        assert!(registry.contains_packet_id(11));
        assert!(registry.contains_packet_id(46));
        assert!(registry.contains_packet_id(49));
        assert!(registry.contains_packet_id(125));
        assert!(!registry.contains_packet_id(26));
    }

    #[test]
    fn rejects_duplicate_snapshot_packet_ids_in_registry_build() {
        let mut manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let entity_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "entitySnapshot")
            .expect("missing entitySnapshot packet in fixture manifest")
            .packet_id;
        let state_entry = manifest
            .remote_packets
            .iter_mut()
            .find(|entry| entry.method == "stateSnapshot")
            .expect("missing stateSnapshot packet in fixture manifest");
        state_entry.packet_id = entity_packet_id;

        let error = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap_err();
        match error {
            RemoteManifestError::InvalidPacketSequence(message) => {
                assert!(
                    message
                        .contains("duplicate high-frequency server-to-client snapshot packet id")
                        || message.contains("duplicate high-frequency remote packet id")
                        || message
                            .contains("duplicate high-frequency server->client snapshot packet id"),
                    "unexpected duplicate packet-id error message: {message}"
                );
            }
            other => panic!("expected InvalidPacketSequence error, got {other:?}"),
        }
    }

    #[test]
    fn classifies_real_snapshot_packet_ids() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let packet = registry.classify(125, &[1, 2, 3, 4]).unwrap();

        assert_eq!(packet.method, HighFrequencyRemoteMethod::StateSnapshot);
        assert_eq!(packet.packet_id, 125);
        assert_eq!(packet.payload, &[1, 2, 3, 4]);
        assert!(registry.classify(26, &[9, 9, 9]).is_none());
    }

    #[test]
    fn ingests_inbound_snapshot_packets_into_session_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let mut stats = NetLoopStats::default();
        let mut state = SessionState::default();

        let state_packet =
            ingest_inbound_packet(&mut stats, &mut state, &registry, 125, &[0, 1, 2]).unwrap();
        assert_eq!(
            state_packet.method,
            HighFrequencyRemoteMethod::StateSnapshot
        );

        let entity_packet =
            ingest_inbound_packet(&mut stats, &mut state, &registry, 46, &[3, 4]).unwrap();
        assert_eq!(
            entity_packet.method,
            HighFrequencyRemoteMethod::EntitySnapshot
        );

        assert_eq!(stats.packets_seen, 2);
        assert_eq!(stats.snapshot_packets_seen, 2);
        assert_eq!(state.received_snapshot_count, 2);
        assert_eq!(
            state.last_snapshot_method,
            Some(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(state.last_snapshot_packet_id, Some(46));
        assert_eq!(state.last_snapshot_payload_len, 2);
        assert!(state.seen_state_snapshot);
        assert!(state.seen_entity_snapshot);
        assert!(!state.seen_block_snapshot);
        assert!(!state.seen_hidden_snapshot);
        assert_eq!(state.failed_state_snapshot_parse_count, 1);
        assert_eq!(state.last_state_snapshot_parse_error_payload_len, Some(3));
        assert_eq!(
            state.last_state_snapshot_parse_error.as_deref(),
            Some("truncated_state_snapshot_payload")
        );
    }

    #[test]
    fn decodes_encoded_snapshot_packets_before_ingest() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let mut stats = NetLoopStats::default();
        let mut state = SessionState::default();
        let payload = vec![7u8; 64];
        let encoded = encode_packet(125, &payload, false).unwrap();

        let method = super::net_loop::ingest_inbound_packet_bytes(
            &mut stats, &mut state, &registry, &encoded,
        )
        .unwrap();

        assert_eq!(method, Some(HighFrequencyRemoteMethod::StateSnapshot));
        assert_eq!(stats.packets_seen, 1);
        assert_eq!(stats.snapshot_packets_seen, 1);
        assert_eq!(state.last_snapshot_payload_len, 64);
        assert!(state.seen_state_snapshot);
        assert_eq!(state.applied_state_snapshot_count, 0);
        assert_eq!(state.failed_state_snapshot_parse_count, 1);
        assert_eq!(state.last_state_snapshot_parse_error_payload_len, Some(64));
        assert_eq!(
            state.last_state_snapshot_parse_error.as_deref(),
            Some("truncated_state_snapshot_payload")
        );
    }

    #[test]
    fn ingests_block_and_hidden_snapshots_with_structured_observability() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();
        let block_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "blockSnapshot")
            .expect("missing blockSnapshot packet in fixture manifest")
            .packet_id;
        let hidden_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "hiddenSnapshot")
            .expect("missing hiddenSnapshot packet in fixture manifest")
            .packet_id;
        let mut stats = NetLoopStats::default();
        let mut state = SessionState::default();

        let block_packet = ingest_inbound_packet(
            &mut stats,
            &mut state,
            &registry,
            block_packet_id,
            &[
                0x00, 0x01, // amount
                0x00, 0x11, // data len
                0x00, 0x64, 0x00, 0x63, // build pos
                0x01, 0x2d, // block id
                0x3f, 0x80, 0x00, 0x00, // health = 1.0
                0x82, // rotation = 2 with version marker bit
                0x05, // team = 5
                0x03, // io version = 3
                0x01, // enabled = true
                0x08, // module bitmask
                0x80, // efficiency
                0x40, // optional efficiency
            ],
        )
        .unwrap();
        assert_eq!(
            block_packet.method,
            HighFrequencyRemoteMethod::BlockSnapshot
        );

        let hidden_packet = ingest_inbound_packet(
            &mut stats,
            &mut state,
            &registry,
            hidden_packet_id,
            &[
                0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x65, 0x00, 0x00,
                0x00, 0xCA,
            ],
        )
        .unwrap();
        assert_eq!(
            hidden_packet.method,
            HighFrequencyRemoteMethod::HiddenSnapshot
        );

        assert_eq!(stats.snapshot_packets_seen, 2);
        assert_eq!(state.received_snapshot_count, 2);
        assert!(state.seen_block_snapshot);
        assert!(state.seen_hidden_snapshot);
        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.last_block_snapshot_payload_len, Some(21));
        assert_eq!(state.applied_block_snapshot_count, 1);
        assert_eq!(
            state.last_block_snapshot.as_ref().map(|value| value.amount),
            Some(1)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .map(|value| value.data_len),
            Some(17)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .and_then(|value| value.first_build_pos),
            Some(0x0064_0063)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .and_then(|value| value.first_block_id),
            Some(301)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .and_then(|value| value.first_rotation),
            Some(2)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .and_then(|value| value.first_team_id),
            Some(5)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .and_then(|value| value.first_enabled),
            Some(true)
        );
        assert_eq!(
            state
                .last_block_snapshot
                .as_ref()
                .and_then(|value| value.first_efficiency),
            Some(0x80)
        );
        assert_eq!(state.failed_block_snapshot_parse_count, 0);
        assert_eq!(state.received_hidden_snapshot_count, 1);
        assert_eq!(state.last_hidden_snapshot_payload_len, Some(16));
        assert_eq!(state.applied_hidden_snapshot_count, 1);
        assert_eq!(
            state.last_hidden_snapshot.as_ref().map(|value| value.count),
            Some(3)
        );
        assert_eq!(
            state
                .last_hidden_snapshot
                .as_ref()
                .and_then(|value| value.first_id),
            Some(100)
        );
        assert_eq!(
            state
                .last_hidden_snapshot
                .as_ref()
                .map(|value| value.sample_ids.clone()),
            Some(vec![100, 101, 202])
        );
        assert_eq!(state.failed_hidden_snapshot_parse_count, 0);
        assert_eq!(
            state.last_snapshot_method,
            Some(HighFrequencyRemoteMethod::HiddenSnapshot)
        );
    }
}
