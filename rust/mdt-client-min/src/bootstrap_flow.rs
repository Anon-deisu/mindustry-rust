use crate::session_state::{
    EntityPlayerSemanticProjection, SessionState, WorldBootstrapProjection,
};
use mdt_protocol::{
    decode_packet, encode_packet, split_stream_chunks, stream_begin_payload, stream_chunk_payload,
    PacketCodecError, CONNECT_PACKET_ID, STREAM_BEGIN_PACKET_ID, STREAM_CHUNK_PACKET_ID,
    WORLD_STREAM_PACKET_ID,
};
use mdt_world::{parse_world_bundle, LoadedWorldBootstrap, WorldBundle};
use std::fmt;

const MAX_WORLD_STREAM_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectPacketEnvelope {
    pub payload: Vec<u8>,
    pub encoded_packet: Vec<u8>,
}

impl ConnectPacketEnvelope {
    pub fn from_payload(payload: &[u8]) -> Result<Self, BootstrapFlowError> {
        let encoded_packet = encode_packet(CONNECT_PACKET_ID, payload, false)?;
        Ok(Self {
            payload: payload.to_vec(),
            encoded_packet,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoginBootstrap {
    pub connect: ConnectPacketEnvelope,
    pub stream_id: i32,
    pub world_bundle: WorldBundle,
    pub bootstrap: LoadedWorldBootstrap,
}

impl LoginBootstrap {
    pub fn from_stream_packets(
        connect_payload: &[u8],
        begin_packet: &[u8],
        chunk_packets: &[Vec<u8>],
        locale: &str,
    ) -> Result<Self, BootstrapFlowError> {
        let connect = ConnectPacketEnvelope::from_payload(connect_payload)?;
        let mut assembler = WorldStreamAssembler::from_stream_begin_packet(begin_packet)?;
        for chunk_packet in chunk_packets {
            assembler.push_stream_chunk_packet(chunk_packet)?;
        }
        let stream_id = assembler.stream_id;
        let compressed_world_stream = assembler.finish()?;
        let world_bundle = parse_world_bundle(&compressed_world_stream)
            .map_err(BootstrapFlowError::WorldBundleParse)?;
        let bootstrap = world_bundle
            .loaded_session()
            .map_err(BootstrapFlowError::WorldBundleParse)?
            .bootstrap(locale);

        Ok(Self {
            connect,
            stream_id,
            world_bundle,
            bootstrap,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldStreamAssembler {
    pub stream_id: i32,
    pub total_bytes: usize,
    pub kind: u8,
    received: Vec<u8>,
}

impl WorldStreamAssembler {
    pub fn from_stream_begin_packet(bytes: &[u8]) -> Result<Self, BootstrapFlowError> {
        let packet = decode_packet(bytes)?;
        if packet.packet_id != STREAM_BEGIN_PACKET_ID {
            return Err(BootstrapFlowError::UnexpectedPacketId {
                expected: STREAM_BEGIN_PACKET_ID,
                actual: packet.packet_id,
            });
        }
        if packet.payload.len() < 9 {
            return Err(BootstrapFlowError::TruncatedPayload {
                context: "stream begin",
                expected_at_least: 9,
                actual: packet.payload.len(),
            });
        }

        let stream_id = read_i32_be(&packet.payload[0..4]);
        let total_bytes_i32 = read_i32_be(&packet.payload[4..8]);
        let total_bytes = usize::try_from(total_bytes_i32)
            .map_err(|_| BootstrapFlowError::InvalidStreamLength(total_bytes_i32))?;
        if total_bytes > MAX_WORLD_STREAM_BYTES {
            return Err(BootstrapFlowError::StreamLengthLimitExceeded {
                actual: total_bytes,
                max: MAX_WORLD_STREAM_BYTES,
            });
        }
        let kind = packet.payload[8];

        if kind != WORLD_STREAM_PACKET_ID {
            return Err(BootstrapFlowError::InvalidWorldStreamKind(kind));
        }

        Ok(Self {
            stream_id,
            total_bytes,
            kind,
            received: Vec::with_capacity(total_bytes),
        })
    }

    pub fn push_stream_chunk_packet(&mut self, bytes: &[u8]) -> Result<bool, BootstrapFlowError> {
        let packet = decode_packet(bytes)?;
        if packet.packet_id != STREAM_CHUNK_PACKET_ID {
            return Err(BootstrapFlowError::UnexpectedPacketId {
                expected: STREAM_CHUNK_PACKET_ID,
                actual: packet.packet_id,
            });
        }
        if packet.payload.len() < 6 {
            return Err(BootstrapFlowError::TruncatedPayload {
                context: "stream chunk",
                expected_at_least: 6,
                actual: packet.payload.len(),
            });
        }

        let stream_id = read_i32_be(&packet.payload[0..4]);
        if stream_id != self.stream_id {
            return Err(BootstrapFlowError::StreamIdMismatch {
                expected: self.stream_id,
                actual: stream_id,
            });
        }

        let declared_len = read_u16_be(&packet.payload[4..6]) as usize;
        let chunk = &packet.payload[6..];
        if declared_len != chunk.len() {
            return Err(BootstrapFlowError::ChunkLengthMismatch {
                declared: declared_len,
                actual: chunk.len(),
            });
        }

        let next_len = self.received.len().saturating_add(chunk.len());
        if next_len > self.total_bytes {
            return Err(BootstrapFlowError::StreamOverflow {
                expected: self.total_bytes,
                actual: next_len,
            });
        }

        self.received.extend_from_slice(chunk);
        Ok(self.is_complete())
    }

    pub fn is_complete(&self) -> bool {
        self.received.len() == self.total_bytes
    }

    pub fn compressed_world_stream(&self) -> &[u8] {
        &self.received
    }

    pub fn finish(self) -> Result<Vec<u8>, BootstrapFlowError> {
        if !self.is_complete() {
            return Err(BootstrapFlowError::IncompleteWorldStream {
                expected: self.total_bytes,
                actual: self.received.len(),
            });
        }
        Ok(self.received)
    }
}

pub fn encode_world_stream_packets(
    compressed_world_stream: &[u8],
    stream_id: i32,
    chunk_size: usize,
) -> Result<(Vec<u8>, Vec<Vec<u8>>), BootstrapFlowError> {
    if chunk_size == 0 {
        return Err(BootstrapFlowError::InvalidChunkSize(chunk_size));
    }

    let total_bytes = i32::try_from(compressed_world_stream.len())
        .map_err(|_| BootstrapFlowError::InvalidStreamLength(i32::MAX))?;
    let begin_packet = encode_packet(
        STREAM_BEGIN_PACKET_ID,
        &stream_begin_payload(stream_id, total_bytes, WORLD_STREAM_PACKET_ID),
        false,
    )?;

    let chunk_packets = split_stream_chunks(compressed_world_stream, chunk_size)
        .into_iter()
        .map(|chunk| {
            let payload = stream_chunk_payload(stream_id, &chunk)?;
            encode_packet(STREAM_CHUNK_PACKET_ID, &payload, true)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((begin_packet, chunk_packets))
}

pub fn apply_login_bootstrap(state: &mut SessionState, bootstrap: &LoginBootstrap) {
    apply_connect_packet(state, &bootstrap.connect);
    apply_world_bootstrap(state, bootstrap.stream_id, &bootstrap.bootstrap);
}

pub fn apply_connect_packet(state: &mut SessionState, connect: &ConnectPacketEnvelope) {
    state.connect_packet_sent = true;
    state.connect_payload_len = connect.payload.len();
    state.connect_packet_len = connect.encoded_packet.len();
}

fn reset_finish_connecting_lifecycle(state: &mut SessionState) {
    state.client_loaded = false;
    state.connect_confirm_sent = false;
    state.connect_confirm_flushed = false;
    state.last_connect_confirm_at_ms = None;
    state.last_connect_confirm_flushed_at_ms = None;
    state.finish_connecting_commit_count = 0;
    state.last_finish_connecting = None;
    state.last_ready_inbound_liveness_anchor_at_ms = None;
    state.ready_inbound_liveness_anchor_count = 0;
}

pub fn apply_world_bootstrap(
    state: &mut SessionState,
    stream_id: i32,
    bootstrap: &LoadedWorldBootstrap,
) {
    reset_finish_connecting_lifecycle(state);
    state.bootstrap_stream_id = Some(stream_id);
    state.world_stream_expected_len = bootstrap.compressed_length;
    state.world_stream_received_len = bootstrap.compressed_length;
    state.world_stream_loaded = true;
    state.world_stream_compressed_len = bootstrap.compressed_length;
    state.world_stream_inflated_len = bootstrap.inflated_length;
    state.world_map_width = bootstrap.map_width;
    state.world_map_height = bootstrap.map_height;
    state.world_player_id = Some(bootstrap.player_id);
    state.world_player_semantic_projection = Some(EntityPlayerSemanticProjection {
        admin: bootstrap.player_admin,
        boosting: bootstrap.player_boosting,
        color_rgba: bootstrap.player_color_rgba,
        mouse_x_bits: bootstrap.mouse_x_bits,
        mouse_y_bits: bootstrap.mouse_y_bits,
        name: Some(bootstrap.player_name.clone()),
        selected_block_id: bootstrap.selected_block_id,
        selected_rotation: bootstrap.selected_rotation,
        shooting: bootstrap.player_shooting,
        team_id: bootstrap.player_team_id,
        typing: bootstrap.player_typing,
    });
    state.world_player_unit_kind = Some(bootstrap.player_unit_kind);
    state.world_player_unit_value = Some(bootstrap.player_unit_value);
    let player_position_bits = sanitize_bootstrap_player_position_bits(bootstrap);
    state.world_player_x_bits = player_position_bits.map(|(x_bits, _)| x_bits);
    state.world_player_y_bits = player_position_bits.map(|(_, y_bits)| y_bits);
    state.world_display_title = bootstrap.display_title.clone();
    state.world_bootstrap_projection = Some(WorldBootstrapProjection {
        rules_sha256: bootstrap.rules_sha256.clone(),
        map_locales_sha256: bootstrap.map_locales_sha256.clone(),
        tags_sha256: bootstrap.tags_sha256.clone(),
        team_count: bootstrap.team_count,
        marker_count: bootstrap.marker_count,
        custom_chunk_count: bootstrap.custom_chunk_count,
        content_patch_count: bootstrap.content_patch_count,
        player_team_plan_count: bootstrap.player_team_plan_count,
        static_fog_team_count: bootstrap.static_fog_team_count,
    });
    state.ready_to_enter_world = bootstrap.ready_to_enter_world;
}

fn sanitize_bootstrap_player_position_bits(
    bootstrap: &LoadedWorldBootstrap,
) -> Option<(u32, u32)> {
    let x = f32::from_bits(bootstrap.player_x_bits);
    let y = f32::from_bits(bootstrap.player_y_bits);
    (x.is_finite() && y.is_finite()).then_some((bootstrap.player_x_bits, bootstrap.player_y_bits))
}

#[derive(Debug)]
pub enum BootstrapFlowError {
    PacketCodec(PacketCodecError),
    UnexpectedPacketId {
        expected: u8,
        actual: u8,
    },
    TruncatedPayload {
        context: &'static str,
        expected_at_least: usize,
        actual: usize,
    },
    InvalidStreamLength(i32),
    InvalidChunkSize(usize),
    StreamLengthLimitExceeded {
        actual: usize,
        max: usize,
    },
    InvalidWorldStreamKind(u8),
    StreamIdMismatch {
        expected: i32,
        actual: i32,
    },
    ChunkLengthMismatch {
        declared: usize,
        actual: usize,
    },
    StreamOverflow {
        expected: usize,
        actual: usize,
    },
    IncompleteWorldStream {
        expected: usize,
        actual: usize,
    },
    WorldBundleParse(String),
}

impl fmt::Display for BootstrapFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketCodec(error) => write!(f, "{error}"),
            Self::UnexpectedPacketId { expected, actual } => {
                write!(f, "unexpected packet id: expected {expected}, got {actual}")
            }
            Self::TruncatedPayload {
                context,
                expected_at_least,
                actual,
            } => write!(
                f,
                "{context} payload too short: expected at least {expected_at_least} bytes, got {actual}"
            ),
            Self::InvalidStreamLength(length) => {
                write!(f, "invalid stream length in begin packet: {length}")
            }
            Self::InvalidChunkSize(chunk_size) => {
                write!(f, "invalid world stream chunk size: {chunk_size}")
            }
            Self::StreamLengthLimitExceeded { actual, max } => {
                write!(f, "stream length {actual} exceeds hard limit {max}")
            }
            Self::InvalidWorldStreamKind(kind) => {
                write!(f, "unexpected stream begin kind: {kind}")
            }
            Self::StreamIdMismatch { expected, actual } => {
                write!(f, "stream chunk id mismatch: expected {expected}, got {actual}")
            }
            Self::ChunkLengthMismatch { declared, actual } => write!(
                f,
                "stream chunk length mismatch: declared {declared}, actual {actual}"
            ),
            Self::StreamOverflow { expected, actual } => write!(
                f,
                "stream chunk overflow: expected at most {expected} bytes, got {actual}"
            ),
            Self::IncompleteWorldStream { expected, actual } => write!(
                f,
                "world stream incomplete: expected {expected} bytes, got {actual}"
            ),
            Self::WorldBundleParse(error) => write!(f, "failed to parse world bundle: {error}"),
        }
    }
}

impl std::error::Error for BootstrapFlowError {}

impl From<PacketCodecError> for BootstrapFlowError {
    fn from(value: PacketCodecError) -> Self {
        Self::PacketCodec(value)
    }
}

fn read_i32_be(bytes: &[u8]) -> i32 {
    i32::from_be_bytes(bytes.try_into().expect("slice length already checked"))
}

fn read_u16_be(bytes: &[u8]) -> u16 {
    u16::from_be_bytes(bytes.try_into().expect("slice length already checked"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_state::FinishConnectingProjection;
    use mdt_protocol::{decode_packet, CONNECT_PACKET_ID};

    fn decode_hex_text(text: &str) -> Vec<u8> {
        let cleaned = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
            .collect()
    }

    fn sample_connect_payload() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../tests/src/test/resources/connect-packet.hex"
        ))
    }

    fn sample_world_stream_bytes() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        ))
    }

    #[test]
    fn encodes_real_connect_packet_payload_as_wire_packet() {
        let payload = sample_connect_payload();
        let envelope = ConnectPacketEnvelope::from_payload(&payload).unwrap();
        let decoded = decode_packet(&envelope.encoded_packet).unwrap();

        assert_eq!(decoded.packet_id, CONNECT_PACKET_ID);
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.raw_length as usize, payload.len());
    }

    #[test]
    fn assembles_real_world_stream_into_login_bootstrap() {
        let connect_payload = sample_connect_payload();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        let login = LoginBootstrap::from_stream_packets(
            &connect_payload,
            &begin_packet,
            &chunk_packets,
            "fr",
        )
        .unwrap();

        assert_eq!(login.stream_id, 7);
        assert_eq!(login.bootstrap.map_width, 8);
        assert_eq!(login.bootstrap.map_height, 8);
        assert_eq!(login.bootstrap.player_id, 7);
        assert_eq!(
            login.bootstrap.display_title.as_deref(),
            Some("Golden Deterministic")
        );
        assert!(login.bootstrap.ready_to_enter_world);
        assert_eq!(
            login.bootstrap.compressed_length,
            compressed_world_stream.len()
        );
        assert_eq!(login.world_bundle.compressed, compressed_world_stream);
    }

    #[test]
    fn applies_login_bootstrap_to_session_state() {
        let connect_payload = sample_connect_payload();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let login = LoginBootstrap::from_stream_packets(
            &connect_payload,
            &begin_packet,
            &chunk_packets,
            "fr",
        )
        .unwrap();
        let mut state = SessionState::default();

        apply_login_bootstrap(&mut state, &login);

        assert!(state.connect_packet_sent);
        assert_eq!(state.connect_payload_len, connect_payload.len());
        assert!(state.connect_packet_len > 0);
        assert_eq!(state.bootstrap_stream_id, Some(7));
        assert!(state.world_stream_loaded);
        assert_eq!(state.world_map_width, 8);
        assert_eq!(state.world_map_height, 8);
        assert_eq!(state.world_player_id, Some(7));
        assert_eq!(
            state.world_player_semantic_projection,
            Some(EntityPlayerSemanticProjection {
                admin: login.bootstrap.player_admin,
                boosting: login.bootstrap.player_boosting,
                color_rgba: login.bootstrap.player_color_rgba,
                mouse_x_bits: login.bootstrap.mouse_x_bits,
                mouse_y_bits: login.bootstrap.mouse_y_bits,
                name: Some(login.bootstrap.player_name.clone()),
                selected_block_id: login.bootstrap.selected_block_id,
                selected_rotation: login.bootstrap.selected_rotation,
                shooting: login.bootstrap.player_shooting,
                team_id: login.bootstrap.player_team_id,
                typing: login.bootstrap.player_typing,
            })
        );
        assert_eq!(
            state.world_player_unit_kind,
            Some(login.bootstrap.player_unit_kind)
        );
        assert_eq!(
            state.world_player_unit_value,
            Some(login.bootstrap.player_unit_value)
        );
        assert_eq!(
            state.world_player_x_bits,
            Some(login.bootstrap.player_x_bits)
        );
        assert_eq!(
            state.world_player_y_bits,
            Some(login.bootstrap.player_y_bits)
        );
        assert_eq!(
            state.world_display_title.as_deref(),
            Some("Golden Deterministic")
        );
        assert_eq!(
            state.world_bootstrap_projection,
            Some(WorldBootstrapProjection {
                rules_sha256: login.bootstrap.rules_sha256.clone(),
                map_locales_sha256: login.bootstrap.map_locales_sha256.clone(),
                tags_sha256: login.bootstrap.tags_sha256.clone(),
                team_count: login.bootstrap.team_count,
                marker_count: login.bootstrap.marker_count,
                custom_chunk_count: login.bootstrap.custom_chunk_count,
                content_patch_count: login.bootstrap.content_patch_count,
                player_team_plan_count: login.bootstrap.player_team_plan_count,
                static_fog_team_count: login.bootstrap.static_fog_team_count,
            })
        );
        assert!(state.ready_to_enter_world);
    }

    #[test]
    fn applies_login_bootstrap_skips_non_finite_player_coordinates() {
        let connect_payload = sample_connect_payload();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let mut login = LoginBootstrap::from_stream_packets(
            &connect_payload,
            &begin_packet,
            &chunk_packets,
            "fr",
        )
        .unwrap();
        login.bootstrap.player_x_bits = f32::NAN.to_bits();
        login.bootstrap.player_y_bits = f32::INFINITY.to_bits();
        let mut state = SessionState::default();

        apply_login_bootstrap(&mut state, &login);

        assert_eq!(state.world_player_x_bits, None);
        assert_eq!(state.world_player_y_bits, None);
    }

    #[test]
    fn reset_finish_connecting_lifecycle_clears_finish_connecting_state() {
        let mut state = SessionState::default();
        state.client_loaded = true;
        state.connect_confirm_sent = true;
        state.connect_confirm_flushed = true;
        state.last_connect_confirm_at_ms = Some(11);
        state.last_connect_confirm_flushed_at_ms = Some(12);
        state.finish_connecting_commit_count = 3;
        state.last_finish_connecting = Some(FinishConnectingProjection {
            committed_at_ms: 10,
            replayed_loading_packet_count: 2,
            total_replayed_loading_packet_count: 4,
            ready_to_enter_world: true,
            client_loaded: true,
            connect_confirm_queued: true,
            connect_confirm_flushed: true,
            snapshot_watchdog_armed_at_ms: Some(10),
        });
        state.last_ready_inbound_liveness_anchor_at_ms = Some(13);
        state.ready_inbound_liveness_anchor_count = 5;

        reset_finish_connecting_lifecycle(&mut state);

        assert!(!state.client_loaded);
        assert!(!state.connect_confirm_sent);
        assert!(!state.connect_confirm_flushed);
        assert_eq!(state.last_connect_confirm_at_ms, None);
        assert_eq!(state.last_connect_confirm_flushed_at_ms, None);
        assert_eq!(state.finish_connecting_commit_count, 0);
        assert_eq!(state.last_finish_connecting, None);
        assert_eq!(state.last_ready_inbound_liveness_anchor_at_ms, None);
        assert_eq!(state.ready_inbound_liveness_anchor_count, 0);
    }

    #[test]
    fn rejects_stream_begin_packet_over_hard_limit() {
        let begin_packet = encode_packet(
            STREAM_BEGIN_PACKET_ID,
            &stream_begin_payload(
                7,
                i32::try_from(MAX_WORLD_STREAM_BYTES + 1).unwrap(),
                WORLD_STREAM_PACKET_ID,
            ),
            false,
        )
        .unwrap();

        let error = WorldStreamAssembler::from_stream_begin_packet(&begin_packet).unwrap_err();

        assert!(matches!(
            error,
            BootstrapFlowError::StreamLengthLimitExceeded {
                actual,
                max,
            } if actual == MAX_WORLD_STREAM_BYTES + 1 && max == MAX_WORLD_STREAM_BYTES
        ));
    }

    #[test]
    fn rejects_zero_chunk_size_when_encoding_world_stream_packets() {
        let error = encode_world_stream_packets(&[1, 2, 3], 7, 0).unwrap_err();

        assert!(matches!(error, BootstrapFlowError::InvalidChunkSize(0)));
    }

    #[test]
    fn rejects_stream_chunk_overflow_against_begin_total() {
        let begin_packet = encode_packet(
            STREAM_BEGIN_PACKET_ID,
            &stream_begin_payload(7, 3, WORLD_STREAM_PACKET_ID),
            false,
        )
        .unwrap();
        let mut assembler = WorldStreamAssembler::from_stream_begin_packet(&begin_packet).unwrap();
        let overflow_chunk = encode_packet(
            STREAM_CHUNK_PACKET_ID,
            &stream_chunk_payload(7, &[1, 2, 3, 4]).unwrap(),
            true,
        )
        .unwrap();

        let error = assembler
            .push_stream_chunk_packet(&overflow_chunk)
            .unwrap_err();

        assert!(matches!(
            error,
            BootstrapFlowError::StreamOverflow {
                expected: 3,
                actual: 4,
            }
        ));
    }

    #[test]
    fn second_world_load_overwrites_previous_world_stream_state() {
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet_first, chunk_packets_first) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let (begin_packet_second, chunk_packets_second) =
            encode_world_stream_packets(&compressed_world_stream, 99, 257).unwrap();
        let login_first = LoginBootstrap::from_stream_packets(
            &[0x11, 0x22, 0x33, 0x44],
            &begin_packet_first,
            &chunk_packets_first,
            "fr",
        )
        .unwrap();
        let login_second = LoginBootstrap::from_stream_packets(
            &[0x55, 0x66],
            &begin_packet_second,
            &chunk_packets_second,
            "fr",
        )
        .unwrap();

        let mut state = SessionState::default();
        apply_login_bootstrap(&mut state, &login_first);
        state.world_stream_expected_len = 1;
        state.world_stream_received_len = 0;
        state.world_stream_loaded = false;
        state.ready_to_enter_world = false;
        state.bootstrap_stream_id = Some(-1);

        apply_login_bootstrap(&mut state, &login_second);

        assert_eq!(state.bootstrap_stream_id, Some(99));
        assert_eq!(state.connect_payload_len, 2);
        assert_eq!(
            state.world_stream_expected_len,
            login_second.bootstrap.compressed_length
        );
        assert_eq!(
            state.world_stream_received_len,
            login_second.bootstrap.compressed_length
        );
        assert!(state.world_stream_loaded);
        assert_eq!(state.world_map_width, login_second.bootstrap.map_width);
        assert_eq!(state.world_map_height, login_second.bootstrap.map_height);
        assert_eq!(
            state.world_player_id,
            Some(login_second.bootstrap.player_id)
        );
        assert!(state.ready_to_enter_world);
    }

    #[test]
    fn second_world_load_clears_loaded_lifecycle_markers_before_finish_connecting() {
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet_first, chunk_packets_first) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let (begin_packet_second, chunk_packets_second) =
            encode_world_stream_packets(&compressed_world_stream, 99, 1024).unwrap();
        let login_first = LoginBootstrap::from_stream_packets(
            &[0x11, 0x22, 0x33, 0x44],
            &begin_packet_first,
            &chunk_packets_first,
            "fr",
        )
        .unwrap();
        let login_second = LoginBootstrap::from_stream_packets(
            &[0x55, 0x66],
            &begin_packet_second,
            &chunk_packets_second,
            "fr",
        )
        .unwrap();

        let mut state = SessionState::default();
        apply_login_bootstrap(&mut state, &login_first);
        state.client_loaded = true;
        state.connect_confirm_sent = true;
        state.connect_confirm_flushed = true;
        state.last_connect_confirm_at_ms = Some(11);
        state.last_connect_confirm_flushed_at_ms = Some(12);
        state.finish_connecting_commit_count = 3;
        state.last_finish_connecting = Some(FinishConnectingProjection {
            committed_at_ms: 10,
            replayed_loading_packet_count: 2,
            total_replayed_loading_packet_count: 4,
            ready_to_enter_world: true,
            client_loaded: true,
            connect_confirm_queued: true,
            connect_confirm_flushed: true,
            snapshot_watchdog_armed_at_ms: Some(10),
        });
        state.last_ready_inbound_liveness_anchor_at_ms = Some(13);
        state.ready_inbound_liveness_anchor_count = 5;

        apply_login_bootstrap(&mut state, &login_second);

        assert!(!state.client_loaded);
        assert!(!state.connect_confirm_sent);
        assert!(!state.connect_confirm_flushed);
        assert_eq!(state.last_connect_confirm_at_ms, None);
        assert_eq!(state.last_connect_confirm_flushed_at_ms, None);
        assert_eq!(state.finish_connecting_commit_count, 0);
        assert_eq!(state.last_finish_connecting, None);
        assert_eq!(state.last_ready_inbound_liveness_anchor_at_ms, None);
        assert_eq!(state.ready_inbound_liveness_anchor_count, 0);
        assert_eq!(state.bootstrap_stream_id, Some(99));
        assert!(state.world_stream_loaded);
    }

    #[test]
    fn second_world_load_overwrites_bootstrap_projection_hash_and_count_fields() {
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet_first, chunk_packets_first) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let (begin_packet_second, chunk_packets_second) =
            encode_world_stream_packets(&compressed_world_stream, 8, 1024).unwrap();
        let login_first = LoginBootstrap::from_stream_packets(
            &[0x10],
            &begin_packet_first,
            &chunk_packets_first,
            "fr",
        )
        .unwrap();
        let mut login_second = LoginBootstrap::from_stream_packets(
            &[0x20],
            &begin_packet_second,
            &chunk_packets_second,
            "fr",
        )
        .unwrap();

        login_second.bootstrap.rules_sha256 = "rules_second_hash".to_string();
        login_second.bootstrap.map_locales_sha256 = "locales_second_hash".to_string();
        login_second.bootstrap.tags_sha256 = "tags_second_hash".to_string();
        login_second.bootstrap.team_count = 9;
        login_second.bootstrap.marker_count = 12;
        login_second.bootstrap.custom_chunk_count = 3;
        login_second.bootstrap.content_patch_count = 4;
        login_second.bootstrap.player_team_plan_count = 7;
        login_second.bootstrap.static_fog_team_count = 2;
        login_second.bootstrap.ready_to_enter_world = false;

        let mut state = SessionState::default();
        apply_login_bootstrap(&mut state, &login_first);
        let first_projection = state.world_bootstrap_projection.clone().unwrap();
        assert_ne!(first_projection.rules_sha256, "rules_second_hash");
        assert_ne!(first_projection.team_count, 9);

        apply_login_bootstrap(&mut state, &login_second);

        assert_eq!(
            state.world_bootstrap_projection,
            Some(WorldBootstrapProjection {
                rules_sha256: "rules_second_hash".to_string(),
                map_locales_sha256: "locales_second_hash".to_string(),
                tags_sha256: "tags_second_hash".to_string(),
                team_count: 9,
                marker_count: 12,
                custom_chunk_count: 3,
                content_patch_count: 4,
                player_team_plan_count: 7,
                static_fog_team_count: 2,
            })
        );
        assert!(!state.ready_to_enter_world);
    }

    #[test]
    fn world_stream_chunk_boundaries_roundtrip_stays_stable() {
        let compressed_world_stream = sample_world_stream_bytes();
        let stream_len = compressed_world_stream.len();
        let chunk_sizes = [1usize, 2, 3, 7, 64, 255, 256, 257, 1024, stream_len];

        for chunk_size in chunk_sizes {
            let (begin_packet, chunk_packets) =
                encode_world_stream_packets(&compressed_world_stream, 7, chunk_size).unwrap();
            let mut assembler =
                WorldStreamAssembler::from_stream_begin_packet(&begin_packet).unwrap();
            assert!(!assembler.is_complete());

            for (index, chunk_packet) in chunk_packets.iter().enumerate() {
                let is_complete = assembler.push_stream_chunk_packet(chunk_packet).unwrap();
                let is_last = index + 1 == chunk_packets.len();
                assert_eq!(is_complete, is_last);
            }

            assert!(assembler.is_complete());
            let rebuilt = assembler.finish().unwrap();
            assert_eq!(rebuilt, compressed_world_stream);
        }
    }
}
