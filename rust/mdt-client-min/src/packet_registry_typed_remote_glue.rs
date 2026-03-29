use std::cell::OnceCell;

use mdt_remote::{
    CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, InboundRemoteRegistry, RemoteManifest,
    RemoteManifestError, RemotePacketRegistry, WellKnownRemoteRegistry,
};

#[derive(Debug, Clone)]
pub(super) struct PacketRegistryTypedRemoteGlue<'a> {
    remote_registry: RemotePacketRegistry<'a>,
    snapshot_packet_resolutions_cache: OnceCell<SnapshotPacketResolutions>,
    inbound_remote: OnceCell<InboundRemoteRegistry>,
    custom_channel: OnceCell<CustomChannelRemoteRegistry>,
    well_known: OnceCell<WellKnownRemoteRegistry>,
}

#[derive(Debug, Clone)]
struct SnapshotPacketResolutions {
    client_snapshot_packet_id: u8,
    inbound_snapshot_packet_specs: [(u8, HighFrequencyRemoteMethod); 4],
}

impl<'a> PacketRegistryTypedRemoteGlue<'a> {
    pub(super) fn from_remote_manifest(
        manifest: &'a RemoteManifest,
    ) -> Result<Self, RemoteManifestError> {
        let glue = Self {
            remote_registry: RemotePacketRegistry::from_manifest(manifest)?,
            snapshot_packet_resolutions_cache: OnceCell::new(),
            inbound_remote: OnceCell::new(),
            custom_channel: OnceCell::new(),
            well_known: OnceCell::new(),
        };
        glue.snapshot_packet_resolutions()?;
        Ok(glue)
    }

    pub(super) fn inbound_snapshot_packet_specs(
        &self,
    ) -> Result<[(u8, HighFrequencyRemoteMethod); 4], RemoteManifestError> {
        Ok(self
            .snapshot_packet_resolutions()?
            .inbound_snapshot_packet_specs)
    }

    pub(super) fn client_snapshot_packet_id(&self) -> Result<u8, RemoteManifestError> {
        Ok(self
            .snapshot_packet_resolutions()?
            .client_snapshot_packet_id)
    }

    pub(super) fn inbound_remote_registry(
        &self,
    ) -> Result<InboundRemoteRegistry, RemoteManifestError> {
        Ok(self.inbound_remote_registry_cached()?.clone())
    }

    pub(super) fn custom_channel_registry(
        &self,
    ) -> Result<CustomChannelRemoteRegistry, RemoteManifestError> {
        Ok(self.custom_channel_registry_cached()?.clone())
    }

    pub(super) fn well_known_registry(
        &self,
    ) -> Result<WellKnownRemoteRegistry, RemoteManifestError> {
        Ok(self.well_known_registry_cached()?.clone())
    }

    fn snapshot_packet_resolutions(
        &self,
    ) -> Result<&SnapshotPacketResolutions, RemoteManifestError> {
        if let Some(resolutions) = self.snapshot_packet_resolutions_cache.get() {
            return Ok(resolutions);
        }

        let mut inbound_snapshot_packet_specs = Vec::with_capacity(4);
        let mut seen_packet_ids = std::collections::HashSet::with_capacity(4);

        for method in [
            HighFrequencyRemoteMethod::StateSnapshot,
            HighFrequencyRemoteMethod::EntitySnapshot,
            HighFrequencyRemoteMethod::BlockSnapshot,
            HighFrequencyRemoteMethod::HiddenSnapshot,
        ] {
            let packet_id = self
                .remote_registry
                .first_high_frequency_method(method)
                .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                    method.method_name(),
                ))?
                .packet_id;
            if !seen_packet_ids.insert(packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate high-frequency server->client snapshot packet id: {packet_id}"
                )));
            }
            inbound_snapshot_packet_specs.push((packet_id, method));
        }

        let inbound_snapshot_packet_specs: [(u8, HighFrequencyRemoteMethod); 4] =
            inbound_snapshot_packet_specs.try_into().map_err(|_| {
                RemoteManifestError::InvalidPacketSequence(
                    "high-frequency server->client snapshot registry length drifted".into(),
                )
            })?;
        let client_snapshot_packet_id = self
            .remote_registry
            .first_high_frequency_method(HighFrequencyRemoteMethod::ClientSnapshot)
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                HighFrequencyRemoteMethod::ClientSnapshot.method_name(),
            ))?
            .packet_id;
        if let Some((_, method)) = inbound_snapshot_packet_specs
            .iter()
            .find(|(packet_id, _)| *packet_id == client_snapshot_packet_id)
        {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "client snapshot packet id collides with inbound snapshot packet id: {client_snapshot_packet_id} ({})",
                method.method_name()
            )));
        }

        let resolutions = SnapshotPacketResolutions {
            client_snapshot_packet_id,
            inbound_snapshot_packet_specs,
        };
        let _ = self.snapshot_packet_resolutions_cache.set(resolutions);
        Ok(self
            .snapshot_packet_resolutions_cache
            .get()
            .expect("snapshot packet resolutions cache should be initialized"))
    }

    fn inbound_remote_registry_cached(
        &self,
    ) -> Result<&InboundRemoteRegistry, RemoteManifestError> {
        if let Some(registry) = self.inbound_remote.get() {
            return Ok(registry);
        }

        let registry = InboundRemoteRegistry::from_remote_registry(&self.remote_registry)?;
        let _ = self.inbound_remote.set(registry);
        Ok(self
            .inbound_remote
            .get()
            .expect("inbound remote registry cache should be initialized"))
    }

    fn custom_channel_registry_cached(
        &self,
    ) -> Result<&CustomChannelRemoteRegistry, RemoteManifestError> {
        if let Some(registry) = self.custom_channel.get() {
            return Ok(registry);
        }

        let registry = CustomChannelRemoteRegistry::from_remote_registry(&self.remote_registry)?;
        let _ = self.custom_channel.set(registry);
        Ok(self
            .custom_channel
            .get()
            .expect("custom-channel registry cache should be initialized"))
    }

    fn well_known_registry_cached(&self) -> Result<&WellKnownRemoteRegistry, RemoteManifestError> {
        if let Some(registry) = self.well_known.get() {
            return Ok(registry);
        }

        let registry = WellKnownRemoteRegistry::from_remote_registry(&self.remote_registry)?;
        let _ = self.well_known.set(registry);
        Ok(self
            .well_known
            .get()
            .expect("well-known registry cache should be initialized"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::CombinedPacketRegistries;
    use super::PacketRegistryTypedRemoteGlue;
    use mdt_remote::{
        read_remote_manifest, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
        RemoteManifestError,
    };
    use std::path::PathBuf;

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn combined_packet_registries_reject_client_snapshot_packet_id_collision_with_inbound_snapshots(
    ) {
        let mut manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let state_snapshot_packet_id = HighFrequencyRemoteRegistry::from_manifest(&manifest)
            .unwrap()
            .packet_id(HighFrequencyRemoteMethod::StateSnapshot)
            .unwrap();
        let client_snapshot_entry = manifest
            .remote_packets
            .iter_mut()
            .find(|entry| entry.method == "clientSnapshot")
            .expect("missing clientSnapshot packet in fixture manifest");
        client_snapshot_entry.packet_id = state_snapshot_packet_id;

        let error = CombinedPacketRegistries::from_remote_manifest(&manifest).unwrap_err();
        match error {
            RemoteManifestError::InvalidPacketSequence(message) => {
                assert!(
                    message.contains(
                        "client snapshot packet id collides with inbound snapshot packet id"
                    ),
                    "unexpected packet-id collision error message: {message}"
                );
            }
            other => panic!("expected InvalidPacketSequence error, got {other:?}"),
        }

        let glue_error =
            PacketRegistryTypedRemoteGlue::from_remote_manifest(&manifest).unwrap_err();
        assert!(matches!(
            glue_error,
            RemoteManifestError::InvalidPacketSequence(_)
        ));
    }
}
