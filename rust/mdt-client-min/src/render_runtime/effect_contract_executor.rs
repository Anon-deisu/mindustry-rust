use crate::effect_runtime::RuntimeEffectContract;
use crate::session_state::EffectBusinessProjection;
use mdt_typeio::{TypeIoObject, TypeIoSemanticRef};

const EFFECT_CONTRACT_MAX_DEPTH: usize = 3;
const EFFECT_CONTRACT_MAX_NODES: usize = 64;
const ITEM_CONTENT_TYPE: u8 = 0;
const DROP_ITEM_EFFECT_LENGTH: f32 = 20.0;

type OverlayOriginProjector = fn(f32, f32, f32, &TypeIoObject) -> Option<(f32, f32)>;
type BusinessWorldPositionProjector = fn(&EffectBusinessProjection) -> Option<(u32, u32)>;

struct RuntimeEffectContractExecutor {
    contract_name: &'static str,
    overlay_origin: OverlayOriginProjector,
    business_world_position: BusinessWorldPositionProjector,
}

const POSITION_TARGET_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "position_target",
    overlay_origin: position_target_overlay_origin,
    business_world_position: position_target_business_world_position,
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

fn executor_for_contract(
    contract: RuntimeEffectContract,
) -> &'static RuntimeEffectContractExecutor {
    match contract {
        RuntimeEffectContract::PositionTarget => &POSITION_TARGET_EXECUTOR,
        RuntimeEffectContract::DropItem => &DROP_ITEM_EXECUTOR,
        RuntimeEffectContract::FloatLength => &FLOAT_LENGTH_EXECUTOR,
        RuntimeEffectContract::UnitParent => &UNIT_PARENT_EXECUTOR,
    }
}

fn executor_for_name(name: &str) -> Option<&'static RuntimeEffectContractExecutor> {
    for executor in [
        &POSITION_TARGET_EXECUTOR,
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

fn drop_item_overlay_origin(
    effect_x: f32,
    effect_y: f32,
    effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    first_contract_match(object, drop_item_candidate)?;
    ray_endpoint(effect_x, effect_y, effect_rotation, DROP_ITEM_EFFECT_LENGTH)
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
        | EffectBusinessProjection::LengthRay {
            target_x_bits,
            target_y_bits,
            ..
        } => Some((*target_x_bits, *target_y_bits)),
        EffectBusinessProjection::ContentRef { .. } | EffectBusinessProjection::FloatValue(_) => {
            None
        }
    }
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

fn drop_item_candidate(value: &TypeIoObject) -> bool {
    matches!(
        value.semantic_ref(),
        Some(TypeIoSemanticRef::Content { content_type, .. }) if content_type == ITEM_CONTENT_TYPE
    )
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
    Some((
        effect_x + cos * length,
        effect_y + sin * length,
    ))
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
}
