use std::fs;
use std::path::Path;

const DEFAULT_WORLD_STREAM_HEX: &str =
    include_str!("../../../fixtures/world-streams/archipelago-6567-world-stream.hex");

pub fn read_world_stream_bytes(path: Option<&Path>) -> Result<Vec<u8>, String> {
    let world_stream_hex = match path {
        Some(path) => fs::read_to_string(path).map_err(|err| err.to_string())?,
        None => DEFAULT_WORLD_STREAM_HEX.to_string(),
    };
    decode_hex(&world_stream_hex)
}

pub fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    let cleaned = text
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>();
    if cleaned.len() % 2 != 0 {
        return Err("hex input length must be even".to_string());
    }

    cleaned
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let pair = std::str::from_utf8(chunk).map_err(|err| err.to_string())?;
            u8::from_str_radix(pair, 16).map_err(|err| err.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{decode_hex, read_world_stream_bytes};

    #[test]
    fn decode_hex_ignores_ascii_whitespace() {
        assert_eq!(decode_hex("0a 0b\n0c\t0d").unwrap(), vec![10, 11, 12, 13]);
    }

    #[test]
    fn read_world_stream_bytes_uses_default_fixture() {
        let bytes = read_world_stream_bytes(None).unwrap();
        assert!(!bytes.is_empty());
    }
}
