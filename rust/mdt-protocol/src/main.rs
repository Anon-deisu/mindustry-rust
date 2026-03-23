use std::{env, error::Error, fs, path::Path};

use mdt_protocol::{
    generate_framework_message_goldens, generate_packet_serializer_goldens,
    generate_world_stream_transport_goldens,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = env::args()
        .nth(1)
        .ok_or("usage: mdt-protocol <output-dir>")?;
    let output_dir = Path::new(&output_dir);
    fs::create_dir_all(output_dir)?;

    let connect_payload_hex = fs::read_to_string("tests/src/test/resources/connect-packet.hex")?;
    let connect_payload = decode_hex(connect_payload_hex.trim())?;
    let compressed_hex = fs::read_to_string("tests/src/test/resources/world-stream.hex")?;
    let compressed = decode_hex(compressed_hex.trim())?;

    fs::write(
        output_dir.join("packet-serializer-goldens.txt"),
        generate_packet_serializer_goldens(&connect_payload)?,
    )?;
    fs::write(
        output_dir.join("framework-message-goldens.txt"),
        generate_framework_message_goldens()?,
    )?;
    fs::write(
        output_dir.join("world-stream-transport-goldens.txt"),
        generate_world_stream_transport_goldens(&compressed)?,
    )?;
    Ok(())
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
