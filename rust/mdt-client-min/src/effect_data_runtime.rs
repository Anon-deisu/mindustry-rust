use crate::effect_runtime::{effect_contract_name, RuntimeEffectContract};
use crate::session_state::{EffectBusinessContentKind, EffectDataSemantic};
use mdt_typeio::{TypeIoEffectPositionHint, TypeIoObject, TypeIoSemanticMatch, TypeIoSemanticRef};

const EFFECT_DATA_MAX_DEPTH: usize = 4;
const EFFECT_DATA_MAX_NODES: usize = 64;
const ITEM_CONTENT_TYPE: u8 = 0;
const BLOCK_CONTENT_TYPE: u8 = 1;
const UNIT_CONTENT_TYPE: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectDataBusinessTargetHint {
    SemanticRef(TypeIoSemanticMatch),
    PositionHint(TypeIoEffectPositionHint),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectDataBusinessHint {
    ContentRef {
        kind: EffectBusinessContentKind,
        content_type: u8,
        content_id: i16,
        path: Vec<usize>,
    },
    ParentRef {
        semantic_ref: TypeIoSemanticRef,
        path: Vec<usize>,
    },
    PositionHint(TypeIoEffectPositionHint),
    FloatBits {
        bits: u32,
        path: Vec<usize>,
    },
    Polyline {
        points: Vec<(u32, u32)>,
        path: Vec<usize>,
    },
    PayloadTargetContent {
        content_kind: EffectBusinessContentKind,
        content_type: u8,
        content_id: i16,
        content_path: Vec<usize>,
        target: EffectDataBusinessTargetHint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectDataBusinessInput {
    pub contract_name: Option<&'static str>,
    pub data_kind: Option<String>,
    pub semantic: Option<EffectDataSemantic>,
    pub primary: Option<EffectDataBusinessHint>,
    pub data_type_tag: Option<u8>,
    pub parse_failed: bool,
    pub parse_error: Option<String>,
}

pub fn effect_data_kind_label(object: &TypeIoObject) -> String {
    object.effect_summary().kind
}

pub fn derive_effect_data_semantic(
    object: Option<&TypeIoObject>,
    data_type_tag: Option<u8>,
    parse_failed: bool,
) -> Option<EffectDataSemantic> {
    let object = match object {
        Some(object) => object,
        None if parse_failed => return data_type_tag.map(EffectDataSemantic::OpaqueTypeTag),
        None => return None,
    };

    if let Some(semantic_ref) = object.semantic_ref() {
        let semantic = match semantic_ref {
            TypeIoSemanticRef::Content {
                content_type,
                content_id,
            } => EffectDataSemantic::ContentRaw {
                content_type,
                content_id,
            },
            TypeIoSemanticRef::TechNode {
                content_type,
                content_id,
            } => EffectDataSemantic::TechNodeRaw {
                content_type,
                content_id,
            },
            TypeIoSemanticRef::Unit { unit_id } => EffectDataSemantic::UnitId(unit_id),
            TypeIoSemanticRef::Building { build_pos } => EffectDataSemantic::BuildingPos(build_pos),
        };
        return Some(semantic);
    }

    match object {
        TypeIoObject::Null => Some(EffectDataSemantic::Null),
        TypeIoObject::Int(value) => Some(EffectDataSemantic::Int(*value)),
        TypeIoObject::Long(value) => Some(EffectDataSemantic::Long(*value)),
        TypeIoObject::Float(value) => Some(EffectDataSemantic::FloatBits(value.to_bits())),
        TypeIoObject::String(value) => Some(EffectDataSemantic::String(value.clone())),
        TypeIoObject::IntSeq(values) => Some(EffectDataSemantic::IntSeqLen(values.len())),
        TypeIoObject::Point2 { x, y } => Some(EffectDataSemantic::Point2 { x: *x, y: *y }),
        TypeIoObject::PackedPoint2Array(values) => {
            Some(EffectDataSemantic::PackedPoint2ArrayLen(values.len()))
        }
        TypeIoObject::Bool(value) => Some(EffectDataSemantic::Bool(*value)),
        TypeIoObject::Double(value) => Some(EffectDataSemantic::DoubleBits(value.to_bits())),
        TypeIoObject::LAccess(value) => Some(EffectDataSemantic::LAccess(*value)),
        TypeIoObject::Bytes(values) => Some(EffectDataSemantic::BytesLen(values.len())),
        TypeIoObject::LegacyUnitCommandNull(value) => {
            Some(EffectDataSemantic::LegacyUnitCommandNull(*value))
        }
        TypeIoObject::BoolArray(values) => Some(EffectDataSemantic::BoolArrayLen(values.len())),
        TypeIoObject::Vec2Array(values) => Some(EffectDataSemantic::Vec2ArrayLen(values.len())),
        TypeIoObject::Vec2 { x, y } => Some(EffectDataSemantic::Vec2 {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        }),
        TypeIoObject::Team(id) => Some(EffectDataSemantic::Team(*id)),
        TypeIoObject::IntArray(values) => Some(EffectDataSemantic::IntArrayLen(values.len())),
        TypeIoObject::ObjectArray(values) => Some(EffectDataSemantic::ObjectArrayLen(values.len())),
        TypeIoObject::UnitCommand(id) => Some(EffectDataSemantic::UnitCommand(*id)),
        TypeIoObject::ContentRaw { .. }
        | TypeIoObject::TechNodeRaw { .. }
        | TypeIoObject::BuildingPos(_)
        | TypeIoObject::UnitId(_) => None,
    }
}

pub fn derive_effect_data_business_input(
    effect_id: Option<i16>,
    object: Option<&TypeIoObject>,
    data_type_tag: Option<u8>,
    parse_failed: bool,
    parse_error: Option<&str>,
) -> EffectDataBusinessInput {
    EffectDataBusinessInput {
        contract_name: effect_contract_name(effect_id),
        data_kind: object.map(effect_data_kind_label),
        semantic: derive_effect_data_semantic(object, data_type_tag, parse_failed),
        primary: object.and_then(|object| derive_primary_business_hint(effect_id, object)),
        data_type_tag,
        parse_failed,
        parse_error: parse_error.map(str::to_string),
    }
}

fn derive_primary_business_hint(
    effect_id: Option<i16>,
    object: &TypeIoObject,
) -> Option<EffectDataBusinessHint> {
    match crate::effect_runtime::effect_contract(effect_id) {
        Some(contract) => derive_contract_business_hint(contract, object).or_else(|| {
            matches!(contract, RuntimeEffectContract::LightningPath)
                .then(|| derive_fallback_business_hint(object))
                .flatten()
        }),
        None => derive_fallback_business_hint(object),
    }
}

fn derive_contract_business_hint(
    contract: RuntimeEffectContract,
    object: &TypeIoObject,
) -> Option<EffectDataBusinessHint> {
    match contract {
        RuntimeEffectContract::LightningPath => lightning_polyline_hint(object),
        RuntimeEffectContract::PositionTarget
        | RuntimeEffectContract::PointBeam
        | RuntimeEffectContract::PointHit
        | RuntimeEffectContract::DrillSteam
        | RuntimeEffectContract::LegDestroy
        | RuntimeEffectContract::ShieldBreak => {
            summary_position_hint(object).or_else(|| summary_parent_hint(object))
        }
        RuntimeEffectContract::BlockContentIcon => {
            first_content_hint(object, |content_type| content_type == BLOCK_CONTENT_TYPE)
        }
        RuntimeEffectContract::ContentIcon => first_content_hint(object, |content_type| {
            matches!(content_type, BLOCK_CONTENT_TYPE | UNIT_CONTENT_TYPE)
        }),
        RuntimeEffectContract::PayloadTargetContent => payload_target_content_hint(object),
        RuntimeEffectContract::DropItem => {
            first_content_hint(object, |content_type| content_type == ITEM_CONTENT_TYPE)
        }
        RuntimeEffectContract::FloatLength => first_float_hint(object),
        RuntimeEffectContract::UnitParent => summary_parent_hint(object)
            .or_else(|| first_parent_semantic_match(object).map(parent_match_hint)),
    }
}

fn derive_fallback_business_hint(object: &TypeIoObject) -> Option<EffectDataBusinessHint> {
    summary_parent_hint(object)
        .or_else(|| first_semantic_match(object).map(semantic_match_hint))
        .or_else(|| summary_position_hint(object))
        .or_else(|| first_float_hint(object))
}

fn summary_parent_hint(object: &TypeIoObject) -> Option<EffectDataBusinessHint> {
    object
        .effect_summary()
        .first_parent_ref
        .map(parent_match_hint)
}

fn summary_position_hint(object: &TypeIoObject) -> Option<EffectDataBusinessHint> {
    object
        .effect_summary()
        .first_position_hint
        .map(EffectDataBusinessHint::PositionHint)
}

fn parent_match_hint(matched: TypeIoSemanticMatch) -> EffectDataBusinessHint {
    EffectDataBusinessHint::ParentRef {
        semantic_ref: matched.semantic_ref,
        path: matched.path,
    }
}

fn semantic_match_hint(matched: TypeIoSemanticMatch) -> EffectDataBusinessHint {
    match matched.semantic_ref {
        TypeIoSemanticRef::Content {
            content_type,
            content_id,
        } => EffectDataBusinessHint::ContentRef {
            kind: EffectBusinessContentKind::Content,
            content_type,
            content_id,
            path: matched.path,
        },
        TypeIoSemanticRef::TechNode {
            content_type,
            content_id,
        } => EffectDataBusinessHint::ContentRef {
            kind: EffectBusinessContentKind::TechNode,
            content_type,
            content_id,
            path: matched.path,
        },
        TypeIoSemanticRef::Building { .. } | TypeIoSemanticRef::Unit { .. } => {
            EffectDataBusinessHint::ParentRef {
                semantic_ref: matched.semantic_ref,
                path: matched.path,
            }
        }
    }
}

fn first_content_hint(
    object: &TypeIoObject,
    predicate: impl Fn(u8) -> bool,
) -> Option<EffectDataBusinessHint> {
    first_semantic_match(object).and_then(|matched| match matched.semantic_ref {
        TypeIoSemanticRef::Content {
            content_type,
            content_id,
        } if predicate(content_type) => Some(EffectDataBusinessHint::ContentRef {
            kind: EffectBusinessContentKind::Content,
            content_type,
            content_id,
            path: matched.path,
        }),
        TypeIoSemanticRef::TechNode { .. }
        | TypeIoSemanticRef::Content { .. }
        | TypeIoSemanticRef::Building { .. }
        | TypeIoSemanticRef::Unit { .. } => None,
    })
}

fn first_semantic_match(object: &TypeIoObject) -> Option<TypeIoSemanticMatch> {
    object.effect_summary().first_semantic_ref
}

fn first_parent_semantic_match(object: &TypeIoObject) -> Option<TypeIoSemanticMatch> {
    object.effect_summary().first_parent_ref
}

fn first_float_hint(object: &TypeIoObject) -> Option<EffectDataBusinessHint> {
    object
        .find_first_dfs_bounded(EFFECT_DATA_MAX_DEPTH, EFFECT_DATA_MAX_NODES, |value| {
            matches!(value, TypeIoObject::Float(_))
        })
        .and_then(|matched| match matched.value {
            TypeIoObject::Float(value) => Some(EffectDataBusinessHint::FloatBits {
                bits: value.to_bits(),
                path: matched.path,
            }),
            _ => None,
        })
}

fn lightning_polyline_hint(object: &TypeIoObject) -> Option<EffectDataBusinessHint> {
    object
        .find_first_dfs_bounded(EFFECT_DATA_MAX_DEPTH, EFFECT_DATA_MAX_NODES, |value| {
            matches!(value, TypeIoObject::Vec2Array(_))
        })
        .and_then(|matched| match matched.value {
            TypeIoObject::Vec2Array(values) => {
                let points = values
                    .iter()
                    .filter_map(|(x, y)| {
                        (x.is_finite() && y.is_finite()).then_some((x.to_bits(), y.to_bits()))
                    })
                    .collect::<Vec<_>>();
                (!points.is_empty()).then_some(EffectDataBusinessHint::Polyline {
                    points,
                    path: matched.path,
                })
            }
            _ => None,
        })
}

fn payload_target_content_hint(object: &TypeIoObject) -> Option<EffectDataBusinessHint> {
    let content = object
        .find_first_dfs_bounded(EFFECT_DATA_MAX_DEPTH, EFFECT_DATA_MAX_NODES, |value| {
            matches!(value.semantic_ref(), Some(TypeIoSemanticRef::Content { content_type, .. })
                if matches!(content_type, BLOCK_CONTENT_TYPE | UNIT_CONTENT_TYPE))
                || matches!(value.semantic_ref(), Some(TypeIoSemanticRef::TechNode { content_type, .. })
                    if matches!(content_type, BLOCK_CONTENT_TYPE | UNIT_CONTENT_TYPE))
        })
        .and_then(|matched| match matched.value.semantic_ref() {
            Some(TypeIoSemanticRef::Content {
                content_type,
                content_id,
            }) => Some((
                EffectBusinessContentKind::Content,
                content_type,
                content_id,
                matched.path,
            )),
            Some(TypeIoSemanticRef::TechNode {
                content_type,
                content_id,
            }) => Some((
                EffectBusinessContentKind::TechNode,
                content_type,
                content_id,
                matched.path,
            )),
            _ => None,
        })?;
    let target = object
        .find_first_dfs_bounded(EFFECT_DATA_MAX_DEPTH, EFFECT_DATA_MAX_NODES, |value| {
            matches!(
                value.semantic_ref(),
                Some(TypeIoSemanticRef::Building { .. } | TypeIoSemanticRef::Unit { .. })
            ) || matches!(
                value,
                TypeIoObject::Point2 { .. }
                    | TypeIoObject::PackedPoint2Array(_)
                    | TypeIoObject::Vec2 { .. }
                    | TypeIoObject::Vec2Array(_)
            )
        })
        .and_then(|matched| target_hint_from_match(matched.value, matched.path))?;
    Some(EffectDataBusinessHint::PayloadTargetContent {
        content_kind: content.0,
        content_type: content.1,
        content_id: content.2,
        content_path: content.3,
        target,
    })
}

fn target_hint_from_match(
    value: &TypeIoObject,
    path: Vec<usize>,
) -> Option<EffectDataBusinessTargetHint> {
    if let Some(semantic_ref) = value.semantic_ref() {
        if matches!(
            semantic_ref,
            TypeIoSemanticRef::Building { .. } | TypeIoSemanticRef::Unit { .. }
        ) {
            return Some(EffectDataBusinessTargetHint::SemanticRef(
                TypeIoSemanticMatch { semantic_ref, path },
            ));
        }
    }
    position_hint_from_value(value, path).map(EffectDataBusinessTargetHint::PositionHint)
}

fn position_hint_from_value(
    value: &TypeIoObject,
    path: Vec<usize>,
) -> Option<TypeIoEffectPositionHint> {
    match value {
        TypeIoObject::Point2 { x, y } => {
            Some(TypeIoEffectPositionHint::Point2 { x: *x, y: *y, path })
        }
        TypeIoObject::PackedPoint2Array(values) => {
            let packed_point2 = *values.first()?;
            let mut path = path;
            path.push(0);
            Some(TypeIoEffectPositionHint::PackedPoint2ArrayFirst {
                packed_point2,
                path,
            })
        }
        TypeIoObject::Vec2 { x, y } => Some(TypeIoEffectPositionHint::Vec2 {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
            path,
        }),
        TypeIoObject::Vec2Array(values) => {
            let (x, y) = values.first()?;
            let mut path = path;
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

#[cfg(test)]
mod tests {
    use super::{
        derive_effect_data_business_input, derive_effect_data_semantic, EffectDataBusinessHint,
        EffectDataBusinessInput, EffectDataBusinessTargetHint,
    };
    use crate::session_state::{EffectBusinessContentKind, EffectDataSemantic};
    use mdt_typeio::{TypeIoEffectPositionHint, TypeIoObject, TypeIoSemanticRef};

    fn nested_object_array(depth: usize, leaf: TypeIoObject) -> TypeIoObject {
        if depth == 0 {
            leaf
        } else {
            TypeIoObject::ObjectArray(vec![nested_object_array(depth - 1, leaf)])
        }
    }

    #[test]
    fn derive_effect_data_business_input_captures_payload_target_content_hints() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Point2 { x: 4, y: 6 },
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 33,
            },
        ]);

        let input =
            derive_effect_data_business_input(Some(26), Some(&object), Some(5), false, None);

        assert_eq!(
            input,
            EffectDataBusinessInput {
                contract_name: Some("payload_target_content"),
                data_kind: Some("object[len=2]{0=Point2,1=Content(raw)}".to_string()),
                semantic: Some(EffectDataSemantic::ObjectArrayLen(2)),
                primary: Some(EffectDataBusinessHint::PayloadTargetContent {
                    content_kind: EffectBusinessContentKind::Content,
                    content_type: 1,
                    content_id: 33,
                    content_path: vec![1],
                    target: EffectDataBusinessTargetHint::PositionHint(
                        TypeIoEffectPositionHint::Point2 {
                            x: 4,
                            y: 6,
                            path: vec![0],
                        },
                    ),
                }),
                data_type_tag: Some(5),
                parse_failed: false,
                parse_error: None,
            }
        );
    }

    #[test]
    fn derive_effect_data_business_input_captures_technode_payload_target_content_hints() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::TechNodeRaw {
                content_type: 1,
                content_id: 33,
            },
            TypeIoObject::Point2 { x: 4, y: 6 },
        ]);

        let input =
            derive_effect_data_business_input(Some(26), Some(&object), Some(5), false, None);

        assert_eq!(
            input,
            EffectDataBusinessInput {
                contract_name: Some("payload_target_content"),
                data_kind: Some("object[len=2]{0=TechNode(raw),1=Point2}".to_string()),
                semantic: Some(EffectDataSemantic::ObjectArrayLen(2)),
                primary: Some(EffectDataBusinessHint::PayloadTargetContent {
                    content_kind: EffectBusinessContentKind::TechNode,
                    content_type: 1,
                    content_id: 33,
                    content_path: vec![0],
                    target: EffectDataBusinessTargetHint::PositionHint(
                        TypeIoEffectPositionHint::Point2 {
                            x: 4,
                            y: 6,
                            path: vec![1],
                        },
                    ),
                }),
                data_type_tag: Some(5),
                parse_failed: false,
                parse_error: None,
            }
        );
    }

    #[test]
    fn derive_effect_data_business_input_captures_deep_payload_target_content_hints() {
        let object = nested_object_array(
            3,
            TypeIoObject::ObjectArray(vec![
                TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 33,
                },
                TypeIoObject::Point2 { x: 4, y: 6 },
            ]),
        );

        let input =
            derive_effect_data_business_input(Some(26), Some(&object), Some(5), false, None);

        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::PayloadTargetContent {
                content_kind: EffectBusinessContentKind::Content,
                content_type: 1,
                content_id: 33,
                content_path: vec![0, 0, 0, 0],
                target: EffectDataBusinessTargetHint::PositionHint(
                    TypeIoEffectPositionHint::Point2 {
                        x: 4,
                        y: 6,
                        path: vec![0, 0, 0, 1],
                    },
                ),
            })
        );
        assert_eq!(input.contract_name, Some("payload_target_content"));
    }

    #[test]
    fn derive_effect_data_business_input_prefers_mixed_content_and_position_hints() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 33,
            },
            TypeIoObject::Point2 { x: 4, y: 6 },
            TypeIoObject::UnitId(404),
        ]);

        let input =
            derive_effect_data_business_input(Some(26), Some(&object), Some(5), false, None);

        assert_eq!(input.contract_name, Some("payload_target_content"));
        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::PayloadTargetContent {
                content_kind: EffectBusinessContentKind::Content,
                content_type: 1,
                content_id: 33,
                content_path: vec![0],
                target: EffectDataBusinessTargetHint::PositionHint(
                    TypeIoEffectPositionHint::Point2 {
                        x: 4,
                        y: 6,
                        path: vec![1],
                    },
                ),
            })
        );
        assert_eq!(
            input.data_kind.as_deref(),
            Some("object[len=3]{0=Content(raw),1=Point2,2=Unit(raw)}")
        );
    }

    #[test]
    fn derive_effect_data_business_input_prefers_parent_ref_for_unit_parent_contract() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::UnitId(404),
            TypeIoObject::Point2 { x: 9, y: 12 },
        ]);

        let input =
            derive_effect_data_business_input(Some(67), Some(&object), Some(18), false, None);

        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::ParentRef {
                semantic_ref: TypeIoSemanticRef::Unit { unit_id: 404 },
                path: vec![0],
            })
        );
        assert_eq!(input.contract_name, Some("unit_parent"));
    }

    #[test]
    fn derive_effect_data_business_input_emits_polyline_for_deep_lightning_contract() {
        let object = nested_object_array(
            4,
            TypeIoObject::Vec2Array(vec![(1.0, 2.0), (3.5, 4.5)]),
        );

        let input =
            derive_effect_data_business_input(Some(13), Some(&object), Some(17), false, None);

        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::Polyline {
                points: vec![
                    (1.0f32.to_bits(), 2.0f32.to_bits()),
                    (3.5f32.to_bits(), 4.5f32.to_bits())
                ],
                path: vec![0, 0, 0, 0],
            })
        );
        assert_eq!(input.contract_name, Some("lightning"));
    }

    #[test]
    fn derive_effect_data_business_input_uses_float_length_hint() {
        let object = TypeIoObject::ObjectArray(vec![TypeIoObject::Float(16.0)]);

        let input =
            derive_effect_data_business_input(Some(200), Some(&object), Some(3), false, None);

        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::FloatBits {
                bits: 16.0f32.to_bits(),
                path: vec![0],
            })
        );
        assert_eq!(input.contract_name, Some("float_length"));
    }

    #[test]
    fn derive_effect_data_business_input_preserves_parse_failure_without_object() {
        let input = derive_effect_data_business_input(None, None, Some(0x7f), true, Some("decode"));

        assert_eq!(
            input.semantic,
            Some(EffectDataSemantic::OpaqueTypeTag(0x7f))
        );
        assert_eq!(input.primary, None);
        assert_eq!(input.parse_error.as_deref(), Some("decode"));
    }

    #[test]
    fn derive_effect_data_business_input_leaves_semantic_empty_when_object_is_missing_without_parse_failure()
    {
        let input = derive_effect_data_business_input(None, None, Some(0x55), false, None);

        assert_eq!(input.contract_name, None);
        assert_eq!(input.data_kind, None);
        assert_eq!(input.semantic, None);
        assert_eq!(input.primary, None);
        assert_eq!(input.data_type_tag, Some(0x55));
        assert!(!input.parse_failed);
        assert_eq!(input.parse_error, None);
    }

    #[test]
    fn derive_effect_data_business_input_preserves_object_semantics_on_parse_failure() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Point2 { x: 4, y: 6 },
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 33,
            },
        ]);

        let input = derive_effect_data_business_input(
            Some(26),
            Some(&object),
            Some(5),
            true,
            Some("decode"),
        );

        assert_eq!(input.contract_name, Some("payload_target_content"));
        assert_eq!(
            input.data_kind.as_deref(),
            Some("object[len=2]{0=Point2,1=Content(raw)}")
        );
        assert_eq!(input.semantic, Some(EffectDataSemantic::ObjectArrayLen(2)));
        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::PayloadTargetContent {
                content_kind: EffectBusinessContentKind::Content,
                content_type: 1,
                content_id: 33,
                content_path: vec![1],
                target: EffectDataBusinessTargetHint::PositionHint(
                    TypeIoEffectPositionHint::Point2 {
                        x: 4,
                        y: 6,
                        path: vec![0],
                    },
                ),
            })
        );
        assert_eq!(input.data_type_tag, Some(5));
        assert!(input.parse_failed);
        assert_eq!(input.parse_error.as_deref(), Some("decode"));
    }

    #[test]
    fn derive_effect_data_business_input_falls_back_to_content_hint_for_unstructured_mixed_payload() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 33,
            },
            TypeIoObject::Point2 { x: 4, y: 6 },
        ]);

        let input = derive_effect_data_business_input(None, Some(&object), Some(5), false, None);

        assert_eq!(input.contract_name, None);
        assert_eq!(input.semantic, Some(EffectDataSemantic::ObjectArrayLen(2)));
        assert_eq!(
            input.primary,
            Some(EffectDataBusinessHint::ContentRef {
                kind: EffectBusinessContentKind::Content,
                content_type: 1,
                content_id: 33,
                path: vec![0],
            })
        );
        assert_eq!(
            input.data_kind.as_deref(),
            Some("object[len=2]{0=Content(raw),1=Point2}")
        );
    }

    #[test]
    fn derive_effect_data_semantic_preserves_existing_scalar_and_semantic_ref_mapping() {
        assert_eq!(
            derive_effect_data_semantic(Some(&TypeIoObject::Int(7)), Some(1), false),
            Some(EffectDataSemantic::Int(7))
        );
        assert_eq!(
            derive_effect_data_semantic(
                Some(&TypeIoObject::BuildingPos(0x0001_0002)),
                Some(13),
                false
            ),
            Some(EffectDataSemantic::BuildingPos(0x0001_0002))
        );
        assert_eq!(
            derive_effect_data_semantic(None, Some(0x55), true),
            Some(EffectDataSemantic::OpaqueTypeTag(0x55))
        );
    }
}
