use crate::session_state::{EffectRuntimeBindingState, SessionState};
use mdt_typeio::{TypeIoEffectPositionHint, TypeIoObject, TypeIoSemanticRef};

const EFFECT_PATH_MAX_DEPTH: usize = 3;
const EFFECT_PATH_MAX_NODES: usize = 64;
const ITEM_CONTENT_TYPE: u8 = 0;
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
        spawn_x_bits: u32,
        spawn_y_bits: u32,
        offset_x_bits: u32,
        offset_y_bits: u32,
        offset_initialized: bool,
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
        rotate_with_parent: bool,
        parent_rotation_reference_bits: u32,
        rotation_offset_bits: u32,
        rotation_initialized: bool,
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
enum ParentUnitRotationSource {
    EntitySemantic,
    SnapshotInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEffectContract {
    PositionTarget,
    LightningPath,
    PointBeam,
    PointHit,
    DrillSteam,
    LegDestroy,
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
            Self::DrillSteam => "drill_steam",
            Self::LegDestroy => "leg_destroy",
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

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct EffectRuntimeInputView {
    pub unit_id: Option<i32>,
    pub position: Option<(f32, f32)>,
    pub rotation: f32,
}

pub fn effect_contract(effect_id: Option<i16>) -> Option<RuntimeEffectContract> {
    match effect_id {
        Some(13) => Some(RuntimeEffectContract::LightningPath),
        Some(10) => Some(RuntimeEffectContract::PointBeam),
        Some(11) => Some(RuntimeEffectContract::PointHit),
        Some(124) => Some(RuntimeEffectContract::DrillSteam),
        Some(263) => Some(RuntimeEffectContract::LegDestroy),
        Some(256) => Some(RuntimeEffectContract::ShieldBreak),
        Some(15 | 20 | 252) => Some(RuntimeEffectContract::BlockContentIcon),
        Some(3 | 35) => Some(RuntimeEffectContract::ContentIcon),
        Some(26) => Some(RuntimeEffectContract::PayloadTargetContent),
        Some(8 | 9 | 178 | 261 | 262) => Some(RuntimeEffectContract::PositionTarget),
        Some(142) => Some(RuntimeEffectContract::DropItem),
        Some(200) => Some(RuntimeEffectContract::FloatLength),
        Some(67 | 68 | 122 | 257 | 260) => Some(RuntimeEffectContract::UnitParent),
        _ => None,
    }
}

pub fn effect_contract_name(effect_id: Option<i16>) -> Option<&'static str> {
    effect_contract(effect_id).map(RuntimeEffectContract::name)
}

pub fn observe_runtime_effect_binding_state(
    effect_id: Option<i16>,
    object: Option<&TypeIoObject>,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> Option<EffectRuntimeBindingState> {
    let object = object?;
    let summary = object.effect_summary();
    let parent_ref = summary.first_parent_ref?;
    match parent_ref.semantic_ref {
        TypeIoSemanticRef::Building { build_pos } => {
            if !parent_building_binding_enabled(effect_id)
                || !has_runtime_parent_building(session_state, build_pos)
            {
                Some(EffectRuntimeBindingState::BindingRejected)
            } else {
                Some(EffectRuntimeBindingState::ParentFollow)
            }
        }
        TypeIoSemanticRef::Unit { unit_id } => {
            if matches!(
                effect_contract(effect_id),
                Some(RuntimeEffectContract::LegDestroy)
            ) {
                return Some(EffectRuntimeBindingState::BindingRejected);
            }
            if resolve_parent_unit_position(unit_id, session_state, input_view).is_some() {
                Some(EffectRuntimeBindingState::ParentFollow)
            } else {
                Some(EffectRuntimeBindingState::UnresolvedFallback)
            }
        }
        TypeIoSemanticRef::Content { .. } | TypeIoSemanticRef::TechNode { .. } => None,
    }
}

pub fn observe_runtime_effect_source_binding_state(
    effect_id: Option<i16>,
    object: Option<&TypeIoObject>,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> Option<EffectRuntimeBindingState> {
    if !source_binding_enabled(effect_id) {
        return None;
    }

    let object = object?;
    let summary = object.effect_summary();
    let parent_ref = summary.first_parent_ref?;
    match parent_ref.semantic_ref {
        TypeIoSemanticRef::Unit { unit_id } => {
            if resolve_parent_unit_position(unit_id, session_state, input_view).is_some() {
                Some(EffectRuntimeBindingState::ParentFollow)
            } else {
                Some(EffectRuntimeBindingState::UnresolvedFallback)
            }
        }
        TypeIoSemanticRef::Building { .. }
        | TypeIoSemanticRef::Content { .. }
        | TypeIoSemanticRef::TechNode { .. } => Some(EffectRuntimeBindingState::BindingRejected),
    }
}

pub fn observe_runtime_effect_overlay_binding_state(
    overlay: &RuntimeEffectOverlay,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> Option<EffectRuntimeBindingState> {
    observe_runtime_effect_overlay_binding_state_from_binding(
        overlay.binding.as_ref(),
        session_state,
        input_view,
    )
}

pub fn observe_runtime_effect_overlay_source_binding_state(
    overlay: &RuntimeEffectOverlay,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> Option<EffectRuntimeBindingState> {
    observe_runtime_effect_overlay_binding_state_from_binding(
        overlay.source_binding.as_ref(),
        session_state,
        input_view,
    )
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
    let spawn_position_bits = (x.to_bits(), y.to_bits());
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
    let selected_position_bits = payload_target_content
        .map(|(target_x_bits, target_y_bits, _)| (target_x_bits, target_y_bits))
        .or_else(|| polyline_points.last().copied())
        .or(binding_initial_position)
        .unwrap_or(spawn_position_bits);
    let (x_bits, y_bits) = if world_bits_are_finite(selected_position_bits) {
        selected_position_bits
    } else {
        binding_initial_position
            .filter(|&(x_bits, y_bits)| world_bits_are_finite((x_bits, y_bits)))
            .unwrap_or(spawn_position_bits)
    };

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
    input_view: &EffectRuntimeInputView,
) -> (u32, u32) {
    let overlay_position = (overlay.x_bits, overlay.y_bits);
    overlay
        .binding
        .as_mut()
        .and_then(|binding| {
            resolve_binding_position(
                binding,
                session_state,
                input_view,
                Some(&mut overlay.rotation_bits),
            )
        })
        .unwrap_or(overlay_position)
}

pub fn resolve_runtime_effect_overlay_source_position(
    overlay: &mut RuntimeEffectOverlay,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> (u32, u32) {
    let overlay_position = (overlay.source_x_bits, overlay.source_y_bits);
    overlay
        .source_binding
        .as_mut()
        .and_then(|binding| resolve_binding_position(binding, session_state, input_view, None))
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
        .and_then(position_hint_world_bits);

    if let Some(parent_ref) = summary.first_parent_ref {
        match parent_ref.semantic_ref {
            TypeIoSemanticRef::Building { build_pos } => {
                let world_position_bits = world_bits_from_tile_pos(build_pos);
                if !parent_building_binding_enabled(effect_id) {
                    return Some(DerivedRuntimeEffectBinding {
                        binding: RuntimeEffectBinding::WorldPosition {
                            x_bits: world_position_bits.0,
                            y_bits: world_position_bits.1,
                        },
                        initial_position_bits: Some(world_position_bits),
                    });
                }
                let initial_position_bits = Some((effect_x_bits, effect_y_bits));
                let (spawn_x_bits, spawn_y_bits) = (effect_x_bits, effect_y_bits);
                return Some(DerivedRuntimeEffectBinding {
                    binding: RuntimeEffectBinding::ParentBuilding {
                        build_pos,
                        spawn_x_bits,
                        spawn_y_bits,
                        offset_x_bits: 0.0f32.to_bits(),
                        offset_y_bits: 0.0f32.to_bits(),
                        offset_initialized: false,
                    },
                    initial_position_bits,
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
                        allow_fallback_offset_initialization:
                            parent_binding_allows_fallback_offset_initialization(effect_id),
                        rotate_with_parent: parent_binding_rotates_with_parent(effect_id),
                        parent_rotation_reference_bits: 0.0f32.to_bits(),
                        rotation_offset_bits: 0.0f32.to_bits(),
                        rotation_initialized: false,
                    },
                    initial_position_bits,
                });
            }
            TypeIoSemanticRef::Content { .. } | TypeIoSemanticRef::TechNode { .. } => {}
        }
    }

    if matches!(
        effect_contract(effect_id),
        Some(RuntimeEffectContract::LegDestroy)
    ) {
        return None;
    }

    position_hint_bits.map(|(x_bits, y_bits)| DerivedRuntimeEffectBinding {
        binding: RuntimeEffectBinding::WorldPosition { x_bits, y_bits },
        initial_position_bits: Some((x_bits, y_bits)),
    })
}

fn observe_runtime_effect_overlay_binding_state_from_binding(
    binding: Option<&RuntimeEffectBinding>,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> Option<EffectRuntimeBindingState> {
    match binding {
        Some(RuntimeEffectBinding::ParentBuilding { build_pos, .. }) => {
            has_runtime_parent_building(session_state, *build_pos)
                .then_some(EffectRuntimeBindingState::ParentFollow)
                .or(Some(EffectRuntimeBindingState::BindingRejected))
        }
        Some(RuntimeEffectBinding::ParentUnit { unit_id, .. }) => {
            if resolve_parent_unit_position(*unit_id, session_state, input_view).is_some() {
                Some(EffectRuntimeBindingState::ParentFollow)
            } else {
                Some(EffectRuntimeBindingState::UnresolvedFallback)
            }
        }
        Some(RuntimeEffectBinding::WorldPosition { .. }) | None => None,
    }
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
            rotate_with_parent: false,
            parent_rotation_reference_bits: 0.0f32.to_bits(),
            rotation_offset_bits: 0.0f32.to_bits(),
            rotation_initialized: false,
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
        | RuntimeEffectContract::DrillSteam
        | RuntimeEffectContract::LegDestroy
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
        | RuntimeEffectContract::DrillSteam
        | RuntimeEffectContract::LegDestroy
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
        RuntimeEffectContract::DropItem => object
            .find_first_dfs_bounded(
                EFFECT_PATH_MAX_DEPTH,
                EFFECT_PATH_MAX_NODES,
                drop_item_content_candidate,
            )
            .and_then(|matched| drop_item_content_ref(matched.value)),
        RuntimeEffectContract::PositionTarget
        | RuntimeEffectContract::LightningPath
        | RuntimeEffectContract::PointBeam
        | RuntimeEffectContract::PointHit
        | RuntimeEffectContract::DrillSteam
        | RuntimeEffectContract::LegDestroy
        | RuntimeEffectContract::ShieldBreak
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
    if values.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return None;
    }
    let points = values
        .iter()
        .map(|(x, y)| (x.to_bits(), y.to_bits()))
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
        Some(
            TypeIoSemanticRef::Content { content_type, .. }
            | TypeIoSemanticRef::TechNode { content_type, .. }
        )
            if [BLOCK_CONTENT_TYPE, UNIT_CONTENT_TYPE].contains(&content_type)
    )
}

fn payload_content_ref(value: &TypeIoObject) -> Option<(u8, i16)> {
    match value.semantic_ref()? {
        TypeIoSemanticRef::Content {
            content_type,
            content_id,
        }
        | TypeIoSemanticRef::TechNode {
            content_type,
            content_id,
        } if [BLOCK_CONTENT_TYPE, UNIT_CONTENT_TYPE].contains(&content_type) => {
            Some((content_type, content_id))
        }
        TypeIoSemanticRef::Content { .. }
        | TypeIoSemanticRef::Building { .. }
        | TypeIoSemanticRef::Unit { .. }
        | TypeIoSemanticRef::TechNode { .. } => None,
    }
}

fn drop_item_content_candidate(value: &TypeIoObject) -> bool {
    matches!(
        value.semantic_ref(),
        Some(TypeIoSemanticRef::Content { content_type, .. }) if content_type == ITEM_CONTENT_TYPE
    )
}

fn drop_item_content_ref(value: &TypeIoObject) -> Option<(u8, i16)> {
    match value.semantic_ref()? {
        TypeIoSemanticRef::Content {
            content_type: ITEM_CONTENT_TYPE,
            content_id,
        } => Some((ITEM_CONTENT_TYPE, content_id)),
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
        TypeIoObject::Vec2 { x, y } => finite_world_position_bits(*x, *y),
        TypeIoObject::Vec2Array(values) => values
            .first()
            .and_then(|(x, y)| finite_world_position_bits(*x, *y)),
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

fn position_hint_world_bits(position_hint: &TypeIoEffectPositionHint) -> Option<(u32, u32)> {
    match position_hint {
        TypeIoEffectPositionHint::Point2 { x, y, .. } => {
            let (world_x, world_y) = point2_world_coords(*x, *y);
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        TypeIoEffectPositionHint::PackedPoint2ArrayFirst { packed_point2, .. } => {
            let (tile_x, tile_y) = unpack_point2(*packed_point2);
            let (world_x, world_y) = point2_world_coords(i32::from(tile_x), i32::from(tile_y));
            Some((world_x.to_bits(), world_y.to_bits()))
        }
        TypeIoEffectPositionHint::Vec2 { x_bits, y_bits, .. }
        | TypeIoEffectPositionHint::Vec2ArrayFirst { x_bits, y_bits, .. } => {
            finite_world_position_bits(f32::from_bits(*x_bits), f32::from_bits(*y_bits))
        }
    }
}

fn finite_world_position_bits(x: f32, y: f32) -> Option<(u32, u32)> {
    (x.is_finite() && y.is_finite()).then_some((x.to_bits(), y.to_bits()))
}

fn world_bits_are_finite((x_bits, y_bits): (u32, u32)) -> bool {
    f32::from_bits(x_bits).is_finite() && f32::from_bits(y_bits).is_finite()
}

fn resolve_binding_position(
    binding: &mut RuntimeEffectBinding,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
    overlay_rotation_bits: Option<&mut u32>,
) -> Option<(u32, u32)> {
    let mut overlay_rotation_bits = overlay_rotation_bits;
    match binding {
        RuntimeEffectBinding::WorldPosition { x_bits, y_bits } => Some((*x_bits, *y_bits)),
        RuntimeEffectBinding::ParentBuilding {
            build_pos,
            spawn_x_bits,
            spawn_y_bits,
            offset_x_bits,
            offset_y_bits,
            offset_initialized,
        } => {
            if !has_runtime_parent_building(session_state, *build_pos) {
                return None;
            }
            let (parent_x_bits, parent_y_bits) = world_bits_from_tile_pos(*build_pos);
            if !*offset_initialized {
                *offset_x_bits = coordinate_delta_bits(*spawn_x_bits, parent_x_bits);
                *offset_y_bits = coordinate_delta_bits(*spawn_y_bits, parent_y_bits);
                *offset_initialized = true;
            }
            Some((
                apply_coordinate_offset_bits(parent_x_bits, *offset_x_bits),
                apply_coordinate_offset_bits(parent_y_bits, *offset_y_bits),
            ))
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
            rotate_with_parent,
            parent_rotation_reference_bits,
            rotation_offset_bits,
            rotation_initialized,
        } => {
            let (parent_x_bits, parent_y_bits, position_source) =
                resolve_parent_unit_position(*unit_id, session_state, input_view)?;
            let parent_rotation = if *rotate_with_parent {
                resolve_parent_unit_rotation(*unit_id, session_state, input_view)
            } else {
                None
            };
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
            let (resolved_offset_x_bits, resolved_offset_y_bits) = if *preserve_spawn_offset {
                resolve_parent_unit_offset_bits(
                    *offset_x_bits,
                    *offset_y_bits,
                    *rotate_with_parent,
                    parent_rotation,
                    *allow_fallback_offset_initialization,
                    parent_rotation_reference_bits,
                    rotation_offset_bits,
                    rotation_initialized,
                    &mut overlay_rotation_bits,
                )
            } else {
                (*offset_x_bits, *offset_y_bits)
            };
            if *preserve_spawn_offset {
                Some((
                    apply_coordinate_offset_bits(parent_x_bits, resolved_offset_x_bits),
                    apply_coordinate_offset_bits(parent_y_bits, resolved_offset_y_bits),
                ))
            } else {
                Some((parent_x_bits, parent_y_bits))
            }
        }
    }
}

fn has_runtime_parent_building(session_state: &SessionState, build_pos: i32) -> bool {
    session_state
        .building_table_projection
        .by_build_pos
        .contains_key(&build_pos)
}

fn resolve_parent_unit_position(
    unit_id: i32,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
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
    if input_view.unit_id == Some(unit_id) {
        if let Some((x, y)) = input_view.position {
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

fn resolve_parent_unit_rotation(
    unit_id: i32,
    session_state: &SessionState,
    input_view: &EffectRuntimeInputView,
) -> Option<(u32, ParentUnitRotationSource)> {
    if let Some(entity) = session_state
        .entity_semantic_projection
        .by_entity_id
        .get(&unit_id)
    {
        if let crate::session_state::EntitySemanticProjection::Unit(unit) = &entity.projection {
            return Some((unit.rotation_bits, ParentUnitRotationSource::EntitySemantic));
        }
    }
    (input_view.unit_id == Some(unit_id)).then_some((
        input_view.rotation.to_bits(),
        ParentUnitRotationSource::SnapshotInput,
    ))
}

fn resolve_parent_unit_offset_bits(
    offset_x_bits: u32,
    offset_y_bits: u32,
    rotate_with_parent: bool,
    parent_rotation: Option<(u32, ParentUnitRotationSource)>,
    allow_fallback_offset_initialization: bool,
    parent_rotation_reference_bits: &mut u32,
    rotation_offset_bits: &mut u32,
    rotation_initialized: &mut bool,
    overlay_rotation_bits: &mut Option<&mut u32>,
) -> (u32, u32) {
    if !rotate_with_parent {
        return (offset_x_bits, offset_y_bits);
    }
    let Some((parent_rotation_bits, rotation_source)) = parent_rotation else {
        return (offset_x_bits, offset_y_bits);
    };
    if !*rotation_initialized {
        let can_initialize_rotation = match rotation_source {
            ParentUnitRotationSource::EntitySemantic => true,
            ParentUnitRotationSource::SnapshotInput => allow_fallback_offset_initialization,
        };
        if can_initialize_rotation {
            *parent_rotation_reference_bits = parent_rotation_bits;
            if let Some(rotation_bits_ref) = overlay_rotation_bits.as_deref_mut() {
                *rotation_offset_bits =
                    rotation_delta_bits(*rotation_bits_ref, parent_rotation_bits);
            }
            *rotation_initialized = true;
        }
    }
    if !*rotation_initialized {
        return (offset_x_bits, offset_y_bits);
    }
    if let Some(rotation_bits_ref) = overlay_rotation_bits.as_deref_mut() {
        *rotation_bits_ref =
            apply_rotation_offset_bits(parent_rotation_bits, *rotation_offset_bits);
    }
    let parent_rotation_delta_bits =
        rotation_delta_bits(parent_rotation_bits, *parent_rotation_reference_bits);
    rotate_coordinate_offset_bits(offset_x_bits, offset_y_bits, parent_rotation_delta_bits)
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

fn rotate_coordinate_offset_bits(
    offset_x_bits: u32,
    offset_y_bits: u32,
    rotation_bits: u32,
) -> (u32, u32) {
    let offset_x = f32::from_bits(offset_x_bits);
    let offset_y = f32::from_bits(offset_y_bits);
    let rotation = f32::from_bits(rotation_bits);
    if !(offset_x.is_finite() && offset_y.is_finite() && rotation.is_finite()) {
        return (offset_x_bits, offset_y_bits);
    }
    let radians = rotation.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();
    (
        (offset_x * cos - offset_y * sin).to_bits(),
        (offset_x * sin + offset_y * cos).to_bits(),
    )
}

fn rotation_delta_bits(value_bits: u32, base_bits: u32) -> u32 {
    coordinate_delta_bits(value_bits, base_bits)
}

fn apply_rotation_offset_bits(base_bits: u32, offset_bits: u32) -> u32 {
    apply_coordinate_offset_bits(base_bits, offset_bits)
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

fn parent_building_binding_enabled(effect_id: Option<i16>) -> bool {
    matches!(
        effect_contract(effect_id),
        Some(RuntimeEffectContract::PositionTarget | RuntimeEffectContract::UnitParent)
    )
}

fn parent_binding_allows_fallback_offset_initialization(effect_id: Option<i16>) -> bool {
    matches!(effect_id, Some(67 | 68 | 122 | 257 | 260))
}

fn parent_binding_rotates_with_parent(effect_id: Option<i16>) -> bool {
    matches!(effect_id, Some(67 | 68 | 122 | 257 | 260))
}

fn source_binding_enabled(effect_id: Option<i16>) -> bool {
    matches!(effect_id, Some(8 | 9 | 10 | 178 | 261 | 262))
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
        effect_contract, effect_contract_name, lightning_path_points,
        observe_runtime_effect_binding_state, observe_runtime_effect_overlay_binding_state,
        observe_runtime_effect_source_binding_state, resolve_runtime_effect_overlay_position,
        resolve_runtime_effect_overlay_source_position, spawn_runtime_effect_overlay,
        EffectRuntimeBindingState, EffectRuntimeInputView, RuntimeEffectBinding,
        RuntimeEffectContract,
    };
    use crate::session_state::{
        EntityProjection, EntitySemanticProjection, EntitySemanticProjectionEntry,
        EntityUnitSemanticProjection, SessionState,
    };
    use mdt_typeio::{pack_point2, TypeIoObject};

    #[test]
    fn effect_runtime_contract_maps_drill_steam_effect_id() {
        assert_eq!(
            effect_contract(Some(124)),
            Some(RuntimeEffectContract::DrillSteam)
        );
        assert_eq!(effect_contract_name(Some(124)), Some("drill_steam"));
    }

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
                rotate_with_parent: false,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
            })
        );
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_preserves_parent_building_spawn_position() {
        let build_pos = (10_i32 << 16) | 20_i32;
        let overlay = spawn_runtime_effect_overlay(
            Some(67),
            92.0,
            148.0,
            92.0,
            148.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::BuildingPos(build_pos)),
            10,
        );

        assert_eq!(overlay.x_bits, 92.0f32.to_bits());
        assert_eq!(overlay.y_bits, 148.0f32.to_bits());
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentBuilding {
                build_pos,
                spawn_x_bits: 92.0f32.to_bits(),
                spawn_y_bits: 148.0f32.to_bits(),
                offset_x_bits: 0.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: false,
            })
        );
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_keeps_leg_destroy_unbound_at_packet_origin() {
        let overlay = spawn_runtime_effect_overlay(
            Some(263),
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::PackedPoint2Array(vec![
                (10_i32 << 16) | 20_i32,
            ])),
            90,
        );

        assert_eq!(overlay.contract_name, Some("leg_destroy"));
        assert_eq!(overlay.binding, None);
        assert_eq!(overlay.source_binding, None);
        assert_eq!(overlay.x_bits, 12.0f32.to_bits());
        assert_eq!(overlay.y_bits, 20.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_extracts_payload_target_content_and_position_from_nested_object_array(
    ) {
        let overlay = spawn_runtime_effect_overlay(
            Some(26),
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::ObjectArray(vec![
                    TypeIoObject::ContentRaw {
                        content_type: 1,
                        content_id: 7,
                    },
                    TypeIoObject::Bool(true),
                ]),
                TypeIoObject::ObjectArray(vec![TypeIoObject::Point2 { x: 9, y: 11 }]),
            ])),
            10,
        );

        assert_eq!(overlay.contract_name, Some("payload_target_content"));
        assert_eq!(overlay.content_ref, Some((1, 7)));
        assert_eq!(overlay.x_bits, 72.0f32.to_bits());
        assert_eq!(overlay.y_bits, 88.0f32.to_bits());
        assert!(overlay.binding.is_none());
        assert!(overlay.polyline_points.is_empty());
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_extracts_payload_target_technode_content() {
        let overlay = spawn_runtime_effect_overlay(
            Some(26),
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::TechNodeRaw {
                    content_type: 1,
                    content_id: 33,
                },
                TypeIoObject::Point2 { x: 9, y: 11 },
            ])),
            10,
        );

        assert_eq!(overlay.contract_name, Some("payload_target_content"));
        assert_eq!(overlay.content_ref, Some((1, 33)));
        assert_eq!(overlay.x_bits, 72.0f32.to_bits());
        assert_eq!(overlay.y_bits, 88.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_extracts_content_icon_technode_content_ref() {
        let overlay = spawn_runtime_effect_overlay(
            Some(3),
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::TechNodeRaw {
                content_type: 1,
                content_id: 33,
            }),
            10,
        );

        assert_eq!(overlay.contract_name, Some("content_icon"));
        assert_eq!(overlay.content_ref, Some((1, 33)));
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_extracts_drop_item_content_ref() {
        let overlay = spawn_runtime_effect_overlay(
            Some(142),
            12.0,
            20.0,
            12.0,
            20.0,
            90.0,
            0,
            false,
            Some(&TypeIoObject::ContentRaw {
                content_type: 0,
                content_id: 12,
            }),
            10,
        );

        assert_eq!(overlay.contract_name, Some("drop_item"));
        assert_eq!(overlay.content_ref, Some((0, 12)));
    }

    #[test]
    fn effect_runtime_payload_target_vec2_non_finite_falls_back_to_spawn_position() {
        let overlay = spawn_runtime_effect_overlay(
            Some(26),
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 7,
                },
                TypeIoObject::Vec2 {
                    x: f32::NAN,
                    y: 160.0,
                },
            ])),
            10,
        );

        assert_eq!(overlay.x_bits, 12.0f32.to_bits());
        assert_eq!(overlay.y_bits, 20.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_payload_target_vec2_array_non_finite_falls_back_to_spawn_position() {
        let overlay = spawn_runtime_effect_overlay(
            Some(26),
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 7,
                },
                TypeIoObject::Vec2Array(vec![(f32::INFINITY, 160.0)]),
            ])),
            10,
        );

        assert_eq!(overlay.x_bits, 12.0f32.to_bits());
        assert_eq!(overlay.y_bits, 20.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_non_finite_initial_position_falls_back_to_spawn_position(
    ) {
        let overlay = spawn_runtime_effect_overlay(
            None,
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::UnitId(9999),
                TypeIoObject::Vec2 {
                    x: f32::NAN,
                    y: 160.0,
                },
            ])),
            10,
        );

        assert_eq!(overlay.x_bits, 12.0f32.to_bits());
        assert_eq!(overlay.y_bits, 20.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_spawn_runtime_effect_overlay_rejects_non_finite_world_position_hint() {
        let overlay = spawn_runtime_effect_overlay(
            None,
            12.0,
            20.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::Vec2 {
                x: f32::NAN,
                y: 160.0,
            }),
            10,
        );

        assert_eq!(overlay.x_bits, 12.0f32.to_bits());
        assert_eq!(overlay.y_bits, 20.0f32.to_bits());
        assert!(overlay.binding.is_none());
    }

    #[test]
    fn effect_runtime_lightning_path_points_rejects_non_finite_points() {
        assert_eq!(
            lightning_path_points(&TypeIoObject::Vec2Array(vec![
                (1.0, 2.0),
                (f32::INFINITY, 4.5),
            ])),
            None
        );
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_lazily_freezes_parent_building_offset(
    ) {
        let build_pos = (10_i32 << 16) | 20_i32;
        let mut overlay = spawn_runtime_effect_overlay(
            Some(67),
            92.0,
            148.0,
            92.0,
            148.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::BuildingPos(build_pos)),
            10,
        );
        let state = session_state_with_building(build_pos);
        let input = EffectRuntimeInputView::default();

        let resolved = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);

        assert_eq!(resolved, (92.0f32.to_bits(), 148.0f32.to_bits()));
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentBuilding {
                build_pos,
                spawn_x_bits: 92.0f32.to_bits(),
                spawn_y_bits: 148.0f32.to_bits(),
                offset_x_bits: 12.0f32.to_bits(),
                offset_y_bits: (-12.0f32).to_bits(),
                offset_initialized: true,
            })
        );
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_keeps_spawn_position_when_parent_building_is_missing(
    ) {
        let build_pos = (10_i32 << 16) | 20_i32;
        let mut overlay = spawn_runtime_effect_overlay(
            Some(67),
            92.0,
            148.0,
            92.0,
            148.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::BuildingPos(build_pos)),
            10,
        );

        let resolved = resolve_runtime_effect_overlay_position(
            &mut overlay,
            &SessionState::default(),
            &EffectRuntimeInputView::default(),
        );

        assert_eq!(resolved, (92.0f32.to_bits(), 148.0f32.to_bits()));
        assert_eq!(
            observe_runtime_effect_overlay_binding_state(
                &overlay,
                &SessionState::default(),
                &EffectRuntimeInputView::default(),
            ),
            Some(EffectRuntimeBindingState::BindingRejected)
        );
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentBuilding {
                build_pos,
                spawn_x_bits: 92.0f32.to_bits(),
                spawn_y_bits: 148.0f32.to_bits(),
                offset_x_bits: 0.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: false,
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
        let input = EffectRuntimeInputView::default();
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
                allow_fallback_offset_initialization: true,
                rotate_with_parent: true,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
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
        let input = EffectRuntimeInputView::default();
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
                rotate_with_parent: false,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
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
    fn effect_runtime_resolve_runtime_effect_overlay_source_position_follows_parent_unit_for_point_beam(
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(10),
            80.0,
            160.0,
            12.0,
            20.0,
            0.0,
            0,
            false,
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::UnitId(404),
                TypeIoObject::Point2 { x: 10, y: 20 },
            ])),
            10,
        );
        let input = EffectRuntimeInputView::default();
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
                rotate_with_parent: false,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
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
    fn effect_runtime_resolve_runtime_effect_overlay_source_position_follows_parent_unit_for_regen_suppress_seek(
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(178),
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
        let input = EffectRuntimeInputView::default();
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
                rotate_with_parent: false,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
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
    fn effect_runtime_resolve_runtime_effect_overlay_source_position_follows_parent_unit_for_chain_lightning(
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(261),
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
        let input = EffectRuntimeInputView::default();
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
    fn effect_runtime_resolve_runtime_effect_overlay_source_position_follows_parent_unit_for_chain_emp(
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(262),
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
        let input = EffectRuntimeInputView::default();
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
        let input = EffectRuntimeInputView {
            unit_id: Some(404),
            position: Some((80.0, 160.0)),
            ..EffectRuntimeInputView::default()
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
                rotate_with_parent: false,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
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

    fn session_state_with_unit_entity(unit_id: i32, x: f32, y: f32) -> SessionState {
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            unit_id,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: x.to_bits(),
                y_bits: y.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state
    }

    fn session_state_with_building(build_pos: i32) -> SessionState {
        let mut state = SessionState::default();
        state.building_table_projection.seed_world_baseline(
            build_pos,
            0x0101,
            Some("conveyor".to_string()),
            0,
            1,
            None,
            None,
            None,
            None,
            None,
            None,
            1.0f32.to_bits(),
            None,
            None,
            None,
            None,
        );
        state
    }

    #[test]
    fn effect_runtime_observe_runtime_effect_binding_state_rejects_lightning_building_parent() {
        let object = TypeIoObject::BuildingPos(pack_point2(7, 11));

        assert_eq!(
            observe_runtime_effect_binding_state(
                Some(13),
                Some(&object),
                &SessionState::default(),
                &EffectRuntimeInputView::default(),
            ),
            Some(EffectRuntimeBindingState::BindingRejected)
        );
    }

    #[test]
    fn effect_runtime_observe_runtime_effect_binding_state_follows_unit_parent_buildings_and_rejects_leg_destroy_unit(
    ) {
        let building_object = TypeIoObject::BuildingPos(pack_point2(7, 11));
        let building_state = session_state_with_building(pack_point2(7, 11));
        for effect_id in [67i16, 68, 122, 257, 260] {
            assert_eq!(
                observe_runtime_effect_binding_state(
                    Some(effect_id),
                    Some(&building_object),
                    &building_state,
                    &EffectRuntimeInputView::default(),
                ),
                Some(EffectRuntimeBindingState::ParentFollow)
            );
            assert_eq!(
                observe_runtime_effect_binding_state(
                    Some(effect_id),
                    Some(&building_object),
                    &SessionState::default(),
                    &EffectRuntimeInputView::default(),
                ),
                Some(EffectRuntimeBindingState::BindingRejected)
            );
        }

        let unit_object = TypeIoObject::UnitId(404);
        assert_eq!(
            observe_runtime_effect_binding_state(
                Some(263),
                Some(&unit_object),
                &session_state_with_unit_entity(404, 32.0, 48.0),
                &EffectRuntimeInputView::default(),
            ),
            Some(EffectRuntimeBindingState::BindingRejected)
        );
    }

    #[test]
    fn effect_runtime_observe_runtime_effect_source_binding_state_tracks_unit_follow_and_fallback()
    {
        let object = TypeIoObject::UnitId(404);
        let followed = session_state_with_unit_entity(404, 32.0, 48.0);
        for effect_id in [8i16, 9, 10, 178, 261, 262] {
            assert_eq!(
                observe_runtime_effect_source_binding_state(
                    Some(effect_id),
                    Some(&object),
                    &followed,
                    &EffectRuntimeInputView::default(),
                ),
                Some(EffectRuntimeBindingState::ParentFollow)
            );
            assert_eq!(
                observe_runtime_effect_source_binding_state(
                    Some(effect_id),
                    Some(&object),
                    &SessionState::default(),
                    &EffectRuntimeInputView::default(),
                ),
                Some(EffectRuntimeBindingState::UnresolvedFallback)
            );
        }
    }

    #[test]
    fn effect_runtime_observe_runtime_effect_source_binding_state_rejects_buildings_and_leaves_unreachable_parent_kinds_empty(
    ) {
        let building_object = TypeIoObject::BuildingPos(pack_point2(3, 5));
        let content_object = TypeIoObject::ContentRaw {
            content_type: 1,
            content_id: 7,
        };
        let technode_object = TypeIoObject::TechNodeRaw {
            content_type: 2,
            content_id: 9,
        };

        for effect_id in [8i16, 9, 10, 178, 261, 262] {
            assert_eq!(
                observe_runtime_effect_source_binding_state(
                    Some(effect_id),
                    Some(&building_object),
                    &SessionState::default(),
                    &EffectRuntimeInputView::default(),
                ),
                Some(EffectRuntimeBindingState::BindingRejected)
            );
            assert_eq!(
                observe_runtime_effect_source_binding_state(
                    Some(effect_id),
                    Some(&content_object),
                    &SessionState::default(),
                    &EffectRuntimeInputView::default(),
                ),
                None
            );
            assert_eq!(
                observe_runtime_effect_source_binding_state(
                    Some(effect_id),
                    Some(&technode_object),
                    &SessionState::default(),
                    &EffectRuntimeInputView::default(),
                ),
                None
            );
        }

        let unit_object = TypeIoObject::UnitId(404);
        assert_eq!(
            observe_runtime_effect_source_binding_state(
                Some(67),
                Some(&unit_object),
                &SessionState::default(),
                &EffectRuntimeInputView::default(),
            ),
            None
        );
    }

    fn assert_effect_runtime_rotates_offset_with_parent_unit(
        effect_id: i16,
        allow_fallback_offset_initialization: bool,
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(effect_id),
            12.0,
            20.0,
            12.0,
            20.0,
            15.0,
            0,
            false,
            Some(&TypeIoObject::UnitId(404)),
            10,
        );
        let input = EffectRuntimeInputView::default();
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 10.0f32.to_bits(),
                y_bits: 20.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            EntitySemanticProjectionEntry {
                class_id: 12,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 1,
                    unit_type_id: 55,
                    health_bits: 0,
                    rotation_bits: 0.0f32.to_bits(),
                    shield_bits: 0,
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            },
        );

        let first_position = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);
        assert_eq!(first_position, (12.0f32.to_bits(), 20.0f32.to_bits()));
        assert_eq!(overlay.rotation_bits, 15.0f32.to_bits());
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 12.0f32.to_bits(),
                spawn_y_bits: 20.0f32.to_bits(),
                offset_x_bits: 2.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization,
                rotate_with_parent: true,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 15.0f32.to_bits(),
                rotation_initialized: true,
            })
        );

        state
            .entity_table_projection
            .by_entity_id
            .get_mut(&404)
            .expect("missing entity 404")
            .x_bits = 16.0f32.to_bits();
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            EntitySemanticProjectionEntry {
                class_id: 12,
                last_seen_entity_snapshot_count: 2,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 1,
                    unit_type_id: 55,
                    health_bits: 0,
                    rotation_bits: 90.0f32.to_bits(),
                    shield_bits: 0,
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            },
        );

        let second_position = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);
        assert_eq!(second_position, (16.0f32.to_bits(), 22.0f32.to_bits()));
        assert_eq!(overlay.rotation_bits, 105.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_rotates_offset_with_parent_unit() {
        assert_effect_runtime_rotates_offset_with_parent_unit(67, true);
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_rotates_offset_with_parent_unit_for_arc_shield_break(
    ) {
        assert_effect_runtime_rotates_offset_with_parent_unit(257, true);
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_rotates_offset_with_parent_unit_for_unit_shield_break(
    ) {
        assert_effect_runtime_rotates_offset_with_parent_unit(260, true);
    }

    fn assert_effect_runtime_rotates_offset_with_fallback_parent_unit(
        effect_id: i16,
        use_world_player_position: bool,
    ) {
        let mut overlay = spawn_runtime_effect_overlay(
            Some(effect_id),
            46.0,
            60.0,
            46.0,
            60.0,
            15.0,
            0,
            false,
            Some(&TypeIoObject::UnitId(404)),
            10,
        );
        let mut input = EffectRuntimeInputView {
            unit_id: Some(404),
            position: (!use_world_player_position).then_some((44.0, 60.0)),
            rotation: 0.0,
        };
        let mut state = SessionState::default();
        if use_world_player_position {
            state.world_player_x_bits = Some(44.0f32.to_bits());
            state.world_player_y_bits = Some(60.0f32.to_bits());
        }

        let first_position = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);
        assert_eq!(first_position, (46.0f32.to_bits(), 60.0f32.to_bits()));
        assert_eq!(overlay.rotation_bits, 15.0f32.to_bits());
        assert_eq!(
            overlay.binding,
            Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 46.0f32.to_bits(),
                spawn_y_bits: 60.0f32.to_bits(),
                offset_x_bits: 2.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization: true,
                rotate_with_parent: true,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 15.0f32.to_bits(),
                rotation_initialized: true,
            })
        );

        input.rotation = 90.0;
        if use_world_player_position {
            state.world_player_x_bits = Some(50.0f32.to_bits());
            state.world_player_y_bits = Some(60.0f32.to_bits());
        } else {
            input.position = Some((50.0, 60.0));
        }

        let second_position = resolve_runtime_effect_overlay_position(&mut overlay, &state, &input);
        assert_eq!(second_position, (50.0f32.to_bits(), 62.0f32.to_bits()));
        assert_eq!(overlay.rotation_bits, 105.0f32.to_bits());
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_preserves_snapshot_input_offset_for_arc_shield_break(
    ) {
        assert_effect_runtime_rotates_offset_with_fallback_parent_unit(257, false);
    }

    #[test]
    fn effect_runtime_resolve_runtime_effect_overlay_position_preserves_world_player_offset_for_unit_shield_break(
    ) {
        assert_effect_runtime_rotates_offset_with_fallback_parent_unit(260, true);
    }
}
