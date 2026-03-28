use std::error::Error;
use std::fmt;

const MAX_ARRAY_LEN: usize = 1000;
const MAX_NORMAL_OBJECT_ARRAY_LEN: usize = 200;
const MAX_BYTE_ARRAY_LEN: usize = 40_000;
const MAX_SAFE_STRING_LEN: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeIoObject {
    Null,
    Int(i32),
    Long(i64),
    Float(f32),
    String(Option<String>),
    ContentRaw { content_type: u8, content_id: i16 },
    IntSeq(Vec<i32>),
    Point2 { x: i32, y: i32 },
    PackedPoint2Array(Vec<i32>),
    TechNodeRaw { content_type: u8, content_id: i16 },
    Bool(bool),
    Double(f64),
    BuildingPos(i32),
    LAccess(i16),
    Bytes(Vec<u8>),
    LegacyUnitCommandNull(u8),
    BoolArray(Vec<bool>),
    UnitId(i32),
    Vec2Array(Vec<(f32, f32)>),
    Vec2 { x: f32, y: f32 },
    Team(u8),
    IntArray(Vec<i32>),
    ObjectArray(Vec<TypeIoObject>),
    UnitCommand(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeIoSemanticRef {
    Content { content_type: u8, content_id: i16 },
    TechNode { content_type: u8, content_id: i16 },
    Unit { unit_id: i32 },
    Building { build_pos: i32 },
}

impl TypeIoSemanticRef {
    pub fn kind(&self) -> &'static str {
        match self {
            TypeIoSemanticRef::Content { .. } => "content",
            TypeIoSemanticRef::TechNode { .. } => "techNode",
            TypeIoSemanticRef::Unit { .. } => "unit",
            TypeIoSemanticRef::Building { .. } => "building",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeIoObjectMatch<'a> {
    pub value: &'a TypeIoObject,
    pub path: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeIoEffectSummaryBudget {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_array_entries: usize,
}

impl Default for TypeIoEffectSummaryBudget {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_nodes: 64,
            max_array_entries: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIoSemanticMatch {
    pub semantic_ref: TypeIoSemanticRef,
    pub path: Vec<usize>,
}

impl TypeIoSemanticMatch {
    pub fn kind(&self) -> &'static str {
        self.semantic_ref.kind()
    }

    fn from_object_match(matched: TypeIoObjectMatch<'_>) -> Option<Self> {
        Some(Self {
            semantic_ref: matched.value.semantic_ref()?,
            path: matched.path,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeIoEffectPositionHint {
    Point2 {
        x: i32,
        y: i32,
        path: Vec<usize>,
    },
    PackedPoint2ArrayFirst {
        packed_point2: i32,
        path: Vec<usize>,
    },
    Vec2 {
        x_bits: u32,
        y_bits: u32,
        path: Vec<usize>,
    },
    Vec2ArrayFirst {
        x_bits: u32,
        y_bits: u32,
        path: Vec<usize>,
    },
}

impl TypeIoEffectPositionHint {
    pub fn kind(&self) -> &'static str {
        match self {
            TypeIoEffectPositionHint::Point2 { .. } => "point2",
            TypeIoEffectPositionHint::PackedPoint2ArrayFirst { .. } => "point2ArrayFirst",
            TypeIoEffectPositionHint::Vec2 { .. } => "vec2",
            TypeIoEffectPositionHint::Vec2ArrayFirst { .. } => "vec2ArrayFirst",
        }
    }

    pub fn path(&self) -> &[usize] {
        match self {
            TypeIoEffectPositionHint::Point2 { path, .. }
            | TypeIoEffectPositionHint::PackedPoint2ArrayFirst { path, .. }
            | TypeIoEffectPositionHint::Vec2 { path, .. }
            | TypeIoEffectPositionHint::Vec2ArrayFirst { path, .. } => path.as_slice(),
        }
    }

    fn from_object_match(matched: TypeIoObjectMatch<'_>) -> Option<Self> {
        match matched.value {
            TypeIoObject::Point2 { x, y } => Some(TypeIoEffectPositionHint::Point2 {
                x: *x,
                y: *y,
                path: matched.path,
            }),
            TypeIoObject::PackedPoint2Array(values) => {
                let mut path = matched.path;
                let packed_point2 = *values.first()?;
                path.push(0);
                Some(TypeIoEffectPositionHint::PackedPoint2ArrayFirst {
                    packed_point2,
                    path,
                })
            }
            TypeIoObject::Vec2 { x, y } => Some(TypeIoEffectPositionHint::Vec2 {
                x_bits: x.to_bits(),
                y_bits: y.to_bits(),
                path: matched.path,
            }),
            TypeIoObject::Vec2Array(values) => {
                let mut path = matched.path;
                let (x, y) = values.first()?;
                path.push(0);
                Some(TypeIoEffectPositionHint::Vec2ArrayFirst {
                    x_bits: x.to_bits(),
                    y_bits: y.to_bits(),
                    path,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIoEffectSummary {
    pub kind: String,
    pub kind_truncated: bool,
    pub first_semantic_ref: Option<TypeIoSemanticMatch>,
    pub first_parent_ref: Option<TypeIoSemanticMatch>,
    pub first_position_hint: Option<TypeIoEffectPositionHint>,
}

impl TypeIoObject {
    pub fn kind(&self) -> &'static str {
        match self {
            TypeIoObject::Null => "null",
            TypeIoObject::Int(_) => "int",
            TypeIoObject::Long(_) => "long",
            TypeIoObject::Float(_) => "float",
            TypeIoObject::String(_) => "string",
            TypeIoObject::ContentRaw { .. } => "Content(raw)",
            TypeIoObject::IntSeq(_) => "IntSeq",
            TypeIoObject::Point2 { .. } => "Point2",
            TypeIoObject::PackedPoint2Array(_) => "Point2[]",
            TypeIoObject::TechNodeRaw { .. } => "TechNode(raw)",
            TypeIoObject::Bool(_) => "bool",
            TypeIoObject::Double(_) => "double",
            TypeIoObject::BuildingPos(_) => "Building(raw)",
            TypeIoObject::LAccess(_) => "LAccess",
            TypeIoObject::Bytes(_) => "byte[]",
            TypeIoObject::LegacyUnitCommandNull(_) => "LegacyUnitCommandNull",
            TypeIoObject::BoolArray(_) => "boolean[]",
            TypeIoObject::UnitId(_) => "Unit(raw)",
            TypeIoObject::Vec2Array(_) => "Vec2[]",
            TypeIoObject::Vec2 { .. } => "Vec2",
            TypeIoObject::Team(_) => "Team",
            TypeIoObject::IntArray(_) => "int[]",
            TypeIoObject::ObjectArray(_) => "object[]",
            TypeIoObject::UnitCommand(_) => "UnitCommand",
        }
    }

    pub fn semantic_ref(&self) -> Option<TypeIoSemanticRef> {
        match self {
            TypeIoObject::ContentRaw {
                content_type,
                content_id,
            } => Some(TypeIoSemanticRef::Content {
                content_type: *content_type,
                content_id: *content_id,
            }),
            TypeIoObject::TechNodeRaw {
                content_type,
                content_id,
            } => Some(TypeIoSemanticRef::TechNode {
                content_type: *content_type,
                content_id: *content_id,
            }),
            TypeIoObject::UnitId(unit_id) => Some(TypeIoSemanticRef::Unit { unit_id: *unit_id }),
            TypeIoObject::BuildingPos(build_pos) => Some(TypeIoSemanticRef::Building {
                build_pos: *build_pos,
            }),
            _ => None,
        }
    }

    pub fn effect_summary(&self) -> TypeIoEffectSummary {
        self.effect_summary_bounded(TypeIoEffectSummaryBudget::default())
    }

    pub fn effect_summary_bounded(&self, budget: TypeIoEffectSummaryBudget) -> TypeIoEffectSummary {
        let mut remaining_kind_nodes = budget.max_nodes;
        let mut kind_truncated = false;
        let kind = self.effect_kind_summary_bounded(
            0,
            budget,
            &mut remaining_kind_nodes,
            &mut kind_truncated,
        );

        let first_semantic_ref = self
            .find_first_dfs_bounded(budget.max_depth, budget.max_nodes, |object| {
                object.semantic_ref().is_some()
            })
            .and_then(TypeIoSemanticMatch::from_object_match);

        let first_parent_ref = self
            .find_first_dfs_bounded(budget.max_depth, budget.max_nodes, |object| {
                matches!(
                    object.semantic_ref(),
                    Some(TypeIoSemanticRef::Unit { .. } | TypeIoSemanticRef::Building { .. })
                )
            })
            .and_then(TypeIoSemanticMatch::from_object_match);

        let first_position_hint = self
            .find_first_dfs_bounded(budget.max_depth, budget.max_nodes, |object| match object {
                TypeIoObject::Point2 { .. } | TypeIoObject::Vec2 { .. } => true,
                TypeIoObject::PackedPoint2Array(values) => !values.is_empty(),
                TypeIoObject::Vec2Array(values) => !values.is_empty(),
                _ => false,
            })
            .and_then(TypeIoEffectPositionHint::from_object_match);

        TypeIoEffectSummary {
            kind,
            kind_truncated,
            first_semantic_ref,
            first_parent_ref,
            first_position_hint,
        }
    }

    pub fn find_first_dfs<P>(&self, predicate: P) -> Option<TypeIoObjectMatch<'_>>
    where
        P: Fn(&TypeIoObject) -> bool,
    {
        self.find_first_dfs_impl(&predicate)
    }

    pub fn find_first_dfs_bounded<P>(
        &self,
        max_depth: usize,
        max_nodes: usize,
        predicate: P,
    ) -> Option<TypeIoObjectMatch<'_>>
    where
        P: Fn(&TypeIoObject) -> bool,
    {
        let mut remaining_nodes = max_nodes;
        self.find_first_dfs_bounded_impl(&predicate, 0, max_depth, &mut remaining_nodes)
    }

    fn find_first_dfs_impl<'a, P>(&'a self, predicate: &P) -> Option<TypeIoObjectMatch<'a>>
    where
        P: Fn(&TypeIoObject) -> bool,
    {
        if predicate(self) {
            return Some(TypeIoObjectMatch {
                value: self,
                path: Vec::new(),
            });
        }
        let TypeIoObject::ObjectArray(values) = self else {
            return None;
        };
        for (index, value) in values.iter().enumerate() {
            if let Some(mut matched) = value.find_first_dfs_impl(predicate) {
                matched.path.insert(0, index);
                return Some(matched);
            }
        }
        None
    }

    fn find_first_dfs_bounded_impl<'a, P>(
        &'a self,
        predicate: &P,
        depth: usize,
        max_depth: usize,
        remaining_nodes: &mut usize,
    ) -> Option<TypeIoObjectMatch<'a>>
    where
        P: Fn(&TypeIoObject) -> bool,
    {
        if *remaining_nodes == 0 {
            return None;
        }
        *remaining_nodes = remaining_nodes.saturating_sub(1);

        if predicate(self) {
            return Some(TypeIoObjectMatch {
                value: self,
                path: Vec::new(),
            });
        }
        if depth >= max_depth {
            return None;
        }

        let TypeIoObject::ObjectArray(values) = self else {
            return None;
        };
        for (index, value) in values.iter().enumerate() {
            if let Some(mut matched) =
                value.find_first_dfs_bounded_impl(predicate, depth + 1, max_depth, remaining_nodes)
            {
                matched.path.insert(0, index);
                return Some(matched);
            }
        }
        None
    }

    fn effect_kind_summary_bounded(
        &self,
        depth: usize,
        budget: TypeIoEffectSummaryBudget,
        remaining_nodes: &mut usize,
        truncated: &mut bool,
    ) -> String {
        if *remaining_nodes == 0 {
            *truncated = true;
            return "...".to_string();
        }
        *remaining_nodes = remaining_nodes.saturating_sub(1);

        let TypeIoObject::ObjectArray(values) = self else {
            return self.kind().to_string();
        };
        if depth >= budget.max_depth {
            if !values.is_empty() {
                *truncated = true;
            }
            return format!("object[len={}]", values.len());
        }

        let mut parts = Vec::new();
        let max_entries = values.len().min(budget.max_array_entries);
        let mut visited_entries = 0usize;
        for index in 0..max_entries {
            if *remaining_nodes == 0 {
                *truncated = true;
                parts.push(format!("{index}=..."));
                visited_entries = index + 1;
                break;
            }
            let value_summary = values[index].effect_kind_summary_bounded(
                depth + 1,
                budget,
                remaining_nodes,
                truncated,
            );
            parts.push(format!("{index}={value_summary}"));
            visited_entries = index + 1;
        }
        if values.len() > visited_entries {
            *truncated = true;
            parts.push(format!("+{}", values.len() - visited_entries));
        }
        format!("object[len={}]{{{}}}", values.len(), parts.join(","))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeIoReadError {
    UnexpectedEof {
        position: usize,
        needed: usize,
        remaining: usize,
    },
    UnsupportedType {
        type_id: u8,
        position: usize,
    },
    UnsupportedPayloadType {
        type_id: u8,
        position: usize,
    },
    InvalidStringMarker {
        marker: u8,
        position: usize,
    },
    InvalidBooleanByte {
        value: u8,
        position: usize,
    },
    NegativeLength {
        field: &'static str,
        length: i32,
        position: usize,
    },
    LengthLimitExceeded {
        field: &'static str,
        length: usize,
        max: usize,
        position: usize,
    },
    InvalidUtf8 {
        position: usize,
        message: String,
    },
    NestedArrayNotAllowed {
        type_id: u8,
        position: usize,
    },
    TrailingBytes {
        consumed: usize,
        total: usize,
    },
}

impl fmt::Display for TypeIoReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeIoReadError::UnexpectedEof {
                position,
                needed,
                remaining,
            } => {
                write!(
                    f,
                    "unexpected EOF at {position}: need {needed} bytes, only {remaining} remaining"
                )
            }
            TypeIoReadError::UnsupportedType { type_id, position } => {
                write!(
                    f,
                    "unsupported TypeIO object type id {type_id} at {position}"
                )
            }
            TypeIoReadError::UnsupportedPayloadType { type_id, position } => {
                write!(f, "unsupported payload type id {type_id} at {position}")
            }
            TypeIoReadError::InvalidStringMarker { marker, position } => {
                write!(f, "invalid string marker {marker} at {position}")
            }
            TypeIoReadError::InvalidBooleanByte { value, position } => {
                write!(f, "invalid boolean byte {value} at {position}")
            }
            TypeIoReadError::NegativeLength {
                field,
                length,
                position,
            } => {
                write!(f, "negative {field} {length} at {position}")
            }
            TypeIoReadError::LengthLimitExceeded {
                field,
                length,
                max,
                position,
            } => {
                write!(f, "{field} {length} exceeds max {max} at {position}")
            }
            TypeIoReadError::InvalidUtf8 { position, message } => {
                write!(f, "invalid UTF-8 at {position}: {message}")
            }
            TypeIoReadError::NestedArrayNotAllowed { type_id, position } => {
                write!(
                    f,
                    "nested array type id {type_id} is not allowed at {position}"
                )
            }
            TypeIoReadError::TrailingBytes { consumed, total } => {
                write!(
                    f,
                    "trailing bytes after TypeIO object: consumed {consumed} of {total}"
                )
            }
        }
    }
}

impl Error for TypeIoReadError {}

/// Write a single TypeIO object to `out`.
pub fn write_object(out: &mut Vec<u8>, value: &TypeIoObject) {
    match value {
        TypeIoObject::Null => out.push(0),
        TypeIoObject::Int(number) => {
            out.push(1);
            out.extend_from_slice(&number.to_be_bytes());
        }
        TypeIoObject::Long(number) => {
            out.push(2);
            out.extend_from_slice(&number.to_be_bytes());
        }
        TypeIoObject::Float(number) => {
            out.push(3);
            out.extend_from_slice(&number.to_bits().to_be_bytes());
        }
        TypeIoObject::String(value) => {
            out.push(4);
            match value {
                Some(text) => {
                    out.push(1);
                    let bytes = text.as_bytes();
                    let len: u16 = bytes.len().try_into().expect("TypeIO string too long");
                    out.extend_from_slice(&len.to_be_bytes());
                    out.extend_from_slice(bytes);
                }
                None => out.push(0),
            }
        }
        TypeIoObject::ContentRaw {
            content_type,
            content_id,
        } => {
            out.push(5);
            out.push(*content_type);
            out.extend_from_slice(&content_id.to_be_bytes());
        }
        TypeIoObject::IntSeq(values) => {
            out.push(6);
            let len: i16 = values
                .len()
                .try_into()
                .expect("TypeIO IntSeq too long for short length");
            out.extend_from_slice(&len.to_be_bytes());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        TypeIoObject::Point2 { x, y } => {
            out.push(7);
            out.extend_from_slice(&x.to_be_bytes());
            out.extend_from_slice(&y.to_be_bytes());
        }
        TypeIoObject::PackedPoint2Array(values) => {
            out.push(8);
            let len: u8 = values
                .len()
                .try_into()
                .expect("TypeIO Point2[] too long for unsigned byte length");
            out.push(len);
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        TypeIoObject::TechNodeRaw {
            content_type,
            content_id,
        } => {
            out.push(9);
            out.push(*content_type);
            out.extend_from_slice(&content_id.to_be_bytes());
        }
        TypeIoObject::Bool(value) => {
            out.push(10);
            out.push(u8::from(*value));
        }
        TypeIoObject::Double(value) => {
            out.push(11);
            out.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        TypeIoObject::BuildingPos(pos) => {
            out.push(12);
            out.extend_from_slice(&pos.to_be_bytes());
        }
        TypeIoObject::LAccess(ordinal) => {
            out.push(13);
            out.extend_from_slice(&ordinal.to_be_bytes());
        }
        TypeIoObject::Bytes(bytes) => {
            out.push(14);
            let len: i32 = bytes.len().try_into().expect("TypeIO byte[] too long");
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(bytes);
        }
        TypeIoObject::LegacyUnitCommandNull(raw) => {
            out.push(15);
            out.push(*raw);
        }
        TypeIoObject::BoolArray(values) => {
            out.push(16);
            let len: i32 = values.len().try_into().expect("TypeIO boolean[] too long");
            out.extend_from_slice(&len.to_be_bytes());
            for value in values {
                out.push(u8::from(*value));
            }
        }
        TypeIoObject::UnitId(unit_id) => {
            out.push(17);
            out.extend_from_slice(&unit_id.to_be_bytes());
        }
        TypeIoObject::Vec2Array(values) => {
            out.push(18);
            let len: i16 = values
                .len()
                .try_into()
                .expect("TypeIO Vec2[] too long for short length");
            out.extend_from_slice(&len.to_be_bytes());
            for (x, y) in values {
                out.extend_from_slice(&x.to_bits().to_be_bytes());
                out.extend_from_slice(&y.to_bits().to_be_bytes());
            }
        }
        TypeIoObject::Vec2 { x, y } => {
            out.push(19);
            out.extend_from_slice(&x.to_bits().to_be_bytes());
            out.extend_from_slice(&y.to_bits().to_be_bytes());
        }
        TypeIoObject::Team(id) => {
            out.push(20);
            out.push(*id);
        }
        TypeIoObject::IntArray(values) => {
            out.push(21);
            let len: i16 = values
                .len()
                .try_into()
                .expect("TypeIO int[] too long for short length");
            out.extend_from_slice(&len.to_be_bytes());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        TypeIoObject::ObjectArray(values) => {
            out.push(22);
            let len: i32 = values.len().try_into().expect("TypeIO object[] too long");
            out.extend_from_slice(&len.to_be_bytes());
            for value in values {
                write_object(out, value);
            }
        }
        TypeIoObject::UnitCommand(id) => {
            out.push(23);
            out.extend_from_slice(&id.to_be_bytes());
        }
    }
}

/// Read a single TypeIO object from `bytes`.
/// Returns an error when extra trailing bytes remain.
pub fn read_object(bytes: &[u8]) -> Result<TypeIoObject, TypeIoReadError> {
    let (value, consumed) = read_object_prefix(bytes)?;
    if consumed != bytes.len() {
        return Err(TypeIoReadError::TrailingBytes {
            consumed,
            total: bytes.len(),
        });
    }
    Ok(value)
}

/// Read a single TypeIO object using Java's "safe" limits.
pub fn read_object_safe(bytes: &[u8]) -> Result<TypeIoObject, TypeIoReadError> {
    let (value, consumed) = read_object_safe_prefix(bytes)?;
    if consumed != bytes.len() {
        return Err(TypeIoReadError::TrailingBytes {
            consumed,
            total: bytes.len(),
        });
    }
    Ok(value)
}

/// Read a single TypeIO object using the safe length limits while allowing nested arrays.
pub fn read_object_effect(bytes: &[u8]) -> Result<TypeIoObject, TypeIoReadError> {
    let (value, consumed) = read_object_effect_prefix(bytes)?;
    if consumed != bytes.len() {
        return Err(TypeIoReadError::TrailingBytes {
            consumed,
            total: bytes.len(),
        });
    }
    Ok(value)
}

/// Read one TypeIO object from the beginning of `bytes`.
/// Returns `(value, consumed_bytes)` and leaves trailing bytes untouched.
pub fn read_object_prefix(bytes: &[u8]) -> Result<(TypeIoObject, usize), TypeIoReadError> {
    let mut reader = Reader::new(bytes);
    let value = read_object_from_reader(&mut reader, ObjectReadOptions::normal())?;
    Ok((value, reader.position()))
}

/// Read one TypeIO object from the beginning of `bytes` using Java's "safe" limits.
pub fn read_object_safe_prefix(bytes: &[u8]) -> Result<(TypeIoObject, usize), TypeIoReadError> {
    let mut reader = Reader::new(bytes);
    let value = read_object_from_reader(&mut reader, ObjectReadOptions::safe())?;
    Ok((value, reader.position()))
}

/// Read one TypeIO object from the beginning of `bytes` for effect payloads.
/// This keeps Java-safe length caps while tolerating nested array payloads used by effects.
pub fn read_object_effect_prefix(bytes: &[u8]) -> Result<(TypeIoObject, usize), TypeIoReadError> {
    let mut reader = Reader::new(bytes);
    let value = read_object_from_reader(&mut reader, ObjectReadOptions::effect_safe())?;
    Ok((value, reader.position()))
}

#[derive(Debug, Clone, Copy)]
struct ObjectReadOptions {
    max_array_len: usize,
    max_string_len: Option<usize>,
    allow_arrays: bool,
    allow_nested_arrays: bool,
}

impl ObjectReadOptions {
    fn normal() -> Self {
        Self {
            max_array_len: MAX_NORMAL_OBJECT_ARRAY_LEN,
            max_string_len: None,
            allow_arrays: true,
            allow_nested_arrays: false,
        }
    }

    fn safe() -> Self {
        Self {
            max_array_len: MAX_ARRAY_LEN,
            max_string_len: Some(MAX_SAFE_STRING_LEN),
            allow_arrays: true,
            allow_nested_arrays: false,
        }
    }

    fn effect_safe() -> Self {
        Self {
            max_array_len: MAX_ARRAY_LEN,
            max_string_len: Some(MAX_SAFE_STRING_LEN),
            allow_arrays: true,
            allow_nested_arrays: true,
        }
    }

    fn nested_value(self) -> Self {
        Self {
            allow_arrays: self.allow_nested_arrays,
            ..self
        }
    }
}

fn read_object_from_reader(
    reader: &mut Reader<'_>,
    options: ObjectReadOptions,
) -> Result<TypeIoObject, TypeIoReadError> {
    let type_position = reader.position();
    let type_id = reader.read_u8()?;
    match type_id {
        0 => Ok(TypeIoObject::Null),
        1 => Ok(TypeIoObject::Int(reader.read_i32()?)),
        2 => Ok(TypeIoObject::Long(reader.read_i64()?)),
        3 => Ok(TypeIoObject::Float(reader.read_f32()?)),
        4 => read_string_value(reader, options.max_string_len),
        5 => Ok(TypeIoObject::ContentRaw {
            content_type: reader.read_u8()?,
            content_id: reader.read_i16()?,
        }),
        6 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_non_negative_i16_len(reader, "IntSeq length")?;
            ensure_length_limit(
                "IntSeq length",
                len,
                options.max_array_len,
                type_position + 1,
            )?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(reader.read_i32()?);
            }
            Ok(TypeIoObject::IntSeq(values))
        }
        7 => Ok(TypeIoObject::Point2 {
            x: reader.read_i32()?,
            y: reader.read_i32()?,
        }),
        8 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_u8_len(reader, "Point2[] length")?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(reader.read_i32()?);
            }
            Ok(TypeIoObject::PackedPoint2Array(values))
        }
        9 => Ok(TypeIoObject::TechNodeRaw {
            content_type: reader.read_u8()?,
            content_id: reader.read_i16()?,
        }),
        10 => Ok(TypeIoObject::Bool(read_binary_bool(reader)?)),
        11 => Ok(TypeIoObject::Double(reader.read_f64()?)),
        12 => Ok(TypeIoObject::BuildingPos(reader.read_i32()?)),
        13 => Ok(TypeIoObject::LAccess(reader.read_i16()?)),
        14 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_non_negative_i32_len(reader, "byte[] length")?;
            ensure_length_limit("byte[] length", len, MAX_BYTE_ARRAY_LEN, type_position + 1)?;
            Ok(TypeIoObject::Bytes(reader.read_vec(len)?))
        }
        15 => Ok(TypeIoObject::LegacyUnitCommandNull(reader.read_u8()?)),
        16 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_non_negative_i32_len(reader, "boolean[] length")?;
            ensure_length_limit(
                "boolean[] length",
                len,
                options.max_array_len,
                type_position + 1,
            )?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(read_binary_bool(reader)?);
            }
            Ok(TypeIoObject::BoolArray(values))
        }
        17 => Ok(TypeIoObject::UnitId(reader.read_i32()?)),
        18 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_non_negative_i16_len(reader, "Vec2[] length")?;
            ensure_length_limit(
                "Vec2[] length",
                len,
                options.max_array_len,
                type_position + 1,
            )?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push((reader.read_f32()?, reader.read_f32()?));
            }
            Ok(TypeIoObject::Vec2Array(values))
        }
        19 => Ok(TypeIoObject::Vec2 {
            x: reader.read_f32()?,
            y: reader.read_f32()?,
        }),
        20 => Ok(TypeIoObject::Team(reader.read_u8()?)),
        21 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_non_negative_i16_len(reader, "int[] length")?;
            ensure_length_limit(
                "int[] length",
                len,
                options.max_array_len,
                type_position + 1,
            )?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(reader.read_i32()?);
            }
            Ok(TypeIoObject::IntArray(values))
        }
        22 => {
            ensure_arrays_allowed(type_id, type_position, options)?;
            let len = read_non_negative_i32_len(reader, "object[] length")?;
            ensure_length_limit(
                "object[] length",
                len,
                options.max_array_len,
                type_position + 1,
            )?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(read_object_from_reader(reader, options.nested_value())?);
            }
            Ok(TypeIoObject::ObjectArray(values))
        }
        23 => Ok(TypeIoObject::UnitCommand(reader.read_u16()?)),
        _ => Err(TypeIoReadError::UnsupportedType {
            type_id,
            position: type_position,
        }),
    }
}

fn read_string_value(
    reader: &mut Reader<'_>,
    max_len: Option<usize>,
) -> Result<TypeIoObject, TypeIoReadError> {
    let marker_position = reader.position();
    let marker = reader.read_u8()?;
    match marker {
        0 => Ok(TypeIoObject::String(None)),
        1 => {
            let len = reader.read_u16()? as usize;
            if let Some(max_len) = max_len {
                ensure_length_limit("string length", len, max_len, reader.position() - 2)?;
            }
            let string_position = reader.position();
            let bytes = reader.read_vec(len)?;
            let value = String::from_utf8(bytes).map_err(|e| TypeIoReadError::InvalidUtf8 {
                position: string_position,
                message: e.to_string(),
            })?;
            Ok(TypeIoObject::String(Some(value)))
        }
        _ => Err(TypeIoReadError::InvalidStringMarker {
            marker,
            position: marker_position,
        }),
    }
}

fn read_binary_bool(reader: &mut Reader<'_>) -> Result<bool, TypeIoReadError> {
    let position = reader.position();
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(TypeIoReadError::InvalidBooleanByte { value, position }),
    }
}

fn read_u8_len(
    reader: &mut Reader<'_>,
    _field: &'static str,
) -> Result<usize, TypeIoReadError> {
    Ok(reader.read_u8()? as usize)
}

fn read_non_negative_i16_len(
    reader: &mut Reader<'_>,
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

fn ensure_arrays_allowed(
    type_id: u8,
    position: usize,
    options: ObjectReadOptions,
) -> Result<(), TypeIoReadError> {
    if options.allow_arrays {
        Ok(())
    } else {
        Err(TypeIoReadError::NestedArrayNotAllowed { type_id, position })
    }
}

fn ensure_length_limit(
    field: &'static str,
    length: usize,
    max: usize,
    position: usize,
) -> Result<(), TypeIoReadError> {
    if length > max {
        Err(TypeIoReadError::LengthLimitExceeded {
            field,
            length,
            max,
            position,
        })
    } else {
        Ok(())
    }
}

fn read_non_negative_i32_len(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<usize, TypeIoReadError> {
    let position = reader.position();
    let len = reader.read_i32()?;
    if len < 0 {
        return Err(TypeIoReadError::NegativeLength {
            field,
            length: len,
            position,
        });
    }
    Ok(len as usize)
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
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

    fn read_i64(&mut self) -> Result<i64, TypeIoReadError> {
        let bytes = self.read_exact(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f32(&mut self) -> Result<f32, TypeIoReadError> {
        Ok(f32::from_bits(self.read_i32()? as u32))
    }

    fn read_f64(&mut self) -> Result<f64, TypeIoReadError> {
        Ok(f64::from_bits(self.read_i64()? as u64))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn with_i32(mut bytes: Vec<u8>, value: i32) -> Vec<u8> {
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes
    }

    fn with_i16(mut bytes: Vec<u8>, value: i16) -> Vec<u8> {
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes
    }

    fn with_i64(mut bytes: Vec<u8>, value: i64) -> Vec<u8> {
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes
    }

    fn with_f32(mut bytes: Vec<u8>, value: f32) -> Vec<u8> {
        bytes.extend_from_slice(&value.to_bits().to_be_bytes());
        bytes
    }

    fn with_f64(mut bytes: Vec<u8>, value: f64) -> Vec<u8> {
        bytes.extend_from_slice(&value.to_bits().to_be_bytes());
        bytes
    }

    #[test]
    fn parses_supported_object_types() {
        let cases = vec![
            (vec![0], TypeIoObject::Null),
            (with_i32(vec![1], 123456), TypeIoObject::Int(123456)),
            (
                with_i64(vec![2], 0x0102_0304_0506_0708),
                TypeIoObject::Long(0x0102_0304_0506_0708),
            ),
            (with_f32(vec![3], 12.5), TypeIoObject::Float(12.5)),
            (
                vec![4, 1, 0, 3, b'a', b'b', b'c'],
                TypeIoObject::String(Some("abc".to_string())),
            ),
            (
                {
                    let mut bytes = vec![5, 1];
                    bytes.extend_from_slice(&0x0101i16.to_be_bytes());
                    bytes
                },
                TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 0x0101,
                },
            ),
            (
                {
                    let mut bytes = vec![6];
                    bytes.extend_from_slice(&3i16.to_be_bytes());
                    bytes.extend_from_slice(&1i32.to_be_bytes());
                    bytes.extend_from_slice(&2i32.to_be_bytes());
                    bytes.extend_from_slice(&3i32.to_be_bytes());
                    bytes
                },
                TypeIoObject::IntSeq(vec![1, 2, 3]),
            ),
            (
                with_i32(with_i32(vec![7], 3), 4),
                TypeIoObject::Point2 { x: 3, y: 4 },
            ),
            (
                {
                    let mut bytes = vec![8, 2];
                    bytes.extend_from_slice(&0x0001_0002i32.to_be_bytes());
                    bytes.extend_from_slice(&0x0003_0004i32.to_be_bytes());
                    bytes
                },
                TypeIoObject::PackedPoint2Array(vec![0x0001_0002, 0x0003_0004]),
            ),
            (
                {
                    let mut bytes = vec![9, 1];
                    bytes.extend_from_slice(&0x0102i16.to_be_bytes());
                    bytes
                },
                TypeIoObject::TechNodeRaw {
                    content_type: 1,
                    content_id: 0x0102,
                },
            ),
            (vec![10, 1], TypeIoObject::Bool(true)),
            (with_f64(vec![11], 12.5), TypeIoObject::Double(12.5)),
            (
                with_i32(vec![12], 0x0001_0002),
                TypeIoObject::BuildingPos(0x0001_0002),
            ),
            (
                {
                    let mut bytes = vec![13];
                    bytes.extend_from_slice(&5i16.to_be_bytes());
                    bytes
                },
                TypeIoObject::LAccess(5),
            ),
            (
                {
                    let mut bytes = vec![14];
                    bytes.extend_from_slice(&3i32.to_be_bytes());
                    bytes.extend_from_slice(&[1, 2, 3]);
                    bytes
                },
                TypeIoObject::Bytes(vec![1, 2, 3]),
            ),
            (vec![15, 0xab], TypeIoObject::LegacyUnitCommandNull(0xab)),
            (
                {
                    let mut bytes = vec![16];
                    bytes.extend_from_slice(&3i32.to_be_bytes());
                    bytes.extend_from_slice(&[1, 0, 1]);
                    bytes
                },
                TypeIoObject::BoolArray(vec![true, false, true]),
            ),
            (
                with_i32(vec![17], 0x0102_0304),
                TypeIoObject::UnitId(0x0102_0304),
            ),
            (
                {
                    let mut bytes = with_i16(vec![18], 2);
                    bytes.extend_from_slice(&(-1.5f32).to_bits().to_be_bytes());
                    bytes.extend_from_slice(&(2.5f32).to_bits().to_be_bytes());
                    bytes.extend_from_slice(&(3.25f32).to_bits().to_be_bytes());
                    bytes.extend_from_slice(&(-4.75f32).to_bits().to_be_bytes());
                    bytes
                },
                TypeIoObject::Vec2Array(vec![(-1.5, 2.5), (3.25, -4.75)]),
            ),
            (
                with_f32(with_f32(vec![19], -2.25), 1.5),
                TypeIoObject::Vec2 { x: -2.25, y: 1.5 },
            ),
            (vec![20, 7], TypeIoObject::Team(7)),
            (
                {
                    let mut bytes = vec![21];
                    bytes.extend_from_slice(&3i16.to_be_bytes());
                    bytes.extend_from_slice(&1i32.to_be_bytes());
                    bytes.extend_from_slice(&2i32.to_be_bytes());
                    bytes.extend_from_slice(&3i32.to_be_bytes());
                    bytes
                },
                TypeIoObject::IntArray(vec![1, 2, 3]),
            ),
            (
                {
                    let mut bytes = vec![22];
                    bytes.extend_from_slice(&3i32.to_be_bytes());
                    bytes.push(0);
                    bytes.extend_from_slice(&with_i32(vec![1], 7));
                    bytes.extend_from_slice(&[4, 1, 0, 2, b'o', b'k']);
                    bytes
                },
                TypeIoObject::ObjectArray(vec![
                    TypeIoObject::Null,
                    TypeIoObject::Int(7),
                    TypeIoObject::String(Some("ok".to_string())),
                ]),
            ),
            (
                {
                    let mut bytes = vec![23];
                    bytes.extend_from_slice(&42u16.to_be_bytes());
                    bytes
                },
                TypeIoObject::UnitCommand(42),
            ),
        ];

        for (bytes, expected) in cases {
            assert_eq!(read_object(&bytes).unwrap(), expected);
        }
    }

    #[test]
    fn parses_typeio_string_null_marker() {
        let value = read_object(&[4, 0]).unwrap();
        assert_eq!(value, TypeIoObject::String(None));
    }

    #[test]
    fn rejects_invalid_typeio_string_marker() {
        assert_eq!(
            read_object(&[4, 2]).unwrap_err(),
            TypeIoReadError::InvalidStringMarker {
                marker: 2,
                position: 1,
            }
        );
    }

    #[test]
    fn rejects_non_binary_boolean_bytes() {
        assert_eq!(
            read_object(&[10, 2]).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 2,
                position: 1,
            }
        );
        assert_eq!(
            read_object(&[16, 0, 0, 0, 2, 0, 3]).unwrap_err(),
            TypeIoReadError::InvalidBooleanByte {
                value: 3,
                position: 6,
            }
        );
    }

    #[test]
    fn reads_prefix_and_reports_trailing_bytes() {
        let bytes = [1, 0, 0, 0, 7, 0xff];
        let (value, consumed) = read_object_prefix(&bytes).unwrap();
        assert_eq!(value, TypeIoObject::Int(7));
        assert_eq!(consumed, 5);
        assert_eq!(
            read_object(&bytes).unwrap_err(),
            TypeIoReadError::TrailingBytes {
                consumed: 5,
                total: 6
            }
        );
    }

    #[test]
    fn rejects_unsupported_type() {
        assert_eq!(
            read_object(&[24]).unwrap_err(),
            TypeIoReadError::UnsupportedType {
                type_id: 24,
                position: 0
            }
        );
    }

    #[test]
    fn rejects_negative_lengths() {
        assert_eq!(
            read_object(&[14, 0xff, 0xff, 0xff, 0xff]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "byte[] length",
                length: -1,
                position: 1
            }
        );
        assert_eq!(
            read_object(&[21, 0xff, 0xff]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "int[] length",
                length: -1,
                position: 1
            }
        );
        assert_eq!(
            read_object(&[6, 0xff, 0xff]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "IntSeq length",
                length: -1,
                position: 1
            }
        );
        assert_eq!(
            read_object(&[22, 0xff, 0xff, 0xff, 0xff]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "object[] length",
                length: -1,
                position: 1
            }
        );
        assert_eq!(
            read_object(&[16, 0xff, 0xff, 0xff, 0xff]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "boolean[] length",
                length: -1,
                position: 1
            }
        );
        assert_eq!(
            read_object(&[18, 0xff, 0xff]).unwrap_err(),
            TypeIoReadError::NegativeLength {
                field: "Vec2[] length",
                length: -1,
                position: 1
            }
        );
    }

    #[test]
    fn write_object_serializes_point2_arrays_up_to_255() {
        let value = TypeIoObject::PackedPoint2Array(vec![0x0001_0002; 255]);
        let mut bytes = Vec::new();

        write_object(&mut bytes, &value);

        assert_eq!(bytes[0], 8);
        assert_eq!(bytes[1], 255);
        assert_eq!(bytes.len(), 2 + 255 * 4);
        assert_eq!(read_object(&bytes).unwrap(), value);
    }

    #[test]
    fn read_object_accepts_point2_arrays_up_to_255() {
        let mut bytes = vec![8, 255];
        let expected = TypeIoObject::PackedPoint2Array(vec![0x0001_0002; 255]);
        for _ in 0..255 {
            bytes.extend_from_slice(&0x0001_0002i32.to_be_bytes());
        }

        assert_eq!(read_object(&bytes).unwrap(), expected);
    }

    #[test]
    fn rejects_invalid_utf8() {
        let err = read_object(&[4, 1, 0, 2, 0xff, 0xff]).unwrap_err();
        match err {
            TypeIoReadError::InvalidUtf8 { position, .. } => assert_eq!(position, 4),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn safe_reader_rejects_strings_longer_than_v156_cap() {
        let mut bytes = vec![4, 1];
        bytes.extend_from_slice(&(1001u16).to_be_bytes());
        bytes.extend(vec![b'a'; 1001]);

        assert_eq!(
            read_object_safe(&bytes).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "string length",
                length: 1001,
                max: 1000,
                position: 2,
            }
        );
    }

    #[test]
    fn readers_reject_array_lengths_above_v156_caps() {
        let mut int_seq = vec![6];
        int_seq.extend_from_slice(&(201i16).to_be_bytes());
        assert_eq!(
            read_object(&int_seq).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "IntSeq length",
                length: 201,
                max: 200,
                position: 1,
            }
        );

        let mut safe_int_seq = vec![6];
        safe_int_seq.extend_from_slice(&(1001i16).to_be_bytes());
        assert_eq!(
            read_object_safe(&safe_int_seq).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "IntSeq length",
                length: 1001,
                max: 1000,
                position: 1,
            }
        );

        let mut bytes = vec![14];
        bytes.extend_from_slice(&(40001i32).to_be_bytes());
        assert_eq!(
            read_object_safe(&bytes).unwrap_err(),
            TypeIoReadError::LengthLimitExceeded {
                field: "byte[] length",
                length: 40001,
                max: 40000,
                position: 1,
            }
        );
    }

    #[test]
    fn readers_reject_nested_arrays_like_java_typeio() {
        let mut bytes = vec![22];
        bytes.extend_from_slice(&(1i32).to_be_bytes());
        bytes.push(21);
        bytes.extend_from_slice(&(1i16).to_be_bytes());
        bytes.extend_from_slice(&(7i32).to_be_bytes());

        assert_eq!(
            read_object(&bytes).unwrap_err(),
            TypeIoReadError::NestedArrayNotAllowed {
                type_id: 21,
                position: 5,
            }
        );
    }

    #[test]
    fn effect_reader_allows_nested_arrays_with_safe_limits() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::ObjectArray(vec![TypeIoObject::Point2 { x: 10, y: 20 }]),
                TypeIoObject::Vec2Array(vec![(5.5, -7.25), (9.0, 11.0)]),
            ]),
            TypeIoObject::Bool(true),
        ]);
        let mut bytes = Vec::new();
        write_object(&mut bytes, &value);

        let (decoded_prefix, consumed) = read_object_effect_prefix(&bytes).unwrap();
        assert_eq!(decoded_prefix, value);
        assert_eq!(consumed, bytes.len());
        assert_eq!(read_object_effect(&bytes).unwrap(), value);
    }

    #[test]
    fn serializes_supported_object_types() {
        let cases = vec![
            (TypeIoObject::Null, vec![0]),
            (TypeIoObject::Int(123456), with_i32(vec![1], 123456)),
            (
                TypeIoObject::Long(0x0102_0304_0506_0708),
                with_i64(vec![2], 0x0102_0304_0506_0708),
            ),
            (TypeIoObject::Float(12.5), with_f32(vec![3], 12.5)),
            (
                TypeIoObject::String(Some("abc".to_string())),
                vec![4, 1, 0, 3, b'a', b'b', b'c'],
            ),
            (TypeIoObject::String(None), vec![4, 0]),
            (
                TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 0x0101,
                },
                {
                    let mut bytes = vec![5, 1];
                    bytes.extend_from_slice(&0x0101i16.to_be_bytes());
                    bytes
                },
            ),
            (TypeIoObject::IntSeq(vec![1, 2, 3]), {
                let mut bytes = with_i16(vec![6], 3);
                bytes.extend_from_slice(&1i32.to_be_bytes());
                bytes.extend_from_slice(&2i32.to_be_bytes());
                bytes.extend_from_slice(&3i32.to_be_bytes());
                bytes
            }),
            (
                TypeIoObject::Point2 { x: 3, y: 4 },
                with_i32(with_i32(vec![7], 3), 4),
            ),
            (
                TypeIoObject::PackedPoint2Array(vec![0x0001_0002, 0x0003_0004]),
                {
                    let mut bytes = vec![8, 2];
                    bytes.extend_from_slice(&0x0001_0002i32.to_be_bytes());
                    bytes.extend_from_slice(&0x0003_0004i32.to_be_bytes());
                    bytes
                },
            ),
            (
                TypeIoObject::TechNodeRaw {
                    content_type: 1,
                    content_id: 0x0102,
                },
                {
                    let mut bytes = vec![9, 1];
                    bytes.extend_from_slice(&0x0102i16.to_be_bytes());
                    bytes
                },
            ),
            (TypeIoObject::Bool(true), vec![10, 1]),
            (TypeIoObject::Double(12.5), with_f64(vec![11], 12.5)),
            (
                TypeIoObject::BuildingPos(0x0001_0002),
                with_i32(vec![12], 0x0001_0002),
            ),
            (TypeIoObject::LAccess(5), {
                let mut bytes = vec![13];
                bytes.extend_from_slice(&5i16.to_be_bytes());
                bytes
            }),
            (TypeIoObject::Bytes(vec![1, 2, 3]), {
                let mut bytes = vec![14];
                bytes.extend_from_slice(&3i32.to_be_bytes());
                bytes.extend_from_slice(&[1, 2, 3]);
                bytes
            }),
            (TypeIoObject::LegacyUnitCommandNull(0xab), vec![15, 0xab]),
            (TypeIoObject::BoolArray(vec![true, false, true]), {
                let mut bytes = vec![16];
                bytes.extend_from_slice(&3i32.to_be_bytes());
                bytes.extend_from_slice(&[1, 0, 1]);
                bytes
            }),
            (
                TypeIoObject::UnitId(0x0102_0304),
                with_i32(vec![17], 0x0102_0304),
            ),
            (TypeIoObject::Vec2Array(vec![(-1.5, 2.5), (3.25, -4.75)]), {
                let mut bytes = with_i16(vec![18], 2);
                bytes.extend_from_slice(&(-1.5f32).to_bits().to_be_bytes());
                bytes.extend_from_slice(&(2.5f32).to_bits().to_be_bytes());
                bytes.extend_from_slice(&(3.25f32).to_bits().to_be_bytes());
                bytes.extend_from_slice(&(-4.75f32).to_bits().to_be_bytes());
                bytes
            }),
            (
                TypeIoObject::Vec2 { x: -2.25, y: 1.5 },
                with_f32(with_f32(vec![19], -2.25), 1.5),
            ),
            (TypeIoObject::Team(7), vec![20, 7]),
            (TypeIoObject::IntArray(vec![1, 2, 3]), {
                let mut bytes = with_i16(vec![21], 3);
                bytes.extend_from_slice(&1i32.to_be_bytes());
                bytes.extend_from_slice(&2i32.to_be_bytes());
                bytes.extend_from_slice(&3i32.to_be_bytes());
                bytes
            }),
            (
                TypeIoObject::ObjectArray(vec![
                    TypeIoObject::Null,
                    TypeIoObject::Int(7),
                    TypeIoObject::String(Some("ok".to_string())),
                ]),
                {
                    let mut bytes = vec![22];
                    bytes.extend_from_slice(&3i32.to_be_bytes());
                    bytes.push(0);
                    bytes.extend_from_slice(&with_i32(vec![1], 7));
                    bytes.extend_from_slice(&[4, 1, 0, 2, b'o', b'k']);
                    bytes
                },
            ),
            (TypeIoObject::UnitCommand(42), {
                let mut bytes = vec![23];
                bytes.extend_from_slice(&42u16.to_be_bytes());
                bytes
            }),
        ];

        for (value, expected_bytes) in cases {
            let mut out = Vec::new();
            write_object(&mut out, &value);
            assert_eq!(out, expected_bytes);
            assert_eq!(read_object(&out).unwrap(), value);
        }
    }

    #[test]
    fn find_first_dfs_matches_root_with_empty_path() {
        let value = TypeIoObject::UnitId(123);
        let matched = value
            .find_first_dfs(|object| matches!(object, TypeIoObject::UnitId(_)))
            .unwrap();
        assert_eq!(matched.value, &TypeIoObject::UnitId(123));
        assert!(matched.path.is_empty());
    }

    #[test]
    fn find_first_dfs_returns_leftmost_depth_first_nested_match_path() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::Null,
                TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]),
            ]),
            TypeIoObject::UnitId(11),
        ]);

        let matched = value
            .find_first_dfs(|object| matches!(object, TypeIoObject::UnitId(_)))
            .unwrap();
        assert_eq!(matched.value, &TypeIoObject::UnitId(7));
        assert_eq!(matched.path, vec![0, 1, 0]);
    }

    #[test]
    fn find_first_dfs_returns_none_when_no_match_exists() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Point2 { x: 1, y: 2 },
            TypeIoObject::ObjectArray(vec![TypeIoObject::Bool(true)]),
        ]);
        assert!(value
            .find_first_dfs(|object| matches!(object, TypeIoObject::UnitId(_)))
            .is_none());
    }

    #[test]
    fn find_first_dfs_bounded_finds_match_within_budget() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::Null,
                TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]),
            ]),
            TypeIoObject::UnitId(11),
        ]);

        let matched = value
            .find_first_dfs_bounded(3, 16, |object| matches!(object, TypeIoObject::UnitId(_)))
            .unwrap();
        assert_eq!(matched.value, &TypeIoObject::UnitId(7));
        assert_eq!(matched.path, vec![0, 1, 0]);
    }

    #[test]
    fn find_first_dfs_bounded_respects_depth_limit() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::Null,
                TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]),
            ]),
            TypeIoObject::UnitId(11),
        ]);
        assert!(value
            .find_first_dfs_bounded(1, 16, |object| {
                matches!(object, TypeIoObject::UnitId(unit_id) if *unit_id == 7)
            })
            .is_none());
    }

    #[test]
    fn find_first_dfs_bounded_respects_node_budget() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::Null,
                TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]),
            ]),
            TypeIoObject::UnitId(11),
        ]);

        assert!(value
            .find_first_dfs_bounded(3, 3, |object| matches!(object, TypeIoObject::UnitId(_)))
            .is_none());
        let matched = value
            .find_first_dfs_bounded(3, 6, |object| matches!(object, TypeIoObject::UnitId(_)))
            .unwrap();
        assert_eq!(matched.value, &TypeIoObject::UnitId(7));
    }

    #[test]
    fn semantic_ref_classifies_supported_reference_objects() {
        assert_eq!(
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 301,
            }
            .semantic_ref(),
            Some(TypeIoSemanticRef::Content {
                content_type: 1,
                content_id: 301,
            })
        );
        assert_eq!(
            TypeIoObject::TechNodeRaw {
                content_type: 2,
                content_id: 41,
            }
            .semantic_ref(),
            Some(TypeIoSemanticRef::TechNode {
                content_type: 2,
                content_id: 41,
            })
        );
        assert_eq!(
            TypeIoObject::UnitId(777).semantic_ref(),
            Some(TypeIoSemanticRef::Unit { unit_id: 777 })
        );
        assert_eq!(
            TypeIoObject::BuildingPos(0x0001_0002).semantic_ref(),
            Some(TypeIoSemanticRef::Building {
                build_pos: 0x0001_0002
            })
        );
    }

    #[test]
    fn semantic_ref_kind_is_stable_and_distinct() {
        assert_eq!(
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 1,
            }
            .semantic_ref()
            .unwrap()
            .kind(),
            "content"
        );
        assert_eq!(
            TypeIoObject::TechNodeRaw {
                content_type: 1,
                content_id: 1,
            }
            .semantic_ref()
            .unwrap()
            .kind(),
            "techNode"
        );
        assert_eq!(
            TypeIoObject::UnitId(1).semantic_ref().unwrap().kind(),
            "unit"
        );
        assert_eq!(
            TypeIoObject::BuildingPos(1).semantic_ref().unwrap().kind(),
            "building"
        );
    }

    #[test]
    fn semantic_ref_returns_none_for_non_reference_objects() {
        assert_eq!(TypeIoObject::Null.semantic_ref(), None);
        assert_eq!(TypeIoObject::Point2 { x: 1, y: 2 }.semantic_ref(), None);
        assert_eq!(
            TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]).semantic_ref(),
            None
        );
    }

    #[test]
    fn effect_summary_reports_stable_kind_semantic_parent_and_position_hints() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::Point2 { x: 10, y: 20 },
                TypeIoObject::Bool(true),
            ]),
            TypeIoObject::UnitId(4321),
        ]);

        let summary = value.effect_summary();

        assert_eq!(
            summary.kind,
            "object[len=3]{0=int,1=object[len=2]{0=Point2,1=bool},2=Unit(raw)}"
        );
        assert!(!summary.kind_truncated);
        assert_eq!(
            summary.first_semantic_ref,
            Some(TypeIoSemanticMatch {
                semantic_ref: TypeIoSemanticRef::Unit { unit_id: 4321 },
                path: vec![2],
            })
        );
        assert_eq!(
            summary.first_parent_ref,
            Some(TypeIoSemanticMatch {
                semantic_ref: TypeIoSemanticRef::Unit { unit_id: 4321 },
                path: vec![2],
            })
        );
        assert_eq!(
            summary.first_position_hint,
            Some(TypeIoEffectPositionHint::Point2 {
                x: 10,
                y: 20,
                path: vec![1, 0],
            })
        );
    }

    #[test]
    fn effect_summary_distinguishes_content_and_tech_node_refs() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::TechNodeRaw {
                content_type: 1,
                content_id: 100,
            },
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 101,
            },
        ]);

        let summary = value.effect_summary();

        assert_eq!(
            summary.kind,
            "object[len=2]{0=TechNode(raw),1=Content(raw)}"
        );
        assert!(!summary.kind_truncated);
        assert_eq!(
            summary.first_semantic_ref,
            Some(TypeIoSemanticMatch {
                semantic_ref: TypeIoSemanticRef::TechNode {
                    content_type: 1,
                    content_id: 100,
                },
                path: vec![0],
            })
        );
        assert_eq!(summary.first_parent_ref, None);
        assert_eq!(summary.first_position_hint, None);
    }

    #[test]
    fn effect_summary_bounded_respects_depth_and_entry_limits() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(
                7,
            )])]),
            TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(11)]),
        ]);

        let summary = value.effect_summary_bounded(TypeIoEffectSummaryBudget {
            max_depth: 1,
            max_nodes: 64,
            max_array_entries: 1,
        });

        assert_eq!(summary.kind, "object[len=2]{0=object[len=1],+1}");
        assert!(summary.kind_truncated);
        assert_eq!(summary.first_semantic_ref, None);
        assert_eq!(summary.first_parent_ref, None);
        assert_eq!(summary.first_position_hint, None);
    }

    #[test]
    fn effect_summary_bounded_respects_node_budget() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]),
            TypeIoObject::UnitId(11),
        ]);

        let summary = value.effect_summary_bounded(TypeIoEffectSummaryBudget {
            max_depth: 3,
            max_nodes: 2,
            max_array_entries: 4,
        });

        assert_eq!(summary.kind, "object[len=2]{0=object[len=1]{0=...},1=...}");
        assert!(summary.kind_truncated);
        assert_eq!(summary.first_semantic_ref, None);
        assert_eq!(summary.first_parent_ref, None);
    }

    #[test]
    fn effect_summary_default_reaches_depth_three_nested_semantic_refs() {
        let value = TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![
            TypeIoObject::ObjectArray(vec![TypeIoObject::UnitId(7)]),
        ])]);

        let summary = value.effect_summary();

        assert_eq!(
            summary.first_semantic_ref,
            Some(TypeIoSemanticMatch {
                semantic_ref: TypeIoSemanticRef::Unit { unit_id: 7 },
                path: vec![0, 0, 0],
            })
        );
    }

    #[test]
    fn effect_summary_position_hint_for_vec2_array_uses_first_element_path() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Bool(false),
            TypeIoObject::Vec2Array(vec![(1.5, -2.25)]),
        ]);

        let summary = value.effect_summary();
        let hint = summary.first_position_hint.clone().unwrap();

        assert_eq!(hint.kind(), "vec2ArrayFirst");
        assert_eq!(hint.path(), [1, 0]);
        assert_eq!(
            summary.first_position_hint,
            Some(TypeIoEffectPositionHint::Vec2ArrayFirst {
                x_bits: 1.5f32.to_bits(),
                y_bits: (-2.25f32).to_bits(),
                path: vec![1, 0],
            })
        );
    }

    #[test]
    fn effect_summary_position_hint_for_packed_point2_array_uses_first_element_path() {
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Bool(false),
            TypeIoObject::PackedPoint2Array(vec![0x0004_0006]),
        ]);

        let summary = value.effect_summary();
        let hint = summary.first_position_hint.clone().unwrap();

        assert_eq!(hint.kind(), "point2ArrayFirst");
        assert_eq!(hint.path(), [1, 0]);
        assert_eq!(
            summary.first_position_hint,
            Some(TypeIoEffectPositionHint::PackedPoint2ArrayFirst {
                packed_point2: 0x0004_0006,
                path: vec![1, 0],
            })
        );
    }
}
