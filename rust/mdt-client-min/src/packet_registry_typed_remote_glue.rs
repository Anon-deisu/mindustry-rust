use mdt_remote::{
    CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
    InboundRemoteRegistry, RemoteManifest, RemoteManifestError, RemotePacketRegistry,
    WellKnownRemoteRegistry,
};

#[derive(Debug, Clone)]
pub(super) struct PacketRegistryTypedRemoteGlue<'a> {
    remote_registry: RemotePacketRegistry<'a>,
}

impl<'a> PacketRegistryTypedRemoteGlue<'a> {
    pub(super) fn from_remote_manifest(
        manifest: &'a RemoteManifest,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            remote_registry: RemotePacketRegistry::from_manifest(manifest)?,
        })
    }

    pub(super) fn inbound_snapshot_packet_specs(
        &self,
    ) -> Result<[(u8, HighFrequencyRemoteMethod); 4], RemoteManifestError> {
        let high_frequency = HighFrequencyRemoteRegistry::from_remote_registry(&self.remote_registry)?;
        let mut resolved = Vec::with_capacity(4);
        let mut seen_packet_ids = std::collections::HashSet::with_capacity(4);

        for method in [
            HighFrequencyRemoteMethod::StateSnapshot,
            HighFrequencyRemoteMethod::EntitySnapshot,
            HighFrequencyRemoteMethod::BlockSnapshot,
            HighFrequencyRemoteMethod::HiddenSnapshot,
        ] {
            let packet_id = high_frequency.packet_id(method).ok_or(
                RemoteManifestError::MissingHighFrequencyPacket(method.method_name()),
            )?;
            if !seen_packet_ids.insert(packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate high-frequency server->client snapshot packet id: {packet_id}"
                )));
            }
            resolved.push((packet_id, method));
        }

        resolved.try_into().map_err(|_| {
            RemoteManifestError::InvalidPacketSequence(
                "high-frequency server->client snapshot registry length drifted".into(),
            )
        })
    }

    pub(super) fn client_snapshot_packet_id(&self) -> Result<u8, RemoteManifestError> {
        HighFrequencyRemoteRegistry::from_remote_registry(&self.remote_registry)?
            .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                HighFrequencyRemoteMethod::ClientSnapshot.method_name(),
            ))
    }

    pub(super) fn inbound_remote_registry(&self) -> Result<InboundRemoteRegistry, RemoteManifestError> {
        InboundRemoteRegistry::from_remote_registry(&self.remote_registry)
    }

    pub(super) fn custom_channel_registry(
        &self,
    ) -> Result<CustomChannelRemoteRegistry, RemoteManifestError> {
        CustomChannelRemoteRegistry::from_remote_registry(&self.remote_registry)
    }

    pub(super) fn well_known_registry(&self) -> Result<WellKnownRemoteRegistry, RemoteManifestError> {
        WellKnownRemoteRegistry::from_remote_registry(&self.remote_registry)
    }
}
