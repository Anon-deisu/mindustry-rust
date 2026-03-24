use mdt_remote::{
    typed_inbound_remote_dispatch_specs, InboundRemoteDispatchSpec, InboundRemoteFamily,
    RemoteManifest, RemoteManifestError, INBOUND_REMOTE_FAMILY_COUNT,
};

pub fn typed_inbound_remote_packet_specs(
    manifest: &RemoteManifest,
) -> Result<[(u8, InboundRemoteDispatchSpec); INBOUND_REMOTE_FAMILY_COUNT], RemoteManifestError> {
    let specs = typed_inbound_remote_dispatch_specs(manifest)?;

    for ((_, spec), expected_family) in specs.iter().zip(InboundRemoteFamily::ordered()) {
        if spec.family != expected_family {
            return Err(RemoteManifestError::InvalidPacketSequence(format!(
                "inbound remote dispatch family order drifted: expected {:?}, got {:?}",
                expected_family, spec.family
            )));
        }
    }

    Ok(specs)
}
