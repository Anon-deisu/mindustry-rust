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
