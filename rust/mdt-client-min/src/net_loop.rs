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
    use mdt_protocol::PacketCodecError;

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
}
