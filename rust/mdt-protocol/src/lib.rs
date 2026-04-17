use flate2::{bufread::ZlibDecoder, write::ZlibEncoder, Compression};
use std::fmt;
use std::io::{Read, Write};

pub const STREAM_BEGIN_PACKET_ID: u8 = 0;
pub const STREAM_CHUNK_PACKET_ID: u8 = 1;
pub const WORLD_STREAM_PACKET_ID: u8 = 2;
pub const CONNECT_PACKET_ID: u8 = 3;
pub const FRAMEWORK_MESSAGE_PREFIX: u8 = 0xfe;
pub const FRAMEWORK_PING_ID: u8 = 0;
pub const FRAMEWORK_DISCOVER_HOST_ID: u8 = 1;
pub const FRAMEWORK_KEEP_ALIVE_ID: u8 = 2;
pub const FRAMEWORK_REGISTER_UDP_ID: u8 = 3;
pub const FRAMEWORK_REGISTER_TCP_ID: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPacket {
    pub packet_id: u8,
    pub raw_length: u16,
    pub compression: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum PacketCodecError {
    TooShort,
    UnsupportedCompression(u8),
    LengthOverflow(usize),
    TrailingBytes(usize),
    Lz4(lz4_flex::block::DecompressError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameworkMessage {
    Ping { id: i32, is_reply: bool },
    DiscoverHost,
    KeepAlive,
    RegisterUdp { connection_id: i32 },
    RegisterTcp { connection_id: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameworkCodecError {
    TooShort,
    InvalidPrefix(u8),
    UnknownType(u8),
    InvalidReplyFlag(u8),
    TrailingBytes(usize),
}

impl fmt::Display for FrameworkCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "framework message buffer too short"),
            Self::InvalidPrefix(value) => write!(f, "invalid framework prefix: {value}"),
            Self::UnknownType(value) => write!(f, "unknown framework message type: {value}"),
            Self::InvalidReplyFlag(value) => write!(f, "invalid ping reply flag: {value}"),
            Self::TrailingBytes(length) => write!(f, "unexpected trailing bytes: {length}"),
        }
    }
}

impl std::error::Error for FrameworkCodecError {}

impl fmt::Display for PacketCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "packet buffer too short"),
            Self::UnsupportedCompression(value) => {
                write!(f, "unsupported compression flag: {value}")
            }
            Self::LengthOverflow(length) => write!(f, "payload too large for u16 length: {length}"),
            Self::TrailingBytes(length) => write!(f, "unexpected trailing bytes: {length}"),
            Self::Lz4(error) => write!(f, "lz4 decode failed: {error}"),
        }
    }
}

impl std::error::Error for PacketCodecError {}

impl From<lz4_flex::block::DecompressError> for PacketCodecError {
    fn from(value: lz4_flex::block::DecompressError) -> Self {
        Self::Lz4(value)
    }
}

pub fn encode_packet(
    packet_id: u8,
    payload: &[u8],
    force_uncompressed: bool,
) -> Result<Vec<u8>, PacketCodecError> {
    let raw_length = u16::try_from(payload.len())
        .map_err(|_| PacketCodecError::LengthOverflow(payload.len()))?;
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.push(packet_id);
    out.extend_from_slice(&raw_length.to_be_bytes());

    if payload.len() < 36 || force_uncompressed {
        out.push(0);
        out.extend_from_slice(payload);
    } else {
        out.push(1);
        out.extend_from_slice(&lz4_flex::block::compress(payload));
    }

    Ok(out)
}

pub fn decode_packet(bytes: &[u8]) -> Result<EncodedPacket, PacketCodecError> {
    if bytes.len() < 4 {
        return Err(PacketCodecError::TooShort);
    }

    let packet_id = bytes[0];
    let raw_length = u16::from_be_bytes([bytes[1], bytes[2]]);
    let compression = bytes[3];
    let remaining = &bytes[4..];
    let raw_length_usize = raw_length as usize;

    let payload = match compression {
        0 => {
            if remaining.len() < raw_length_usize {
                return Err(PacketCodecError::TooShort);
            }
            if remaining.len() != raw_length_usize {
                return Err(PacketCodecError::TrailingBytes(remaining.len() - raw_length_usize));
            }
            remaining.to_vec()
        }
        1 => {
            let consumed = lz4_block_consumed_len(remaining, raw_length_usize)?;
            if consumed != remaining.len() {
                return Err(PacketCodecError::TrailingBytes(remaining.len() - consumed));
            }
            lz4_flex::block::decompress(remaining, raw_length_usize)?
        }
        value => return Err(PacketCodecError::UnsupportedCompression(value)),
    };

    Ok(EncodedPacket {
        packet_id,
        raw_length,
        compression,
        payload,
    })
}

fn read_lz4_length(input: &[u8], pos: &mut usize) -> Result<usize, PacketCodecError> {
    let mut extra = 0usize;
    loop {
        if *pos >= input.len() {
            return Err(PacketCodecError::TooShort);
        }
        let byte = input[*pos];
        *pos += 1;
        extra += byte as usize;
        if byte != 0xFF {
            return Ok(extra);
        }
    }
}

fn lz4_block_consumed_len(input: &[u8], raw_length: usize) -> Result<usize, PacketCodecError> {
    let mut pos = 0usize;
    let mut produced = 0usize;

    while pos < input.len() {
        let token = input[pos];
        pos += 1;

        let mut literal_length = (token >> 4) as usize;
        if literal_length == 15 {
            literal_length += read_lz4_length(input, &mut pos)?;
        }

        if input.len() - pos < literal_length {
            return Err(PacketCodecError::TooShort);
        }
        pos += literal_length;
        produced += literal_length;
        if produced == raw_length {
            return Ok(pos);
        }
        if produced > raw_length {
            return Err(PacketCodecError::TooShort);
        }

        if input.len() - pos < 2 {
            return Err(PacketCodecError::TooShort);
        }
        pos += 2;

        let mut match_length = 4 + (token & 0x0F) as usize;
        if match_length == 19 {
            match_length += read_lz4_length(input, &mut pos)?;
        }
        produced += match_length;
        if produced == raw_length {
            return Ok(pos);
        }
        if produced > raw_length {
            return Err(PacketCodecError::TooShort);
        }
    }

    Err(PacketCodecError::TooShort)
}

pub fn stream_begin_payload(id: i32, total: i32, kind: u8) -> Vec<u8> {
    let mut payload = Vec::with_capacity(9);
    payload.extend_from_slice(&id.to_be_bytes());
    payload.extend_from_slice(&total.to_be_bytes());
    payload.push(kind);
    payload
}

pub fn stream_chunk_payload(id: i32, data: &[u8]) -> Result<Vec<u8>, PacketCodecError> {
    let length =
        u16::try_from(data.len()).map_err(|_| PacketCodecError::LengthOverflow(data.len()))?;
    let mut payload = Vec::with_capacity(data.len() + 6);
    payload.extend_from_slice(&id.to_be_bytes());
    payload.extend_from_slice(&length.to_be_bytes());
    payload.extend_from_slice(data);
    Ok(payload)
}

pub fn deflate_zlib(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()
}

pub fn inflate_zlib(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    let trailing = decoder.into_inner();
    if !trailing.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected trailing bytes after zlib stream: {}", trailing.len()),
        ));
    }
    Ok(out)
}

pub fn split_stream_chunks(bytes: &[u8], chunk_size: usize) -> Vec<Vec<u8>> {
    assert!(chunk_size > 0, "chunk_size must be > 0");
    bytes
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

pub fn reassemble_stream_chunks(chunks: &[Vec<u8>]) -> Vec<u8> {
    let total = chunks.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(total);
    for chunk in chunks {
        out.extend_from_slice(chunk);
    }
    out
}

pub fn encode_framework_message(message: &FrameworkMessage) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    out.push(FRAMEWORK_MESSAGE_PREFIX);

    match message {
        FrameworkMessage::Ping { id, is_reply } => {
            out.push(FRAMEWORK_PING_ID);
            out.extend_from_slice(&id.to_be_bytes());
            out.push(u8::from(*is_reply));
        }
        FrameworkMessage::DiscoverHost => {
            out.push(FRAMEWORK_DISCOVER_HOST_ID);
        }
        FrameworkMessage::KeepAlive => {
            out.push(FRAMEWORK_KEEP_ALIVE_ID);
        }
        FrameworkMessage::RegisterUdp { connection_id } => {
            out.push(FRAMEWORK_REGISTER_UDP_ID);
            out.extend_from_slice(&connection_id.to_be_bytes());
        }
        FrameworkMessage::RegisterTcp { connection_id } => {
            out.push(FRAMEWORK_REGISTER_TCP_ID);
            out.extend_from_slice(&connection_id.to_be_bytes());
        }
    }

    out
}

pub fn decode_framework_message(bytes: &[u8]) -> Result<FrameworkMessage, FrameworkCodecError> {
    if bytes.len() < 2 {
        return Err(FrameworkCodecError::TooShort);
    }
    if bytes[0] != FRAMEWORK_MESSAGE_PREFIX {
        return Err(FrameworkCodecError::InvalidPrefix(bytes[0]));
    }

    match bytes[1] {
        FRAMEWORK_PING_ID => {
            if bytes.len() < 7 {
                return Err(FrameworkCodecError::TooShort);
            }
            if bytes.len() != 7 {
                return Err(FrameworkCodecError::TrailingBytes(bytes.len() - 7));
            }
            let is_reply = match bytes[6] {
                0 => false,
                1 => true,
                value => return Err(FrameworkCodecError::InvalidReplyFlag(value)),
            };
            Ok(FrameworkMessage::Ping {
                id: i32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
                is_reply,
            })
        }
        FRAMEWORK_DISCOVER_HOST_ID => {
            if bytes.len() != 2 {
                return Err(FrameworkCodecError::TrailingBytes(bytes.len() - 2));
            }
            Ok(FrameworkMessage::DiscoverHost)
        }
        FRAMEWORK_KEEP_ALIVE_ID => {
            if bytes.len() != 2 {
                return Err(FrameworkCodecError::TrailingBytes(bytes.len() - 2));
            }
            Ok(FrameworkMessage::KeepAlive)
        }
        FRAMEWORK_REGISTER_UDP_ID => {
            if bytes.len() < 6 {
                return Err(FrameworkCodecError::TooShort);
            }
            if bytes.len() != 6 {
                return Err(FrameworkCodecError::TrailingBytes(bytes.len() - 6));
            }
            Ok(FrameworkMessage::RegisterUdp {
                connection_id: i32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            })
        }
        FRAMEWORK_REGISTER_TCP_ID => {
            if bytes.len() < 6 {
                return Err(FrameworkCodecError::TooShort);
            }
            if bytes.len() != 6 {
                return Err(FrameworkCodecError::TrailingBytes(bytes.len() - 6));
            }
            Ok(FrameworkMessage::RegisterTcp {
                connection_id: i32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            })
        }
        value => Err(FrameworkCodecError::UnknownType(value)),
    }
}

pub fn generate_packet_serializer_goldens(connect_payload: &[u8]) -> Result<String, String> {
    let stream_begin = encode_packet(
        STREAM_BEGIN_PACKET_ID,
        &stream_begin_payload(7, 300, WORLD_STREAM_PACKET_ID),
        false,
    )
    .map_err(|e| e.to_string())?;
    let connect_packet =
        encode_packet(CONNECT_PACKET_ID, connect_payload, false).map_err(|e| e.to_string())?;
    let stream_chunk_data = (1u8..=48).collect::<Vec<_>>();
    let stream_chunk = encode_packet(
        STREAM_CHUNK_PACKET_ID,
        &stream_chunk_payload(7, &stream_chunk_data).map_err(|e| e.to_string())?,
        true,
    )
    .map_err(|e| e.to_string())?;

    let mut text = String::new();
    append_packet_case(&mut text, "streamBegin", &stream_begin)?;
    append_packet_case(&mut text, "connect", &connect_packet)?;
    append_packet_case(&mut text, "streamChunk", &stream_chunk)?;
    Ok(text)
}

pub fn generate_framework_message_goldens() -> Result<String, String> {
    let cases = [
        (
            "ping",
            FrameworkMessage::Ping {
                id: 123_456_789,
                is_reply: true,
            },
        ),
        ("discoverHost", FrameworkMessage::DiscoverHost),
        ("keepAlive", FrameworkMessage::KeepAlive),
        (
            "registerUdp",
            FrameworkMessage::RegisterUdp {
                connection_id: 321_654_987,
            },
        ),
        (
            "registerTcp",
            FrameworkMessage::RegisterTcp {
                connection_id: 135_792_468,
            },
        ),
    ];

    let mut out = String::new();
    for (index, (name, message)) in cases.iter().enumerate() {
        let encoded = encode_framework_message(message);
        let decoded = decode_framework_message(&encoded).map_err(|e| e.to_string())?;
        if &decoded != message {
            return Err(format!("framework round-trip mismatch for {name}"));
        }
        out.push_str(&format!("{name}.packet={}", encode_hex(&encoded)));
        if index + 1 < cases.len() {
            out.push('\n');
        }
    }

    Ok(out)
}

pub fn generate_world_stream_transport_goldens(compressed: &[u8]) -> Result<String, String> {
    let inflated = inflate_zlib(compressed).map_err(|e| e.to_string())?;
    let chunks = split_stream_chunks(compressed, 1024);
    let rebuilt = reassemble_stream_chunks(&chunks);

    if rebuilt != compressed {
        return Err("stream chunk rebuild mismatch".to_string());
    }

    let begin_payload = stream_begin_payload(
        7,
        checked_stream_total_len(compressed.len())?,
        WORLD_STREAM_PACKET_ID,
    );
    let first_size = chunks.first().map(|chunk| chunk.len()).unwrap_or(0);
    let last_size = chunks.last().map(|chunk| chunk.len()).unwrap_or(0);

    Ok(format!(
        concat!(
            "compressed.length={:08x}\n",
            "compressed.sha256={}\n",
            "inflated.length={:08x}\n",
            "inflated.sha256={}\n",
            "streamBegin.payload={}\n",
            "chunk.count={:08x}\n",
            "chunk.firstSize={:08x}\n",
            "chunk.lastSize={:08x}"
        ),
        compressed.len(),
        sha256_hex(compressed),
        inflated.len(),
        sha256_hex(&inflated),
        encode_hex(&begin_payload),
        chunks.len(),
        first_size,
        last_size,
    ))
}

fn checked_stream_total_len(len: usize) -> Result<i32, String> {
    i32::try_from(len).map_err(|_| format!("stream payload too large: {len}"))
}

fn append_packet_case(out: &mut String, prefix: &str, encoded: &[u8]) -> Result<(), String> {
    let decoded = decode_packet(encoded).map_err(|e| e.to_string())?;
    out.push_str(&format!("{prefix}.packetId={:02x}\n", decoded.packet_id));
    out.push_str(&format!("{prefix}.rawLength={:04x}\n", decoded.raw_length));
    out.push_str(&format!(
        "{prefix}.compression={:02x}\n",
        decoded.compression
    ));
    out.push_str(&format!(
        "{prefix}.payload={}\n",
        encode_hex(&decoded.payload)
    ));
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex_text(text: &str) -> Vec<u8> {
        let cleaned = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>()
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

    fn assert_no_duplicate_text(label: &str, text: &str) {
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();

        for line in text.lines() {
            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            if !seen.insert(key.to_string()) {
                duplicates.push(key.to_string());
            }
        }

        assert!(
            duplicates.is_empty(),
            "{label} contains duplicate keys: {}",
            duplicates.join(", ")
        );
    }

    #[test]
    fn encode_hex_formats_empty_and_preserves_leading_zeroes() {
        assert_eq!(encode_hex(&[]), "");
        assert_eq!(encode_hex(&[0x00, 0x01, 0x0a, 0x10]), "00010a10");
    }

    #[test]
    fn sha256_hex_formats_empty_and_known_digest() {
        assert_eq!(
            sha256_hex(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn read_lz4_length_accumulates_ff_runs_and_stops_on_non_ff() {
        let bytes = [0xff, 0xff, 0x05, 0xaa];
        let mut pos = 0usize;

        assert_eq!(read_lz4_length(&bytes, &mut pos).unwrap(), 0xff + 0xff + 0x05);
        assert_eq!(pos, 3);
        assert_eq!(bytes[pos], 0xaa);
    }

    #[test]
    fn read_lz4_length_handles_empty_input_and_stops_on_first_non_ff() {
        let mut empty_pos = 0usize;
        assert!(matches!(
            read_lz4_length(&[], &mut empty_pos),
            Err(PacketCodecError::TooShort)
        ));
        assert_eq!(empty_pos, 0);

        let bytes = [0xff, 0xff, 0x04, 0xaa];
        let mut pos = 0usize;
        assert_eq!(read_lz4_length(&bytes, &mut pos).unwrap(), 0xff + 0xff + 0x04);
        assert_eq!(pos, 3);
        assert_eq!(bytes[pos], 0xaa);
    }

    #[test]
    fn lz4_block_consumed_len_rejects_truncated_lz4_sequences() {
        let mut pos = 0usize;
        assert!(matches!(
            read_lz4_length(&[0xff], &mut pos),
            Err(PacketCodecError::TooShort)
        ));

        assert!(matches!(
            lz4_block_consumed_len(&[0xf0, 0xff], 16),
            Err(PacketCodecError::TooShort)
        ));
        assert!(matches!(
            lz4_block_consumed_len(&[0x0f, 0x01, 0x00, 0xff], 24),
            Err(PacketCodecError::TooShort)
        ));
    }

    #[test]
    fn lz4_block_consumed_len_matches_lz4_flex_block_output_without_trailing_bytes() {
        let payload = (0u8..=127).collect::<Vec<_>>();
        let compressed = lz4_flex::block::compress(&payload);

        assert_eq!(
            lz4_block_consumed_len(&compressed, payload.len()).unwrap(),
            compressed.len()
        );
    }

    #[test]
    fn small_packet_stays_uncompressed() {
        let payload = stream_begin_payload(7, 300, 2);
        let encoded = encode_packet(STREAM_BEGIN_PACKET_ID, &payload, false).unwrap();
        let decoded = decode_packet(&encoded).unwrap();

        assert_eq!(decoded.packet_id, STREAM_BEGIN_PACKET_ID);
        assert_eq!(decoded.compression, 0);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn large_packet_compresses() {
        let payload = (0u8..=63).collect::<Vec<_>>();
        let encoded = encode_packet(CONNECT_PACKET_ID, &payload, false).unwrap();
        let decoded = decode_packet(&encoded).unwrap();

        assert_eq!(decoded.packet_id, CONNECT_PACKET_ID);
        assert_eq!(decoded.compression, 1);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn packet_length_overflow_is_rejected() {
        let payload = vec![0u8; 65_536];

        assert!(matches!(
            encode_packet(CONNECT_PACKET_ID, &payload, false),
            Err(PacketCodecError::LengthOverflow(65_536))
        ));
        assert!(matches!(
            stream_chunk_payload(7, &payload),
            Err(PacketCodecError::LengthOverflow(65_536))
        ));
    }

    #[test]
    fn stream_begin_payload_encodes_id_total_and_kind() {
        let payload = stream_begin_payload(0x0102_0304, 0x0506_0708, 0x09);

        assert_eq!(
            payload,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]
        );
    }

    #[test]
    fn stream_chunk_payload_rejects_length_overflow_and_encodes_header() {
        let payload = vec![0x11, 0x22, 0x33, 0x44];
        let encoded = stream_chunk_payload(0x0102_0304, &payload).unwrap();

        assert_eq!(&encoded[..4], &0x0102_0304i32.to_be_bytes());
        assert_eq!(&encoded[4..6], &(payload.len() as u16).to_be_bytes());
        assert_eq!(&encoded[6..], payload.as_slice());

        let overflow = vec![0u8; 65_536];
        assert!(matches!(
            stream_chunk_payload(7, &overflow),
            Err(PacketCodecError::LengthOverflow(65_536))
        ));
    }

    #[test]
    fn checked_stream_total_len_handles_zero_and_boundary_lengths() {
        assert_eq!(checked_stream_total_len(0).unwrap(), 0);
        assert_eq!(checked_stream_total_len(i32::MAX as usize).unwrap(), i32::MAX);
        assert_eq!(
            checked_stream_total_len(i32::MAX as usize + 1).unwrap_err(),
            format!("stream payload too large: {}", i32::MAX as usize + 1)
        );
    }

    #[test]
    fn forced_uncompressed_packet_round_trips() {
        let payload = stream_chunk_payload(7, &(1u8..=48).collect::<Vec<_>>()).unwrap();
        let encoded = encode_packet(STREAM_CHUNK_PACKET_ID, &payload, true).unwrap();
        let decoded = decode_packet(&encoded).unwrap();

        assert_eq!(decoded.packet_id, STREAM_CHUNK_PACKET_ID);
        assert_eq!(decoded.compression, 0);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn decode_packet_accepts_uncompressed_empty_payload() {
        let decoded = decode_packet(&[CONNECT_PACKET_ID, 0x00, 0x00, 0x00]).unwrap();

        assert_eq!(decoded.packet_id, CONNECT_PACKET_ID);
        assert_eq!(decoded.raw_length, 0);
        assert_eq!(decoded.compression, 0);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn uncompressed_decode_uses_declared_raw_length() {
        let encoded = vec![
            CONNECT_PACKET_ID,
            0x00,
            0x03,
            0x00,
            0x10,
            0x20,
            0x30,
        ];
        let decoded = decode_packet(&encoded).unwrap();

        assert_eq!(decoded.packet_id, CONNECT_PACKET_ID);
        assert_eq!(decoded.raw_length, 3);
        assert_eq!(decoded.compression, 0);
        assert_eq!(decoded.payload, vec![0x10, 0x20, 0x30]);
    }

    #[test]
    fn uncompressed_decode_rejects_truncated_declared_raw_length() {
        let encoded = vec![CONNECT_PACKET_ID, 0x00, 0x05, 0x00, 0x10, 0x20, 0x30];

        assert!(matches!(
            decode_packet(&encoded),
            Err(PacketCodecError::TooShort)
        ));
    }

    #[test]
    fn decode_packet_rejects_uncompressed_trailing_bytes() {
        let encoded = vec![
            CONNECT_PACKET_ID,
            0x00,
            0x03,
            0x00,
            0x10,
            0x20,
            0x30,
            0x40,
        ];

        assert!(matches!(
            decode_packet(&encoded),
            Err(PacketCodecError::TrailingBytes(1))
        ));
    }

    #[test]
    fn decode_packet_rejects_compressed_trailing_bytes() {
        let payload = (0u8..=63).collect::<Vec<_>>();
        let mut encoded = encode_packet(CONNECT_PACKET_ID, &payload, false).unwrap();
        encoded.push(0x00);

        assert!(matches!(
            decode_packet(&encoded),
            Err(PacketCodecError::TrailingBytes(1))
        ));
    }

    #[test]
    fn decode_packet_rejects_unsupported_compression_without_trailing_bytes() {
        let encoded = vec![CONNECT_PACKET_ID, 0x00, 0x00, 0x09];

        assert!(matches!(
            decode_packet(&encoded),
            Err(PacketCodecError::UnsupportedCompression(9))
        ));
    }

    #[test]
    fn framework_messages_round_trip() {
        let cases = vec![
            FrameworkMessage::Ping {
                id: 123_456_789,
                is_reply: true,
            },
            FrameworkMessage::DiscoverHost,
            FrameworkMessage::KeepAlive,
            FrameworkMessage::RegisterUdp {
                connection_id: 321_654_987,
            },
            FrameworkMessage::RegisterTcp {
                connection_id: 135_792_468,
            },
        ];

        for case in cases {
            let encoded = encode_framework_message(&case);
            let decoded = decode_framework_message(&encoded).unwrap();
            assert_eq!(decoded, case);
        }
    }

    #[test]
    fn decode_framework_message_rejects_trailing_bytes() {
        let encoded = vec![FRAMEWORK_MESSAGE_PREFIX, FRAMEWORK_KEEP_ALIVE_ID, 0x7f];

        assert!(matches!(
            decode_framework_message(&encoded),
            Err(FrameworkCodecError::TrailingBytes(1))
        ));
    }

    #[test]
    fn decode_framework_message_rejects_invalid_ping_reply_flag() {
        let encoded = vec![
            FRAMEWORK_MESSAGE_PREFIX,
            FRAMEWORK_PING_ID,
            0x00,
            0x00,
            0x00,
            0x01,
            0x02,
        ];

        assert!(matches!(
            decode_framework_message(&encoded),
            Err(FrameworkCodecError::InvalidReplyFlag(2))
        ));
    }

    #[test]
    fn decode_framework_message_rejects_unknown_type_without_trailing_bytes() {
        let encoded = vec![FRAMEWORK_MESSAGE_PREFIX, 9];

        assert!(matches!(
            decode_framework_message(&encoded),
            Err(FrameworkCodecError::UnknownType(9))
        ));
    }

    #[test]
    fn decode_framework_message_rejects_invalid_prefix() {
        let encoded = vec![0x7f, FRAMEWORK_KEEP_ALIVE_ID];

        assert!(matches!(
            decode_framework_message(&encoded),
            Err(FrameworkCodecError::InvalidPrefix(0x7f))
        ));
    }

    #[test]
    fn decode_framework_message_rejects_short_buffers() {
        assert!(matches!(
            decode_framework_message(&[]),
            Err(FrameworkCodecError::TooShort)
        ));
        assert!(matches!(
            decode_framework_message(&[FRAMEWORK_MESSAGE_PREFIX]),
            Err(FrameworkCodecError::TooShort)
        ));
        assert!(matches!(
            decode_framework_message(&[
                FRAMEWORK_MESSAGE_PREFIX,
                FRAMEWORK_PING_ID,
                0x00,
                0x00,
                0x00,
                0x01,
            ]),
            Err(FrameworkCodecError::TooShort)
        ));
        assert!(matches!(
            decode_framework_message(&[
                FRAMEWORK_MESSAGE_PREFIX,
                FRAMEWORK_REGISTER_UDP_ID,
                0x00,
                0x00,
                0x00,
            ]),
            Err(FrameworkCodecError::TooShort)
        ));
    }

    #[test]
    fn codec_error_display_strings_remain_stable() {
        assert_eq!(
            PacketCodecError::TooShort.to_string(),
            "packet buffer too short"
        );
        assert_eq!(
            PacketCodecError::UnsupportedCompression(7).to_string(),
            "unsupported compression flag: 7"
        );
        assert_eq!(
            PacketCodecError::LengthOverflow(42).to_string(),
            "payload too large for u16 length: 42"
        );
        assert_eq!(
            PacketCodecError::TrailingBytes(3).to_string(),
            "unexpected trailing bytes: 3"
        );
        let mut bad_lz4 = lz4_flex::block::compress(b"codec");
        bad_lz4.truncate(1);
        let lz4_error = lz4_flex::block::decompress(&bad_lz4, 5).unwrap_err();
        assert!(
            PacketCodecError::from(lz4_error)
                .to_string()
                .starts_with("lz4 decode failed: ")
        );

        assert_eq!(
            FrameworkCodecError::TooShort.to_string(),
            "framework message buffer too short"
        );
        assert_eq!(
            FrameworkCodecError::InvalidPrefix(254).to_string(),
            "invalid framework prefix: 254"
        );
        assert_eq!(
            FrameworkCodecError::UnknownType(9).to_string(),
            "unknown framework message type: 9"
        );
        assert_eq!(
            FrameworkCodecError::InvalidReplyFlag(2).to_string(),
            "invalid ping reply flag: 2"
        );
        assert_eq!(
            FrameworkCodecError::TrailingBytes(1).to_string(),
            "unexpected trailing bytes: 1"
        );
    }

    #[test]
    fn zlib_round_trip() {
        let payload = (0u8..=255).cycle().take(4096).collect::<Vec<_>>();
        let encoded = deflate_zlib(&payload).unwrap();
        let decoded = inflate_zlib(&encoded).unwrap();

        assert_eq!(decoded, payload);
    }

    #[test]
    fn inflate_zlib_rejects_trailing_bytes() {
        let mut encoded = deflate_zlib(b"mindustry").unwrap();
        encoded.extend_from_slice(&[0xde, 0xad]);

        let error = inflate_zlib(&encoded).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("unexpected trailing bytes"));
    }

    #[test]
    fn stream_chunks_round_trip() {
        let payload = (0u8..=255).cycle().take(2500).collect::<Vec<_>>();
        let chunks = split_stream_chunks(&payload, 1024);
        let rebuilt = reassemble_stream_chunks(&chunks);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 1024);
        assert_eq!(chunks[1].len(), 1024);
        assert_eq!(chunks[2].len(), 452);
        assert_eq!(rebuilt, payload);
    }

    #[test]
    fn generate_world_stream_transport_goldens_rejects_oversized_stream_length() {
        let oversized = i32::MAX as usize + 1;
        assert_eq!(
            checked_stream_total_len(oversized).unwrap_err(),
            format!("stream payload too large: {oversized}")
        );
    }

    #[test]
    fn protocol_goldens_are_duplicate_free() {
        let connect_payload = sample_connect_payload();
        let world_stream = sample_world_stream_bytes();

        assert_no_duplicate_text(
            "packet-serializer-goldens",
            &generate_packet_serializer_goldens(&connect_payload).unwrap(),
        );
        assert_no_duplicate_text(
            "framework-message-goldens",
            &generate_framework_message_goldens().unwrap(),
        );
        assert_no_duplicate_text(
            "world-stream-transport-goldens",
            &generate_world_stream_transport_goldens(&world_stream).unwrap(),
        );
    }
}
