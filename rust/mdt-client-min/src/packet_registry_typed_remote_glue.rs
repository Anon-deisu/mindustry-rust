use mdt_remote::{
    CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
    InboundRemoteRegistry, RemoteManifest, RemoteManifestError, TypedRemoteRegistries,
    WellKnownRemoteRegistry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PacketRegistryTypedRemoteGlue {
    pub(super) high_frequency: HighFrequencyRemoteRegistry,
    pub(super) inbound_remote: InboundRemoteRegistry,
    pub(super) custom_channel: CustomChannelRemoteRegistry,
    pub(super) well_known: WellKnownRemoteRegistry,
}

impl PacketRegistryTypedRemoteGlue {
    pub(super) fn from_remote_manifest(
        manifest: &RemoteManifest,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self::from_typed_registries(
            TypedRemoteRegistries::from_manifest(manifest)?,
        ))
    }

    pub(super) fn from_typed_registries(registries: TypedRemoteRegistries) -> Self {
        Self {
            high_frequency: registries.high_frequency,
            inbound_remote: registries.inbound_remote,
            custom_channel: registries.custom_channel,
            well_known: registries.well_known,
        }
    }

    pub(super) fn inbound_snapshot_packet_specs(
        &self,
    ) -> Result<[(u8, HighFrequencyRemoteMethod); 4], RemoteManifestError> {
        let mut resolved = Vec::with_capacity(4);
        let mut seen_packet_ids = std::collections::HashSet::with_capacity(4);

        for method in [
            HighFrequencyRemoteMethod::StateSnapshot,
            HighFrequencyRemoteMethod::EntitySnapshot,
            HighFrequencyRemoteMethod::BlockSnapshot,
            HighFrequencyRemoteMethod::HiddenSnapshot,
        ] {
            let packet_id = self.high_frequency.packet_id(method).ok_or(
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
        self.high_frequency
            .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                HighFrequencyRemoteMethod::ClientSnapshot.method_name(),
            ))
    }
}
