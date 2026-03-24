use mdt_remote::{
    RemoteFlow, RemoteManifest, RemoteManifestError, RemotePacketRegistry, RemotePacketSelector,
};
use std::collections::HashSet;

const INBOUND_REMOTE_FAMILY_COUNT: usize = 6;

const SERVER_PACKET_TEXT_PARAMS: [&str; 3] = ["Player", "java.lang.String", "java.lang.String"];
const SERVER_PACKET_BINARY_PARAMS: [&str; 3] = ["Player", "java.lang.String", "byte[]"];
const CLIENT_LOGIC_DATA_PARAMS: [&str; 3] = ["Player", "java.lang.String", "java.lang.Object"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundRemoteFamily {
    ServerPacketReliable,
    ServerPacketUnreliable,
    ServerBinaryPacketReliable,
    ServerBinaryPacketUnreliable,
    ClientLogicDataReliable,
    ClientLogicDataUnreliable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRemotePacketRegistry {
    by_packet_id: [(u8, InboundRemoteFamily); INBOUND_REMOTE_FAMILY_COUNT],
}

impl InboundRemoteFamily {
    pub fn ordered() -> [Self; INBOUND_REMOTE_FAMILY_COUNT] {
        [
            Self::ServerPacketReliable,
            Self::ServerPacketUnreliable,
            Self::ServerBinaryPacketReliable,
            Self::ServerBinaryPacketUnreliable,
            Self::ClientLogicDataReliable,
            Self::ClientLogicDataUnreliable,
        ]
    }

    pub fn method_name(self) -> &'static str {
        match self {
            Self::ServerPacketReliable => "serverPacketReliable",
            Self::ServerPacketUnreliable => "serverPacketUnreliable",
            Self::ServerBinaryPacketReliable => "serverBinaryPacketReliable",
            Self::ServerBinaryPacketUnreliable => "serverBinaryPacketUnreliable",
            Self::ClientLogicDataReliable => "clientLogicDataReliable",
            Self::ClientLogicDataUnreliable => "clientLogicDataUnreliable",
        }
    }

    pub fn unreliable(self) -> bool {
        matches!(
            self,
            Self::ServerPacketUnreliable
                | Self::ServerBinaryPacketUnreliable
                | Self::ClientLogicDataUnreliable
        )
    }

    fn selector_flow(self) -> RemoteFlow {
        // `RemotePacketRegistry` currently maps manifest `targets=client` to
        // `RemoteFlow::ClientToServer`, so these inbound-to-client helper families
        // must select that stored flow value rather than the intuitive wire direction.
        RemoteFlow::ClientToServer
    }

    pub fn param_java_types(self) -> &'static [&'static str] {
        match self {
            Self::ServerPacketReliable | Self::ServerPacketUnreliable => &SERVER_PACKET_TEXT_PARAMS,
            Self::ServerBinaryPacketReliable | Self::ServerBinaryPacketUnreliable => {
                &SERVER_PACKET_BINARY_PARAMS
            }
            Self::ClientLogicDataReliable | Self::ClientLogicDataUnreliable => {
                &CLIENT_LOGIC_DATA_PARAMS
            }
        }
    }
}

impl InboundRemotePacketRegistry {
    pub fn from_remote_manifest(manifest: &RemoteManifest) -> Result<Self, RemoteManifestError> {
        let registry = RemotePacketRegistry::from_manifest(manifest)?;
        let mut resolved_entries = Vec::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);
        let mut seen_packet_ids = HashSet::with_capacity(INBOUND_REMOTE_FAMILY_COUNT);

        for family in InboundRemoteFamily::ordered() {
            let entry = registry
                .first_matching(RemotePacketSelector {
                    method: family.method_name(),
                    flow: Some(family.selector_flow()),
                    unreliable: Some(family.unreliable()),
                    param_java_types: family.param_java_types(),
                })
                .ok_or(RemoteManifestError::InvalidRemotePacketMetadata(format!(
                    "missing inbound remote family packet in manifest: {}",
                    family.method_name(),
                )))?;
            if !seen_packet_ids.insert(entry.packet_id) {
                return Err(RemoteManifestError::InvalidPacketSequence(format!(
                    "duplicate inbound remote family packet id: {}",
                    entry.packet_id
                )));
            }
            resolved_entries.push((entry.packet_id, family));
        }

        let by_packet_id = resolved_entries
            .try_into()
            .expect("inbound remote family registry length should stay fixed");
        Ok(Self { by_packet_id })
    }

    pub fn classify(&self, packet_id: u8) -> Option<InboundRemoteFamily> {
        self.by_packet_id
            .iter()
            .find_map(|(known_packet_id, family)| {
                (*known_packet_id == packet_id).then_some(*family)
            })
    }

    pub fn packet_id(&self, family: InboundRemoteFamily) -> Option<u8> {
        self.by_packet_id
            .iter()
            .find_map(|(packet_id, known_family)| (*known_family == family).then_some(*packet_id))
    }

    pub fn len(&self) -> usize {
        self.by_packet_id.len()
    }
}
