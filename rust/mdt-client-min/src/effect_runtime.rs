use crate::client_session::ClientSnapshotInputState;
use crate::session_state::SessionState;
use mdt_typeio::{TypeIoEffectPositionHint, TypeIoObject, TypeIoSemanticRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEffectBinding {
    WorldPosition { x_bits: u32, y_bits: u32 },
    ParentBuilding { build_pos: i32 },
    ParentUnit { unit_id: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEffectContract {
    PositionTarget,
    PointBeam,
    DropItem,
    FloatLength,
    UnitParent,
}

impl RuntimeEffectContract {
    pub const fn name(self) -> &'static str {
        match self {
            Self::PositionTarget => "position_target",
            Self::PointBeam => "point_beam",
            Self::DropItem => "drop_item",
            Self::FloatLength => "float_length",
            Self::UnitParent => "unit_parent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEffectOverlay {
    pub effect_id: Option<i16>,
    pub source_x_bits: u32,
    pub source_y_bits: u32,
    pub x_bits: u32,
    pub y_bits: u32,
    pub rotation_bits: u32,
    pub color_rgba: u32,
    pub reliable: bool,
    pub has_data: bool,
    pub remaining_ticks: u8,
    pub contract_name: Option<&'static str>,
    pub binding: Option<RuntimeEffectBinding>,
}

pub fn effect_contract(effect_id: Option<i16>) -> Option<RuntimeEffectContract> {
    match effect_id {
        Some(10) => Some(RuntimeEffectContract::PointBeam),
        Some(8 | 9 | 178 | 261 | 262) => Some(RuntimeEffectContract::PositionTarget),
        Some(142) => Some(RuntimeEffectContract::DropItem),
        Some(200) => Some(RuntimeEffectContract::FloatLength),
        Some(257 | 260) => Some(RuntimeEffectContract::UnitParent),
        _ => None,
    }
}

pub fn effect_contract_name(effect_id: Option<i16>) -> Option<&'static str> {
    effect_contract(effect_id).map(RuntimeEffectContract::name)
}

pub fn spawn_runtime_effect_overlay(
    effect_id: Option<i16>,
    x: f32,
    y: f32,
    source_x: f32,
    source_y: f32,
    rotation: f32,
    color_rgba: u32,
    reliable: bool,
    data_object: Option<&TypeIoObject>,
    remaining_ticks: u8,
) -> RuntimeEffectOverlay {
    let binding = derive_runtime_effect_binding(data_object);
    let (x_bits, y_bits) = binding
        .as_ref()
        .and_then(initial_position_from_binding)
        .unwrap_or((x.to_bits(), y.to_bits()));

    RuntimeEffectOverlay {
        effect_id,
        source_x_bits: source_x.to_bits(),
        source_y_bits: source_y.to_bits(),
        x_bits,
        y_bits,
        rotation_bits: rotation.to_bits(),
        color_rgba,
        reliable,
        has_data: data_object.is_some(),
        remaining_ticks,
        contract_name: effect_contract_name(effect_id),
        binding,
    }
}

pub fn resolve_runtime_effect_overlay_position(
    overlay: &RuntimeEffectOverlay,
    session_state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
) -> (u32, u32) {
    overlay
        .binding
        .as_ref()
        .and_then(|binding| resolve_binding_position(binding, session_state, snapshot_input))
        .unwrap_or((overlay.x_bits, overlay.y_bits))
}

fn derive_runtime_effect_binding(object: Option<&TypeIoObject>) -> Option<RuntimeEffectBinding> {
    let object = object?;
    let summary = object.effect_summary();

    if let Some(parent_ref) = summary.first_parent_ref {
        match parent_ref.semantic_ref {
            TypeIoSemanticRef::Building { build_pos } => {
                return Some(RuntimeEffectBinding::ParentBuilding { build_pos });
            }
            TypeIoSemanticRef::Unit { unit_id } => {
                return Some(RuntimeEffectBinding::ParentUnit { unit_id });
            }
            TypeIoSemanticRef::Content { .. } | TypeIoSemanticRef::TechNode { .. } => {}
        }
    }

    summary
        .first_position_hint
        .as_ref()
        .map(binding_from_position_hint)
}

fn binding_from_position_hint(position_hint: &TypeIoEffectPositionHint) -> RuntimeEffectBinding {
    match position_hint {
        TypeIoEffectPositionHint::Point2 { x, y, .. } => {
            let (world_x, world_y) = point2_world_coords(*x, *y);
            RuntimeEffectBinding::WorldPosition {
                x_bits: world_x.to_bits(),
                y_bits: world_y.to_bits(),
            }
        }
        TypeIoEffectPositionHint::PackedPoint2ArrayFirst { packed_point2, .. } => {
            let (tile_x, tile_y) = unpack_point2(*packed_point2);
            let (world_x, world_y) = point2_world_coords(i32::from(tile_x), i32::from(tile_y));
            RuntimeEffectBinding::WorldPosition {
                x_bits: world_x.to_bits(),
                y_bits: world_y.to_bits(),
            }
        }
        TypeIoEffectPositionHint::Vec2 { x_bits, y_bits, .. }
        | TypeIoEffectPositionHint::Vec2ArrayFirst { x_bits, y_bits, .. } => {
            RuntimeEffectBinding::WorldPosition {
                x_bits: *x_bits,
                y_bits: *y_bits,
            }
        }
    }
}

fn initial_position_from_binding(binding: &RuntimeEffectBinding) -> Option<(u32, u32)> {
    match binding {
        RuntimeEffectBinding::WorldPosition { x_bits, y_bits } => Some((*x_bits, *y_bits)),
        RuntimeEffectBinding::ParentBuilding { build_pos } => {
            let (world_x, world_y) = world_coords_from_tile_pos(*build_pos);
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        RuntimeEffectBinding::ParentUnit { .. } => None,
    }
}

fn resolve_binding_position(
    binding: &RuntimeEffectBinding,
    session_state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
) -> Option<(u32, u32)> {
    match binding {
        RuntimeEffectBinding::WorldPosition { x_bits, y_bits } => Some((*x_bits, *y_bits)),
        RuntimeEffectBinding::ParentBuilding { build_pos } => {
            let (world_x, world_y) = world_coords_from_tile_pos(*build_pos);
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        RuntimeEffectBinding::ParentUnit { unit_id } => {
            if let Some(entity) = session_state
                .entity_table_projection
                .by_entity_id
                .get(unit_id)
            {
                return Some((entity.x_bits, entity.y_bits));
            }
            if snapshot_input.unit_id == Some(*unit_id) {
                if let Some((x, y)) = snapshot_input.position {
                    return Some((x.to_bits(), y.to_bits()));
                }
                if let (Some(x_bits), Some(y_bits)) = (
                    session_state.world_player_x_bits,
                    session_state.world_player_y_bits,
                ) {
                    return Some((x_bits, y_bits));
                }
            }
            None
        }
    }
}

fn world_coords_from_tile_pos(tile_pos: i32) -> (f32, f32) {
    let (tile_x, tile_y) = unpack_point2(tile_pos);
    point2_world_coords(i32::from(tile_x), i32::from(tile_y))
}

fn point2_world_coords(x: i32, y: i32) -> (f32, f32) {
    (x as f32 * 8.0, y as f32 * 8.0)
}

fn unpack_point2(value: i32) -> (i16, i16) {
    let x = ((value >> 16) & 0xffff) as i16;
    let y = (value & 0xffff) as i16;
    (x, y)
}
