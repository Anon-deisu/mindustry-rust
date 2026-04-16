use crate::packet_registry::InboundSnapshotPacketRegistry;
use crate::session_state::SessionState;
use crate::snapshot_ingest::{ingest_inbound_snapshot, InboundSnapshot};
use mdt_protocol::{decode_packet, PacketCodecError};
use mdt_remote::HighFrequencyRemoteMethod;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NetLoopStats {
    pub frames: u64,
    pub packets_seen: u64,
    pub snapshot_packets_seen: u64,
}

pub fn step(stats: &mut NetLoopStats) {
    stats.frames = stats.frames.saturating_add(1);
}

pub fn ingest_inbound_packet<'a>(
    stats: &mut NetLoopStats,
    state: &mut SessionState,
    registry: &InboundSnapshotPacketRegistry,
    packet_id: u8,
    payload: &'a [u8],
) -> Option<InboundSnapshot<'a>> {
    stats.packets_seen = stats.packets_seen.saturating_add(1);

    let packet = registry.classify(packet_id, payload)?;
    stats.snapshot_packets_seen = stats.snapshot_packets_seen.saturating_add(1);
    ingest_inbound_snapshot(state, packet);
    Some(packet)
}

pub fn ingest_inbound_packet_bytes(
    stats: &mut NetLoopStats,
    state: &mut SessionState,
    registry: &InboundSnapshotPacketRegistry,
    bytes: &[u8],
) -> Result<Option<HighFrequencyRemoteMethod>, PacketCodecError> {
    let packet = decode_packet(bytes)?;
    Ok(
        ingest_inbound_packet(stats, state, registry, packet.packet_id, &packet.payload)
            .map(|snapshot| snapshot.method),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdt_protocol::{encode_packet, PacketCodecError};
    use mdt_remote::read_remote_manifest;
    use std::path::PathBuf;

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn step_saturating_increments_frames_without_touching_other_stats() {
        let mut stats = NetLoopStats {
            frames: u64::MAX,
            packets_seen: 11,
            snapshot_packets_seen: 13,
        };

        step(&mut stats);

        assert_eq!(stats.frames, u64::MAX);
        assert_eq!(stats.packets_seen, 11);
        assert_eq!(stats.snapshot_packets_seen, 13);
    }

    #[test]
    fn ingest_inbound_packet_bytes_decode_failure_leaves_stats_unchanged() {
        let mut stats = NetLoopStats::default();
        stats.frames = 7;
        stats.packets_seen = 11;
        stats.snapshot_packets_seen = 13;

        let mut state = SessionState::default();
        let registry = InboundSnapshotPacketRegistry::default();
        let bytes = [0x2a, 0x00, 0x00, 0x63];

        let stats_before = stats;
        let state_before = state.clone();

        let result = ingest_inbound_packet_bytes(&mut stats, &mut state, &registry, &bytes);

        assert!(matches!(
            result,
            Err(PacketCodecError::UnsupportedCompression(0x63))
        ));
        assert_eq!(stats, stats_before);
        assert_eq!(state, state_before);
    }

    #[test]
    fn ingest_inbound_packet_bytes_unknown_packet_leaves_state_unchanged_and_counts_packet_only() {
        let mut stats = NetLoopStats::default();
        stats.frames = 3;
        stats.packets_seen = 5;
        stats.snapshot_packets_seen = 7;

        let mut state = SessionState {
            session_id: Some(42),
            last_applied_tick: 99,
            client_loaded: true,
            world_map_width: 128,
            world_map_height: 96,
            world_display_title: Some("unchanged".to_string()),
            ..SessionState::default()
        };
        let registry = InboundSnapshotPacketRegistry::default();
        let bytes = encode_packet(26, &[1, 2, 3, 4], false).unwrap();

        let stats_before = stats;
        let state_before = state.clone();

        let result = ingest_inbound_packet_bytes(&mut stats, &mut state, &registry, &bytes);

        assert!(matches!(result, Ok(None)));
        assert_eq!(stats.frames, stats_before.frames);
        assert_eq!(stats.packets_seen, stats_before.packets_seen + 1);
        assert_eq!(stats.snapshot_packets_seen, stats_before.snapshot_packets_seen);
        assert_eq!(state, state_before);
    }

    #[test]
    fn ingest_inbound_packet_counts_unknown_packet_without_touching_state() {
        let mut stats = NetLoopStats {
            frames: 3,
            packets_seen: 5,
            snapshot_packets_seen: 7,
        };
        let mut state = SessionState {
            session_id: Some(42),
            last_applied_tick: 99,
            client_loaded: true,
            world_map_width: 128,
            world_map_height: 96,
            world_display_title: Some("unchanged".to_string()),
            ..SessionState::default()
        };
        let registry = InboundSnapshotPacketRegistry::default();

        let stats_before = stats;
        let state_before = state.clone();

        let result = ingest_inbound_packet(&mut stats, &mut state, &registry, 26, &[]);

        assert_eq!(result, None);
        assert_eq!(stats.frames, stats_before.frames);
        assert_eq!(stats.packets_seen, stats_before.packets_seen + 1);
        assert_eq!(stats.snapshot_packets_seen, stats_before.snapshot_packets_seen);
        assert_eq!(state, state_before);
    }

    #[test]
    fn ingest_inbound_packet_bytes_classified_packet_advances_stats_and_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(&manifest).unwrap();

        let mut stats = NetLoopStats::default();
        stats.frames = 2;
        stats.packets_seen = 9;
        stats.snapshot_packets_seen = 11;

        let mut state = SessionState::default();
        let state_before = state.clone();
        let stats_before = stats;
        let bytes = encode_packet(125, &[1, 2, 3, 4], false).unwrap();

        let result = ingest_inbound_packet_bytes(&mut stats, &mut state, &registry, &bytes);

        assert_eq!(result.unwrap(), Some(HighFrequencyRemoteMethod::StateSnapshot));
        assert_eq!(stats.frames, stats_before.frames);
        assert_eq!(stats.packets_seen, stats_before.packets_seen + 1);
        assert_eq!(stats.snapshot_packets_seen, stats_before.snapshot_packets_seen + 1);
        assert_ne!(state, state_before);
        assert_eq!(state.received_snapshot_count, 1);
        assert_eq!(state.last_snapshot_packet_id, Some(125));
        assert_eq!(state.last_snapshot_method, Some(HighFrequencyRemoteMethod::StateSnapshot));
        assert_eq!(state.last_snapshot_payload_len, 4);
        assert!(state.seen_state_snapshot);
    }
}
