use crate::generated::remote_high_frequency_gen::{
    BLOCK_SNAPSHOT_PACKET_ID, ENTITY_SNAPSHOT_PACKET_ID, HIDDEN_SNAPSHOT_PACKET_ID,
    STATE_SNAPSHOT_PACKET_ID,
};
use mdt_remote::{
    HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry, RemoteManifest, RemoteManifestError,
};

const INBOUND_SNAPSHOT_METHODS: [(u8, HighFrequencyRemoteMethod); 4] = [
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

pub fn typed_inbound_snapshot_packet_specs(
    manifest: &RemoteManifest,
) -> Result<[(u8, HighFrequencyRemoteMethod); 4], RemoteManifestError> {
    let registry = HighFrequencyRemoteRegistry::from_manifest(manifest)?;
    let mut resolved_entries = Vec::with_capacity(INBOUND_SNAPSHOT_METHODS.len());

    for (expected_packet_id, method) in INBOUND_SNAPSHOT_METHODS {
        let packet_id =
            registry
                .packet_id(method)
                .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                    method.method_name(),
                ))?;
        if packet_id != expected_packet_id {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "generated high-frequency snapshot packet id mismatch for {}: manifest={}, generated={expected_packet_id}",
                method.method_name(),
                packet_id,
            )));
        }
        resolved_entries.push((packet_id, method));
    }

    resolved_entries.try_into().map_err(|_| {
        RemoteManifestError::InvalidPacketSequence(
            "inbound snapshot registry length drifted".into(),
        )
    })
}
