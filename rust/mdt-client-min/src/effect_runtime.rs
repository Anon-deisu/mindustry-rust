use crate::client_session::ClientSnapshotInputState;
use crate::session_state::SessionState;
use mdt_typeio::{TypeIoEffectPositionHint, TypeIoObject, TypeIoSemanticRef};

const EFFECT_PATH_MAX_DEPTH: usize = 3;
const EFFECT_PATH_MAX_NODES: usize = 64;
const BLOCK_CONTENT_TYPE: u8 = 1;
const UNIT_CONTENT_TYPE: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEffectBinding {
    WorldPosition {
        x_bits: u32,
        y_bits: u32,
    },
    ParentBuilding {
        build_pos: i32,
    },
    ParentUnit {
        unit_id: i32,
        spawn_x_bits: u32,
        spawn_y_bits: u32,
        offset_x_bits: u32,
        offset_y_bits: u32,
        offset_initialized: bool,
        preserve_spawn_offset: bool,
        allow_fallback_offset_initialization: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedRuntimeEffectBinding {
    binding: RuntimeEffectBinding,
    initial_position_bits: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentUnitPositionSource {
    EntityTable,
    SnapshotInput,
    WorldPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEffectContract {
    PositionTarget,
    LightningPath,
    PointBeam,
    PointHit,
    ShieldBreak,
    BlockContentIcon,
    ContentIcon,
    PayloadTargetContent,
    DropItem,
    FloatLength,
    UnitParent,
}

impl RuntimeEffectContract {
    pub const fn name(self) -> &'static str {
        match self {
            Self::PositionTarget => "position_target",
            Self::LightningPath => "lightning",
            Self::PointBeam => "point_beam",
            Self::PointHit => "point_hit",
            Self::ShieldBreak => "shield_break",
            Self::BlockContentIcon => "block_content_icon",
            Self::ContentIcon => "content_icon",
            Self::PayloadTargetContent => "payload_target_content",
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
    pub source_binding: Option<RuntimeEffectBinding>,
    pub x_bits: u32,
    pub y_bits: u32,
    pub rotation_bits: u32,
    pub color_rgba: u32,
    pub reliable: bool,
    pub has_data: bool,
    pub lifetime_ticks: u8,
    pub remaining_ticks: u8,
    pub contract_name: Option<&'static str>,
    pub binding: Option<RuntimeEffectBinding>,
    pub content_ref: Option<(u8, i16)>,
    pub polyline_points: Vec<(u32, u32)>,
}

pub fn effect_contract(effect_id: Option<i16>) -> Option<RuntimeEffectContract> {
    match effect_id {
        Some(13) => Some(RuntimeEffectContract::LightningPath),
        Some(10) => Some(RuntimeEffectContract::PointBeam),
        Some(11) => Some(RuntimeEffectContract::PointHit),
        Some(256) => Some(RuntimeEffectContract::ShieldBreak),
        Some(15 | 20 | 252) => Some(RuntimeEffectContract::BlockContentIcon),
        Some(3 | 35) => Some(RuntimeEffectContract::ContentIcon),
        Some(26) => Some(RuntimeEffectContract::PayloadTargetContent),
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
    lifetime_ticks: u8,
) -> RuntimeEffectOverlay {
    let contract = effect_contract(effect_id);
    let payload_target_content = contract
        .and_then(|contract| derive_runtime_effect_payload_target_content(contract, data_object));
    let content_ref = payload_target_content
        .map(|(_, _, content_ref)| content_ref)
        .or_else(|| {
            contract.and_then(|contract| derive_runtime_effect_content_ref(contract, data_object))
        });
    let polyline_points = contract
        .and_then(|contract| derive_runtime_effect_polyline(contract, data_object))
        .unwrap_or_default();
    let binding = if payload_target_content.is_none() && polyline_points.is_empty() {
        derive_runtime_effect_binding(effect_id, data_object, x.to_bits(), y.to_bits())
    } else {
        None
    };
    let source_binding = derive_runtime_effect_source_binding(
        effect_id,
        data_object,
        source_x.to_bits(),
        source_y.to_bits(),
    );
    let binding_initial_position = binding
        .as_ref()
        .and_then(|binding| binding.initial_position_bits);
    let binding = binding.map(|binding| binding.binding);
    let (x_bits, y_bits) = payload_target_content
        .map(|(target_x_bits, target_y_bits, _)| (target_x_bits, target_y_bits))
        .or_else(|| polyline_points.last().copied())
        .or(binding_initial_position)
        .unwrap_or((x.to_bits(), y.to_bits()));

    RuntimeEffectOverlay {
        effect_id,
        source_x_bits: source_x.to_bits(),
        source_y_bits: source_y.to_bits(),
        source_binding,
        x_bits,
        y_bits,
        rotation_bits: rotation.to_bits(),
        color_rgba,
        reliable,
        has_data: data_object.is_some(),
        lifetime_ticks,
        remaining_ticks: lifetime_ticks,
        contract_name: contract.map(RuntimeEffectContract::name),
        binding,
        content_ref,
        polyline_points,
    }
}

pub fn resolve_runtime_effect_overlay_position(
    overlay: &mut RuntimeEffectOverlay,
    session_state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
) -> (u32, u32) {
    let overlay_position = (overlay.x_bits, overlay.y_bits);
    overlay
        .binding
        .as_mut()
        .and_then(|binding| resolve_binding_position(binding, session_state, snapshot_input))
        .unwrap_or(overlay_position)
}

pub fn resolve_runtime_effect_overlay_source_position(
    overlay: &mut RuntimeEffectOverlay,
    session_state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
) -> (u32, u32) {
    let overlay_position = (overlay.source_x_bits, overlay.source_y_bits);
    overlay
        .source_binding
        .as_mut()
        .and_then(|binding| resolve_binding_position(binding, session_state, snapshot_input))
        .unwrap_or(overlay_position)
}

fn derive_runtime_effect_binding(
    effect_id: Option<i16>,
    object: Option<&TypeIoObject>,
    effect_x_bits: u32,
    effect_y_bits: u32,
) -> Option<DerivedRuntimeEffectBinding> {
    let object = object?;
    let summary = object.effect_summary();
    let position_hint_bits = summary
        .first_position_hint
        .as_ref()
        .map(position_hint_world_bits);

    if let Some(parent_ref) = summary.first_parent_ref {
        match parent_ref.semantic_ref {
            TypeIoSemanticRef::Building { build_pos } => {
                return Some(DerivedRuntimeEffectBinding {
                    binding: RuntimeEffectBinding::ParentBuilding { build_pos },
                    initial_position_bits: Some(world_bits_from_tile_pos(build_pos)),
                });
            }
            TypeIoSemanticRef::Unit { unit_id } => {
                let initial_position_bits =
                    position_hint_bits.or(Some((effect_x_bits, effect_y_bits)));
                return Some(DerivedRuntimeEffectBinding {
                    binding: RuntimeEffectBinding::ParentUnit {
                        unit_id,
                        spawn_x_bits: effect_x_bits,
                        spawn_y_bits: effect_y_bits,
                        offset_x_bits: 0.0f32.to_bits(),
                        offset_y_bits: 0.0f32.to_bits(),
                        offset_initialized: false,
                        preserve_spawn_offset: parent_binding_preserves_spawn_offset(effect_id),
                        allow_fallback_offset_initialization: false,
                    },
                    initial_position_bits,
                });
            }
            TypeIoSemanticRef::Content { .. } | TypeIoSemanticRef::TechNode { .. } => {}
        }
    }

    position_hint_bits.map(|(x_bits, y_bits)| DerivedRuntimeEffectBinding {
        binding: RuntimeEffectBinding::WorldPosition { x_bits, y_bits },
        initial_position_bits: Some((x_bits, y_bits)),
    })
}

fn derive_runtime_effect_source_binding(
    effect_id: Option<i16>,
    object: Option<&TypeIoObject>,
    source_x_bits: u32,
    source_y_bits: u32,
) -> Option<RuntimeEffectBinding> {
    if !source_binding_enabled(effect_id) {
        return None;
    }

    let object = object?;
    let summary = object.effect_summary();
    let parent_ref = summary.first_parent_ref?;
    match parent_ref.semantic_ref {
        TypeIoSemanticRef::Unit { unit_id } => Some(RuntimeEffectBinding::ParentUnit {
            unit_id,
            spawn_x_bits: source_x_bits,
            spawn_y_bits: source_y_bits,
            offset_x_bits: 0.0f32.to_bits(),
            offset_y_bits: 0.0f32.to_bits(),
            offset_initialized: false,
            preserve_spawn_offset: true,
            allow_fallback_offset_initialization: true,
        }),
        TypeIoSemanticRef::Building { .. }
        | TypeIoSemanticRef::Content { .. }
        | TypeIoSemanticRef::TechNode { .. } => None,
    }
}

fn derive_runtime_effect_polyline(
    contract: RuntimeEffectContract,
    object: Option<&TypeIoObject>,
) -> Option<Vec<(u32, u32)>> {
    let object = object?;
    match contract {
        RuntimeEffectContract::LightningPath => object
            .find_first_dfs_bounded(
                EFFECT_PATH_MAX_DEPTH,
                EFFECT_PATH_MAX_NODES,
                lightning_path_candidate,
            )
            .and_then(|matched| lightning_path_points(matched.value)),
        RuntimeEffectContract::PositionTarget
        | RuntimeEffectContract::PointBeam
        | RuntimeEffectContract::PointHit
        | RuntimeEffectContract::ShieldBreak
        | RuntimeEffectContract::BlockContentIcon
        | RuntimeEffectContract::ContentIcon
        | RuntimeEffectContract::PayloadTargetContent
        | RuntimeEffectContract::DropItem
        | RuntimeEffectContract::FloatLength
        | RuntimeEffectContract::UnitParent => None,
    }
}

fn derive_runtime_effect_payload_target_content(
    contract: RuntimeEffectContract,
    object: Option<&TypeIoObject>,
) -> Option<(u32, u32, (u8, i16))> {
    let object = object?;
    match contract {
        RuntimeEffectContract::PayloadTargetContent => {
            let content_ref = object
                .find_first_dfs_bounded(
                    EFFECT_PATH_MAX_DEPTH,
                    EFFECT_PATH_MAX_NODES,
                    payload_content_candidate,
                )
                .and_then(|matched| payload_content_ref(matched.value))?;
            let (target_x_bits, target_y_bits) = object
                .find_first_dfs_bounded(
                    EFFECT_PATH_MAX_DEPTH,
                    EFFECT_PATH_MAX_NODES,
                    payload_target_candidate,
                )
                .and_then(|matched| payload_target_world_bits(matched.value))?;
            Some((target_x_bits, target_y_bits, content_ref))
        }
        RuntimeEffectContract::PositionTarget
        | RuntimeEffectContract::LightningPath
        | RuntimeEffectContract::PointBeam
        | RuntimeEffectContract::PointHit
        | RuntimeEffectContract::ShieldBreak
        | RuntimeEffectContract::BlockContentIcon
        | RuntimeEffectContract::ContentIcon
        | RuntimeEffectContract::DropItem
        | RuntimeEffectContract::FloatLength
        | RuntimeEffectContract::UnitParent => None,
    }
}

fn derive_runtime_effect_content_ref(
    contract: RuntimeEffectContract,
    object: Option<&TypeIoObject>,
) -> Option<(u8, i16)> {
    let object = object?;
    match contract {
        RuntimeEffectContract::BlockContentIcon => object
            .find_first_dfs_bounded(
                EFFECT_PATH_MAX_DEPTH,
                EFFECT_PATH_MAX_NODES,
                block_content_candidate,
            )
            .and_then(|matched| block_content_ref(matched.value)),
        RuntimeEffectContract::ContentIcon => object
            .find_first_dfs_bounded(
                EFFECT_PATH_MAX_DEPTH,
                EFFECT_PATH_MAX_NODES,
                payload_content_candidate,
            )
            .and_then(|matched| payload_content_ref(matched.value)),
        RuntimeEffectContract::PositionTarget
        | RuntimeEffectContract::LightningPath
        | RuntimeEffectContract::PointBeam
        | RuntimeEffectContract::PointHit
        | RuntimeEffectContract::ShieldBreak
        | RuntimeEffectContract::DropItem
        | RuntimeEffectContract::FloatLength
        | RuntimeEffectContract::PayloadTargetContent
        | RuntimeEffectContract::UnitParent => None,
    }
}

fn lightning_path_candidate(value: &TypeIoObject) -> bool {
    matches!(value, TypeIoObject::Vec2Array(values) if !values.is_empty())
}

fn lightning_path_points(value: &TypeIoObject) -> Option<Vec<(u32, u32)>> {
    let TypeIoObject::Vec2Array(values) = value else {
        return None;
    };
    let points = values
        .iter()
        .filter_map(|(x, y)| (x.is_finite() && y.is_finite()).then_some((x.to_bits(), y.to_bits())))
        .collect::<Vec<_>>();
    (!points.is_empty()).then_some(points)
}

fn block_content_candidate(value: &TypeIoObject) -> bool {
    matches!(
        value.semantic_ref(),
        Some(TypeIoSemanticRef::Content {
            content_type: BLOCK_CONTENT_TYPE,
            ..
        })
    )
}

fn block_content_ref(value: &TypeIoObject) -> Option<(u8, i16)> {
    match value.semantic_ref()? {
        TypeIoSemanticRef::Content {
            content_type: BLOCK_CONTENT_TYPE,
            content_id,
        } => Some((BLOCK_CONTENT_TYPE, content_id)),
        TypeIoSemanticRef::Content { .. }
        | TypeIoSemanticRef::TechNode { .. }
        | TypeIoSemanticRef::Building { .. }
        | TypeIoSemanticRef::Unit { .. } => None,
    }
}

fn payload_content_candidate(value: &TypeIoObject) -> bool {
    matches!(
        value.semantic_ref(),
        Some(TypeIoSemanticRef::Content { content_type, .. })
            if [BLOCK_CONTENT_TYPE, UNIT_CONTENT_TYPE].contains(&content_type)
    )
}

fn payload_content_ref(value: &TypeIoObject) -> Option<(u8, i16)> {
    match value.semantic_ref()? {
        TypeIoSemanticRef::Content {
            content_type,
            content_id,
        } if [BLOCK_CONTENT_TYPE, UNIT_CONTENT_TYPE].contains(&content_type) => {
            Some((content_type, content_id))
        }
        TypeIoSemanticRef::Content { .. }
        | TypeIoSemanticRef::TechNode { .. }
        | TypeIoSemanticRef::Building { .. }
        | TypeIoSemanticRef::Unit { .. } => None,
    }
}

fn payload_target_candidate(value: &TypeIoObject) -> bool {
    payload_target_world_bits(value).is_some()
}

fn payload_target_world_bits(value: &TypeIoObject) -> Option<(u32, u32)> {
    match value {
        TypeIoObject::Point2 { x, y } => {
            let (world_x, world_y) = point2_world_coords(*x, *y);
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        TypeIoObject::PackedPoint2Array(values) => {
            let (tile_x, tile_y) = unpack_point2(*values.first()?);
            let (world_x, world_y) = point2_world_coords(i32::from(tile_x), i32::from(tile_y));
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        TypeIoObject::Vec2 { x, y } => Some((x.to_bits(), y.to_bits())),
        TypeIoObject::Vec2Array(values) => values.first().map(|(x, y)| (x.to_bits(), y.to_bits())),
        _ => match value.semantic_ref()? {
            TypeIoSemanticRef::Building { build_pos } => {
                let (world_x, world_y) = world_coords_from_tile_pos(build_pos);
                Some((world_x.to_bits(), world_y.to_bits()))
            }
            TypeIoSemanticRef::Content { .. }
            | TypeIoSemanticRef::TechNode { .. }
            | TypeIoSemanticRef::Unit { .. } => None,
        },
    }
}

fn position_hint_world_bits(position_hint: &TypeIoEffectPositionHint) -> (u32, u32) {
    match position_hint {
        TypeIoEffectPositionHint::Point2 { x, y, .. } => {
            let (world_x, world_y) = point2_world_coords(*x, *y);
            (world_x.to_bits(), world_y.to_bits())
        }
        TypeIoEffectPositionHint::PackedPoint2ArrayFirst { packed_point2, .. } => {
            let (tile_x, tile_y) = unpack_point2(*packed_point2);
            let (world_x, world_y) = point2_world_coords(i32::from(tile_x), i32::from(tile_y));
            (world_x.to_bits(), world_y.to_bits())
        }
        TypeIoEffectPositionHint::Vec2 { x_bits, y_bits, .. }
        | TypeIoEffectPositionHint::Vec2ArrayFirst { x_bits, y_bits, .. } => (*x_bits, *y_bits),
    }
}

fn resolve_binding_position(
    binding: &mut RuntimeEffectBinding,
    session_state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
) -> Option<(u32, u32)> {
    match binding {
        RuntimeEffectBinding::WorldPosition { x_bits, y_bits } => Some((*x_bits, *y_bits)),
        RuntimeEffectBinding::ParentBuilding { build_pos } => {
            Some(world_bits_from_tile_pos(*build_pos))
        }
        RuntimeEffectBinding::ParentUnit {
            unit_id,
            spawn_x_bits,
            spawn_y_bits,
            offset_x_bits,
            offset_y_bits,
            offset_initialized,
            preserve_spawn_offset,
            allow_fallback_offset_initialization,
        } => {
            let (parent_x_bits, parent_y_bits, position_source) =
                resolve_parent_unit_position(*unit_id, session_state, snapshot_input)?;
            if !*offset_initialized {
                if !*preserve_spawn_offset {
                    return Some((parent_x_bits, parent_y_bits));
                }
                let can_initialize_offset = match position_source {
                    ParentUnitPositionSource::EntityTable => true,
                    ParentUnitPositionSource::SnapshotInput
                    | ParentUnitPositionSource::WorldPlayer => {
                        *allow_fallback_offset_initialization
                    }
                };
                if !can_initialize_offset {
                    return Some((parent_x_bits, parent_y_bits));
                }
                *offset_x_bits = coordinate_delta_bits(*spawn_x_bits, parent_x_bits);
                *offset_y_bits = coordinate_delta_bits(*spawn_y_bits, parent_y_bits);
                *offset_initialized = true;
            }
            if *preserve_spawn_offset {
                Some((
                    apply_coordinate_offset_bits(parent_x_bits, *offset_x_bits),
                    apply_coordinate_offset_bits(parent_y_bits, *offset_y_bits),
                ))
            } else {
                Some((parent_x_bits, parent_y_bits))
            }
        }
    }
}

fn resolve_parent_unit_position(
    unit_id: i32,
    session_state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
) -> Option<(u32, u32, ParentUnitPositionSource)> {
    if let Some(entity) = session_state
        .entity_table_projection
        .by_entity_id
        .get(&unit_id)
    {
        return Some((
            entity.x_bits,
            entity.y_bits,
            ParentUnitPositionSource::EntityTable,
        ));
    }
    if snapshot_input.unit_id == Some(unit_id) {
        if let Some((x, y)) = snapshot_input.position {
            return Some((
                x.to_bits(),
                y.to_bits(),
                ParentUnitPositionSource::SnapshotInput,
            ));
        }
        if let (Some(x_bits), Some(y_bits)) = (
            session_state.world_player_x_bits,
            session_state.world_player_y_bits,
        ) {
            return Some((x_bits, y_bits, ParentUnitPositionSource::WorldPlayer));
        }
    }
    None
}

fn coordinate_delta_bits(value_bits: u32, base_bits: u32) -> u32 {
    let value = f32::from_bits(value_bits);
    let base = f32::from_bits(base_bits);
    if value.is_finite() && base.is_finite() {
        (value - base).to_bits()
    } else {
        0.0f32.to_bits()
    }
}

fn apply_coordinate_offset_bits(base_bits: u32, offset_bits: u32) -> u32 {
    let base = f32::from_bits(base_bits);
    let offset = f32::from_bits(offset_bits);
    if base.is_finite() && offset.is_finite() {
        (base + offset).to_bits()
    } else {
        base_bits
    }
}

fn world_bits_from_tile_pos(tile_pos: i32) -> (u32, u32) {
    let (world_x, world_y) = world_coords_from_tile_pos(tile_pos);
    (world_x.to_bits(), world_y.to_bits())
}

fn parent_binding_preserves_spawn_offset(effect_id: Option<i16>) -> bool {
    matches!(
        effect_contract(effect_id),
        Some(RuntimeEffectContract::UnitParent)
    )
}

fn source_binding_enabled(effect_id: Option<i16>) -> bool {
    matches!(effect_id, Some(8 | 9))
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

#[cfg(test)]
mod tests {
    use super::{
        resolve_runtime_effect_overlay_position, resolve_runtime_effect_overlay_source_position,
        spawn_runtime_effect_overlay, RuntimeEffectBinding,
    };
    use crate::client_session::ClientSnapshotInputState;
    use crate::session_state::{EntityProjection, SessionState};
    use mdt_typeio::TypeIoObject;

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_uses_position_hint_for_unresolved_parent_unit() {
        let overlay = spawn_runtime_effect_overlay(
            None,
            1.0,
            2.0,
            1.0,
            2.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::UnitId(9999),
                TypeIoObject::Point2 { x: 10, y: 20 },
            ])),
            10,
        );

        assert_eq!(overlay.x_bits, 80.0f32.to_bits());
        assert_eq!(overlay.y_bits, 160.0f32.to_bits());
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 9999,
                spawn_x_bits: 1.0f32.to_bits(),
                spawn_y_bits: 2.0f32.to_bits(),
                offset_x_bits: 0.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: false,
                preserve_spawn_offset: false,
                allow_fallback_offset_initialization: false,
            })
        );
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_lazily_freezes_parent_unit_offset() {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(257),
            20.0,
            24.0,
            20.0,
            24.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::UnitId(404)),
            10,
        );
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 12.0f32.to_bits(),
                y_bits: 16.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );

        let first_position = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);
        assert_eq!(first_position, (20.0f32.to_bits(), 24.0f32.to_bits()));
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 20.0f32.to_bits(),
                spawn_y_bits: 24.0f32.to_bits(),
                offset_x_bits: 8.0f32.to_bits(),
                offset_y_bits: 8.0f32.to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization: false,
            })
        );

        state
            .entity_table_projection
            .by_entity_id
            .get_mut(&404)
            .expect("missing entity 404")
            .x_bits = 24.0f32.to_bits();
        state
            .entity_table_projection
            .by_entity_id
            .get_mut(&404)
            .expect("missing entity 404")
            .y_bits = 28.0f32.to_bits();

        let second_position = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);
        assert_eq!(second_position, (32.0f32.to_bits(), 36.0f32.to_bits()));
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_source_position_follows_parent_unit_for_item_transfer(
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(9),
            80.0,
            160.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::UnitId(404)),
            10,
        );
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );

        let first_position =
            resolve_runtime_effect_overlay_source_position(&mut overlay, &state, &input);
        assert_eq!(first_position, (12.0f32.to_bits(), 20.0f32.to_bits()));
        assert_eq!(
            overlay.source_binding,
            Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 12.0f32.to_bits(),
                spawn_y_bits: 20.0f32.to_bits(),
                offset_x_bits: (-68.0f32).to_bits(),
                offset_y_bits: (-140.0f32).to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization: true,
            })
        );

        state
            .entity_table_projection
            .by_entity_id
            .get_mut(&404)
            .expect("missing entity 404")
            .x_bits = 96.0f32.to_bits();
        state
            .entity_table_projection
            .by_entity_id
            .get_mut(&404)
            .expect("missing entity 404")
            .y_bits = 184.0f32.to_bits();

        let second_position =
            resolve_runtime_effect_overlay_source_position(&mut overlay, &state, &input);
        assert_eq!(second_position, (28.0f32.to_bits(), 44.0f32.to_bits()));
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_source_position_freezes_offset_from_snapshot_fallback(
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(9),
            80.0,
            160.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::UnitId(404)),
            10,
        );
        let input = ClientSnapshotInputState {
            unit_id: Some(404),
            position: Some((80.0, 160.0)),
            ..ClientSnapshotInputState::default()
        };
        let mut state = SessionState::default();

        let first_position =
            resolve_runtime_effect_overlay_source_position(&mut overlay, &state, &input);
        assert_eq!(first_position, (12.0f32.to_bits(), 20.0f32.to_bits()));
        assert_eq!(
            overlay.source_binding,
            Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 12.0f32.to_bits(),
                spawn_y_bits: 20.0f32.to_bits(),
                offset_x_bits: (-68.0f32).to_bits(),
                offset_y_bits: (-140.0f32).to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization: true,
            })
        );

        state.entity_table_projection.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 96.0f32.to_bits(),
                y_bits: 184.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );

        let second_position =
            resolve_runtime_effect_overlay_source_position(&mut overlay, &state, &input);
        assert_eq!(second_position, (28.0f32.to_bits(), 44.0f32.to_bits()));
    }
}
