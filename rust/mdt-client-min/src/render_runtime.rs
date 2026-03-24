use crate::client_session::{
    BuildHealthPair, ClientBuildPlan, ClientBuildPlanConfig, ClientSessionEvent,
    ClientSnapshotInputState, StateSnapshotAppliedProjection,
};
use crate::effect_runtime::{
    resolve_runtime_effect_overlay_position, spawn_runtime_effect_overlay, RuntimeEffectOverlay,
};
use crate::session_state::{
    AuthoritativeStateMirror, BuilderPlanStage, BuilderQueueProjection,
    BuildingProjectionUpdateKind, BuildingTableProjection, ConfiguredBlockOutcome,
    ConfiguredBlockProjection, ConfiguredContentRef, EffectBusinessContentKind,
    EffectBusinessPositionSource, EffectBusinessProjection, EffectDataSemantic,
    HiddenSnapshotDeltaProjection, SessionResetKind, SessionState, SessionTimeoutKind,
    StateSnapshotAuthorityProjection, StateSnapshotBusinessProjection,
    TileConfigAuthoritySource, TileConfigProjection, UnitRefProjection,
    WorldBootstrapProjection, WorldReloadProjection,
};
use mdt_remote::{HighFrequencyRemoteMethod, HIGH_FREQUENCY_REMOTE_METHOD_COUNT};
use mdt_render_ui::{
    HudModel, RenderModel, RenderObject, RuntimeHudTextObservability,
    RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const EFFECT_OVERLAY_LIMIT: usize = 8;
const EFFECT_OVERLAY_TTL_TICKS: u8 = 3;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderRuntimeAdapter {
    world_overlay: RuntimeWorldOverlay,
}

impl RenderRuntimeAdapter {
    pub fn observe_events(&mut self, events: &[ClientSessionEvent]) {
        advance_runtime_effect_overlays(&mut self.world_overlay);
        observe_runtime_world_events(&mut self.world_overlay, events);
    }

    pub fn apply(
        &self,
        scene: &mut RenderModel,
        hud: &mut HudModel,
        snapshot_input: &ClientSnapshotInputState,
        session_state: &SessionState,
    ) {
        let config_stats = runtime_build_plan_config_stats(snapshot_input.plans.as_deref());
        append_runtime_build_plan_objects(scene, snapshot_input.plans.as_deref());
        append_runtime_world_overlay_objects(
            scene,
            &self.world_overlay,
            snapshot_input,
            session_state,
        );
        append_building_table_projection_objects(scene, session_state);
        append_block_snapshot_projection_objects(scene, session_state);
        let bootstrap_projection = session_state.world_bootstrap_projection.as_ref();
        let runtime_state_mirror = session_state.authoritative_state_mirror.as_ref();
        let state_authority_projection = session_state.state_snapshot_authority_projection.as_ref();
        let state_business_projection = session_state.state_snapshot_business_projection.as_ref();
        hud.runtime_ui = Some(runtime_ui_observability(session_state));
        hud.status_text = format!(
            "{} runtime_selected={} runtime_plans={} runtime_cfg_int={} runtime_cfg_long={} runtime_cfg_float={} runtime_cfg_bool={} runtime_cfg_int_seq={} runtime_cfg_point2={} runtime_cfg_point2_array={} runtime_cfg_tech_node={} runtime_cfg_double={} runtime_cfg_building_pos={} runtime_cfg_laccess={} runtime_cfg_string={} runtime_cfg_bytes={} runtime_cfg_legacy_unit_command_null={} runtime_cfg_bool_array={} runtime_cfg_unit_id={} runtime_cfg_vec2_array={} runtime_cfg_vec2={} runtime_cfg_team={} runtime_cfg_int_array={} runtime_cfg_object_array={} runtime_cfg_content={} runtime_cfg_unit_command={} runtime_world_tiles={} runtime_health={} building={} runtime_builder={} runtime_builder_head={} runtime_entity_local={} runtime_entity_hidden={} runtime_entity_gate={} runtime_entity_sync={} runtime_snap_last={} runtime_snap_events={} runtime_snap_apply={} runtime_wave={} runtime_enemies={} runtime_tps={} runtime_state_apply={} runtime_core_teams={} runtime_core_items={} runtime_buildings={} runtime_block={} runtime_block_fail={} runtime_hidden={} runtime_hidden_delta={} runtime_hidden_fail={} runtime_effects={} runtime_effect_data_kind={} runtime_effect_contract={} runtime_effect_data_semantic={} runtime_effect_apply={} runtime_effect_path={} runtime_effect_data_fail={} bootstrap_rules={} bootstrap_tags={} bootstrap_locales={} bootstrap_teams={} bootstrap_markers={} bootstrap_chunks={} bootstrap_patches={} bootstrap_plans={} bootstrap_fog_teams={} runtime_view_center={} runtime_view_size={} runtime_position={} runtime_pointer={} runtime_selected_rotation={} runtime_input_flags={} runtime_snap_client={} runtime_snap_state={} runtime_snap_entity={} runtime_snap_block={} runtime_snap_hidden={} runtime_tilecfg_events={} runtime_tilecfg_parse_fail={} runtime_tilecfg_noapply={} runtime_tilecfg_rollback={} runtime_tilecfg_pending_mismatch={} runtime_tilecfg_apply={} runtime_configured={} runtime_take_items={} runtime_transfer_item={} runtime_transfer_item_unit={} runtime_payload_drop={} runtime_payload_pick_build={} runtime_payload_pick_unit={} runtime_unit_entered_payload={} runtime_unit_despawn={} runtime_unit_lifecycle={} runtime_spawn_fx={} runtime_audio={} runtime_admin={} runtime_kick={} runtime_loading={} runtime_rules={} runtime_ui_notice={} runtime_ui_menu={} runtime_world_label={} runtime_marker={} runtime_logic_sync={} runtime_resource_delta={} runtime_command_ctrl={} runtime_gameplay_signal={}",
            hud.status_text,
            runtime_selected_block_label(snapshot_input.selected_block_id),
            snapshot_input.plans.as_ref().map_or(0, Vec::len),
            config_stats.int,
            config_stats.long,
            config_stats.float,
            config_stats.bool,
            config_stats.int_seq,
            config_stats.point2,
            config_stats.point2_array,
            config_stats.tech_node,
            config_stats.double,
            config_stats.building_pos,
            config_stats.laccess,
            config_stats.string,
            config_stats.bytes,
            config_stats.legacy_unit_command_null,
            config_stats.bool_array,
            config_stats.unit_id,
            config_stats.vec2_array,
            config_stats.vec2,
            config_stats.team,
            config_stats.int_array,
            config_stats.object_array,
            config_stats.content,
            config_stats.unit_command,
            self.world_overlay.tile_overlays.len(),
            self.world_overlay.health_overlay_count(),
            if snapshot_input.building { 1 } else { 0 },
            runtime_builder_queue_label(&session_state.builder_queue_projection),
            runtime_builder_queue_head_label(&session_state.builder_queue_projection),
            runtime_local_entity_label(session_state),
            session_state.entity_table_projection.hidden_count,
            runtime_entity_gate_label(session_state),
            runtime_entity_sync_label(session_state),
            runtime_snapshot_method_label(self.world_overlay.last_snapshot_method),
            self.world_overlay.snapshot_refresh_count,
            runtime_state_snapshot_applied_event_label(
                self.world_overlay.last_state_snapshot_applied.as_ref(),
            ),
            runtime_state_mirror
                .map(|projection| projection.wave)
                .or_else(|| state_authority_projection.map(|projection| projection.wave))
                .or_else(|| state_business_projection.map(|projection| projection.wave))
                .or_else(|| session_state.last_state_snapshot.as_ref().map(|snapshot| snapshot.wave))
                .unwrap_or_default(),
            runtime_state_mirror
                .map(|projection| projection.enemies)
                .or_else(|| state_authority_projection.map(|projection| projection.enemies))
                .or_else(|| state_business_projection.map(|projection| projection.enemies))
                .or_else(|| {
                    session_state
                        .last_state_snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.enemies)
                })
                .unwrap_or_default(),
            runtime_state_mirror
                .map(|projection| projection.tps)
                .or_else(|| state_authority_projection.map(|projection| projection.tps))
                .or_else(|| state_business_projection.map(|projection| projection.tps))
                .or_else(|| session_state.last_state_snapshot.as_ref().map(|snapshot| snapshot.tps))
                .unwrap_or_default(),
            runtime_state_projection_label(
                runtime_state_mirror,
                state_authority_projection,
                state_business_projection,
            ),
            runtime_state_mirror
                .map(|projection| projection.core_inventory_team_count)
                .or_else(|| {
                    state_authority_projection.map(|projection| projection.core_inventory_team_count)
                })
                .or_else(|| {
                    state_business_projection.map(|projection| projection.core_inventory_team_count)
                })
                .or_else(|| {
                    session_state
                        .last_good_state_snapshot_core_data
                        .as_ref()
                        .map(|core_data| usize::from(core_data.team_count))
                })
                .unwrap_or_default(),
            runtime_state_mirror
                .map(|projection| projection.core_inventory_item_entry_count)
                .or_else(|| {
                    state_authority_projection
                        .map(|projection| projection.core_inventory_item_entry_count)
                })
                .or_else(|| {
                    state_business_projection
                        .map(|projection| projection.core_inventory_item_entry_count)
                })
                .or_else(|| {
                    session_state
                        .last_good_state_snapshot_core_data
                        .as_ref()
                        .map(|core_data| {
                        core_data
                            .teams
                            .iter()
                            .map(|team| team.items.len())
                            .sum::<usize>()
                        })
                })
                .unwrap_or_default(),
            runtime_building_table_label(&session_state.building_table_projection),
            runtime_block_snapshot_label(session_state),
            session_state.failed_block_snapshot_parse_count,
            runtime_hidden_snapshot_label(session_state),
            runtime_hidden_snapshot_delta_label(session_state),
            session_state.failed_hidden_snapshot_parse_count,
            session_state.received_effect_count,
            runtime_effect_data_kind_label(session_state.last_effect_data_kind.as_deref()),
            runtime_effect_contract_label(session_state),
            runtime_effect_data_semantic_label(session_state.last_effect_data_semantic.as_ref()),
            runtime_effect_business_projection_label(
                session_state.last_effect_business_projection.as_ref(),
            ),
            runtime_effect_path_label(session_state.last_effect_business_path.as_deref()),
            runtime_effect_data_fail_label(session_state),
            runtime_bootstrap_hash_label(bootstrap_projection, |projection| {
                projection.rules_sha256.as_str()
            }),
            runtime_bootstrap_hash_label(bootstrap_projection, |projection| {
                projection.tags_sha256.as_str()
            }),
            runtime_bootstrap_hash_label(bootstrap_projection, |projection| {
                projection.map_locales_sha256.as_str()
            }),
            bootstrap_projection.map_or(0, |projection| projection.team_count),
            bootstrap_projection.map_or(0, |projection| projection.marker_count),
            bootstrap_projection.map_or(0, |projection| projection.custom_chunk_count),
            bootstrap_projection.map_or(0, |projection| projection.content_patch_count),
            bootstrap_projection.map_or(0, |projection| projection.player_team_plan_count),
            bootstrap_projection.map_or(0, |projection| projection.static_fog_team_count),
            runtime_optional_vec2_label(snapshot_input.view_center),
            runtime_optional_vec2_label(snapshot_input.view_size),
            runtime_optional_vec2_label(snapshot_input.position),
            runtime_optional_vec2_label(snapshot_input.pointer),
            snapshot_input.selected_rotation,
            runtime_input_flags_label(snapshot_input),
            self.world_overlay
                .snapshot_method_count(HighFrequencyRemoteMethod::ClientSnapshot),
            self.world_overlay
                .snapshot_method_count(HighFrequencyRemoteMethod::StateSnapshot),
            self.world_overlay
                .snapshot_method_count(HighFrequencyRemoteMethod::EntitySnapshot),
            self.world_overlay
                .snapshot_method_count(HighFrequencyRemoteMethod::BlockSnapshot),
            self.world_overlay
                .snapshot_method_count(HighFrequencyRemoteMethod::HiddenSnapshot),
            self.world_overlay.tile_config_event_count,
            self.world_overlay.tile_config_parse_failed_count,
            self.world_overlay.tile_config_business_not_applied_count,
            self.world_overlay.tile_config_rollback_count,
            self.world_overlay.tile_config_pending_mismatch_count,
            runtime_tile_config_business_label(&session_state.tile_config_projection),
            runtime_configured_block_projection_label(&session_state.configured_block_projection),
            session_state.received_take_items_count,
            session_state.received_transfer_item_to_count,
            session_state.received_transfer_item_to_unit_count,
            session_state.received_payload_dropped_count,
            session_state.received_picked_build_payload_count,
            session_state.received_picked_unit_payload_count,
            session_state.received_unit_entered_payload_count,
            session_state.received_unit_despawn_count,
            runtime_unit_lifecycle_label(session_state),
            runtime_spawn_fx_label(session_state),
            runtime_audio_label(session_state),
            runtime_admin_label(session_state),
            runtime_kick_label(&self.world_overlay),
            runtime_loading_label(session_state),
            runtime_rules_label(session_state),
            runtime_ui_notice_label(session_state),
            runtime_ui_menu_label(session_state),
            runtime_world_label_label(session_state),
            runtime_marker_label(session_state),
            runtime_logic_sync_label(session_state),
            runtime_resource_delta_label(session_state),
            runtime_command_control_label(session_state),
            runtime_gameplay_signal_label(session_state),
        );
    }

    pub fn world_overlay(&self) -> &RuntimeWorldOverlay {
        &self.world_overlay
    }

    pub fn clear(&mut self) {
        self.world_overlay.clear();
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeWorldOverlay {
    pub tile_overlays: BTreeMap<(i32, i32), RuntimeTileOverlay>,
    pub effect_overlays: Vec<RuntimeEffectOverlay>,
    pub snapshot_refresh_count: u64,
    pub last_snapshot_method: Option<HighFrequencyRemoteMethod>,
    pub snapshot_method_counts: [u64; HIGH_FREQUENCY_REMOTE_METHOD_COUNT],
    pub state_snapshot_applied_count: u64,
    pub last_state_snapshot_applied: Option<StateSnapshotAppliedProjection>,
    pub tile_config_event_count: u64,
    pub tile_config_parse_failed_count: u64,
    pub tile_config_business_not_applied_count: u64,
    pub tile_config_rollback_count: u64,
    pub tile_config_pending_mismatch_count: u64,
    pub last_kick_reason_text: Option<String>,
    pub last_kick_reason_ordinal: Option<i32>,
    pub last_kick_duration_ms: Option<u64>,
    pub last_kick_hint_category: Option<&'static str>,
    pub last_kick_hint_text: Option<&'static str>,
}

impl RuntimeWorldOverlay {
    pub fn clear(&mut self) {
        self.tile_overlays.clear();
        self.effect_overlays.clear();
        self.snapshot_refresh_count = 0;
        self.last_snapshot_method = None;
        self.snapshot_method_counts = [0; HIGH_FREQUENCY_REMOTE_METHOD_COUNT];
        self.state_snapshot_applied_count = 0;
        self.last_state_snapshot_applied = None;
        self.tile_config_event_count = 0;
        self.tile_config_parse_failed_count = 0;
        self.tile_config_business_not_applied_count = 0;
        self.tile_config_rollback_count = 0;
        self.tile_config_pending_mismatch_count = 0;
        self.last_kick_reason_text = None;
        self.last_kick_reason_ordinal = None;
        self.last_kick_duration_ms = None;
        self.last_kick_hint_category = None;
        self.last_kick_hint_text = None;
    }

    pub fn health_overlay_count(&self) -> usize {
        self.tile_overlays
            .values()
            .filter(|overlay| overlay.health_bits.is_some())
            .count()
    }

    pub fn snapshot_method_count(&self, method: HighFrequencyRemoteMethod) -> u64 {
        self.snapshot_method_counts[runtime_snapshot_method_bucket_index(method)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTileOverlay {
    pub kind: RuntimeTileOverlayKind,
    pub block_id: Option<i16>,
    pub health_bits: Option<u32>,
    pub config_kind_name: Option<String>,
    pub parse_failed: bool,
    pub business_applied: bool,
    pub pending_local_match: Option<bool>,
    pub rollback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTileOverlayKind {
    Constructed,
    Deconstructed,
    HealthUpdated,
    Configured,
}

pub fn observe_runtime_world_events(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    events: &[ClientSessionEvent],
) {
    for event in events {
        match event {
            ClientSessionEvent::WorldDataBegin
            | ClientSessionEvent::WorldStreamStarted { .. }
            | ClientSessionEvent::ConnectRedirectRequested { .. } => runtime_world_overlay.clear(),
            ClientSessionEvent::Kicked {
                reason_text,
                reason_ordinal,
                duration_ms,
            } => {
                runtime_world_overlay.clear();
                runtime_world_overlay.last_kick_reason_text = reason_text.clone();
                runtime_world_overlay.last_kick_reason_ordinal = *reason_ordinal;
                runtime_world_overlay.last_kick_duration_ms = *duration_ms;
                let (hint_category, hint_text) =
                    runtime_kick_hint_from(reason_text.as_deref(), *reason_ordinal)
                        .unwrap_or((None, None));
                runtime_world_overlay.last_kick_hint_category = hint_category;
                runtime_world_overlay.last_kick_hint_text = hint_text;
            }
            ClientSessionEvent::ConstructFinish {
                tile_pos, block_id, ..
            } => {
                runtime_world_overlay.tile_overlays.insert(
                    unpack_runtime_point2(*tile_pos),
                    RuntimeTileOverlay {
                        kind: RuntimeTileOverlayKind::Constructed,
                        block_id: *block_id,
                        health_bits: None,
                        config_kind_name: None,
                        parse_failed: false,
                        business_applied: true,
                        pending_local_match: None,
                        rollback: false,
                    },
                );
            }
            ClientSessionEvent::DeconstructFinish {
                tile_pos, block_id, ..
            } => {
                runtime_world_overlay.tile_overlays.insert(
                    unpack_runtime_point2(*tile_pos),
                    RuntimeTileOverlay {
                        kind: RuntimeTileOverlayKind::Deconstructed,
                        block_id: *block_id,
                        health_bits: None,
                        config_kind_name: None,
                        parse_failed: false,
                        business_applied: true,
                        pending_local_match: None,
                        rollback: false,
                    },
                );
            }
            ClientSessionEvent::TileConfig {
                build_pos,
                config_kind_name,
                parse_failed,
                business_applied,
                was_rollback,
                pending_local_match,
                ..
            } => {
                runtime_world_overlay.tile_config_event_count = runtime_world_overlay
                    .tile_config_event_count
                    .saturating_add(1);
                if *parse_failed {
                    runtime_world_overlay.tile_config_parse_failed_count = runtime_world_overlay
                        .tile_config_parse_failed_count
                        .saturating_add(1);
                }
                if !*business_applied {
                    runtime_world_overlay.tile_config_business_not_applied_count =
                        runtime_world_overlay
                            .tile_config_business_not_applied_count
                            .saturating_add(1);
                }
                if *was_rollback {
                    runtime_world_overlay.tile_config_rollback_count = runtime_world_overlay
                        .tile_config_rollback_count
                        .saturating_add(1);
                }
                if matches!(pending_local_match, Some(false)) {
                    runtime_world_overlay.tile_config_pending_mismatch_count =
                        runtime_world_overlay
                            .tile_config_pending_mismatch_count
                            .saturating_add(1);
                }
                if let Some(build_pos) = build_pos {
                    runtime_world_overlay.tile_overlays.insert(
                        unpack_runtime_point2(*build_pos),
                        RuntimeTileOverlay {
                            kind: RuntimeTileOverlayKind::Configured,
                            block_id: None,
                            health_bits: None,
                            config_kind_name: config_kind_name.clone(),
                            parse_failed: *parse_failed,
                            business_applied: *business_applied,
                            pending_local_match: *pending_local_match,
                            rollback: *was_rollback,
                        },
                    );
                }
            }
            ClientSessionEvent::BuildHealthUpdate { pairs, .. } => {
                observe_build_health_pairs(runtime_world_overlay, pairs);
            }
            ClientSessionEvent::EffectRequested {
                effect_id,
                x,
                y,
                rotation,
                color_rgba,
                data_object,
            } => {
                push_runtime_effect_overlay(
                    runtime_world_overlay,
                    spawn_runtime_effect_overlay(
                        *effect_id,
                        *x,
                        *y,
                        *rotation,
                        *color_rgba,
                        false,
                        data_object.as_ref(),
                        EFFECT_OVERLAY_TTL_TICKS,
                    ),
                );
            }
            ClientSessionEvent::EffectReliableRequested {
                effect_id,
                x,
                y,
                rotation,
                color_rgba,
            } => {
                push_runtime_effect_overlay(
                    runtime_world_overlay,
                    spawn_runtime_effect_overlay(
                        *effect_id,
                        *x,
                        *y,
                        *rotation,
                        *color_rgba,
                        true,
                        None,
                        EFFECT_OVERLAY_TTL_TICKS,
                    ),
                );
            }
            ClientSessionEvent::SnapshotReceived(method) => {
                runtime_world_overlay.snapshot_refresh_count = runtime_world_overlay
                    .snapshot_refresh_count
                    .saturating_add(1);
                runtime_world_overlay.last_snapshot_method = Some(*method);
                let bucket = runtime_snapshot_method_bucket_index(*method);
                runtime_world_overlay.snapshot_method_counts[bucket] =
                    runtime_world_overlay.snapshot_method_counts[bucket].saturating_add(1);
            }
            ClientSessionEvent::StateSnapshotApplied { projection } => {
                runtime_world_overlay.snapshot_refresh_count = runtime_world_overlay
                    .snapshot_refresh_count
                    .saturating_add(1);
                runtime_world_overlay.last_snapshot_method =
                    Some(HighFrequencyRemoteMethod::StateSnapshot);
                runtime_world_overlay.snapshot_method_counts
                    [runtime_snapshot_method_bucket_index(
                        HighFrequencyRemoteMethod::StateSnapshot,
                    )] = runtime_world_overlay.snapshot_method_counts
                    [runtime_snapshot_method_bucket_index(
                        HighFrequencyRemoteMethod::StateSnapshot,
                    )]
                .saturating_add(1);
                runtime_world_overlay.state_snapshot_applied_count = runtime_world_overlay
                    .state_snapshot_applied_count
                    .saturating_add(1);
                runtime_world_overlay.last_state_snapshot_applied = Some(projection.clone());
            }
            _ => {}
        }
    }
}

fn runtime_snapshot_method_bucket_index(method: HighFrequencyRemoteMethod) -> usize {
    match method {
        HighFrequencyRemoteMethod::ClientSnapshot => 0,
        HighFrequencyRemoteMethod::StateSnapshot => 1,
        HighFrequencyRemoteMethod::EntitySnapshot => 2,
        HighFrequencyRemoteMethod::BlockSnapshot => 3,
        HighFrequencyRemoteMethod::HiddenSnapshot => 4,
    }
}

fn push_runtime_effect_overlay(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    overlay: RuntimeEffectOverlay,
) {
    runtime_world_overlay.effect_overlays.push(overlay);
    if runtime_world_overlay.effect_overlays.len() > EFFECT_OVERLAY_LIMIT {
        let overflow = runtime_world_overlay.effect_overlays.len() - EFFECT_OVERLAY_LIMIT;
        runtime_world_overlay.effect_overlays.drain(0..overflow);
    }
}

fn advance_runtime_effect_overlays(runtime_world_overlay: &mut RuntimeWorldOverlay) {
    for overlay in &mut runtime_world_overlay.effect_overlays {
        overlay.remaining_ticks = overlay.remaining_ticks.saturating_sub(1);
    }
    runtime_world_overlay
        .effect_overlays
        .retain(|overlay| overlay.remaining_ticks > 0);
}

fn runtime_snapshot_method_label(method: Option<HighFrequencyRemoteMethod>) -> &'static str {
    match method {
        Some(method) => method.method_name(),
        None => "none",
    }
}

fn runtime_state_snapshot_applied_event_label(
    projection: Option<&StateSnapshotAppliedProjection>,
) -> String {
    projection
        .map(|projection| {
            let wave_window = match (projection.wave_advance_from, projection.wave_advance_to) {
                (Some(from), Some(to)) => format!("{from}->{to}"),
                _ => "none".to_string(),
            };
            format!(
                "w{}:s{}:gt{}:adv{}@{}:app{}:nd{}:rb{}:tr{}:wr{}:cpf{}:fb{}",
                projection.wave,
                projection.gameplay_state_name(),
                projection.gameplay_state_transition_count,
                if projection.wave_advanced { 1 } else { 0 },
                wave_window,
                projection.apply_count,
                projection.net_seconds_delta,
                if projection.net_seconds_rollback {
                    1
                } else {
                    0
                },
                projection.time_regress_count,
                projection.wave_regress_count,
                projection.core_parse_fail_count,
                if projection.used_last_good_core_fallback {
                    1
                } else {
                    0
                },
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_block_snapshot_label(session_state: &SessionState) -> String {
    session_state
        .last_block_snapshot
        .as_ref()
        .map(
            |snapshot| match (snapshot.first_build_pos, snapshot.first_block_id) {
                (Some(first_build_pos), Some(first_block_id)) => {
                    let (x, y) = unpack_runtime_point2(first_build_pos);
                    let mut label = format!(
                        "{}x{}@{}:{}#{}",
                        snapshot.amount, snapshot.data_len, x, y, first_block_id
                    );
                    if let Some(rotation) = snapshot.first_rotation {
                        label.push_str(&format!(":r{rotation}"));
                    }
                    if let Some(team_id) = snapshot.first_team_id {
                        label.push_str(&format!(":t{team_id}"));
                    }
                    if let Some(version) = snapshot.first_io_version {
                        label.push_str(&format!(":v{version}"));
                    }
                    if let Some(enabled) = snapshot.first_enabled {
                        label.push_str(&format!(":on{}", if enabled { 1 } else { 0 }));
                    }
                    if let Some(efficiency) = snapshot.first_efficiency {
                        label.push_str(&format!(":e{efficiency}"));
                    }
                    if let Some(optional_efficiency) = snapshot.first_optional_efficiency {
                        label.push_str(&format!(":oe{optional_efficiency}"));
                    }
                    if let Some(module_bitmask) = snapshot.first_module_bitmask {
                        label.push_str(&format!(":m{module_bitmask}"));
                    }
                    if let Some(visible_flags) = snapshot.first_visible_flags {
                        label.push_str(&format!(":vf{visible_flags}"));
                    }
                    label
                }
                _ => format!("{}x{}", snapshot.amount, snapshot.data_len),
            },
        )
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_hidden_snapshot_label(session_state: &SessionState) -> String {
    session_state
        .last_hidden_snapshot
        .as_ref()
        .map(|snapshot| {
            if snapshot.sample_ids.is_empty() {
                return match snapshot.first_id {
                    Some(first_id) => format!("{}@{}", snapshot.count, first_id),
                    None => snapshot.count.to_string(),
                };
            }
            let joined = snapshot
                .sample_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let remaining = usize::try_from(snapshot.count)
                .unwrap_or_default()
                .saturating_sub(snapshot.sample_ids.len());
            if remaining > 0 {
                format!("{}@{}+{}", snapshot.count, joined, remaining)
            } else {
                format!("{}@{}", snapshot.count, joined)
            }
        })
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_hidden_snapshot_delta_label(session_state: &SessionState) -> String {
    session_state
        .hidden_snapshot_delta_projection
        .as_ref()
        .map(|projection| {
            format!(
                "{}|{}",
                format_hidden_snapshot_delta_side('+', projection),
                format_hidden_snapshot_delta_side('-', projection),
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_state_business_projection_label(
    projection: Option<&StateSnapshotBusinessProjection>,
) -> String {
    projection
        .map(|projection| {
            format!(
                "w{}:e{}:t{}:c{}/{}:adv{}:core{}:s{}:nd{}:tr{}:wreg{}:ca{}:cas{}",
                projection.wave,
                projection.enemies,
                projection.tps,
                projection.core_inventory_team_count,
                projection.core_inventory_item_entry_count,
                if projection.last_wave_advanced { 1 } else { 0 },
                if projection.core_inventory_synced {
                    1
                } else {
                    0
                },
                match projection.gameplay_state {
                    crate::session_state::GameplayStateProjection::Playing => "play",
                    crate::session_state::GameplayStateProjection::Paused => "pause",
                    crate::session_state::GameplayStateProjection::GameOver => "gameover",
                },
                projection.net_seconds_delta,
                if projection.last_net_seconds_rollback {
                    1
                } else {
                    0
                },
                projection.state_snapshot_wave_regress_count,
                projection.core_inventory_changed_team_count,
                runtime_core_inventory_changed_team_sample_label(
                    &projection.core_inventory_changed_team_sample,
                    projection.core_inventory_changed_team_count,
                ),
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_configured_block_projection_label(projection: &ConfiguredBlockProjection) -> String {
    [
        runtime_configured_content_family_label(
            "uc",
            &projection.unit_cargo_unload_point_item_by_build_pos,
        ),
        runtime_configured_content_family_label("is", &projection.item_source_item_by_build_pos),
        runtime_configured_content_family_label(
            "ls",
            &projection.liquid_source_liquid_by_build_pos,
        ),
        runtime_configured_content_family_label("lp", &projection.landing_pad_item_by_build_pos),
        runtime_configured_content_family_label("so", &projection.sorter_item_by_build_pos),
        runtime_configured_content_family_label(
            "iv",
            &projection.inverted_sorter_item_by_build_pos,
        ),
        runtime_configured_bool_family_label("sw", &projection.switch_enabled_by_build_pos),
        runtime_configured_bool_family_label("do", &projection.door_open_by_build_pos),
        runtime_configured_string_family_label("mg", &projection.message_text_by_build_pos),
        runtime_configured_content_family_label(
            "ct",
            &projection.constructor_recipe_block_by_build_pos,
        ),
        runtime_configured_int_family_label("il", &projection.light_color_by_build_pos),
        runtime_configured_raw_content_family_label(
            "ps",
            &projection.payload_source_content_by_build_pos,
        ),
        runtime_configured_raw_content_family_label(
            "pr",
            &projection.payload_router_sorted_content_by_build_pos,
        ),
        runtime_configured_link_family_label("ib", &projection.item_bridge_link_by_build_pos),
        runtime_configured_content_family_label("ul", &projection.unloader_item_by_build_pos),
        runtime_configured_content_family_label("du", &projection.duct_unloader_item_by_build_pos),
        runtime_configured_content_family_label("dr", &projection.duct_router_item_by_build_pos),
        runtime_configured_link_family_label("md", &projection.mass_driver_link_by_build_pos),
        runtime_configured_link_family_label(
            "pm",
            &projection.payload_mass_driver_link_by_build_pos,
        ),
        runtime_configured_power_node_family_label("pn", &projection.power_node_links_by_build_pos),
        runtime_configured_unit_command_family_label(
            "rc",
            &projection.reconstructor_command_by_build_pos,
        ),
    ]
    .join(":")
}

fn runtime_configured_content_family_label(
    prefix: &str,
    values: &BTreeMap<i32, Option<i16>>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, Some(content_id))) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}={content_id}")
        }
        Some((build_pos, None)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=clear")
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_bool_family_label(
    prefix: &str,
    values: &BTreeMap<i32, Option<bool>>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, Some(value))) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}={}", if *value { 1 } else { 0 })
        }
        Some((build_pos, None)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=clear")
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_string_family_label(prefix: &str, values: &BTreeMap<i32, String>) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, text)) if text.is_empty() => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=empty")
        }
        Some((build_pos, text)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=len{}", text.chars().count())
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_int_family_label(prefix: &str, values: &BTreeMap<i32, i32>) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, value)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}={value:08x}")
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_raw_content_family_label(
    prefix: &str,
    values: &BTreeMap<i32, Option<ConfiguredContentRef>>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, Some(content))) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            let kind = match content.content_type {
                1 => "b",
                6 => "u",
                _ => "c",
            };
            format!("{prefix}{count}@{x}:{y}={kind}:{}", content.content_id)
        }
        Some((build_pos, None)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=clear")
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_link_family_label(
    prefix: &str,
    values: &BTreeMap<i32, Option<i32>>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, Some(link_pos))) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            let (target_x, target_y) = unpack_runtime_point2(*link_pos);
            format!("{prefix}{count}@{x}:{y}={target_x}:{target_y}")
        }
        Some((build_pos, None)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=clear")
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_power_node_family_label(
    prefix: &str,
    values: &BTreeMap<i32, BTreeSet<i32>>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, targets)) if targets.is_empty() => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=clear")
        }
        Some((build_pos, targets)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            let target_label = targets
                .iter()
                .map(|target_pos| {
                    let (target_x, target_y) = unpack_runtime_point2(*target_pos);
                    format!("{target_x}:{target_y}")
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("{prefix}{count}@{x}:{y}=n{}:{target_label}", targets.len())
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_unit_command_family_label(
    prefix: &str,
    values: &BTreeMap<i32, Option<u16>>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, Some(command_id))) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}={command_id}")
        }
        Some((build_pos, None)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            format!("{prefix}{count}@{x}:{y}=clear")
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_state_projection_label(
    runtime: Option<&AuthoritativeStateMirror>,
    authority: Option<&StateSnapshotAuthorityProjection>,
    business: Option<&StateSnapshotBusinessProjection>,
) -> String {
    if let Some(projection) = runtime {
        return format!(
            "w{}:e{}:t{}:c{}/{}:adv{}:core{}:s{}:nd{}:tr{}:wreg{}:ca{}:cas{}",
            projection.wave,
            projection.enemies,
            projection.tps,
            projection.core_inventory_team_count,
            projection.core_inventory_item_entry_count,
            if projection.last_wave_advanced { 1 } else { 0 },
            if projection.last_core_sync_ok { 1 } else { 0 },
            match projection.gameplay_state {
                crate::session_state::GameplayStateProjection::Playing => "play",
                crate::session_state::GameplayStateProjection::Paused => "pause",
                crate::session_state::GameplayStateProjection::GameOver => "gameover",
            },
            projection.net_seconds_delta,
            if projection.last_net_seconds_rollback {
                1
            } else {
                0
            },
            projection.wave_regress_count,
            projection.core_inventory_changed_team_count,
            runtime_core_inventory_changed_team_sample_label(
                &projection.core_inventory_changed_team_sample,
                projection.core_inventory_changed_team_count,
            ),
        );
    }
    if let Some(projection) = authority {
        return format!(
            "w{}:e{}:t{}:c{}/{}:adv{}:core{}:s{}:nd{}:tr{}:wreg{}:ca{}:cas{}",
            projection.wave,
            projection.enemies,
            projection.tps,
            projection.core_inventory_team_count,
            projection.core_inventory_item_entry_count,
            if projection.last_wave_advanced { 1 } else { 0 },
            if projection.last_core_sync_ok { 1 } else { 0 },
            match projection.gameplay_state {
                crate::session_state::GameplayStateProjection::Playing => "play",
                crate::session_state::GameplayStateProjection::Paused => "pause",
                crate::session_state::GameplayStateProjection::GameOver => "gameover",
            },
            projection.net_seconds_delta,
            if projection.last_net_seconds_rollback {
                1
            } else {
                0
            },
            projection.state_snapshot_wave_regress_count,
            projection.core_inventory_changed_team_count,
            runtime_core_inventory_changed_team_sample_label(
                &projection.core_inventory_changed_team_sample,
                projection.core_inventory_changed_team_count,
            ),
        );
    }
    runtime_state_business_projection_label(business)
}

fn runtime_core_inventory_changed_team_sample_label(sample: &[u8], changed_count: usize) -> String {
    if sample.is_empty() {
        return "none".to_string();
    }
    let joined = sample
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let remaining = changed_count.saturating_sub(sample.len());
    if remaining > 0 {
        format!("{joined}+{remaining}")
    } else {
        joined
    }
}

fn runtime_builder_queue_label(projection: &BuilderQueueProjection) -> String {
    let stage = match projection.last_stage {
        Some(BuilderPlanStage::Queued) => "queued",
        Some(BuilderPlanStage::InFlight) => "flight",
        Some(BuilderPlanStage::Finished) => "finish",
        Some(BuilderPlanStage::Removed) => "remove",
        None => "none",
    };
    let tile = match (projection.last_x, projection.last_y) {
        (Some(x), Some(y)) => format!("{x}:{y}"),
        _ => "none".to_string(),
    };
    let mode = match projection.last_breaking {
        Some(true) => "break",
        Some(false) => "place",
        None => "none",
    };
    format!(
        "q{}:i{}:f{}:r{}:o{}:{}@{}:{}:local{}",
        projection.queued_count,
        projection.inflight_count,
        projection.finished_count,
        projection.removed_count,
        projection.orphan_authoritative_count,
        stage,
        tile,
        mode,
        if projection.last_removed_local_plan {
            1
        } else {
            0
        },
    )
}

fn runtime_builder_queue_head_label(projection: &BuilderQueueProjection) -> String {
    let stage = match projection.head_stage {
        Some(BuilderPlanStage::Queued) => "queued",
        Some(BuilderPlanStage::InFlight) => "flight",
        Some(BuilderPlanStage::Finished) => "finish",
        Some(BuilderPlanStage::Removed) => "remove",
        None => "none",
    };
    let tile = match (projection.head_x, projection.head_y) {
        (Some(x), Some(y)) => format!("{x}:{y}"),
        _ => "none".to_string(),
    };
    let mode = match projection.head_breaking {
        Some(true) => "break",
        Some(false) => "place",
        None => "none",
    };
    let block = projection
        .head_block_id
        .map(|block_id| block_id.to_string())
        .unwrap_or_else(|| "none".to_string());
    let rotation = projection
        .head_rotation
        .map(|rotation| rotation.to_string())
        .unwrap_or_else(|| "none".to_string());
    format!("{stage}@{tile}:{mode}:b{block}:r{rotation}")
}

fn runtime_building_table_label(projection: &BuildingTableProjection) -> String {
    let update = match projection.last_update {
        Some(BuildingProjectionUpdateKind::WorldBaseline) => "bootstrap",
        Some(BuildingProjectionUpdateKind::BlockSnapshotHead) => "head",
        Some(BuildingProjectionUpdateKind::ConstructFinish) => "construct",
        Some(BuildingProjectionUpdateKind::TileConfig) => "config",
        Some(BuildingProjectionUpdateKind::DeconstructFinish) => "deconstruct",
        Some(BuildingProjectionUpdateKind::BuildHealthUpdate) => "health",
        None => "none",
    };
    let tile = projection
        .last_build_pos
        .map(|build_pos| {
            let (x, y) = unpack_runtime_point2(build_pos);
            format!("{x}:{y}")
        })
        .unwrap_or_else(|| "none".to_string());
    let block = projection
        .last_block_id
        .map(|block_id| block_id.to_string())
        .unwrap_or_else(|| "none".to_string());
    format!(
        "{}:b{}:c{}:{}@{}#{}:rm{}:on{}:e{}:oe{}:v{}:m{}:vf{}:trb{}",
        projection.by_build_pos.len(),
        projection.block_known_count,
        projection.configured_count,
        update,
        tile,
        block,
        if projection.last_removed { 1 } else { 0 },
        projection
            .last_enabled
            .map(|enabled| if enabled { 1 } else { 0 })
            .unwrap_or(-1),
        projection.last_efficiency.map(i32::from).unwrap_or(-1),
        projection
            .last_optional_efficiency
            .map(i32::from)
            .unwrap_or(-1),
        projection.last_io_version.map(i32::from).unwrap_or(-1),
        projection.last_module_bitmask.map(i32::from).unwrap_or(-1),
        projection
            .last_visible_flags
            .map(|flags| flags.to_string())
            .unwrap_or_else(|| "-1".to_string()),
        projection
            .last_build_turret_rotation_bits
            .map(|bits| format!("0x{bits:08x}"))
            .unwrap_or_else(|| "none".to_string()),
    )
}

fn runtime_tile_config_business_label(projection: &TileConfigProjection) -> String {
    let source = match projection.last_business_source {
        Some(TileConfigAuthoritySource::TileConfigPacket) => "packet",
        Some(TileConfigAuthoritySource::ConstructFinish) => "construct",
        None => "none",
    };
    let tile = projection
        .last_business_build_pos
        .map(|build_pos| {
            let (x, y) = unpack_runtime_point2(build_pos);
            format!("{x}:{y}")
        })
        .unwrap_or_else(|| "none".to_string());
    let pending_match = match projection.last_pending_local_match {
        Some(true) => 1,
        Some(false) => 0,
        None => -1,
    };
    let configured_outcome = projection
        .last_configured_block_outcome
        .map(ConfiguredBlockOutcome::as_str)
        .unwrap_or("none");
    let configured_block = projection
        .last_configured_block_name
        .as_deref()
        .unwrap_or("none");
    format!(
        "a{}:p{}:c{}:ca{}:cr{}:{}@{}:cl{}:rb{}:pm{}:co{}:cb{}",
        projection.applied_authoritative_count,
        projection.applied_tile_config_packet_count,
        projection.applied_construct_finish_count,
        projection.configured_applied_count,
        projection.configured_rejected_count,
        source,
        tile,
        if projection.last_cleared_pending_local {
            1
        } else {
            0
        },
        if projection.last_was_rollback { 1 } else { 0 },
        pending_match,
        configured_outcome,
        configured_block,
    )
}

fn runtime_local_entity_label(session_state: &SessionState) -> String {
    let Some(entity_id) = session_state.entity_table_projection.local_player_entity_id else {
        return "none".to_string();
    };
    let Some(entity) = session_state
        .entity_table_projection
        .by_entity_id
        .get(&entity_id)
    else {
        return format!("{entity_id}:missing");
    };
    format!(
        "{}:c{}:u{}:{}:h{}",
        entity_id,
        entity.class_id,
        entity.unit_kind,
        entity.unit_value,
        if entity.hidden { 1 } else { 0 },
    )
}

fn runtime_entity_gate_label(session_state: &SessionState) -> String {
    let skip_count = session_state.entity_snapshot_tombstone_skip_count;
    let active_tombstones = session_state.entity_snapshot_tombstones.len();
    let sample = &session_state.last_entity_snapshot_tombstone_skipped_ids_sample;
    if sample.is_empty() {
        return format!("ts{skip_count}:a{active_tombstones}");
    }
    let joined = sample
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let sample_len = u64::try_from(sample.len()).unwrap_or(u64::MAX);
    let remaining = skip_count.saturating_sub(sample_len);
    if remaining > 0 {
        format!("ts{skip_count}@{joined}+{remaining}:a{active_tombstones}")
    } else {
        format!("ts{skip_count}@{joined}:a{active_tombstones}")
    }
}

fn runtime_entity_sync_label(session_state: &SessionState) -> String {
    format!(
        "lt{}:tp{}:ok{}:amb{}@{}:miss{}:fail{}",
        session_state.entity_snapshot_with_local_target_count,
        runtime_optional_display_label(session_state.last_entity_snapshot_target_player_id),
        u8::from(session_state.last_entity_snapshot_local_player_sync_applied),
        u8::from(session_state.last_entity_snapshot_local_player_sync_ambiguous),
        session_state.last_entity_snapshot_local_player_sync_match_count,
        session_state.missed_local_player_sync_from_entity_snapshot_count,
        session_state.failed_entity_snapshot_parse_count,
    )
}

fn format_hidden_snapshot_delta_side(
    direction: char,
    projection: &HiddenSnapshotDeltaProjection,
) -> String {
    let (count, sample_ids) = if direction == '+' {
        (
            projection.added_count,
            projection.added_sample_ids.as_slice(),
        )
    } else {
        (
            projection.removed_count,
            projection.removed_sample_ids.as_slice(),
        )
    };
    if sample_ids.is_empty() {
        return format!("{direction}{count}");
    }
    let joined = sample_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let remaining = count.saturating_sub(sample_ids.len());
    if remaining > 0 {
        format!("{direction}{count}@{joined}+{remaining}")
    } else {
        format!("{direction}{count}@{joined}")
    }
}

fn runtime_effect_data_kind_label(data_kind: Option<&str>) -> String {
    data_kind
        .filter(|kind| !kind.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_effect_contract_label(session_state: &SessionState) -> String {
    let last = session_state
        .last_effect_contract_name
        .as_deref()
        .unwrap_or("none");
    let reliable = session_state
        .last_effect_reliable_contract_name
        .as_deref()
        .unwrap_or("none");
    format!("{last}/{reliable}")
}

fn runtime_effect_data_semantic_label(semantic: Option<&EffectDataSemantic>) -> String {
    match semantic {
        Some(EffectDataSemantic::Null) => "null".to_string(),
        Some(EffectDataSemantic::Int(value)) => format!("int:{value}"),
        Some(EffectDataSemantic::Long(value)) => format!("long:{value}"),
        Some(EffectDataSemantic::FloatBits(bits)) => format!("floatBits:0x{bits:08x}"),
        Some(EffectDataSemantic::String(Some(value))) => format!("string:len{}", value.len()),
        Some(EffectDataSemantic::String(None)) => "string:none".to_string(),
        Some(EffectDataSemantic::ContentRaw {
            content_type,
            content_id,
        }) => format!("contentRaw:{content_type}:{content_id}"),
        Some(EffectDataSemantic::IntSeqLen(len)) => format!("intSeq:len{len}"),
        Some(EffectDataSemantic::Point2 { x, y }) => format!("point2:{x}:{y}"),
        Some(EffectDataSemantic::PackedPoint2ArrayLen(len)) => {
            format!("packedPoint2Array:len{len}")
        }
        Some(EffectDataSemantic::TechNodeRaw {
            content_type,
            content_id,
        }) => format!("techNodeRaw:{content_type}:{content_id}"),
        Some(EffectDataSemantic::Bool(value)) => format!("bool:{}", if *value { 1 } else { 0 }),
        Some(EffectDataSemantic::DoubleBits(bits)) => format!("doubleBits:0x{bits:016x}"),
        Some(EffectDataSemantic::BuildingPos(value)) => format!("buildingPos:{value}"),
        Some(EffectDataSemantic::LAccess(value)) => format!("lAccess:{value}"),
        Some(EffectDataSemantic::BytesLen(len)) => format!("bytes:len{len}"),
        Some(EffectDataSemantic::LegacyUnitCommandNull(value)) => {
            format!("legacyUnitCommandNull:0x{value:02x}")
        }
        Some(EffectDataSemantic::BoolArrayLen(len)) => format!("boolArray:len{len}"),
        Some(EffectDataSemantic::UnitId(value)) => format!("unitId:{value}"),
        Some(EffectDataSemantic::Vec2ArrayLen(len)) => format!("vec2Array:len{len}"),
        Some(EffectDataSemantic::Vec2 { x_bits, y_bits }) => {
            format!("vec2:0x{x_bits:08x}:0x{y_bits:08x}")
        }
        Some(EffectDataSemantic::Team(id)) => format!("team:{id}"),
        Some(EffectDataSemantic::IntArrayLen(len)) => format!("intArray:len{len}"),
        Some(EffectDataSemantic::ObjectArrayLen(len)) => format!("objectArray:len{len}"),
        Some(EffectDataSemantic::UnitCommand(id)) => format!("unitCommand:{id}"),
        Some(EffectDataSemantic::OpaqueTypeTag(tag)) => format!("opaqueTypeTag:0x{tag:02x}"),
        None => "none".to_string(),
    }
}

fn runtime_effect_business_projection_label(
    projection: Option<&EffectBusinessProjection>,
) -> String {
    match projection {
        Some(EffectBusinessProjection::ContentRef {
            kind,
            content_type,
            content_id,
        }) => {
            let kind = match kind {
                EffectBusinessContentKind::Content => "content",
                EffectBusinessContentKind::TechNode => "techNode",
            };
            format!("content:{kind}:{content_type}:{content_id}")
        }
        Some(EffectBusinessProjection::ParentRef {
            source,
            value,
            x_bits,
            y_bits,
        }) => {
            let source = match source {
                EffectBusinessPositionSource::BuildingPos => "build",
                EffectBusinessPositionSource::Point2 => "point2",
                EffectBusinessPositionSource::Vec2 => "vec2",
                EffectBusinessPositionSource::EntityUnitId => "entityUnit",
                EffectBusinessPositionSource::LocalUnitId => "localUnit",
            };
            format!(
                "parent:{source}:0x{:08x}:0x{x_bits:08x}:0x{y_bits:08x}",
                *value as u32
            )
        }
        Some(EffectBusinessProjection::WorldPosition {
            source,
            x_bits,
            y_bits,
        }) => {
            let source = match source {
                EffectBusinessPositionSource::BuildingPos => "build",
                EffectBusinessPositionSource::Point2 => "point2",
                EffectBusinessPositionSource::Vec2 => "vec2",
                EffectBusinessPositionSource::EntityUnitId => "entityUnit",
                EffectBusinessPositionSource::LocalUnitId => "localUnit",
            };
            format!("pos:{source}:0x{x_bits:08x}:0x{y_bits:08x}")
        }
        Some(EffectBusinessProjection::FloatValue(bits)) => {
            format!("floatBits:0x{bits:08x}")
        }
        None => "none".to_string(),
    }
}

fn runtime_effect_data_fail_label(session_state: &SessionState) -> String {
    format!(
        "{}@{}",
        session_state.failed_effect_data_parse_count,
        runtime_effect_data_error_label(session_state.last_effect_data_parse_error.as_deref()),
    )
}

fn runtime_effect_data_error_label(error: Option<&str>) -> String {
    match error {
        Some(error) if error.contains("trailing bytes after effect data object") => {
            "trail".to_string()
        }
        Some(error) if error.contains("failed to parse effect data object") => "decode".to_string(),
        Some(error) if error.contains("truncated") => "trunc".to_string(),
        Some(error) => runtime_compact_text_label(Some(error)),
        None => "none".to_string(),
    }
}

fn runtime_ui_observability(session_state: &SessionState) -> RuntimeUiObservability {
    RuntimeUiObservability {
        hud_text: RuntimeHudTextObservability {
            set_count: session_state.received_set_hud_text_count,
            set_reliable_count: session_state.received_set_hud_text_reliable_count,
            hide_count: session_state.received_hide_hud_text_count,
            last_message: session_state.last_set_hud_text_message.clone(),
            last_reliable_message: session_state.last_set_hud_text_reliable_message.clone(),
        },
        toast: RuntimeToastObservability {
            info_count: session_state.received_info_toast_count,
            warning_count: session_state.received_warning_toast_count,
            last_info_message: session_state.last_info_toast_message.clone(),
            last_warning_text: session_state.last_warning_toast_text.clone(),
        },
        text_input: RuntimeTextInputObservability {
            open_count: session_state.received_text_input_count,
            last_id: session_state.last_text_input_id,
            last_title: session_state.last_text_input_title.clone(),
            last_message: session_state.last_text_input_message.clone(),
            last_default_text: session_state.last_text_input_default_text.clone(),
            last_length: session_state.last_text_input_length,
            last_numeric: session_state.last_text_input_numeric,
            last_allow_empty: session_state.last_text_input_allow_empty,
        },
    }
}

fn runtime_ui_notice_label(session_state: &SessionState) -> String {
    let last_clipboard = session_state.last_copy_to_clipboard_text.as_deref();
    let last_uri = session_state.last_open_uri.as_deref();
    format!(
        "hud{}:hudr{}:hide{}:ann{}:info{}:toast{}:warn{}:popup{}:popr{}:clip{}@{}#{}:uri{}@{}#{}:{}",
        session_state.received_set_hud_text_count,
        session_state.received_set_hud_text_reliable_count,
        session_state.received_hide_hud_text_count,
        session_state.received_announce_count,
        session_state.received_info_message_count,
        session_state.received_info_toast_count,
        session_state.received_warning_toast_count,
        session_state.received_info_popup_count,
        session_state.received_info_popup_reliable_count,
        session_state.received_copy_to_clipboard_count,
        runtime_compact_text_label(last_clipboard),
        runtime_compact_text_len_label(last_clipboard),
        session_state.received_open_uri_count,
        runtime_compact_text_label(last_uri),
        runtime_compact_text_len_label(last_uri),
        runtime_uri_scheme_label(last_uri),
    )
}

fn runtime_audio_label(session_state: &SessionState) -> String {
    format!(
        "snd{}@{}:sf{}:sat{}@{}:saf{}",
        session_state.received_sound_count,
        runtime_optional_display_label(session_state.last_sound_id),
        session_state.failed_sound_parse_count,
        session_state.received_sound_at_count,
        runtime_optional_display_label(session_state.last_sound_at_id),
        session_state.failed_sound_at_parse_count,
    )
}

fn runtime_spawn_fx_label(session_state: &SessionState) -> String {
    format!(
        "cw{}@{}:se{}@{}:lx{}@{}:{}{}{}{}:us{}@{}/{}#{}:ubs{}@{}:utbs{}@{}#{}",
        session_state.received_create_weather_count,
        runtime_optional_display_label(session_state.last_create_weather_id),
        session_state.received_spawn_effect_count,
        runtime_optional_display_label(session_state.last_spawn_effect_unit_type_id),
        session_state.received_logic_explosion_count,
        runtime_optional_display_label(session_state.last_logic_explosion_team_id),
        runtime_optional_bool_label(session_state.last_logic_explosion_air),
        runtime_optional_bool_label(session_state.last_logic_explosion_ground),
        runtime_optional_bool_label(session_state.last_logic_explosion_pierce),
        runtime_optional_bool_label(session_state.last_logic_explosion_effect),
        session_state.received_unit_spawn_count,
        runtime_optional_display_label(session_state.last_unit_spawn_id),
        runtime_optional_display_label(session_state.last_unit_spawn_class_id),
        runtime_optional_display_label(session_state.last_unit_spawn_trailing_bytes),
        session_state.received_unit_block_spawn_count,
        runtime_optional_display_label(session_state.last_unit_block_spawn_tile_pos),
        session_state.received_unit_tether_block_spawned_count,
        runtime_optional_display_label(session_state.last_unit_tether_block_spawned_tile_pos),
        runtime_optional_display_label(session_state.last_unit_tether_block_spawned_id),
    )
}

fn runtime_admin_label(session_state: &SessionState) -> String {
    format!(
        "trace{}@{}:tf{}:dbgr{}:drf{}:dbgu{}@{}:duf{}",
        session_state.received_trace_info_count,
        runtime_optional_display_label(session_state.last_trace_info_player_id),
        session_state.failed_trace_info_parse_count,
        session_state.received_debug_status_client_count,
        session_state.failed_debug_status_client_parse_count,
        session_state.received_debug_status_client_unreliable_count,
        runtime_optional_display_label(session_state.last_debug_status_value),
        session_state.failed_debug_status_client_unreliable_parse_count,
    )
}

fn runtime_kick_label(world_overlay: &RuntimeWorldOverlay) -> String {
    format!(
        "{}@{}:{}:{}",
        runtime_compact_text_label(world_overlay.last_kick_reason_text.as_deref()),
        runtime_optional_display_label(world_overlay.last_kick_reason_ordinal),
        world_overlay.last_kick_hint_category.unwrap_or("none"),
        runtime_compact_text_label(world_overlay.last_kick_hint_text),
    )
}

fn runtime_kick_hint_from(
    reason_text: Option<&str>,
    reason_ordinal: Option<i32>,
) -> Option<(Option<&'static str>, Option<&'static str>)> {
    let normalized = match reason_text {
        Some("banned") => Some("banned"),
        Some("clientOutdated") => Some("clientOutdated"),
        Some("recentKick") => Some("recentKick"),
        Some("nameInUse") => Some("nameInUse"),
        Some("idInUse") => Some("idInUse"),
        Some("nameEmpty") => Some("nameEmpty"),
        Some("serverOutdated") => Some("serverOutdated"),
        Some("customClient") => Some("customClient"),
        Some("typeMismatch") => Some("typeMismatch"),
        Some("whitelist") => Some("whitelist"),
        Some("playerLimit") => Some("playerLimit"),
        Some("serverRestarting") => Some("serverRestarting"),
        _ => reason_ordinal.and_then(runtime_kick_reason_name_from_ordinal),
    };

    match normalized {
        Some("banned") => Some((
            Some("Banned"),
            Some(
                "server reports this identity or name is banned; use a different account or ask the server admin to review the ban.",
            ),
        )),
        Some("clientOutdated") => Some((
            Some("ClientOutdated"),
            Some("client build is outdated; upgrade this client to the server version."),
        )),
        Some("recentKick") => Some((
            Some("RecentKick"),
            Some(
                "server still remembers a recent kick; wait for the cooldown to expire before reconnecting.",
            ),
        )),
        Some("nameInUse") => Some((
            Some("NameInUse"),
            Some("player name is already in use; retry with a different --name value."),
        )),
        Some("idInUse") => Some((
            Some("IdInUse"),
            Some(
                "uuid or usid is already in use; wait for the old session to clear or regenerate the connect identity.",
            ),
        )),
        Some("nameEmpty") => Some((
            Some("NameEmpty"),
            Some(
                "player name is empty or invalid; set --name to a non-empty value accepted by the server.",
            ),
        )),
        Some("serverOutdated") => Some((
            Some("ServerOutdated"),
            Some(
                "server build is older than this client; use a matching server or older client build.",
            ),
        )),
        Some("customClient") => Some((
            Some("CustomClientRejected"),
            Some(
                "server rejected custom clients; connect to a server that allows custom clients.",
            ),
        )),
        Some("typeMismatch") => Some((
            Some("TypeMismatch"),
            Some("version type/protocol mismatch; align client/server version type and mod set."),
        )),
        Some("whitelist") => Some((
            Some("WhitelistRequired"),
            Some("server requires whitelist access; ask the server admin to whitelist this identity."),
        )),
        Some("playerLimit") => Some((
            Some("PlayerLimit"),
            Some("server is full; wait for an open slot or use an identity with reserved access."),
        )),
        Some("serverRestarting") => Some((
            Some("ServerRestarting"),
            Some("server is restarting; retry connection shortly."),
        )),
        _ => None,
    }
}

fn runtime_kick_reason_name_from_ordinal(reason_ordinal: i32) -> Option<&'static str> {
    match reason_ordinal {
        3 => Some("banned"),
        1 => Some("clientOutdated"),
        2 => Some("serverOutdated"),
        5 => Some("recentKick"),
        6 => Some("nameInUse"),
        7 => Some("idInUse"),
        8 => Some("nameEmpty"),
        9 => Some("customClient"),
        12 => Some("typeMismatch"),
        13 => Some("whitelist"),
        14 => Some("playerLimit"),
        15 => Some("serverRestarting"),
        _ => None,
    }
}

fn runtime_loading_label(session_state: &SessionState) -> String {
    format!(
        "defer{}:replay{}:drop{}:qdrop{}:sfail{}:scfail{}:efail{}:rdy{}@{}:to{}:cto{}:rto{}:lt{}@{}:rs{}:rr{}:wr{}:kr{}:lr{}:lwr{}",
        session_state.deferred_inbound_packet_count,
        session_state.replayed_inbound_packet_count,
        session_state.dropped_loading_low_priority_packet_count,
        session_state.dropped_loading_deferred_overflow_count,
        session_state.failed_state_snapshot_parse_count,
        session_state.failed_state_snapshot_core_data_parse_count,
        session_state.failed_entity_snapshot_parse_count,
        session_state.ready_inbound_liveness_anchor_count,
        runtime_optional_display_label(session_state.last_ready_inbound_liveness_anchor_at_ms),
        session_state.timeout_count,
        session_state.connect_or_loading_timeout_count,
        session_state.ready_snapshot_timeout_count,
        runtime_timeout_kind_label(session_state.last_timeout.as_ref().map(|timeout| timeout.kind)),
        runtime_optional_display_label(session_state.last_timeout.as_ref().map(|timeout| timeout.idle_ms)),
        session_state.reset_count,
        session_state.reconnect_reset_count,
        session_state.world_reload_count,
        session_state.kick_reset_count,
        runtime_reset_kind_label(session_state.last_reset_kind),
        runtime_world_reload_label(session_state.last_world_reload.as_ref()),
    )
}

fn runtime_timeout_kind_label(kind: Option<SessionTimeoutKind>) -> &'static str {
    match kind {
        Some(SessionTimeoutKind::ConnectOrLoading) => "cload",
        Some(SessionTimeoutKind::ReadySnapshotStall) => "ready",
        None => "none",
    }
}

fn runtime_reset_kind_label(kind: Option<SessionResetKind>) -> &'static str {
    match kind {
        Some(SessionResetKind::Reconnect) => "reconnect",
        Some(SessionResetKind::WorldReload) => "reload",
        Some(SessionResetKind::Kick) => "kick",
        None => "none",
    }
}

fn runtime_world_reload_label(projection: Option<&WorldReloadProjection>) -> String {
    match projection {
        Some(projection) => format!(
            "@lw{}:cl{}:rd{}:cc{}:p{}:d{}:r{}",
            if projection.had_loaded_world { 1 } else { 0 },
            if projection.had_client_loaded { 1 } else { 0 },
            if projection.was_ready_to_enter_world {
                1
            } else {
                0
            },
            if projection.had_connect_confirm_sent {
                1
            } else {
                0
            },
            projection.cleared_pending_packets,
            projection.cleared_deferred_inbound_packets,
            projection.cleared_replayed_loading_events,
        ),
        None => "none".to_string(),
    }
}

fn runtime_rules_label(session_state: &SessionState) -> String {
    format!(
        "sr{}:srf{}:so{}:sof{}:rule{}:rf{}:clr{}:cmp{}:wv{}:pvp{}:obj{}:q{}:par{}:fg{}:oor{}:last{}",
        session_state.received_set_rules_count,
        session_state.failed_set_rules_parse_count,
        session_state.received_set_objectives_count,
        session_state.failed_set_objectives_parse_count,
        session_state.received_set_rule_count,
        session_state.failed_set_rule_parse_count,
        session_state.received_clear_objectives_count,
        session_state.received_complete_objective_count,
        runtime_optional_bool_label(session_state.rules_projection.waves),
        runtime_optional_bool_label(session_state.rules_projection.pvp),
        session_state.objectives_projection.objectives.len(),
        session_state.objectives_projection.qualified_count(),
        session_state.objectives_projection.parent_edge_count(),
        session_state.objectives_projection.objective_flags.len(),
        session_state
            .objectives_projection
            .complete_out_of_range_count,
        session_state
            .objectives_projection
            .last_completed_index
            .unwrap_or(-1),
    )
}

fn runtime_ui_menu_label(session_state: &SessionState) -> String {
    format!(
        "menu{}:fmenu{}:hfm{}:tin{}@{}:{}:{}#{}:n{}:e{}",
        session_state.received_menu_open_count,
        session_state.received_follow_up_menu_open_count,
        session_state.received_hide_follow_up_menu_count,
        session_state.received_text_input_count,
        runtime_optional_display_label(session_state.last_text_input_id),
        runtime_compact_text_label(session_state.last_text_input_title.as_deref()),
        runtime_compact_text_label(session_state.last_text_input_default_text.as_deref()),
        session_state.last_text_input_length.unwrap_or_default(),
        session_state.last_text_input_numeric.unwrap_or(false) as u8,
        session_state.last_text_input_allow_empty.unwrap_or(false) as u8,
    )
}

fn runtime_compact_text_len_label(value: Option<&str>) -> usize {
    value.map(|text| text.chars().count()).unwrap_or_default()
}

fn runtime_uri_scheme_label(value: Option<&str>) -> String {
    match value
        .and_then(|uri| uri.split_once(':').map(|(scheme, _)| scheme))
        .filter(|scheme| !scheme.is_empty())
    {
        Some(scheme) => runtime_compact_text_label(Some(scheme)),
        None => "none".to_string(),
    }
}

fn runtime_compact_text_label(value: Option<&str>) -> String {
    match value {
        Some(value) => {
            let mut compact = String::new();
            for (index, ch) in value.chars().enumerate() {
                if index == 12 {
                    compact.push('~');
                    break;
                }
                compact.push(match ch {
                    ':' | ' ' | '\t' | '\r' | '\n' => '_',
                    _ => ch,
                });
            }
            if compact.is_empty() {
                "-".to_string()
            } else {
                compact
            }
        }
        None => "none".to_string(),
    }
}

fn runtime_optional_bool_label(value: Option<bool>) -> char {
    match value {
        Some(true) => '1',
        Some(false) => '0',
        None => 'n',
    }
}

fn runtime_optional_display_label<T: fmt::Display + Copy>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_optional_runtime_point2_label(value: Option<i32>) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| {
            let (x, y) = unpack_runtime_point2(value);
            format!("{x}:{y}")
        },
    )
}

fn runtime_optional_bits_label(value: Option<u32>) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| format!("0x{value:08x}"),
    )
}

fn runtime_optional_bits_pair_label(x_bits: Option<u32>, y_bits: Option<u32>) -> String {
    match (x_bits, y_bits) {
        (Some(x_bits), Some(y_bits)) => format!("0x{x_bits:08x}:0x{y_bits:08x}"),
        _ => "none".to_string(),
    }
}

fn runtime_optional_text_len_label(value: Option<&str>) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| format!("len{}", value.len()),
    )
}

fn runtime_optional_unit_ref_label(value: Option<UnitRefProjection>) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| format!("{}:{}", value.kind, value.value),
    )
}

fn runtime_world_label_label(session_state: &SessionState) -> String {
    format!(
        "lbl{}:lblr{}:rml{}",
        session_state.received_world_label_count,
        session_state.received_world_label_reliable_count,
        session_state.received_remove_world_label_count,
    )
}

fn runtime_marker_label(session_state: &SessionState) -> String {
    format!(
        "cr{}:rm{}:up{}:txt{}:tex{}:fail{}:last{}:{}",
        session_state.received_create_marker_count,
        session_state.received_remove_marker_count,
        session_state.received_update_marker_count,
        session_state.received_update_marker_text_count,
        session_state.received_update_marker_texture_count,
        session_state.failed_marker_decode_count,
        runtime_optional_display_label(session_state.last_marker_id),
        session_state
            .last_marker_control_name
            .as_deref()
            .unwrap_or("none"),
    )
}

fn runtime_logic_sync_label(session_state: &SessionState) -> String {
    format!(
        "ov{}@{}:{}:{}:sv{}@{}:{}:{}",
        session_state.received_set_tile_overlays_count,
        runtime_optional_display_label(session_state.last_set_tile_overlays_block_id),
        session_state.last_set_tile_overlays_count,
        runtime_optional_display_label(session_state.last_set_tile_overlays_first_position),
        session_state.received_sync_variable_count,
        runtime_optional_display_label(session_state.last_sync_variable_build_pos),
        runtime_optional_display_label(session_state.last_sync_variable_index),
        session_state
            .last_sync_variable_value_kind_name
            .as_deref()
            .unwrap_or("none"),
    )
}

fn runtime_unit_lifecycle_label(session_state: &SessionState) -> String {
    format!(
        "bd{}@{}:ud{}@{}:ux{}@{}:uy{}@{}:us{}@{}:uc{}@{}",
        session_state.received_build_destroyed_count,
        runtime_optional_display_label(session_state.last_build_destroyed_build_pos),
        session_state.received_unit_death_count,
        runtime_optional_display_label(session_state.last_unit_death_id),
        session_state.received_unit_destroy_count,
        runtime_optional_display_label(session_state.last_unit_destroy_id),
        session_state.received_unit_env_death_count,
        runtime_optional_unit_ref_label(session_state.last_unit_env_death),
        session_state.received_unit_safe_death_count,
        runtime_optional_unit_ref_label(session_state.last_unit_safe_death),
        session_state.received_unit_cap_death_count,
        runtime_optional_unit_ref_label(session_state.last_unit_cap_death),
    )
}

fn runtime_resource_delta_label(session_state: &SessionState) -> String {
    format!(
        "rmt{}:st{}:sf{}:so{}:seti{}:setis{}:setl{}:setls{}:cli{}:cll{}:sti{}:stl{}:tk{}:tb{}:tu{}:{}@{}#{}:bp{}:u{}:eid{}:b{}:bs{}:e{}:au{}:da{}:sk{}:cf{}:lb{}:le{}:li{}:la{}",
        session_state.received_remove_tile_count,
        session_state.received_set_tile_count,
        session_state.received_set_floor_count,
        session_state.received_set_overlay_count,
        session_state.received_set_item_count,
        session_state.received_set_items_count,
        session_state.received_set_liquid_count,
        session_state.received_set_liquids_count,
        session_state.received_clear_items_count,
        session_state.received_clear_liquids_count,
        session_state.received_set_tile_items_count,
        session_state.received_set_tile_liquids_count,
        session_state.resource_delta_projection.take_items_count,
        session_state.resource_delta_projection.transfer_item_to_count,
        session_state.resource_delta_projection.transfer_item_to_unit_count,
        session_state
            .resource_delta_projection
            .last_kind
            .unwrap_or("none"),
        runtime_optional_display_label(session_state.resource_delta_projection.last_item_id),
        runtime_optional_display_label(session_state.resource_delta_projection.last_amount),
        runtime_optional_display_label(session_state.resource_delta_projection.last_build_pos),
        runtime_optional_unit_ref_label(session_state.resource_delta_projection.last_unit),
        runtime_optional_display_label(session_state.resource_delta_projection.last_to_entity_id),
        session_state.resource_delta_projection.build_count(),
        session_state.resource_delta_projection.build_stack_count(),
        session_state.resource_delta_projection.entity_count(),
        session_state
            .resource_delta_projection
            .authoritative_build_update_count,
        session_state.resource_delta_projection.delta_apply_count,
        session_state.resource_delta_projection.delta_skip_count,
        session_state.resource_delta_projection.delta_conflict_count,
        runtime_optional_display_label(session_state.resource_delta_projection.last_changed_build_pos),
        runtime_optional_display_label(session_state.resource_delta_projection.last_changed_entity_id),
        runtime_optional_display_label(session_state.resource_delta_projection.last_changed_item_id),
        runtime_optional_display_label(session_state.resource_delta_projection.last_changed_amount),
    )
}

fn runtime_command_control_label(session_state: &SessionState) -> String {
    format!(
        "spte{}@t{}:mc{}@{}/{}:tir{}@{}#{}:ri{}@{}#{}x{}:bcs{}@{}:ucl{}:uct{}@{}:ubcs{}@{}/{}:cb{}@n{}:{}->{}:cu{}@n{}:u{}:b{}:t{}:p{}:q{}:f{}:suc{}@n{}:u{}:c{}:sus{}@n{}:u{}:s{}:e{}:rot{}@{}:d{}:tinv{}@{}:rbp{}@{}:rdp{}@{}:rup{}@{}:drop{}@{}:dpl{}@n{}:{}:tap{}@{}",
        session_state.received_set_player_team_editor_count,
        runtime_optional_display_label(session_state.last_set_player_team_editor_team_id),
        session_state.received_menu_choose_count,
        runtime_optional_display_label(session_state.last_menu_choose_menu_id),
        runtime_optional_display_label(session_state.last_menu_choose_option),
        session_state.received_text_input_result_count,
        runtime_optional_display_label(session_state.last_text_input_result_id),
        runtime_optional_text_len_label(session_state.last_text_input_result_text.as_deref()),
        session_state.received_request_item_count,
        runtime_optional_runtime_point2_label(session_state.last_request_item_build_pos),
        runtime_optional_display_label(session_state.last_request_item_item_id),
        runtime_optional_display_label(session_state.last_request_item_amount),
        session_state.received_building_control_select_count,
        runtime_optional_runtime_point2_label(session_state.last_building_control_select_build_pos),
        session_state.received_unit_clear_count,
        session_state.received_unit_control_count,
        runtime_optional_unit_ref_label(session_state.last_unit_control_target),
        session_state.received_unit_building_control_select_count,
        runtime_optional_unit_ref_label(session_state.last_unit_building_control_select_target),
        runtime_optional_runtime_point2_label(
            session_state.last_unit_building_control_select_build_pos,
        ),
        session_state.received_command_building_count,
        session_state.last_command_building_count,
        runtime_optional_runtime_point2_label(session_state.last_command_building_first_build_pos),
        runtime_optional_bits_pair_label(
            session_state.last_command_building_x_bits,
            session_state.last_command_building_y_bits,
        ),
        session_state.received_command_units_count,
        session_state.last_command_units_count,
        runtime_optional_display_label(session_state.last_command_units_first_unit_id),
        runtime_optional_runtime_point2_label(session_state.last_command_units_build_target),
        runtime_optional_unit_ref_label(session_state.last_command_units_unit_target),
        runtime_optional_bits_pair_label(
            session_state.last_command_units_x_bits,
            session_state.last_command_units_y_bits,
        ),
        runtime_optional_bool_label(session_state.last_command_units_queue),
        runtime_optional_bool_label(session_state.last_command_units_final_batch),
        session_state.received_set_unit_command_count,
        session_state.last_set_unit_command_count,
        runtime_optional_display_label(session_state.last_set_unit_command_first_unit_id),
        runtime_optional_display_label(session_state.last_set_unit_command_id),
        session_state.received_set_unit_stance_count,
        session_state.last_set_unit_stance_count,
        runtime_optional_display_label(session_state.last_set_unit_stance_first_unit_id),
        runtime_optional_display_label(session_state.last_set_unit_stance_id),
        runtime_optional_bool_label(session_state.last_set_unit_stance_enable),
        session_state.received_rotate_block_count,
        runtime_optional_runtime_point2_label(session_state.last_rotate_block_build_pos),
        runtime_optional_bool_label(session_state.last_rotate_block_direction),
        session_state.received_transfer_inventory_count,
        runtime_optional_runtime_point2_label(session_state.last_transfer_inventory_build_pos),
        session_state.received_request_build_payload_count,
        runtime_optional_runtime_point2_label(session_state.last_request_build_payload_build_pos),
        session_state.received_request_drop_payload_count,
        runtime_optional_bits_pair_label(
            session_state.last_request_drop_payload_x_bits,
            session_state.last_request_drop_payload_y_bits,
        ),
        session_state.received_request_unit_payload_count,
        runtime_optional_unit_ref_label(session_state.last_request_unit_payload_target),
        session_state.received_drop_item_count,
        runtime_optional_bits_label(session_state.last_drop_item_angle_bits),
        session_state.received_delete_plans_count,
        session_state.last_delete_plans_count,
        runtime_optional_runtime_point2_label(session_state.last_delete_plans_first_pos),
        session_state.received_tile_tap_count,
        runtime_optional_runtime_point2_label(session_state.last_tile_tap_pos),
    )
}

fn runtime_gameplay_signal_label(session_state: &SessionState) -> String {
    format!(
        "flag{}:go{}:ugo{}:sc{}:res{}",
        session_state.received_set_flag_count,
        session_state.received_game_over_count,
        session_state.received_update_game_over_count,
        session_state.received_sector_capture_count,
        session_state.received_researched_count,
    )
}

fn runtime_effect_path_label(path: Option<&[usize]>) -> String {
    match path {
        Some(path) if !path.is_empty() => path
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        _ => "none".to_string(),
    }
}

fn runtime_bootstrap_hash_label<F>(
    projection: Option<&WorldBootstrapProjection>,
    selector: F,
) -> String
where
    F: Fn(&WorldBootstrapProjection) -> &str,
{
    projection
        .map(selector)
        .filter(|hash| !hash.is_empty())
        .map(|hash| hash.chars().take(8).collect())
        .unwrap_or_else(|| "none".to_string())
}

pub fn observe_build_health_pairs(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    pairs: &[BuildHealthPair],
) {
    for pair in pairs {
        let entry = runtime_world_overlay
            .tile_overlays
            .entry(unpack_runtime_point2(pair.build_pos))
            .or_insert(RuntimeTileOverlay {
                kind: RuntimeTileOverlayKind::HealthUpdated,
                block_id: None,
                health_bits: Some(pair.health_bits),
                config_kind_name: None,
                parse_failed: false,
                business_applied: true,
                pending_local_match: None,
                rollback: false,
            });
        if entry.kind != RuntimeTileOverlayKind::Constructed {
            entry.kind = RuntimeTileOverlayKind::HealthUpdated;
        }
        entry.health_bits = Some(pair.health_bits);
    }
}

pub fn pack_runtime_point2(x: i32, y: i32) -> i32 {
    ((x as i16 as u16 as u32) << 16 | (y as i16 as u16 as u32)) as i32
}

pub fn unpack_runtime_point2(value: i32) -> (i32, i32) {
    let raw = value as u32;
    let x = ((raw >> 16) as u16) as i16;
    let y = (raw as u16) as i16;
    (i32::from(x), i32::from(y))
}

fn append_runtime_build_plan_objects(scene: &mut RenderModel, plans: Option<&[ClientBuildPlan]>) {
    const TILE_SIZE: f32 = 8.0;

    let Some(plans) = plans else {
        return;
    };

    for (index, plan) in plans.iter().enumerate() {
        scene.objects.push(RenderObject {
            id: runtime_build_plan_object_id(index, plan),
            layer: if plan.breaking { 31 } else { 21 },
            x: plan.tile.0 as f32 * TILE_SIZE,
            y: plan.tile.1 as f32 * TILE_SIZE,
        });
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct RuntimeBuildPlanConfigStats {
    int: usize,
    long: usize,
    float: usize,
    bool: usize,
    int_seq: usize,
    point2: usize,
    point2_array: usize,
    tech_node: usize,
    double: usize,
    building_pos: usize,
    laccess: usize,
    string: usize,
    bytes: usize,
    legacy_unit_command_null: usize,
    bool_array: usize,
    unit_id: usize,
    vec2_array: usize,
    vec2: usize,
    team: usize,
    int_array: usize,
    object_array: usize,
    content: usize,
    unit_command: usize,
}

fn runtime_build_plan_config_stats(
    plans: Option<&[ClientBuildPlan]>,
) -> RuntimeBuildPlanConfigStats {
    let mut stats = RuntimeBuildPlanConfigStats::default();
    let Some(plans) = plans else {
        return stats;
    };

    for plan in plans {
        match &plan.config {
            ClientBuildPlanConfig::Int(_) => stats.int = stats.int.saturating_add(1),
            ClientBuildPlanConfig::Long(_) => stats.long = stats.long.saturating_add(1),
            ClientBuildPlanConfig::FloatBits(_) => stats.float = stats.float.saturating_add(1),
            ClientBuildPlanConfig::Bool(_) => stats.bool = stats.bool.saturating_add(1),
            ClientBuildPlanConfig::IntSeq(_) => stats.int_seq = stats.int_seq.saturating_add(1),
            ClientBuildPlanConfig::Point2 { .. } => stats.point2 = stats.point2.saturating_add(1),
            ClientBuildPlanConfig::Point2Array(_) => {
                stats.point2_array = stats.point2_array.saturating_add(1)
            }
            ClientBuildPlanConfig::TechNodeRaw { .. } => {
                stats.tech_node = stats.tech_node.saturating_add(1)
            }
            ClientBuildPlanConfig::DoubleBits(_) => stats.double = stats.double.saturating_add(1),
            ClientBuildPlanConfig::BuildingPos(_) => {
                stats.building_pos = stats.building_pos.saturating_add(1)
            }
            ClientBuildPlanConfig::LAccess(_) => stats.laccess = stats.laccess.saturating_add(1),
            ClientBuildPlanConfig::String(_) => stats.string = stats.string.saturating_add(1),
            ClientBuildPlanConfig::Bytes(_) => stats.bytes = stats.bytes.saturating_add(1),
            ClientBuildPlanConfig::LegacyUnitCommandNull(_) => {
                stats.legacy_unit_command_null = stats.legacy_unit_command_null.saturating_add(1)
            }
            ClientBuildPlanConfig::BoolArray(_) => {
                stats.bool_array = stats.bool_array.saturating_add(1)
            }
            ClientBuildPlanConfig::UnitId(_) => stats.unit_id = stats.unit_id.saturating_add(1),
            ClientBuildPlanConfig::Vec2Array(_) => {
                stats.vec2_array = stats.vec2_array.saturating_add(1)
            }
            ClientBuildPlanConfig::Vec2 { .. } => stats.vec2 = stats.vec2.saturating_add(1),
            ClientBuildPlanConfig::Team(_) => stats.team = stats.team.saturating_add(1),
            ClientBuildPlanConfig::IntArray(_) => {
                stats.int_array = stats.int_array.saturating_add(1)
            }
            ClientBuildPlanConfig::ObjectArray(_) => {
                stats.object_array = stats.object_array.saturating_add(1)
            }
            ClientBuildPlanConfig::Content { .. } => {
                stats.content = stats.content.saturating_add(1)
            }
            ClientBuildPlanConfig::UnitCommand(_) => {
                stats.unit_command = stats.unit_command.saturating_add(1)
            }
            ClientBuildPlanConfig::None => {}
        }
    }

    stats
}

fn append_runtime_world_overlay_objects(
    scene: &mut RenderModel,
    runtime_world_overlay: &RuntimeWorldOverlay,
    snapshot_input: &ClientSnapshotInputState,
    session_state: &SessionState,
) {
    const TILE_SIZE: f32 = 8.0;

    for ((tile_x, tile_y), overlay) in &runtime_world_overlay.tile_overlays {
        let x = *tile_x as f32 * TILE_SIZE;
        let y = *tile_y as f32 * TILE_SIZE;
        match overlay.kind {
            RuntimeTileOverlayKind::Constructed => scene.objects.push(RenderObject {
                id: format!(
                    "block:runtime-construct:{}:{}:{}",
                    tile_x,
                    tile_y,
                    overlay.block_id.unwrap_or(-1)
                ),
                layer: 16,
                x,
                y,
            }),
            RuntimeTileOverlayKind::Deconstructed => scene.objects.push(RenderObject {
                id: format!("terrain:runtime-deconstruct:{}:{}", tile_x, tile_y),
                layer: 16,
                x,
                y,
            }),
            RuntimeTileOverlayKind::Configured => {
                let prefix = if overlay.parse_failed {
                    "marker:runtime-config-parse-fail"
                } else if !overlay.business_applied {
                    "marker:runtime-config-noapply"
                } else if overlay.rollback {
                    "marker:runtime-config-rollback"
                } else if matches!(overlay.pending_local_match, Some(false)) {
                    "marker:runtime-config-pending-mismatch"
                } else {
                    "marker:runtime-config"
                };
                let kind = overlay.config_kind_name.as_deref().unwrap_or("unknown");
                scene.objects.push(RenderObject {
                    id: format!("{prefix}:{tile_x}:{tile_y}:{kind}"),
                    layer: 24,
                    x,
                    y,
                });
            }
            RuntimeTileOverlayKind::HealthUpdated => {}
        }
        if overlay.health_bits.is_some() {
            scene.objects.push(RenderObject {
                id: format!("marker:runtime-health:{}:{}", tile_x, tile_y),
                layer: 32,
                x,
                y,
            });
        }
    }

    for overlay in &runtime_world_overlay.effect_overlays {
        let (x_bits, y_bits) =
            resolve_runtime_effect_overlay_position(overlay, session_state, snapshot_input);
        let reliable = if overlay.reliable {
            "reliable"
        } else {
            "normal"
        };
        let data = if overlay.has_data { 1 } else { 0 };
        scene.objects.push(RenderObject {
            id: format!(
                "marker:runtime-effect:{reliable}:{}:0x{:08x}:0x{:08x}:{}",
                overlay.effect_id.unwrap_or(-1),
                x_bits,
                y_bits,
                data
            ),
            layer: 26,
            x: f32::from_bits(x_bits),
            y: f32::from_bits(y_bits),
        });
    }
}

fn append_block_snapshot_projection_objects(scene: &mut RenderModel, session_state: &SessionState) {
    const TILE_SIZE: f32 = 8.0;

    let Some(projection) = session_state.block_snapshot_head_projection.as_ref() else {
        return;
    };
    let (tile_x, tile_y) = unpack_runtime_point2(projection.build_pos);
    scene.objects.push(RenderObject {
        id: format!(
            "block:runtime-snapshot-head:{}:{}:{}",
            tile_x, tile_y, projection.block_id
        ),
        layer: 15,
        x: tile_x as f32 * TILE_SIZE,
        y: tile_y as f32 * TILE_SIZE,
    });
}

fn append_building_table_projection_objects(scene: &mut RenderModel, session_state: &SessionState) {
    const TILE_SIZE: f32 = 8.0;

    for (&build_pos, building) in &session_state.building_table_projection.by_build_pos {
        let Some(block_id) = building.block_id else {
            continue;
        };
        let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
        scene.objects.push(RenderObject {
            id: format!("block:runtime-building:{tile_x}:{tile_y}:{block_id}"),
            layer: 14,
            x: tile_x as f32 * TILE_SIZE,
            y: tile_y as f32 * TILE_SIZE,
        });
    }
}

fn runtime_build_plan_object_id(index: usize, plan: &ClientBuildPlan) -> String {
    if plan.breaking {
        format!(
            "marker:runtime-break:{index}:{}:{}",
            plan.tile.0, plan.tile.1
        )
    } else {
        format!("plan:runtime-place:{index}:{}:{}", plan.tile.0, plan.tile.1)
    }
}

fn runtime_selected_block_label(selected_block_id: Option<i16>) -> String {
    selected_block_id
        .map(|block_id| format!("0x{:04x}", block_id as u16))
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_optional_vec2_label(value: Option<(f32, f32)>) -> String {
    value
        .map(|(x, y)| format!("0x{:08x}:0x{:08x}", x.to_bits(), y.to_bits()))
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_input_flags_label(snapshot_input: &ClientSnapshotInputState) -> String {
    format!(
        "boosting{}:shooting{}:chatting{}:building{}",
        if snapshot_input.boosting { 1 } else { 0 },
        if snapshot_input.shooting { 1 } else { 0 },
        if snapshot_input.chatting { 1 } else { 0 },
        if snapshot_input.building { 1 } else { 0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap_flow::encode_world_stream_packets;
    use crate::client_session::{ClientBuildPlanConfig, ClientSession};
    use mdt_protocol::encode_packet;
    use mdt_remote::read_remote_manifest;
    use mdt_render_ui::project_scene_models_with_player_position;
    use std::path::PathBuf;

    fn decode_hex_text(text: &str) -> Vec<u8> {
        let cleaned = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
            .collect()
    }

    fn sample_world_stream_bytes() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        ))
    }

    fn ingest_sample_world(session: &mut ClientSession) {
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
    }

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    fn first_runtime_effect_marker(scene: &RenderModel) -> &RenderObject {
        scene
            .objects
            .iter()
            .find(|object| object.id.starts_with("marker:runtime-effect:"))
            .expect("expected runtime effect marker")
    }

    #[test]
    fn render_runtime_adapter_applies_local_build_queue_to_scene_and_hud() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        {
            let input = session.snapshot_input_mut();
            input.building = true;
            input.boosting = true;
            input.chatting = true;
            input.position = Some((10.0, 20.0));
            input.pointer = Some((12.5, 24.0));
            input.view_center = Some((30.0, 40.0));
            input.view_size = Some((320.0, 180.0));
            input.selected_block_id = Some(0x0101);
            input.selected_rotation = 2;
            input.plans = Some(vec![
                ClientBuildPlan {
                    tile: (5, 4),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 1,
                    config: ClientBuildPlanConfig::Point2 { x: 8, y: 9 },
                },
                ClientBuildPlan {
                    tile: (4, 4),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
            ]);
        }

        let bundle = session.loaded_world_bundle().unwrap();
        let loaded_session = bundle.loaded_session().unwrap();
        let (mut scene, mut hud) =
            project_scene_models_with_player_position(&loaded_session, "en_US", Some((32.0, 32.0)));

        RenderRuntimeAdapter::default().apply(
            &mut scene,
            &mut hud,
            session.snapshot_input(),
            session.state(),
        );

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("plan:runtime-place:")));
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-break:")));
        assert!(hud.status_text.contains("runtime_selected=0x0101"));
        assert!(hud.status_text.contains("runtime_plans=2"));
        assert!(hud.status_text.contains("runtime_cfg_int=0"));
        assert!(hud.status_text.contains("runtime_cfg_bool=0"));
        assert!(hud.status_text.contains("runtime_cfg_point2=1"));
        assert!(hud.status_text.contains("runtime_cfg_point2_array=0"));
        assert!(hud.status_text.contains("runtime_cfg_string=0"));
        assert!(hud.status_text.contains("runtime_cfg_bytes=0"));
        assert!(hud.status_text.contains("runtime_cfg_content=0"));
        assert!(hud.status_text.contains("runtime_cfg_unit_command=0"));
        assert!(hud.status_text.contains("runtime_world_tiles=0"));
        assert!(hud.status_text.contains("building=1"));
        assert!(hud.status_text.contains(&format!(
            "runtime_view_center=0x{:08x}:0x{:08x}",
            30.0f32.to_bits(),
            40.0f32.to_bits()
        )));
        assert!(hud.status_text.contains(&format!(
            "runtime_view_size=0x{:08x}:0x{:08x}",
            320.0f32.to_bits(),
            180.0f32.to_bits()
        )));
        assert!(hud.status_text.contains(&format!(
            "runtime_position=0x{:08x}:0x{:08x}",
            10.0f32.to_bits(),
            20.0f32.to_bits()
        )));
        assert!(hud.status_text.contains(&format!(
            "runtime_pointer=0x{:08x}:0x{:08x}",
            12.5f32.to_bits(),
            24.0f32.to_bits()
        )));
        assert!(hud.status_text.contains("runtime_selected_rotation=2"));
        assert!(hud
            .status_text
            .contains("runtime_input_flags=boosting1:shooting0:chatting1:building1"));
        assert!(hud.status_text.contains(&format!(
            "runtime_builder=q0:i0:f0:r0:o0:none@none:none:local0 runtime_builder_head=none@none:none:bnone:rnone runtime_entity_local={} runtime_entity_hidden=0",
            runtime_local_entity_label(session.state())
        )));
        assert!(hud.status_text.contains("runtime_kick=none@none:none:none"));
    }

    #[test]
    fn render_runtime_adapter_reports_build_plan_config_subset_in_hud() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        {
            let input = session.snapshot_input_mut();
            input.plans = Some(vec![
                ClientBuildPlan {
                    tile: (2, 3),
                    breaking: false,
                    block_id: Some(0x0100),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Int(7),
                },
                ClientBuildPlan {
                    tile: (3, 3),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Bool(true),
                },
                ClientBuildPlan {
                    tile: (4, 3),
                    breaking: false,
                    block_id: Some(0x0102),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Point2 { x: 1, y: 2 },
                },
                ClientBuildPlan {
                    tile: (5, 3),
                    breaking: false,
                    block_id: Some(0x0103),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Point2Array(vec![(1, 2), (3, 4)]),
                },
                ClientBuildPlan {
                    tile: (6, 3),
                    breaking: false,
                    block_id: Some(0x0104),
                    rotation: 0,
                    config: ClientBuildPlanConfig::String("router".to_string()),
                },
                ClientBuildPlan {
                    tile: (7, 3),
                    breaking: false,
                    block_id: Some(0x0105),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Bytes(vec![1, 2, 3]),
                },
                ClientBuildPlan {
                    tile: (8, 3),
                    breaking: false,
                    block_id: Some(0x0106),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Content {
                        content_type: 1,
                        content_id: 0x0107,
                    },
                },
                ClientBuildPlan {
                    tile: (9, 3),
                    breaking: false,
                    block_id: Some(0x0108),
                    rotation: 0,
                    config: ClientBuildPlanConfig::UnitCommand(42),
                },
            ]);
        }

        let bundle = session.loaded_world_bundle().unwrap();
        let loaded_session = bundle.loaded_session().unwrap();
        let (mut scene, mut hud) =
            project_scene_models_with_player_position(&loaded_session, "en_US", Some((32.0, 32.0)));

        RenderRuntimeAdapter::default().apply(
            &mut scene,
            &mut hud,
            session.snapshot_input(),
            session.state(),
        );

        assert!(hud.status_text.contains("runtime_cfg_int=1"));
        assert!(hud.status_text.contains("runtime_cfg_bool=1"));
        assert!(hud.status_text.contains("runtime_cfg_point2=1"));
        assert!(hud.status_text.contains("runtime_cfg_point2_array=1"));
        assert!(hud.status_text.contains("runtime_cfg_string=1"));
        assert!(hud.status_text.contains("runtime_cfg_bytes=1"));
        assert!(hud.status_text.contains("runtime_cfg_content=1"));
        assert!(hud.status_text.contains("runtime_cfg_unit_command=1"));
    }

    #[test]
    fn runtime_world_overlay_tracks_authoritative_events() {
        let mut adapter = RenderRuntimeAdapter::default();

        adapter.observe_events(&[
            ClientSessionEvent::ConstructFinish {
                tile_pos: pack_runtime_point2(5, 4),
                block_id: Some(0x0101),
                builder_kind: 0,
                builder_value: 0,
                rotation: 0,
                team_id: 1,
                config_kind: 0,
                removed_local_plan: true,
            },
            ClientSessionEvent::BuildHealthUpdate {
                pair_count: 2,
                first_build_pos: Some(pack_runtime_point2(5, 4)),
                first_health_bits: Some(0x3f800000),
                pairs: vec![
                    BuildHealthPair {
                        build_pos: pack_runtime_point2(5, 4),
                        health_bits: 0x3f800000,
                    },
                    BuildHealthPair {
                        build_pos: pack_runtime_point2(6, 7),
                        health_bits: 0x3f000000,
                    },
                ],
            },
            ClientSessionEvent::DeconstructFinish {
                tile_pos: pack_runtime_point2(6, 7),
                block_id: Some(0x0102),
                builder_kind: 0,
                builder_value: 0,
                removed_local_plan: true,
            },
        ]);

        assert_eq!(adapter.world_overlay().tile_overlays.len(), 2);
        assert_eq!(
            adapter.world_overlay().tile_overlays.get(&(5, 4)),
            Some(&RuntimeTileOverlay {
                kind: RuntimeTileOverlayKind::Constructed,
                block_id: Some(0x0101),
                health_bits: Some(0x3f800000),
                config_kind_name: None,
                parse_failed: false,
                business_applied: true,
                pending_local_match: None,
                rollback: false,
            })
        );
        assert_eq!(
            adapter.world_overlay().tile_overlays.get(&(6, 7)),
            Some(&RuntimeTileOverlay {
                kind: RuntimeTileOverlayKind::Deconstructed,
                block_id: Some(0x0102),
                health_bits: None,
                config_kind_name: None,
                parse_failed: false,
                business_applied: true,
                pending_local_match: None,
                rollback: false,
            })
        );
        assert_eq!(adapter.world_overlay().health_overlay_count(), 1);
        assert_eq!(adapter.world_overlay().snapshot_refresh_count, 0);

        adapter.observe_events(&[ClientSessionEvent::WorldDataBegin]);
        assert!(adapter.world_overlay().tile_overlays.is_empty());
    }

    #[test]
    fn runtime_world_overlay_clears_on_session_reset_events() {
        let mut adapter = RenderRuntimeAdapter::default();
        adapter.observe_events(&[ClientSessionEvent::ConstructFinish {
            tile_pos: pack_runtime_point2(5, 4),
            block_id: Some(0x0101),
            builder_kind: 0,
            builder_value: 0,
            rotation: 0,
            team_id: 1,
            config_kind: 0,
            removed_local_plan: true,
        }]);
        assert_eq!(adapter.world_overlay().tile_overlays.len(), 1);

        adapter.observe_events(&[ClientSessionEvent::WorldStreamStarted {
            stream_id: 7,
            total_bytes: 1024,
        }]);
        assert!(adapter.world_overlay().tile_overlays.is_empty());

        adapter.observe_events(&[ClientSessionEvent::ConstructFinish {
            tile_pos: pack_runtime_point2(6, 7),
            block_id: Some(0x0102),
            builder_kind: 0,
            builder_value: 0,
            rotation: 0,
            team_id: 1,
            config_kind: 0,
            removed_local_plan: true,
        }]);
        assert_eq!(adapter.world_overlay().tile_overlays.len(), 1);

        adapter.observe_events(&[ClientSessionEvent::ConnectRedirectRequested {
            ip: "127.0.0.1".to_string(),
            port: 6567,
        }]);
        assert!(adapter.world_overlay().tile_overlays.is_empty());

        adapter.observe_events(&[ClientSessionEvent::ConstructFinish {
            tile_pos: pack_runtime_point2(8, 9),
            block_id: Some(0x0103),
            builder_kind: 0,
            builder_value: 0,
            rotation: 0,
            team_id: 1,
            config_kind: 0,
            removed_local_plan: true,
        }]);
        assert_eq!(adapter.world_overlay().tile_overlays.len(), 1);

        adapter.observe_events(&[ClientSessionEvent::Kicked {
            reason_text: None,
            reason_ordinal: None,
            duration_ms: None,
        }]);
        assert!(adapter.world_overlay().tile_overlays.is_empty());
        assert_eq!(adapter.world_overlay().snapshot_refresh_count, 0);
    }

    #[test]
    fn render_runtime_adapter_surfaces_kick_hint_in_overlay_and_hud() {
        let mut adapter = RenderRuntimeAdapter::default();
        adapter.observe_events(&[ClientSessionEvent::Kicked {
            reason_text: Some("server restart".to_string()),
            reason_ordinal: Some(15),
            duration_ms: Some(5_000),
        }]);

        assert_eq!(
            adapter.world_overlay().last_kick_reason_text.as_deref(),
            Some("server restart")
        );
        assert_eq!(adapter.world_overlay().last_kick_reason_ordinal, Some(15));
        assert_eq!(adapter.world_overlay().last_kick_duration_ms, Some(5_000));
        assert_eq!(
            adapter.world_overlay().last_kick_hint_category,
            Some("ServerRestarting")
        );
        assert_eq!(
            adapter.world_overlay().last_kick_hint_text,
            Some("server is restarting; retry connection shortly.")
        );

        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud.status_text.contains("runtime_kick="));
        assert!(hud.status_text.contains(":ServerRestarting:"));
        assert!(hud.status_text.contains("server_is_re~"));
    }

    #[test]
    fn runtime_kick_hint_from_surfaces_identity_conflict_reason() {
        assert_eq!(
            runtime_kick_hint_from(Some("idInUse"), Some(7)),
            Some((
                Some("IdInUse"),
                Some(
                    "uuid or usid is already in use; wait for the old session to clear or regenerate the connect identity.",
                ),
            ))
        );
    }

    #[test]
    fn render_runtime_adapter_adds_authoritative_world_overlay_objects() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        let bundle = session.loaded_world_bundle().unwrap();
        let loaded_session = bundle.loaded_session().unwrap();
        let (mut scene, mut hud) =
            project_scene_models_with_player_position(&loaded_session, "en_US", Some((32.0, 32.0)));
        let mut adapter = RenderRuntimeAdapter::default();

        adapter.observe_events(&[
            ClientSessionEvent::ConstructFinish {
                tile_pos: pack_runtime_point2(5, 4),
                block_id: Some(0x0101),
                builder_kind: 0,
                builder_value: 0,
                rotation: 0,
                team_id: 1,
                config_kind: 0,
                removed_local_plan: true,
            },
            ClientSessionEvent::DeconstructFinish {
                tile_pos: pack_runtime_point2(4, 4),
                block_id: Some(0x0102),
                builder_kind: 0,
                builder_value: 0,
                removed_local_plan: true,
            },
            ClientSessionEvent::BuildHealthUpdate {
                pair_count: 2,
                first_build_pos: Some(pack_runtime_point2(5, 4)),
                first_health_bits: Some(0x3f800000),
                pairs: vec![
                    BuildHealthPair {
                        build_pos: pack_runtime_point2(5, 4),
                        health_bits: 0x3f800000,
                    },
                    BuildHealthPair {
                        build_pos: pack_runtime_point2(9, 9),
                        health_bits: 0x3f000000,
                    },
                ],
            },
        ]);

        adapter.apply(
            &mut scene,
            &mut hud,
            session.snapshot_input(),
            session.state(),
        );

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("block:runtime-construct:")));
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("terrain:runtime-deconstruct:")));
        assert_eq!(
            scene
                .objects
                .iter()
                .filter(|object| object.id.starts_with("marker:runtime-health:"))
                .count(),
            2
        );
        assert!(hud.status_text.contains("runtime_world_tiles=3"));
        assert!(hud.status_text.contains("runtime_health=2"));
    }

    #[test]
    fn render_runtime_adapter_renders_block_snapshot_head_projection() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let block_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "blockSnapshot")
            .expect("missing blockSnapshot packet in fixture manifest")
            .packet_id;
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        let payload = [
            0x00, 0x01, // amount
            0x00, 0x11, // data len
            0x00, 0x07, 0x00, 0x05, // first build pos = pack(7, 5)
            0x01, 0x09, // first block id = 265
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x80, // rotation = 0 with version marker bit
            0x01, // team = 1
            0x03, // io version = 3
            0x01, // enabled = true
            0x08, // module bitmask
            0x00, // efficiency
            0x00, // optional efficiency
        ];
        let packet = encode_packet(block_snapshot_packet_id, &payload, false).unwrap();
        session.ingest_packet_bytes(&packet).unwrap();
        let bundle = session.loaded_world_bundle().unwrap();
        let loaded_session = bundle.loaded_session().unwrap();
        let (mut scene, mut hud) =
            project_scene_models_with_player_position(&loaded_session, "en_US", Some((32.0, 32.0)));

        RenderRuntimeAdapter::default().apply(
            &mut scene,
            &mut hud,
            session.snapshot_input(),
            session.state(),
        );

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "block:runtime-snapshot-head:7:5:265"));
    }

    #[test]
    fn render_runtime_adapter_renders_authoritative_building_table_projection() {
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.building_table_projection.by_build_pos.insert(
            pack_runtime_point2(12, 6),
            crate::session_state::BuildingProjection {
                block_id: Some(0x0102),
                rotation: Some(1),
                team_id: Some(2),
                io_version: None,
                module_bitmask: None,
                time_scale_bits: None,
                time_scale_duration_bits: None,
                last_disabler_pos: None,
                legacy_consume_connected: None,
                config: Some(mdt_typeio::TypeIoObject::Int(7)),
                health_bits: Some(0x3f800000),
                enabled: Some(true),
                efficiency: Some(0x80),
                optional_efficiency: Some(0x40),
                visible_flags: None,
                build_turret_rotation_bits: None,
                build_turret_plans_present: None,
                build_turret_plan_count: None,
                last_update: crate::session_state::BuildingProjectionUpdateKind::ConstructFinish,
            },
        );
        state.building_table_projection.block_known_count = 1;
        state.building_table_projection.configured_count = 1;
        state.building_table_projection.construct_finish_apply_count = 1;
        state.building_table_projection.last_build_pos = Some(pack_runtime_point2(12, 6));
        state.building_table_projection.last_block_id = Some(0x0102);
        state.building_table_projection.last_rotation = Some(1);
        state.building_table_projection.last_team_id = Some(2);
        state.building_table_projection.last_config = Some(mdt_typeio::TypeIoObject::Int(7));
        state.building_table_projection.last_health_bits = Some(0x3f800000);
        state.building_table_projection.last_enabled = Some(true);
        state.building_table_projection.last_efficiency = Some(0x80);
        state.building_table_projection.last_optional_efficiency = Some(0x40);
        state.building_table_projection.last_update =
            Some(crate::session_state::BuildingProjectionUpdateKind::ConstructFinish);

        RenderRuntimeAdapter::default().apply(&mut scene, &mut hud, &input, &state);

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "block:runtime-building:12:6:258"));
    }

    #[test]
    fn render_runtime_adapter_renders_tile_config_overlay() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::TileConfig {
            build_pos: Some(pack_runtime_point2(3, 2)),
            config_kind: Some(4),
            config_kind_name: Some("string".to_string()),
            parse_failed: false,
            business_applied: true,
            cleared_pending_local: false,
            was_rollback: false,
            pending_local_match: None,
            configured_block_outcome: None,
            configured_block_name: None,
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "marker:runtime-config:3:2:string"));
    }

    #[test]
    fn render_runtime_adapter_renders_tile_config_rollback_overlay() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::TileConfig {
            build_pos: Some(pack_runtime_point2(9, 7)),
            config_kind: Some(1),
            config_kind_name: Some("int".to_string()),
            parse_failed: false,
            business_applied: true,
            cleared_pending_local: true,
            was_rollback: true,
            pending_local_match: Some(false),
            configured_block_outcome: None,
            configured_block_name: None,
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "marker:runtime-config-rollback:9:7:int"));
    }

    #[test]
    fn render_runtime_adapter_renders_tile_config_pending_mismatch_overlay() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::TileConfig {
            build_pos: Some(pack_runtime_point2(6, 5)),
            config_kind: Some(1),
            config_kind_name: Some("int".to_string()),
            parse_failed: false,
            business_applied: true,
            cleared_pending_local: false,
            was_rollback: false,
            pending_local_match: Some(false),
            configured_block_outcome: None,
            configured_block_name: None,
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "marker:runtime-config-pending-mismatch:6:5:int"));
    }

    #[test]
    fn runtime_world_overlay_tracks_tile_config_observability_and_clears_on_world_data_begin() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::TileConfig {
            build_pos: Some(pack_runtime_point2(4, 1)),
            config_kind: Some(1),
            config_kind_name: Some("int".to_string()),
            parse_failed: true,
            business_applied: false,
            cleared_pending_local: false,
            was_rollback: false,
            pending_local_match: None,
            configured_block_outcome: None,
            configured_block_name: None,
        }]);
        assert_eq!(
            adapter.world_overlay().tile_overlays.get(&(4, 1)),
            Some(&RuntimeTileOverlay {
                kind: RuntimeTileOverlayKind::Configured,
                block_id: None,
                health_bits: None,
                config_kind_name: Some("int".to_string()),
                parse_failed: true,
                business_applied: false,
                pending_local_match: None,
                rollback: false,
            })
        );

        adapter.observe_events(&[ClientSessionEvent::TileConfig {
            build_pos: Some(pack_runtime_point2(4, 1)),
            config_kind: Some(1),
            config_kind_name: Some("int".to_string()),
            parse_failed: false,
            business_applied: false,
            cleared_pending_local: false,
            was_rollback: true,
            pending_local_match: Some(false),
            configured_block_outcome: None,
            configured_block_name: None,
        }]);
        assert_eq!(adapter.world_overlay().tile_overlays.len(), 1);
        assert_eq!(adapter.world_overlay().tile_config_event_count, 2);
        assert_eq!(adapter.world_overlay().tile_config_parse_failed_count, 1);
        assert_eq!(
            adapter
                .world_overlay()
                .tile_config_business_not_applied_count,
            2
        );
        assert_eq!(adapter.world_overlay().tile_config_rollback_count, 1);
        assert_eq!(
            adapter.world_overlay().tile_config_pending_mismatch_count,
            1
        );

        adapter.apply(&mut scene, &mut hud, &input, &state);
        assert!(hud.status_text.contains("runtime_tilecfg_events=2"));
        assert!(hud.status_text.contains("runtime_tilecfg_parse_fail=1"));
        assert!(hud.status_text.contains("runtime_tilecfg_noapply=2"));
        assert!(hud.status_text.contains("runtime_tilecfg_rollback=1"));
        assert!(hud
            .status_text
            .contains("runtime_tilecfg_pending_mismatch=1"));

        adapter.observe_events(&[ClientSessionEvent::WorldDataBegin]);
        assert!(adapter.world_overlay().tile_overlays.is_empty());
        assert_eq!(adapter.world_overlay().tile_config_event_count, 0);
        assert_eq!(adapter.world_overlay().tile_config_parse_failed_count, 0);
        assert_eq!(
            adapter
                .world_overlay()
                .tile_config_business_not_applied_count,
            0
        );
        assert_eq!(adapter.world_overlay().tile_config_rollback_count, 0);
        assert_eq!(
            adapter.world_overlay().tile_config_pending_mismatch_count,
            0
        );
    }

    #[test]
    fn runtime_world_overlay_tracks_snapshot_refresh_observability() {
        let mut adapter = RenderRuntimeAdapter::default();

        adapter.observe_events(&[
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::StateSnapshot),
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::HiddenSnapshot),
        ]);

        assert_eq!(adapter.world_overlay().snapshot_refresh_count, 2);
        assert_eq!(
            adapter.world_overlay().last_snapshot_method,
            Some(HighFrequencyRemoteMethod::HiddenSnapshot)
        );
        assert_eq!(
            adapter
                .world_overlay()
                .snapshot_method_count(HighFrequencyRemoteMethod::ClientSnapshot),
            0
        );
        assert_eq!(
            adapter
                .world_overlay()
                .snapshot_method_count(HighFrequencyRemoteMethod::StateSnapshot),
            1
        );
        assert_eq!(
            adapter
                .world_overlay()
                .snapshot_method_count(HighFrequencyRemoteMethod::EntitySnapshot),
            0
        );
        assert_eq!(
            adapter
                .world_overlay()
                .snapshot_method_count(HighFrequencyRemoteMethod::BlockSnapshot),
            0
        );
        assert_eq!(
            adapter
                .world_overlay()
                .snapshot_method_count(HighFrequencyRemoteMethod::HiddenSnapshot),
            1
        );
    }

    #[test]
    fn render_runtime_adapter_reports_unified_tile_config_business_chain_in_hud() {
        let adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();

        state.tile_config_projection.applied_authoritative_count = 3;
        state
            .tile_config_projection
            .applied_tile_config_packet_count = 2;
        state.tile_config_projection.applied_construct_finish_count = 1;
        state.tile_config_projection.configured_applied_count = 1;
        state.tile_config_projection.configured_rejected_count = 2;
        state.tile_config_projection.last_business_build_pos = Some(pack_runtime_point2(9, 4));
        state.tile_config_projection.last_business_applied = true;
        state.tile_config_projection.last_cleared_pending_local = true;
        state.tile_config_projection.last_was_rollback = true;
        state.tile_config_projection.last_pending_local_match = Some(false);
        state.tile_config_projection.last_business_source =
            Some(TileConfigAuthoritySource::ConstructFinish);
        state.tile_config_projection.last_configured_block_outcome =
            Some(ConfiguredBlockOutcome::Applied);
        state.tile_config_projection.last_configured_block_name = Some("item-source".to_string());

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud
            .status_text
            .contains("runtime_tilecfg_apply=a3:p2:c1:ca1:cr2:construct@9:4:cl1:rb1:pm0:coapplied:cbitem-source"));
    }

    #[test]
    fn render_runtime_adapter_reports_configured_block_projection_in_hud() {
        let adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .configured_block_projection
            .unit_cargo_unload_point_item_by_build_pos
            .insert(pack_runtime_point2(14, 36), None);
        state
            .configured_block_projection
            .item_source_item_by_build_pos
            .insert(pack_runtime_point2(12, 34), Some(0));
        state
            .configured_block_projection
            .liquid_source_liquid_by_build_pos
            .insert(pack_runtime_point2(13, 35), Some(0));
        state
            .configured_block_projection
            .message_text_by_build_pos
            .insert(pack_runtime_point2(18, 40), "hello".to_string());
        state
            .configured_block_projection
            .constructor_recipe_block_by_build_pos
            .insert(pack_runtime_point2(19, 41), Some(5));
        state
            .configured_block_projection
            .light_color_by_build_pos
            .insert(pack_runtime_point2(20, 42), 0x11223344);
        state
            .configured_block_projection
            .payload_source_content_by_build_pos
            .insert(
                pack_runtime_point2(21, 43),
                Some(ConfiguredContentRef {
                    content_type: 1,
                    content_id: 7,
                }),
            );
        state
            .configured_block_projection
            .payload_router_sorted_content_by_build_pos
            .insert(
                pack_runtime_point2(22, 44),
                Some(ConfiguredContentRef {
                    content_type: 6,
                    content_id: 9,
                }),
            );
        state
            .configured_block_projection
            .power_node_links_by_build_pos
            .insert(
                pack_runtime_point2(23, 45),
                [pack_runtime_point2(24, 46), pack_runtime_point2(25, 47)]
                    .into_iter()
                    .collect(),
            );
        state
            .configured_block_projection
            .reconstructor_command_by_build_pos
            .insert(pack_runtime_point2(26, 48), Some(12));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud
            .status_text
            .contains("runtime_configured=uc1@14:36=clear:is1@12:34=0:ls1@13:35=0"));
        assert!(hud.status_text.contains(":mg1@18:40=len5:"));
        assert!(hud.status_text.contains(":ct1@19:41=5:"));
        assert!(hud.status_text.contains(":il1@20:42=11223344:"));
        assert!(hud.status_text.contains(":ps1@21:43=b:7:"));
        assert!(hud.status_text.contains(":pr1@22:44=u:9:"));
        assert!(hud.status_text.contains(":pn1@23:45=n2:24:46|25:47:"));
        assert!(hud.status_text.contains(":rc1@26:48=12"));
    }

    #[test]
    fn runtime_configured_power_node_family_label_renders_clear_for_empty_set() {
        let mut values = BTreeMap::new();
        values.insert(pack_runtime_point2(12, 34), BTreeSet::new());

        assert_eq!(
            runtime_configured_power_node_family_label("pn", &values),
            "pn1@12:34=clear"
        );
    }

    #[test]
    fn render_runtime_adapter_renders_recent_effect_overlays_and_clears_them() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: Some(mdt_typeio::TypeIoObject::Int(7)),
            },
            ClientSessionEvent::EffectReliableRequested {
                effect_id: Some(21),
                x: 12.0,
                y: 16.0,
                rotation: 45.0,
                color_rgba: 0x55667788,
            },
        ]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:runtime-effect:normal:13:0x42020000:0x42400000:1"
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:runtime-effect:reliable:21:0x41400000:0x41800000:0"
        }));

        for _ in 0..(EFFECT_OVERLAY_TTL_TICKS - 1) {
            adapter.observe_events(&[]);
            let mut decayed_scene = RenderModel::default();
            let mut decayed_hud = HudModel::default();
            adapter.apply(&mut decayed_scene, &mut decayed_hud, &input, &state);
            assert!(decayed_scene
                .objects
                .iter()
                .any(|object| object.id.starts_with("marker:runtime-effect:")));
        }

        adapter.observe_events(&[]);
        let mut expired_scene = RenderModel::default();
        let mut expired_hud = HudModel::default();
        adapter.apply(&mut expired_scene, &mut expired_hud, &input, &state);
        assert!(!expired_scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));

        adapter.observe_events(&[ClientSessionEvent::WorldDataBegin]);
        let mut cleared_scene = RenderModel::default();
        let mut cleared_hud = HudModel::default();
        adapter.apply(&mut cleared_scene, &mut cleared_hud, &input, &state);
        assert!(!cleared_scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
    }

    #[test]
    fn render_runtime_adapter_projects_point2_effect_payload_to_world_position() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(8),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            "marker:runtime-effect:normal:8:0x42a00000:0x43200000:1"
        );
        assert_eq!(marker.x, 80.0);
        assert_eq!(marker.y, 160.0);
    }

    #[test]
    fn render_runtime_adapter_projects_building_pos_effect_payload_to_tile_world_position() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(13),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::BuildingPos(pack_runtime_point2(
                1, 2,
            ))),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            "marker:runtime-effect:normal:13:0x41000000:0x41800000:1"
        );
        assert_eq!(marker.x, 8.0);
        assert_eq!(marker.y, 16.0);
    }

    #[test]
    fn render_runtime_adapter_recomputes_unit_parent_effect_marker_position_each_apply() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            404,
            crate::session_state::EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 88,
                x_bits: 12.0f32.to_bits(),
                y_bits: 16.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(257),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            "marker:runtime-effect:normal:257:0x41400000:0x41800000:1"
        );
        assert_eq!(marker.x, 12.0);
        assert_eq!(marker.y, 16.0);

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

        let mut updated_scene = RenderModel::default();
        let mut updated_hud = HudModel::default();
        adapter.apply(&mut updated_scene, &mut updated_hud, &input, &state);

        let updated_marker = first_runtime_effect_marker(&updated_scene);
        assert_eq!(
            updated_marker.id,
            "marker:runtime-effect:normal:257:0x41c00000:0x41e00000:1"
        );
        assert_eq!(updated_marker.x, 24.0);
        assert_eq!(updated_marker.y, 28.0);
    }

    #[test]
    fn render_runtime_adapter_falls_back_to_snapshot_input_position_for_missing_parent_unit() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState {
            unit_id: Some(404),
            dead: false,
            position: Some((44.0, 60.0)),
            ..Default::default()
        };
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(257),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:257:0x{:08x}:0x{:08x}:1",
                44.0f32.to_bits(),
                60.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 44.0);
        assert_eq!(marker.y, 60.0);
    }

    #[test]
    fn render_runtime_adapter_falls_back_to_world_player_position_for_missing_parent_unit() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState {
            unit_id: Some(404),
            dead: false,
            ..Default::default()
        };
        let mut state = SessionState::default();
        state.world_player_x_bits = Some(52.0f32.to_bits());
        state.world_player_y_bits = Some(68.0f32.to_bits());

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(260),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:260:0x{:08x}:0x{:08x}:1",
                52.0f32.to_bits(),
                68.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 52.0);
        assert_eq!(marker.y, 68.0);
    }

    #[test]
    fn render_runtime_adapter_projects_packed_point2_array_first_effect_payload() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(13),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::PackedPoint2Array(vec![
                pack_runtime_point2(9, 6),
                pack_runtime_point2(1, 2),
            ])),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:13:0x{:08x}:0x{:08x}:1",
                72.0f32.to_bits(),
                48.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 72.0);
        assert_eq!(marker.y, 48.0);
    }

    #[test]
    fn runtime_effect_business_projection_label_formats_supported_variants() {
        assert_eq!(
            runtime_effect_business_projection_label(Some(&EffectBusinessProjection::ContentRef {
                kind: EffectBusinessContentKind::Content,
                content_type: 2,
                content_id: 0x0123,
            })),
            "content:content:2:291"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(&EffectBusinessProjection::ContentRef {
                kind: EffectBusinessContentKind::TechNode,
                content_type: 4,
                content_id: 0x0102,
            })),
            "content:techNode:4:258"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(&EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::LocalUnitId,
                value: 77,
                x_bits: 64.0f32.to_bits(),
                y_bits: 72.0f32.to_bits(),
            })),
            "parent:localUnit:0x0000004d:0x42800000:0x42900000"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(&EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::EntityUnitId,
                value: 12,
                x_bits: 12.0f32.to_bits(),
                y_bits: 24.0f32.to_bits(),
            })),
            "parent:entityUnit:0x0000000c:0x41400000:0x41c00000"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(&EffectBusinessProjection::FloatValue(
                12.5f32.to_bits()
            ))),
            "floatBits:0x41480000"
        );
        assert_eq!(runtime_effect_business_projection_label(None), "none");
    }

    #[test]
    fn runtime_effect_data_fail_label_compacts_last_parse_error_reason() {
        let mut state = SessionState::default();
        assert_eq!(runtime_effect_data_fail_label(&state), "0@none");

        state.failed_effect_data_parse_count = 2;
        state.last_effect_data_parse_error =
            Some("trailing bytes after effect data object".to_string());
        assert_eq!(runtime_effect_data_fail_label(&state), "2@trail");

        state.last_effect_data_parse_error =
            Some("failed to parse effect data object: unsupported type".to_string());
        assert_eq!(runtime_effect_data_fail_label(&state), "2@decode");
    }

    #[test]
    fn render_runtime_adapter_reports_snapshot_observability_in_hud() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.last_state_snapshot = Some(crate::session_state::AppliedStateSnapshot {
            wave: 7,
            enemies: 3,
            tps: 60,
            ..Default::default()
        });
        state.last_state_snapshot_core_data =
            Some(crate::session_state::AppliedStateSnapshotCoreData {
                team_count: 1,
                teams: vec![crate::session_state::AppliedStateSnapshotCoreDataTeam {
                    team_id: 1,
                    items: vec![
                        crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 0,
                            amount: 321,
                        },
                        crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 1,
                            amount: 45,
                        },
                    ],
                }],
            });
        state.last_good_state_snapshot_core_data =
            Some(crate::session_state::AppliedStateSnapshotCoreData {
                team_count: 1,
                teams: vec![crate::session_state::AppliedStateSnapshotCoreDataTeam {
                    team_id: 1,
                    items: vec![
                        crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 0,
                            amount: 321,
                        },
                        crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 1,
                            amount: 45,
                        },
                    ],
                }],
            });
        state.state_snapshot_business_projection =
            Some(crate::session_state::StateSnapshotBusinessProjection {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 3,
                paused: false,
                game_over: false,
                time_data: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                gameplay_state: crate::session_state::GameplayStateProjection::Playing,
                gameplay_state_transition_count: 1,
                last_wave_advanced: true,
                last_wave_advance_from: Some(6),
                last_wave_advance_to: Some(7),
                wave_advance_count: 2,
                net_seconds_applied_count: 3,
                last_net_seconds_rollback: false,
                net_seconds_delta: 60,
                state_snapshot_apply_count: 3,
                state_snapshot_time_regress_count: 0,
                state_snapshot_wave_regress_count: 0,
                core_inventory_synced: true,
                core_inventory_team_count: 1,
                core_inventory_item_entry_count: 2,
                core_inventory_total_amount: 366,
                core_inventory_nonzero_item_count: 2,
                core_inventory_changed_team_count: 1,
                core_inventory_changed_team_sample: vec![1],
                core_inventory_by_team: BTreeMap::from([(
                    1,
                    BTreeMap::from([(0u16, 321), (1u16, 45)]),
                )]),
            });
        state.last_block_snapshot = Some(crate::session_state::AppliedBlockSnapshotEnvelope {
            amount: 1,
            data_len: 39,
            first_build_pos: Some(pack_runtime_point2(100, 99)),
            first_block_id: Some(301),
            first_health_bits: Some(1100.0f32.to_bits()),
            first_rotation: Some(0),
            first_team_id: Some(1),
            first_io_version: Some(3),
            first_enabled: Some(true),
            first_module_bitmask: Some(9),
            first_time_scale_bits: None,
            first_time_scale_duration_bits: None,
            first_last_disabler_pos: None,
            first_legacy_consume_connected: None,
            first_efficiency: Some(0),
            first_optional_efficiency: Some(0),
            first_visible_flags: None,
        });
        state.last_hidden_snapshot = Some(crate::session_state::AppliedHiddenSnapshotIds {
            count: 3,
            first_id: Some(100),
            sample_ids: vec![100, 101, 202],
        });
        state.hidden_snapshot_delta_projection =
            Some(crate::session_state::HiddenSnapshotDeltaProjection {
                active_count: 3,
                added_count: 1,
                removed_count: 2,
                added_sample_ids: vec![303],
                removed_sample_ids: vec![100, 202],
            });
        state.entity_snapshot_tombstone_skip_count = 5;
        state.last_entity_snapshot_tombstone_skipped_ids_sample = vec![100, 202];
        state.entity_snapshot_tombstones = BTreeMap::from([(100, 11), (202, 12)]);
        state.entity_snapshot_with_local_target_count = 6;
        state.last_entity_snapshot_target_player_id = Some(404);
        state.last_entity_snapshot_local_player_sync_applied = false;
        state.last_entity_snapshot_local_player_sync_ambiguous = true;
        state.last_entity_snapshot_local_player_sync_match_count = 2;
        state.missed_local_player_sync_from_entity_snapshot_count = 7;
        state.world_bootstrap_projection = Some(crate::session_state::WorldBootstrapProjection {
            rules_sha256: "0123456789abcdef".to_string(),
            map_locales_sha256: "fedcba9876543210".to_string(),
            tags_sha256: "0011223344556677".to_string(),
            team_count: 2,
            marker_count: 4,
            custom_chunk_count: 1,
            content_patch_count: 3,
            player_team_plan_count: 5,
            static_fog_team_count: 1,
        });
        state.failed_block_snapshot_parse_count = 2;
        state.failed_hidden_snapshot_parse_count = 1;
        state.received_effect_count = 11;
        state.last_effect_data_kind = Some("Point2".to_string());
        state.last_effect_contract_name = Some("position_target".to_string());
        state.last_effect_reliable_contract_name = Some("unit_parent".to_string());
        state.last_effect_data_semantic =
            Some(crate::session_state::EffectDataSemantic::Point2 { x: 3, y: 4 });
        state.last_effect_business_projection = Some(EffectBusinessProjection::WorldPosition {
            source: EffectBusinessPositionSource::Point2,
            x_bits: 24.0f32.to_bits(),
            y_bits: 32.0f32.to_bits(),
        });
        state.last_effect_business_path = Some(vec![1, 0]);
        state.failed_effect_data_parse_count = 2;
        state.last_effect_data_parse_error =
            Some("trailing bytes after effect data object".to_string());
        state.received_take_items_count = 1;
        state.received_transfer_item_to_count = 2;
        state.received_transfer_item_to_unit_count = 3;
        state.received_payload_dropped_count = 4;
        state.received_picked_build_payload_count = 5;
        state.received_picked_unit_payload_count = 6;
        state.received_unit_entered_payload_count = 7;
        state.received_unit_despawn_count = 8;
        state.received_build_destroyed_count = 66;
        state.last_build_destroyed_build_pos = Some(pack_runtime_point2(3, 12));
        state.received_unit_death_count = 67;
        state.last_unit_death_id = Some(701);
        state.received_unit_destroy_count = 68;
        state.last_unit_destroy_id = Some(702);
        state.received_unit_env_death_count = 69;
        state.last_unit_env_death = Some(crate::session_state::UnitRefProjection {
            kind: 2,
            value: 703,
        });
        state.received_unit_safe_death_count = 70;
        state.last_unit_safe_death = Some(crate::session_state::UnitRefProjection {
            kind: 1,
            value: pack_runtime_point2(11, 12),
        });
        state.received_unit_cap_death_count = 71;
        state.last_unit_cap_death = Some(crate::session_state::UnitRefProjection {
            kind: 2,
            value: 704,
        });
        state.received_create_weather_count = 72;
        state.last_create_weather_id = Some(5);
        state.received_spawn_effect_count = 73;
        state.last_spawn_effect_unit_type_id = Some(19);
        state.received_logic_explosion_count = 79;
        state.last_logic_explosion_team_id = Some(2);
        state.last_logic_explosion_air = Some(true);
        state.last_logic_explosion_ground = Some(false);
        state.last_logic_explosion_pierce = Some(true);
        state.last_logic_explosion_effect = Some(true);
        state.received_unit_spawn_count = 74;
        state.last_unit_spawn_id = Some(404);
        state.last_unit_spawn_class_id = Some(36);
        state.last_unit_spawn_trailing_bytes = Some(3);
        state.received_unit_block_spawn_count = 75;
        state.last_unit_block_spawn_tile_pos = Some(pack_runtime_point2(4, 15));
        state.received_unit_tether_block_spawned_count = 76;
        state.last_unit_tether_block_spawned_tile_pos = Some(pack_runtime_point2(8, 3));
        state.last_unit_tether_block_spawned_id = Some(404);
        state.received_sound_count = 54;
        state.received_sound_at_count = 55;
        state.received_trace_info_count = 56;
        state.received_debug_status_client_count = 57;
        state.received_debug_status_client_unreliable_count = 58;
        state.failed_sound_parse_count = 74;
        state.failed_sound_at_parse_count = 75;
        state.failed_trace_info_parse_count = 76;
        state.failed_debug_status_client_parse_count = 77;
        state.failed_debug_status_client_unreliable_parse_count = 78;
        state.last_sound_id = Some(7);
        state.last_sound_at_id = Some(11);
        state.last_trace_info_player_id = Some(123456);
        state.last_debug_status_value = Some(12);
        state.deferred_inbound_packet_count = 59;
        state.replayed_inbound_packet_count = 60;
        state.dropped_loading_low_priority_packet_count = 61;
        state.dropped_loading_deferred_overflow_count = 0;
        state.failed_state_snapshot_parse_count = 62;
        state.failed_state_snapshot_core_data_parse_count = 63;
        state.failed_entity_snapshot_parse_count = 64;
        state.ready_inbound_liveness_anchor_count = 65;
        state.last_ready_inbound_liveness_anchor_at_ms = Some(66);
        state.received_set_rules_count = 67;
        state.failed_set_rules_parse_count = 68;
        state.received_set_objectives_count = 69;
        state.failed_set_objectives_parse_count = 70;
        state.received_set_rule_count = 71;
        state.failed_set_rule_parse_count = 72;
        state.received_clear_objectives_count = 73;
        state.received_complete_objective_count = 74;
        state.rules_projection.waves = Some(true);
        state.rules_projection.pvp = Some(false);
        state.objectives_projection.objectives = vec![
            crate::rules_objectives_semantics::ObjectiveProjection::default(),
            crate::rules_objectives_semantics::ObjectiveProjection::default(),
        ];
        state.objectives_projection.objectives[0].qualified = true;
        state.objectives_projection.objectives[0].parents = vec![1];
        state
            .objectives_projection
            .objective_flags
            .insert("alpha".to_string());
        state
            .objectives_projection
            .objective_flags
            .insert("beta".to_string());
        state.objectives_projection.complete_out_of_range_count = 75;
        state.objectives_projection.last_completed_index = Some(9);
        state.received_set_hud_text_count = 9;
        state.received_set_hud_text_reliable_count = 10;
        state.received_hide_hud_text_count = 11;
        state.last_set_hud_text_message = Some("hud".to_string());
        state.last_set_hud_text_reliable_message = Some("hud reliable".to_string());
        state.received_announce_count = 12;
        state.received_info_message_count = 13;
        state.received_info_toast_count = 14;
        state.received_warning_toast_count = 15;
        state.last_info_toast_message = Some("toast".to_string());
        state.last_warning_toast_text = Some("warning".to_string());
        state.received_menu_open_count = 16;
        state.received_follow_up_menu_open_count = 17;
        state.received_hide_follow_up_menu_count = 18;
        state.received_world_label_count = 19;
        state.received_world_label_reliable_count = 20;
        state.received_remove_world_label_count = 21;
        state.received_create_marker_count = 54;
        state.received_remove_marker_count = 55;
        state.received_update_marker_count = 56;
        state.received_update_marker_text_count = 57;
        state.received_update_marker_texture_count = 58;
        state.failed_marker_decode_count = 2;
        state.last_marker_id = Some(808);
        state.last_marker_control_name = Some("flushText".to_string());
        state.received_set_tile_overlays_count = 59;
        state.last_set_tile_overlays_block_id = Some(17);
        state.last_set_tile_overlays_count = 2;
        state.last_set_tile_overlays_first_position = Some(pack_runtime_point2(5, 6));
        state.received_sync_variable_count = 60;
        state.last_sync_variable_build_pos = Some(pack_runtime_point2(9, 10));
        state.last_sync_variable_index = Some(4);
        state.last_sync_variable_value_kind = Some(4);
        state.last_sync_variable_value_kind_name = Some("string".to_string());
        state.received_set_item_count = 22;
        state.received_set_items_count = 23;
        state.received_set_liquid_count = 24;
        state.received_set_liquids_count = 25;
        state.received_clear_items_count = 84;
        state.received_clear_liquids_count = 85;
        state.received_set_tile_items_count = 26;
        state.received_set_tile_liquids_count = 27;
        state.resource_delta_projection.take_items_count = 1;
        state.resource_delta_projection.transfer_item_to_count = 2;
        state.resource_delta_projection.transfer_item_to_unit_count = 3;
        state.resource_delta_projection.last_kind = Some("to_unit");
        state.resource_delta_projection.last_item_id = Some(6);
        state.resource_delta_projection.last_amount = None;
        state.resource_delta_projection.last_build_pos = None;
        state.resource_delta_projection.last_unit = None;
        state.resource_delta_projection.last_to_entity_id = Some(404);
        state
            .resource_delta_projection
            .building_items_by_build
            .insert(pack_runtime_point2(1, 1), std::collections::BTreeMap::from([(4, 6), (7, 8)]));
        state
            .resource_delta_projection
            .building_items_by_build
            .insert(pack_runtime_point2(2, 2), std::collections::BTreeMap::from([(9, 10)]));
        state.resource_delta_projection.entity_item_stack_by_entity_id.insert(
            900,
            crate::session_state::ResourceUnitItemStack {
                item_id: Some(6),
                amount: 3,
            },
        );
        state.resource_delta_projection.authoritative_build_update_count = 4;
        state.resource_delta_projection.delta_apply_count = 5;
        state.resource_delta_projection.delta_skip_count = 6;
        state.resource_delta_projection.delta_conflict_count = 7;
        state.resource_delta_projection.last_changed_build_pos = Some(pack_runtime_point2(9, 9));
        state.resource_delta_projection.last_changed_entity_id = Some(900);
        state.resource_delta_projection.last_changed_item_id = Some(6);
        state.resource_delta_projection.last_changed_amount = Some(1);
        state.received_remove_tile_count = 80;
        state.received_set_tile_count = 81;
        state.received_set_floor_count = 82;
        state.received_set_overlay_count = 83;
        state.received_set_player_team_editor_count = 28;
        state.received_menu_choose_count = 29;
        state.received_text_input_result_count = 30;
        state.received_copy_to_clipboard_count = 51;
        state.received_open_uri_count = 52;
        state.received_text_input_count = 53;
        state.last_copy_to_clipboard_text = Some("copied".to_string());
        state.last_open_uri = Some("https://example.com".to_string());
        state.last_text_input_id = Some(404);
        state.last_text_input_title = Some("Digits".to_string());
        state.last_text_input_message = Some("Only numbers".to_string());
        state.last_text_input_default_text = Some("12345".to_string());
        state.last_text_input_length = Some(16);
        state.last_text_input_numeric = Some(true);
        state.last_text_input_allow_empty = Some(true);
        state.last_set_player_team_editor_team_id = Some(7);
        state.last_menu_choose_menu_id = Some(404);
        state.last_menu_choose_option = Some(2);
        state.last_text_input_result_id = Some(405);
        state.last_text_input_result_text = Some("ok123".to_string());
        state.received_request_item_count = 31;
        state.last_request_item_build_pos = Some(pack_runtime_point2(6, 7));
        state.last_request_item_item_id = Some(9);
        state.last_request_item_amount = Some(12);
        state.received_building_control_select_count = 32;
        state.last_building_control_select_build_pos = Some(pack_runtime_point2(10, 11));
        state.received_unit_clear_count = 33;
        state.received_unit_control_count = 34;
        state.last_unit_control_target = Some(crate::session_state::UnitRefProjection {
            kind: 2,
            value: 404,
        });
        state.received_unit_building_control_select_count = 35;
        state.last_unit_building_control_select_target =
            Some(crate::session_state::UnitRefProjection {
                kind: 1,
                value: 505,
            });
        state.last_unit_building_control_select_build_pos = Some(pack_runtime_point2(12, 13));
        state.received_command_building_count = 36;
        state.last_command_building_count = 2;
        state.last_command_building_first_build_pos = Some(pack_runtime_point2(14, 15));
        state.last_command_building_x_bits = Some(1.5f32.to_bits());
        state.last_command_building_y_bits = Some(2.5f32.to_bits());
        state.received_command_units_count = 37;
        state.last_command_units_count = 2;
        state.last_command_units_first_unit_id = Some(700);
        state.last_command_units_build_target = Some(pack_runtime_point2(16, 17));
        state.last_command_units_unit_target = Some(crate::session_state::UnitRefProjection {
            kind: 2,
            value: 808,
        });
        state.last_command_units_x_bits = Some(3.5f32.to_bits());
        state.last_command_units_y_bits = Some(4.5f32.to_bits());
        state.last_command_units_queue = Some(true);
        state.last_command_units_final_batch = Some(false);
        state.received_set_unit_command_count = 38;
        state.last_set_unit_command_count = 3;
        state.last_set_unit_command_first_unit_id = Some(701);
        state.last_set_unit_command_id = Some(9);
        state.received_set_unit_stance_count = 39;
        state.last_set_unit_stance_count = 4;
        state.last_set_unit_stance_first_unit_id = Some(702);
        state.last_set_unit_stance_id = Some(5);
        state.last_set_unit_stance_enable = Some(true);
        state.received_rotate_block_count = 40;
        state.last_rotate_block_build_pos = Some(pack_runtime_point2(18, 19));
        state.last_rotate_block_direction = Some(false);
        state.received_transfer_inventory_count = 41;
        state.last_transfer_inventory_build_pos = Some(pack_runtime_point2(20, 21));
        state.received_request_build_payload_count = 42;
        state.last_request_build_payload_build_pos = Some(pack_runtime_point2(22, 23));
        state.received_request_unit_payload_count = 43;
        state.last_request_unit_payload_target = Some(crate::session_state::UnitRefProjection {
            kind: 1,
            value: 909,
        });
        state.received_drop_item_count = 44;
        state.last_drop_item_angle_bits = Some(7.5f32.to_bits());
        state.received_delete_plans_count = 45;
        state.last_delete_plans_count = 3;
        state.last_delete_plans_first_pos = Some(pack_runtime_point2(24, 25));
        state.received_request_drop_payload_count = 46;
        state.last_request_drop_payload_x_bits = Some(5.5f32.to_bits());
        state.last_request_drop_payload_y_bits = Some(6.5f32.to_bits());
        state.received_tile_tap_count = 47;
        state.last_tile_tap_pos = Some(pack_runtime_point2(26, 27));
        state.received_set_flag_count = 46;
        state.received_game_over_count = 47;
        state.received_update_game_over_count = 48;
        state.received_sector_capture_count = 49;
        state.received_researched_count = 50;
        state.builder_queue_projection = crate::session_state::BuilderQueueProjection {
            active_by_tile: BTreeMap::new(),
            ordered_tiles: vec![(100, 99), (98, 97)],
            queued_count: 1,
            inflight_count: 2,
            finished_count: 3,
            removed_count: 4,
            orphan_authoritative_count: 1,
            head_x: Some(100),
            head_y: Some(99),
            head_breaking: Some(false),
            head_block_id: Some(301),
            head_rotation: Some(1),
            head_stage: Some(crate::session_state::BuilderPlanStage::InFlight),
            last_stage: Some(crate::session_state::BuilderPlanStage::Finished),
            last_x: Some(100),
            last_y: Some(99),
            last_breaking: Some(false),
            last_block_id: Some(301),
            last_rotation: Some(1),
            last_team_id: Some(2),
            last_builder_kind: Some(3),
            last_builder_value: Some(44),
            last_removed_local_plan: true,
            last_orphan_authoritative: false,
        };
        state.building_table_projection = crate::session_state::BuildingTableProjection {
            by_build_pos: BTreeMap::from([(
                pack_runtime_point2(100, 99),
                crate::session_state::BuildingProjection {
                    block_id: Some(301),
                    rotation: Some(1),
                    team_id: Some(2),
                    io_version: None,
                    module_bitmask: None,
                    time_scale_bits: None,
                    time_scale_duration_bits: None,
                    last_disabler_pos: None,
                    legacy_consume_connected: None,
                    config: Some(mdt_typeio::TypeIoObject::Int(7)),
                    health_bits: Some(0x3f800000),
                    enabled: Some(true),
                    efficiency: Some(0x80),
                    optional_efficiency: Some(0x40),
                    visible_flags: None,
                    build_turret_rotation_bits: Some(0x4210_0000),
                    build_turret_plans_present: None,
                    build_turret_plan_count: None,
                    last_update: crate::session_state::BuildingProjectionUpdateKind::TileConfig,
                },
            )]),
            block_known_count: 1,
            configured_count: 1,
            block_snapshot_head_apply_count: 0,
            block_snapshot_head_conflict_skip_count: 0,
            construct_finish_apply_count: 1,
            tile_config_apply_count: 2,
            deconstruct_finish_apply_count: 0,
            build_health_apply_count: 1,
            last_build_pos: Some(pack_runtime_point2(100, 99)),
            last_block_id: Some(301),
            last_rotation: Some(1),
            last_team_id: Some(2),
            last_io_version: None,
            last_module_bitmask: None,
            last_time_scale_bits: None,
            last_time_scale_duration_bits: None,
            last_last_disabler_pos: None,
            last_legacy_consume_connected: None,
            last_config: Some(mdt_typeio::TypeIoObject::Int(7)),
            last_health_bits: Some(0x3f800000),
            last_enabled: Some(true),
            last_efficiency: Some(0x80),
            last_optional_efficiency: Some(0x40),
            last_visible_flags: None,
            last_build_turret_rotation_bits: Some(0x4210_0000),
            last_build_turret_plans_present: None,
            last_build_turret_plan_count: None,
            last_update: Some(crate::session_state::BuildingProjectionUpdateKind::TileConfig),
            last_removed: false,
            last_block_snapshot_head_conflict: false,
        };
        adapter.observe_events(&[
            ClientSessionEvent::StateSnapshotApplied {
                projection: StateSnapshotAppliedProjection {
                    wave: 7,
                    enemies: 3,
                    tps: 60,
                    gameplay_state: crate::session_state::GameplayStateProjection::Playing,
                    gameplay_state_transition_count: 2,
                    wave_advanced: true,
                    wave_advance_from: Some(6),
                    wave_advance_to: Some(7),
                    apply_count: 4,
                    net_seconds_delta: 9,
                    net_seconds_rollback: false,
                    time_regress_count: 1,
                    wave_regress_count: 0,
                    core_inventory_team_count: 1,
                    core_inventory_item_entry_count: 2,
                    core_inventory_total_amount: 20,
                    core_inventory_changed_team_count: 1,
                    core_inventory_changed_team_sample: vec![1],
                    core_parse_failed: false,
                    core_parse_fail_count: 0,
                    used_last_good_core_fallback: false,
                },
            },
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::BlockSnapshot),
        ]);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud.status_text.contains("runtime_snap_last=blockSnapshot"));
        assert!(hud.status_text.contains("runtime_snap_events=2"));
        assert!(hud
            .status_text
            .contains("runtime_snap_apply=w7:splay:gt2:adv1@6->7:app4:nd9:rb0:tr1:wr0:cpf0:fb0"));
        assert!(hud.status_text.contains("runtime_snap_client=0"));
        assert!(hud.status_text.contains("runtime_snap_state=1"));
        assert!(hud.status_text.contains("runtime_snap_entity=0"));
        assert!(hud.status_text.contains("runtime_snap_block=1"));
        assert!(hud.status_text.contains("runtime_snap_hidden=0"));
        assert!(hud.status_text.contains("runtime_wave=7"));
        assert!(hud.status_text.contains("runtime_enemies=3"));
        assert!(hud.status_text.contains("runtime_tps=60"));
        assert!(hud
            .status_text
            .contains("runtime_state_apply=w7:e3:t60:c1/2:adv1:core1"));
        assert!(hud.status_text.contains(":ca1:cas1"));
        assert!(hud
            .status_text
            .contains("runtime_builder=q1:i2:f3:r4:o1:finish@100:99:place:local1"));
        assert!(hud
            .status_text
            .contains("runtime_builder_head=flight@100:99:place:b301:r1"));
        assert!(hud.status_text.contains("runtime_core_teams=1"));
        assert!(hud.status_text.contains("runtime_core_items=2"));
        assert!(hud
            .status_text
            .contains("runtime_buildings=1:b1:c1:config@100:99#301:rm0:on1:e128:oe64"));
        assert!(hud.status_text.contains(":trb0x42100000"));
        assert!(hud
            .status_text
            .contains("runtime_block=1x39@100:99#301:r0:t1:v3:on1:e0:oe0"));
        assert!(hud.status_text.contains("runtime_block_fail=2"));
        assert!(hud.status_text.contains("runtime_hidden=3@100,101,202"));
        assert!(hud
            .status_text
            .contains("runtime_hidden_delta=+1@303|-2@100,202"));
        assert!(hud.status_text.contains("runtime_hidden_fail=1"));
        assert!(hud
            .status_text
            .contains("runtime_entity_gate=ts5@100,202+3:a2"));
        assert!(hud
            .status_text
            .contains("runtime_entity_sync=lt6:tp404:ok0:amb1@2:miss7:fail64"));
        assert!(hud.status_text.contains("runtime_effects=11"));
        assert!(hud.status_text.contains("runtime_effect_data_kind=Point2"));
        assert!(hud
            .status_text
            .contains("runtime_effect_contract=position_target/unit_parent"));
        assert!(hud
            .status_text
            .contains("runtime_effect_data_semantic=point2:3:4"));
        assert!(hud
            .status_text
            .contains("runtime_effect_apply=pos:point2:0x41c00000:0x42000000"));
        assert!(hud.status_text.contains("runtime_effect_path=1/0"));
        assert!(hud.status_text.contains("runtime_effect_data_fail=2@trail"));
        assert!(hud.status_text.contains("bootstrap_rules=01234567"));
        assert!(hud.status_text.contains("bootstrap_tags=00112233"));
        assert!(hud.status_text.contains("bootstrap_locales=fedcba98"));
        assert!(hud.status_text.contains("bootstrap_teams=2"));
        assert!(hud.status_text.contains("bootstrap_markers=4"));
        assert!(hud.status_text.contains("bootstrap_chunks=1"));
        assert!(hud.status_text.contains("bootstrap_patches=3"));
        assert!(hud.status_text.contains("bootstrap_plans=5"));
        assert!(hud.status_text.contains("bootstrap_fog_teams=1"));
        assert!(hud.status_text.contains("runtime_tilecfg_events=0"));
        assert!(hud.status_text.contains("runtime_tilecfg_parse_fail=0"));
        assert!(hud.status_text.contains("runtime_tilecfg_noapply=0"));
        assert!(hud.status_text.contains("runtime_tilecfg_rollback=0"));
        assert!(hud
            .status_text
            .contains("runtime_tilecfg_pending_mismatch=0"));
        assert!(hud.status_text.contains("runtime_take_items=1"));
        assert!(hud.status_text.contains("runtime_transfer_item=2"));
        assert!(hud.status_text.contains("runtime_transfer_item_unit=3"));
        assert!(hud.status_text.contains("runtime_payload_drop=4"));
        assert!(hud.status_text.contains("runtime_payload_pick_build=5"));
        assert!(hud.status_text.contains("runtime_payload_pick_unit=6"));
        assert!(hud.status_text.contains("runtime_unit_entered_payload=7"));
        assert!(hud.status_text.contains("runtime_unit_despawn=8"));
        assert!(hud.status_text.contains(&format!(
            "runtime_unit_lifecycle=bd66@{}:ud67@701:ux68@702:uy69@2:703:us70@1:{}:uc71@2:704",
            pack_runtime_point2(3, 12),
            pack_runtime_point2(11, 12),
        )));
        assert!(hud.status_text.contains(&format!(
            "runtime_spawn_fx=cw72@5:se73@19:lx79@2:1011:us74@404/36#3:ubs75@{}:utbs76@{}#404",
            pack_runtime_point2(4, 15),
            pack_runtime_point2(8, 3),
        )));
        assert!(hud
            .status_text
            .contains("runtime_audio=snd54@7:sf74:sat55@11:saf75"));
        assert!(hud
            .status_text
            .contains("runtime_admin=trace56@123456:tf76:dbgr57:drf77:dbgu58@12:duf78"));
        assert!(hud.status_text.contains(
            "runtime_loading=defer59:replay60:drop61:qdrop0:sfail62:scfail63:efail64:rdy65@66"
        ));
        assert!(hud.status_text.contains(
            "runtime_rules=sr67:srf68:so69:sof70:rule71:rf72:clr73:cmp74:wv1:pvp0:obj2:q1:par1:fg2:oor75:last9"
        ));
        assert!(hud.status_text.contains(
            "runtime_ui_notice=hud9:hudr10:hide11:ann12:info13:toast14:warn15:popup0:popr0:clip51@copied#6:uri52@https_//exam~#19:https"
        ));
        assert!(hud
            .status_text
            .contains("runtime_ui_menu=menu16:fmenu17:hfm18:tin53@404:Digits:12345#16:n1:e1"));
        assert!(!hud
            .status_text
            .contains("runtime_ui_menu=menu16:fmenu17:hfm18:tin53@404:Digits:Only_numbers"));
        let runtime_ui = hud
            .runtime_ui
            .as_ref()
            .expect("runtime_ui observability should be present");
        assert_eq!(runtime_ui.hud_text.set_count, 9);
        assert_eq!(runtime_ui.hud_text.set_reliable_count, 10);
        assert_eq!(runtime_ui.hud_text.hide_count, 11);
        assert_eq!(runtime_ui.hud_text.last_message.as_deref(), Some("hud"));
        assert_eq!(
            runtime_ui.hud_text.last_reliable_message.as_deref(),
            Some("hud reliable")
        );
        assert_eq!(runtime_ui.toast.info_count, 14);
        assert_eq!(runtime_ui.toast.warning_count, 15);
        assert_eq!(runtime_ui.toast.last_info_message.as_deref(), Some("toast"));
        assert_eq!(
            runtime_ui.toast.last_warning_text.as_deref(),
            Some("warning")
        );
        assert_eq!(runtime_ui.text_input.open_count, 53);
        assert_eq!(runtime_ui.text_input.last_id, Some(404));
        assert_eq!(runtime_ui.text_input.last_title.as_deref(), Some("Digits"));
        assert_eq!(
            runtime_ui.text_input.last_message.as_deref(),
            Some("Only numbers")
        );
        assert_eq!(
            runtime_ui.text_input.last_default_text.as_deref(),
            Some("12345")
        );
        assert_eq!(runtime_ui.text_input.last_length, Some(16));
        assert_eq!(runtime_ui.text_input.last_numeric, Some(true));
        assert_eq!(runtime_ui.text_input.last_allow_empty, Some(true));
        assert!(hud
            .status_text
            .contains("runtime_world_label=lbl19:lblr20:rml21"));
        assert!(hud
            .status_text
            .contains("runtime_marker=cr54:rm55:up56:txt57:tex58:fail2:last808:flushText"));
        assert!(hud.status_text.contains(&format!(
            "runtime_logic_sync=ov59@17:2:{}:sv60@{}:4:string",
            pack_runtime_point2(5, 6),
            pack_runtime_point2(9, 10),
        )));
        assert!(hud.status_text.contains(&format!(
            "runtime_resource_delta=rmt80:st81:sf82:so83:seti22:setis23:setl24:setls25:cli84:cll85:sti26:stl27:tk1:tb2:tu3:to_unit@6#none:bpnone:unone:eid404:b2:bs3:e1:au4:da5:sk6:cf7:lb{}:le900:li6:la1",
            pack_runtime_point2(9, 9),
        )));
        assert!(hud
            .status_text
            .contains("runtime_command_ctrl=spte28@t7:mc29@404/2:tir30@405#len5:ri31@6:7#9x12:bcs32@10:11:ucl33:uct34@2:404:ubcs35@1:505/12:13:cb36@n2:14:15->0x3fc00000:0x40200000:cu37@n2:u700:b16:17:t2:808:p0x40600000:0x40900000:q1:f0:suc38@n3:u701:c9:sus39@n4:u702:s5:e1:rot40@18:19:d0:tinv41@20:21:rbp42@22:23:rdp46@0x40b00000:0x40d00000:rup43@1:909:drop44@0x40f00000:dpl45@n3:24:25:tap47@26:27"));
        assert!(hud
            .status_text
            .contains("runtime_gameplay_signal=flag46:go47:ugo48:sc49:res50"));
    }

    #[test]
    fn render_runtime_adapter_prefers_authoritative_state_mirror_in_hud() {
        let adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.last_state_snapshot = Some(crate::session_state::AppliedStateSnapshot {
            wave: 5,
            enemies: 1,
            tps: 120,
            ..Default::default()
        });
        state.last_state_snapshot_core_data =
            Some(crate::session_state::AppliedStateSnapshotCoreData {
                team_count: 3,
                teams: vec![
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 0,
                            amount: 7,
                        }],
                    },
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 9,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 2,
                            amount: 8,
                        }],
                    },
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 11,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 4,
                            amount: 9,
                        }],
                    },
                ],
            });
        state.last_good_state_snapshot_core_data =
            Some(crate::session_state::AppliedStateSnapshotCoreData {
                team_count: 3,
                teams: vec![
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 0,
                            amount: 7,
                        }],
                    },
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 9,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 2,
                            amount: 8,
                        }],
                    },
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 10,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 3,
                            amount: 9,
                        }],
                    },
                ],
            });
        state.state_snapshot_business_projection =
            Some(crate::session_state::StateSnapshotBusinessProjection {
                wave_time_bits: 0,
                wave: 7,
                enemies: 3,
                paused: false,
                game_over: false,
                time_data: 99,
                tps: 60,
                rand0: 1,
                rand1: 2,
                gameplay_state: crate::session_state::GameplayStateProjection::Playing,
                gameplay_state_transition_count: 1,
                last_wave_advanced: true,
                last_wave_advance_from: Some(6),
                last_wave_advance_to: Some(7),
                wave_advance_count: 2,
                net_seconds_applied_count: 2,
                last_net_seconds_rollback: false,
                net_seconds_delta: 1,
                state_snapshot_apply_count: 2,
                state_snapshot_time_regress_count: 0,
                state_snapshot_wave_regress_count: 0,
                core_inventory_synced: true,
                core_inventory_team_count: 1,
                core_inventory_item_entry_count: 2,
                core_inventory_total_amount: 20,
                core_inventory_nonzero_item_count: 2,
                core_inventory_changed_team_count: 1,
                core_inventory_changed_team_sample: vec![1],
                core_inventory_by_team: BTreeMap::from([(
                    1,
                    BTreeMap::from([(0u16, 10), (1u16, 10)]),
                )]),
            });
        state.authoritative_state_mirror = Some(crate::session_state::AuthoritativeStateMirror {
            wave_time_bits: 0,
            wave: 11,
            enemies: 6,
            paused: false,
            game_over: true,
            net_seconds: 120,
            tps: 24,
            rand0: 30,
            rand1: 40,
            gameplay_state: crate::session_state::GameplayStateProjection::GameOver,
            last_wave_advanced: true,
            wave_advance_count: 5,
            apply_count: 5,
            last_net_seconds_rollback: false,
            net_seconds_delta: 12,
            wave_regress_count: 1,
            core_inventory_team_count: 3,
            core_inventory_item_entry_count: 5,
            core_inventory_total_amount: 77,
            core_inventory_nonzero_item_count: 5,
            core_inventory_changed_team_count: 2,
            core_inventory_changed_team_sample: vec![2, 7],
            core_inventory_by_team: BTreeMap::from([
                (2, BTreeMap::from([(0u16, 11), (1u16, 12)])),
                (7, BTreeMap::from([(2u16, 13)])),
                (9, BTreeMap::from([(3u16, 20), (4u16, 21)])),
            ]),
            last_core_sync_ok: true,
            core_parse_fail_count: 0,
        });
        state.state_snapshot_authority_projection =
            Some(crate::session_state::StateSnapshotAuthorityProjection {
                wave_time_bits: 0,
                wave: 9,
                enemies: 5,
                paused: true,
                game_over: false,
                time_data: 110,
                tps: 30,
                rand0: 3,
                rand1: 4,
                gameplay_state: crate::session_state::GameplayStateProjection::Paused,
                last_wave_advanced: true,
                wave_advance_count: 4,
                state_snapshot_apply_count: 4,
                last_net_seconds_rollback: false,
                net_seconds_delta: 11,
                state_snapshot_wave_regress_count: 0,
                core_inventory_team_count: 2,
                core_inventory_item_entry_count: 4,
                core_inventory_total_amount: 42,
                core_inventory_nonzero_item_count: 4,
                core_inventory_changed_team_count: 2,
                core_inventory_changed_team_sample: vec![1, 8],
                core_inventory_by_team: BTreeMap::from([
                    (1, BTreeMap::from([(0u16, 11), (1u16, 12)])),
                    (8, BTreeMap::from([(2u16, 9), (3u16, 10)])),
                ]),
                last_core_sync_ok: true,
                core_parse_fail_count: 0,
            });

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud.status_text.contains("runtime_wave=11"));
        assert!(hud.status_text.contains("runtime_enemies=6"));
        assert!(hud.status_text.contains("runtime_tps=24"));
        assert!(hud.status_text.contains(
            "runtime_state_apply=w11:e6:t24:c3/5:adv1:core1:sgameover:nd12:tr0:wreg1:ca2:cas2,7"
        ));
        assert!(hud.status_text.contains("runtime_core_teams=3"));
        assert!(hud.status_text.contains("runtime_core_items=5"));
    }

    #[test]
    fn render_runtime_adapter_falls_back_to_last_good_state_snapshot_core_data() {
        let adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.last_state_snapshot = Some(crate::session_state::AppliedStateSnapshot {
            wave: 4,
            enemies: 2,
            tps: 60,
            ..Default::default()
        });
        state.last_state_snapshot_core_data = None;
        state.last_good_state_snapshot_core_data =
            Some(crate::session_state::AppliedStateSnapshotCoreData {
                team_count: 2,
                teams: vec![
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![
                            crate::session_state::AppliedStateSnapshotCoreDataItem {
                                item_id: 0,
                                amount: 3,
                            },
                            crate::session_state::AppliedStateSnapshotCoreDataItem {
                                item_id: 1,
                                amount: 4,
                            },
                        ],
                    },
                    crate::session_state::AppliedStateSnapshotCoreDataTeam {
                        team_id: 4,
                        items: vec![crate::session_state::AppliedStateSnapshotCoreDataItem {
                            item_id: 6,
                            amount: 9,
                        }],
                    },
                ],
            });
        state.failed_state_snapshot_core_data_parse_count = 1;

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud.status_text.contains("runtime_core_teams=2"));
        assert!(hud.status_text.contains("runtime_core_items=3"));
    }

    #[test]
    fn runtime_entity_gate_label_compacts_tombstone_skip_observability() {
        let mut state = SessionState::default();
        assert_eq!(runtime_entity_gate_label(&state), "ts0:a0");

        state.entity_snapshot_tombstone_skip_count = 2;
        state.last_entity_snapshot_tombstone_skipped_ids_sample = vec![8, 9];
        state.entity_snapshot_tombstones = BTreeMap::from([(8, 4)]);
        assert_eq!(runtime_entity_gate_label(&state), "ts2@8,9:a1");

        state.entity_snapshot_tombstone_skip_count = 5;
        assert_eq!(runtime_entity_gate_label(&state), "ts5@8,9+3:a1");
    }

    #[test]
    fn runtime_entity_sync_label_compacts_local_player_sync_observability() {
        let mut state = SessionState::default();
        assert_eq!(
            runtime_entity_sync_label(&state),
            "lt0:tpnone:ok0:amb0@0:miss0:fail0"
        );

        state.entity_snapshot_with_local_target_count = 3;
        state.last_entity_snapshot_target_player_id = Some(44);
        state.last_entity_snapshot_local_player_sync_applied = true;
        state.last_entity_snapshot_local_player_sync_ambiguous = false;
        state.last_entity_snapshot_local_player_sync_match_count = 1;
        state.missed_local_player_sync_from_entity_snapshot_count = 2;
        state.failed_entity_snapshot_parse_count = 5;
        assert_eq!(
            runtime_entity_sync_label(&state),
            "lt3:tp44:ok1:amb0@1:miss2:fail5"
        );
    }

    #[test]
    fn runtime_command_control_label_compacts_last_targets_and_batches() {
        let mut state = SessionState::default();
        state.received_set_player_team_editor_count = 1;
        state.last_set_player_team_editor_team_id = Some(9);
        state.received_menu_choose_count = 2;
        state.last_menu_choose_menu_id = Some(300);
        state.last_menu_choose_option = Some(4);
        state.received_text_input_result_count = 3;
        state.last_text_input_result_id = Some(301);
        state.last_text_input_result_text = Some("ready".to_string());
        state.received_request_item_count = 4;
        state.last_request_item_build_pos = Some(pack_runtime_point2(1, 2));
        state.last_request_item_item_id = Some(6);
        state.last_request_item_amount = Some(7);
        state.received_building_control_select_count = 5;
        state.last_building_control_select_build_pos = Some(pack_runtime_point2(3, 4));
        state.received_unit_clear_count = 6;
        state.received_unit_control_count = 7;
        state.last_unit_control_target = Some(crate::session_state::UnitRefProjection {
            kind: 2,
            value: 88,
        });
        state.received_unit_building_control_select_count = 8;
        state.last_unit_building_control_select_target =
            Some(crate::session_state::UnitRefProjection {
                kind: 1,
                value: 77,
            });
        state.last_unit_building_control_select_build_pos = Some(pack_runtime_point2(5, 6));
        state.received_command_building_count = 9;
        state.last_command_building_count = 2;
        state.last_command_building_first_build_pos = Some(pack_runtime_point2(7, 8));
        state.last_command_building_x_bits = Some(1.0f32.to_bits());
        state.last_command_building_y_bits = Some(2.0f32.to_bits());
        state.received_command_units_count = 10;
        state.last_command_units_count = 3;
        state.last_command_units_first_unit_id = Some(500);
        state.last_command_units_build_target = Some(pack_runtime_point2(9, 10));
        state.last_command_units_unit_target = Some(crate::session_state::UnitRefProjection {
            kind: 2,
            value: 600,
        });
        state.last_command_units_x_bits = Some(3.0f32.to_bits());
        state.last_command_units_y_bits = Some(4.0f32.to_bits());
        state.last_command_units_queue = Some(false);
        state.last_command_units_final_batch = Some(true);
        state.received_set_unit_command_count = 11;
        state.last_set_unit_command_count = 4;
        state.last_set_unit_command_first_unit_id = Some(501);
        state.last_set_unit_command_id = Some(12);
        state.received_set_unit_stance_count = 13;
        state.last_set_unit_stance_count = 5;
        state.last_set_unit_stance_first_unit_id = Some(502);
        state.last_set_unit_stance_id = Some(14);
        state.last_set_unit_stance_enable = Some(false);
        state.received_rotate_block_count = 15;
        state.last_rotate_block_build_pos = Some(pack_runtime_point2(11, 12));
        state.last_rotate_block_direction = Some(true);
        state.received_transfer_inventory_count = 16;
        state.last_transfer_inventory_build_pos = Some(pack_runtime_point2(13, 14));
        state.received_request_build_payload_count = 17;
        state.last_request_build_payload_build_pos = Some(pack_runtime_point2(15, 16));
        state.received_request_drop_payload_count = 18;
        state.last_request_drop_payload_x_bits = Some(5.0f32.to_bits());
        state.last_request_drop_payload_y_bits = Some(6.0f32.to_bits());
        state.received_request_unit_payload_count = 19;
        state.last_request_unit_payload_target = Some(crate::session_state::UnitRefProjection {
            kind: 1,
            value: 700,
        });
        state.received_drop_item_count = 20;
        state.last_drop_item_angle_bits = Some(7.0f32.to_bits());
        state.received_delete_plans_count = 21;
        state.last_delete_plans_count = 6;
        state.last_delete_plans_first_pos = Some(pack_runtime_point2(17, 18));
        state.received_tile_tap_count = 22;
        state.last_tile_tap_pos = Some(pack_runtime_point2(19, 20));

        assert_eq!(
            runtime_command_control_label(&state),
            "spte1@t9:mc2@300/4:tir3@301#len5:ri4@1:2#6x7:bcs5@3:4:ucl6:uct7@2:88:ubcs8@1:77/5:6:cb9@n2:7:8->0x3f800000:0x40000000:cu10@n3:u500:b9:10:t2:600:p0x40400000:0x40800000:q0:f1:suc11@n4:u501:c12:sus13@n5:u502:s14:e0:rot15@11:12:d1:tinv16@13:14:rbp17@15:16:rdp18@0x40a00000:0x40c00000:rup19@1:700:drop20@0x40e00000:dpl21@n6:17:18:tap22@19:20"
        );
    }

    #[test]
    fn runtime_loading_label_compacts_timeout_reset_and_world_reload_taxonomy() {
        let mut state = SessionState::default();
        assert_eq!(
            runtime_loading_label(&state),
            "defer0:replay0:drop0:qdrop0:sfail0:scfail0:efail0:rdy0@none:to0:cto0:rto0:ltnone@none:rs0:rr0:wr0:kr0:lrnone:lwrnone"
        );

        state.deferred_inbound_packet_count = 5;
        state.replayed_inbound_packet_count = 6;
        state.dropped_loading_low_priority_packet_count = 7;
        state.dropped_loading_deferred_overflow_count = 8;
        state.failed_state_snapshot_parse_count = 9;
        state.failed_state_snapshot_core_data_parse_count = 10;
        state.failed_entity_snapshot_parse_count = 11;
        state.ready_inbound_liveness_anchor_count = 12;
        state.last_ready_inbound_liveness_anchor_at_ms = Some(1300);
        state.timeout_count = 2;
        state.connect_or_loading_timeout_count = 1;
        state.ready_snapshot_timeout_count = 1;
        state.last_timeout = Some(crate::session_state::SessionTimeoutProjection {
            kind: SessionTimeoutKind::ReadySnapshotStall,
            idle_ms: 20000,
        });
        state.reset_count = 3;
        state.reconnect_reset_count = 1;
        state.world_reload_count = 1;
        state.kick_reset_count = 1;
        state.last_reset_kind = Some(SessionResetKind::WorldReload);
        state.last_world_reload = Some(WorldReloadProjection {
            had_loaded_world: true,
            had_client_loaded: false,
            was_ready_to_enter_world: true,
            had_connect_confirm_sent: false,
            cleared_pending_packets: 4,
            cleared_deferred_inbound_packets: 5,
            cleared_replayed_loading_events: 6,
        });

        assert_eq!(
            runtime_loading_label(&state),
            "defer5:replay6:drop7:qdrop8:sfail9:scfail10:efail11:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
        );
    }

    #[test]
    fn render_runtime_adapter_reports_session_taxonomy_in_loading_hud() {
        let adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.timeout_count = 4;
        state.connect_or_loading_timeout_count = 3;
        state.ready_snapshot_timeout_count = 1;
        state.last_timeout = Some(crate::session_state::SessionTimeoutProjection {
            kind: SessionTimeoutKind::ConnectOrLoading,
            idle_ms: 300000,
        });
        state.reset_count = 5;
        state.reconnect_reset_count = 2;
        state.world_reload_count = 2;
        state.kick_reset_count = 1;
        state.last_reset_kind = Some(SessionResetKind::Kick);
        state.last_world_reload = Some(WorldReloadProjection {
            had_loaded_world: true,
            had_client_loaded: true,
            was_ready_to_enter_world: false,
            had_connect_confirm_sent: true,
            cleared_pending_packets: 7,
            cleared_deferred_inbound_packets: 8,
            cleared_replayed_loading_events: 9,
        });

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud.status_text.contains(
            "runtime_loading=defer0:replay0:drop0:qdrop0:sfail0:scfail0:efail0:rdy0@none:to4:cto3:rto1:ltcload@300000:rs5:rr2:wr2:kr1:lrkick:lwr@lw1:cl1:rd0:cc1:p7:d8:r9"
        ));
    }
}
