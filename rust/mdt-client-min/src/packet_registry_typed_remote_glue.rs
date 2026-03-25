use std::cell::OnceCell;

use mdt_remote::{
    CustomChannelRemoteRegistry, HighFrequencyRemoteMethod, HighFrequencyRemoteRegistry,
    InboundRemoteRegistry, RemoteManifest, RemoteManifestError, RemotePacketRegistry,
    WellKnownRemoteRegistry,
};

#[derive(Debug, Clone)]
pub(super) struct PacketRegistryTypedRemoteGlue<'a> {
    remote_registry: RemotePacketRegistry<'a>,
    high_frequency: OnceCell<HighFrequencyRemoteRegistry>,
    inbound_remote: OnceCell<InboundRemoteRegistry>,
    custom_channel: OnceCell<CustomChannelRemoteRegistry>,
    well_known: OnceCell<WellKnownRemoteRegistry>,
}

impl<'a> PacketRegistryTypedRemoteGlue<'a> {
    pub(super) fn from_remote_manifest(
        manifest: &'a RemoteManifest,
    ) -> Result<Self, RemoteManifestError> {
        Ok(Self {
            remote_registry: RemotePacketRegistry::from_manifest(manifest)?,
            high_frequency: OnceCell::new(),
            inbound_remote: OnceCell::new(),
            custom_channel: OnceCell::new(),
            well_known: OnceCell::new(),
        })
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
            let packet_id = self.high_frequency_registry()?.packet_id(method).ok_or(
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
        self.high_frequency_registry()?
            .packet_id(HighFrequencyRemoteMethod::ClientSnapshot)
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                HighFrequencyRemoteMethod::ClientSnapshot.method_name(),
            ))
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

    fn high_frequency_registry(&self) -> Result<&HighFrequencyRemoteRegistry, RemoteManifestError> {
        if let Some(registry) = self.high_frequency.get() {
            return Ok(registry);
        }

        let registry = HighFrequencyRemoteRegistry::from_remote_registry(&self.remote_registry)?;
        let _ = self.high_frequency.set(registry);
        Ok(self
            .high_frequency
            .get()
            .expect("high-frequency registry cache should be initialized"))
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
