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
pub const OBJECTIVES_BASIC_JSON: &str =
    "{objectives:[{type:Research,content:{type:item,id:1}},{type:DestroyBlock,position:[4,5],team:2}]}";
pub const OBJECTIVE_MARKER_BASIC_JSON: &str =
    "{type:ShapeText,x:12.5,y:-3.25,world:true,text:objective-ready}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildPayloadHeader {
    pub block_id_raw: u16,
    pub build_revision: u8,
}

impl BuildPayloadHeader {
    pub fn block_id_i16(self) -> i16 {
        i16::from_be_bytes(self.block_id_raw.to_be_bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitPayloadHeader {
    pub class_id: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadType {
    Unit,
    Build,
}

impl PayloadType {
    pub fn id(self) -> u8 {
        match self {
            PayloadType::Unit => 0,
            PayloadType::Build => 1,
        }
    }

    fn from_id(type_id: u8) -> Option<Self> {
        match type_id {
            0 => Some(PayloadType::Unit),
            1 => Some(PayloadType::Build),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedPayload<TBuild, TUnit> {
    Null,
    Build(TBuild),
    Unit(TUnit),
}

impl<TBuild, TUnit> TypedPayload<TBuild, TUnit> {
    pub fn kind(&self) -> &'static str {
        match self {
            TypedPayload::Null => "null",
            TypedPayload::Build(_) => "build",
            TypedPayload::Unit(_) => "unit",
        }
    }

    pub fn payload_present(&self) -> bool {
        !matches!(self, TypedPayload::Null)
    }
}

pub type PayloadHeader = TypedPayload<BuildPayloadHeader, UnitPayloadHeader>;

impl PayloadHeader {
    pub fn payload_type(&self) -> Option<PayloadType> {
        match self {
            TypedPayload::Null => None,
            TypedPayload::Build(_) => Some(PayloadType::Build),
            TypedPayload::Unit(_) => Some(PayloadType::Unit),
        }
    }

    pub fn summary(&self, prefix_len: usize) -> PayloadSummary {
        PayloadSummary {
            kind: self.kind(),
            payload_present: self.payload_present(),
            payload_type: self.payload_type(),
            prefix_len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadSummary {
    pub kind: &'static str,
    pub payload_present: bool,
    pub payload_type: Option<PayloadType>,
    pub prefix_len: usize,
}

impl PayloadSummary {
    pub fn payload_type_id(&self) -> Option<u8> {
        self.payload_type.map(PayloadType::id)
    }
}

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

pub fn unpack_point2(packed: i32) -> (i32, i32) {
    (((packed >> 16) as i16) as i32, (packed as i16) as i32)
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
    write_rules_json(out, RULES_BASIC_JSON);
}

pub fn write_rules_json(out: &mut Vec<u8>, value: &str) {
    write_length_prefixed_json(out, value);
}

pub fn write_objectives_json(out: &mut Vec<u8>, value: &str) {
    write_length_prefixed_json(out, value);
}

pub fn write_objective_marker_json(out: &mut Vec<u8>, value: &str) {
    write_length_prefixed_json(out, value);
}

pub fn write_payload_null(out: &mut Vec<u8>) {
    write_payload_header(out, &TypedPayload::Null);
}

pub fn write_payload_unit_header(out: &mut Vec<u8>, class_id: u8) {
    write_payload_header(out, &TypedPayload::Unit(UnitPayloadHeader { class_id }));
}

pub fn write_payload_build_header(out: &mut Vec<u8>, block_id_raw: u16, build_revision: u8) {
    write_payload_header(
        out,
        &TypedPayload::Build(BuildPayloadHeader {
            block_id_raw,
            build_revision,
        }),
    );
}

pub fn write_payload_header(out: &mut Vec<u8>, value: &PayloadHeader) {
    match value {
        TypedPayload::Null => write_bool(out, false),
        TypedPayload::Unit(header) => {
            write_bool(out, true);
            write_byte(out, PayloadType::Unit.id());
            write_byte(out, header.class_id);
        }
        TypedPayload::Build(header) => {
            write_bool(out, true);
            write_byte(out, PayloadType::Build.id());
            out.extend_from_slice(&header.block_id_raw.to_be_bytes());
            write_byte(out, header.build_revision);
        }
    }
}

pub fn read_bool(bytes: &[u8]) -> Result<bool, TypeIoReadError> {
    let (value, consumed) = read_bool_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_bool_prefix(bytes: &[u8]) -> Result<(bool, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_u8()? != 0, reader.position()))
}

pub fn read_byte(bytes: &[u8]) -> Result<u8, TypeIoReadError> {
    let (value, consumed) = read_byte_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_byte_prefix(bytes: &[u8]) -> Result<(u8, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_u8()?, reader.position()))
}

pub fn read_short(bytes: &[u8]) -> Result<i16, TypeIoReadError> {
    let (value, consumed) = read_short_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_short_prefix(bytes: &[u8]) -> Result<(i16, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_i16()?, reader.position()))
}

pub fn read_int(bytes: &[u8]) -> Result<i32, TypeIoReadError> {
    let (value, consumed) = read_int_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_int_prefix(bytes: &[u8]) -> Result<(i32, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_i32()?, reader.position()))
}

pub fn read_float(bytes: &[u8]) -> Result<f32, TypeIoReadError> {
    let (value, consumed) = read_float_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_float_prefix(bytes: &[u8]) -> Result<(f32, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_f32()?, reader.position()))
}

pub fn read_string(bytes: &[u8]) -> Result<Option<String>, TypeIoReadError> {
    let (value, consumed) = read_string_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_string_prefix(bytes: &[u8]) -> Result<(Option<String>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let marker = reader.read_u8()?;
    if marker == 0 {
        return Ok((None, reader.position()));
    }

    let len = reader.read_u16()? as usize;
    let string_position = reader.position();
    let raw = reader.read_vec(len)?;
    let value = String::from_utf8(raw).map_err(|error| TypeIoReadError::InvalidUtf8 {
        position: string_position,
        message: error.to_string(),
    })?;
    Ok((Some(value), reader.position()))
}

pub fn read_block(bytes: &[u8]) -> Result<i16, TypeIoReadError> {
    read_short(bytes)
}

pub fn read_block_prefix(bytes: &[u8]) -> Result<(i16, usize), TypeIoReadError> {
    read_short_prefix(bytes)
}

pub fn read_content(bytes: &[u8]) -> Result<(u8, i16), TypeIoReadError> {
    let (value, consumed) = read_content_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_content_prefix(bytes: &[u8]) -> Result<((u8, i16), usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let content_type = reader.read_u8()?;
    let content_id = reader.read_i16()?;
    Ok(((content_type, content_id), reader.position()))
}

pub fn read_team(bytes: &[u8]) -> Result<u8, TypeIoReadError> {
    read_byte(bytes)
}

pub fn read_team_prefix(bytes: &[u8]) -> Result<(u8, usize), TypeIoReadError> {
    read_byte_prefix(bytes)
}

pub fn read_tile(bytes: &[u8]) -> Result<(i32, i32), TypeIoReadError> {
    let (value, consumed) = read_tile_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_tile_prefix(bytes: &[u8]) -> Result<((i32, i32), usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let packed = reader.read_i32()?;
    Ok((unpack_point2(packed), reader.position()))
}

pub fn read_unit_null(bytes: &[u8]) -> Result<(u8, i32), TypeIoReadError> {
    let (value, consumed) = read_unit_null_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_unit_null_prefix(bytes: &[u8]) -> Result<((u8, i32), usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let marker = reader.read_u8()?;
    let unit_id = reader.read_i32()?;
    Ok(((marker, unit_id), reader.position()))
}

pub fn read_vec2(bytes: &[u8]) -> Result<(f32, f32), TypeIoReadError> {
    let (value, consumed) = read_vec2_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_vec2_prefix(bytes: &[u8]) -> Result<((f32, f32), usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    Ok(((x, y), reader.position()))
}

pub fn read_rules_json(bytes: &[u8]) -> Result<String, TypeIoReadError> {
    let (value, consumed) = read_rules_json_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_rules_json_prefix(bytes: &[u8]) -> Result<(String, usize), TypeIoReadError> {
    read_length_prefixed_json_prefix(bytes, "rules length")
}

pub fn read_objectives_json(bytes: &[u8]) -> Result<String, TypeIoReadError> {
    let (value, consumed) = read_objectives_json_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_objectives_json_prefix(bytes: &[u8]) -> Result<(String, usize), TypeIoReadError> {
    read_length_prefixed_json_prefix(bytes, "objectives length")
}

pub fn read_objective_marker_json(bytes: &[u8]) -> Result<String, TypeIoReadError> {
    let (value, consumed) = read_objective_marker_json_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_objective_marker_json_prefix(bytes: &[u8]) -> Result<(String, usize), TypeIoReadError> {
    read_length_prefixed_json_prefix(bytes, "objective marker length")
}

pub fn read_payload_header(bytes: &[u8]) -> Result<PayloadHeader, TypeIoReadError> {
    let (value, consumed) = read_payload_header_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_payload_header_prefix(bytes: &[u8]) -> Result<(PayloadHeader, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let payload_present = reader.read_u8()? != 0;
    if !payload_present {
        return Ok((TypedPayload::Null, reader.position()));
    }

    let type_position = reader.position();
    let payload_type = reader.read_u8()?;
    let value = match PayloadType::from_id(payload_type) {
        Some(PayloadType::Unit) => TypedPayload::Unit(UnitPayloadHeader {
            class_id: reader.read_u8()?,
        }),
        Some(PayloadType::Build) => TypedPayload::Build(BuildPayloadHeader {
            block_id_raw: reader.read_u16()?,
            build_revision: reader.read_u8()?,
        }),
        None => {
            return Err(TypeIoReadError::UnsupportedPayloadType {
                type_id: payload_type,
                position: type_position,
            })
        }
    };
    Ok((value, reader.position()))
}

pub fn read_payload_summary(bytes: &[u8]) -> Result<PayloadSummary, TypeIoReadError> {
    let (value, consumed) = read_payload_summary_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_payload_summary_prefix(
    bytes: &[u8],
) -> Result<(PayloadSummary, usize), TypeIoReadError> {
    let (value, consumed) = read_payload_header_prefix(bytes)?;
    Ok((value.summary(consumed), consumed))
}

fn write_length_prefixed_json(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    write_int(out, bytes.len() as i32);
    out.extend_from_slice(bytes);
}

fn read_length_prefixed_json_prefix(
    bytes: &[u8],
    field: &'static str,
) -> Result<(String, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let length_position = reader.position();
    let len = reader.read_i32()?;
    if len < 0 {
        return Err(TypeIoReadError::NegativeLength {
            field,
            length: len,
            position: length_position,
        });
    }
    let string_position = reader.position();
    let raw = reader.read_vec(len as usize)?;
    let value = String::from_utf8(raw).map_err(|error| TypeIoReadError::InvalidUtf8 {
        position: string_position,
        message: error.to_string(),
    })?;
    Ok((value, reader.position()))
}

fn ensure_consumed(consumed: usize, total: usize) -> Result<(), TypeIoReadError> {
    if consumed == total {
        Ok(())
    } else {
        Err(TypeIoReadError::TrailingBytes { consumed, total })
    }
}

struct PrimitiveReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> PrimitiveReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn read_u8(&mut self) -> Result<u8, TypeIoReadError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, TypeIoReadError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_i16(&mut self) -> Result<i16, TypeIoReadError> {
        let bytes = self.read_exact(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32(&mut self) -> Result<i32, TypeIoReadError> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_f32(&mut self) -> Result<f32, TypeIoReadError> {
        Ok(f32::from_bits(self.read_i32()? as u32))
    }

    fn read_vec(&mut self, len: usize) -> Result<Vec<u8>, TypeIoReadError> {
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], TypeIoReadError> {
        let remaining = self.remaining();
        if remaining < len {
            return Err(TypeIoReadError::UnexpectedEof {
                position: self.position,
                needed: len,
                remaining,
            });
        }
        let start = self.position;
        self.position += len;
        Ok(&self.bytes[start..self.position])
    }
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
    write_objectives_json(&mut bytes, OBJECTIVES_BASIC_JSON);
    samples.insert("objectives.basic", encode_hex(&bytes));

    bytes.clear();
    write_objective_marker_json(&mut bytes, OBJECTIVE_MARKER_BASIC_JSON);
    samples.insert("objectiveMarker.basic", encode_hex(&bytes));

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
        assert!(text.contains("objectives.basic="));
        assert!(text.contains("objectiveMarker.basic="));
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
    fn objectives_and_objective_marker_lengths_match_payloads() {
        let mut bytes = Vec::new();
        write_objectives_json(&mut bytes, OBJECTIVES_BASIC_JSON);
        let declared = i32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(declared, bytes.len() - 4);
        assert_eq!(&bytes[4..], OBJECTIVES_BASIC_JSON.as_bytes());

        bytes.clear();
        write_objective_marker_json(&mut bytes, OBJECTIVE_MARKER_BASIC_JSON);
        let declared = i32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(declared, bytes.len() - 4);
        assert_eq!(&bytes[4..], OBJECTIVE_MARKER_BASIC_JSON.as_bytes());
    }

    #[test]
    fn basic_codec_readers_round_trip_expected_payloads() {
        let mut bytes = Vec::new();
        write_bool(&mut bytes, true);
        assert_eq!(read_bool(&bytes).unwrap(), true);

        bytes.clear();
        write_byte(&mut bytes, 7);
        assert_eq!(read_byte(&bytes).unwrap(), 7);

        bytes.clear();
        write_short(&mut bytes, 301);
        assert_eq!(read_short(&bytes).unwrap(), 301);
        assert_eq!(read_block(&bytes).unwrap(), 301);

        bytes.clear();
        write_int(&mut bytes, 0x1122_3344);
        assert_eq!(read_int(&bytes).unwrap(), 0x1122_3344);

        bytes.clear();
        write_float(&mut bytes, 12.5);
        assert_eq!(read_float(&bytes).unwrap(), 12.5);

        bytes.clear();
        write_string(&mut bytes, Some("hello"));
        assert_eq!(read_string(&bytes).unwrap().as_deref(), Some("hello"));

        bytes.clear();
        write_string(&mut bytes, None);
        assert_eq!(read_string(&bytes).unwrap(), None);

        bytes.clear();
        write_content(&mut bytes, CONTENT_TYPE_BLOCK, CONVEYOR_BLOCK_ID);
        assert_eq!(
            read_content(&bytes).unwrap(),
            (CONTENT_TYPE_BLOCK, CONVEYOR_BLOCK_ID)
        );

        bytes.clear();
        write_team(&mut bytes, TEAM_SHARDED_ID);
        assert_eq!(read_team(&bytes).unwrap(), TEAM_SHARDED_ID);

        bytes.clear();
        write_tile(&mut bytes, -2, 17);
        assert_eq!(read_tile(&bytes).unwrap(), (-2, 17));

        bytes.clear();
        write_unit_null(&mut bytes);
        assert_eq!(read_unit_null(&bytes).unwrap(), (0, 0));

        bytes.clear();
        write_vec2(&mut bytes, 12.5, -3.25);
        assert_eq!(read_vec2(&bytes).unwrap(), (12.5, -3.25));

        bytes.clear();
        write_rules_basic(&mut bytes);
        assert_eq!(read_rules_json(&bytes).unwrap(), RULES_BASIC_JSON);

        bytes.clear();
        write_objectives_json(&mut bytes, OBJECTIVES_BASIC_JSON);
        assert_eq!(read_objectives_json(&bytes).unwrap(), OBJECTIVES_BASIC_JSON);

        bytes.clear();
        write_objective_marker_json(&mut bytes, OBJECTIVE_MARKER_BASIC_JSON);
        assert_eq!(
            read_objective_marker_json(&bytes).unwrap(),
            OBJECTIVE_MARKER_BASIC_JSON
        );
    }

    #[test]
    fn payload_header_codecs_round_trip_expected_variants() {
        let mut bytes = Vec::new();
        write_payload_null(&mut bytes);
        assert_eq!(read_payload_header(&bytes).unwrap(), TypedPayload::Null);
        assert_eq!(
            read_payload_summary(&bytes).unwrap(),
            PayloadSummary {
                kind: "null",
                payload_present: false,
                payload_type: None,
                prefix_len: 1,
            }
        );

        bytes.clear();
        write_payload_unit_header(&mut bytes, 26);
        assert_eq!(
            read_payload_header(&bytes).unwrap(),
            TypedPayload::Unit(UnitPayloadHeader { class_id: 26 })
        );
        assert_eq!(
            read_payload_summary(&bytes).unwrap(),
            PayloadSummary {
                kind: "unit",
                payload_present: true,
                payload_type: Some(PayloadType::Unit),
                prefix_len: 3,
            }
        );

        bytes.clear();
        write_payload_build_header(&mut bytes, 0x8123, 7);
        let header = read_payload_header(&bytes).unwrap();
        assert_eq!(
            header,
            TypedPayload::Build(BuildPayloadHeader {
                block_id_raw: 0x8123,
                build_revision: 7,
            })
        );
        match header {
            TypedPayload::Build(build) => assert_eq!(build.block_id_i16(), -32477),
            _ => unreachable!(),
        }
        assert_eq!(
            read_payload_summary(&bytes).unwrap(),
            PayloadSummary {
                kind: "build",
                payload_present: true,
                payload_type: Some(PayloadType::Build),
                prefix_len: 5,
            }
        );
    }

    #[test]
    fn unpack_point2_restores_signed_coordinates() {
        let packed = pack_point2(-10, 300);
        assert_eq!(unpack_point2(packed), (-10, 300));
    }

    #[test]
    fn basic_codec_prefix_readers_leave_trailing_bytes_untouched() {
        let mut bytes = Vec::new();
        write_string(&mut bytes, Some("abc"));
        bytes.extend_from_slice(&[0x99, 0x88]);

        let (text, consumed) = read_string_prefix(&bytes).unwrap();
        assert_eq!(text.as_deref(), Some("abc"));
        assert_eq!(consumed, bytes.len() - 2);
        assert!(matches!(
            read_string(&bytes),
            Err(TypeIoReadError::TrailingBytes {
                consumed,
                total
            }) if consumed == bytes.len() - 2 && total == bytes.len()
        ));
    }

    #[test]
    fn rules_reader_rejects_negative_lengths() {
        let bytes = (-1i32).to_be_bytes();
        assert!(matches!(
            read_rules_json(&bytes),
            Err(TypeIoReadError::NegativeLength {
                field: "rules length",
                length: -1,
                position: 0
            })
        ));
        assert!(matches!(
            read_objectives_json(&bytes),
            Err(TypeIoReadError::NegativeLength {
                field: "objectives length",
                length: -1,
                position: 0
            })
        ));
        assert!(matches!(
            read_objective_marker_json(&bytes),
            Err(TypeIoReadError::NegativeLength {
                field: "objective marker length",
                length: -1,
                position: 0
            })
        ));
    }

    #[test]
    fn payload_header_prefix_reader_leaves_body_bytes_untouched() {
        let mut bytes = Vec::new();
        write_payload_unit_header(&mut bytes, 43);
        bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc]);

        let (header, consumed) = read_payload_header_prefix(&bytes).unwrap();
        assert_eq!(
            header,
            TypedPayload::Unit(UnitPayloadHeader { class_id: 43 })
        );
        assert_eq!(consumed, 3);
        assert!(matches!(
            read_payload_header(&bytes),
            Err(TypeIoReadError::TrailingBytes {
                consumed: 3,
                total
            }) if total == bytes.len()
        ));
    }

    #[test]
    fn payload_header_reader_rejects_unknown_payload_type_ids() {
        let bytes = [1u8, 9u8];
        assert!(matches!(
            read_payload_header(&bytes),
            Err(TypeIoReadError::UnsupportedPayloadType {
                type_id: 9,
                position: 1
            })
        ));
    }

    #[test]
    fn typeio_goldens_are_duplicate_free() {
        let text = generate_typeio_goldens();
        assert_no_duplicate_text("typeio-goldens", &text);
    }
}
