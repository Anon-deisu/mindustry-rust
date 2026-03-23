use std::collections::BTreeMap;

mod object;

pub use object::{
    read_object, read_object_prefix, write_object, TypeIoEffectPositionHint, TypeIoEffectSummary,
    TypeIoEffectSummaryBudget, TypeIoObject, TypeIoObjectMatch, TypeIoReadError,
    TypeIoSemanticMatch, TypeIoSemanticRef,
};

pub const CONVEYOR_BLOCK_ID: i16 = 0x0101;
pub const CONTENT_TYPE_BLOCK: u8 = 1;
pub const TEAM_SHARDED_ID: u8 = 1;
pub const RULES_BASIC_JSON: &str = "{teams:{},attackMode:true,buildSpeedMultiplier:2.5,attributes:{},objectives:[],tags:{mode:{class:java.lang.String,value:golden}}}";

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

pub fn pack_point2(x: i32, y: i32) -> i32 {
    ((x & 0xffff) << 16) | (y & 0xffff)
}

pub fn write_bool(out: &mut Vec<u8>, value: bool) {
    out.push(u8::from(value));
}

pub fn write_byte(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

pub fn write_short(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn write_int(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn write_float(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_bits().to_be_bytes());
}

pub fn write_string(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            write_byte(out, 1);
            let bytes = value.as_bytes();
            let len: u16 = bytes.len().try_into().expect("string too long");
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(bytes);
        }
        None => write_byte(out, 0),
    }
}

pub fn write_block(out: &mut Vec<u8>, block_id: i16) {
    write_short(out, block_id);
}

pub fn write_content(out: &mut Vec<u8>, content_type: u8, content_id: i16) {
    write_byte(out, content_type);
    write_short(out, content_id);
}

pub fn write_team(out: &mut Vec<u8>, team_id: u8) {
    write_byte(out, team_id);
}

pub fn write_tile(out: &mut Vec<u8>, x: i32, y: i32) {
    write_int(out, pack_point2(x, y));
}

pub fn write_unit_null(out: &mut Vec<u8>) {
    write_byte(out, 0);
    write_int(out, 0);
}

pub fn write_vec2(out: &mut Vec<u8>, x: f32, y: f32) {
    write_float(out, x);
    write_float(out, y);
}

pub fn write_object_point2(out: &mut Vec<u8>, x: i32, y: i32) {
    write_byte(out, 7);
    write_int(out, x);
    write_int(out, y);
}

pub fn write_plan_place(
    out: &mut Vec<u8>,
    x: i32,
    y: i32,
    rotation: u8,
    block_id: i16,
    config_x: i32,
    config_y: i32,
) {
    write_byte(out, 0);
    write_int(out, pack_point2(x, y));
    write_short(out, block_id);
    write_byte(out, rotation);
    write_byte(out, 1);
    write_object_point2(out, config_x, config_y);
}

pub fn write_plan_break(out: &mut Vec<u8>, x: i32, y: i32) {
    write_byte(out, 1);
    write_int(out, pack_point2(x, y));
}

pub fn write_plans_queue_net(out: &mut Vec<u8>) {
    write_int(out, 2);
    write_plan_place(out, 1, 2, 1, CONVEYOR_BLOCK_ID, 3, 4);
    write_plan_break(out, 5, 6);
}

pub fn write_rules_basic(out: &mut Vec<u8>) {
    let bytes = RULES_BASIC_JSON.as_bytes();
    write_int(out, bytes.len() as i32);
    out.extend_from_slice(bytes);
}

pub fn generate_typeio_goldens() -> String {
    let mut samples = BTreeMap::new();

    let mut bytes = Vec::new();
    write_block(&mut bytes, CONVEYOR_BLOCK_ID);
    samples.insert("block.conveyor", encode_hex(&bytes));

    bytes.clear();
    write_content(&mut bytes, CONTENT_TYPE_BLOCK, CONVEYOR_BLOCK_ID);
    samples.insert("content.block.conveyor", encode_hex(&bytes));

    bytes.clear();
    write_object_point2(&mut bytes, 3, 4);
    samples.insert("object.point2", encode_hex(&bytes));

    bytes.clear();
    write_plan_place(&mut bytes, 1, 2, 1, CONVEYOR_BLOCK_ID, 3, 4);
    samples.insert("plan.place", encode_hex(&bytes));

    bytes.clear();
    write_plans_queue_net(&mut bytes);
    samples.insert("plans.queue.net", encode_hex(&bytes));

    bytes.clear();
    write_rules_basic(&mut bytes);
    samples.insert("rules.basic", encode_hex(&bytes));

    bytes.clear();
    write_string(&mut bytes, Some("golden-字符串"));
    samples.insert("string.nonNull", encode_hex(&bytes));

    bytes.clear();
    write_string(&mut bytes, None);
    samples.insert("string.null", encode_hex(&bytes));

    bytes.clear();
    write_team(&mut bytes, TEAM_SHARDED_ID);
    samples.insert("team.sharded", encode_hex(&bytes));

    bytes.clear();
    write_tile(&mut bytes, 1, 2);
    samples.insert("tile.1.2", encode_hex(&bytes));

    bytes.clear();
    write_unit_null(&mut bytes);
    samples.insert("unit.null", encode_hex(&bytes));

    bytes.clear();
    write_vec2(&mut bytes, 12.5, -3.25);
    samples.insert("vec2.basic", encode_hex(&bytes));

    let mut out = String::new();
    for (index, (key, value)) in samples.into_iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(key);
        out.push('=');
        out.push_str(&value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_no_duplicate_text(label: &str, text: &str) {
        let mut seen = BTreeMap::new();
        let mut duplicates = Vec::new();

        for line in text.lines() {
            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            if seen.insert(key.to_string(), ()).is_some() {
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
    fn goldens_match_expected_lines() {
        let text = generate_typeio_goldens();
        assert!(text.contains("block.conveyor=0101"));
        assert!(text.contains("content.block.conveyor=010101"));
        assert!(text.contains("object.point2=070000000300000004"));
        assert!(text.contains("team.sharded=01"));
        assert!(text.contains("vec2.basic=41480000c0500000"));
    }

    #[test]
    fn rules_basic_length_matches_payload() {
        let mut bytes = Vec::new();
        write_rules_basic(&mut bytes);
        let declared = i32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(declared, bytes.len() - 4);
        assert_eq!(&bytes[4..], RULES_BASIC_JSON.as_bytes());
    }

    #[test]
    fn typeio_goldens_are_duplicate_free() {
        let text = generate_typeio_goldens();
        assert_no_duplicate_text("typeio-goldens", &text);
    }
}
