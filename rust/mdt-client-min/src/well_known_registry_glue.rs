use super::WellKnownRemotePacketIds;
use mdt_remote::WellKnownRemoteRegistry;

pub(super) fn from_typed_registry(registry: WellKnownRemoteRegistry) -> WellKnownRemotePacketIds {
    WellKnownRemotePacketIds::from_resolved_packet_ids(registry.resolved_packet_ids())
}

#[cfg(test)]
pub(super) fn assert_typed_registry_roundtrip(
    typed_registry: &WellKnownRemoteRegistry,
    well_known: &WellKnownRemotePacketIds,
) {
    use mdt_remote::WellKnownRemoteMethod;

    assert_eq!(well_known.resolved_packet_ids(), typed_registry.resolved_packet_ids());

    let typed_fixed_table = typed_registry.packet_id_fixed_table();
    for method in WellKnownRemoteMethod::ordered() {
        assert_eq!(well_known.packet_id(method), typed_registry.packet_id(method));
    }
    for packet_id in 0..=u8::MAX {
        assert_eq!(well_known.method(packet_id), typed_fixed_table.get(packet_id));
        assert_eq!(
            well_known.contains_packet_id(packet_id),
            typed_fixed_table.contains_packet_id(packet_id)
        );
    }
}
