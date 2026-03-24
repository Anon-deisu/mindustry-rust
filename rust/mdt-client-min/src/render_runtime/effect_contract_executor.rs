use crate::effect_runtime::{RuntimeEffectContract, RuntimeEffectOverlay};
use crate::session_state::EffectBusinessProjection;
use mdt_typeio::{TypeIoObject, TypeIoSemanticRef};

const EFFECT_CONTRACT_MAX_DEPTH: usize = 3;
const EFFECT_CONTRACT_MAX_NODES: usize = 64;
const BLOCK_CONTENT_TYPE: u8 = 1;
const ITEM_CONTENT_TYPE: u8 = 0;
const UNIT_CONTENT_TYPE: u8 = 6;
const DROP_ITEM_EFFECT_LENGTH: f32 = 20.0;
#[cfg(test)]
const PAYLOAD_DEPOSIT_EFFECT_ID: i16 = 26;
const PAYLOAD_DEPOSIT_OVERLAY_TTL_TICKS: u8 = 3;
const LIGHTNING_EFFECT_ID: i16 = 13;
const POINT_BEAM_EFFECT_ID: i16 = 10;
const SHIELD_BREAK_EFFECT_ID: i16 = 256;
const CHAIN_LIGHTNING_EFFECT_ID: i16 = 261;
const CHAIN_EMP_EFFECT_ID: i16 = 262;
const CHAIN_SEGMENT_TARGET_PIXELS: f32 = 24.0;
const CHAIN_MIN_SEGMENTS: usize = 3;
const CHAIN_MAX_SEGMENTS: usize = 8;
const SHIELD_BREAK_SIDE_COUNT: usize = 6;
const SHIELD_BREAK_RADIUS_GROWTH: f32 = 1.0;

type OverlayOriginProjector = fn(f32, f32, f32, &TypeIoObject) -> Option<(f32, f32)>;
type BusinessWorldPositionProjector = fn(&EffectBusinessProjection) -> Option<(u32, u32)>;

struct RuntimeEffectContractExecutor {
    contract_name: &'static str,
    overlay_origin: OverlayOriginProjector,
    business_world_position: BusinessWorldPositionProjector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeEffectLineProjection {
    pub kind: &'static str,
    pub source_x_bits: u32,
    pub source_y_bits: u32,
    pub target_x_bits: u32,
    pub target_y_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeEffectContentProjection {
    pub kind: &'static str,
    pub content_type: u8,
    pub content_id: i16,
    pub x_bits: u32,
    pub y_bits: u32,
}

const POSITION_TARGET_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "position_target",
    overlay_origin: position_target_overlay_origin,
    business_world_position: position_target_business_world_position,
};

const LIGHTNING_PATH_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "lightning",
    overlay_origin: lightning_path_overlay_origin,
    business_world_position: lightning_path_business_world_position,
};

const POINT_BEAM_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "point_beam",
    overlay_origin: position_target_overlay_origin,
    business_world_position: position_target_business_world_position,
};

const SHIELD_BREAK_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "shield_break",
    overlay_origin: unsupported_overlay_origin,
    business_world_position: unsupported_business_world_position,
};

const BLOCK_CONTENT_ICON_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "block_content_icon",
    overlay_origin: block_content_icon_overlay_origin,
    business_world_position: unsupported_business_world_position,
};

const CONTENT_ICON_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "content_icon",
    overlay_origin: unsupported_overlay_origin,
    business_world_position: unsupported_business_world_position,
};

const PAYLOAD_TARGET_CONTENT_EXECUTOR: RuntimeEffectContractExecutor =
    RuntimeEffectContractExecutor {
        contract_name: "payload_target_content",
        overlay_origin: payload_target_content_overlay_origin,
        business_world_position: payload_target_content_business_world_position,
    };

const DROP_ITEM_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "drop_item",
    overlay_origin: drop_item_overlay_origin,
    business_world_position: unsupported_business_world_position,
};

const FLOAT_LENGTH_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "float_length",
    overlay_origin: float_length_overlay_origin,
    business_world_position: float_length_business_world_position,
};

const UNIT_PARENT_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "unit_parent",
    overlay_origin: unsupported_overlay_origin,
    business_world_position: unit_parent_business_world_position,
};

pub(crate) fn overlay_origin_from_contract(
    contract: RuntimeEffectContract,
    effect_x: f32,
    effect_y: f32,
    effect_rotation: f32,
    object: Option<&TypeIoObject>,
) -> Option<(f32, f32)> {
    let object = object?;
    (executor_for_contract(contract).overlay_origin)(effect_x, effect_y, effect_rotation, object)
}

pub(crate) fn world_position_from_contract_business_projection(
    contract_name: Option<&str>,
    projection: Option<&EffectBusinessProjection>,
) -> Option<(u32, u32)> {
    let projection = projection?;
    contract_name
        .and_then(executor_for_name)
        .and_then(|executor| (executor.business_world_position)(projection))
        .or_else(|| generic_business_world_position(projection))
}

pub(crate) fn line_projections_for_effect_overlay(
    overlay: &RuntimeEffectOverlay,
    target_x_bits: u32,
    target_y_bits: u32,
) -> Vec<RuntimeEffectLineProjection> {
    match overlay.effect_id {
        Some(LIGHTNING_EFFECT_ID) => lightning_line_projections(&overlay.polyline_points),
        Some(POINT_BEAM_EFFECT_ID) => vec![RuntimeEffectLineProjection {
            kind: "point-beam",
            source_x_bits: overlay.source_x_bits,
            source_y_bits: overlay.source_y_bits,
            target_x_bits,
            target_y_bits,
        }],
        Some(SHIELD_BREAK_EFFECT_ID) => shield_break_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.rotation_bits,
            overlay.remaining_ticks,
        ),
        Some(effect_id @ (CHAIN_LIGHTNING_EFFECT_ID | CHAIN_EMP_EFFECT_ID)) => {
            chain_line_kind(effect_id)
                .map(|kind| {
                    chain_line_projections(
                        kind,
                        overlay.source_x_bits,
                        overlay.source_y_bits,
                        target_x_bits,
                        target_y_bits,
                    )
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn content_projections_for_effect_overlay(
    overlay: &RuntimeEffectOverlay,
    target_x_bits: u32,
    target_y_bits: u32,
) -> Vec<RuntimeEffectContentProjection> {
    match (overlay.contract_name, overlay.content_ref) {
        (
            Some("payload_target_content"),
            Some((content_type @ (BLOCK_CONTENT_TYPE | UNIT_CONTENT_TYPE), content_id)),
        ) => {
            let (x_bits, y_bits) =
                payload_deposit_content_position(overlay, target_x_bits, target_y_bits);
            vec![RuntimeEffectContentProjection {
                kind: "payload-deposit",
                content_type,
                content_id,
                x_bits,
                y_bits,
            }]
        }
        (
            Some("content_icon"),
            Some((content_type @ (BLOCK_CONTENT_TYPE | UNIT_CONTENT_TYPE), content_id)),
        ) => {
            vec![RuntimeEffectContentProjection {
                kind: "content-icon",
                content_type,
                content_id,
                x_bits: target_x_bits,
                y_bits: target_y_bits,
            }]
        }
        (Some("block_content_icon"), Some((BLOCK_CONTENT_TYPE, content_id))) => {
            vec![RuntimeEffectContentProjection {
                kind: "block-content-icon",
                content_type: BLOCK_CONTENT_TYPE,
                content_id,
                x_bits: target_x_bits,
                y_bits: target_y_bits,
            }]
        }
        _ => Vec::new(),
    }
}

fn lightning_line_projections(points: &[(u32, u32)]) -> Vec<RuntimeEffectLineProjection> {
    points
        .windows(2)
        .filter_map(|pair| {
            let [(source_x_bits, source_y_bits), (target_x_bits, target_y_bits)] = pair else {
                return None;
            };
            Some(RuntimeEffectLineProjection {
                kind: "lightning",
                source_x_bits: *source_x_bits,
                source_y_bits: *source_y_bits,
                target_x_bits: *target_x_bits,
                target_y_bits: *target_y_bits,
            })
        })
        .collect()
}

fn chain_line_kind(effect_id: i16) -> Option<&'static str> {
    match effect_id {
        CHAIN_LIGHTNING_EFFECT_ID => Some("chain-lightning"),
        CHAIN_EMP_EFFECT_ID => Some("chain-emp"),
        _ => None,
    }
}

fn chain_line_projections(
    kind: &'static str,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
) -> Vec<RuntimeEffectLineProjection> {
    let source_x = f32::from_bits(source_x_bits);
    let source_y = f32::from_bits(source_y_bits);
    let target_x = f32::from_bits(target_x_bits);
    let target_y = f32::from_bits(target_y_bits);
    if !source_x.is_finite()
        || !source_y.is_finite()
        || !target_x.is_finite()
        || !target_y.is_finite()
    {
        return Vec::new();
    }

    let dx = target_x - source_x;
    let dy = target_y - source_y;
    let distance = (dx * dx + dy * dy).sqrt();
    if !distance.is_finite() || distance <= f32::EPSILON {
        return Vec::new();
    }

    let segment_count = ((distance / CHAIN_SEGMENT_TARGET_PIXELS).round() as usize)
        .clamp(CHAIN_MIN_SEGMENTS, CHAIN_MAX_SEGMENTS);
    let inv_distance = distance.recip();
    let normal_x = -dy * inv_distance;
    let normal_y = dx * inv_distance;
    let amplitude = (distance / 8.0).clamp(2.0, 10.0);

    let mut points = Vec::with_capacity(segment_count + 1);
    points.push((source_x_bits, source_y_bits));
    for index in 1..segment_count {
        let t = index as f32 / segment_count as f32;
        let base_x = source_x + dx * t;
        let base_y = source_y + dy * t;
        let wave = if index % 2 == 0 { -1.0 } else { 1.0 };
        let taper = 1.0 - (2.0 * t - 1.0).abs() * 0.35;
        let offset = amplitude * wave * taper;
        points.push((
            (base_x + normal_x * offset).to_bits(),
            (base_y + normal_y * offset).to_bits(),
        ));
    }
    points.push((target_x_bits, target_y_bits));

    points
        .windows(2)
        .filter_map(|pair| {
            let [(source_x_bits, source_y_bits), (target_x_bits, target_y_bits)] = pair else {
                return None;
            };
            Some(RuntimeEffectLineProjection {
                kind,
                source_x_bits: *source_x_bits,
                source_y_bits: *source_y_bits,
                target_x_bits: *target_x_bits,
                target_y_bits: *target_y_bits,
            })
        })
        .collect()
}

fn shield_break_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    rotation_bits: u32,
    remaining_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    let base_radius = f32::from_bits(rotation_bits);
    if !center_x.is_finite() || !center_y.is_finite() || !base_radius.is_finite() {
        return Vec::new();
    }

    let radius = (base_radius + shield_break_progress(remaining_ticks) * SHIELD_BREAK_RADIUS_GROWTH)
        .max(0.0);
    if radius <= f32::EPSILON {
        return Vec::new();
    }

    let vertices = (0..SHIELD_BREAK_SIDE_COUNT)
        .map(|index| {
            let angle = index as f32 * std::f32::consts::TAU / SHIELD_BREAK_SIDE_COUNT as f32;
            (
                (center_x + angle.cos() * radius).to_bits(),
                (center_y + angle.sin() * radius).to_bits(),
            )
        })
        .collect::<Vec<_>>();

    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(SHIELD_BREAK_SIDE_COUNT)
        .map(
            |((source_x_bits, source_y_bits), (target_x_bits, target_y_bits))| {
                RuntimeEffectLineProjection {
                    kind: "shield-break",
                    source_x_bits,
                    source_y_bits,
                    target_x_bits,
                    target_y_bits,
                }
            },
        )
        .collect()
}

fn shield_break_progress(remaining_ticks: u8) -> f32 {
    let total_steps = PAYLOAD_DEPOSIT_OVERLAY_TTL_TICKS.saturating_sub(1);
    if total_steps == 0 {
        return 1.0;
    }
    let elapsed = PAYLOAD_DEPOSIT_OVERLAY_TTL_TICKS
        .saturating_sub(remaining_ticks)
        .min(total_steps);
    elapsed as f32 / total_steps as f32
}

fn executor_for_contract(
    contract: RuntimeEffectContract,
) -> &'static RuntimeEffectContractExecutor {
    match contract {
        RuntimeEffectContract::PositionTarget => &POSITION_TARGET_EXECUTOR,
        RuntimeEffectContract::LightningPath => &LIGHTNING_PATH_EXECUTOR,
        RuntimeEffectContract::PointBeam => &POINT_BEAM_EXECUTOR,
        RuntimeEffectContract::ShieldBreak => &SHIELD_BREAK_EXECUTOR,
        RuntimeEffectContract::BlockContentIcon => &BLOCK_CONTENT_ICON_EXECUTOR,
        RuntimeEffectContract::ContentIcon => &CONTENT_ICON_EXECUTOR,
        RuntimeEffectContract::PayloadTargetContent => &PAYLOAD_TARGET_CONTENT_EXECUTOR,
        RuntimeEffectContract::DropItem => &DROP_ITEM_EXECUTOR,
        RuntimeEffectContract::FloatLength => &FLOAT_LENGTH_EXECUTOR,
        RuntimeEffectContract::UnitParent => &UNIT_PARENT_EXECUTOR,
    }
}

fn executor_for_name(name: &str) -> Option<&'static RuntimeEffectContractExecutor> {
    for executor in [
        &POSITION_TARGET_EXECUTOR,
        &LIGHTNING_PATH_EXECUTOR,
        &POINT_BEAM_EXECUTOR,
        &SHIELD_BREAK_EXECUTOR,
        &BLOCK_CONTENT_ICON_EXECUTOR,
        &CONTENT_ICON_EXECUTOR,
        &PAYLOAD_TARGET_CONTENT_EXECUTOR,
        &DROP_ITEM_EXECUTOR,
        &FLOAT_LENGTH_EXECUTOR,
        &UNIT_PARENT_EXECUTOR,
    ] {
        if executor.contract_name == name {
            return Some(executor);
        }
    }
    None
}

fn unsupported_overlay_origin(
    _effect_x: f32,
    _effect_y: f32,
    _effect_rotation: f32,
    _object: &TypeIoObject,
) -> Option<(f32, f32)> {
    None
}

fn unsupported_business_world_position(
    _projection: &EffectBusinessProjection,
) -> Option<(u32, u32)> {
    None
}

fn position_target_overlay_origin(
    _effect_x: f32,
    _effect_y: f32,
    _effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    first_contract_match(object, position_target_candidate)
        .and_then(position_target_world_position)
        .map(bits_to_world_position)
}

fn lightning_path_overlay_origin(
    _effect_x: f32,
    _effect_y: f32,
    _effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    first_contract_match(object, lightning_path_candidate)
        .and_then(lightning_path_world_position)
        .map(bits_to_world_position)
}

fn position_target_business_world_position(
    projection: &EffectBusinessProjection,
) -> Option<(u32, u32)> {
    match projection {
        EffectBusinessProjection::PositionTarget {
            target_x_bits,
            target_y_bits,
            ..
        } => Some((*target_x_bits, *target_y_bits)),
        _ => None,
    }
}

fn lightning_path_business_world_position(
    projection: &EffectBusinessProjection,
) -> Option<(u32, u32)> {
    match projection {
        EffectBusinessProjection::LightningPath { points } => points.last().copied(),
        _ => None,
    }
}

fn block_content_icon_overlay_origin(
    effect_x: f32,
    effect_y: f32,
    _effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    first_contract_match(object, block_content_icon_candidate)?;
    (effect_x.is_finite() && effect_y.is_finite()).then_some((effect_x, effect_y))
}

fn drop_item_overlay_origin(
    effect_x: f32,
    effect_y: f32,
    effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    first_contract_match(object, drop_item_candidate)?;
    ray_endpoint(effect_x, effect_y, effect_rotation, DROP_ITEM_EFFECT_LENGTH)
}

fn payload_target_content_overlay_origin(
    _effect_x: f32,
    _effect_y: f32,
    _effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    first_contract_match(object, position_target_candidate)
        .and_then(position_target_world_position)
        .map(bits_to_world_position)
}

fn float_length_overlay_origin(
    effect_x: f32,
    effect_y: f32,
    effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    let matched = first_contract_match(object, |value| matches!(value, TypeIoObject::Float(_)))?;
    let TypeIoObject::Float(length) = matched else {
        return None;
    };
    ray_endpoint(effect_x, effect_y, effect_rotation, *length)
}

fn float_length_business_world_position(
    projection: &EffectBusinessProjection,
) -> Option<(u32, u32)> {
    match projection {
        EffectBusinessProjection::LengthRay {
            target_x_bits,
            target_y_bits,
            ..
        } => Some((*target_x_bits, *target_y_bits)),
        _ => None,
    }
}

fn payload_target_content_business_world_position(
    projection: &EffectBusinessProjection,
) -> Option<(u32, u32)> {
    match projection {
        EffectBusinessProjection::PayloadTargetContent {
            target_x_bits,
            target_y_bits,
            ..
        } => Some((*target_x_bits, *target_y_bits)),
        _ => None,
    }
}

fn unit_parent_business_world_position(
    projection: &EffectBusinessProjection,
) -> Option<(u32, u32)> {
    match projection {
        EffectBusinessProjection::ParentRef { x_bits, y_bits, .. } => Some((*x_bits, *y_bits)),
        _ => None,
    }
}

fn generic_business_world_position(projection: &EffectBusinessProjection) -> Option<(u32, u32)> {
    match projection {
        EffectBusinessProjection::ParentRef { x_bits, y_bits, .. }
        | EffectBusinessProjection::WorldPosition { x_bits, y_bits, .. } => {
            Some((*x_bits, *y_bits))
        }
        EffectBusinessProjection::PositionTarget {
            target_x_bits,
            target_y_bits,
            ..
        }
        | EffectBusinessProjection::PayloadTargetContent {
            target_x_bits,
            target_y_bits,
            ..
        }
        | EffectBusinessProjection::LengthRay {
            target_x_bits,
            target_y_bits,
            ..
        } => Some((*target_x_bits, *target_y_bits)),
        EffectBusinessProjection::LightningPath { points } => points.last().copied(),
        EffectBusinessProjection::ContentRef { .. } | EffectBusinessProjection::FloatValue(_) => {
            None
        }
    }
}

fn payload_deposit_content_position(
    overlay: &RuntimeEffectOverlay,
    target_x_bits: u32,
    target_y_bits: u32,
) -> (u32, u32) {
    let source_x = f32::from_bits(overlay.source_x_bits);
    let source_y = f32::from_bits(overlay.source_y_bits);
    let target_x = f32::from_bits(target_x_bits);
    let target_y = f32::from_bits(target_y_bits);
    if !source_x.is_finite()
        || !source_y.is_finite()
        || !target_x.is_finite()
        || !target_y.is_finite()
    {
        return (target_x_bits, target_y_bits);
    }

    let progress = payload_deposit_progress(overlay.remaining_ticks);
    (
        (source_x + (target_x - source_x) * progress).to_bits(),
        (source_y + (target_y - source_y) * progress).to_bits(),
    )
}

fn payload_deposit_progress(remaining_ticks: u8) -> f32 {
    let total_steps = PAYLOAD_DEPOSIT_OVERLAY_TTL_TICKS.saturating_sub(1);
    if total_steps == 0 {
        return 1.0;
    }
    let elapsed = PAYLOAD_DEPOSIT_OVERLAY_TTL_TICKS
        .saturating_sub(remaining_ticks)
        .min(total_steps);
    elapsed as f32 / total_steps as f32
}

fn first_contract_match<'a, P>(object: &'a TypeIoObject, predicate: P) -> Option<&'a TypeIoObject>
where
    P: Fn(&TypeIoObject) -> bool,
{
    object
        .find_first_dfs_bounded(
            EFFECT_CONTRACT_MAX_DEPTH,
            EFFECT_CONTRACT_MAX_NODES,
            predicate,
        )
        .map(|matched| matched.value)
}

fn position_target_candidate(value: &TypeIoObject) -> bool {
    match value {
        TypeIoObject::Point2 { .. } | TypeIoObject::Vec2 { .. } => true,
        TypeIoObject::PackedPoint2Array(values) => !values.is_empty(),
        TypeIoObject::Vec2Array(values) => !values.is_empty(),
        _ => matches!(
            value.semantic_ref(),
            Some(TypeIoSemanticRef::Building { .. } | TypeIoSemanticRef::Unit { .. })
        ),
    }
}

fn block_content_icon_candidate(value: &TypeIoObject) -> bool {
    matches!(
        value.semantic_ref(),
        Some(TypeIoSemanticRef::Content {
            content_type: BLOCK_CONTENT_TYPE,
            ..
        })
    )
}

fn drop_item_candidate(value: &TypeIoObject) -> bool {
    matches!(
        value.semantic_ref(),
        Some(TypeIoSemanticRef::Content { content_type, .. }) if content_type == ITEM_CONTENT_TYPE
    )
}

fn lightning_path_candidate(value: &TypeIoObject) -> bool {
    matches!(value, TypeIoObject::Vec2Array(values) if !values.is_empty())
}

fn position_target_world_position(value: &TypeIoObject) -> Option<(u32, u32)> {
    match value {
        TypeIoObject::Point2 { x, y } => {
            let (world_x, world_y) = tile_world_coords(*x, *y);
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        TypeIoObject::PackedPoint2Array(values) => {
            let (tile_x, tile_y) = super::unpack_runtime_point2(*values.first()?);
            let (world_x, world_y) = tile_world_coords(tile_x, tile_y);
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        TypeIoObject::Vec2 { x, y } => Some((x.to_bits(), y.to_bits())),
        TypeIoObject::Vec2Array(values) => values.first().map(|(x, y)| (x.to_bits(), y.to_bits())),
        _ => match value.semantic_ref()? {
            TypeIoSemanticRef::Building { build_pos } => {
                let (tile_x, tile_y) = super::unpack_runtime_point2(build_pos);
                let (world_x, world_y) = tile_world_coords(tile_x, tile_y);
                Some((world_x.to_bits(), world_y.to_bits()))
            }
            TypeIoSemanticRef::Content { .. }
            | TypeIoSemanticRef::TechNode { .. }
            | TypeIoSemanticRef::Unit { .. } => None,
        },
    }
}

fn lightning_path_world_position(value: &TypeIoObject) -> Option<(u32, u32)> {
    let TypeIoObject::Vec2Array(values) = value else {
        return None;
    };
    values
        .last()
        .and_then(|(x, y)| (x.is_finite() && y.is_finite()).then_some((x.to_bits(), y.to_bits())))
}

fn ray_endpoint(
    effect_x: f32,
    effect_y: f32,
    effect_rotation: f32,
    length: f32,
) -> Option<(f32, f32)> {
    if !effect_x.is_finite()
        || !effect_y.is_finite()
        || !effect_rotation.is_finite()
        || !length.is_finite()
    {
        return None;
    }
    let radians = effect_rotation.to_radians();
    let cos = snap_trig_component(radians.cos());
    let sin = snap_trig_component(radians.sin());
    Some((effect_x + cos * length, effect_y + sin * length))
}

fn snap_trig_component(value: f32) -> f32 {
    const TRIG_SNAP_EPSILON: f32 = 1e-6;

    if value.abs() <= TRIG_SNAP_EPSILON {
        0.0
    } else if (value - 1.0).abs() <= TRIG_SNAP_EPSILON {
        1.0
    } else if (value + 1.0).abs() <= TRIG_SNAP_EPSILON {
        -1.0
    } else {
        value
    }
}

fn bits_to_world_position((x_bits, y_bits): (u32, u32)) -> (f32, f32) {
    (f32::from_bits(x_bits), f32::from_bits(y_bits))
}

fn tile_world_coords(x: i32, y: i32) -> (f32, f32) {
    (x as f32 * 8.0, y as f32 * 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_target_overlay_origin_projects_nested_building_payload() {
        let object = TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![
            TypeIoObject::BuildingPos(super::super::pack_runtime_point2(9, 6)),
        ])]);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::PositionTarget,
                1.0,
                2.0,
                0.0,
                Some(&object),
            ),
            Some((72.0, 48.0))
        );
    }

    #[test]
    fn float_length_overlay_origin_projects_nested_float_payload() {
        let object =
            TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![TypeIoObject::Float(
                16.0,
            )])]);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::FloatLength,
                10.0,
                20.0,
                0.0,
                Some(&object),
            ),
            Some((26.0, 20.0))
        );
    }

    #[test]
    fn payload_target_content_overlay_origin_projects_nested_point2_target() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::ContentRaw {
                content_type: UNIT_CONTENT_TYPE,
                content_id: 9,
            },
            TypeIoObject::ObjectArray(vec![TypeIoObject::Point2 { x: 10, y: 20 }]),
        ]);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::PayloadTargetContent,
                12.0,
                20.0,
                0.0,
                Some(&object),
            ),
            Some((80.0, 160.0))
        );
    }

    #[test]
    fn block_content_icon_overlay_origin_keeps_effect_origin_for_nested_block_content() {
        let object = TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![
            TypeIoObject::ContentRaw {
                content_type: BLOCK_CONTENT_TYPE,
                content_id: 42,
            },
        ])]);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::BlockContentIcon,
                12.0,
                20.0,
                45.0,
                Some(&object),
            ),
            Some((12.0, 20.0))
        );
    }

    #[test]
    fn world_position_from_contract_business_projection_uses_named_executor() {
        let projection = EffectBusinessProjection::PositionTarget {
            source_x_bits: 10.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            target_x_bits: 80.0f32.to_bits(),
            target_y_bits: 160.0f32.to_bits(),
        };

        assert_eq!(
            world_position_from_contract_business_projection(
                Some(POSITION_TARGET_EXECUTOR.contract_name),
                Some(&projection),
            ),
            Some((80.0f32.to_bits(), 160.0f32.to_bits()))
        );
    }

    #[test]
    fn world_position_from_contract_business_projection_uses_payload_target_content_named_executor()
    {
        let projection = EffectBusinessProjection::PayloadTargetContent {
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            target_x_bits: 84.0f32.to_bits(),
            target_y_bits: 140.0f32.to_bits(),
            content_type: UNIT_CONTENT_TYPE,
            content_id: 9,
        };

        assert_eq!(
            world_position_from_contract_business_projection(
                Some(PAYLOAD_TARGET_CONTENT_EXECUTOR.contract_name),
                Some(&projection),
            ),
            Some((84.0f32.to_bits(), 140.0f32.to_bits()))
        );
    }

    #[test]
    fn world_position_from_contract_business_projection_uses_point_beam_named_executor() {
        let projection = EffectBusinessProjection::PositionTarget {
            source_x_bits: 10.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            target_x_bits: 80.0f32.to_bits(),
            target_y_bits: 160.0f32.to_bits(),
        };

        assert_eq!(
            world_position_from_contract_business_projection(
                Some(POINT_BEAM_EXECUTOR.contract_name),
                Some(&projection),
            ),
            Some((80.0f32.to_bits(), 160.0f32.to_bits()))
        );
    }

    #[test]
    fn lightning_path_overlay_origin_projects_last_vec2_point() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::Vec2Array(vec![(10.0, 20.0), (80.0, 160.0)]),
        ]);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::LightningPath,
                1.0,
                2.0,
                0.0,
                Some(&object),
            ),
            Some((80.0, 160.0))
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_point_beam_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(POINT_BEAM_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("point_beam"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        assert_eq!(
            line_projections_for_effect_overlay(&overlay, 80.0f32.to_bits(), 160.0f32.to_bits(),),
            vec![RuntimeEffectLineProjection {
                kind: "point-beam",
                source_x_bits: 12.0f32.to_bits(),
                source_y_bits: 20.0f32.to_bits(),
                target_x_bits: 80.0f32.to_bits(),
                target_y_bits: 160.0f32.to_bits(),
            }]
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_shield_break_hexagon() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(SHIELD_BREAK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 6.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: false,
            remaining_ticks: 3,
            contract_name: Some("shield_break"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines =
            line_projections_for_effect_overlay(&overlay, 32.0f32.to_bits(), 48.0f32.to_bits());

        assert_eq!(lines.len(), SHIELD_BREAK_SIDE_COUNT);
        assert!(lines.iter().all(|line| line.kind == "shield-break"));
        assert!(lines.iter().any(|line| {
            line.source_x_bits == 38.0f32.to_bits() && line.source_y_bits == 48.0f32.to_bits()
        }));
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_chain_segments() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(CHAIN_LIGHTNING_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines =
            line_projections_for_effect_overlay(&overlay, 80.0f32.to_bits(), 160.0f32.to_bits());

        assert!(lines.len() >= CHAIN_MIN_SEGMENTS);
        assert_eq!(lines.first().map(|line| line.kind), Some("chain-lightning"));
        assert_eq!(
            lines
                .first()
                .map(|line| (line.source_x_bits, line.source_y_bits)),
            Some((12.0f32.to_bits(), 20.0f32.to_bits()))
        );
        assert_eq!(
            lines
                .last()
                .map(|line| (line.target_x_bits, line.target_y_bits)),
            Some((80.0f32.to_bits(), 160.0f32.to_bits()))
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_chain_emp_segments() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(CHAIN_EMP_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines =
            line_projections_for_effect_overlay(&overlay, 80.0f32.to_bits(), 160.0f32.to_bits());

        assert!(lines.len() >= CHAIN_MIN_SEGMENTS);
        assert_eq!(lines.first().map(|line| line.kind), Some("chain-emp"));
        assert_eq!(
            lines
                .first()
                .map(|line| (line.source_x_bits, line.source_y_bits)),
            Some((12.0f32.to_bits(), 20.0f32.to_bits()))
        );
        assert_eq!(
            lines
                .last()
                .map(|line| (line.target_x_bits, line.target_y_bits)),
            Some((80.0f32.to_bits(), 160.0f32.to_bits()))
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_ignores_other_effect_ids() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(8),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        assert_eq!(
            line_projections_for_effect_overlay(&overlay, 80.0f32.to_bits(), 160.0f32.to_bits(),),
            Vec::<RuntimeEffectLineProjection>::new()
        );
    }

    #[test]
    fn world_position_from_contract_business_projection_uses_lightning_path_named_executor() {
        let projection = EffectBusinessProjection::LightningPath {
            points: vec![
                (10.0f32.to_bits(), 20.0f32.to_bits()),
                (80.0f32.to_bits(), 160.0f32.to_bits()),
            ],
        };

        assert_eq!(
            world_position_from_contract_business_projection(
                Some(LIGHTNING_PATH_EXECUTOR.contract_name),
                Some(&projection),
            ),
            Some((80.0f32.to_bits(), 160.0f32.to_bits()))
        );
    }

    #[test]
    fn content_projections_for_effect_overlay_returns_block_content_icon_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(252),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 12.0f32.to_bits(),
            y_bits: 20.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("block_content_icon"),
            binding: None,
            content_ref: Some((BLOCK_CONTENT_TYPE, 42)),
            polyline_points: Vec::new(),
        };

        assert_eq!(
            content_projections_for_effect_overlay(&overlay, 12.0f32.to_bits(), 20.0f32.to_bits()),
            vec![RuntimeEffectContentProjection {
                kind: "block-content-icon",
                content_type: BLOCK_CONTENT_TYPE,
                content_id: 42,
                x_bits: 12.0f32.to_bits(),
                y_bits: 20.0f32.to_bits(),
            }]
        );
    }

    #[test]
    fn content_projections_for_effect_overlay_returns_payload_target_content_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(PAYLOAD_DEPOSIT_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 84.0f32.to_bits(),
            y_bits: 140.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 2,
            contract_name: Some("payload_target_content"),
            binding: None,
            content_ref: Some((UNIT_CONTENT_TYPE, 9)),
            polyline_points: Vec::new(),
        };

        assert_eq!(
            content_projections_for_effect_overlay(&overlay, 84.0f32.to_bits(), 140.0f32.to_bits()),
            vec![RuntimeEffectContentProjection {
                kind: "payload-deposit",
                content_type: UNIT_CONTENT_TYPE,
                content_id: 9,
                x_bits: 48.0f32.to_bits(),
                y_bits: 80.0f32.to_bits(),
            }]
        );
    }

    #[test]
    fn content_projections_for_effect_overlay_returns_content_icon_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(35),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 12.0f32.to_bits(),
            y_bits: 20.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("content_icon"),
            binding: None,
            content_ref: Some((UNIT_CONTENT_TYPE, 9)),
            polyline_points: Vec::new(),
        };

        assert_eq!(
            content_projections_for_effect_overlay(&overlay, 12.0f32.to_bits(), 20.0f32.to_bits()),
            vec![RuntimeEffectContentProjection {
                kind: "content-icon",
                content_type: UNIT_CONTENT_TYPE,
                content_id: 9,
                x_bits: 12.0f32.to_bits(),
                y_bits: 20.0f32.to_bits(),
            }]
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_lightning_path_segments() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(LIGHTNING_EFFECT_ID),
            source_x_bits: 1.0f32.to_bits(),
            source_y_bits: 2.0f32.to_bits(),
            x_bits: 50.0f32.to_bits(),
            y_bits: 60.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            remaining_ticks: 3,
            contract_name: Some("lightning"),
            binding: None,
            content_ref: None,
            polyline_points: vec![
                (10.0f32.to_bits(), 20.0f32.to_bits()),
                (30.0f32.to_bits(), 40.0f32.to_bits()),
                (50.0f32.to_bits(), 60.0f32.to_bits()),
            ],
        };

        assert_eq!(
            line_projections_for_effect_overlay(&overlay, 50.0f32.to_bits(), 60.0f32.to_bits()),
            vec![
                RuntimeEffectLineProjection {
                    kind: "lightning",
                    source_x_bits: 10.0f32.to_bits(),
                    source_y_bits: 20.0f32.to_bits(),
                    target_x_bits: 30.0f32.to_bits(),
                    target_y_bits: 40.0f32.to_bits(),
                },
                RuntimeEffectLineProjection {
                    kind: "lightning",
                    source_x_bits: 30.0f32.to_bits(),
                    source_y_bits: 40.0f32.to_bits(),
                    target_x_bits: 50.0f32.to_bits(),
                    target_y_bits: 60.0f32.to_bits(),
                },
            ]
        );
    }
}
