use std::collections::BTreeMap;

mod object;
mod unit_sync;

pub use object::{
    read_object, read_object_effect, read_object_effect_prefix, read_object_prefix,
    read_object_safe, read_object_safe_prefix, write_object, TypeIoEffectPositionHint,
    TypeIoEffectSummary, TypeIoEffectSummaryBudget, TypeIoObject, TypeIoObjectMatch,
    TypeIoReadError, TypeIoSemanticMatch, TypeIoSemanticRef,
};
pub use unit_sync::{
    read_abilities, read_abilities_into, read_abilities_into_prefix, read_abilities_prefix,
    read_status_entries, read_status_entries_prefix, read_status_entry, read_status_entry_prefix,
    read_weapon_mounts, read_weapon_mounts_into, read_weapon_mounts_into_prefix,
    read_weapon_mounts_prefix, status_id_uses_dynamic_fields, status_name_uses_dynamic_fields,
    write_abilities, write_status_entries, write_status_entry, write_weapon_mounts, AbilityRaw,
    StatusDynamicFieldsRaw, StatusEntryRaw, WeaponMountRaw,
};

pub const CONVEYOR_BLOCK_ID: i16 = 0x0101;
pub const CONTENT_TYPE_BLOCK: u8 = 1;
pub const TEAM_SHARDED_ID: u8 = 1;
pub const RULES_BASIC_JSON: &str = "{teams:{},attackMode:true,buildSpeedMultiplier:2.5,attributes:{},objectives:[],tags:{mode:{class:java.lang.String,value:golden}}}";
pub const OBJECTIVES_BASIC_JSON: &str =
    "{objectives:[{type:Research,content:{type:item,id:1}},{type:DestroyBlock,position:[4,5],team:2}]}";
pub const OBJECTIVE_MARKER_BASIC_JSON: &str =
    "{type:ShapeText,x:12.5,y:-3.25,world:true,text:objective-ready}";
const MAX_PLANS_QUEUE_LEN: usize = 999;
const MAX_RULES_JSON_LEN: usize = 40_000;
const MAX_OBJECTIVES_JSON_LEN: usize = 60_000;
const MAX_OBJECTIVE_MARKER_JSON_LEN: usize = 40_000;

#[derive(Debug, Clone, PartialEq)]
pub struct BuildPlanRaw {
    pub breaking: bool,
    pub packed_position: i32,
    pub x: i32,
    pub y: i32,
    pub block_id: Option<i16>,
    pub rotation: u8,
    pub has_config: bool,
    pub config: TypeIoObject,
}

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
pub enum UnitRefRaw {
    Null,
    Block(i32),
    Standard(i32),
}

impl UnitRefRaw {
    pub fn kind(self) -> u8 {
        match self {
            UnitRefRaw::Null => 0,
            UnitRefRaw::Block(_) => 1,
            UnitRefRaw::Standard(_) => 2,
        }
    }

    pub fn value(self) -> i32 {
        match self {
            UnitRefRaw::Null => 0,
            UnitRefRaw::Block(value) | UnitRefRaw::Standard(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemStackRaw {
    pub item_id: Option<i16>,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidStackRaw {
    pub liquid_id: Option<i16>,
    pub amount: f32,
}

pub type ColorRaw = u32;
pub type StringsRaw = Vec<Option<String>>;
pub type StringArrayRaw = Vec<StringsRaw>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceInfoRaw {
    pub ip: Option<String>,
    pub uuid: Option<String>,
    pub locale: Option<String>,
    pub modded: bool,
    pub mobile: bool,
    pub times_joined: i32,
    pub times_kicked: i32,
    pub ips: StringsRaw,
    pub names: StringsRaw,
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

pub fn write_ints(out: &mut Vec<u8>, values: &[i32]) {
    write_short(
        out,
        i16::try_from(values.len()).expect("int[] length exceeds i16"),
    );
    for value in values {
        write_int(out, *value);
    }
}

pub fn write_bytes(out: &mut Vec<u8>, value: &[u8]) {
    write_short(
        out,
        i16::try_from(value.len()).expect("byte[] length exceeds i16"),
    );
    out.extend_from_slice(value);
}

pub fn write_long(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn write_float(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_bits().to_be_bytes());
}

pub fn write_color(out: &mut Vec<u8>, color_rgba: ColorRaw) {
    out.extend_from_slice(&color_rgba.to_be_bytes());
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

pub fn write_string_data(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            let bytes = value.as_bytes();
            write_short(
                out,
                i16::try_from(bytes.len()).expect("string data length exceeds i16"),
            );
            out.extend_from_slice(bytes);
        }
        None => write_short(out, -1),
    }
}

pub fn write_strings(out: &mut Vec<u8>, values: &[Option<String>]) {
    write_byte(
        out,
        u8::try_from(values.len()).expect("string count exceeds u8"),
    );
    for value in values {
        write_string(out, value.as_deref());
    }
}

pub fn write_strings_capped(out: &mut Vec<u8>, values: &[Option<String>], max_len: usize) {
    let capped_len = values.len().min(max_len);
    write_byte(
        out,
        u8::try_from(capped_len).expect("capped string count exceeds u8"),
    );
    for value in values.iter().take(capped_len) {
        write_string(out, value.as_deref());
    }
}

pub fn write_trace_info(out: &mut Vec<u8>, trace: &TraceInfoRaw) {
    write_string(out, trace.ip.as_deref());
    write_string(out, trace.uuid.as_deref());
    write_string(out, trace.locale.as_deref());
    write_bool(out, trace.modded);
    write_bool(out, trace.mobile);
    write_int(out, trace.times_joined);
    write_int(out, trace.times_kicked);
    write_strings_capped(out, &trace.ips, 12);
    write_strings_capped(out, &trace.names, 12);
}

pub fn write_string_array(out: &mut Vec<u8>, rows: &[StringsRaw]) {
    write_byte(
        out,
        u8::try_from(rows.len()).expect("string array row count exceeds u8"),
    );
    for row in rows {
        write_strings(out, row);
    }
}

pub fn write_block(out: &mut Vec<u8>, block_id: i16) {
    write_short(out, block_id);
}

pub fn write_content(out: &mut Vec<u8>, content_type: u8, content_id: i16) {
    write_byte(out, content_type);
    write_short(out, content_id);
}

pub fn write_item(out: &mut Vec<u8>, item_id: Option<i16>) {
    write_short(out, item_id.unwrap_or(-1));
}

pub fn write_liquid(out: &mut Vec<u8>, liquid_id: Option<i16>) {
    write_short(out, liquid_id.unwrap_or(-1));
}

pub fn write_item_stack(out: &mut Vec<u8>, stack: ItemStackRaw) {
    write_item(out, stack.item_id);
    write_int(out, stack.amount);
}

pub fn write_item_stacks(out: &mut Vec<u8>, stacks: &[ItemStackRaw]) {
    write_short(
        out,
        i16::try_from(stacks.len()).expect("item stack count exceeds i16"),
    );
    for stack in stacks {
        write_item_stack(out, *stack);
    }
}

pub fn write_liquid_stack(out: &mut Vec<u8>, stack: LiquidStackRaw) {
    write_liquid(out, stack.liquid_id);
    write_float(out, stack.amount);
}

pub fn write_liquid_stacks(out: &mut Vec<u8>, stacks: &[LiquidStackRaw]) {
    write_short(
        out,
        i16::try_from(stacks.len()).expect("liquid stack count exceeds i16"),
    );
    for stack in stacks {
        write_liquid_stack(out, *stack);
    }
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

pub fn write_unit_ref(out: &mut Vec<u8>, value: UnitRefRaw) {
    write_byte(out, value.kind());
    write_int(out, value.value());
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

pub fn read_plan(bytes: &[u8]) -> Result<BuildPlanRaw, TypeIoReadError> {
    let (value, consumed) = read_plan_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_plan_prefix(bytes: &[u8]) -> Result<(BuildPlanRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let breaking = read_binary_bool(&mut reader)?;
    let packed_position = reader.read_i32()?;
    let (x, y) = unpack_point2(packed_position);
    let value = if breaking {
        BuildPlanRaw {
            breaking: true,
            packed_position,
            x,
            y,
            block_id: None,
            rotation: 0,
            has_config: false,
            config: TypeIoObject::Null,
        }
    } else {
        let block_id = reader.read_i16()?;
        let rotation = reader.read_u8()?;
        let has_config = read_binary_bool(&mut reader)?;
        let config = if has_config {
            read_object_safe_from_reader(&mut reader)?
        } else {
            TypeIoObject::Null
        };
        BuildPlanRaw {
            breaking: false,
            packed_position,
            x,
            y,
            block_id: Some(block_id),
            rotation,
            has_config,
            config,
        }
    };
    Ok((value, reader.position()))
}

pub fn read_plans_queue_net(bytes: &[u8]) -> Result<Option<Vec<BuildPlanRaw>>, TypeIoReadError> {
    let (value, consumed) = read_plans_queue_net_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_plans_queue_net_prefix(
    bytes: &[u8],
) -> Result<(Option<Vec<BuildPlanRaw>>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let count_position = reader.position();
    let count = reader.read_i32()?;
    if count == -1 {
        return Ok((None, reader.position()));
    }
    if count < -1 {
        return Err(TypeIoReadError::NegativeLength {
            field: "plans queue length",
            length: count,
            position: count_position,
        });
    }

    let count = count as usize;
    if count > MAX_PLANS_QUEUE_LEN {
        return Err(TypeIoReadError::LengthLimitExceeded {
            field: "plans queue length",
            length: count,
            max: MAX_PLANS_QUEUE_LEN,
            position: count_position,
        });
    }

    let mut plans = Vec::with_capacity(count);
    for _ in 0..count {
        plans.push(read_plan_from_reader(&mut reader)?);
    }
    Ok((Some(plans), reader.position()))
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
    Ok((read_binary_bool(&mut reader)?, reader.position()))
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

pub fn read_ints(bytes: &[u8]) -> Result<Vec<i32>, TypeIoReadError> {
    let (value, consumed) = read_ints_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_ints_prefix(bytes: &[u8]) -> Result<(Vec<i32>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let count = read_non_negative_i16_len(&mut reader, "int[] length")?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(reader.read_i32()?);
    }
    Ok((values, reader.position()))
}

pub fn read_bytes(bytes: &[u8]) -> Result<Vec<u8>, TypeIoReadError> {
    let (value, consumed) = read_bytes_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_bytes_prefix(bytes: &[u8]) -> Result<(Vec<u8>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let count = read_non_negative_i16_len(&mut reader, "byte[] length")?;
    let value = reader.read_vec(count)?;
    Ok((value, reader.position()))
}

pub fn read_long(bytes: &[u8]) -> Result<i64, TypeIoReadError> {
    let (value, consumed) = read_long_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_long_prefix(bytes: &[u8]) -> Result<(i64, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_i64()?, reader.position()))
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

pub fn read_color(bytes: &[u8]) -> Result<ColorRaw, TypeIoReadError> {
    let (value, consumed) = read_color_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_color_prefix(bytes: &[u8]) -> Result<(ColorRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((reader.read_u32()?, reader.position()))
}

pub fn read_string(bytes: &[u8]) -> Result<Option<String>, TypeIoReadError> {
    let (value, consumed) = read_string_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_string_prefix(bytes: &[u8]) -> Result<(Option<String>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((read_string_from_reader(&mut reader)?, reader.position()))
}

pub fn read_string_data(bytes: &[u8]) -> Result<Option<String>, TypeIoReadError> {
    let (value, consumed) = read_string_data_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_string_data_prefix(bytes: &[u8]) -> Result<(Option<String>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    Ok((
        read_string_data_from_reader(&mut reader)?,
        reader.position(),
    ))
}

pub fn read_strings(bytes: &[u8]) -> Result<StringsRaw, TypeIoReadError> {
    let (value, consumed) = read_strings_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_strings_prefix(bytes: &[u8]) -> Result<(StringsRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let count = reader.read_u8()? as usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_string_from_reader(&mut reader)?);
    }
    Ok((values, reader.position()))
}

pub fn read_trace_info(bytes: &[u8]) -> Result<TraceInfoRaw, TypeIoReadError> {
    let (value, consumed) = read_trace_info_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_trace_info_prefix(bytes: &[u8]) -> Result<(TraceInfoRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let trace = read_trace_info_from_reader(&mut reader)?;
    Ok((trace, reader.position()))
}

pub fn read_string_array(bytes: &[u8]) -> Result<StringArrayRaw, TypeIoReadError> {
    let (value, consumed) = read_string_array_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_string_array_prefix(bytes: &[u8]) -> Result<(StringArrayRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let row_count = reader.read_u8()? as usize;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let row = read_strings_from_reader(&mut reader)?;
        rows.push(row);
    }
    Ok((rows, reader.position()))
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

pub fn read_item(bytes: &[u8]) -> Result<Option<i16>, TypeIoReadError> {
    let (value, consumed) = read_item_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_item_prefix(bytes: &[u8]) -> Result<(Option<i16>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let value = read_optional_content_id(&mut reader, "item id")?;
    Ok((value, reader.position()))
}

pub fn read_liquid(bytes: &[u8]) -> Result<Option<i16>, TypeIoReadError> {
    let (value, consumed) = read_liquid_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_liquid_prefix(bytes: &[u8]) -> Result<(Option<i16>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let value = read_optional_content_id(&mut reader, "liquid id")?;
    Ok((value, reader.position()))
}

pub fn read_item_stack(bytes: &[u8]) -> Result<ItemStackRaw, TypeIoReadError> {
    let (value, consumed) = read_item_stack_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_item_stack_prefix(bytes: &[u8]) -> Result<(ItemStackRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let item_id = read_optional_content_id(&mut reader, "item id")?;
    let amount = reader.read_i32()?;
    Ok((ItemStackRaw { item_id, amount }, reader.position()))
}

pub fn read_item_stacks(bytes: &[u8]) -> Result<Vec<ItemStackRaw>, TypeIoReadError> {
    let (value, consumed) = read_item_stacks_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_item_stacks_prefix(
    bytes: &[u8],
) -> Result<(Vec<ItemStackRaw>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let count = read_non_negative_i16_len(&mut reader, "item stack count")?;
    let mut stacks = Vec::with_capacity(count);
    for _ in 0..count {
        let item_id = read_optional_content_id(&mut reader, "item id")?;
        let amount = reader.read_i32()?;
        stacks.push(ItemStackRaw { item_id, amount });
    }
    Ok((stacks, reader.position()))
}

pub fn read_liquid_stack(bytes: &[u8]) -> Result<LiquidStackRaw, TypeIoReadError> {
    let (value, consumed) = read_liquid_stack_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_liquid_stack_prefix(bytes: &[u8]) -> Result<(LiquidStackRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let liquid_id = read_optional_content_id(&mut reader, "liquid id")?;
    let amount = reader.read_f32()?;
    Ok((LiquidStackRaw { liquid_id, amount }, reader.position()))
}

pub fn read_liquid_stacks(bytes: &[u8]) -> Result<Vec<LiquidStackRaw>, TypeIoReadError> {
    let (value, consumed) = read_liquid_stacks_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_liquid_stacks_prefix(
    bytes: &[u8],
) -> Result<(Vec<LiquidStackRaw>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let count = read_non_negative_i16_len(&mut reader, "liquid stack count")?;
    let mut stacks = Vec::with_capacity(count);
    for _ in 0..count {
        let liquid_id = read_optional_content_id(&mut reader, "liquid id")?;
        let amount = reader.read_f32()?;
        stacks.push(LiquidStackRaw { liquid_id, amount });
    }
    Ok((stacks, reader.position()))
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

pub fn read_unit_ref(bytes: &[u8]) -> Result<UnitRefRaw, TypeIoReadError> {
    let (value, consumed) = read_unit_ref_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_unit_ref_prefix(bytes: &[u8]) -> Result<(UnitRefRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let type_position = reader.position();
    let kind = reader.read_u8()?;
    let value = reader.read_i32()?;
    let unit_ref = match kind {
        0 => UnitRefRaw::Null,
        1 => UnitRefRaw::Block(value),
        2 => UnitRefRaw::Standard(value),
        _ => {
            return Err(TypeIoReadError::UnsupportedType {
                type_id: kind,
                position: type_position,
            })
        }
    };
    Ok((unit_ref, reader.position()))
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
    read_length_prefixed_json_prefix(bytes, "rules length", MAX_RULES_JSON_LEN)
}

pub fn read_objectives_json(bytes: &[u8]) -> Result<String, TypeIoReadError> {
    let (value, consumed) = read_objectives_json_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_objectives_json_prefix(bytes: &[u8]) -> Result<(String, usize), TypeIoReadError> {
    read_length_prefixed_json_prefix(bytes, "objectives length", MAX_OBJECTIVES_JSON_LEN)
}

pub fn read_objective_marker_json(bytes: &[u8]) -> Result<String, TypeIoReadError> {
    let (value, consumed) = read_objective_marker_json_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_objective_marker_json_prefix(bytes: &[u8]) -> Result<(String, usize), TypeIoReadError> {
    read_length_prefixed_json_prefix(
        bytes,
        "objective marker length",
        MAX_OBJECTIVE_MARKER_JSON_LEN,
    )
}

pub fn read_payload_header(bytes: &[u8]) -> Result<PayloadHeader, TypeIoReadError> {
    let (value, consumed) = read_payload_header_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(value)
}

pub fn read_payload_header_prefix(bytes: &[u8]) -> Result<(PayloadHeader, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let payload_present = read_binary_bool(&mut reader)?;
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
    write_length_prefixed_json_bytes(out, value.as_bytes());
}

fn write_length_prefixed_json_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_length_prefixed_json_len(out, bytes.len(), bytes);
}

fn write_length_prefixed_json_len(out: &mut Vec<u8>, len: usize, bytes: &[u8]) {
    let len: i32 = len.try_into().expect("length-prefixed json too long");
    write_int(out, len);
    out.extend_from_slice(bytes);
}

fn read_plan_from_reader(
    reader: &mut PrimitiveReader<'_>,
) -> Result<BuildPlanRaw, TypeIoReadError> {
    let breaking = read_binary_bool(reader)?;
    let packed_position = reader.read_i32()?;
    let (x, y) = unpack_point2(packed_position);
    if breaking {
        return Ok(BuildPlanRaw {
            breaking: true,
            packed_position,
            x,
            y,
            block_id: None,
            rotation: 0,
            has_config: false,
            config: TypeIoObject::Null,
        });
    }

    let block_id = reader.read_i16()?;
    let rotation = reader.read_u8()?;
    let has_config = read_binary_bool(reader)?;
    let config = if has_config {
        read_object_safe_from_reader(reader)?
    } else {
        TypeIoObject::Null
    };
    Ok(BuildPlanRaw {
        breaking: false,
        packed_position,
        x,
        y,
        block_id: Some(block_id),
        rotation,
        has_config,
        config,
    })
}

fn read_object_safe_from_reader(
    reader: &mut PrimitiveReader<'_>,
) -> Result<TypeIoObject, TypeIoReadError> {
    let position = reader.position();
    let (value, consumed) = read_object_safe_prefix(&reader.bytes[position..])?;
    let _ = reader.read_exact(consumed)?;
    Ok(value)
}

fn read_length_prefixed_json_prefix(
    bytes: &[u8],
    field: &'static str,
    max_len: usize,
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
    let len = len as usize;
    if len > max_len {
        return Err(TypeIoReadError::LengthLimitExceeded {
            field,
            length: len,
            max: max_len,
            position: length_position,
        });
    }
    let string_position = reader.position();
    let raw = reader.read_vec(len)?;
    let value = String::from_utf8(raw).map_err(|error| TypeIoReadError::InvalidUtf8 {
        position: string_position,
        message: error.to_string(),
    })?;
    Ok((value, reader.position()))
}

fn read_non_negative_i16_len(
    reader: &mut PrimitiveReader<'_>,
    field: &'static str,
) -> Result<usize, TypeIoReadError> {
    let position = reader.position();
    let len = reader.read_i16()?;
    if len < 0 {
        return Err(TypeIoReadError::NegativeLength {
            field,
            length: len as i32,
            position,
        });
    }
    Ok(len as usize)
}

fn read_binary_bool(reader: &mut PrimitiveReader<'_>) -> Result<bool, TypeIoReadError> {
    let position = reader.position();
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(TypeIoReadError::InvalidBooleanByte { value, position }),
    }
}

fn read_optional_content_id(
    reader: &mut PrimitiveReader<'_>,
    field: &'static str,
) -> Result<Option<i16>, TypeIoReadError> {
    let position = reader.position();
    match reader.read_i16()? {
        -1 => Ok(None),
        value if value >= 0 => Ok(Some(value)),
        value => Err(TypeIoReadError::NegativeLength {
            field,
            length: i32::from(value),
            position,
        }),
    }
}

fn read_capped_strings_from_reader(
    reader: &mut PrimitiveReader<'_>,
    field: &'static str,
    max: usize,
) -> Result<StringsRaw, TypeIoReadError> {
    let position = reader.position();
    let count = reader.read_u8()? as usize;
    if count > max {
        return Err(TypeIoReadError::LengthLimitExceeded {
            field,
            length: count,
            max,
            position,
        });
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_string_from_reader(reader)?);
    }
    Ok(values)
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

    fn read_u32(&mut self) -> Result<u32, TypeIoReadError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i64(&mut self) -> Result<i64, TypeIoReadError> {
        let bytes = self.read_exact(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
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

fn read_string_from_reader(
    reader: &mut PrimitiveReader<'_>,
) -> Result<Option<String>, TypeIoReadError> {
    let marker_position = reader.position();
    let marker = reader.read_u8()?;
    match marker {
        0 => Ok(None),
        1 => {
            let len = reader.read_u16()? as usize;
            let string_position = reader.position();
            let raw = reader.read_vec(len)?;
            let value = String::from_utf8(raw).map_err(|error| TypeIoReadError::InvalidUtf8 {
                position: string_position,
                message: error.to_string(),
            })?;
            Ok(Some(value))
        }
        _ => Err(TypeIoReadError::InvalidStringMarker {
            marker,
            position: marker_position,
        }),
    }
}

fn read_string_data_from_reader(
    reader: &mut PrimitiveReader<'_>,
) -> Result<Option<String>, TypeIoReadError> {
    let length_position = reader.position();
    let length = reader.read_i16()?;
    match length {
        -1 => Ok(None),
        0.. => {
            let raw = reader.read_vec(length as usize)?;
            let string_position = reader.position() - raw.len();
            let value = String::from_utf8(raw).map_err(|error| TypeIoReadError::InvalidUtf8 {
                position: string_position,
                message: error.to_string(),
            })?;
            Ok(Some(value))
        }
        _ => Err(TypeIoReadError::NegativeLength {
            field: "string data length",
            length: i32::from(length),
            position: length_position,
        }),
    }
}

fn read_strings_from_reader(
    reader: &mut PrimitiveReader<'_>,
) -> Result<StringsRaw, TypeIoReadError> {
    let count = reader.read_u8()? as usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_string_from_reader(reader)?);
    }
    Ok(values)
}

fn read_trace_info_from_reader(
    reader: &mut PrimitiveReader<'_>,
) -> Result<TraceInfoRaw, TypeIoReadError> {
    Ok(TraceInfoRaw {
        ip: read_string_from_reader(reader)?,
        uuid: read_string_from_reader(reader)?,
        locale: read_string_from_reader(reader)?,
        modded: read_binary_bool(reader)?,
        mobile: read_binary_bool(reader)?,
        times_joined: reader.read_i32()?,
        times_kicked: reader.read_i32()?,
        ips: read_capped_strings_from_reader(reader, "trace ips count", 12)?,
        names: read_capped_strings_from_reader(reader, "trace names count", 12)?,
    })
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
    write_long(&mut bytes, 0x0102_0304_0506_0708);
    samples.insert("long.basic", encode_hex(&bytes));

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
        assert!(text.contains("long.basic=0102030405060708"));
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
    fn length_prefixed_json_round_trip_preserves_payload() {
        let mut bytes = Vec::new();
        write_length_prefixed_json(&mut bytes, RULES_BASIC_JSON);

        assert_eq!(
            i32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize,
            RULES_BASIC_JSON.len()
        );
        assert_eq!(read_rules_json(&bytes).unwrap(), RULES_BASIC_JSON);
    }

    #[test]
    #[should_panic(expected = "length-prefixed json too long")]
    fn length_prefixed_json_rejects_lengths_outside_i32_range() {
        let mut bytes = Vec::new();
        write_length_prefixed_json_len(&mut bytes, i32::MAX as usize + 1, b"x");
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

        let ints = vec![0x1122_3344, -7, 0, i32::MAX];
        bytes.clear();
        write_ints(&mut bytes, &ints);
        assert_eq!(read_ints(&bytes).unwrap(), ints);

        let raw_bytes = vec![0x01, 0x23, 0x45, 0x67, 0x89];
        bytes.clear();
        write_bytes(&mut bytes, &raw_bytes);
        assert_eq!(read_bytes(&bytes).unwrap(), raw_bytes);

        bytes.clear();
        write_long(&mut bytes, -0x0102_0304_0506_0708);
        assert_eq!(read_long(&bytes).unwrap(), -0x0102_0304_0506_0708);

        bytes.clear();
        write_long(&mut bytes, 0x0102_0304_0506_0708);
        bytes.extend_from_slice(&[0xaa, 0xbb]);
        let (value, consumed) = read_long_prefix(&bytes).unwrap();
        assert_eq!(value, 0x0102_0304_0506_0708);
        assert_eq!(consumed, 8);
        assert!(matches!(
            read_long(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == 8 && total == 10
        ));

        bytes.clear();
        write_float(&mut bytes, 12.5);
        assert_eq!(read_float(&bytes).unwrap(), 12.5);

        bytes.clear();
        write_color(&mut bytes, 0x1122_3344);
        assert_eq!(read_color(&bytes).unwrap(), 0x1122_3344);

        bytes.clear();
        write_string(&mut bytes, Some("hello"));
        assert_eq!(read_string(&bytes).unwrap().as_deref(), Some("hello"));

        bytes.clear();
        write_string_data(&mut bytes, Some("data-hello"));
        assert_eq!(
            read_string_data(&bytes).unwrap().as_deref(),
            Some("data-hello")
        );

        bytes.clear();
        write_string(&mut bytes, None);
        assert_eq!(read_string(&bytes).unwrap(), None);

        bytes.clear();
        write_string_data(&mut bytes, None);
        assert_eq!(read_string_data(&bytes).unwrap(), None);

        bytes.clear();
        write_ints(&mut bytes, &[1, -2, 3]);
        bytes.push(0xaa);
        let (decoded_ints, consumed) = read_ints_prefix(&bytes).unwrap();
        assert_eq!(decoded_ints, vec![1, -2, 3]);
        assert_eq!(consumed, bytes.len() - 1);
        assert!(matches!(
            read_ints(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == bytes.len() - 1 && total == bytes.len()
        ));

        assert!(matches!(
            read_ints(&[0, 2, 0, 0, 0, 1, 0, 0, 0]),
            Err(TypeIoReadError::UnexpectedEof {
                position: 6,
                needed: 4,
                remaining: 3
            })
        ));
        assert_eq!(
            read_ints(&(-1i16).to_be_bytes()).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "int[] length",
                length: -1,
                position: 0,
            }
        );
        bytes.clear();
        write_bytes(&mut bytes, &[0xAA, 0xBB, 0xCC]);
        bytes.push(0xDD);
        let (decoded_bytes, consumed) = read_bytes_prefix(&bytes).unwrap();
        assert_eq!(decoded_bytes, vec![0xAA, 0xBB, 0xCC]);
        assert_eq!(consumed, bytes.len() - 1);
        assert!(matches!(
            read_bytes(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == bytes.len() - 1 && total == bytes.len()
        ));
        assert!(matches!(
            read_bytes(&[0, 3, 0xAA, 0xBB]),
            Err(TypeIoReadError::UnexpectedEof {
                position: 2,
                needed: 3,
                remaining: 2
            })
        ));
        assert_eq!(
            read_bytes(&(-1i16).to_be_bytes()).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "byte[] length",
                length: -1,
                position: 0,
            }
        );

        let strings = vec![Some("alpha".to_string()), None, Some("gamma".to_string())];
        bytes.clear();
        write_strings(&mut bytes, &strings);
        assert_eq!(read_strings(&bytes).unwrap(), strings);

        bytes.clear();
        write_strings_capped(&mut bytes, &strings, 2);
        assert_eq!(
            read_strings(&bytes).unwrap(),
            vec![Some("alpha".to_string()), None]
        );

        let trace = TraceInfoRaw {
            ip: Some("127.0.0.1".to_string()),
            uuid: Some("uuid-1".to_string()),
            locale: Some("zh_CN".to_string()),
            modded: true,
            mobile: false,
            times_joined: 7,
            times_kicked: 2,
            ips: (0..14)
                .map(|index| Some(format!("10.0.0.{index}")))
                .collect(),
            names: (0..14).map(|index| Some(format!("user-{index}"))).collect(),
        };
        bytes.clear();
        write_trace_info(&mut bytes, &trace);
        let decoded_trace = read_trace_info(&bytes).unwrap();
        assert_eq!(decoded_trace.ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(decoded_trace.uuid.as_deref(), Some("uuid-1"));
        assert_eq!(decoded_trace.locale.as_deref(), Some("zh_CN"));
        assert!(decoded_trace.modded);
        assert!(!decoded_trace.mobile);
        assert_eq!(decoded_trace.times_joined, 7);
        assert_eq!(decoded_trace.times_kicked, 2);
        assert_eq!(decoded_trace.ips.len(), 12);
        assert_eq!(decoded_trace.names.len(), 12);
        assert_eq!(
            decoded_trace.ips.first().and_then(|value| value.as_deref()),
            Some("10.0.0.0")
        );
        assert_eq!(
            decoded_trace
                .names
                .last()
                .and_then(|value| value.as_deref()),
            Some("user-11")
        );

        let string_array = vec![
            vec![Some("menu".to_string()), Some("play".to_string())],
            vec![None, Some("quit".to_string())],
        ];
        bytes.clear();
        write_string_array(&mut bytes, &string_array);
        assert_eq!(read_string_array(&bytes).unwrap(), string_array);

        bytes.clear();
        write_content(&mut bytes, CONTENT_TYPE_BLOCK, CONVEYOR_BLOCK_ID);
        assert_eq!(
            read_content(&bytes).unwrap(),
            (CONTENT_TYPE_BLOCK, CONVEYOR_BLOCK_ID)
        );

        bytes.clear();
        write_item(&mut bytes, Some(11));
        assert_eq!(read_item(&bytes).unwrap(), Some(11));

        bytes.clear();
        write_liquid(&mut bytes, None);
        assert_eq!(read_liquid(&bytes).unwrap(), None);

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
        write_unit_ref(&mut bytes, UnitRefRaw::Null);
        assert_eq!(read_unit_ref(&bytes).unwrap(), UnitRefRaw::Null);

        bytes.clear();
        write_unit_ref(&mut bytes, UnitRefRaw::Standard(123));
        assert_eq!(read_unit_ref(&bytes).unwrap(), UnitRefRaw::Standard(123));

        bytes.clear();
        write_unit_ref(&mut bytes, UnitRefRaw::Block(pack_point2(7, 8)));
        assert_eq!(
            read_unit_ref(&bytes).unwrap(),
            UnitRefRaw::Block(pack_point2(7, 8))
        );

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

        bytes.clear();
        write_plan_place(&mut bytes, 1, 2, 1, CONVEYOR_BLOCK_ID, 3, 4);
        assert_eq!(
            read_plan(&bytes).unwrap(),
            BuildPlanRaw {
                breaking: false,
                packed_position: pack_point2(1, 2),
                x: 1,
                y: 2,
                block_id: Some(CONVEYOR_BLOCK_ID),
                rotation: 1,
                has_config: true,
                config: TypeIoObject::Point2 { x: 3, y: 4 },
            }
        );

        bytes.clear();
        write_plan_break(&mut bytes, 5, 6);
        assert_eq!(
            read_plan(&bytes).unwrap(),
            BuildPlanRaw {
                breaking: true,
                packed_position: pack_point2(5, 6),
                x: 5,
                y: 6,
                block_id: None,
                rotation: 0,
                has_config: false,
                config: TypeIoObject::Null,
            }
        );

        bytes.clear();
        write_plans_queue_net(&mut bytes);
        let plans = read_plans_queue_net(&bytes).unwrap().unwrap();
        assert_eq!(plans.len(), 2);
        assert!(plans[0].has_config);
        assert!(plans[1].breaking);
    }

    #[test]
    fn bool_reader_rejects_non_binary_bytes() {
        assert_eq!(
            read_bool(&[2]).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 0,
            }
        );
    }

    #[test]
    fn plan_reader_rejects_non_binary_boolean_bytes() {
        let mut bytes = Vec::new();
        write_plan_break(&mut bytes, 5, 6);
        bytes[0] = 2;
        assert_eq!(
            read_plan(&bytes).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 0,
            }
        );

        bytes.clear();
        write_plan_place(&mut bytes, 1, 2, 1, CONVEYOR_BLOCK_ID, 3, 4);
        bytes[8] = 2;
        assert_eq!(
            read_plan(&bytes).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 8,
            }
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
    fn color_and_string_collection_prefix_readers_preserve_boundaries() {
        let mut bytes = Vec::new();
        write_color(&mut bytes, 0xaabb_ccdd);
        bytes.push(0xee);

        let (color, consumed) = read_color_prefix(&bytes).unwrap();
        assert_eq!(color, 0xaabb_ccdd);
        assert_eq!(consumed, 4);
        assert!(matches!(
            read_color(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed: 4, total })
                if total == bytes.len()
        ));

        let strings = vec![Some("alpha".to_string()), None, Some("gamma".to_string())];
        bytes.clear();
        write_strings(&mut bytes, &strings);
        bytes.push(0xdd);

        let (decoded_strings, consumed) = read_strings_prefix(&bytes).unwrap();
        assert_eq!(decoded_strings, strings);
        assert_eq!(consumed, bytes.len() - 1);
        assert!(matches!(
            read_strings(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == bytes.len() - 1 && total == bytes.len()
        ));

        let string_array = vec![
            vec![Some("menu".to_string()), Some("play".to_string())],
            vec![None, Some("quit".to_string())],
        ];
        bytes.clear();
        write_string_array(&mut bytes, &string_array);
        bytes.push(0xcc);

        let (decoded_rows, consumed) = read_string_array_prefix(&bytes).unwrap();
        assert_eq!(decoded_rows, string_array);
        assert_eq!(consumed, bytes.len() - 1);
        assert!(matches!(
            read_string_array(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == bytes.len() - 1 && total == bytes.len()
        ));

        let trace = TraceInfoRaw {
            ip: Some("127.0.0.1".to_string()),
            uuid: None,
            locale: Some("en_US".to_string()),
            modded: false,
            mobile: true,
            times_joined: 9,
            times_kicked: 3,
            ips: vec![Some("10.0.0.1".to_string())],
            names: vec![Some("runner".to_string())],
        };
        bytes.clear();
        write_trace_info(&mut bytes, &trace);
        bytes.push(0xbb);

        let (decoded_trace, consumed) = read_trace_info_prefix(&bytes).unwrap();
        assert_eq!(decoded_trace, trace);
        assert_eq!(consumed, bytes.len() - 1);
        assert!(matches!(
            read_trace_info(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == bytes.len() - 1 && total == bytes.len()
        ));

        bytes.clear();
        write_string_data(&mut bytes, Some("data"));
        bytes.push(0xaa);

        let (decoded_string_data, consumed) = read_string_data_prefix(&bytes).unwrap();
        assert_eq!(decoded_string_data.as_deref(), Some("data"));
        assert_eq!(consumed, bytes.len() - 1);
        assert!(matches!(
            read_string_data(&bytes),
            Err(TypeIoReadError::TrailingBytes { consumed, total })
                if consumed == bytes.len() - 1 && total == bytes.len()
        ));
    }

    #[test]
    fn string_collection_readers_reject_truncated_payloads() {
        assert!(matches!(
            read_strings(&[1, 1, 0]),
            Err(TypeIoReadError::UnexpectedEof {
                position: 2,
                needed: 2,
                remaining: 1
            })
        ));
        assert!(matches!(
            read_string_array(&[1, 1, 1, 1]),
            Err(TypeIoReadError::UnexpectedEof {
                position: 3,
                needed: 2,
                remaining: 1
            })
        ));
    }

    #[test]
    fn trace_info_reader_rejects_truncated_payloads() {
        let mut bytes = Vec::new();
        write_trace_info(
            &mut bytes,
            &TraceInfoRaw {
                ip: Some("127.0.0.1".to_string()),
                uuid: Some("uuid-1".to_string()),
                locale: Some("en_US".to_string()),
                modded: true,
                mobile: false,
                times_joined: 1,
                times_kicked: 0,
                ips: vec![Some("10.0.0.1".to_string())],
                names: vec![Some("runner".to_string())],
            },
        );
        bytes.pop();

        assert!(matches!(
            read_trace_info(&bytes),
            Err(TypeIoReadError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn trace_info_reader_rejects_counts_above_writer_cap() {
        let trace = TraceInfoRaw {
            ip: None,
            uuid: None,
            locale: None,
            modded: false,
            mobile: false,
            times_joined: 0,
            times_kicked: 0,
            ips: Vec::new(),
            names: Vec::new(),
        };
        let mut bytes = Vec::new();
        write_trace_info(&mut bytes, &trace);

        bytes[13] = 13;
        assert_eq!(
            read_trace_info(&bytes).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "trace ips count",
                length: 13,
                max: 12,
                position: 13,
            }
        );

        bytes[13] = 0;
        bytes[14] = 13;
        assert_eq!(
            read_trace_info(&bytes).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "trace names count",
                length: 13,
                max: 12,
                position: 14,
            }
        );
    }

    #[test]
    fn trace_info_reader_rejects_non_binary_boolean_bytes() {
        let trace = TraceInfoRaw {
            ip: None,
            uuid: None,
            locale: None,
            modded: false,
            mobile: false,
            times_joined: 0,
            times_kicked: 0,
            ips: Vec::new(),
            names: Vec::new(),
        };
        let mut bytes = Vec::new();
        write_trace_info(&mut bytes, &trace);

        bytes[3] = 2;
        assert_eq!(
            read_trace_info(&bytes).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 3,
            }
        );

        bytes[3] = 0;
        bytes[4] = 2;
        assert_eq!(
            read_trace_info(&bytes).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 4,
            }
        );
    }

    #[test]
    fn string_data_reader_rejects_truncated_negative_and_invalid_utf8_payloads() {
        assert!(matches!(
            read_string_data(&[0, 3, b'a', b'b']),
            Err(TypeIoReadError::UnexpectedEof {
                position: 2,
                needed: 3,
                remaining: 2
            })
        ));
        assert_eq!(
            read_string_data(&(-2i16).to_be_bytes()).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "string data length",
                length: -2,
                position: 0,
            }
        );
        assert!(matches!(
            read_string_data(&[0, 1, 0xFF]),
            Err(TypeIoReadError::InvalidUtf8 { position: 2, .. })
        ));
    }

    #[test]
    #[should_panic(expected = "int[] length exceeds i16")]
    fn write_ints_rejects_lengths_outside_i16_range() {
        let ints = vec![0i32; i16::MAX as usize + 1];
        let mut bytes = Vec::new();
        write_ints(&mut bytes, &ints);
    }

    #[test]
    #[should_panic(expected = "byte[] length exceeds i16")]
    fn write_bytes_rejects_lengths_outside_i16_range() {
        let bytes = vec![0u8; i16::MAX as usize + 1];
        let mut out = Vec::new();
        write_bytes(&mut out, &bytes);
    }

    #[test]
    #[should_panic(expected = "string data length exceeds i16")]
    fn write_string_data_rejects_lengths_outside_i16_range() {
        let text = "x".repeat(i16::MAX as usize + 1);
        let mut out = Vec::new();
        write_string_data(&mut out, Some(&text));
    }

    #[test]
    #[should_panic(expected = "string too long")]
    fn write_string_rejects_lengths_outside_u16_range() {
        let text = "x".repeat(u16::MAX as usize + 1);
        let mut out = Vec::new();
        write_string(&mut out, Some(&text));
    }

    #[test]
    #[should_panic(expected = "string count exceeds u8")]
    fn write_strings_rejects_lengths_outside_u8_range() {
        let strings = vec![None; u8::MAX as usize + 1];
        let mut out = Vec::new();
        write_strings(&mut out, &strings);
    }

    #[test]
    #[should_panic(expected = "string array row count exceeds u8")]
    fn write_string_array_rejects_lengths_outside_u8_range() {
        let rows = vec![Vec::<Option<String>>::new(); u8::MAX as usize + 1];
        let mut out = Vec::new();
        write_string_array(&mut out, &rows);
    }

    #[test]
    #[should_panic(expected = "capped string count exceeds u8")]
    fn write_strings_capped_rejects_lengths_outside_u8_range() {
        let strings = vec![None; u8::MAX as usize + 1];
        let mut out = Vec::new();
        write_strings_capped(&mut out, &strings, strings.len());
    }

    #[test]
    fn unit_ref_codec_handles_null_standard_block_and_truncation() {
        assert_eq!(read_unit_ref(&[0, 0, 0, 0, 0]).unwrap(), UnitRefRaw::Null);
        assert_eq!(
            read_unit_ref(&[2, 0, 0, 0, 7]).unwrap(),
            UnitRefRaw::Standard(7)
        );
        assert_eq!(
            read_unit_ref(&[1, 0, 1, 0, 2]).unwrap(),
            UnitRefRaw::Block(pack_point2(1, 2))
        );

        assert!(matches!(
            read_unit_ref(&[2, 0, 0, 0]).unwrap_err(),
            TypeIoReadError::UnexpectedEof {
                position: 1,
                needed: 4,
                remaining: 3
            }
        ));
    }

    #[test]
    fn item_and_liquid_stack_codecs_handle_empty_single_multiple_and_reader_edges() {
        let empty_items = Vec::<ItemStackRaw>::new();
        let mut item_bytes = Vec::new();
        write_item_stacks(&mut item_bytes, &empty_items);
        assert_eq!(read_item_stacks(&item_bytes).unwrap(), empty_items);

        let item_stacks = vec![
            ItemStackRaw {
                item_id: Some(3),
                amount: 7,
            },
            ItemStackRaw {
                item_id: None,
                amount: 11,
            },
        ];
        item_bytes.clear();
        write_item_stacks(&mut item_bytes, &item_stacks);
        assert_eq!(read_item_stacks(&item_bytes).unwrap(), item_stacks);

        let mut item_bytes_with_tail = item_bytes.clone();
        item_bytes_with_tail.push(0xff);
        assert!(matches!(
            read_item_stacks(&item_bytes_with_tail),
            Err(TypeIoReadError::TrailingBytes {
                consumed,
                total
            }) if consumed == item_bytes.len() && total == item_bytes.len() + 1
        ));
        assert!(matches!(
            read_item_stacks(&[0, 1, 0, 3, 0, 0, 0]),
            Err(TypeIoReadError::UnexpectedEof {
                position: 4,
                needed: 4,
                remaining: 3
            })
        ));

        let empty_liquids = Vec::<LiquidStackRaw>::new();
        let mut liquid_bytes = Vec::new();
        write_liquid_stacks(&mut liquid_bytes, &empty_liquids);
        assert_eq!(read_liquid_stacks(&liquid_bytes).unwrap(), empty_liquids);

        let liquid_stacks = vec![
            LiquidStackRaw {
                liquid_id: Some(5),
                amount: 1.5,
            },
            LiquidStackRaw {
                liquid_id: None,
                amount: 0.25,
            },
        ];
        liquid_bytes.clear();
        write_liquid_stacks(&mut liquid_bytes, &liquid_stacks);
        assert_eq!(read_liquid_stacks(&liquid_bytes).unwrap(), liquid_stacks);

        let mut liquid_bytes_with_tail = liquid_bytes.clone();
        liquid_bytes_with_tail.push(0xaa);
        assert!(matches!(
            read_liquid_stacks(&liquid_bytes_with_tail),
            Err(TypeIoReadError::TrailingBytes {
                consumed,
                total
            }) if consumed == liquid_bytes.len() && total == liquid_bytes.len() + 1
        ));
        assert!(matches!(
            read_liquid_stacks(&[0, 1, 0, 5, 0x3f, 0xc0, 0x00]),
            Err(TypeIoReadError::UnexpectedEof {
                position: 4,
                needed: 4,
                remaining: 3
            })
        ));
    }

    #[test]
    fn item_and_liquid_stack_readers_reject_negative_lengths() {
        assert!(matches!(
            read_item_stacks(&(-1i16).to_be_bytes()),
            Err(TypeIoReadError::NegativeLength {
                field: "item stack count",
                length: -1,
                position: 0
            })
        ));
        assert!(matches!(
            read_liquid_stacks(&(-1i16).to_be_bytes()),
            Err(TypeIoReadError::NegativeLength {
                field: "liquid stack count",
                length: -1,
                position: 0
            })
        ));
    }

    #[test]
    fn item_and_liquid_readers_reject_negative_ids_other_than_minus_one() {
        assert_eq!(
            read_item(&(-2i16).to_be_bytes()).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "item id",
                length: -2,
                position: 0,
            }
        );
        assert_eq!(
            read_liquid(&(-3i16).to_be_bytes()).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "liquid id",
                length: -3,
                position: 0,
            }
        );
        assert_eq!(
            read_item_stack(&[0xff, 0xfe, 0, 0, 0, 1]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "item id",
                length: -2,
                position: 0,
            }
        );
        assert_eq!(
            read_liquid_stack(&[0xff, 0xfd, 0x3f, 0x80, 0x00, 0x00]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "liquid id",
                length: -3,
                position: 0,
            }
        );
        assert_eq!(
            read_item_stacks(&[0, 1, 0xff, 0xfc, 0, 0, 0, 1]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "item id",
                length: -4,
                position: 2,
            }
        );
        assert_eq!(
            read_liquid_stacks(&[0, 1, 0xff, 0xfb, 0x3f, 0x80, 0x00, 0x00]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "liquid id",
                length: -5,
                position: 2,
            }
        );
    }

    #[test]
    #[should_panic(expected = "item stack count exceeds i16")]
    fn write_item_stacks_rejects_lengths_outside_i16_range() {
        let stacks = vec![
            ItemStackRaw {
                item_id: None,
                amount: 0,
            };
            i16::MAX as usize + 1
        ];
        let mut bytes = Vec::new();
        write_item_stacks(&mut bytes, &stacks);
    }

    #[test]
    #[should_panic(expected = "liquid stack count exceeds i16")]
    fn write_liquid_stacks_rejects_lengths_outside_i16_range() {
        let stacks = vec![
            LiquidStackRaw {
                liquid_id: None,
                amount: 0.0,
            };
            i16::MAX as usize + 1
        ];
        let mut bytes = Vec::new();
        write_liquid_stacks(&mut bytes, &stacks);
    }

    #[test]
    fn string_reader_rejects_invalid_markers() {
        assert!(matches!(
            read_string_prefix(&[2u8]),
            Err(TypeIoReadError::InvalidStringMarker {
                marker: 2,
                position: 0
            })
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
    fn json_readers_reject_lengths_above_v156_caps() {
        let too_large_rules = (MAX_RULES_JSON_LEN as i32 + 1).to_be_bytes();
        assert!(matches!(
            read_rules_json(&too_large_rules),
            Err(TypeIoReadError::LengthLimitExceeded {
                field: "rules length",
                length,
                max: MAX_RULES_JSON_LEN,
                position: 0
            }) if length == MAX_RULES_JSON_LEN + 1
        ));

        let too_large_objectives = (MAX_OBJECTIVES_JSON_LEN as i32 + 1).to_be_bytes();
        assert!(matches!(
            read_objectives_json(&too_large_objectives),
            Err(TypeIoReadError::LengthLimitExceeded {
                field: "objectives length",
                length,
                max: MAX_OBJECTIVES_JSON_LEN,
                position: 0
            }) if length == MAX_OBJECTIVES_JSON_LEN + 1
        ));

        let too_large_marker = (MAX_OBJECTIVE_MARKER_JSON_LEN as i32 + 1).to_be_bytes();
        assert!(matches!(
            read_objective_marker_json(&too_large_marker),
            Err(TypeIoReadError::LengthLimitExceeded {
                field: "objective marker length",
                length,
                max: MAX_OBJECTIVE_MARKER_JSON_LEN,
                position: 0
            }) if length == MAX_OBJECTIVE_MARKER_JSON_LEN + 1
        ));
    }

    #[test]
    fn plans_queue_reader_rejects_invalid_lengths() {
        assert_eq!(
            read_plans_queue_net(&(-2i32).to_be_bytes()).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "plans queue length",
                length: -2,
                position: 0,
            }
        );

        let too_many = (1000i32).to_be_bytes();
        assert_eq!(
            read_plans_queue_net(&too_many).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "plans queue length",
                length: 1000,
                max: 999,
                position: 0,
            }
        );
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
    fn payload_header_reader_rejects_non_binary_presence_flags() {
        assert_eq!(
            read_payload_header(&[2]).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 0,
            }
        );
    }

    #[test]
    fn typeio_goldens_are_duplicate_free() {
        let text = generate_typeio_goldens();
        assert_no_duplicate_text("typeio-goldens", &text);
    }
}
