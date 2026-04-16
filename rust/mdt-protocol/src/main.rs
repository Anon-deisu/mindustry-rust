use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

use mdt_protocol::{
    generate_framework_message_goldens, generate_packet_serializer_goldens,
    generate_world_stream_transport_goldens,
};

const USAGE: &str = "usage: mdt-protocol <output-dir>";

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = parse_args(env::args().skip(1))?;
    let output_dir = Path::new(&output_dir);
    fs::create_dir_all(output_dir).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to create output directory {}: {err}",
                output_dir.display()
            ),
        )
    })?;

    let repo_root = repo_root_from_manifest_dir()?;
    let tests_resources = repo_root
        .join("tests")
        .join("src")
        .join("test")
        .join("resources");

    let connect_payload_hex = read_text(
        &tests_resources.join("connect-packet.hex"),
        "connect packet golden",
    )?;
    let connect_payload = decode_hex(connect_payload_hex.trim())?;
    let compressed_hex = read_text(
        &tests_resources.join("world-stream.hex"),
        "world stream hex",
    )?;
    let compressed = decode_hex(compressed_hex.trim())?;

    write_text(
        output_dir.join("packet-serializer-goldens.txt"),
        generate_packet_serializer_goldens(&connect_payload)?,
        "packet serializer goldens",
    )?;
    write_text(
        output_dir.join("framework-message-goldens.txt"),
        generate_framework_message_goldens()?,
        "framework message goldens",
    )?;
    write_text(
        output_dir.join("world-stream-transport-goldens.txt"),
        generate_world_stream_transport_goldens(&compressed)?,
        "world stream transport goldens",
    )?;
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<PathBuf, io::Error> {
    let mut args = args;
    let output_dir = args
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, USAGE))?;
    if args.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE));
    }

    Ok(PathBuf::from(output_dir))
}

fn decode_hex(text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let cleaned = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    if cleaned.len() % 2 != 0 {
        return Err("hex text must contain an even number of digits".into());
    }

    let mut bytes = Vec::with_capacity(cleaned.len() / 2);
    let chars = cleaned.as_bytes();
    for index in (0..chars.len()).step_by(2) {
        let byte = u8::from_str_radix(&cleaned[index..index + 2], 16)?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn repo_root_from_manifest_dir() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "failed to resolve repo root from CARGO_MANIFEST_DIR={}",
                    manifest_dir.display()
                ),
            )
            .into()
        })
}

fn read_text(path: &Path, label: &str) -> Result<String, Box<dyn Error>> {
    fs::read_to_string(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to read {label} from {}: {err}", path.display()),
        )
        .into()
    })
}

fn write_text(path: PathBuf, contents: String, label: &str) -> Result<(), Box<dyn Error>> {
    fs::write(&path, contents).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to write {label} to {}: {err}", path.display()),
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_hex, parse_args, USAGE};
    use std::path::PathBuf;

    #[test]
    fn rejects_extra_arguments() {
        let err = parse_args(vec!["out".to_string(), "extra".to_string()].into_iter())
            .unwrap_err();

        assert_eq!(err.to_string(), USAGE);
    }

    #[test]
    fn accepts_single_output_dir() {
        let output_dir = parse_args(vec!["out".to_string()].into_iter()).unwrap();

        assert_eq!(output_dir, PathBuf::from("out"));
    }

    #[test]
    fn decode_hex_ignores_whitespace_and_rejects_odd_length() {
        assert_eq!(decode_hex("0a 0B\n1c\t2D").unwrap(), vec![0x0a, 0x0b, 0x1c, 0x2d]);
        assert!(decode_hex("abc").is_err());
    }

    #[test]
    fn decode_hex_rejects_invalid_hex_digits() {
        assert!(decode_hex("zz").is_err());
        assert!(decode_hex("0g").is_err());
    }
}
