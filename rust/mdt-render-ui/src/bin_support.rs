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
        .enumerate()
        .map(|(pair_index, chunk)| {
            let pair = std::str::from_utf8(chunk)
                .map_err(|err| format!("invalid hex at byte-pair {pair_index}: {err}"))?;
            u8::from_str_radix(pair, 16).map_err(|err| {
                format!("invalid hex at byte-pair {pair_index} ({pair}): {err}")
            })
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
    fn decode_hex_accepts_uppercase_hex_digits() {
        assert_eq!(decode_hex("0A0B0C0D").unwrap(), vec![10, 11, 12, 13]);
    }

    #[test]
    fn decode_hex_reports_invalid_pair_index() {
        let err = decode_hex("0a zz 0c").expect_err("invalid pair should fail");

        assert!(err.contains("byte-pair 1"));
        assert!(err.contains("zz"));
    }

    #[test]
    fn decode_hex_rejects_odd_length_input() {
        let err = decode_hex("0a0").expect_err("odd-length input should fail");

        assert_eq!(err, "hex input length must be even");
    }

    #[test]
    fn decode_hex_accepts_empty_input() {
        assert_eq!(decode_hex("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn decode_hex_treats_whitespace_only_input_as_empty() {
        assert_eq!(decode_hex(" \t\n\r").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn read_world_stream_bytes_uses_default_fixture() {
        let bytes = read_world_stream_bytes(None).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn read_world_stream_bytes_reads_custom_fixture_path() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mdt-render-ui-bin-support-{}-{}.hex",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "0a 0b\n0c").unwrap();

        let bytes = read_world_stream_bytes(Some(path.as_path())).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(bytes, vec![10, 11, 12]);
    }

    #[test]
    fn read_world_stream_bytes_rejects_odd_length_custom_fixture_content() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mdt-render-ui-bin-support-odd-{}-{}.hex",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "0a0").unwrap();

        let err = read_world_stream_bytes(Some(path.as_path())).unwrap_err();
        let _ = std::fs::remove_file(&path);

        assert_eq!(err, "hex input length must be even");
    }

    #[test]
    fn read_world_stream_bytes_rejects_non_utf8_custom_fixture_content() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mdt-render-ui-bin-support-utf8-{}-{}.hex",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();

        let err = read_world_stream_bytes(Some(path.as_path())).unwrap_err();
        let _ = std::fs::remove_file(&path);

        assert!(err.contains("UTF-8") || err.contains("utf-8"), "{err}");
    }

    #[test]
    fn read_world_stream_bytes_propagates_invalid_hex_from_custom_fixture_path() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mdt-render-ui-bin-support-invalid-hex-{}-{}.hex",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "0a zz 0c").unwrap();

        let err = read_world_stream_bytes(Some(path.as_path())).unwrap_err();
        let _ = std::fs::remove_file(&path);

        assert!(err.contains("byte-pair 1"), "{err}");
        assert!(err.contains("zz"), "{err}");
    }

    #[test]
    fn read_world_stream_bytes_reports_missing_custom_fixture_path() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mdt-render-ui-bin-support-missing-{}-{}.hex",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let err = read_world_stream_bytes(Some(path.as_path())).unwrap_err();

        assert!(
            err.contains("os error 2")
                || err.contains("No such file")
                || err.contains("cannot find")
                || err.contains("系统找不到")
        );
    }
}
