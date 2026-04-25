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

#[cfg(test)]
mod tests {
    use super::from_typed_registry;
    use mdt_remote::{read_remote_manifest, WellKnownRemoteMethod, WellKnownRemoteRegistry};
    use std::path::PathBuf;

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn from_typed_registry_roundtrips_real_well_known_registry_packet_ids() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let typed_registry = WellKnownRemoteRegistry::from_manifest(&manifest).unwrap();
        let well_known = from_typed_registry(typed_registry.clone());
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
}