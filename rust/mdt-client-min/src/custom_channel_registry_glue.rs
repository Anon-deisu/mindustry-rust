use mdt_remote::{
    typed_custom_channel_remote_dispatch_specs, CustomChannelRemoteDispatchSpec,
    CustomChannelRemoteFamily, RemoteManifest, RemoteManifestError,
    CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT,
};

pub fn typed_custom_channel_remote_packet_specs(
    manifest: &RemoteManifest,
) -> Result<
    [(u8, CustomChannelRemoteDispatchSpec); CUSTOM_CHANNEL_REMOTE_FAMILY_COUNT],
    RemoteManifestError,
> {
    let specs = typed_custom_channel_remote_dispatch_specs(manifest)?;

    for ((_, spec), expected_family) in specs.iter().zip(CustomChannelRemoteFamily::ordered()) {
        if spec.family != expected_family {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "custom-channel remote dispatch family order drifted: expected {:?}, got {:?}",
                expected_family, spec.family
            )));
        }
    }

    Ok(specs)
}
