use crate::custom_packet_runtime::RuntimeCustomPacketSemanticKind;
use crate::custom_packet_runtime_surface::RuntimeCustomPacketSurfaceSummaryEntry;
use crate::session_state::SessionState;
use mdt_input::{
    CommandModePositionTarget, CommandModeState, CommandModeTargetProjection, CommandUnitRef,
};
use mdt_typeio::unpack_point2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeCustomPacketBusinessMarkerSource {
    Surface,
    RuntimeEntity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCustomPacketBusinessMarker {
    pub source: RuntimeCustomPacketBusinessMarkerSource,
    pub x: f32,
    pub y: f32,
}

pub fn resolve_runtime_custom_packet_business_marker(
    entry: &RuntimeCustomPacketSurfaceSummaryEntry,
    session_state: &SessionState,
) -> Option<RuntimeCustomPacketBusinessMarker> {
    if let Some(marker) = entry.marker.as_ref() {
        return Some(RuntimeCustomPacketBusinessMarker {
            source: RuntimeCustomPacketBusinessMarkerSource::Surface,
            x: marker.x,
            y: marker.y,
        });
    }
    if entry.semantic != RuntimeCustomPacketSemanticKind::UnitId {
        return None;
    }
    let unit_id = entry.stable_value.parse::<i32>().ok()?;
    let projection = session_state.runtime_typed_entity_projection();
    let entity = projection.entity_at(unit_id)?;
    Some(RuntimeCustomPacketBusinessMarker {
        source: RuntimeCustomPacketBusinessMarkerSource::RuntimeEntity,
        x: f32::from_bits(entity.base().x_bits),
        y: f32::from_bits(entity.base().y_bits),
    })
}

pub fn resolve_runtime_custom_packet_command_target(
    entry: &RuntimeCustomPacketSurfaceSummaryEntry,
    session_state: &SessionState,
    marker: Option<&RuntimeCustomPacketBusinessMarker>,
) -> Option<CommandModeTargetProjection> {
    let resolved_marker = marker
        .cloned()
        .or_else(|| resolve_runtime_custom_packet_business_marker(entry, session_state));
    let marker = resolved_marker.as_ref();
    match entry.semantic {
        RuntimeCustomPacketSemanticKind::WorldPos => {
            let (x, y) = marker
                .map(|marker| (marker.x, marker.y))
                .or_else(|| parse_world_pos(&entry.stable_value))?;
            Some(CommandModeTargetProjection {
                position_target: Some(position_target(x, y)),
                ..CommandModeTargetProjection::default()
            })
        }
        RuntimeCustomPacketSemanticKind::BuildPos => {
            let build_pos = entry.stable_value.trim().parse::<i32>().ok()?;
            let (x, y) = marker
                .map(|marker| (marker.x, marker.y))
                .unwrap_or_else(|| build_pos_world_pos(build_pos));
            Some(CommandModeTargetProjection {
                build_target: Some(build_pos),
                position_target: Some(position_target(x, y)),
                ..CommandModeTargetProjection::default()
            })
        }
        RuntimeCustomPacketSemanticKind::UnitId => {
            let unit_id = entry.stable_value.trim().parse::<i32>().ok()?;
            Some(CommandModeTargetProjection {
                unit_target: Some(CommandUnitRef {
                    kind: 2,
                    value: unit_id,
                }),
                position_target: marker.map(|marker| position_target(marker.x, marker.y)),
                ..CommandModeTargetProjection::default()
            })
        }
        _ => None,
    }
}

pub fn apply_runtime_custom_packet_command_target(
    runtime_command_mode: &mut CommandModeState,
    target: CommandModeTargetProjection,
) {
    if target.is_empty() {
        return;
    }
    runtime_command_mode.record_command_units(
        &[],
        target.build_target,
        target.unit_target,
        target
            .position_target
            .map(|target| (f32::from_bits(target.x_bits), f32::from_bits(target.y_bits))),
    );
}

fn position_target(x: f32, y: f32) -> CommandModePositionTarget {
    CommandModePositionTarget {
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
    }
}

fn parse_world_pos(value: &str) -> Option<(f32, f32)> {
    if let Some((x, y)) = value.split_once(',') {
        return Some((x.trim().parse().ok()?, y.trim().parse().ok()?));
    }
    if let Some((x, y)) = value.split_once(':') {
        return Some((x.trim().parse().ok()?, y.trim().parse().ok()?));
    }
    None
}

fn build_pos_world_pos(build_pos: i32) -> (f32, f32) {
    let (tile_x, tile_y) = unpack_point2(build_pos);
    (tile_x as f32 * 8.0, tile_y as f32 * 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_packet_runtime::RuntimeCustomPacketSemanticEncoding;
    use crate::custom_packet_runtime_surface::RuntimeCustomPacketOverlayMarker;
    use mdt_typeio::pack_point2;

    #[test]
    fn resolve_runtime_custom_packet_command_target_maps_build_pos_into_target_projection() {
        let build_pos = pack_point2(3, 5);
        let entry = RuntimeCustomPacketSurfaceSummaryEntry {
            key: "build.select".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
            stable_value: build_pos.to_string(),
            marker: None,
        };

        assert_eq!(
            resolve_runtime_custom_packet_command_target(&entry, &SessionState::default(), None),
            Some(CommandModeTargetProjection {
                build_target: Some(build_pos),
                unit_target: None,
                position_target: Some(CommandModePositionTarget {
                    x_bits: 24.0f32.to_bits(),
                    y_bits: 40.0f32.to_bits(),
                }),
                rect_target: None,
            })
        );
    }

    #[test]
    fn resolve_runtime_custom_packet_command_target_uses_runtime_entity_position_for_unit_routes() {
        let entry = RuntimeCustomPacketSurfaceSummaryEntry {
            key: "logic.unit".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::UnitId,
            stable_value: "77".to_string(),
            marker: None,
        };
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .by_entity_id
            .insert(
                77,
                crate::session_state::TypedRuntimeEntityModel::Player(
                    crate::session_state::TypedRuntimePlayerEntity {
                        base: crate::session_state::TypedRuntimeEntityBase {
                            entity_id: 77,
                            class_id: 0,
                            hidden: false,
                            is_local_player: false,
                            unit_kind: 0,
                            unit_value: 0,
                            x_bits: 48.0f32.to_bits(),
                            y_bits: 120.0f32.to_bits(),
                            last_seen_entity_snapshot_count: 1,
                        },
                        semantic: crate::session_state::EntityPlayerSemanticProjection::default(),
                    },
                ),
            );

        assert_eq!(
            resolve_runtime_custom_packet_business_marker(&entry, &state),
            Some(RuntimeCustomPacketBusinessMarker {
                source: RuntimeCustomPacketBusinessMarkerSource::RuntimeEntity,
                x: 48.0,
                y: 120.0,
            })
        );
        assert_eq!(
            resolve_runtime_custom_packet_command_target(&entry, &state, None),
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: Some(CommandUnitRef { kind: 2, value: 77 }),
                position_target: Some(CommandModePositionTarget {
                    x_bits: 48.0f32.to_bits(),
                    y_bits: 120.0f32.to_bits(),
                }),
                rect_target: None,
            })
        );
    }

    #[test]
    fn apply_runtime_custom_packet_command_target_updates_command_mode_without_selection() {
        let mut runtime_command_mode = CommandModeState::default();
        runtime_command_mode.bind_control_group(4, &[88, 99]);
        let target = CommandModeTargetProjection {
            build_target: Some(pack_point2(4, 6)),
            unit_target: Some(CommandUnitRef { kind: 2, value: 77 }),
            position_target: Some(CommandModePositionTarget {
                x_bits: 32.0f32.to_bits(),
                y_bits: 48.0f32.to_bits(),
            }),
            rect_target: None,
        };

        apply_runtime_custom_packet_command_target(&mut runtime_command_mode, target);

        assert!(runtime_command_mode.is_active());
        assert!(runtime_command_mode.projection().selected_units.is_empty());
        assert_eq!(runtime_command_mode.projection().last_target, Some(target));
        assert_eq!(
            runtime_command_mode.projection().control_groups,
            vec![mdt_input::CommandModeControlGroupProjection {
                index: 4,
                unit_ids: vec![88, 99],
            }]
        );
    }

    #[test]
    fn resolve_runtime_custom_packet_command_target_prefers_surface_marker_for_world_pos() {
        let entry = RuntimeCustomPacketSurfaceSummaryEntry {
            key: "logic.world".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
            stable_value: "7,9".to_string(),
            marker: Some(RuntimeCustomPacketOverlayMarker {
                key: "logic.world".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                x: 12.5,
                y: -4.0,
            }),
        };

        assert_eq!(
            resolve_runtime_custom_packet_command_target(&entry, &SessionState::default(), None),
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: None,
                position_target: Some(CommandModePositionTarget {
                    x_bits: 12.5f32.to_bits(),
                    y_bits: (-4.0f32).to_bits(),
                }),
                rect_target: None,
            })
        );
    }
}
