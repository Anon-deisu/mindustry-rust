use crate::custom_packet_runtime::RuntimeCustomPacketSemanticKind;
use crate::custom_packet_runtime_surface::{
    build_pos_world_pos, finite_surface_world_pos, parse_surface_build_pos,
    parse_surface_world_pos, RuntimeCustomPacketSurfaceSummaryEntry,
};
use crate::session_state::SessionState;
use mdt_input::{
    CommandModePositionTarget, CommandModeState, CommandModeTargetProjection, CommandUnitRef,
};

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
        return finite_business_marker(RuntimeCustomPacketBusinessMarker {
            source: RuntimeCustomPacketBusinessMarkerSource::Surface,
            x: marker.x,
            y: marker.y,
        });
    }
    if entry.semantic != RuntimeCustomPacketSemanticKind::UnitId {
        return None;
    }
    let unit_id = entry.stable_value.trim().parse::<i32>().ok()?;
    let projection = session_state.runtime_typed_entity_projection();
    let entity = projection.entity_at(unit_id)?;
    finite_business_marker(RuntimeCustomPacketBusinessMarker {
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
    let marker = match resolved_marker {
        Some(marker) => Some(finite_business_marker(marker)?),
        None => None,
    };
    let marker = marker.as_ref();
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
            let build_pos = parse_surface_build_pos(&entry.stable_value)?;
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
    let position_target = target
        .position_target
        .map(|target| (f32::from_bits(target.x_bits), f32::from_bits(target.y_bits)));
    if position_target
        .and_then(|(x, y)| finite_surface_world_pos(x, y))
        .is_none()
        && target.position_target.is_some()
    {
        return;
    }
    runtime_command_mode.record_command_units(
        &[],
        target.build_target,
        target.unit_target,
        position_target,
    );
}

fn position_target(x: f32, y: f32) -> CommandModePositionTarget {
    CommandModePositionTarget {
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
    }
}

fn parse_world_pos(value: &str) -> Option<(f32, f32)> {
    parse_surface_world_pos(value)
}

fn finite_business_marker(
    marker: RuntimeCustomPacketBusinessMarker,
) -> Option<RuntimeCustomPacketBusinessMarker> {
    let (x, y) = finite_surface_world_pos(marker.x, marker.y)?;
    Some(RuntimeCustomPacketBusinessMarker { x, y, ..marker })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_packet_runtime::RuntimeCustomPacketSemanticEncoding;
    use crate::custom_packet_runtime_surface::RuntimeCustomPacketOverlayMarker;
    use crate::session_state::{
        EntityPlayerSemanticProjection, TypedRuntimeEntityBase, TypedRuntimeEntityModel,
        TypedRuntimePlayerEntity,
    };
    use mdt_typeio::pack_point2;

    fn summary_entry(
        key: &str,
        encoding: RuntimeCustomPacketSemanticEncoding,
        semantic: RuntimeCustomPacketSemanticKind,
        stable_value: impl Into<String>,
    ) -> RuntimeCustomPacketSurfaceSummaryEntry {
        RuntimeCustomPacketSurfaceSummaryEntry {
            key: key.to_string(),
            encoding,
            semantic,
            stable_value: stable_value.into(),
            marker: None,
        }
    }

    fn summary_entry_with_marker(
        key: &str,
        encoding: RuntimeCustomPacketSemanticEncoding,
        semantic: RuntimeCustomPacketSemanticKind,
        stable_value: impl Into<String>,
        x: f32,
        y: f32,
    ) -> RuntimeCustomPacketSurfaceSummaryEntry {
        RuntimeCustomPacketSurfaceSummaryEntry {
            key: key.to_string(),
            encoding,
            semantic,
            stable_value: stable_value.into(),
            marker: Some(RuntimeCustomPacketOverlayMarker {
                key: key.to_string(),
                encoding,
                semantic,
                x,
                y,
            }),
        }
    }

    fn session_state_with_player(entity_id: i32, x: f32, y: f32) -> SessionState {
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .by_entity_id
            .insert(
                entity_id,
                TypedRuntimeEntityModel::Player(TypedRuntimePlayerEntity {
                    base: TypedRuntimeEntityBase {
                        entity_id,
                        class_id: 0,
                        hidden: false,
                        is_local_player: false,
                        unit_kind: 0,
                        unit_value: 0,
                        x_bits: x.to_bits(),
                        y_bits: y.to_bits(),
                        last_seen_entity_snapshot_count: 1,
                    },
                    semantic: EntityPlayerSemanticProjection::default(),
                }),
            );
        state
    }

    #[test]
    fn resolve_runtime_custom_packet_command_target_maps_build_pos_into_target_projection() {
        let build_pos = pack_point2(3, 5);
        let entry = summary_entry(
            "build.select",
            RuntimeCustomPacketSemanticEncoding::Text,
            RuntimeCustomPacketSemanticKind::BuildPos,
            build_pos.to_string(),
        );

        assert_eq!(
            resolve_runtime_custom_packet_command_target(&entry, &SessionState::default(), None),
            Some(CommandModeTargetProjection {
                build_target: Some(build_pos),
                unit_target: None,
                position_target: Some(position_target(24.0, 40.0)),
                rect_target: None,
            })
        );
    }

    #[test]
    fn resolve_runtime_custom_packet_command_target_rejects_non_finite_world_pos() {
        let entry = summary_entry(
            "logic.target",
            RuntimeCustomPacketSemanticEncoding::Text,
            RuntimeCustomPacketSemanticKind::WorldPos,
            "NaN,12",
        );

        assert_eq!(
            resolve_runtime_custom_packet_command_target(&entry, &SessionState::default(), None),
            None
        );
    }

    #[test]
    fn resolve_runtime_custom_packet_command_target_uses_runtime_entity_position_for_unit_routes() {
        let entry = summary_entry(
            "logic.unit",
            RuntimeCustomPacketSemanticEncoding::LogicData,
            RuntimeCustomPacketSemanticKind::UnitId,
            "77",
        );
        let state = session_state_with_player(77, 48.0, 120.0);

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
                position_target: Some(position_target(48.0, 120.0)),
                rect_target: None,
            })
        );
    }

    #[test]
    fn resolve_runtime_custom_packet_business_marker_trims_unit_id_whitespace() {
        let entry = summary_entry(
            "logic.unit",
            RuntimeCustomPacketSemanticEncoding::LogicData,
            RuntimeCustomPacketSemanticKind::UnitId,
            " 77 ",
        );
        let state = session_state_with_player(77, 16.0, 24.0);

        assert_eq!(
            resolve_runtime_custom_packet_business_marker(&entry, &state),
            Some(RuntimeCustomPacketBusinessMarker {
                source: RuntimeCustomPacketBusinessMarkerSource::RuntimeEntity,
                x: 16.0,
                y: 24.0,
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
            position_target: Some(position_target(32.0, 48.0)),
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
        let entry = summary_entry_with_marker(
            "logic.world",
            RuntimeCustomPacketSemanticEncoding::LogicData,
            RuntimeCustomPacketSemanticKind::WorldPos,
            "7,9",
            12.5,
            -4.0,
        );

        assert_eq!(
            resolve_runtime_custom_packet_command_target(&entry, &SessionState::default(), None),
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: None,
                position_target: Some(position_target(12.5, -4.0)),
                rect_target: None,
            })
        );
    }

    #[test]
    fn reject_non_finite_runtime_entity_marker_for_build_and_unit_targets() {
        let state = session_state_with_player(77, f32::NAN, f32::INFINITY);

        let unit_entry = summary_entry(
            "logic.unit",
            RuntimeCustomPacketSemanticEncoding::LogicData,
            RuntimeCustomPacketSemanticKind::UnitId,
            "77",
        );
        let build_entry = summary_entry(
            "build.select",
            RuntimeCustomPacketSemanticEncoding::Text,
            RuntimeCustomPacketSemanticKind::BuildPos,
            pack_point2(3, 5).to_string(),
        );
        let marker = RuntimeCustomPacketBusinessMarker {
            source: RuntimeCustomPacketBusinessMarkerSource::RuntimeEntity,
            x: f32::NAN,
            y: f32::INFINITY,
        };

        assert_eq!(
            resolve_runtime_custom_packet_business_marker(&unit_entry, &state),
            None
        );
        assert_eq!(
            resolve_runtime_custom_packet_command_target(&build_entry, &state, Some(&marker)),
            None
        );
        assert_eq!(
            resolve_runtime_custom_packet_command_target(&unit_entry, &state, Some(&marker)),
            None
        );
    }

    #[test]
    fn reject_non_finite_surface_marker_for_world_and_unit_targets() {
        let world_entry = summary_entry_with_marker(
            "logic.world",
            RuntimeCustomPacketSemanticEncoding::LogicData,
            RuntimeCustomPacketSemanticKind::WorldPos,
            "7,9",
            f32::NAN,
            9.0,
        );
        let unit_entry = summary_entry_with_marker(
            "logic.unit",
            RuntimeCustomPacketSemanticEncoding::LogicData,
            RuntimeCustomPacketSemanticKind::UnitId,
            "77",
            4.0,
            f32::INFINITY,
        );

        assert_eq!(
            resolve_runtime_custom_packet_business_marker(&world_entry, &SessionState::default()),
            None
        );
        assert_eq!(
            resolve_runtime_custom_packet_business_marker(&unit_entry, &SessionState::default()),
            None
        );
        assert_eq!(
            resolve_runtime_custom_packet_command_target(
                &world_entry,
                &SessionState::default(),
                None
            ),
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: None,
                position_target: Some(position_target(7.0, 9.0)),
                rect_target: None,
            })
        );
        assert_eq!(
            resolve_runtime_custom_packet_command_target(
                &unit_entry,
                &SessionState::default(),
                None
            ),
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: Some(CommandUnitRef { kind: 2, value: 77 }),
                position_target: None,
                rect_target: None,
            })
        );
    }
}
