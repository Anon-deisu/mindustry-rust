use crate::effect_runtime::{RuntimeEffectBinding, RuntimeEffectContract, RuntimeEffectOverlay};
use crate::session_state::{EffectBusinessProjection, EntitySemanticProjection, SessionState};
use mdt_typeio::{TypeIoObject, TypeIoSemanticRef};

const EFFECT_CONTRACT_MAX_DEPTH: usize = 3;
const EFFECT_CONTRACT_MAX_NODES: usize = 64;
const BLOCK_CONTENT_TYPE: u8 = 1;
const ITEM_CONTENT_TYPE: u8 = 0;
const UNIT_CONTENT_TYPE: u8 = 6;
const DROP_ITEM_EFFECT_LENGTH: f32 = 20.0;
#[cfg(test)]
const PAYLOAD_DEPOSIT_EFFECT_ID: i16 = 26;
const LIGHTNING_EFFECT_ID: i16 = 13;
const MOVE_COMMAND_EFFECT_ID: i16 = 12;
const UNIT_SPIRIT_EFFECT_ID: i16 = 8;
const ITEM_TRANSFER_EFFECT_ID: i16 = 9;
const POINT_BEAM_EFFECT_ID: i16 = 10;
const POINT_HIT_EFFECT_ID: i16 = 11;
const REGEN_SUPPRESS_SEEK_EFFECT_ID: i16 = 178;
const FLOAT_LENGTH_EFFECT_ID: i16 = 200;
const LEG_DESTROY_EFFECT_ID: i16 = 263;
const DRILL_STEAM_EFFECT_ID: i16 = 124;
const GREEN_LASER_CHARGE_EFFECT_ID: i16 = 67;
const GREEN_LASER_CHARGE_SMALL_EFFECT_ID: i16 = 68;
const NEOPLASM_HEAL_EFFECT_ID: i16 = 122;
const SHIELD_BREAK_EFFECT_ID: i16 = 256;
const ARC_SHIELD_BREAK_EFFECT_ID: i16 = 257;
const UNIT_SHIELD_BREAK_EFFECT_ID: i16 = 260;
const CHAIN_LIGHTNING_EFFECT_ID: i16 = 261;
const CHAIN_EMP_EFFECT_ID: i16 = 262;
const CHAIN_SEGMENT_TARGET_PIXELS: f32 = 24.0;
const CHAIN_MIN_SEGMENTS: usize = 3;
const CHAIN_MAX_SEGMENTS: usize = 8;
const ITEM_TRANSFER_CIRCLE_SEGMENT_COUNT: usize = 8;
const ITEM_TRANSFER_LATERAL_OFFSET_MAX: f32 = 10.0;
const ITEM_TRANSFER_OUTER_RADIUS: f32 = 3.0;
const ITEM_TRANSFER_INNER_RADIUS: f32 = 1.5;
const REGEN_SUPPRESS_SEEK_PATH_SEGMENT_COUNT: usize = 6;
const REGEN_SUPPRESS_SEEK_LATERAL_OFFSET_MAX: f32 = 50.0;
const UNIT_SPIRIT_SIDE_COUNT: usize = 4;
const UNIT_SPIRIT_BASE_RADIUS: f32 = 2.5;
const UNIT_SPIRIT_OUTER_RADIUS_SCALE: f32 = 1.5;
const DRILL_STEAM_PARTICLE_COUNT: usize = 3;
const DRILL_STEAM_RING_SEGMENT_COUNT: usize = 8;
const DRILL_STEAM_MIN_LENGTH: f32 = 3.0;
const DRILL_STEAM_LENGTH_GROWTH: f32 = 20.0;
const DRILL_STEAM_MIN_RADIUS: f32 = 1.3;
const DRILL_STEAM_RADIUS_GROWTH: f32 = 2.4;
const DRILL_STEAM_FSLOPE_GROWTH: f32 = 1.2;
const GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT: usize = 12;
const GREEN_LASER_CHARGE_SPOKE_COUNT: usize = 4;
const GREEN_LASER_CHARGE_RADIUS_BASE: f32 = 4.0;
const GREEN_LASER_CHARGE_RADIUS_GROWTH: f32 = 100.0;
const GREEN_LASER_CHARGE_SMALL_RADIUS_GROWTH: f32 = 50.0;
const GREEN_LASER_CHARGE_SPOKE_RADIUS: f32 = 40.0;
const NEOPLASM_HEAL_DIAMOND_SIDE_COUNT: usize = 4;
const NEOPLASM_HEAL_OFFSET_MAX: f32 = 3.0;
const NEOPLASM_HEAL_RADIUS_BASE: f32 = 0.2;
const NEOPLASM_HEAL_RADIUS_GROWTH: f32 = 2.0;
const SHIELD_BREAK_SIDE_COUNT: usize = 6;
const SHIELD_BREAK_RADIUS_GROWTH: f32 = 1.0;
const ARC_SHIELD_BREAK_SEGMENT_COUNT: usize = 8;
const ARC_SHIELD_BREAK_SWEEP_DEGREES: f32 = 140.0;
const ARC_SHIELD_BREAK_BASE_RADIUS: f32 = 16.0;
const ARC_SHIELD_BREAK_BAND_WIDTH: f32 = 4.0;
const ARC_SHIELD_BREAK_RADIUS_GROWTH: f32 = 2.0;
const UNIT_SHIELD_BREAK_CIRCLE_SEGMENT_COUNT: usize = 12;
const UNIT_SHIELD_BREAK_BASE_RADIUS: f32 = 14.0;
const UNIT_SHIELD_BREAK_RADIUS_GROWTH: f32 = 3.0;
const UNIT_SHIELD_BREAK_BURST_COUNT: usize = 8;
const UNIT_SHIELD_BREAK_BURST_INSET: f32 = 4.0;
const UNIT_SHIELD_BREAK_BURST_LENGTH: f32 = 6.0;
const UNIT_SHIELD_BREAK_BURST_GROWTH: f32 = 5.0;
const POINT_HIT_CIRCLE_SEGMENT_COUNT: usize = 12;
const POINT_HIT_MAX_RADIUS: f32 = 6.0;

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

const POINT_HIT_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "point_hit",
    overlay_origin: unsupported_overlay_origin,
    business_world_position: unsupported_business_world_position,
};

const MOVE_COMMAND_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "move_command",
    overlay_origin: position_target_overlay_origin,
    business_world_position: position_target_business_world_position,
};

const DRILL_STEAM_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "drill_steam",
    overlay_origin: unsupported_overlay_origin,
    business_world_position: unsupported_business_world_position,
};

const LEG_DESTROY_EXECUTOR: RuntimeEffectContractExecutor = RuntimeEffectContractExecutor {
    contract_name: "leg_destroy",
    overlay_origin: leg_destroy_overlay_origin,
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
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
    session_state: &SessionState,
) -> Vec<RuntimeEffectLineProjection> {
    match overlay.effect_id {
        Some(LIGHTNING_EFFECT_ID) => lightning_line_projections(&overlay.polyline_points),
        Some(MOVE_COMMAND_EFFECT_ID) => move_command_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(UNIT_SPIRIT_EFFECT_ID) => unit_spirit_line_projections(
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(ITEM_TRANSFER_EFFECT_ID) => item_transfer_line_projections(
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(REGEN_SUPPRESS_SEEK_EFFECT_ID) => regen_suppress_seek_line_projections(
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        ),
        Some(POINT_BEAM_EFFECT_ID) => vec![RuntimeEffectLineProjection {
            kind: "point-beam",
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        }],
        Some(FLOAT_LENGTH_EFFECT_ID) => vec![RuntimeEffectLineProjection {
            kind: "float-length",
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        }],
        Some(LEG_DESTROY_EFFECT_ID) => vec![RuntimeEffectLineProjection {
            kind: "leg-destroy",
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        }],
        Some(POINT_HIT_EFFECT_ID) => point_hit_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(DRILL_STEAM_EFFECT_ID) => drill_steam_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.color_rgba,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(GREEN_LASER_CHARGE_EFFECT_ID) => green_laser_charge_line_projections(
            target_x_bits,
            target_y_bits,
            unit_parent_rotation_bits(overlay, session_state).unwrap_or(overlay.rotation_bits),
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(GREEN_LASER_CHARGE_SMALL_EFFECT_ID) => green_laser_charge_small_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(NEOPLASM_HEAL_EFFECT_ID) => neoplasm_heal_line_projections(
            target_x_bits,
            target_y_bits,
            unit_parent_rotation_bits(overlay, session_state).unwrap_or(overlay.rotation_bits),
            overlay.color_rgba,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(SHIELD_BREAK_EFFECT_ID) => shield_break_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.rotation_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(ARC_SHIELD_BREAK_EFFECT_ID) => arc_shield_break_line_projections(
            target_x_bits,
            target_y_bits,
            unit_parent_rotation_bits(overlay, session_state).unwrap_or(overlay.rotation_bits),
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(UNIT_SHIELD_BREAK_EFFECT_ID) => unit_shield_break_line_projections(
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        Some(effect_id @ (CHAIN_LIGHTNING_EFFECT_ID | CHAIN_EMP_EFFECT_ID)) => {
            chain_line_kind(effect_id)
                .map(|kind| {
                    chain_line_projections(
                        kind,
                        source_x_bits,
                        source_y_bits,
                        target_x_bits,
                        target_y_bits,
                    )
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn marker_position_for_effect_overlay(
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
) -> Option<(u32, u32)> {
    match overlay.effect_id {
        Some(ITEM_TRANSFER_EFFECT_ID) => item_transfer_geometry(
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        )
        .map(|(center_x, center_y, _, _)| (center_x.to_bits(), center_y.to_bits())),
        Some(REGEN_SUPPRESS_SEEK_EFFECT_ID) => regen_suppress_seek_marker_position(
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        ),
        _ => None,
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
        (Some("drop_item"), Some((ITEM_CONTENT_TYPE, content_id))) => {
            vec![RuntimeEffectContentProjection {
                kind: "drop-item",
                content_type: ITEM_CONTENT_TYPE,
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

fn item_transfer_line_projections(
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let Some((center_x, center_y, outer_radius, inner_radius)) = item_transfer_geometry(
        overlay,
        source_x_bits,
        source_y_bits,
        target_x_bits,
        target_y_bits,
        remaining_ticks,
        lifetime_ticks,
    ) else {
        return Vec::new();
    };

    let outer_points = regular_polygon_points(
        center_x,
        center_y,
        outer_radius,
        ITEM_TRANSFER_CIRCLE_SEGMENT_COUNT,
        0.0,
    );
    let inner_points = regular_polygon_points(
        center_x,
        center_y,
        inner_radius,
        ITEM_TRANSFER_CIRCLE_SEGMENT_COUNT,
        0.0,
    );

    let mut lines = closed_polyline_line_projections("item-transfer", &outer_points);
    lines.extend(closed_polyline_line_projections(
        "item-transfer",
        &inner_points,
    ));
    lines
}

fn regen_suppress_seek_line_projections(
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
) -> Vec<RuntimeEffectLineProjection> {
    let Some((source_x, source_y, control_x, control_y, target_x, target_y)) =
        regen_suppress_seek_curve_points(
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        )
    else {
        return Vec::new();
    };

    let points = (0..=REGEN_SUPPRESS_SEEK_PATH_SEGMENT_COUNT)
        .map(|index| {
            let t = index as f32 / REGEN_SUPPRESS_SEEK_PATH_SEGMENT_COUNT as f32;
            let (x, y) = quadratic_bezier_point(
                source_x, source_y, control_x, control_y, target_x, target_y, t,
            );
            (x.to_bits(), y.to_bits())
        })
        .collect::<Vec<_>>();

    polyline_line_projections("regen-suppress-seek", &points)
}

fn regen_suppress_seek_marker_position(
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Option<(u32, u32)> {
    let (source_x, source_y, control_x, control_y, target_x, target_y) =
        regen_suppress_seek_curve_points(
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        )?;
    let curve_t = 1.0 - inclusive_overlay_progress(remaining_ticks, lifetime_ticks);
    let (x, y) = quadratic_bezier_point(
        source_x, source_y, control_x, control_y, target_x, target_y, curve_t,
    );
    Some((x.to_bits(), y.to_bits()))
}

fn regen_suppress_seek_curve_points(
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
) -> Option<(f32, f32, f32, f32, f32, f32)> {
    let source_x = f32::from_bits(source_x_bits);
    let source_y = f32::from_bits(source_y_bits);
    let target_x = f32::from_bits(target_x_bits);
    let target_y = f32::from_bits(target_y_bits);
    if !source_x.is_finite()
        || !source_y.is_finite()
        || !target_x.is_finite()
        || !target_y.is_finite()
    {
        return None;
    }

    let dx = target_x - source_x;
    let dy = target_y - source_y;
    let distance = (dx * dx + dy * dy).sqrt();
    let (control_x, control_y) = if distance.is_finite() && distance > f32::EPSILON {
        let lateral =
            effect_overlay_signed_seed(overlay, 0.0) * REGEN_SUPPRESS_SEEK_LATERAL_OFFSET_MAX;
        let normal_x = -dy / distance;
        let normal_y = dx / distance;
        (source_x + normal_x * lateral, source_y + normal_y * lateral)
    } else {
        (source_x, source_y)
    };

    Some((source_x, source_y, control_x, control_y, target_x, target_y))
}

fn item_transfer_geometry(
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Option<(f32, f32, f32, f32)> {
    let source_x = f32::from_bits(source_x_bits);
    let source_y = f32::from_bits(source_y_bits);
    let target_x = f32::from_bits(target_x_bits);
    let target_y = f32::from_bits(target_y_bits);
    if !source_x.is_finite()
        || !source_y.is_finite()
        || !target_x.is_finite()
        || !target_y.is_finite()
    {
        return None;
    }

    let progress = inclusive_overlay_progress(remaining_ticks, lifetime_ticks);
    let slope = midlife_slope(progress);
    let outer_radius = slope * ITEM_TRANSFER_OUTER_RADIUS;
    let inner_radius = slope * ITEM_TRANSFER_INNER_RADIUS;
    let path_t = progress.powi(3);
    let (mut center_x, mut center_y) = lerp_point(source_x, source_y, target_x, target_y, path_t);

    let dx = target_x - source_x;
    let dy = target_y - source_y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance.is_finite() && distance > f32::EPSILON {
        let normal_x = -dy / distance;
        let normal_y = dx / distance;
        let lateral =
            effect_overlay_signed_seed(overlay, 0.0) * slope * ITEM_TRANSFER_LATERAL_OFFSET_MAX;
        center_x += normal_x * lateral;
        center_y += normal_y * lateral;
    }

    (center_x.is_finite() && center_y.is_finite()).then_some((
        center_x,
        center_y,
        outer_radius,
        inner_radius,
    ))
}

fn effect_overlay_delta_bits(value_bits: u32, base_bits: u32) -> u32 {
    let value = f32::from_bits(value_bits);
    let base = f32::from_bits(base_bits);
    if value.is_finite() && base.is_finite() {
        (value - base).to_bits()
    } else {
        0.0f32.to_bits()
    }
}

fn effect_overlay_binding_seed(binding: Option<&RuntimeEffectBinding>) -> u32 {
    match binding {
        Some(RuntimeEffectBinding::ParentBuilding { build_pos, .. }) => {
            0x4b1d_0001 ^ (*build_pos as u32).rotate_left(7)
        }
        Some(RuntimeEffectBinding::ParentUnit { unit_id, .. }) => {
            0x7f4a_7c15 ^ (*unit_id as u32).rotate_left(11)
        }
        Some(RuntimeEffectBinding::WorldPosition { .. }) => 0x6d2b_79f5,
        None => 0,
    }
}

fn mix_effect_overlay_seed(mut hash: u32) -> u32 {
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;
    hash
}

fn item_transfer_overlay_seed(overlay: &RuntimeEffectOverlay) -> u32 {
    let delta_x_bits = effect_overlay_delta_bits(overlay.x_bits, overlay.source_x_bits);
    let delta_y_bits = effect_overlay_delta_bits(overlay.y_bits, overlay.source_y_bits);
    let mut hash = overlay
        .effect_id
        .map(|effect_id| effect_id as u16 as u32)
        .unwrap_or_default()
        ^ delta_x_bits.rotate_left(19)
        ^ delta_y_bits.rotate_left(23)
        ^ overlay.rotation_bits.rotate_left(3)
        ^ overlay.color_rgba.rotate_left(11)
        ^ u32::from(overlay.lifetime_ticks).rotate_left(27)
        ^ u32::from(overlay.reliable).rotate_left(29)
        ^ u32::from(overlay.has_data).rotate_left(31)
        ^ u32::try_from(overlay.polyline_points.len())
            .unwrap_or(u32::MAX)
            .rotate_left(17)
        ^ effect_overlay_binding_seed(overlay.source_binding.as_ref()).rotate_left(5)
        ^ effect_overlay_binding_seed(overlay.binding.as_ref()).rotate_left(9);
    if let Some((content_type, content_id)) = overlay.content_ref {
        hash ^= u32::from(content_type).rotate_left(9) ^ (content_id as u16 as u32).rotate_left(15);
    }
    mix_effect_overlay_seed(hash)
}

fn effect_overlay_instance_seed(overlay: &RuntimeEffectOverlay) -> u32 {
    if overlay.effect_id == Some(ITEM_TRANSFER_EFFECT_ID) {
        return item_transfer_overlay_seed(overlay);
    }
    let mut hash = overlay
        .effect_id
        .map(|effect_id| effect_id as u16 as u32)
        .unwrap_or_default()
        ^ overlay.source_x_bits.rotate_left(7)
        ^ overlay.source_y_bits.rotate_left(13)
        ^ overlay.x_bits.rotate_left(19)
        ^ overlay.y_bits.rotate_left(23)
        ^ overlay.rotation_bits.rotate_left(3)
        ^ overlay.color_rgba.rotate_left(11)
        ^ u32::from(overlay.lifetime_ticks).rotate_left(27)
        ^ u32::from(overlay.reliable).rotate_left(29)
        ^ u32::from(overlay.has_data).rotate_left(31)
        ^ u32::try_from(overlay.polyline_points.len())
            .unwrap_or(u32::MAX)
            .rotate_left(17);
    if let Some((content_type, content_id)) = overlay.content_ref {
        hash ^= u32::from(content_type).rotate_left(9) ^ (content_id as u16 as u32).rotate_left(15);
    }
    mix_effect_overlay_seed(hash)
}

fn effect_overlay_signed_seed(overlay: &RuntimeEffectOverlay, min_abs: f32) -> f32 {
    let signed = effect_overlay_instance_seed(overlay) as f32 / u32::MAX as f32 * 2.0 - 1.0;
    if min_abs > 0.0 && signed.abs() < min_abs {
        if signed.is_sign_negative() {
            -min_abs
        } else {
            min_abs
        }
    } else {
        signed
    }
}

fn neoplasm_heal_seed_angle(
    center_x_bits: u32,
    center_y_bits: u32,
    rotation_bits: u32,
    color_rgba: u32,
) -> f32 {
    let mut hash = center_x_bits
        ^ center_y_bits.rotate_left(7)
        ^ rotation_bits.rotate_left(13)
        ^ color_rgba.rotate_left(21)
        ^ (NEOPLASM_HEAL_EFFECT_ID as u16 as u32).rotate_left(3);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;

    hash as f32 / u32::MAX as f32 * std::f32::consts::TAU
}

fn drill_steam_particle_seed(
    center_x_bits: u32,
    center_y_bits: u32,
    color_rgba: u32,
    index: usize,
    salt: u32,
) -> f32 {
    let mut hash = center_x_bits
        ^ center_y_bits.rotate_left(7)
        ^ color_rgba.rotate_left(13)
        ^ (DRILL_STEAM_EFFECT_ID as u16 as u32).rotate_left(21)
        ^ (index as u32).rotate_left(3)
        ^ salt.rotate_left(11);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;

    hash as f32 / u32::MAX as f32
}

fn unit_spirit_line_projections(
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
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

    let progress = inclusive_overlay_progress(remaining_ticks, lifetime_ticks);
    let outer_center = lerp_point(source_x, source_y, target_x, target_y, progress.powi(2));
    let inner_center = lerp_point(source_x, source_y, target_x, target_y, progress.powi(5));
    let base_radius = UNIT_SPIRIT_BASE_RADIUS * progress;
    let outer_points = regular_polygon_points(
        outer_center.0,
        outer_center.1,
        base_radius * UNIT_SPIRIT_OUTER_RADIUS_SCALE,
        UNIT_SPIRIT_SIDE_COUNT,
        std::f32::consts::FRAC_PI_4,
    );
    let inner_points = regular_polygon_points(
        inner_center.0,
        inner_center.1,
        base_radius,
        UNIT_SPIRIT_SIDE_COUNT,
        std::f32::consts::FRAC_PI_4,
    );

    let mut lines = closed_polyline_line_projections("unit-spirit", &outer_points);
    lines.extend(closed_polyline_line_projections(
        "unit-spirit",
        &inner_points,
    ));
    lines
}

fn shield_break_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    rotation_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    let base_radius = f32::from_bits(rotation_bits);
    if !center_x.is_finite() || !center_y.is_finite() || !base_radius.is_finite() {
        return Vec::new();
    }

    let radius = (base_radius
        + shield_break_progress(remaining_ticks, lifetime_ticks) * SHIELD_BREAK_RADIUS_GROWTH)
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

fn point_hit_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let radius = point_hit_progress(remaining_ticks, lifetime_ticks) * POINT_HIT_MAX_RADIUS;
    let circle_points = regular_polygon_points(
        center_x,
        center_y,
        radius,
        POINT_HIT_CIRCLE_SEGMENT_COUNT,
        0.0,
    );
    closed_polyline_line_projections("point-hit", &circle_points)
}

fn drill_steam_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    color_rgba: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let fin = shield_break_progress(remaining_ticks, lifetime_ticks);
    let fslope = midlife_slope(fin);
    let length = DRILL_STEAM_MIN_LENGTH + fin.powi(2) * DRILL_STEAM_LENGTH_GROWTH;
    let mut lines = Vec::new();
    for index in 0..DRILL_STEAM_PARTICLE_COUNT {
        let Some((particle_x, particle_y, particle_radius)) = drill_steam_particle_geometry(
            center_x_bits,
            center_y_bits,
            color_rgba,
            length,
            fslope,
            index,
        ) else {
            continue;
        };
        let ring_points = regular_polygon_points(
            particle_x,
            particle_y,
            particle_radius,
            DRILL_STEAM_RING_SEGMENT_COUNT,
            0.0,
        );
        lines.extend(closed_polyline_line_projections(
            "drill-steam",
            &ring_points,
        ));
    }
    lines
}

fn drill_steam_particle_geometry(
    center_x_bits: u32,
    center_y_bits: u32,
    color_rgba: u32,
    length: f32,
    fslope: f32,
    index: usize,
) -> Option<(f32, f32, f32)> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() || !length.is_finite() || !fslope.is_finite()
    {
        return None;
    }

    let angle = drill_steam_particle_seed(center_x_bits, center_y_bits, color_rgba, index, 0)
        * std::f32::consts::TAU;
    let distance = length
        * (0.25
            + drill_steam_particle_seed(center_x_bits, center_y_bits, color_rgba, index, 1) * 0.75);
    let radius = DRILL_STEAM_MIN_RADIUS
        + drill_steam_particle_seed(center_x_bits, center_y_bits, color_rgba, index, 2)
            * DRILL_STEAM_RADIUS_GROWTH
        + fslope * DRILL_STEAM_FSLOPE_GROWTH;
    let (particle_x_bits, particle_y_bits) = polar_point(center_x, center_y, distance, angle);
    Some((
        f32::from_bits(particle_x_bits),
        f32::from_bits(particle_y_bits),
        radius,
    ))
}

fn green_laser_charge_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    rotation_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let fin = shield_break_progress(remaining_ticks, lifetime_ticks);
    let fout = (1.0 - fin).max(0.0);
    let radius = GREEN_LASER_CHARGE_RADIUS_BASE + fout * GREEN_LASER_CHARGE_RADIUS_GROWTH;
    let circle_points = regular_polygon_points(
        center_x,
        center_y,
        radius,
        GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT,
        0.0,
    );
    let mut lines = closed_polyline_line_projections("green-laser-charge", &circle_points);

    let spoke_radius = fout * GREEN_LASER_CHARGE_SPOKE_RADIUS;
    if spoke_radius > 1.0 {
        let facing_radians = rotation_radians(rotation_bits);
        lines.extend((0..GREEN_LASER_CHARGE_SPOKE_COUNT).map(|index| {
            let angle = facing_radians
                + index as f32 * std::f32::consts::TAU / GREEN_LASER_CHARGE_SPOKE_COUNT as f32;
            line_projection(
                "green-laser-charge",
                (center_x_bits, center_y_bits),
                polar_point(center_x, center_y, spoke_radius, angle),
            )
        }));
    }

    lines
}

fn green_laser_charge_small_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let fin = shield_break_progress(remaining_ticks, lifetime_ticks);
    let fout = (1.0 - fin).max(0.0);
    let radius = fout * GREEN_LASER_CHARGE_SMALL_RADIUS_GROWTH;
    let circle_points = regular_polygon_points(
        center_x,
        center_y,
        radius,
        GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT,
        0.0,
    );
    closed_polyline_line_projections("green-laser-charge-small", &circle_points)
}

fn neoplasm_heal_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    rotation_bits: u32,
    color_rgba: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let fin = shield_break_progress(remaining_ticks, lifetime_ticks);
    let radius = NEOPLASM_HEAL_RADIUS_BASE + midlife_slope(fin) * NEOPLASM_HEAL_RADIUS_GROWTH;
    let rotation_radians = rotation_radians(rotation_bits);
    let offset_angle = rotation_radians
        + neoplasm_heal_seed_angle(center_x_bits, center_y_bits, rotation_bits, color_rgba);
    let offset_distance = fin * NEOPLASM_HEAL_OFFSET_MAX;
    let (offset_center_x_bits, offset_center_y_bits) =
        polar_point(center_x, center_y, offset_distance, offset_angle);
    let offset_center_x = f32::from_bits(offset_center_x_bits);
    let offset_center_y = f32::from_bits(offset_center_y_bits);
    let diamond_points = regular_polygon_points(
        offset_center_x,
        offset_center_y,
        radius,
        NEOPLASM_HEAL_DIAMOND_SIDE_COUNT,
        rotation_radians + std::f32::consts::FRAC_PI_4,
    );
    closed_polyline_line_projections("neoplasm-heal", &diamond_points)
}

fn arc_shield_break_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    rotation_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let progress = shield_break_progress(remaining_ticks, lifetime_ticks);
    let facing_degrees = f32::from_bits(rotation_bits);
    let facing_radians = if facing_degrees.is_finite() {
        facing_degrees.to_radians()
    } else {
        0.0
    };
    let outer_radius = ARC_SHIELD_BREAK_BASE_RADIUS + progress * ARC_SHIELD_BREAK_RADIUS_GROWTH;
    let inner_radius = (outer_radius - ARC_SHIELD_BREAK_BAND_WIDTH).max(1.0);
    let sweep_radians = ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians();
    let start_angle = facing_radians - sweep_radians / 2.0;
    let outer_points = arc_points(
        center_x,
        center_y,
        outer_radius,
        start_angle,
        sweep_radians,
        ARC_SHIELD_BREAK_SEGMENT_COUNT,
    );
    let inner_points = arc_points(
        center_x,
        center_y,
        inner_radius,
        start_angle,
        sweep_radians,
        ARC_SHIELD_BREAK_SEGMENT_COUNT,
    );
    if outer_points.len() < 2 || inner_points.len() < 2 {
        return Vec::new();
    }

    let mut lines = polyline_line_projections("arc-shield-break", &outer_points);
    lines.extend(polyline_line_projections("arc-shield-break", &inner_points));
    lines.push(line_projection(
        "arc-shield-break",
        outer_points[0],
        inner_points[0],
    ));
    lines.push(line_projection(
        "arc-shield-break",
        *outer_points.last().expect("outer arc missing endpoint"),
        *inner_points.last().expect("inner arc missing endpoint"),
    ));
    lines
}

fn unit_shield_break_line_projections(
    center_x_bits: u32,
    center_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let center_x = f32::from_bits(center_x_bits);
    let center_y = f32::from_bits(center_y_bits);
    if !center_x.is_finite() || !center_y.is_finite() {
        return Vec::new();
    }

    let progress = shield_break_progress(remaining_ticks, lifetime_ticks);
    let radius = UNIT_SHIELD_BREAK_BASE_RADIUS + progress * UNIT_SHIELD_BREAK_RADIUS_GROWTH;
    let burst_inner_radius = (radius - UNIT_SHIELD_BREAK_BURST_INSET).max(1.0);
    let burst_outer_radius =
        radius + UNIT_SHIELD_BREAK_BURST_LENGTH + progress * UNIT_SHIELD_BREAK_BURST_GROWTH;
    let circle_points = regular_polygon_points(
        center_x,
        center_y,
        radius,
        UNIT_SHIELD_BREAK_CIRCLE_SEGMENT_COUNT,
        0.0,
    );
    if circle_points.len() < 3 {
        return Vec::new();
    }

    let mut lines = closed_polyline_line_projections("unit-shield-break", &circle_points);
    lines.extend((0..UNIT_SHIELD_BREAK_BURST_COUNT).map(|index| {
        let angle = index as f32 * std::f32::consts::TAU / UNIT_SHIELD_BREAK_BURST_COUNT as f32;
        line_projection(
            "unit-shield-break",
            polar_point(center_x, center_y, burst_inner_radius, angle),
            polar_point(center_x, center_y, burst_outer_radius, angle),
        )
    }));
    lines
}

fn shield_break_progress(remaining_ticks: u8, lifetime_ticks: u8) -> f32 {
    let total_steps = lifetime_ticks.saturating_sub(1);
    if total_steps == 0 {
        return 1.0;
    }
    let elapsed = lifetime_ticks
        .saturating_sub(remaining_ticks)
        .min(total_steps);
    elapsed as f32 / total_steps as f32
}

fn inclusive_overlay_progress(remaining_ticks: u8, lifetime_ticks: u8) -> f32 {
    let total_steps = lifetime_ticks.max(1);
    let elapsed = lifetime_ticks
        .saturating_sub(remaining_ticks)
        .saturating_add(1)
        .min(total_steps);
    elapsed as f32 / total_steps as f32
}

fn point_hit_progress(remaining_ticks: u8, lifetime_ticks: u8) -> f32 {
    inclusive_overlay_progress(remaining_ticks, lifetime_ticks)
}

fn midlife_slope(progress: f32) -> f32 {
    (1.0 - (progress * 2.0 - 1.0).abs()).max(0.0)
}

fn lerp_point(source_x: f32, source_y: f32, target_x: f32, target_y: f32, t: f32) -> (f32, f32) {
    (
        source_x + (target_x - source_x) * t,
        source_y + (target_y - source_y) * t,
    )
}

fn quadratic_bezier_point(
    source_x: f32,
    source_y: f32,
    control_x: f32,
    control_y: f32,
    target_x: f32,
    target_y: f32,
    t: f32,
) -> (f32, f32) {
    let clamped_t = t.clamp(0.0, 1.0);
    let inv_t = 1.0 - clamped_t;
    (
        inv_t * inv_t * source_x
            + 2.0 * inv_t * clamped_t * control_x
            + clamped_t * clamped_t * target_x,
        inv_t * inv_t * source_y
            + 2.0 * inv_t * clamped_t * control_y
            + clamped_t * clamped_t * target_y,
    )
}

fn unit_parent_rotation_bits(
    overlay: &RuntimeEffectOverlay,
    session_state: &SessionState,
) -> Option<u32> {
    let RuntimeEffectBinding::ParentUnit { unit_id, .. } = overlay.binding.as_ref()? else {
        return None;
    };
    let projection = &session_state
        .entity_semantic_projection
        .by_entity_id
        .get(unit_id)?
        .projection;
    match projection {
        EntitySemanticProjection::Unit(unit) => Some(unit.rotation_bits),
        _ => None,
    }
}

fn executor_for_contract(
    contract: RuntimeEffectContract,
) -> &'static RuntimeEffectContractExecutor {
    match contract {
        RuntimeEffectContract::PositionTarget => &POSITION_TARGET_EXECUTOR,
        RuntimeEffectContract::LightningPath => &LIGHTNING_PATH_EXECUTOR,
        RuntimeEffectContract::PointBeam => &POINT_BEAM_EXECUTOR,
        RuntimeEffectContract::PointHit => &POINT_HIT_EXECUTOR,
        RuntimeEffectContract::DrillSteam => &DRILL_STEAM_EXECUTOR,
        RuntimeEffectContract::LegDestroy => &LEG_DESTROY_EXECUTOR,
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
        &POINT_HIT_EXECUTOR,
        &MOVE_COMMAND_EXECUTOR,
        &DRILL_STEAM_EXECUTOR,
        &LEG_DESTROY_EXECUTOR,
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

fn leg_destroy_overlay_origin(
    _effect_x: f32,
    _effect_y: f32,
    _effect_rotation: f32,
    object: &TypeIoObject,
) -> Option<(f32, f32)> {
    nth_contract_world_position(object, explicit_position_candidate, 1)
        .or_else(|| nth_contract_world_position(object, explicit_position_candidate, 0))
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

    let progress = payload_deposit_progress(overlay.remaining_ticks, overlay.lifetime_ticks);
    (
        (source_x + (target_x - source_x) * progress).to_bits(),
        (source_y + (target_y - source_y) * progress).to_bits(),
    )
}

fn payload_deposit_progress(remaining_ticks: u8, lifetime_ticks: u8) -> f32 {
    let total_steps = lifetime_ticks.saturating_sub(1);
    if total_steps == 0 {
        return 1.0;
    }
    let elapsed = lifetime_ticks
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

fn nth_contract_match_visit<'a, P>(
    value: &'a TypeIoObject,
    predicate: &P,
    depth: usize,
    max_depth: usize,
    remaining_nodes: &mut usize,
    target_index: usize,
    seen_count: &mut usize,
) -> Option<&'a TypeIoObject>
where
    P: Fn(&TypeIoObject) -> bool,
{
    if depth > max_depth || *remaining_nodes == 0 {
        return None;
    }

    *remaining_nodes -= 1;
    if predicate(value) {
        if *seen_count == target_index {
            return Some(value);
        }
        *seen_count += 1;
    }

    if depth == max_depth {
        return None;
    }

    let TypeIoObject::ObjectArray(values) = value else {
        return None;
    };

    for nested in values {
        if let Some(found) = nth_contract_match_visit(
            nested,
            predicate,
            depth + 1,
            max_depth,
            remaining_nodes,
            target_index,
            seen_count,
        ) {
            return Some(found);
        }
        if *remaining_nodes == 0 {
            break;
        }
    }

    None
}

fn nth_contract_match<'a, P>(
    object: &'a TypeIoObject,
    predicate: P,
    target_index: usize,
) -> Option<&'a TypeIoObject>
where
    P: Fn(&TypeIoObject) -> bool,
{
    let mut remaining_nodes = EFFECT_CONTRACT_MAX_NODES;
    let mut seen_count = 0usize;
    nth_contract_match_visit(
        object,
        &predicate,
        0,
        EFFECT_CONTRACT_MAX_DEPTH,
        &mut remaining_nodes,
        target_index,
        &mut seen_count,
    )
}

fn nth_contract_world_position<P>(
    object: &TypeIoObject,
    predicate: P,
    target_index: usize,
) -> Option<(u32, u32)>
where
    P: Fn(&TypeIoObject) -> bool,
{
    nth_contract_match(object, predicate, target_index).and_then(position_target_world_position)
}

fn explicit_position_candidate(value: &TypeIoObject) -> bool {
    match value {
        TypeIoObject::Point2 { .. } | TypeIoObject::Vec2 { .. } => true,
        TypeIoObject::PackedPoint2Array(values) => !values.is_empty(),
        TypeIoObject::Vec2Array(values) => !values.is_empty(),
        _ => false,
    }
}

fn position_target_candidate(value: &TypeIoObject) -> bool {
    explicit_position_candidate(value)
        || matches!(
            value.semantic_ref(),
            Some(TypeIoSemanticRef::Building { .. } | TypeIoSemanticRef::Unit { .. })
        )
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

fn line_projection(
    kind: &'static str,
    (source_x_bits, source_y_bits): (u32, u32),
    (target_x_bits, target_y_bits): (u32, u32),
) -> RuntimeEffectLineProjection {
    RuntimeEffectLineProjection {
        kind,
        source_x_bits,
        source_y_bits,
        target_x_bits,
        target_y_bits,
    }
}

fn move_command_line_projections(
    target_x_bits: u32,
    target_y_bits: u32,
    remaining_ticks: u8,
    lifetime_ticks: u8,
) -> Vec<RuntimeEffectLineProjection> {
    let target_x = f32::from_bits(target_x_bits);
    let target_y = f32::from_bits(target_y_bits);
    if !target_x.is_finite() || !target_y.is_finite() {
        return Vec::new();
    }

    let radius = 6.0 + inclusive_overlay_progress(remaining_ticks, lifetime_ticks) * 2.0;
    let points = regular_polygon_points(target_x, target_y, radius, 12, 0.0);
    closed_polyline_line_projections("move-command", &points)
}

fn polyline_line_projections(
    kind: &'static str,
    points: &[(u32, u32)],
) -> Vec<RuntimeEffectLineProjection> {
    points
        .windows(2)
        .filter_map(|pair| {
            let [source, target] = pair else {
                return None;
            };
            Some(line_projection(kind, *source, *target))
        })
        .collect()
}

fn closed_polyline_line_projections(
    kind: &'static str,
    points: &[(u32, u32)],
) -> Vec<RuntimeEffectLineProjection> {
    if points.len() < 2 {
        return Vec::new();
    }

    points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
        .map(|(source, target)| line_projection(kind, source, target))
        .collect()
}

fn regular_polygon_points(
    center_x: f32,
    center_y: f32,
    radius: f32,
    side_count: usize,
    phase_radians: f32,
) -> Vec<(u32, u32)> {
    if !center_x.is_finite()
        || !center_y.is_finite()
        || !radius.is_finite()
        || radius <= f32::EPSILON
        || side_count < 3
    {
        return Vec::new();
    }

    (0..side_count)
        .map(|index| {
            let angle = phase_radians + index as f32 * std::f32::consts::TAU / side_count as f32;
            polar_point(center_x, center_y, radius, angle)
        })
        .collect()
}

fn arc_points(
    center_x: f32,
    center_y: f32,
    radius: f32,
    start_radians: f32,
    sweep_radians: f32,
    segment_count: usize,
) -> Vec<(u32, u32)> {
    if !center_x.is_finite()
        || !center_y.is_finite()
        || !radius.is_finite()
        || !start_radians.is_finite()
        || !sweep_radians.is_finite()
        || radius <= f32::EPSILON
        || segment_count == 0
    {
        return Vec::new();
    }

    (0..=segment_count)
        .map(|index| {
            let t = index as f32 / segment_count as f32;
            polar_point(
                center_x,
                center_y,
                radius,
                start_radians + sweep_radians * t,
            )
        })
        .collect()
}

fn polar_point(center_x: f32, center_y: f32, radius: f32, angle_radians: f32) -> (u32, u32) {
    let cos = snap_trig_component(angle_radians.cos());
    let sin = snap_trig_component(angle_radians.sin());
    (
        (center_x + cos * radius).to_bits(),
        (center_y + sin * radius).to_bits(),
    )
}

fn rotation_radians(rotation_bits: u32) -> f32 {
    let rotation_degrees = f32::from_bits(rotation_bits);
    if rotation_degrees.is_finite() {
        rotation_degrees.to_radians()
    } else {
        0.0
    }
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
    fn rotation_radians_returns_zero_for_non_finite_bits_and_converts_finite_degrees() {
        assert_eq!(rotation_radians(f32::NAN.to_bits()), 0.0);
        assert!((rotation_radians(180.0f32.to_bits()) - std::f32::consts::PI).abs() < 1e-6);
    }

    fn test_line_projections_for_overlay(
        overlay: &RuntimeEffectOverlay,
        target_x_bits: u32,
        target_y_bits: u32,
        session_state: &SessionState,
    ) -> Vec<RuntimeEffectLineProjection> {
        line_projections_for_effect_overlay(
            overlay,
            overlay.source_x_bits,
            overlay.source_y_bits,
            target_x_bits,
            target_y_bits,
            session_state,
        )
    }

    fn test_marker_position_for_overlay(
        overlay: &RuntimeEffectOverlay,
        target_x_bits: u32,
        target_y_bits: u32,
    ) -> Option<(u32, u32)> {
        marker_position_for_effect_overlay(
            overlay,
            overlay.source_x_bits,
            overlay.source_y_bits,
            target_x_bits,
            target_y_bits,
        )
    }

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
    fn world_position_from_contract_business_projection_uses_leg_destroy_named_executor() {
        let projection = EffectBusinessProjection::PositionTarget {
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            target_x_bits: 72.0f32.to_bits(),
            target_y_bits: 96.0f32.to_bits(),
        };

        assert_eq!(
            world_position_from_contract_business_projection(
                Some(LEG_DESTROY_EXECUTOR.contract_name),
                Some(&projection),
            ),
            Some((72.0f32.to_bits(), 96.0f32.to_bits()))
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
    fn leg_destroy_overlay_origin_projects_second_explicit_position() {
        let object = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Vec2 { x: 40.0, y: 60.0 },
            TypeIoObject::ObjectArray(vec![TypeIoObject::Vec2 { x: 72.0, y: 96.0 }]),
            TypeIoObject::Null,
        ]);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::LegDestroy,
                12.0,
                20.0,
                0.0,
                Some(&object),
            ),
            Some((72.0, 96.0))
        );
    }

    #[test]
    fn leg_destroy_overlay_origin_falls_back_when_second_explicit_position_exceeds_search_bounds(
    ) {
        let mut values = Vec::new();
        values.push(TypeIoObject::Vec2 { x: 40.0, y: 60.0 });
        values.extend((0..62).map(TypeIoObject::Int));
        values.push(TypeIoObject::Vec2 { x: 72.0, y: 96.0 });
        let object = TypeIoObject::ObjectArray(values);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::LegDestroy,
                12.0,
                20.0,
                0.0,
                Some(&object),
            ),
            Some((40.0, 60.0))
        );
    }

    #[test]
    fn leg_destroy_overlay_origin_prefers_second_explicit_position_when_within_search_bounds() {
        let mut values = Vec::new();
        values.push(TypeIoObject::Vec2 { x: 40.0, y: 60.0 });
        values.extend((0..61).map(TypeIoObject::Int));
        values.push(TypeIoObject::Vec2 { x: 72.0, y: 96.0 });
        let object = TypeIoObject::ObjectArray(values);

        assert_eq!(
            overlay_origin_from_contract(
                RuntimeEffectContract::LegDestroy,
                12.0,
                20.0,
                0.0,
                Some(&object),
            ),
            Some((72.0, 96.0))
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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("point_beam"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        assert_eq!(
            test_line_projections_for_overlay(
                &overlay,
                80.0f32.to_bits(),
                160.0f32.to_bits(),
                &SessionState::default(),
            ),
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
    fn line_projections_for_effect_overlay_returns_leg_destroy_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(LEG_DESTROY_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 72.0f32.to_bits(),
            y_bits: 96.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 90,
            remaining_ticks: 90,
            contract_name: Some("leg_destroy"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        assert_eq!(
            test_line_projections_for_overlay(
                &overlay,
                72.0f32.to_bits(),
                96.0f32.to_bits(),
                &SessionState::default(),
            ),
            vec![RuntimeEffectLineProjection {
                kind: "leg-destroy",
                source_x_bits: 12.0f32.to_bits(),
                source_y_bits: 20.0f32.to_bits(),
                target_x_bits: 72.0f32.to_bits(),
                target_y_bits: 96.0f32.to_bits(),
            }]
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_float_length_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(FLOAT_LENGTH_EFFECT_ID),
            source_x_bits: 10.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 26.0f32.to_bits(),
            y_bits: 20.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("float_length"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        assert_eq!(
            test_line_projections_for_overlay(
                &overlay,
                26.0f32.to_bits(),
                20.0f32.to_bits(),
                &SessionState::default(),
            ),
            vec![RuntimeEffectLineProjection {
                kind: "float-length",
                source_x_bits: 10.0f32.to_bits(),
                source_y_bits: 20.0f32.to_bits(),
                target_x_bits: 26.0f32.to_bits(),
                target_y_bits: 20.0f32.to_bits(),
            }]
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_regen_suppress_seek_curve() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(REGEN_SUPPRESS_SEEK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 140,
            remaining_ticks: 140,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let (source_x, source_y, control_x, control_y, target_x, target_y) =
            regen_suppress_seek_curve_points(
                &overlay,
                overlay.source_x_bits,
                overlay.source_y_bits,
                overlay.x_bits,
                overlay.y_bits,
            )
            .expect("curve points should resolve");
        let first_curve_point = quadratic_bezier_point(
            source_x,
            source_y,
            control_x,
            control_y,
            target_x,
            target_y,
            1.0 / REGEN_SUPPRESS_SEEK_PATH_SEGMENT_COUNT as f32,
        );

        assert_eq!(lines.len(), REGEN_SUPPRESS_SEEK_PATH_SEGMENT_COUNT);
        assert!(lines.iter().all(|line| line.kind == "regen-suppress-seek"));
        assert_eq!(
            lines.first(),
            Some(&RuntimeEffectLineProjection {
                kind: "regen-suppress-seek",
                source_x_bits: 12.0f32.to_bits(),
                source_y_bits: 20.0f32.to_bits(),
                target_x_bits: first_curve_point.0.to_bits(),
                target_y_bits: first_curve_point.1.to_bits(),
            })
        );
        assert_eq!(
            lines
                .last()
                .map(|line| (line.target_x_bits, line.target_y_bits)),
            Some((80.0f32.to_bits(), 160.0f32.to_bits()))
        );
    }

    #[test]
    fn marker_position_for_effect_overlay_returns_regen_suppress_seek_curve_position() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(REGEN_SUPPRESS_SEEK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 140,
            remaining_ticks: 140,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let (source_x, source_y, control_x, control_y, target_x, target_y) =
            regen_suppress_seek_curve_points(
                &overlay,
                overlay.source_x_bits,
                overlay.source_y_bits,
                overlay.x_bits,
                overlay.y_bits,
            )
            .expect("curve points should resolve");
        let expected_curve_t =
            1.0 - inclusive_overlay_progress(overlay.remaining_ticks, overlay.lifetime_ticks);
        let expected_marker = quadratic_bezier_point(
            source_x,
            source_y,
            control_x,
            control_y,
            target_x,
            target_y,
            expected_curve_t,
        );

        assert_eq!(
            test_marker_position_for_overlay(&overlay, 80.0f32.to_bits(), 160.0f32.to_bits()),
            Some((expected_marker.0.to_bits(), expected_marker.1.to_bits()))
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_unit_spirit_double_diamond() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(UNIT_SPIRIT_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let progress = inclusive_overlay_progress(overlay.remaining_ticks, overlay.lifetime_ticks);
        let outer_points = regular_polygon_points(
            12.0 + (80.0 - 12.0) * progress.powi(2),
            20.0 + (160.0 - 20.0) * progress.powi(2),
            UNIT_SPIRIT_BASE_RADIUS * progress * UNIT_SPIRIT_OUTER_RADIUS_SCALE,
            UNIT_SPIRIT_SIDE_COUNT,
            std::f32::consts::FRAC_PI_4,
        );
        let inner_points = regular_polygon_points(
            12.0 + (80.0 - 12.0) * progress.powi(5),
            20.0 + (160.0 - 20.0) * progress.powi(5),
            UNIT_SPIRIT_BASE_RADIUS * progress,
            UNIT_SPIRIT_SIDE_COUNT,
            std::f32::consts::FRAC_PI_4,
        );

        assert_eq!(lines.len(), UNIT_SPIRIT_SIDE_COUNT * 2);
        assert!(lines.iter().all(|line| line.kind == "unit-spirit"));
        assert!(lines.contains(&line_projection(
            "unit-spirit",
            outer_points[0],
            outer_points[1],
        )));
        assert!(lines.contains(&line_projection(
            "unit-spirit",
            inner_points[0],
            inner_points[1],
        )));
    }

    #[test]
    fn line_projections_for_effect_overlay_uses_resolved_source_for_unit_spirit() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(UNIT_SPIRIT_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let first_lines = line_projections_for_effect_overlay(
            &overlay,
            12.0f32.to_bits(),
            20.0f32.to_bits(),
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let shifted_lines = line_projections_for_effect_overlay(
            &overlay,
            28.0f32.to_bits(),
            44.0f32.to_bits(),
            96.0f32.to_bits(),
            184.0f32.to_bits(),
            &SessionState::default(),
        );

        assert_eq!(first_lines.len(), shifted_lines.len());
        for (first, shifted) in first_lines.iter().zip(shifted_lines.iter()) {
            assert!(
                (f32::from_bits(shifted.source_x_bits)
                    - f32::from_bits(first.source_x_bits)
                    - 16.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.source_y_bits)
                    - f32::from_bits(first.source_y_bits)
                    - 24.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.target_x_bits)
                    - f32::from_bits(first.target_x_bits)
                    - 16.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.target_y_bits)
                    - f32::from_bits(first.target_y_bits)
                    - 24.0)
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_item_transfer_rings() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ITEM_TRANSFER_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let (center_x, center_y, outer_radius, inner_radius) = item_transfer_geometry(
            &overlay,
            overlay.source_x_bits,
            overlay.source_y_bits,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        )
        .expect("item transfer geometry");
        let outer_points = regular_polygon_points(
            center_x,
            center_y,
            outer_radius,
            ITEM_TRANSFER_CIRCLE_SEGMENT_COUNT,
            0.0,
        );
        let inner_points = regular_polygon_points(
            center_x,
            center_y,
            inner_radius,
            ITEM_TRANSFER_CIRCLE_SEGMENT_COUNT,
            0.0,
        );

        assert_eq!(lines.len(), ITEM_TRANSFER_CIRCLE_SEGMENT_COUNT * 2);
        assert!(lines.iter().all(|line| line.kind == "item-transfer"));
        assert!(lines.contains(&line_projection(
            "item-transfer",
            outer_points[0],
            outer_points[1],
        )));
        assert!(lines.contains(&line_projection(
            "item-transfer",
            inner_points[0],
            inner_points[1],
        )));
    }

    #[test]
    fn marker_position_for_effect_overlay_returns_item_transfer_curve_position() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ITEM_TRANSFER_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let marker =
            test_marker_position_for_overlay(&overlay, 80.0f32.to_bits(), 160.0f32.to_bits())
                .expect("item transfer marker override");

        assert_ne!(marker, (80.0f32.to_bits(), 160.0f32.to_bits()));
        assert_ne!(marker, (12.0f32.to_bits(), 20.0f32.to_bits()));
    }

    #[test]
    fn marker_position_for_effect_overlay_uses_resolved_source_for_item_transfer() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ITEM_TRANSFER_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let first_marker = marker_position_for_effect_overlay(
            &overlay,
            12.0f32.to_bits(),
            20.0f32.to_bits(),
            80.0f32.to_bits(),
            160.0f32.to_bits(),
        )
        .expect("first marker");
        let shifted_marker = marker_position_for_effect_overlay(
            &overlay,
            28.0f32.to_bits(),
            44.0f32.to_bits(),
            96.0f32.to_bits(),
            184.0f32.to_bits(),
        )
        .expect("shifted marker");

        assert!(
            (f32::from_bits(shifted_marker.0) - f32::from_bits(first_marker.0) - 16.0).abs() < 0.01
        );
        assert!(
            (f32::from_bits(shifted_marker.1) - f32::from_bits(first_marker.1) - 24.0).abs() < 0.01
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_uses_stable_overlay_seed_for_regen_suppress_seek() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(REGEN_SUPPRESS_SEEK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 15.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 140,
            remaining_ticks: 140,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let first_lines = line_projections_for_effect_overlay(
            &overlay,
            12.0f32.to_bits(),
            20.0f32.to_bits(),
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let shifted_lines = line_projections_for_effect_overlay(
            &overlay,
            28.0f32.to_bits(),
            44.0f32.to_bits(),
            96.0f32.to_bits(),
            184.0f32.to_bits(),
            &SessionState::default(),
        );

        assert_eq!(first_lines.len(), shifted_lines.len());
        for (first, shifted) in first_lines.iter().zip(shifted_lines.iter()) {
            assert!(
                (f32::from_bits(shifted.source_x_bits)
                    - f32::from_bits(first.source_x_bits)
                    - 16.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.source_y_bits)
                    - f32::from_bits(first.source_y_bits)
                    - 24.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.target_x_bits)
                    - f32::from_bits(first.target_x_bits)
                    - 16.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.target_y_bits)
                    - f32::from_bits(first.target_y_bits)
                    - 24.0)
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn line_projections_for_effect_overlay_uses_stable_overlay_seed_for_item_transfer() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ITEM_TRANSFER_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 15.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 20,
            remaining_ticks: 10,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let first_lines = line_projections_for_effect_overlay(
            &overlay,
            12.0f32.to_bits(),
            20.0f32.to_bits(),
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let shifted_lines = line_projections_for_effect_overlay(
            &overlay,
            28.0f32.to_bits(),
            44.0f32.to_bits(),
            96.0f32.to_bits(),
            184.0f32.to_bits(),
            &SessionState::default(),
        );

        assert_eq!(first_lines.len(), shifted_lines.len());
        for (first, shifted) in first_lines.iter().zip(shifted_lines.iter()) {
            assert!(
                (f32::from_bits(shifted.source_x_bits)
                    - f32::from_bits(first.source_x_bits)
                    - 16.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.source_y_bits)
                    - f32::from_bits(first.source_y_bits)
                    - 24.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.target_x_bits)
                    - f32::from_bits(first.target_x_bits)
                    - 16.0)
                    .abs()
                    < 0.01
            );
            assert!(
                (f32::from_bits(shifted.target_y_bits)
                    - f32::from_bits(first.target_y_bits)
                    - 24.0)
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn item_transfer_seed_ignores_overlay_absolute_translation() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ITEM_TRANSFER_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 15.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 20,
            remaining_ticks: 10,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };
        let shifted_overlay = RuntimeEffectOverlay {
            source_x_bits: 28.0f32.to_bits(),
            source_y_bits: 44.0f32.to_bits(),
            x_bits: 96.0f32.to_bits(),
            y_bits: 184.0f32.to_bits(),
            ..overlay.clone()
        };

        assert_eq!(
            effect_overlay_instance_seed(&overlay),
            effect_overlay_instance_seed(&shifted_overlay)
        );
        assert_eq!(
            effect_overlay_signed_seed(&overlay, 0.0),
            effect_overlay_signed_seed(&shifted_overlay, 0.0)
        );

        let first_geometry = item_transfer_geometry(
            &overlay,
            overlay.source_x_bits,
            overlay.source_y_bits,
            overlay.x_bits,
            overlay.y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        )
        .expect("first geometry");
        let shifted_geometry = item_transfer_geometry(
            &shifted_overlay,
            shifted_overlay.source_x_bits,
            shifted_overlay.source_y_bits,
            shifted_overlay.x_bits,
            shifted_overlay.y_bits,
            shifted_overlay.remaining_ticks,
            shifted_overlay.lifetime_ticks,
        )
        .expect("shifted geometry");

        assert!(((shifted_geometry.0 - first_geometry.0) - 16.0).abs() < 0.01);
        assert!(((shifted_geometry.1 - first_geometry.1) - 24.0).abs() < 0.01);
        assert!((shifted_geometry.2 - first_geometry.2).abs() < 0.01);
        assert!((shifted_geometry.3 - first_geometry.3).abs() < 0.01);
    }

    #[test]
    fn item_transfer_geometry_uses_raw_stable_overlay_seed_without_flooring() {
        let base_overlay = RuntimeEffectOverlay {
            effect_id: Some(ITEM_TRANSFER_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 15.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 20,
            remaining_ticks: 10,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let mut overlay = None;
        let mut raw_seed = 0.0f32;
        for color_rgba in 0..4096u32 {
            let candidate = RuntimeEffectOverlay {
                color_rgba,
                ..base_overlay.clone()
            };
            let seed = effect_overlay_signed_seed(&candidate, 0.0);
            if seed.abs() < 0.25 {
                overlay = Some(candidate);
                raw_seed = seed;
                break;
            }
        }

        let overlay = overlay.expect("expected to find a low-magnitude stable seed");
        let (center_x, center_y, outer_radius, inner_radius) = item_transfer_geometry(
            &overlay,
            overlay.source_x_bits,
            overlay.source_y_bits,
            overlay.x_bits,
            overlay.y_bits,
            overlay.remaining_ticks,
            overlay.lifetime_ticks,
        )
        .expect("item transfer geometry");

        let progress = inclusive_overlay_progress(overlay.remaining_ticks, overlay.lifetime_ticks);
        let slope = midlife_slope(progress);
        let path_t = progress.powi(3);
        let (base_x, base_y) = lerp_point(
            f32::from_bits(overlay.source_x_bits),
            f32::from_bits(overlay.source_y_bits),
            f32::from_bits(overlay.x_bits),
            f32::from_bits(overlay.y_bits),
            path_t,
        );
        let dx = f32::from_bits(overlay.x_bits) - f32::from_bits(overlay.source_x_bits);
        let dy = f32::from_bits(overlay.y_bits) - f32::from_bits(overlay.source_y_bits);
        let distance = (dx * dx + dy * dy).sqrt();
        let normal_x = -dy / distance;
        let normal_y = dx / distance;
        let expected_lateral = raw_seed * slope * ITEM_TRANSFER_LATERAL_OFFSET_MAX;
        let expected_center_x = base_x + normal_x * expected_lateral;
        let expected_center_y = base_y + normal_y * expected_lateral;

        assert!(
            raw_seed.abs() < 0.25,
            "expected a seed below the legacy floor, got {raw_seed}"
        );
        assert!((center_x - expected_center_x).abs() < 0.01);
        assert!((center_y - expected_center_y).abs() < 0.01);
        assert!(outer_radius > inner_radius);
    }

    #[test]
    fn stable_overlay_seed_ignores_overlay_progress_ticks() {
        let initial_overlay = RuntimeEffectOverlay {
            effect_id: Some(REGEN_SUPPRESS_SEEK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            source_binding: None,
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 15.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 140,
            remaining_ticks: 140,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };
        let progressed_overlay = RuntimeEffectOverlay {
            remaining_ticks: 73,
            ..initial_overlay.clone()
        };

        assert_eq!(
            effect_overlay_instance_seed(&initial_overlay),
            effect_overlay_instance_seed(&progressed_overlay)
        );
        assert_eq!(
            effect_overlay_signed_seed(&initial_overlay, 0.0),
            effect_overlay_signed_seed(&progressed_overlay, 0.0)
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_point_hit_circle() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(POINT_HIT_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: false,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("point_hit"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );

        assert_eq!(lines.len(), POINT_HIT_CIRCLE_SEGMENT_COUNT);
        assert!(lines.iter().all(|line| line.kind == "point-hit"));
        assert!(lines.iter().any(|line| {
            line.source_x_bits == 34.0f32.to_bits() && line.source_y_bits == 48.0f32.to_bits()
        }));
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_drill_steam_particle_rings() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(DRILL_STEAM_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: false,
            lifetime_ticks: 220,
            remaining_ticks: 220,
            contract_name: None,
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );
        let (particle_x, particle_y, particle_radius) = drill_steam_particle_geometry(
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            overlay.color_rgba,
            DRILL_STEAM_MIN_LENGTH,
            0.0,
            0,
        )
        .expect("drill steam particle geometry");
        let particle_points = regular_polygon_points(
            particle_x,
            particle_y,
            particle_radius,
            DRILL_STEAM_RING_SEGMENT_COUNT,
            0.0,
        );

        assert_eq!(
            lines.len(),
            DRILL_STEAM_PARTICLE_COUNT * DRILL_STEAM_RING_SEGMENT_COUNT
        );
        assert!(lines.iter().all(|line| line.kind == "drill-steam"));
        assert!(lines.contains(&line_projection(
            "drill-steam",
            particle_points[0],
            particle_points[1],
        )));
    }

    #[test]
    fn executor_for_name_resolves_drill_steam_contract() {
        let executor = executor_for_name("drill_steam").expect("drill steam executor");

        assert_eq!(executor.contract_name, "drill_steam");
    }

    #[test]
    fn executor_for_name_resolves_move_command_contract() {
        let executor = executor_for_name("move_command").expect("move command executor");

        assert_eq!(executor.contract_name, "move_command");
        assert_eq!(
            (executor.overlay_origin)(12.0, 20.0, 0.0, &TypeIoObject::ObjectArray(vec![
                TypeIoObject::Point2 { x: 10, y: 20 }
            ])),
            Some((80.0, 160.0))
        );
    }

    #[test]
    fn executor_for_name_resolves_all_named_contracts() {
        let expected = [
            "position_target",
            "lightning",
            "point_beam",
            "point_hit",
            "move_command",
            "drill_steam",
            "leg_destroy",
            "shield_break",
            "block_content_icon",
            "content_icon",
            "payload_target_content",
            "drop_item",
            "float_length",
            "unit_parent",
        ];

        assert_eq!(
            expected
                .iter()
                .map(|name| {
                    executor_for_name(name)
                        .expect("contract executor should resolve")
                        .contract_name
                })
                .collect::<Vec<_>>(),
            expected
        );
        assert!(executor_for_name("missing_contract").is_none());
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_green_laser_charge_circle_and_spokes() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(GREEN_LASER_CHARGE_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );
        let circle_points = regular_polygon_points(
            32.0,
            48.0,
            GREEN_LASER_CHARGE_RADIUS_BASE + GREEN_LASER_CHARGE_RADIUS_GROWTH,
            GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT,
            0.0,
        );

        assert_eq!(
            lines.len(),
            GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT + GREEN_LASER_CHARGE_SPOKE_COUNT
        );
        assert!(lines.iter().all(|line| line.kind == "green-laser-charge"));
        assert!(lines.contains(&line_projection(
            "green-laser-charge",
            circle_points[0],
            circle_points[1],
        )));
        assert!(lines.contains(&line_projection(
            "green-laser-charge",
            (32.0f32.to_bits(), 48.0f32.to_bits()),
            polar_point(32.0, 48.0, GREEN_LASER_CHARGE_SPOKE_RADIUS, 0.0),
        )));
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_green_laser_charge_small_circle() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(GREEN_LASER_CHARGE_SMALL_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );
        let circle_points = regular_polygon_points(
            32.0,
            48.0,
            GREEN_LASER_CHARGE_SMALL_RADIUS_GROWTH,
            GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT,
            0.0,
        );

        assert_eq!(lines.len(), GREEN_LASER_CHARGE_CIRCLE_SEGMENT_COUNT);
        assert!(lines
            .iter()
            .all(|line| line.kind == "green-laser-charge-small"));
        assert!(lines.contains(&line_projection(
            "green-laser-charge-small",
            circle_points[0],
            circle_points[1],
        )));
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_neoplasm_heal_diamond() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(NEOPLASM_HEAL_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x55667788,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 2,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );
        let fin = shield_break_progress(overlay.remaining_ticks, overlay.lifetime_ticks);
        let radius = NEOPLASM_HEAL_RADIUS_BASE + midlife_slope(fin) * NEOPLASM_HEAL_RADIUS_GROWTH;
        let offset_angle = neoplasm_heal_seed_angle(
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            overlay.rotation_bits,
            overlay.color_rgba,
        );
        let (offset_center_x_bits, offset_center_y_bits) =
            polar_point(32.0, 48.0, fin * NEOPLASM_HEAL_OFFSET_MAX, offset_angle);
        let diamond_points = regular_polygon_points(
            f32::from_bits(offset_center_x_bits),
            f32::from_bits(offset_center_y_bits),
            radius,
            NEOPLASM_HEAL_DIAMOND_SIDE_COUNT,
            std::f32::consts::FRAC_PI_4,
        );

        assert_eq!(lines.len(), NEOPLASM_HEAL_DIAMOND_SIDE_COUNT);
        assert!(lines.iter().all(|line| line.kind == "neoplasm-heal"));
        assert!(lines.contains(&line_projection(
            "neoplasm-heal",
            diamond_points[0],
            diamond_points[1],
        )));
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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("shield_break"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );

        assert_eq!(lines.len(), SHIELD_BREAK_SIDE_COUNT);
        assert!(lines.iter().all(|line| line.kind == "shield-break"));
        assert!(lines.iter().any(|line| {
            line.source_x_bits == 38.0f32.to_bits() && line.source_y_bits == 48.0f32.to_bits()
        }));
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_arc_shield_break_bands_and_endcaps() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ARC_SHIELD_BREAK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );
        let outer_points = arc_points(
            32.0,
            48.0,
            ARC_SHIELD_BREAK_BASE_RADIUS,
            -ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians() / 2.0,
            ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians(),
            ARC_SHIELD_BREAK_SEGMENT_COUNT,
        );
        let inner_points = arc_points(
            32.0,
            48.0,
            ARC_SHIELD_BREAK_BASE_RADIUS - ARC_SHIELD_BREAK_BAND_WIDTH,
            -ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians() / 2.0,
            ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians(),
            ARC_SHIELD_BREAK_SEGMENT_COUNT,
        );

        assert_eq!(lines.len(), ARC_SHIELD_BREAK_SEGMENT_COUNT * 2 + 2);
        assert!(lines.iter().all(|line| line.kind == "arc-shield-break"));
        assert!(lines.contains(&line_projection(
            "arc-shield-break",
            outer_points[0],
            inner_points[0],
        )));
        assert!(lines.contains(&line_projection(
            "arc-shield-break",
            *outer_points.last().expect("missing outer endpoint"),
            *inner_points.last().expect("missing inner endpoint"),
        )));
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_unit_shield_break_circle_and_burst_spokes() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(UNIT_SHIELD_BREAK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &SessionState::default(),
        );
        let circle_points = regular_polygon_points(
            32.0,
            48.0,
            UNIT_SHIELD_BREAK_BASE_RADIUS,
            UNIT_SHIELD_BREAK_CIRCLE_SEGMENT_COUNT,
            0.0,
        );
        let first_burst = line_projection(
            "unit-shield-break",
            polar_point(
                32.0,
                48.0,
                UNIT_SHIELD_BREAK_BASE_RADIUS - UNIT_SHIELD_BREAK_BURST_INSET,
                0.0,
            ),
            polar_point(
                32.0,
                48.0,
                UNIT_SHIELD_BREAK_BASE_RADIUS + UNIT_SHIELD_BREAK_BURST_LENGTH,
                0.0,
            ),
        );

        assert_eq!(
            lines.len(),
            UNIT_SHIELD_BREAK_CIRCLE_SEGMENT_COUNT + UNIT_SHIELD_BREAK_BURST_COUNT
        );
        assert!(lines.iter().all(|line| line.kind == "unit-shield-break"));
        assert!(lines.contains(&line_projection(
            "unit-shield-break",
            circle_points[0],
            circle_points[1],
        )));
        assert!(lines.contains(&first_burst));
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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );

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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );

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
    fn chain_line_projections_clamps_segment_count_and_amplitude_at_distance_extremes() {
        let short_lines = chain_line_projections(
            "chain-lightning",
            0.0f32.to_bits(),
            0.0f32.to_bits(),
            1.0f32.to_bits(),
            0.0f32.to_bits(),
        );
        let longer_short_lines = chain_line_projections(
            "chain-lightning",
            0.0f32.to_bits(),
            0.0f32.to_bits(),
            8.0f32.to_bits(),
            0.0f32.to_bits(),
        );
        let long_lines = chain_line_projections(
            "chain-lightning",
            0.0f32.to_bits(),
            0.0f32.to_bits(),
            200.0f32.to_bits(),
            0.0f32.to_bits(),
        );
        let farther_long_lines = chain_line_projections(
            "chain-lightning",
            0.0f32.to_bits(),
            0.0f32.to_bits(),
            240.0f32.to_bits(),
            0.0f32.to_bits(),
        );

        assert_eq!(short_lines.len(), CHAIN_MIN_SEGMENTS);
        assert_eq!(longer_short_lines.len(), CHAIN_MIN_SEGMENTS);
        assert_eq!(long_lines.len(), CHAIN_MAX_SEGMENTS);
        assert_eq!(farther_long_lines.len(), CHAIN_MAX_SEGMENTS);
        assert_eq!(short_lines[0].target_y_bits, longer_short_lines[0].target_y_bits);
        assert_eq!(long_lines[0].target_y_bits, farther_long_lines[0].target_y_bits);
        assert_ne!(short_lines[0].target_y_bits, long_lines[0].target_y_bits);
    }

    #[test]
    fn line_projections_for_effect_overlay_ignores_other_effect_ids() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(99),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        assert_eq!(
            test_line_projections_for_overlay(
                &overlay,
                80.0f32.to_bits(),
                160.0f32.to_bits(),
                &SessionState::default(),
            ),
            Vec::<RuntimeEffectLineProjection>::new()
        );
    }

    #[test]
    fn line_projections_for_effect_overlay_returns_move_command_circle() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(MOVE_COMMAND_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 80.0f32.to_bits(),
            y_bits: 160.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 20,
            remaining_ticks: 20,
            contract_name: Some("move_command"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        };

        let lines = test_line_projections_for_overlay(
            &overlay,
            80.0f32.to_bits(),
            160.0f32.to_bits(),
            &SessionState::default(),
        );
        let radius =
            6.0 + inclusive_overlay_progress(overlay.remaining_ticks, overlay.lifetime_ticks) * 2.0;
        let circle_points = regular_polygon_points(80.0, 160.0, radius, 12, 0.0);

        assert_eq!(lines.len(), 12);
        assert!(lines.iter().all(|line| line.kind == "move-command"));
        assert_eq!(
            lines.first(),
            Some(&line_projection(
                "move-command",
                circle_points[0],
                circle_points[1],
            ))
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
    fn world_position_from_contract_business_projection_uses_move_command_named_executor() {
        let projection = EffectBusinessProjection::PositionTarget {
            source_x_bits: 10.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            target_x_bits: 80.0f32.to_bits(),
            target_y_bits: 160.0f32.to_bits(),
        };

        assert_eq!(
            world_position_from_contract_business_projection(
                Some("move_command"),
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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("block_content_icon"),
            source_binding: None,
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
            lifetime_ticks: 3,
            remaining_ticks: 2,
            contract_name: Some("payload_target_content"),
            source_binding: None,
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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("content_icon"),
            source_binding: None,
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
    fn content_projections_for_effect_overlay_returns_drop_item_projection() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(142),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 12.0f32.to_bits(),
            y_bits: 40.0f32.to_bits(),
            rotation_bits: 90.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("drop_item"),
            source_binding: None,
            binding: None,
            content_ref: Some((ITEM_CONTENT_TYPE, 12)),
            polyline_points: Vec::new(),
        };

        assert_eq!(
            content_projections_for_effect_overlay(&overlay, 12.0f32.to_bits(), 40.0f32.to_bits()),
            vec![RuntimeEffectContentProjection {
                kind: "drop-item",
                content_type: ITEM_CONTENT_TYPE,
                content_id: 12,
                x_bits: 12.0f32.to_bits(),
                y_bits: 40.0f32.to_bits(),
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
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("lightning"),
            source_binding: None,
            binding: None,
            content_ref: None,
            polyline_points: vec![
                (10.0f32.to_bits(), 20.0f32.to_bits()),
                (30.0f32.to_bits(), 40.0f32.to_bits()),
                (50.0f32.to_bits(), 60.0f32.to_bits()),
            ],
        };

        assert_eq!(
            test_line_projections_for_overlay(
                &overlay,
                50.0f32.to_bits(),
                60.0f32.to_bits(),
                &SessionState::default(),
            ),
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

    #[test]
    fn line_projections_for_effect_overlay_uses_parent_unit_rotation_for_green_laser_charge() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(GREEN_LASER_CHARGE_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 12.0f32.to_bits(),
                spawn_y_bits: 20.0f32.to_bits(),
                offset_x_bits: 0.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization: true,
                rotate_with_parent: true,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
            }),
            content_ref: None,
            polyline_points: Vec::new(),
        };
        let mut state = SessionState::default();
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            crate::session_state::EntitySemanticProjectionEntry {
                class_id: 4,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(
                    crate::session_state::EntityUnitSemanticProjection {
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
                    },
                ),
            },
        );

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &state,
        );

        assert!(lines.contains(&line_projection(
            "green-laser-charge",
            (32.0f32.to_bits(), 48.0f32.to_bits()),
            polar_point(
                32.0,
                48.0,
                GREEN_LASER_CHARGE_SPOKE_RADIUS,
                90.0f32.to_radians(),
            ),
        )));
    }

    #[test]
    fn line_projections_for_effect_overlay_uses_parent_unit_rotation_for_arc_shield_break() {
        let overlay = RuntimeEffectOverlay {
            effect_id: Some(ARC_SHIELD_BREAK_EFFECT_ID),
            source_x_bits: 12.0f32.to_bits(),
            source_y_bits: 20.0f32.to_bits(),
            x_bits: 32.0f32.to_bits(),
            y_bits: 48.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0x11223344,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            source_binding: None,
            binding: Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 12.0f32.to_bits(),
                spawn_y_bits: 20.0f32.to_bits(),
                offset_x_bits: 0.0f32.to_bits(),
                offset_y_bits: 0.0f32.to_bits(),
                offset_initialized: true,
                preserve_spawn_offset: true,
                allow_fallback_offset_initialization: false,
                rotate_with_parent: false,
                parent_rotation_reference_bits: 0.0f32.to_bits(),
                rotation_offset_bits: 0.0f32.to_bits(),
                rotation_initialized: false,
            }),
            content_ref: None,
            polyline_points: Vec::new(),
        };
        let mut state = SessionState::default();
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            crate::session_state::EntitySemanticProjectionEntry {
                class_id: 4,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(
                    crate::session_state::EntityUnitSemanticProjection {
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
                    },
                ),
            },
        );

        let lines = test_line_projections_for_overlay(
            &overlay,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            &state,
        );
        let outer_points = arc_points(
            32.0,
            48.0,
            ARC_SHIELD_BREAK_BASE_RADIUS,
            90.0f32.to_radians() - ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians() / 2.0,
            ARC_SHIELD_BREAK_SWEEP_DEGREES.to_radians(),
            ARC_SHIELD_BREAK_SEGMENT_COUNT,
        );

        assert!(lines.contains(&line_projection(
            "arc-shield-break",
            outer_points[0],
            outer_points[1],
        )));
    }
}
