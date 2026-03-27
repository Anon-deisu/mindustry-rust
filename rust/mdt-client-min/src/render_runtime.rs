#[path = "render_runtime/effect_contract_executor.rs"]
mod effect_contract_executor;

use crate::client_session::{
    BuildHealthPair, BuildingLiveStateView, ClientBuildPlan, ClientBuildPlanConfig, ClientSession,
    ClientSessionEvent, ClientSnapshotInputState, StateSnapshotAppliedProjection,
};
use crate::effect_data_runtime::EffectDataBusinessHint;
use crate::effect_runtime::{
    effect_contract, observe_runtime_effect_overlay_binding_state,
    observe_runtime_effect_overlay_source_binding_state, resolve_runtime_effect_overlay_position,
    resolve_runtime_effect_overlay_source_position, spawn_runtime_effect_overlay,
    EffectRuntimeInputView, RuntimeEffectBinding, RuntimeEffectOverlay,
};
use crate::session_state::{
    AuthoritativeStateMirror, BuilderPlanStage, BuilderQueueProjection, BuildingProjection,
    BuildingProjectionUpdateKind, BuildingTableProjection, ConfiguredBlockOutcome,
    ConfiguredBlockProjection, ConfiguredContentRef, CoreInventoryRuntimeBindingKind,
    EffectBusinessContentKind, EffectBusinessPositionSource, EffectBusinessProjection,
    EffectDataSemantic, EffectRuntimeBindingState, HiddenSnapshotDeltaProjection,
    PayloadLoaderRuntimeProjection, PayloadRouterPayloadKind, ReconnectPhaseProjection,
    ReconnectReasonKind, SessionResetKind, SessionState, SessionTimeoutKind,
    StateSnapshotAuthorityProjection, StateSnapshotBusinessProjection, TileConfigAuthoritySource,
    TileConfigProjection, TypedBuildingRuntimeKind, TypedBuildingRuntimeModel,
    TypedBuildingRuntimeProjection, TypedBuildingRuntimeValue, TypedRuntimeEntityModel,
    TypedRuntimeEntityProjection, UnitAssemblerRuntimeProjection, UnitFactoryRuntimeProjection,
    UnitRefProjection, WorldBootstrapProjection, WorldReloadProjection,
};
use mdt_remote::{HighFrequencyRemoteMethod, HIGH_FREQUENCY_REMOTE_METHOD_COUNT};
use mdt_render_ui::hud_model::{
    RuntimeChatObservability, RuntimeCoreBindingKindObservability, RuntimeCoreBindingObservability,
    RuntimeKickObservability, RuntimeLoadingObservability, RuntimeMarkerObservability,
    RuntimeReconnectObservability, RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind,
    RuntimeResourceDeltaObservability, RuntimeSessionObservability, RuntimeSessionResetKind,
    RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
};
use mdt_render_ui::{
    BuildConfigAuthoritySourceObservability, BuildConfigInspectorEntryObservability,
    BuildConfigOutcomeObservability, BuildConfigRollbackStripObservability,
    BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudModel, RenderModel,
    RenderObject, RuntimeAdminObservability, RuntimeCommandUnitRefObservability,
    RuntimeHudTextObservability, RuntimeLiveEffectPositionSource,
    RuntimeLiveEffectSummaryObservability, RuntimeLiveEntitySummaryObservability,
    RuntimeLiveSummaryObservability, RuntimeMenuObservability, RuntimeRulesObservability,
    RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
    RuntimeWorldLabelObservability, RuntimeWorldPositionObservability,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const EFFECT_OVERLAY_LIMIT: usize = 8;
const DEFAULT_EFFECT_OVERLAY_TTL_TICKS: u8 = 3;
const DEFAULT_EFFECT_OVERLAY_CLIP_SIZE: f32 = 50.0;
const WORLD_LABEL_OVERLAY_LIMIT: usize = 16;
const WORLD_EVENT_MARKER_OVERLAY_LIMIT: usize = 24;
const CREATE_BULLET_MARKER_TTL_TICKS: u8 = 5;
const LOGIC_EXPLOSION_MARKER_TTL_TICKS: u8 = 8;
const SOUND_AT_MARKER_TTL_TICKS: u8 = 4;
const TILE_WORLD_ACTION_MARKER_TTL_TICKS: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeEffectClipView {
    pub center: (f32, f32),
    pub size: (f32, f32),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderRuntimeAdapter {
    world_overlay: RuntimeWorldOverlay,
}

impl RenderRuntimeAdapter {
    pub fn observe_events(&mut self, events: &[ClientSessionEvent]) {
        self.observe_events_with_view(events, None);
    }

    pub fn observe_events_with_view(
        &mut self,
        events: &[ClientSessionEvent],
        clip_view: Option<RuntimeEffectClipView>,
    ) {
        advance_runtime_effect_overlays(&mut self.world_overlay);
        advance_runtime_world_label_overlays(&mut self.world_overlay);
        advance_runtime_world_event_markers(&mut self.world_overlay);
        observe_runtime_world_events(&mut self.world_overlay, events, clip_view);
    }

    pub fn apply(
        &mut self,
        scene: &mut RenderModel,
        hud: &mut HudModel,
        snapshot_input: &ClientSnapshotInputState,
        session_state: &SessionState,
    ) {
        let runtime_typed_building_projection = session_state.runtime_typed_building_projection();
        let runtime_buildings_label =
            runtime_building_table_label(&session_state.building_table_projection);
        self.apply_with_building_view(
            scene,
            hud,
            snapshot_input,
            session_state,
            &session_state.building_table_projection.by_build_pos,
            &runtime_typed_building_projection,
            runtime_buildings_label.as_str(),
        );
    }

    pub fn apply_with_client_session(
        &mut self,
        scene: &mut RenderModel,
        hud: &mut HudModel,
        snapshot_input: &ClientSnapshotInputState,
        session: &ClientSession,
    ) {
        let session_state = session.state();
        let building_live_state = session.building_live_state_projection();
        let building_projection_by_build_pos =
            live_building_projection_by_build_pos(&building_live_state);
        let runtime_typed_building_projection =
            live_runtime_typed_building_projection(&building_live_state);
        let runtime_buildings_label = runtime_building_live_state_label(
            &building_live_state,
            &session_state.building_table_projection,
        );
        self.apply_with_building_view(
            scene,
            hud,
            snapshot_input,
            session_state,
            &building_projection_by_build_pos,
            &runtime_typed_building_projection,
            runtime_buildings_label.as_str(),
        );
    }

    fn apply_with_building_view(
        &mut self,
        scene: &mut RenderModel,
        hud: &mut HudModel,
        snapshot_input: &ClientSnapshotInputState,
        session_state: &SessionState,
        building_projection_by_build_pos: &BTreeMap<i32, BuildingProjection>,
        runtime_typed_building_projection: &TypedBuildingRuntimeProjection,
        runtime_buildings_label: &str,
    ) {
        cull_hidden_parent_unit_effect_overlays(&mut self.world_overlay, session_state);
        let config_stats = runtime_build_plan_config_stats(snapshot_input.plans.as_deref());
        append_runtime_build_plan_objects(scene, snapshot_input.plans.as_deref());
        append_runtime_world_overlay_objects(
            scene,
            &mut self.world_overlay,
            snapshot_input,
            session_state,
        );
        append_runtime_ping_location_objects(scene, session_state);
        append_runtime_command_mode_overlay_objects(scene, snapshot_input, session_state);
        append_building_projection_objects(
            scene,
            building_projection_by_build_pos,
            runtime_typed_building_projection,
            &session_state.configured_block_projection,
        );
        append_block_snapshot_projection_objects(scene, session_state);
        append_runtime_live_entity_objects(scene, session_state);
        let bootstrap_projection = session_state.world_bootstrap_projection.as_ref();
        let runtime_state_mirror = session_state.authoritative_state_mirror.as_ref();
        let state_authority_projection = session_state.state_snapshot_authority_projection.as_ref();
        let state_business_projection = session_state.state_snapshot_business_projection.as_ref();
        hud.runtime_ui = Some(runtime_ui_observability(
            snapshot_input,
            session_state,
            &self.world_overlay,
        ));
        hud.build_ui = Some(runtime_build_ui_observability(
            snapshot_input,
            &session_state.builder_queue_projection,
            &session_state.tile_config_projection,
            runtime_typed_building_projection,
        ));
        hud.status_text = format!(
            "{} runtime_selected={} runtime_plans={} runtime_cfg_int={} runtime_cfg_long={} runtime_cfg_float={} runtime_cfg_bool={} runtime_cfg_int_seq={} runtime_cfg_point2={} runtime_cfg_point2_array={} runtime_cfg_tech_node={} runtime_cfg_double={} runtime_cfg_building_pos={} runtime_cfg_laccess={} runtime_cfg_string={} runtime_cfg_bytes={} runtime_cfg_legacy_unit_command_null={} runtime_cfg_bool_array={} runtime_cfg_unit_id={} runtime_cfg_vec2_array={} runtime_cfg_vec2={} runtime_cfg_team={} runtime_cfg_int_array={} runtime_cfg_object_array={} runtime_cfg_content={} runtime_cfg_unit_command={} runtime_world_tiles={} runtime_health={} building={} runtime_builder={} runtime_builder_head={} runtime_entity_local={} runtime_entity_hidden={} runtime_entity_gate={} runtime_entity_sync={} runtime_snap_last={} runtime_snap_events={} runtime_snap_apply={} runtime_wave={} runtime_enemies={} runtime_tps={} runtime_state_apply={} runtime_core_teams={} runtime_core_items={} runtime_core_binding={} runtime_buildings={} runtime_block={} runtime_block_fail={} runtime_hidden={} runtime_hidden_delta={} runtime_hidden_fail={} runtime_effects={} runtime_effect_data_kind={} runtime_effect_contract={} runtime_effect_data_semantic={} runtime_effect_data_hint={} runtime_effect_apply={} runtime_effect_path={} runtime_effect_binding={} runtime_effect_data_fail={} bootstrap_rules={} bootstrap_tags={} bootstrap_locales={} bootstrap_teams={} bootstrap_markers={} bootstrap_chunks={} bootstrap_patches={} bootstrap_plans={} bootstrap_fog_teams={} runtime_view_center={} runtime_view_size={} runtime_position={} runtime_pointer={} runtime_selected_rotation={} runtime_input_flags={} runtime_snap_client={} runtime_snap_state={} runtime_snap_entity={} runtime_snap_block={} runtime_snap_hidden={} runtime_tilecfg_events={} runtime_tilecfg_parse_fail={} runtime_tilecfg_noapply={} runtime_tilecfg_rollback={} runtime_tilecfg_pending_mismatch={} runtime_tilecfg_apply={} runtime_configured={} runtime_take_items={} runtime_transfer_item={} runtime_transfer_item_unit={} runtime_payload_drop={} runtime_payload_pick_build={} runtime_payload_pick_unit={} runtime_unit_entered_payload={} runtime_unit_despawn={} runtime_unit_lifecycle={} runtime_spawn_fx={} runtime_audio={} runtime_admin={} runtime_kick={} runtime_loading={} runtime_rules={} runtime_ui_notice={} runtime_ui_menu={} runtime_chat={} runtime_world_label={} runtime_marker={} runtime_logic_sync={} runtime_resource_delta={} runtime_command_ctrl={} runtime_gameplay_signal={}",
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
            runtime_core_binding_label(session_state),
            runtime_buildings_label,
            runtime_block_snapshot_label(session_state),
            session_state.failed_block_snapshot_parse_count,
            runtime_hidden_snapshot_label(session_state),
            runtime_hidden_snapshot_delta_label(session_state),
            session_state.failed_hidden_snapshot_parse_count,
            session_state.received_effect_count,
            runtime_effect_data_kind_label(session_state.last_effect_data_kind.as_deref()),
            runtime_effect_contract_label(session_state),
            runtime_effect_data_semantic_label(session_state.last_effect_data_semantic.as_ref()),
            runtime_effect_business_hint_label(
                session_state.last_effect_data_business_hint.as_ref(),
            ),
            runtime_effect_business_projection_label(
                session_state.last_effect_business_projection.as_ref(),
            ),
            runtime_effect_path_label(session_state.last_effect_business_path.as_deref()),
            runtime_effect_binding_label(snapshot_input, session_state, &self.world_overlay),
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
            runtime_chat_label(session_state),
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

fn live_building_projection_by_build_pos(
    live_state: &BTreeMap<i32, BuildingLiveStateView>,
) -> BTreeMap<i32, BuildingProjection> {
    live_state
        .iter()
        .map(|(&build_pos, building)| (build_pos, building.projection.clone()))
        .collect()
}

fn live_runtime_typed_building_projection(
    live_state: &BTreeMap<i32, BuildingLiveStateView>,
) -> TypedBuildingRuntimeProjection {
    TypedBuildingRuntimeProjection {
        by_build_pos: live_state
            .values()
            .filter_map(|building| {
                building
                    .runtime
                    .clone()
                    .map(|runtime| (runtime.build_pos, runtime))
            })
            .collect(),
    }
}

fn runtime_building_live_state_label(
    live_state: &BTreeMap<i32, BuildingLiveStateView>,
    projection: &BuildingTableProjection,
) -> String {
    let mut merged = projection.clone();
    merged.by_build_pos = live_building_projection_by_build_pos(live_state);
    merged.block_known_count = merged
        .by_build_pos
        .values()
        .filter(|building| building.block_id.is_some())
        .count();
    merged.configured_count = merged
        .by_build_pos
        .values()
        .filter(|building| building.config.is_some())
        .count();
    match merged
        .last_build_pos
        .and_then(|build_pos| live_state.get(&build_pos))
    {
        Some(building) => {
            let projection = &building.projection;
            merged.last_block_id = projection.block_id;
            merged.last_block_name = projection.block_name.clone();
            merged.last_rotation = projection.rotation;
            merged.last_team_id = projection.team_id;
            merged.last_io_version = projection.io_version;
            merged.last_module_bitmask = projection.module_bitmask;
            merged.last_time_scale_bits = projection.time_scale_bits;
            merged.last_time_scale_duration_bits = projection.time_scale_duration_bits;
            merged.last_last_disabler_pos = projection.last_disabler_pos;
            merged.last_legacy_consume_connected = projection.legacy_consume_connected;
            merged.last_config = projection.config.clone();
            merged.last_health_bits = projection.health_bits;
            merged.last_enabled = projection.enabled;
            merged.last_efficiency = projection.efficiency;
            merged.last_optional_efficiency = projection.optional_efficiency;
            merged.last_visible_flags = projection.visible_flags;
            merged.last_turret_reload_counter_bits = projection.turret_reload_counter_bits;
            merged.last_turret_rotation_bits = projection.turret_rotation_bits;
            merged.last_item_turret_ammo_count = projection.item_turret_ammo_count;
            merged.last_continuous_turret_last_length_bits =
                projection.continuous_turret_last_length_bits;
            merged.last_build_turret_rotation_bits = projection.build_turret_rotation_bits;
            merged.last_build_turret_plans_present = projection.build_turret_plans_present;
            merged.last_build_turret_plan_count = projection.build_turret_plan_count;
            merged.last_update = Some(projection.last_update);
            merged.last_removed = false;
        }
        None if merged.last_build_pos.is_some() => {
            merged.last_removed = true;
        }
        None => {}
    }
    runtime_building_table_label(&merged)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeWorldOverlay {
    pub tile_overlays: BTreeMap<(i32, i32), RuntimeTileOverlay>,
    pub effect_overlays: Vec<RuntimeEffectOverlay>,
    pub world_label_overlays: Vec<RuntimeWorldLabelOverlay>,
    pub next_world_label_overlay_key: u64,
    pub world_event_markers: Vec<RuntimeWorldEventMarkerOverlay>,
    pub next_world_event_marker_key: u64,
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
        self.world_label_overlays.clear();
        self.next_world_label_overlay_key = 0;
        self.world_event_markers.clear();
        self.next_world_event_marker_key = 0;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorldLabelOverlay {
    pub overlay_key: u64,
    pub label_id: Option<i32>,
    pub reliable: bool,
    pub message: Option<String>,
    pub x_bits: u32,
    pub y_bits: u32,
    pub remaining_ticks: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorldEventMarkerOverlay {
    pub overlay_key: u64,
    pub object_id: String,
    pub layer: i32,
    pub x_bits: u32,
    pub y_bits: u32,
    pub remaining_ticks: u8,
}

pub fn observe_runtime_world_events(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    events: &[ClientSessionEvent],
    clip_view: Option<RuntimeEffectClipView>,
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
            ClientSessionEvent::WorldLabel {
                reliable,
                label_id,
                message,
                duration,
                world_x,
                world_y,
            } => upsert_runtime_world_label_overlay(
                runtime_world_overlay,
                *label_id,
                *reliable,
                message.clone(),
                *duration,
                *world_x,
                *world_y,
            ),
            ClientSessionEvent::RemoveWorldLabel { label_id } => {
                runtime_world_overlay
                    .world_label_overlays
                    .retain(|overlay| overlay.label_id != Some(*label_id));
            }
            ClientSessionEvent::SoundAtRequested { sound_id, x, y, .. } => {
                if !runtime_effect_event_inside_clip_view(None, *x, *y, clip_view) {
                    continue;
                }
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-sound-at:{overlay_key}:{}",
                            sound_id.unwrap_or(-1)
                        ),
                        layer: 28,
                        x_bits: x.to_bits(),
                        y_bits: y.to_bits(),
                        remaining_ticks: SOUND_AT_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::UnitBlockSpawn { tile_pos } => {
                let Some(tile_pos) = *tile_pos else {
                    continue;
                };
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
                let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(tile_pos);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-unit-block-spawn:{overlay_key}:{tile_x}:{tile_y}"
                        ),
                        layer: 28,
                        x_bits,
                        y_bits,
                        remaining_ticks: TILE_WORLD_ACTION_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::UnitTetherBlockSpawned { tile_pos, unit_id } => {
                let Some(tile_pos) = *tile_pos else {
                    continue;
                };
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
                let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(tile_pos);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-unit-tether-block-spawned:{overlay_key}:{tile_x}:{tile_y}:{unit_id}"
                        ),
                        layer: 28,
                        x_bits,
                        y_bits,
                        remaining_ticks: TILE_WORLD_ACTION_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::AutoDoorToggle { tile_pos, open } => {
                let Some(tile_pos) = *tile_pos else {
                    continue;
                };
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
                let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(tile_pos);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-auto-door-toggle:{overlay_key}:{tile_x}:{tile_y}:{}",
                            u8::from(*open)
                        ),
                        layer: 28,
                        x_bits,
                        y_bits,
                        remaining_ticks: TILE_WORLD_ACTION_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::LandingPadLanded { tile_pos } => {
                let Some(tile_pos) = *tile_pos else {
                    continue;
                };
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
                let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(tile_pos);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-landing-pad-landed:{overlay_key}:{tile_x}:{tile_y}"
                        ),
                        layer: 28,
                        x_bits,
                        y_bits,
                        remaining_ticks: TILE_WORLD_ACTION_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::AssemblerDroneSpawned { tile_pos, unit_id } => {
                let Some(tile_pos) = *tile_pos else {
                    continue;
                };
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
                let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(tile_pos);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-assembler-drone-spawned:{overlay_key}:{tile_x}:{tile_y}:{unit_id}"
                        ),
                        layer: 28,
                        x_bits,
                        y_bits,
                        remaining_ticks: TILE_WORLD_ACTION_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::AssemblerUnitSpawned { tile_pos } => {
                let Some(tile_pos) = *tile_pos else {
                    continue;
                };
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
                let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(tile_pos);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-assembler-unit-spawned:{overlay_key}:{tile_x}:{tile_y}"
                        ),
                        layer: 28,
                        x_bits,
                        y_bits,
                        remaining_ticks: TILE_WORLD_ACTION_MARKER_TTL_TICKS,
                    },
                );
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
                if !runtime_effect_event_inside_clip_view(*effect_id, *x, *y, clip_view) {
                    continue;
                }
                let (overlay_x, overlay_y) = runtime_effect_overlay_origin(
                    *effect_id,
                    *x,
                    *y,
                    *rotation,
                    data_object.as_ref(),
                );
                let lifetime_ticks = runtime_effect_overlay_ttl_ticks(*effect_id);
                let mut overlay = spawn_runtime_effect_overlay(
                    *effect_id,
                    overlay_x,
                    overlay_y,
                    *x,
                    *y,
                    *rotation,
                    *color_rgba,
                    false,
                    data_object.as_ref(),
                    lifetime_ticks,
                );
                overlay.remaining_ticks = overlay
                    .remaining_ticks
                    .saturating_add(runtime_effect_overlay_start_delay_ticks(*effect_id));
                push_runtime_effect_overlay(runtime_world_overlay, overlay);
            }
            ClientSessionEvent::EffectReliableRequested {
                effect_id,
                x,
                y,
                rotation,
                color_rgba,
            } => {
                if !runtime_effect_event_inside_clip_view(*effect_id, *x, *y, clip_view) {
                    continue;
                }
                let mut overlay = spawn_runtime_effect_overlay(
                    *effect_id,
                    *x,
                    *y,
                    *x,
                    *y,
                    *rotation,
                    *color_rgba,
                    true,
                    None,
                    runtime_effect_overlay_ttl_ticks(*effect_id),
                );
                overlay.remaining_ticks = overlay
                    .remaining_ticks
                    .saturating_add(runtime_effect_overlay_start_delay_ticks(*effect_id));
                push_runtime_effect_overlay(runtime_world_overlay, overlay);
            }
            ClientSessionEvent::SpawnEffect {
                x,
                y,
                rotation,
                unit_type_id,
            } => {
                if !runtime_effect_event_inside_clip_view(None, *x, *y, clip_view) {
                    continue;
                }
                let mut overlay = spawn_runtime_effect_overlay(
                    None,
                    *x,
                    *y,
                    *x,
                    *y,
                    *rotation,
                    0xffffffff,
                    false,
                    None,
                    DEFAULT_EFFECT_OVERLAY_TTL_TICKS,
                );
                if let Some(unit_type_id) = *unit_type_id {
                    overlay.contract_name = Some("content_icon");
                    overlay.content_ref = Some((6, unit_type_id));
                }
                push_runtime_effect_overlay(runtime_world_overlay, overlay);
            }
            ClientSessionEvent::CreateBullet { projection } => {
                let x = f32::from_bits(projection.x_bits);
                let y = f32::from_bits(projection.y_bits);
                if !runtime_effect_event_inside_clip_view(None, x, y, clip_view) {
                    continue;
                }
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-bullet:{overlay_key}:{}:{}",
                            projection.bullet_type_id.unwrap_or(-1),
                            projection.team_id
                        ),
                        layer: 28,
                        x_bits: projection.x_bits,
                        y_bits: projection.y_bits,
                        remaining_ticks: CREATE_BULLET_MARKER_TTL_TICKS,
                    },
                );
            }
            ClientSessionEvent::LogicExplosionObserved {
                team_id,
                x,
                y,
                radius,
                air,
                ground,
                pierce,
                effect,
                ..
            } => {
                if !runtime_effect_event_inside_clip_view(None, *x, *y, clip_view) {
                    continue;
                }
                let overlay_key = allocate_runtime_world_event_marker_key(runtime_world_overlay);
                push_runtime_world_event_marker_overlay(
                    runtime_world_overlay,
                    RuntimeWorldEventMarkerOverlay {
                        overlay_key,
                        object_id: format!(
                            "marker:runtime-logic-explosion:{overlay_key}:{team_id}:0x{:08x}:{}:{}:{}:{}",
                            radius.to_bits(),
                            u8::from(*effect),
                            u8::from(*air),
                            u8::from(*ground),
                            u8::from(*pierce)
                        ),
                        layer: 28,
                        x_bits: x.to_bits(),
                        y_bits: y.to_bits(),
                        remaining_ticks: LOGIC_EXPLOSION_MARKER_TTL_TICKS,
                    },
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

fn upsert_runtime_world_label_overlay(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    label_id: Option<i32>,
    reliable: bool,
    message: Option<String>,
    duration: f32,
    world_x: f32,
    world_y: f32,
) {
    let remaining_ticks = runtime_world_label_overlay_ttl_ticks(duration);
    let existing_overlay_key = label_id.and_then(|label_id| {
        runtime_world_overlay
            .world_label_overlays
            .iter()
            .position(|overlay| overlay.label_id == Some(label_id))
            .map(|index| {
                runtime_world_overlay
                    .world_label_overlays
                    .remove(index)
                    .overlay_key
            })
    });
    if remaining_ticks == 0 {
        return;
    }
    let overlay = RuntimeWorldLabelOverlay {
        overlay_key: existing_overlay_key.unwrap_or_else(|| {
            let overlay_key = runtime_world_overlay.next_world_label_overlay_key;
            runtime_world_overlay.next_world_label_overlay_key = runtime_world_overlay
                .next_world_label_overlay_key
                .saturating_add(1);
            overlay_key
        }),
        label_id,
        reliable,
        message,
        x_bits: world_x.to_bits(),
        y_bits: world_y.to_bits(),
        remaining_ticks,
    };
    runtime_world_overlay.world_label_overlays.push(overlay);
    if runtime_world_overlay.world_label_overlays.len() > WORLD_LABEL_OVERLAY_LIMIT {
        let overflow = runtime_world_overlay.world_label_overlays.len() - WORLD_LABEL_OVERLAY_LIMIT;
        runtime_world_overlay
            .world_label_overlays
            .drain(0..overflow);
    }
}

fn allocate_runtime_world_event_marker_key(runtime_world_overlay: &mut RuntimeWorldOverlay) -> u64 {
    let overlay_key = runtime_world_overlay.next_world_event_marker_key;
    runtime_world_overlay.next_world_event_marker_key = runtime_world_overlay
        .next_world_event_marker_key
        .saturating_add(1);
    overlay_key
}

fn push_runtime_world_event_marker_overlay(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    overlay: RuntimeWorldEventMarkerOverlay,
) {
    runtime_world_overlay.world_event_markers.push(overlay);
    if runtime_world_overlay.world_event_markers.len() > WORLD_EVENT_MARKER_OVERLAY_LIMIT {
        let overflow =
            runtime_world_overlay.world_event_markers.len() - WORLD_EVENT_MARKER_OVERLAY_LIMIT;
        runtime_world_overlay.world_event_markers.drain(0..overflow);
    }
}

fn runtime_effect_overlay_origin(
    effect_id: Option<i16>,
    x: f32,
    y: f32,
    rotation: f32,
    data_object: Option<&mdt_typeio::TypeIoObject>,
) -> (f32, f32) {
    effect_contract(effect_id)
        .and_then(|contract| {
            effect_contract_executor::overlay_origin_from_contract(
                contract,
                x,
                y,
                rotation,
                data_object,
            )
        })
        .unwrap_or((x, y))
}

fn runtime_effect_overlay_clip_size(effect_id: Option<i16>) -> f32 {
    match effect_id {
        Some(10) => 300.0,
        Some(13) => 500.0,
        Some(178) => 200.0,
        Some(67) | Some(68) => 100.0,
        _ => DEFAULT_EFFECT_OVERLAY_CLIP_SIZE,
    }
}

fn runtime_effect_event_inside_clip_view(
    effect_id: Option<i16>,
    x: f32,
    y: f32,
    clip_view: Option<RuntimeEffectClipView>,
) -> bool {
    let Some(clip_view) = clip_view else {
        return true;
    };
    if !clip_view.center.0.is_finite()
        || !clip_view.center.1.is_finite()
        || !clip_view.size.0.is_finite()
        || !clip_view.size.1.is_finite()
        || clip_view.size.0 <= 0.0
        || clip_view.size.1 <= 0.0
    {
        return false;
    }

    let clip_half = runtime_effect_overlay_clip_size(effect_id) * 0.5;
    let view_half_width = clip_view.size.0 * 0.5;
    let view_half_height = clip_view.size.1 * 0.5;

    let clip_left = x - clip_half;
    let clip_right = x + clip_half;
    let clip_top = y - clip_half;
    let clip_bottom = y + clip_half;

    let view_left = clip_view.center.0 - view_half_width;
    let view_right = clip_view.center.0 + view_half_width;
    let view_top = clip_view.center.1 - view_half_height;
    let view_bottom = clip_view.center.1 + view_half_height;

    clip_right >= view_left
        && clip_left <= view_right
        && clip_bottom >= view_top
        && clip_top <= view_bottom
}

fn runtime_effect_overlay_ttl_ticks(effect_id: Option<i16>) -> u8 {
    match effect_id {
        Some(8) => 17,
        Some(9) => 12,
        Some(10) => 25,
        Some(11) => 8,
        Some(13) => 10,
        Some(178) => 140,
        Some(263) => 90,
        Some(124) => 220,
        Some(67) => 80,
        Some(68) => 40,
        Some(122) => 120,
        Some(26) => 30,
        Some(142) => 20,
        Some(252) => 20,
        Some(256) => 40,
        Some(257) => 40,
        Some(260) => 35,
        Some(261) => 20,
        Some(262) => 30,
        _ => DEFAULT_EFFECT_OVERLAY_TTL_TICKS,
    }
}

fn runtime_effect_overlay_start_delay_ticks(effect_id: Option<i16>) -> u8 {
    match effect_id {
        Some(124) => 30,
        _ => 0,
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

fn runtime_world_label_overlay_ttl_ticks(duration: f32) -> u16 {
    if !duration.is_finite() {
        return 0;
    }
    (duration.max(0.0) * 60.0)
        .ceil()
        .clamp(1.0, u16::MAX as f32) as u16
}

fn advance_runtime_world_label_overlays(runtime_world_overlay: &mut RuntimeWorldOverlay) {
    for overlay in &mut runtime_world_overlay.world_label_overlays {
        overlay.remaining_ticks = overlay.remaining_ticks.saturating_sub(1);
    }
    runtime_world_overlay
        .world_label_overlays
        .retain(|overlay| overlay.remaining_ticks > 0);
}

fn advance_runtime_world_event_markers(runtime_world_overlay: &mut RuntimeWorldOverlay) {
    for overlay in &mut runtime_world_overlay.world_event_markers {
        overlay.remaining_ticks = overlay.remaining_ticks.saturating_sub(1);
    }
    runtime_world_overlay
        .world_event_markers
        .retain(|overlay| overlay.remaining_ticks > 0);
}

fn cull_hidden_parent_unit_effect_overlays(
    runtime_world_overlay: &mut RuntimeWorldOverlay,
    session_state: &SessionState,
) {
    let hidden_ids = &session_state.hidden_snapshot_ids;
    if hidden_ids.is_empty() {
        return;
    }
    let local_player_entity_id = session_state.entity_table_projection.local_player_entity_id;
    runtime_world_overlay.effect_overlays.retain(|overlay| {
        !runtime_effect_overlay_binds_hidden_non_local_unit(
            overlay,
            hidden_ids,
            local_player_entity_id,
        )
    });
}

fn runtime_effect_overlay_binds_hidden_non_local_unit(
    overlay: &RuntimeEffectOverlay,
    hidden_ids: &BTreeSet<i32>,
    local_player_entity_id: Option<i32>,
) -> bool {
    runtime_effect_binding_hidden_non_local_unit(
        overlay.binding.as_ref(),
        hidden_ids,
        local_player_entity_id,
    )
    .is_some()
        || runtime_effect_binding_hidden_non_local_unit(
            overlay.source_binding.as_ref(),
            hidden_ids,
            local_player_entity_id,
        )
        .is_some()
}

fn runtime_effect_binding_hidden_non_local_unit(
    binding: Option<&RuntimeEffectBinding>,
    hidden_ids: &BTreeSet<i32>,
    local_player_entity_id: Option<i32>,
) -> Option<i32> {
    match binding {
        Some(RuntimeEffectBinding::ParentUnit { unit_id, .. })
            if Some(*unit_id) != local_player_entity_id && hidden_ids.contains(unit_id) =>
        {
            Some(*unit_id)
        }
        _ => None,
    }
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

fn runtime_core_binding_label(session_state: &SessionState) -> String {
    let kind = session_state
        .core_inventory_runtime_binding_kind
        .map(|kind| kind.as_str())
        .unwrap_or("none");
    format!(
        "{}:a{}@{}:m{}@{}",
        kind,
        session_state.core_inventory_runtime_ambiguous_team_count,
        runtime_core_inventory_changed_team_sample_label(
            &session_state.core_inventory_runtime_ambiguous_team_sample,
            session_state.core_inventory_runtime_ambiguous_team_count,
        ),
        session_state.core_inventory_runtime_missing_team_count,
        runtime_core_inventory_changed_team_sample_label(
            &session_state.core_inventory_runtime_missing_team_sample,
            session_state.core_inventory_runtime_missing_team_count,
        ),
    )
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
        runtime_configured_payload_loader_family_label(
            "pl",
            &projection.payload_loader_runtime_by_build_pos,
        ),
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
        runtime_configured_unit_factory_family_label(
            "uf",
            &projection.unit_factory_current_plan_by_build_pos,
            &projection.unit_factory_runtime_by_build_pos,
        ),
        runtime_configured_unit_assembler_family_label(
            "ua",
            &projection.unit_assembler_by_build_pos,
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

fn runtime_configured_payload_loader_family_label(
    prefix: &str,
    values: &BTreeMap<i32, PayloadLoaderRuntimeProjection>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, projection)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            let payload = projection
                .payload_build_block_id
                .map(|content_id| {
                    let mut label = format!("b:{content_id}");
                    if let Some(revision) = projection.payload_build_revision {
                        label.push_str(&format!("@r{revision}"));
                    }
                    label
                })
                .or_else(|| {
                    projection.payload_unit_class_id.map(|class_id| {
                        let mut label = format!("uc:{class_id}");
                        if let Some(payload_len) = projection.payload_unit_payload_len {
                            label.push_str(&format!(":l{payload_len}"));
                        }
                        if let Some(payload_sha256) =
                            projection.payload_unit_payload_sha256.as_deref()
                        {
                            label.push_str(&format!(
                                ":s{}",
                                payload_sha256.chars().take(12).collect::<String>()
                            ));
                        }
                        label
                    })
                })
                .unwrap_or_else(|| {
                    if projection.payload_present {
                        "present".to_string()
                    } else {
                        "none".to_string()
                    }
                });
            format!(
                "{prefix}{count}@{x}:{y}={}:y{}{}:r{:08x}:{payload}",
                if projection.exporting { "exp" } else { "imp" },
                if projection.payload_present { 1 } else { 0 },
                projection
                    .payload_type
                    .map(|payload_type| format!(":t{payload_type}"))
                    .unwrap_or_default(),
                projection.pay_rotation_bits,
            )
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

fn runtime_configured_unit_assembler_family_label(
    prefix: &str,
    values: &BTreeMap<i32, UnitAssemblerRuntimeProjection>,
) -> String {
    let count = values.len();
    match values.last_key_value() {
        Some((build_pos, assembler)) => {
            let (x, y) = unpack_runtime_point2(*build_pos);
            let block_sample = assembler
                .block_sample
                .as_ref()
                .map(runtime_build_config_content_ref_label)
                .unwrap_or_else(|| "none".to_string());
            format!(
                "{prefix}{count}@{x}:{y}=p{:08x}:u{}:b{}:s{block_sample}:c{}:y{}:r{:08x}",
                assembler.progress_bits,
                assembler.unit_ids.len(),
                assembler.block_entry_count,
                runtime_optional_command_pos_bits_label(assembler.command_pos),
                if assembler.payload_present { 1 } else { 0 },
                assembler.pay_rotation_bits,
            )
        }
        None => format!("{prefix}{count}"),
    }
}

fn runtime_configured_unit_factory_family_label(
    prefix: &str,
    current_plans: &BTreeMap<i32, i16>,
    runtimes: &BTreeMap<i32, UnitFactoryRuntimeProjection>,
) -> String {
    let count = current_plans.len().max(runtimes.len());
    let build_pos = current_plans
        .last_key_value()
        .map(|(build_pos, _)| *build_pos)
        .or_else(|| runtimes.last_key_value().map(|(build_pos, _)| *build_pos));
    let Some(build_pos) = build_pos else {
        return format!("{prefix}{count}");
    };
    let (x, y) = unpack_runtime_point2(build_pos);
    let current_plan = current_plans.get(&build_pos).copied();
    let runtime = runtimes.get(&build_pos);
    let mut parts = Vec::new();
    if let Some(current_plan) = current_plan {
        parts.push(format!("cp{current_plan}"));
    }
    if let Some(runtime) = runtime {
        parts.push(format!("p{:08x}", runtime.progress_bits));
        parts.push(format!(
            "c{}",
            runtime_optional_command_pos_bits_label(runtime.command_pos)
        ));
        if let Some(command_id) = runtime.command_id {
            parts.push(format!("cmd{command_id}"));
        }
        parts.push(format!("y{}", if runtime.payload_present { 1 } else { 0 }));
        parts.push(format!("r{:08x}", runtime.pay_rotation_bits));
    }
    if parts.is_empty() {
        format!("{prefix}{count}@{x}:{y}=unset")
    } else {
        format!("{prefix}{count}@{x}:{y}={}", parts.join(":"))
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
        "{}:b{}:c{}:{}@{}#{}:rm{}:on{}:e{}:oe{}:v{}:m{}:vf{}:tur{}:trb{}:bn{}",
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
        runtime_building_table_turret_tail_label(projection),
        projection
            .last_build_turret_rotation_bits
            .map(|bits| format!("0x{bits:08x}"))
            .unwrap_or_else(|| "none".to_string()),
        projection.last_block_name.as_deref().unwrap_or("none"),
    )
}

fn runtime_building_table_turret_tail_label(projection: &BuildingTableProjection) -> String {
    let reload = projection
        .last_turret_reload_counter_bits
        .map(|bits| format!("r0x{bits:08x}"))
        .unwrap_or_else(|| "rnone".to_string());
    let rotation = projection
        .last_turret_rotation_bits
        .map(|bits| format!("t0x{bits:08x}"))
        .unwrap_or_else(|| "tnone".to_string());
    let ammo = projection
        .last_item_turret_ammo_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "none".to_string());
    let length = projection
        .last_continuous_turret_last_length_bits
        .map(|bits| format!("0x{bits:08x}"))
        .unwrap_or_else(|| "none".to_string());
    format!("{reload}:{rotation}:a{ammo}:l{length}")
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
    let base = format!(
        "{}:c{}:u{}:{}:h{}",
        entity_id,
        entity.class_id,
        entity.unit_kind,
        entity.unit_value,
        if entity.hidden { 1 } else { 0 },
    );
    let runtime_entity_projection = session_state.runtime_typed_entity_projection();
    let Some(owned_unit_entity_id) = runtime_entity_projection.local_player_owned_unit_entity_id
    else {
        return base;
    };
    let Some(TypedRuntimeEntityModel::Unit(unit)) =
        runtime_entity_projection.entity_at(owned_unit_entity_id)
    else {
        return base;
    };
    let Some(runtime_sync) = unit.semantic.runtime_sync.as_ref() else {
        return format!("{base}:ou{owned_unit_entity_id}");
    };
    let base_rotation = runtime_sync
        .base_rotation_bits
        .map(|bits| format!("0x{bits:08x}"))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "{base}:ou{owned_unit_entity_id}@am0x{:08x}:el0x{:08x}:fg0x{:016x}:br{}",
        runtime_sync.ammo_bits, runtime_sync.elevation_bits, runtime_sync.flag_bits, base_rotation,
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
        Some(EffectBusinessProjection::PositionTarget {
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
        }) => format!(
            "target:0x{source_x_bits:08x}:0x{source_y_bits:08x}:0x{target_x_bits:08x}:0x{target_y_bits:08x}"
        ),
        Some(EffectBusinessProjection::PayloadTargetContent {
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            content_type,
            content_id,
        }) => format!(
            "payloadTarget:0x{source_x_bits:08x}:0x{source_y_bits:08x}:0x{target_x_bits:08x}:0x{target_y_bits:08x}:{content_type}:{content_id}"
        ),
        Some(EffectBusinessProjection::LengthRay {
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            rotation_bits,
            length_bits,
        }) => format!(
            "ray:0x{source_x_bits:08x}:0x{source_y_bits:08x}:0x{target_x_bits:08x}:0x{target_y_bits:08x}:0x{rotation_bits:08x}:0x{length_bits:08x}"
        ),
        Some(EffectBusinessProjection::LightningPath { points }) => {
            let last = points
                .last()
                .map(|(x_bits, y_bits)| format!(":0x{x_bits:08x}:0x{y_bits:08x}"))
                .unwrap_or_default();
            format!("lightningPath:{}{}", points.len(), last)
        }
        Some(EffectBusinessProjection::FloatValue(bits)) => {
            format!("floatBits:0x{bits:08x}")
        }
        None => "none".to_string(),
    }
}

fn runtime_effect_business_hint_label(hint: Option<&EffectDataBusinessHint>) -> String {
    match hint {
        Some(EffectDataBusinessHint::ContentRef {
            kind,
            content_type,
            content_id,
            path,
        }) => format!(
            "content:{}:{content_type}:{content_id}@{}",
            runtime_effect_content_kind_label(*kind),
            runtime_effect_hint_path_label(path)
        ),
        Some(EffectDataBusinessHint::ParentRef { semantic_ref, path }) => format!(
            "parent:{}@{}",
            runtime_effect_semantic_ref_label(*semantic_ref),
            runtime_effect_hint_path_label(path)
        ),
        Some(EffectDataBusinessHint::PositionHint(position)) => {
            runtime_effect_position_hint_label(position)
        }
        Some(EffectDataBusinessHint::FloatBits { bits, path }) => format!(
            "float:0x{bits:08x}@{}",
            runtime_effect_hint_path_label(path)
        ),
        Some(EffectDataBusinessHint::Polyline { points, path }) => {
            let tail = points
                .last()
                .map(|(x_bits, y_bits)| format!(":0x{x_bits:08x}:0x{y_bits:08x}"))
                .unwrap_or_default();
            format!(
                "poly:{}{}@{}",
                points.len(),
                tail,
                runtime_effect_hint_path_label(path)
            )
        }
        Some(EffectDataBusinessHint::PayloadTargetContent {
            content_kind,
            content_type,
            content_id,
            content_path,
            target,
        }) => format!(
            "payload:{}:{content_type}:{content_id}@{}>{}",
            runtime_effect_content_kind_label(*content_kind),
            runtime_effect_hint_path_label(content_path),
            runtime_effect_target_hint_label(target)
        ),
        None => "none".to_string(),
    }
}

fn runtime_effect_content_kind_label(kind: EffectBusinessContentKind) -> &'static str {
    match kind {
        EffectBusinessContentKind::Content => "content",
        EffectBusinessContentKind::TechNode => "techNode",
    }
}

fn runtime_effect_hint_path_label(path: &[usize]) -> String {
    if path.is_empty() {
        "root".to_string()
    } else {
        path.iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn runtime_effect_semantic_ref_label(semantic_ref: mdt_typeio::TypeIoSemanticRef) -> String {
    match semantic_ref {
        mdt_typeio::TypeIoSemanticRef::Content {
            content_type,
            content_id,
        } => format!("content:{content_type}:{content_id}"),
        mdt_typeio::TypeIoSemanticRef::TechNode {
            content_type,
            content_id,
        } => format!("techNode:{content_type}:{content_id}"),
        mdt_typeio::TypeIoSemanticRef::Unit { unit_id } => format!("unit:{unit_id}"),
        mdt_typeio::TypeIoSemanticRef::Building { build_pos } => format!("building:{build_pos}"),
    }
}

fn runtime_effect_position_hint_label(position: &mdt_typeio::TypeIoEffectPositionHint) -> String {
    match position {
        mdt_typeio::TypeIoEffectPositionHint::Point2 { x, y, path } => {
            format!(
                "pos:point2:{x}:{y}@{}",
                runtime_effect_hint_path_label(path)
            )
        }
        mdt_typeio::TypeIoEffectPositionHint::PackedPoint2ArrayFirst {
            packed_point2,
            path,
        } => {
            let (x, y) = unpack_runtime_point2(*packed_point2);
            format!(
                "pos:point2Array:{x}:{y}@{}",
                runtime_effect_hint_path_label(path)
            )
        }
        mdt_typeio::TypeIoEffectPositionHint::Vec2 {
            x_bits,
            y_bits,
            path,
        } => format!(
            "pos:vec2:0x{x_bits:08x}:0x{y_bits:08x}@{}",
            runtime_effect_hint_path_label(path)
        ),
        mdt_typeio::TypeIoEffectPositionHint::Vec2ArrayFirst {
            x_bits,
            y_bits,
            path,
        } => format!(
            "pos:vec2Array:0x{x_bits:08x}:0x{y_bits:08x}@{}",
            runtime_effect_hint_path_label(path)
        ),
    }
}

fn runtime_effect_target_hint_label(
    target: &crate::effect_data_runtime::EffectDataBusinessTargetHint,
) -> String {
    match target {
        crate::effect_data_runtime::EffectDataBusinessTargetHint::SemanticRef(matched) => {
            format!(
                "{}@{}",
                runtime_effect_semantic_ref_label(matched.semantic_ref),
                runtime_effect_hint_path_label(&matched.path)
            )
        }
        crate::effect_data_runtime::EffectDataBusinessTargetHint::PositionHint(position) => {
            runtime_effect_position_hint_label(position)
        }
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

fn runtime_ui_observability(
    snapshot_input: &ClientSnapshotInputState,
    session_state: &SessionState,
    world_overlay: &RuntimeWorldOverlay,
) -> RuntimeUiObservability {
    RuntimeUiObservability {
        hud_text: RuntimeHudTextObservability {
            set_count: session_state.received_set_hud_text_count,
            set_reliable_count: session_state.received_set_hud_text_reliable_count,
            hide_count: session_state.received_hide_hud_text_count,
            last_message: session_state.last_set_hud_text_message.clone(),
            last_reliable_message: session_state.last_set_hud_text_reliable_message.clone(),
            announce_count: session_state.received_announce_count,
            last_announce_message: session_state.last_announce_message.clone(),
            info_message_count: session_state.received_info_message_count,
            last_info_message: session_state.last_info_message.clone(),
        },
        toast: RuntimeToastObservability {
            info_count: session_state.received_info_toast_count,
            warning_count: session_state.received_warning_toast_count,
            last_info_message: session_state.last_info_toast_message.clone(),
            last_warning_text: session_state.last_warning_toast_text.clone(),
            info_popup_count: session_state.received_info_popup_count,
            info_popup_reliable_count: session_state.received_info_popup_reliable_count,
            last_info_popup_reliable: session_state.last_info_popup_reliable,
            last_info_popup_id: session_state.last_info_popup_id.clone(),
            last_info_popup_message: session_state.last_info_popup_message.clone(),
            last_info_popup_duration_bits: session_state.last_info_popup_duration_bits,
            last_info_popup_align: session_state.last_info_popup_align,
            last_info_popup_top: session_state.last_info_popup_top,
            last_info_popup_left: session_state.last_info_popup_left,
            last_info_popup_bottom: session_state.last_info_popup_bottom,
            last_info_popup_right: session_state.last_info_popup_right,
            clipboard_count: session_state.received_copy_to_clipboard_count,
            last_clipboard_text: session_state.last_copy_to_clipboard_text.clone(),
            open_uri_count: session_state.received_open_uri_count,
            last_open_uri: session_state.last_open_uri.clone(),
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
        chat: runtime_chat_observability(session_state),
        admin: runtime_admin_observability(session_state),
        menu: runtime_menu_observability(session_state),
        command_mode: runtime_command_mode_observability(&snapshot_input.command_mode),
        rules: runtime_rules_observability(session_state),
        world_labels: runtime_world_label_observability(session_state),
        markers: runtime_marker_observability(session_state),
        session: runtime_session_observability(session_state, world_overlay),
        live: runtime_live_summary_observability(session_state, world_overlay),
    }
}

fn runtime_command_mode_observability(
    projection: &mdt_input::CommandModeProjection,
) -> mdt_render_ui::RuntimeCommandModeObservability {
    mdt_render_ui::RuntimeCommandModeObservability {
        active: projection.active,
        selected_units: projection.selected_units.clone(),
        command_buildings: projection.command_buildings.clone(),
        command_rect: projection.command_rect.map(|rect| {
            mdt_render_ui::RuntimeCommandRectObservability {
                x0: rect.x0,
                y0: rect.y0,
                x1: rect.x1,
                y1: rect.y1,
            }
        }),
        control_groups: projection
            .control_groups
            .iter()
            .map(
                |group| mdt_render_ui::RuntimeCommandControlGroupObservability {
                    index: group.index,
                    unit_ids: group.unit_ids.clone(),
                },
            )
            .collect(),
        last_target: projection.last_target.map(|target| {
            mdt_render_ui::RuntimeCommandTargetObservability {
                build_target: target.build_target,
                unit_target: target.unit_target.map(|unit| {
                    mdt_render_ui::RuntimeCommandUnitRefObservability {
                        kind: unit.kind,
                        value: unit.value,
                    }
                }),
                position_target: target.position_target.map(|position| {
                    mdt_render_ui::RuntimeWorldPositionObservability {
                        x_bits: position.x_bits,
                        y_bits: position.y_bits,
                    }
                }),
                rect_target: target.rect_target.map(|rect| {
                    mdt_render_ui::RuntimeCommandRectObservability {
                        x0: rect.x0,
                        y0: rect.y0,
                        x1: rect.x1,
                        y1: rect.y1,
                    }
                }),
            }
        }),
        last_command_selection: projection.last_command_selection.map(|selection| {
            mdt_render_ui::RuntimeCommandSelectionObservability {
                command_id: selection.command_id,
            }
        }),
        last_stance_selection: projection.last_stance_selection.map(|selection| {
            mdt_render_ui::RuntimeCommandStanceObservability {
                stance_id: selection.stance_id,
                enabled: selection.enabled,
            }
        }),
    }
}

fn runtime_chat_observability(session_state: &SessionState) -> RuntimeChatObservability {
    RuntimeChatObservability {
        server_message_count: session_state.received_server_message_count,
        last_server_message: session_state.last_server_message.clone(),
        chat_message_count: session_state.received_chat_message_count,
        last_chat_message: session_state.last_chat_message.clone(),
        last_chat_unformatted: session_state.last_chat_unformatted.clone(),
        last_chat_sender_entity_id: session_state.last_chat_sender_entity_id,
    }
}

fn runtime_admin_observability(session_state: &SessionState) -> RuntimeAdminObservability {
    RuntimeAdminObservability {
        trace_info_count: session_state.received_trace_info_count,
        trace_info_parse_fail_count: session_state.failed_trace_info_parse_count,
        last_trace_info_player_id: session_state.last_trace_info_player_id,
        debug_status_client_count: session_state.received_debug_status_client_count,
        debug_status_client_parse_fail_count: session_state.failed_debug_status_client_parse_count,
        debug_status_client_unreliable_count: session_state
            .received_debug_status_client_unreliable_count,
        debug_status_client_unreliable_parse_fail_count: session_state
            .failed_debug_status_client_unreliable_parse_count,
        last_debug_status_value: session_state.last_debug_status_value,
    }
}

fn runtime_menu_observability(session_state: &SessionState) -> RuntimeMenuObservability {
    RuntimeMenuObservability {
        menu_open_count: session_state.received_menu_open_count,
        follow_up_menu_open_count: session_state.received_follow_up_menu_open_count,
        hide_follow_up_menu_count: session_state.received_hide_follow_up_menu_count,
        last_menu_open_id: session_state.last_menu_open_id,
        last_menu_open_title: session_state.last_menu_open_title.clone(),
        last_menu_open_message: session_state.last_menu_open_message.clone(),
        last_menu_open_option_rows: session_state.last_menu_open_option_rows,
        last_menu_open_first_row_len: session_state.last_menu_open_first_row_len,
        last_follow_up_menu_open_id: session_state.last_follow_up_menu_open_id,
        last_follow_up_menu_open_title: session_state.last_follow_up_menu_open_title.clone(),
        last_follow_up_menu_open_message: session_state.last_follow_up_menu_open_message.clone(),
        last_follow_up_menu_open_option_rows: session_state.last_follow_up_menu_open_option_rows,
        last_follow_up_menu_open_first_row_len: session_state
            .last_follow_up_menu_open_first_row_len,
        last_hide_follow_up_menu_id: session_state.last_hide_follow_up_menu_id,
        menu_choose_count: session_state.received_menu_choose_count,
        last_menu_choose_menu_id: session_state.last_menu_choose_menu_id,
        last_menu_choose_option: session_state.last_menu_choose_option,
        text_input_result_count: session_state.received_text_input_result_count,
        last_text_input_result_id: session_state.last_text_input_result_id,
        last_text_input_result_text: session_state.last_text_input_result_text.clone(),
    }
}

fn runtime_rules_observability(session_state: &SessionState) -> RuntimeRulesObservability {
    RuntimeRulesObservability {
        set_rules_count: session_state.received_set_rules_count,
        set_rules_parse_fail_count: session_state.failed_set_rules_parse_count,
        set_objectives_count: session_state.received_set_objectives_count,
        set_objectives_parse_fail_count: session_state.failed_set_objectives_parse_count,
        set_rule_count: session_state.received_set_rule_count,
        set_rule_parse_fail_count: session_state.failed_set_rule_parse_count,
        clear_objectives_count: session_state.received_clear_objectives_count,
        complete_objective_count: session_state.received_complete_objective_count,
        waves: session_state.rules_projection.waves,
        pvp: session_state.rules_projection.pvp,
        objective_count: session_state.objectives_projection.objectives.len(),
        qualified_objective_count: session_state.objectives_projection.qualified_count(),
        objective_parent_edge_count: session_state.objectives_projection.parent_edge_count(),
        objective_flag_count: session_state.objectives_projection.objective_flags.len(),
        complete_out_of_range_count: session_state
            .objectives_projection
            .complete_out_of_range_count,
        last_completed_index: session_state.objectives_projection.last_completed_index,
    }
}

fn runtime_world_label_observability(
    session_state: &SessionState,
) -> RuntimeWorldLabelObservability {
    let typed_projection = session_state.runtime_typed_entity_projection();
    let active_labels = typed_projection
        .by_entity_id
        .iter()
        .filter_map(|(&entity_id, entity)| match entity {
            TypedRuntimeEntityModel::WorldLabel(world_label) if !world_label.base.hidden => {
                Some((entity_id, world_label))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let inactive_count = typed_projection
        .by_entity_id
        .values()
        .filter(|entity| {
            matches!(
                entity,
                TypedRuntimeEntityModel::WorldLabel(world_label) if world_label.base.hidden
            )
        })
        .count();
    let last_active_label = active_labels
        .iter()
        .max_by_key(|(entity_id, world_label)| {
            (world_label.base.last_seen_entity_snapshot_count, *entity_id)
        })
        .copied();
    RuntimeWorldLabelObservability {
        label_count: session_state.received_world_label_count,
        reliable_label_count: session_state.received_world_label_reliable_count,
        remove_label_count: session_state.received_remove_world_label_count,
        active_count: active_labels.len(),
        inactive_count,
        last_entity_id: last_active_label.map(|(entity_id, _)| entity_id),
        last_text: last_active_label.and_then(|(_, world_label)| world_label.semantic.text.clone()),
        last_flags: last_active_label.map(|(_, world_label)| world_label.semantic.flags),
        last_font_size_bits: last_active_label
            .map(|(_, world_label)| world_label.semantic.font_size_bits),
        last_z_bits: last_active_label.map(|(_, world_label)| world_label.semantic.z_bits),
        last_position: last_active_label.map(|(_, world_label)| {
            RuntimeWorldPositionObservability {
                x_bits: world_label.base.x_bits,
                y_bits: world_label.base.y_bits,
            }
        }),
    }
}

fn runtime_marker_observability(session_state: &SessionState) -> RuntimeMarkerObservability {
    RuntimeMarkerObservability {
        create_count: session_state.received_create_marker_count,
        remove_count: session_state.received_remove_marker_count,
        update_count: session_state.received_update_marker_count,
        update_text_count: session_state.received_update_marker_text_count,
        update_texture_count: session_state.received_update_marker_texture_count,
        decode_fail_count: session_state.failed_marker_decode_count,
        last_marker_id: session_state.last_marker_id,
        last_control_name: session_state.last_marker_control_name.clone(),
    }
}

fn runtime_session_observability(
    session_state: &SessionState,
    world_overlay: &RuntimeWorldOverlay,
) -> RuntimeSessionObservability {
    RuntimeSessionObservability {
        core_binding: RuntimeCoreBindingObservability {
            kind: session_state
                .core_inventory_runtime_binding_kind
                .map(runtime_core_binding_kind_observability),
            ambiguous_team_count: session_state.core_inventory_runtime_ambiguous_team_count,
            ambiguous_team_sample: session_state
                .core_inventory_runtime_ambiguous_team_sample
                .clone(),
            missing_team_count: session_state.core_inventory_runtime_missing_team_count,
            missing_team_sample: session_state
                .core_inventory_runtime_missing_team_sample
                .clone(),
        },
        resource_delta: runtime_resource_delta_observability(session_state),
        kick: RuntimeKickObservability {
            reason_text: world_overlay.last_kick_reason_text.clone(),
            reason_ordinal: world_overlay.last_kick_reason_ordinal,
            hint_category: world_overlay.last_kick_hint_category.map(str::to_string),
            hint_text: world_overlay.last_kick_hint_text.map(str::to_string),
        },
        loading: RuntimeLoadingObservability {
            deferred_inbound_packet_count: session_state.deferred_inbound_packet_count,
            replayed_inbound_packet_count: session_state.replayed_inbound_packet_count,
            dropped_loading_low_priority_packet_count: session_state
                .dropped_loading_low_priority_packet_count,
            dropped_loading_deferred_overflow_count: session_state
                .dropped_loading_deferred_overflow_count,
            failed_state_snapshot_parse_count: session_state.failed_state_snapshot_parse_count,
            failed_state_snapshot_core_data_parse_count: session_state
                .failed_state_snapshot_core_data_parse_count,
            failed_entity_snapshot_parse_count: session_state.failed_entity_snapshot_parse_count,
            ready_inbound_liveness_anchor_count: session_state.ready_inbound_liveness_anchor_count,
            last_ready_inbound_liveness_anchor_at_ms: session_state
                .last_ready_inbound_liveness_anchor_at_ms,
            timeout_count: session_state.timeout_count,
            connect_or_loading_timeout_count: session_state.connect_or_loading_timeout_count,
            ready_snapshot_timeout_count: session_state.ready_snapshot_timeout_count,
            last_timeout_kind: session_state
                .last_timeout
                .as_ref()
                .map(|timeout| runtime_session_timeout_kind_observability(timeout.kind)),
            last_timeout_idle_ms: session_state
                .last_timeout
                .as_ref()
                .map(|timeout| timeout.idle_ms),
            reset_count: session_state.reset_count,
            reconnect_reset_count: session_state.reconnect_reset_count,
            world_reload_count: session_state.world_reload_count,
            kick_reset_count: session_state.kick_reset_count,
            last_reset_kind: session_state
                .last_reset_kind
                .map(runtime_session_reset_kind_observability),
            last_world_reload: session_state
                .last_world_reload
                .as_ref()
                .map(|world_reload| RuntimeWorldReloadObservability {
                    had_loaded_world: world_reload.had_loaded_world,
                    had_client_loaded: world_reload.had_client_loaded,
                    was_ready_to_enter_world: world_reload.was_ready_to_enter_world,
                    had_connect_confirm_sent: world_reload.had_connect_confirm_sent,
                    cleared_pending_packets: world_reload.cleared_pending_packets,
                    cleared_deferred_inbound_packets: world_reload.cleared_deferred_inbound_packets,
                    cleared_replayed_loading_events: world_reload.cleared_replayed_loading_events,
                }),
        },
        reconnect: RuntimeReconnectObservability {
            phase: runtime_reconnect_phase_observability(session_state.reconnect_projection.phase),
            phase_transition_count: session_state.reconnect_projection.phase_transition_count,
            reason_kind: session_state
                .reconnect_projection
                .reason_kind
                .map(runtime_reconnect_reason_kind_observability),
            reason_text: session_state.reconnect_projection.reason_text.clone(),
            reason_ordinal: session_state.reconnect_projection.reason_ordinal,
            hint_text: session_state.reconnect_projection.hint_text.clone(),
            redirect_count: session_state.received_connect_redirect_count,
            last_redirect_ip: session_state.last_connect_redirect_ip.clone(),
            last_redirect_port: session_state.last_connect_redirect_port,
        },
    }
}

fn runtime_resource_delta_observability(
    session_state: &SessionState,
) -> RuntimeResourceDeltaObservability {
    RuntimeResourceDeltaObservability {
        remove_tile_count: session_state.received_remove_tile_count,
        set_tile_count: session_state.received_set_tile_count,
        set_floor_count: session_state.received_set_floor_count,
        set_overlay_count: session_state.received_set_overlay_count,
        set_item_count: session_state.received_set_item_count,
        set_items_count: session_state.received_set_items_count,
        set_liquid_count: session_state.received_set_liquid_count,
        set_liquids_count: session_state.received_set_liquids_count,
        clear_items_count: session_state.received_clear_items_count,
        clear_liquids_count: session_state.received_clear_liquids_count,
        set_tile_items_count: session_state.received_set_tile_items_count,
        set_tile_liquids_count: session_state.received_set_tile_liquids_count,
        take_items_count: session_state.resource_delta_projection.take_items_count,
        transfer_item_to_count: session_state
            .resource_delta_projection
            .transfer_item_to_count,
        transfer_item_to_unit_count: session_state
            .resource_delta_projection
            .transfer_item_to_unit_count,
        last_kind: session_state
            .resource_delta_projection
            .last_kind
            .map(str::to_string),
        last_item_id: session_state.resource_delta_projection.last_item_id,
        last_amount: session_state.resource_delta_projection.last_amount,
        last_build_pos: session_state.resource_delta_projection.last_build_pos,
        last_unit: session_state
            .resource_delta_projection
            .last_unit
            .map(runtime_command_unit_ref_observability),
        last_to_entity_id: session_state.resource_delta_projection.last_to_entity_id,
        build_count: session_state.resource_delta_projection.build_count(),
        build_stack_count: session_state.resource_delta_projection.build_stack_count(),
        entity_count: session_state.resource_delta_projection.entity_count(),
        authoritative_build_update_count: session_state
            .resource_delta_projection
            .authoritative_build_update_count,
        delta_apply_count: session_state.resource_delta_projection.delta_apply_count,
        delta_skip_count: session_state.resource_delta_projection.delta_skip_count,
        delta_conflict_count: session_state.resource_delta_projection.delta_conflict_count,
        last_changed_build_pos: session_state
            .resource_delta_projection
            .last_changed_build_pos,
        last_changed_entity_id: session_state
            .resource_delta_projection
            .last_changed_entity_id,
        last_changed_item_id: session_state.resource_delta_projection.last_changed_item_id,
        last_changed_amount: session_state.resource_delta_projection.last_changed_amount,
    }
}

fn runtime_command_unit_ref_observability(
    unit: UnitRefProjection,
) -> RuntimeCommandUnitRefObservability {
    RuntimeCommandUnitRefObservability {
        kind: unit.kind,
        value: unit.value,
    }
}

fn runtime_session_timeout_kind_observability(
    kind: SessionTimeoutKind,
) -> RuntimeSessionTimeoutKind {
    match kind {
        SessionTimeoutKind::ConnectOrLoading => RuntimeSessionTimeoutKind::ConnectOrLoading,
        SessionTimeoutKind::ReadySnapshotStall => RuntimeSessionTimeoutKind::ReadySnapshotStall,
    }
}

fn runtime_core_binding_kind_observability(
    kind: CoreInventoryRuntimeBindingKind,
) -> RuntimeCoreBindingKindObservability {
    match kind {
        CoreInventoryRuntimeBindingKind::FirstCorePerTeamApproximation => {
            RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation
        }
    }
}

fn runtime_session_reset_kind_observability(kind: SessionResetKind) -> RuntimeSessionResetKind {
    match kind {
        SessionResetKind::Reconnect => RuntimeSessionResetKind::Reconnect,
        SessionResetKind::WorldReload => RuntimeSessionResetKind::WorldReload,
        SessionResetKind::Kick => RuntimeSessionResetKind::Kick,
    }
}

fn runtime_reconnect_phase_observability(
    phase: ReconnectPhaseProjection,
) -> RuntimeReconnectPhaseObservability {
    match phase {
        ReconnectPhaseProjection::Idle => RuntimeReconnectPhaseObservability::Idle,
        ReconnectPhaseProjection::Scheduled => RuntimeReconnectPhaseObservability::Scheduled,
        ReconnectPhaseProjection::Attempting => RuntimeReconnectPhaseObservability::Attempting,
        ReconnectPhaseProjection::Succeeded => RuntimeReconnectPhaseObservability::Succeeded,
        ReconnectPhaseProjection::Aborted => RuntimeReconnectPhaseObservability::Aborted,
    }
}

fn runtime_reconnect_reason_kind_observability(
    kind: ReconnectReasonKind,
) -> RuntimeReconnectReasonKind {
    match kind {
        ReconnectReasonKind::ConnectRedirect => RuntimeReconnectReasonKind::ConnectRedirect,
        ReconnectReasonKind::Kick => RuntimeReconnectReasonKind::Kick,
        ReconnectReasonKind::Timeout => RuntimeReconnectReasonKind::Timeout,
        ReconnectReasonKind::ManualConnect => RuntimeReconnectReasonKind::ManualConnect,
    }
}

fn runtime_live_summary_observability(
    session_state: &SessionState,
    world_overlay: &RuntimeWorldOverlay,
) -> RuntimeLiveSummaryObservability {
    RuntimeLiveSummaryObservability {
        entity: runtime_live_entity_summary_observability(session_state),
        effect: runtime_live_effect_summary_observability(session_state, world_overlay),
    }
}

fn runtime_live_entity_summary_observability(
    session_state: &SessionState,
) -> RuntimeLiveEntitySummaryObservability {
    let typed_projection = session_state.runtime_typed_entity_projection();
    let local_entity = typed_projection.local_player().map(|player| &player.base);

    RuntimeLiveEntitySummaryObservability {
        entity_count: session_state.entity_table_projection.by_entity_id.len(),
        hidden_count: session_state.entity_table_projection.hidden_count,
        player_count: typed_projection.player_count,
        unit_count: typed_projection.unit_count,
        last_entity_id: typed_projection.last_entity_id,
        last_player_entity_id: typed_projection.last_player_entity_id,
        last_unit_entity_id: typed_projection.last_unit_entity_id,
        local_entity_id: local_entity.map(|entity| entity.entity_id),
        local_unit_kind: local_entity.map(|entity| entity.unit_kind),
        local_unit_value: local_entity.map(|entity| entity.unit_value),
        local_hidden: local_entity.map(|entity| entity.hidden),
        local_last_seen_entity_snapshot_count: local_entity
            .map(|entity| entity.last_seen_entity_snapshot_count),
        local_position: local_entity.map(|entity| RuntimeWorldPositionObservability {
            x_bits: entity.x_bits,
            y_bits: entity.y_bits,
        }),
    }
}

fn runtime_live_effect_summary_observability(
    session_state: &SessionState,
    world_overlay: &RuntimeWorldOverlay,
) -> RuntimeLiveEffectSummaryObservability {
    let (last_position_source, last_position_hint) =
        runtime_live_effect_position_hint_observability(session_state);
    let active_overlay = world_overlay.effect_overlays.last();
    RuntimeLiveEffectSummaryObservability {
        effect_count: session_state.received_effect_count,
        spawn_effect_count: session_state.received_spawn_effect_count,
        active_overlay_count: world_overlay.effect_overlays.len(),
        active_effect_id: active_overlay.and_then(|overlay| overlay.effect_id),
        active_contract_name: active_overlay
            .and_then(|overlay| overlay.contract_name.map(str::to_string)),
        active_reliable: active_overlay.map(|overlay| overlay.reliable),
        active_position: active_overlay.map(|overlay| RuntimeWorldPositionObservability {
            x_bits: overlay.x_bits,
            y_bits: overlay.y_bits,
        }),
        last_effect_id: session_state.last_effect_id,
        last_spawn_effect_unit_type_id: session_state.last_spawn_effect_unit_type_id,
        last_kind: session_state.last_effect_data_kind.clone(),
        last_contract_name: session_state.last_effect_contract_name.clone(),
        last_reliable_contract_name: session_state.last_effect_reliable_contract_name.clone(),
        last_business_hint: Some(runtime_effect_business_hint_label(
            session_state.last_effect_data_business_hint.as_ref(),
        ))
        .filter(|label| label != "none"),
        last_position_hint,
        last_position_source,
    }
}

fn runtime_live_effect_position_hint_observability(
    session_state: &SessionState,
) -> (
    Option<RuntimeLiveEffectPositionSource>,
    Option<RuntimeWorldPositionObservability>,
) {
    if let Some(position) = runtime_world_position_from_named_effect_business_projection(
        session_state.last_effect_contract_name.as_deref(),
        session_state.last_effect_business_projection.as_ref(),
    ) {
        return (
            Some(RuntimeLiveEffectPositionSource::BusinessProjection),
            Some(position),
        );
    }
    if let Some(position) = runtime_world_position_observability(
        session_state.last_effect_x_bits,
        session_state.last_effect_y_bits,
    ) {
        return (
            Some(RuntimeLiveEffectPositionSource::EffectPacket),
            Some(position),
        );
    }
    if let Some(position) = runtime_world_position_observability(
        session_state.last_spawn_effect_x_bits,
        session_state.last_spawn_effect_y_bits,
    ) {
        return (
            Some(RuntimeLiveEffectPositionSource::SpawnEffectPacket),
            Some(position),
        );
    }
    (None, None)
}

#[cfg(test)]
fn runtime_world_position_from_effect_business_projection(
    projection: Option<&EffectBusinessProjection>,
) -> Option<RuntimeWorldPositionObservability> {
    match projection {
        Some(EffectBusinessProjection::ParentRef { x_bits, y_bits, .. })
        | Some(EffectBusinessProjection::WorldPosition { x_bits, y_bits, .. }) => {
            Some(RuntimeWorldPositionObservability {
                x_bits: *x_bits,
                y_bits: *y_bits,
            })
        }
        Some(EffectBusinessProjection::PositionTarget {
            target_x_bits,
            target_y_bits,
            ..
        })
        | Some(EffectBusinessProjection::PayloadTargetContent {
            target_x_bits,
            target_y_bits,
            ..
        })
        | Some(EffectBusinessProjection::LengthRay {
            target_x_bits,
            target_y_bits,
            ..
        }) => Some(RuntimeWorldPositionObservability {
            x_bits: *target_x_bits,
            y_bits: *target_y_bits,
        }),
        Some(EffectBusinessProjection::LightningPath { points }) => {
            points
                .last()
                .map(|(x_bits, y_bits)| RuntimeWorldPositionObservability {
                    x_bits: *x_bits,
                    y_bits: *y_bits,
                })
        }
        Some(EffectBusinessProjection::ContentRef { .. })
        | Some(EffectBusinessProjection::FloatValue(_))
        | None => None,
    }
}

fn runtime_world_position_from_named_effect_business_projection(
    contract_name: Option<&str>,
    projection: Option<&EffectBusinessProjection>,
) -> Option<RuntimeWorldPositionObservability> {
    effect_contract_executor::world_position_from_contract_business_projection(
        contract_name,
        projection,
    )
    .map(|(x_bits, y_bits)| RuntimeWorldPositionObservability { x_bits, y_bits })
}

fn runtime_world_position_observability(
    x_bits: Option<u32>,
    y_bits: Option<u32>,
) -> Option<RuntimeWorldPositionObservability> {
    Some(RuntimeWorldPositionObservability {
        x_bits: x_bits?,
        y_bits: y_bits?,
    })
}

fn runtime_build_ui_observability(
    snapshot_input: &ClientSnapshotInputState,
    projection: &BuilderQueueProjection,
    tile_config_projection: &TileConfigProjection,
    runtime_typed_building_projection: &TypedBuildingRuntimeProjection,
) -> BuildUiObservability {
    BuildUiObservability {
        selected_block_id: snapshot_input.selected_block_id,
        selected_rotation: snapshot_input.selected_rotation,
        building: snapshot_input.building,
        queued_count: projection.queued_count,
        inflight_count: projection.inflight_count,
        finished_count: projection.finished_count,
        removed_count: projection.removed_count,
        orphan_authoritative_count: projection.orphan_authoritative_count,
        head: runtime_build_queue_head_observability(projection),
        rollback_strip: runtime_build_config_rollback_strip_observability(tile_config_projection),
        inspector_entries: runtime_build_config_inspector_entries(
            runtime_typed_building_projection,
        ),
    }
}

fn runtime_build_config_rollback_strip_observability(
    projection: &TileConfigProjection,
) -> BuildConfigRollbackStripObservability {
    BuildConfigRollbackStripObservability {
        applied_authoritative_count: projection.applied_authoritative_count,
        rollback_count: projection.rollback_count,
        last_build_tile: projection
            .last_business_build_pos
            .map(unpack_runtime_point2),
        last_business_applied: projection.last_business_applied,
        last_cleared_pending_local: projection.last_cleared_pending_local,
        last_was_rollback: projection.last_was_rollback,
        last_pending_local_match: projection.last_pending_local_match,
        last_source: projection
            .last_business_source
            .map(runtime_build_config_authority_source_observability),
        last_configured_outcome: projection
            .last_configured_block_outcome
            .map(runtime_build_config_outcome_observability),
        last_configured_block_name: projection.last_configured_block_name.clone(),
    }
}

fn runtime_build_config_authority_source_observability(
    source: TileConfigAuthoritySource,
) -> BuildConfigAuthoritySourceObservability {
    match source {
        TileConfigAuthoritySource::TileConfigPacket => {
            BuildConfigAuthoritySourceObservability::TileConfig
        }
        TileConfigAuthoritySource::ConstructFinish => {
            BuildConfigAuthoritySourceObservability::ConstructFinish
        }
    }
}

fn runtime_build_config_outcome_observability(
    outcome: ConfiguredBlockOutcome,
) -> BuildConfigOutcomeObservability {
    match outcome {
        ConfiguredBlockOutcome::Applied => BuildConfigOutcomeObservability::Applied,
        ConfiguredBlockOutcome::RejectedMissingBuilding => {
            BuildConfigOutcomeObservability::RejectedMissingBuilding
        }
        ConfiguredBlockOutcome::RejectedMissingBlockMetadata => {
            BuildConfigOutcomeObservability::RejectedMissingBlockMetadata
        }
        ConfiguredBlockOutcome::RejectedUnsupportedBlock => {
            BuildConfigOutcomeObservability::RejectedUnsupportedBlock
        }
        ConfiguredBlockOutcome::RejectedUnsupportedConfigType => {
            BuildConfigOutcomeObservability::RejectedUnsupportedConfigType
        }
    }
}

fn runtime_build_queue_head_observability(
    projection: &BuilderQueueProjection,
) -> Option<BuildQueueHeadObservability> {
    Some(BuildQueueHeadObservability {
        x: projection.head_x?,
        y: projection.head_y?,
        breaking: projection.head_breaking?,
        block_id: projection.head_block_id,
        rotation: projection.head_rotation,
        stage: runtime_build_queue_head_stage(projection.head_stage?),
    })
}

fn runtime_build_queue_head_stage(stage: BuilderPlanStage) -> BuildQueueHeadStage {
    match stage {
        BuilderPlanStage::Queued => BuildQueueHeadStage::Queued,
        BuilderPlanStage::InFlight => BuildQueueHeadStage::InFlight,
        BuilderPlanStage::Finished => BuildQueueHeadStage::Finished,
        BuilderPlanStage::Removed => BuildQueueHeadStage::Removed,
    }
}

fn runtime_build_config_inspector_entries(
    projection: &TypedBuildingRuntimeProjection,
) -> Vec<BuildConfigInspectorEntryObservability> {
    let mut grouped: BTreeMap<TypedBuildingRuntimeKind, (usize, String)> = BTreeMap::new();
    for building in projection.buildings() {
        let sample = runtime_typed_build_config_sample(&building);
        grouped
            .entry(building.kind)
            .and_modify(|(count, current_sample)| {
                *count += 1;
                *current_sample = sample.clone();
            })
            .or_insert((1, sample));
    }
    grouped
        .into_iter()
        .map(
            |(kind, (tracked_count, sample))| BuildConfigInspectorEntryObservability {
                family: kind.family_name().to_string(),
                tracked_count,
                sample,
            },
        )
        .collect()
}

fn runtime_typed_build_config_sample(building: &TypedBuildingRuntimeModel) -> String {
    let mut sample = runtime_build_config_pos_label(building.build_pos);
    if building.block_name != building.kind.family_name() {
        sample.push(':');
        sample.push_str(&building.block_name);
    }
    sample.push(':');
    sample.push_str(&runtime_typed_build_config_value_label(
        building.kind,
        &building.value,
    ));
    sample
}

fn runtime_typed_build_config_value_label(
    kind: TypedBuildingRuntimeKind,
    value: &TypedBuildingRuntimeValue,
) -> String {
    match value {
        TypedBuildingRuntimeValue::Core => "core".to_string(),
        TypedBuildingRuntimeValue::Item(value) => format!(
            "{}={}",
            runtime_typed_build_config_item_label(kind),
            value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "clear".to_string())
        ),
        TypedBuildingRuntimeValue::Liquid(value) => format!(
            "liquid={}",
            value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "clear".to_string())
        ),
        TypedBuildingRuntimeValue::Bool(value) => format!(
            "{}={}",
            runtime_typed_build_config_bool_label(kind),
            value
                .map(|value| if value {
                    "1".to_string()
                } else {
                    "0".to_string()
                })
                .unwrap_or_else(|| "clear".to_string())
        ),
        TypedBuildingRuntimeValue::Text(text) => {
            if text.is_empty() {
                "text=empty".to_string()
            } else {
                format!(
                    "len={}:text={}",
                    text.chars().count(),
                    runtime_build_config_text_sample(text, 24),
                )
            }
        }
        TypedBuildingRuntimeValue::Constructor {
            recipe_block_id,
            progress_bits,
            payload_present,
            pay_rotation_bits,
            payload_build_block_id,
            payload_unit_class_id,
        } => {
            let mut parts = vec![format!(
                "recipe={}",
                recipe_block_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "clear".to_string())
            )];
            if let Some(progress_bits) = progress_bits {
                parts.push(format!("p{progress_bits:08x}"));
            }
            if let Some(payload_present) = payload_present {
                parts.push(format!("y{}", if *payload_present { 1 } else { 0 }));
            }
            if let Some(pay_rotation_bits) = pay_rotation_bits {
                parts.push(format!("r{pay_rotation_bits:08x}"));
            }
            if let Some(payload_build_block_id) = payload_build_block_id {
                parts.push(format!("payload=b:{payload_build_block_id}"));
            } else if let Some(payload_unit_class_id) = payload_unit_class_id {
                parts.push(format!("payload=uc:{payload_unit_class_id}"));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::Block(value) => format!(
            "recipe={}",
            value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "clear".to_string())
        ),
        TypedBuildingRuntimeValue::Color(value) => format!("color=0x{value:08x}"),
        TypedBuildingRuntimeValue::PayloadLoader {
            exporting,
            payload_present,
            payload_type,
            pay_rotation_bits,
            payload_build_block_id,
            payload_build_revision,
            payload_unit_class_id,
            payload_unit_payload_len,
            payload_unit_payload_sha256,
        } => {
            let mut parts = vec![format!(
                "mode={}",
                match exporting {
                    Some(true) => "export",
                    Some(false) => "import",
                    None => "unknown",
                }
            )];
            if let Some(payload_present) = payload_present {
                parts.push(format!("y{}", if *payload_present { 1 } else { 0 }));
            }
            if let Some(payload_type) = payload_type {
                parts.push(format!("payload-type={payload_type}"));
            }
            if let Some(pay_rotation_bits) = pay_rotation_bits {
                parts.push(format!("r{pay_rotation_bits:08x}"));
            }
            if let Some(payload_build_block_id) = payload_build_block_id {
                let mut payload_ref = format!("b:{payload_build_block_id}");
                if let Some(payload_build_revision) = payload_build_revision {
                    payload_ref.push_str(&format!("@r{payload_build_revision}"));
                }
                parts.push(format!("payload={payload_ref}"));
            } else if let Some(payload_unit_class_id) = payload_unit_class_id {
                parts.push(format!("payload=uc:{payload_unit_class_id}"));
            }
            if let Some(payload_unit_payload_len) = payload_unit_payload_len {
                parts.push(format!("unit-len={payload_unit_payload_len}"));
            }
            if let Some(payload_unit_payload_sha256) = payload_unit_payload_sha256.as_deref() {
                parts.push(format!(
                    "unit-sha={}",
                    payload_unit_payload_sha256
                        .chars()
                        .take(12)
                        .collect::<String>()
                ));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::PayloadSource {
            configured_content,
            command_pos,
            pay_vector_x_bits,
            pay_vector_y_bits,
            pay_rotation_bits,
            payload_present,
            payload_type,
            payload_build_block_id,
            payload_build_revision,
            payload_unit_class_id,
            payload_unit_payload_len,
            payload_unit_payload_sha256,
        } => {
            let mut parts = vec![format!(
                "content={}",
                configured_content
                    .as_ref()
                    .map(runtime_build_config_content_ref_label)
                    .unwrap_or_else(|| "clear".to_string())
            )];
            parts.push(format!(
                "command={}",
                runtime_optional_command_pos_bits_label(*command_pos)
            ));
            parts.push(format!(
                "payload={}",
                payload_present
                    .map(|value| if value { 1 } else { 0 }.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ));
            if let Some(payload_type) = payload_type {
                parts.push(format!("payload-type={payload_type}"));
            }
            if let (Some(pay_vector_x_bits), Some(pay_vector_y_bits)) =
                (pay_vector_x_bits, pay_vector_y_bits)
            {
                parts.push(format!(
                    "vec=0x{pay_vector_x_bits:08x}:0x{pay_vector_y_bits:08x}"
                ));
            }
            if let Some(pay_rotation_bits) = pay_rotation_bits {
                parts.push(format!("rot=0x{pay_rotation_bits:08x}"));
            }
            if let Some(payload_build_block_id) = payload_build_block_id {
                let mut payload_ref = format!("b:{payload_build_block_id}");
                if let Some(payload_build_revision) = payload_build_revision {
                    payload_ref.push_str(&format!("@r{payload_build_revision}"));
                }
                parts.push(format!("payload-ref={payload_ref}"));
            } else if let Some(payload_unit_class_id) = payload_unit_class_id {
                parts.push(format!("payload-ref=uc:{payload_unit_class_id}"));
            }
            if let Some(payload_unit_payload_len) = payload_unit_payload_len {
                parts.push(format!("unit-len={payload_unit_payload_len}"));
            }
            if let Some(payload_unit_payload_sha256) = payload_unit_payload_sha256.as_deref() {
                parts.push(format!(
                    "unit-sha={}",
                    payload_unit_payload_sha256.chars().take(12).collect::<String>()
                ));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::PayloadRouter {
            sorted_content,
            progress_bits,
            item_rotation_bits,
            payload_present,
            payload_type,
            payload_kind,
            payload_build_block_id,
            payload_build_revision,
            payload_unit_class_id,
            payload_unit_revision,
            payload_serialized_len,
            payload_serialized_sha256,
            rec_dir,
        } => {
            let mut parts = vec![format!(
                "content={}",
                sorted_content
                    .as_ref()
                    .map(runtime_build_config_content_ref_label)
                    .unwrap_or_else(|| "clear".to_string())
            )];
            parts.push(format!(
                "progress={}",
                progress_bits
                    .map(|bits| format!("0x{bits:08x}"))
                    .unwrap_or_else(|| "none".to_string())
            ));
            parts.push(format!(
                "item-rot={}",
                item_rotation_bits
                    .map(|bits| format!("0x{bits:08x}"))
                    .unwrap_or_else(|| "none".to_string())
            ));
            parts.push(format!(
                "payload={}",
                payload_present
                    .map(|value| if value { 1 } else { 0 }.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ));
            if let Some(payload_type) = payload_type {
                parts.push(format!("payload-type={payload_type}"));
            }
            if let Some(payload_kind) = payload_kind {
                parts.push(format!(
                    "payload-kind={}",
                    match payload_kind {
                        PayloadRouterPayloadKind::Null => "null",
                        PayloadRouterPayloadKind::Build => "build",
                        PayloadRouterPayloadKind::Unit => "unit",
                    }
                ));
            }
            if let Some(payload_build_block_id) = payload_build_block_id {
                let mut payload_ref = format!("b:{payload_build_block_id}");
                if let Some(payload_build_revision) = payload_build_revision {
                    payload_ref.push_str(&format!("@r{payload_build_revision}"));
                }
                parts.push(format!("payload-ref={payload_ref}"));
            } else if let Some(payload_unit_class_id) = payload_unit_class_id {
                let mut payload_ref = format!("uc:{payload_unit_class_id}");
                if let Some(payload_unit_revision) = payload_unit_revision {
                    payload_ref.push_str(&format!("@r{payload_unit_revision}"));
                }
                parts.push(format!("payload-ref={payload_ref}"));
            }
            if let Some(payload_serialized_len) = payload_serialized_len {
                parts.push(format!("payload-len={payload_serialized_len}"));
            }
            if let Some(payload_serialized_sha256) = payload_serialized_sha256.as_deref() {
                parts.push(format!(
                    "payload-sha={}",
                    payload_serialized_sha256.chars().take(12).collect::<String>()
                ));
            }
            if let Some(rec_dir) = rec_dir {
                parts.push(format!("rec-dir={rec_dir}"));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::MassDriver {
            link,
            rotation_bits,
            state_ordinal,
        } => {
            let mut parts = vec![format!(
                "link={}",
                link.map(runtime_build_config_pos_label)
                    .unwrap_or_else(|| "clear".to_string())
            )];
            if let Some(rotation_bits) = rotation_bits {
                parts.push(format!("rot=0x{rotation_bits:08x}"));
            }
            if let Some(state_ordinal) = state_ordinal {
                parts.push(format!("state={state_ordinal}"));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::PayloadMassDriver {
            link,
            turret_rotation_bits,
            state_ordinal,
            reload_counter_bits,
            charge_bits,
            loaded,
            charging,
            payload_present,
        } => {
            let mut parts = vec![format!(
                "link={}",
                link.map(runtime_build_config_pos_label)
                    .unwrap_or_else(|| "clear".to_string())
            )];
            if let Some(turret_rotation_bits) = turret_rotation_bits {
                parts.push(format!("rot=0x{turret_rotation_bits:08x}"));
            }
            if let Some(state_ordinal) = state_ordinal {
                parts.push(format!("state={state_ordinal}"));
            }
            if let Some(reload_counter_bits) = reload_counter_bits {
                parts.push(format!("reload=0x{reload_counter_bits:08x}"));
            }
            if let Some(charge_bits) = charge_bits {
                parts.push(format!("charge=0x{charge_bits:08x}"));
            }
            if let Some(loaded) = loaded {
                parts.push(format!("loaded={}", if *loaded { 1 } else { 0 }));
            }
            if let Some(charging) = charging {
                parts.push(format!("charging={}", if *charging { 1 } else { 0 }));
            }
            if let Some(payload_present) = payload_present {
                parts.push(format!("payload={}", if *payload_present { 1 } else { 0 }));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::Sorter {
            item_id,
            legacy,
            non_empty_side_mask,
            buffered_item_count,
        } => {
            let mut parts = vec![format!(
                "item={}",
                item_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "clear".to_string())
            )];
            if let Some(legacy) = legacy {
                parts.push(format!("legacy={}", if *legacy { 1 } else { 0 }));
            }
            if let Some(non_empty_side_mask) = non_empty_side_mask {
                parts.push(format!("sides=0x{non_empty_side_mask:02x}"));
            }
            if let Some(buffered_item_count) = buffered_item_count {
                parts.push(format!("buffered={buffered_item_count}"));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::Content(content) => format!(
            "content={}",
            content
                .as_ref()
                .map(runtime_build_config_content_ref_label)
                .unwrap_or_else(|| "clear".to_string())
        ),
        TypedBuildingRuntimeValue::Link(link) => format!(
            "link={}",
            link.map(runtime_build_config_pos_label)
                .unwrap_or_else(|| "clear".to_string())
        ),
        TypedBuildingRuntimeValue::Links(targets) => {
            if targets.is_empty() {
                "links=clear".to_string()
            } else {
                let links = targets
                    .iter()
                    .map(|target_pos| runtime_build_config_pos_label(*target_pos))
                    .collect::<Vec<_>>()
                    .join("|");
                format!("links={links}")
            }
        }
        TypedBuildingRuntimeValue::UnitFactory {
            current_plan,
            progress_bits,
            command_pos,
            command_id,
            payload_present,
            pay_rotation_bits,
        } => {
            let mut parts = vec![format!(
                "plan={}",
                current_plan
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            )];
            parts.push(format!(
                "progress={}",
                progress_bits
                    .map(|bits| format!("0x{bits:08x}"))
                    .unwrap_or_else(|| "none".to_string())
            ));
            parts.push(format!(
                "command={}",
                runtime_optional_command_pos_bits_label(*command_pos)
            ));
            parts.push(format!(
                "command-id={}",
                command_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ));
            parts.push(format!(
                "payload={}",
                payload_present
                    .map(|value| if value { 1 } else { 0 }.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ));
            parts.push(format!(
                "pay-rot={}",
                pay_rotation_bits
                    .map(|bits| format!("0x{bits:08x}"))
                    .unwrap_or_else(|| "none".to_string())
            ));
            parts.join(":")
        }
        TypedBuildingRuntimeValue::Reconstructor {
            command_id,
            progress_bits,
            command_pos,
            payload_present,
            pay_rotation_bits,
        } => {
            let mut parts = vec![format!(
                "command={}",
                command_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "clear".to_string())
            )];
            if let Some(progress_bits) = progress_bits {
                parts.push(format!("p{progress_bits:08x}"));
            }
            if let Some((x_bits, y_bits)) = command_pos {
                parts.push(format!("c0x{x_bits:08x}:0x{y_bits:08x}"));
            }
            if let Some(payload_present) = payload_present {
                parts.push(format!("y{}", if *payload_present { 1 } else { 0 }));
            }
            if let Some(pay_rotation_bits) = pay_rotation_bits {
                parts.push(format!("r{pay_rotation_bits:08x}"));
            }
            parts.join(":")
        }
        TypedBuildingRuntimeValue::UnitAssembler {
            progress_bits,
            unit_count,
            block_count,
            block_sample,
            command_pos,
            payload_present,
            pay_rotation_bits,
        } => format!(
            "progress=0x{progress_bits:08x}:units={unit_count}:blocks={block_count}:sample={}:command={}:payload={}:pay-rot=0x{pay_rotation_bits:08x}",
            block_sample
                .as_ref()
                .map(runtime_build_config_content_ref_label)
                .unwrap_or_else(|| "none".to_string()),
            runtime_optional_command_pos_bits_label(*command_pos),
            if *payload_present { 1 } else { 0 },
        ),
        TypedBuildingRuntimeValue::Turret {
            reload_counter_bits,
            rotation_bits,
        } => format!(
            "reload={}:rot={}",
            reload_counter_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string()),
            rotation_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        TypedBuildingRuntimeValue::ItemTurret {
            reload_counter_bits,
            rotation_bits,
            ammo_count,
        } => format!(
            "reload={}:rot={}:ammo={}",
            reload_counter_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string()),
            rotation_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string()),
            ammo_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        TypedBuildingRuntimeValue::ContinuousTurret {
            reload_counter_bits,
            rotation_bits,
            last_length_bits,
        } => format!(
            "reload={}:rot={}:len={}",
            reload_counter_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string()),
            rotation_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string()),
            last_length_bits
                .map(|bits| format!("0x{bits:08x}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        TypedBuildingRuntimeValue::BuildTower {
            rotation_bits,
            plans_present,
            plan_count,
        } => {
            let plans = match plans_present {
                Some(true) => plan_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                Some(false) => "none".to_string(),
                None => "unknown".to_string(),
            };
            format!(
                "rot={}:plans={plans}",
                rotation_bits
                    .map(|bits| format!("0x{bits:08x}"))
                    .unwrap_or_else(|| "none".to_string())
            )
        }
        TypedBuildingRuntimeValue::Bytes(bytes) => format!(
            "len={}:hex={}",
            bytes.len(),
            runtime_build_config_bytes_sample(bytes, 8),
        ),
        TypedBuildingRuntimeValue::Memory(words) => format!(
            "len={}:bits={}",
            words.len(),
            runtime_build_config_memory_words_sample(words, 4),
        ),
    }
}

fn runtime_typed_build_config_item_label(kind: TypedBuildingRuntimeKind) -> &'static str {
    match kind {
        TypedBuildingRuntimeKind::LiquidSource => "liquid",
        TypedBuildingRuntimeKind::Constructor => "recipe",
        _ => "item",
    }
}

fn runtime_typed_build_config_bool_label(kind: TypedBuildingRuntimeKind) -> &'static str {
    match kind {
        TypedBuildingRuntimeKind::Door => "open",
        _ => "enabled",
    }
}

fn runtime_build_config_pos_label(build_pos: i32) -> String {
    let (x, y) = unpack_runtime_point2(build_pos);
    format!("{x}:{y}")
}

fn runtime_build_config_text_sample(text: &str, limit: usize) -> String {
    let mut sample = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index == limit {
            sample.push('~');
            break;
        }
        sample.push(match ch {
            ' ' | '\t' | '\r' | '\n' => '_',
            _ => ch,
        });
    }
    if sample.is_empty() {
        "empty".to_string()
    } else {
        sample
    }
}

fn runtime_build_config_bytes_sample(bytes: &[u8], limit: usize) -> String {
    let mut sample = String::new();
    for (index, byte) in bytes.iter().take(limit).enumerate() {
        if index > 0 {
            sample.push('-');
        }
        sample.push_str(&format!("{byte:02x}"));
    }
    if bytes.len() > limit {
        sample.push('~');
    }
    if sample.is_empty() {
        "empty".to_string()
    } else {
        sample
    }
}

fn runtime_build_config_memory_words_sample(words: &[u64], limit: usize) -> String {
    let mut sample = String::new();
    for (index, word) in words.iter().take(limit).enumerate() {
        if index > 0 {
            sample.push('-');
        }
        sample.push_str(&format!("{word:016x}"));
    }
    if words.len() > limit {
        sample.push('~');
    }
    if sample.is_empty() {
        "empty".to_string()
    } else {
        sample
    }
}

fn runtime_build_config_content_ref_label(content: &ConfiguredContentRef) -> String {
    let kind = match content.content_type {
        1 => "b",
        6 => "u",
        _ => "c",
    };
    format!("{kind}:{}", content.content_id)
}

fn runtime_optional_command_pos_bits_label(value: Option<(u32, u32)>) -> String {
    value
        .map(|(x_bits, y_bits)| format!("0x{x_bits:08x}:0x{y_bits:08x}"))
        .unwrap_or_else(|| "none".to_string())
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
        "sr{}:srf{}:so{}:sof{}:rule{}:rf{}:clr{}:cmp{}:wv{}:pvp{}:uc{}:dt{}:wt{}:iws{}:obj{}:q{}:par{}:fg{}:oor{}:last{}",
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
        runtime_optional_display_label(session_state.rules_projection.unit_cap),
        runtime_optional_display_label(session_state.rules_projection.default_team_id),
        runtime_optional_display_label(session_state.rules_projection.wave_team_id),
        runtime_optional_display_label(session_state.rules_projection.initial_wave_spacing),
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

fn runtime_chat_label(session_state: &SessionState) -> String {
    let observability = runtime_chat_observability(session_state);
    format!(
        "srv{}@{}:msg{}@{}:raw{}:s{}",
        observability.server_message_count,
        runtime_compact_text_label(observability.last_server_message.as_deref()),
        observability.chat_message_count,
        runtime_compact_text_label(observability.last_chat_message.as_deref()),
        runtime_compact_text_label(observability.last_chat_unformatted.as_deref()),
        runtime_optional_display_label(observability.last_chat_sender_entity_id),
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
    value.map_or_else(|| "none".to_string(), |value| format!("0x{value:08x}"))
}

fn runtime_optional_bits_pair_label(x_bits: Option<u32>, y_bits: Option<u32>) -> String {
    match (x_bits, y_bits) {
        (Some(x_bits), Some(y_bits)) => format!("0x{x_bits:08x}:0x{y_bits:08x}"),
        _ => "none".to_string(),
    }
}

fn runtime_optional_text_len_label(value: Option<&str>) -> String {
    value.map_or_else(|| "none".to_string(), |value| format!("len{}", value.len()))
}

fn runtime_optional_unit_ref_label(value: Option<UnitRefProjection>) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| format!("{}:{}", value.kind, value.value),
    )
}

fn runtime_world_label_label(session_state: &SessionState) -> String {
    let observability = runtime_world_label_observability(session_state);
    format!(
        "lbl{}:lblr{}:rml{}:act{}:last{}:f{}:fs{}:z{}:pos{}:txt{}",
        observability.label_count,
        observability.reliable_label_count,
        observability.remove_label_count,
        observability.active_count,
        runtime_optional_display_label(observability.last_entity_id),
        runtime_optional_display_label(observability.last_flags),
        runtime_optional_display_label(observability.last_font_size_bits),
        runtime_optional_display_label(observability.last_z_bits),
        runtime_optional_world_label_position_label(observability.last_position),
        runtime_optional_world_label_text_label(observability.last_text.as_deref()),
    )
}

fn runtime_optional_world_label_position_label(
    value: Option<RuntimeWorldPositionObservability>,
) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| {
            let x = f32::from_bits(value.x_bits);
            let y = f32::from_bits(value.y_bits);
            if x.is_finite() && y.is_finite() {
                format!("{x:.1}:{y:.1}")
            } else {
                format!("0x{:08x}:0x{:08x}", value.x_bits, value.y_bits)
            }
        },
    )
}

fn runtime_optional_world_label_text_label(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    runtime_build_config_text_sample(value, 16).replace(' ', "_")
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
        "flag{}:go{}:ugo{}:sc{}:res{}:wave{}@{}>{}#{}",
        session_state.received_set_flag_count,
        session_state.received_game_over_count,
        session_state.received_update_game_over_count,
        session_state.received_sector_capture_count,
        session_state.received_researched_count,
        session_state.received_wave_advance_signal_count,
        session_state
            .last_wave_advance_signal_from
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        session_state
            .last_wave_advance_signal_to
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        session_state
            .last_wave_advance_signal_apply_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
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

fn runtime_effect_binding_label(
    snapshot_input: &ClientSnapshotInputState,
    session_state: &SessionState,
    world_overlay: &RuntimeWorldOverlay,
) -> String {
    let input_view = EffectRuntimeInputView {
        unit_id: snapshot_input.unit_id,
        position: snapshot_input.position,
        rotation: snapshot_input.rotation,
    };
    let active_overlay = world_overlay.effect_overlays.last();
    let overlay_target = active_overlay.and_then(|overlay| {
        observe_runtime_effect_overlay_binding_state(overlay, session_state, &input_view)
    });
    let overlay_source = active_overlay.and_then(|overlay| {
        observe_runtime_effect_overlay_source_binding_state(overlay, session_state, &input_view)
    });
    let session_target = session_state.last_effect_runtime_binding_state;
    let session_source = session_state.last_effect_runtime_source_binding_state;
    let (target, source) = if session_target.is_some() || session_source.is_some() {
        (session_target, session_source)
    } else {
        (overlay_target, overlay_source)
    };
    format!(
        "{}/{}",
        runtime_optional_effect_binding_state_label(target),
        runtime_optional_effect_binding_state_label(source)
    )
}

fn runtime_optional_effect_binding_state_label(
    state: Option<EffectRuntimeBindingState>,
) -> &'static str {
    state.map_or("none", EffectRuntimeBindingState::as_str)
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

fn runtime_world_xy_bits_from_tile_pos(tile_pos: i32) -> (u32, u32) {
    const TILE_SIZE: f32 = 8.0;

    let (tile_x, tile_y) = unpack_runtime_point2(tile_pos);
    (
        (tile_x as f32 * TILE_SIZE).to_bits(),
        (tile_y as f32 * TILE_SIZE).to_bits(),
    )
}

fn append_runtime_command_mode_overlay_objects(
    scene: &mut RenderModel,
    snapshot_input: &ClientSnapshotInputState,
    session_state: &SessionState,
) {
    let command_mode = &snapshot_input.command_mode;
    if !command_mode.active {
        return;
    }

    for &entity_id in &command_mode.selected_units {
        let Some(entity) = session_state
            .entity_table_projection
            .by_entity_id
            .get(&entity_id)
        else {
            continue;
        };
        let x = f32::from_bits(entity.x_bits);
        let y = f32::from_bits(entity.y_bits);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        scene.objects.push(RenderObject {
            id: format!("marker:runtime-command-selected-unit:{entity_id}"),
            layer: 29,
            x,
            y,
        });
    }

    for &build_pos in &command_mode.command_buildings {
        let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
        let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(build_pos);
        scene.objects.push(RenderObject {
            id: format!("marker:runtime-command-building:{tile_x}:{tile_y}"),
            layer: 29,
            x: f32::from_bits(x_bits),
            y: f32::from_bits(y_bits),
        });
    }

    if let Some(rect) = command_mode.command_rect {
        append_runtime_command_mode_rect_objects(scene, rect);
    }

    if let Some(target) = command_mode.last_target {
        if let Some(build_pos) = target.build_target {
            let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
            let (x_bits, y_bits) = runtime_world_xy_bits_from_tile_pos(build_pos);
            scene.objects.push(RenderObject {
                id: format!("marker:runtime-command-build-target:{tile_x}:{tile_y}"),
                layer: 29,
                x: f32::from_bits(x_bits),
                y: f32::from_bits(y_bits),
            });
        }
        if let Some(position) = target.position_target {
            let x = f32::from_bits(position.x_bits);
            let y = f32::from_bits(position.y_bits);
            if x.is_finite() && y.is_finite() {
                scene.objects.push(RenderObject {
                    id: format!(
                        "marker:runtime-command-position-target:0x{:08x}:0x{:08x}",
                        position.x_bits, position.y_bits
                    ),
                    layer: 29,
                    x,
                    y,
                });
            }
        }
        if let Some(unit_target) = target.unit_target {
            if let Some(entity) = session_state
                .entity_table_projection
                .by_entity_id
                .get(&unit_target.value)
            {
                let x = f32::from_bits(entity.x_bits);
                let y = f32::from_bits(entity.y_bits);
                if x.is_finite() && y.is_finite() {
                    scene.objects.push(RenderObject {
                        id: format!(
                            "marker:runtime-command-unit-target:{}:{}",
                            unit_target.kind, unit_target.value
                        ),
                        layer: 29,
                        x,
                        y,
                    });
                }
            }
        }
        if let Some(rect) = target.rect_target {
            append_runtime_command_mode_target_rect_objects(scene, rect);
        }
        if let Some((target_x, target_y)) =
            runtime_command_target_world_position(target, session_state)
        {
            append_runtime_command_mode_target_lines(
                scene,
                &command_mode.selected_units,
                (target_x, target_y),
                session_state,
            );
            append_runtime_command_mode_building_target_lines(
                scene,
                &command_mode.command_buildings,
                (target_x, target_y),
            );
        }
    }
}

fn append_runtime_command_mode_target_lines(
    scene: &mut RenderModel,
    selected_units: &[i32],
    target: (f32, f32),
    session_state: &SessionState,
) {
    const LINK_LAYER: i32 = 29;

    for (ordinal, &entity_id) in selected_units.iter().take(4).enumerate() {
        let Some(entity) = session_state
            .entity_table_projection
            .by_entity_id
            .get(&entity_id)
        else {
            continue;
        };
        let source_x = f32::from_bits(entity.x_bits);
        let source_y = f32::from_bits(entity.y_bits);
        if !source_x.is_finite() || !source_y.is_finite() {
            continue;
        }
        if source_x == target.0 && source_y == target.1 {
            continue;
        }
        let line_id = format!(
            "marker:line:runtime-command-target-link:{ordinal}:{}:{}:{}:{}",
            source_x.to_bits(),
            source_y.to_bits(),
            target.0.to_bits(),
            target.1.to_bits()
        );
        scene.objects.push(RenderObject {
            id: line_id.clone(),
            layer: LINK_LAYER,
            x: source_x,
            y: source_y,
        });
        scene.objects.push(RenderObject {
            id: format!("{line_id}:line-end"),
            layer: LINK_LAYER,
            x: target.0,
            y: target.1,
        });
    }
}

fn append_runtime_command_mode_building_target_lines(
    scene: &mut RenderModel,
    command_buildings: &[i32],
    target: (f32, f32),
) {
    const TILE_SIZE: f32 = 8.0;
    const LINK_LAYER: i32 = 29;

    for (ordinal, &build_pos) in command_buildings.iter().take(4).enumerate() {
        let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
        let source_x = (tile_x as f32 + 0.5) * TILE_SIZE;
        let source_y = (tile_y as f32 + 0.5) * TILE_SIZE;
        if source_x == target.0 && source_y == target.1 {
            continue;
        }
        let line_id = format!(
            "marker:line:runtime-command-building-target-link:{ordinal}:{}:{}:{}:{}",
            source_x.to_bits(),
            source_y.to_bits(),
            target.0.to_bits(),
            target.1.to_bits()
        );
        scene.objects.push(RenderObject {
            id: line_id.clone(),
            layer: LINK_LAYER,
            x: source_x,
            y: source_y,
        });
        scene.objects.push(RenderObject {
            id: format!("{line_id}:line-end"),
            layer: LINK_LAYER,
            x: target.0,
            y: target.1,
        });
    }
}

fn runtime_command_target_world_position(
    target: mdt_input::CommandModeTargetProjection,
    session_state: &SessionState,
) -> Option<(f32, f32)> {
    const TILE_SIZE: f32 = 8.0;

    if let Some(position) = target.position_target {
        let x = f32::from_bits(position.x_bits);
        let y = f32::from_bits(position.y_bits);
        if x.is_finite() && y.is_finite() {
            return Some((x, y));
        }
    }
    if let Some(unit_target) = target.unit_target {
        let entity = session_state
            .entity_table_projection
            .by_entity_id
            .get(&unit_target.value)?;
        let x = f32::from_bits(entity.x_bits);
        let y = f32::from_bits(entity.y_bits);
        if x.is_finite() && y.is_finite() {
            return Some((x, y));
        }
    }
    let build_pos = target.build_target?;
    let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
    Some((
        (tile_x as f32 + 0.5) * TILE_SIZE,
        (tile_y as f32 + 0.5) * TILE_SIZE,
    ))
}

fn append_runtime_command_mode_rect_objects(
    scene: &mut RenderModel,
    rect: mdt_input::CommandModeRectProjection,
) {
    append_runtime_command_mode_rect_outline(scene, "runtime-command-rect", rect, 29);
}

fn append_runtime_command_mode_target_rect_objects(
    scene: &mut RenderModel,
    rect: mdt_input::CommandModeRectProjection,
) {
    append_runtime_command_mode_rect_outline(scene, "runtime-command-target-rect", rect, 29);
}

fn append_runtime_command_mode_rect_outline(
    scene: &mut RenderModel,
    family: &str,
    rect: mdt_input::CommandModeRectProjection,
    layer: i32,
) {
    const TILE_SIZE: f32 = 8.0;

    let rect = rect.normalized();
    let left = rect.x0 as f32 * TILE_SIZE;
    let top = rect.y0 as f32 * TILE_SIZE;
    let right = rect.x1 as f32 * TILE_SIZE;
    let bottom = rect.y1 as f32 * TILE_SIZE;
    for (edge, source, target) in [
        ("top", (left, top), (right, top)),
        ("right", (right, top), (right, bottom)),
        ("bottom", (right, bottom), (left, bottom)),
        ("left", (left, bottom), (left, top)),
    ] {
        let line_id = format!(
            "marker:line:{family}:{edge}:{}:{}:{}:{}",
            source.0.to_bits(),
            source.1.to_bits(),
            target.0.to_bits(),
            target.1.to_bits()
        );
        scene.objects.push(RenderObject {
            id: line_id.clone(),
            layer,
            x: source.0,
            y: source.1,
        });
        if source != target {
            scene.objects.push(RenderObject {
                id: format!("{line_id}:line-end"),
                layer,
                x: target.0,
                y: target.1,
            });
        }
    }
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
        if plan.breaking {
            append_runtime_break_plan_rect_outline(scene, plan.tile.0, plan.tile.1);
        } else {
            append_runtime_build_plan_config_top_objects(scene, index, plan);
        }
    }
}

fn append_runtime_build_plan_config_top_objects(
    scene: &mut RenderModel,
    index: usize,
    plan: &ClientBuildPlan,
) {
    match &plan.config {
        ClientBuildPlanConfig::Content {
            content_type,
            content_id,
        }
        | ClientBuildPlanConfig::TechNodeRaw {
            content_type,
            content_id,
        } => append_runtime_build_plan_config_content_icon(
            scene,
            plan.tile,
            "plan-content",
            *content_type,
            *content_id,
        ),
        ClientBuildPlanConfig::Point2 { x, y } => {
            append_runtime_build_plan_config_link(scene, index, 0, plan.tile, (*x, *y));
        }
        ClientBuildPlanConfig::Point2Array(points) => {
            for (ordinal, &(x, y)) in points.iter().take(4).enumerate() {
                append_runtime_build_plan_config_link(scene, index, ordinal, plan.tile, (x, y));
            }
        }
        ClientBuildPlanConfig::BuildingPos(value) => {
            append_runtime_build_plan_config_link(
                scene,
                index,
                0,
                plan.tile,
                unpack_runtime_point2(*value),
            );
        }
        _ => {}
    }
}

fn append_runtime_build_plan_config_content_icon(
    scene: &mut RenderModel,
    tile: (i32, i32),
    family: &str,
    content_type: u8,
    content_id: i16,
) {
    const TILE_SIZE: f32 = 8.0;
    const ICON_LAYER: i32 = 23;

    scene.objects.push(RenderObject {
        id: format!(
            "marker:runtime-build-config-icon:{family}:{}:{}:{content_type}:{content_id}",
            tile.0, tile.1
        ),
        layer: ICON_LAYER,
        x: tile.0 as f32 * TILE_SIZE,
        y: tile.1 as f32 * TILE_SIZE,
    });
}

fn append_runtime_build_plan_config_link(
    scene: &mut RenderModel,
    index: usize,
    ordinal: usize,
    source_tile: (i32, i32),
    target_tile: (i32, i32),
) {
    const TILE_SIZE: f32 = 8.0;
    const LINK_LAYER: i32 = 22;

    if source_tile == target_tile {
        return;
    }

    let source = (
        (source_tile.0 as f32 + 0.5) * TILE_SIZE,
        (source_tile.1 as f32 + 0.5) * TILE_SIZE,
    );
    let target = (
        (target_tile.0 as f32 + 0.5) * TILE_SIZE,
        (target_tile.1 as f32 + 0.5) * TILE_SIZE,
    );
    let line_id = format!(
        "marker:line:runtime-plan-config-link:{index}:{ordinal}:{}:{}:{}:{}",
        source.0.to_bits(),
        source.1.to_bits(),
        target.0.to_bits(),
        target.1.to_bits()
    );
    scene.objects.push(RenderObject {
        id: line_id.clone(),
        layer: LINK_LAYER,
        x: source.0,
        y: source.1,
    });
    scene.objects.push(RenderObject {
        id: format!("{line_id}:line-end"),
        layer: LINK_LAYER,
        x: target.0,
        y: target.1,
    });
}

fn append_runtime_break_plan_rect_outline(scene: &mut RenderModel, tile_x: i32, tile_y: i32) {
    const TILE_SIZE: f32 = 8.0;
    const RECT_LAYER: i32 = 30;

    let left = tile_x as f32 * TILE_SIZE;
    let top = tile_y as f32 * TILE_SIZE;
    let right = (tile_x as f32 + 1.0) * TILE_SIZE;
    let bottom = (tile_y as f32 + 1.0) * TILE_SIZE;
    for (edge, source, target) in [
        ("top", (left, top), (right, top)),
        ("right", (right, top), (right, bottom)),
        ("bottom", (right, bottom), (left, bottom)),
        ("left", (left, bottom), (left, top)),
    ] {
        let line_id = format!(
            "marker:line:runtime-break-rect:{edge}:{}:{}:{}:{}",
            source.0.to_bits(),
            source.1.to_bits(),
            target.0.to_bits(),
            target.1.to_bits()
        );
        scene.objects.push(RenderObject {
            id: line_id.clone(),
            layer: RECT_LAYER,
            x: source.0,
            y: source.1,
        });
        scene.objects.push(RenderObject {
            id: format!("{line_id}:line-end"),
            layer: RECT_LAYER,
            x: target.0,
            y: target.1,
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
    runtime_world_overlay: &mut RuntimeWorldOverlay,
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

    for overlay in &runtime_world_overlay.world_label_overlays {
        let x = f32::from_bits(overlay.x_bits);
        let y = f32::from_bits(overlay.y_bits);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        scene.objects.push(RenderObject {
            id: render_text_scene_object_id(
                format!("world-label:event:{}", overlay.overlay_key),
                overlay.message.as_deref(),
            ),
            layer: 39,
            x,
            y,
        });
    }

    for overlay in &runtime_world_overlay.world_event_markers {
        let x = f32::from_bits(overlay.x_bits);
        let y = f32::from_bits(overlay.y_bits);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        scene.objects.push(RenderObject {
            id: overlay.object_id.clone(),
            layer: overlay.layer,
            x,
            y,
        });
    }

    let effect_input_view = EffectRuntimeInputView {
        unit_id: snapshot_input.unit_id,
        position: snapshot_input.position,
        rotation: snapshot_input.rotation,
    };

    for overlay in &mut runtime_world_overlay.effect_overlays {
        if overlay.remaining_ticks > overlay.lifetime_ticks {
            continue;
        }
        let (source_x_bits, source_y_bits) = resolve_runtime_effect_overlay_source_position(
            overlay,
            session_state,
            &effect_input_view,
        );
        let (target_x_bits, target_y_bits) =
            resolve_runtime_effect_overlay_position(overlay, session_state, &effect_input_view);
        append_runtime_effect_executor_objects(
            scene,
            overlay,
            source_x_bits,
            source_y_bits,
            target_x_bits,
            target_y_bits,
            session_state,
        );
        let (marker_x_bits, marker_y_bits) =
            effect_contract_executor::marker_position_for_effect_overlay(
                overlay,
                source_x_bits,
                source_y_bits,
                target_x_bits,
                target_y_bits,
            )
            .unwrap_or((target_x_bits, target_y_bits));
        let reliable = runtime_effect_delivery_label(overlay.reliable);
        let data = if overlay.has_data { 1 } else { 0 };
        scene.objects.push(RenderObject {
            id: format!(
                "marker:runtime-effect:{reliable}:{}:0x{:08x}:0x{:08x}:{}",
                overlay.effect_id.unwrap_or(-1),
                marker_x_bits,
                marker_y_bits,
                data
            ),
            layer: 26,
            x: f32::from_bits(marker_x_bits),
            y: f32::from_bits(marker_y_bits),
        });
    }
}

fn append_runtime_effect_executor_objects(
    scene: &mut RenderModel,
    overlay: &RuntimeEffectOverlay,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
    session_state: &SessionState,
) {
    for line in effect_contract_executor::line_projections_for_effect_overlay(
        overlay,
        source_x_bits,
        source_y_bits,
        target_x_bits,
        target_y_bits,
        session_state,
    ) {
        let line_id = runtime_effect_line_object_id(
            line.kind,
            overlay.effect_id,
            overlay.reliable,
            line.source_x_bits,
            line.source_y_bits,
            line.target_x_bits,
            line.target_y_bits,
        );
        scene.objects.push(RenderObject {
            id: line_id.clone(),
            layer: 25,
            x: f32::from_bits(line.source_x_bits),
            y: f32::from_bits(line.source_y_bits),
        });

        if (line.source_x_bits, line.source_y_bits) != (line.target_x_bits, line.target_y_bits) {
            scene.objects.push(RenderObject {
                id: format!("{line_id}:line-end"),
                layer: 25,
                x: f32::from_bits(line.target_x_bits),
                y: f32::from_bits(line.target_y_bits),
            });
        }
    }

    for content in effect_contract_executor::content_projections_for_effect_overlay(
        overlay,
        target_x_bits,
        target_y_bits,
    ) {
        scene.objects.push(RenderObject {
            id: runtime_effect_content_object_id(
                content.kind,
                overlay.effect_id,
                overlay.reliable,
                content.content_type,
                content.content_id,
                content.x_bits,
                content.y_bits,
            ),
            layer: 27,
            x: f32::from_bits(content.x_bits),
            y: f32::from_bits(content.y_bits),
        });
    }
}

fn runtime_effect_delivery_label(reliable: bool) -> &'static str {
    if reliable {
        "reliable"
    } else {
        "normal"
    }
}

fn runtime_effect_line_object_id(
    kind: &str,
    effect_id: Option<i16>,
    reliable: bool,
    source_x_bits: u32,
    source_y_bits: u32,
    target_x_bits: u32,
    target_y_bits: u32,
) -> String {
    format!(
        "marker:line:runtime-effect-{kind}:{}:{}:0x{:08x}:0x{:08x}:0x{:08x}:0x{:08x}",
        runtime_effect_delivery_label(reliable),
        effect_id.unwrap_or(-1),
        source_x_bits,
        source_y_bits,
        target_x_bits,
        target_y_bits,
    )
}

fn runtime_effect_content_object_id(
    kind: &str,
    effect_id: Option<i16>,
    reliable: bool,
    content_type: u8,
    content_id: i16,
    x_bits: u32,
    y_bits: u32,
) -> String {
    format!(
        "marker:runtime-effect-icon:{kind}:{}:{}:{}:{}:0x{:08x}:0x{:08x}",
        runtime_effect_delivery_label(reliable),
        effect_id.unwrap_or(-1),
        content_type,
        content_id,
        x_bits,
        y_bits,
    )
}

fn append_runtime_ping_location_objects(scene: &mut RenderModel, session_state: &SessionState) {
    let projection = &session_state.ping_location_projection;
    if projection.received_count == 0 {
        return;
    }

    let (Some(x_bits), Some(y_bits)) = (projection.last_x_bits, projection.last_y_bits) else {
        return;
    };
    let x = f32::from_bits(x_bits);
    let y = f32::from_bits(y_bits);
    if !x.is_finite() || !y.is_finite() {
        return;
    }

    let text = projection
        .last_text
        .as_deref()
        .filter(|text| !text.is_empty())
        .unwrap_or("ping");
    scene.objects.push(RenderObject {
        id: render_text_scene_object_id(
            format!(
                "marker:text:runtime-ping:{}",
                projection.last_player_id.unwrap_or(-1)
            ),
            Some(text),
        ),
        layer: 31,
        x,
        y,
    });
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

fn append_runtime_live_entity_objects(scene: &mut RenderModel, session_state: &SessionState) {
    let projection = session_state.runtime_typed_entity_projection();
    if projection.by_entity_id.is_empty() {
        return;
    }

    let has_player_focus_object = scene
        .objects
        .iter()
        .any(|object| object.id.starts_with("player:"));

    let local_player_entity_id = projection
        .local_player()
        .map(|player| player.base.entity_id);
    if !has_player_focus_object {
        if let Some(local_player_entity_id) = local_player_entity_id {
            if let Some(model) = projection.entity_at(local_player_entity_id) {
                if let Some(object) = runtime_live_entity_scene_object(model) {
                    scene.objects.push(object);
                }
            }
        }
    }

    append_runtime_live_entity_non_local_objects(scene, &projection, local_player_entity_id);
}

fn append_runtime_live_entity_non_local_objects(
    scene: &mut RenderModel,
    projection: &TypedRuntimeEntityProjection,
    local_player_entity_id: Option<i32>,
) {
    for (&entity_id, model) in &projection.by_entity_id {
        if Some(entity_id) == local_player_entity_id {
            continue;
        }
        if let Some(object) = runtime_live_entity_scene_object(model) {
            scene.objects.push(object);
        }
    }
}

fn runtime_live_entity_scene_object(model: &TypedRuntimeEntityModel) -> Option<RenderObject> {
    let base = model.base();
    if base.hidden {
        return None;
    }

    let (x, y) = runtime_live_entity_world_xy(model)?;
    Some(RenderObject {
        id: runtime_live_entity_scene_object_id(model),
        layer: runtime_live_entity_layer(model),
        x,
        y,
    })
}

fn runtime_live_entity_scene_object_id(model: &TypedRuntimeEntityModel) -> String {
    let entity_id = model.entity_id();
    match model {
        TypedRuntimeEntityModel::Player(_) => format!("player:{entity_id}"),
        TypedRuntimeEntityModel::Unit(_) => format!("unit:{entity_id}"),
        TypedRuntimeEntityModel::Fire(_) => format!("fire:{entity_id}"),
        TypedRuntimeEntityModel::Puddle(_) => format!("puddle:{entity_id}"),
        TypedRuntimeEntityModel::WeatherState(_) => format!("weather:{entity_id}"),
        TypedRuntimeEntityModel::WorldLabel(world_label) => render_text_scene_object_id(
            format!("world-label:{entity_id}"),
            world_label.semantic.text.as_deref(),
        ),
    }
}

fn render_text_scene_object_id(base_id: String, text: Option<&str>) -> String {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return base_id;
    };

    let encoded_text = text
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    format!("{base_id}:text:{encoded_text}")
}

fn runtime_live_entity_world_xy(model: &TypedRuntimeEntityModel) -> Option<(f32, f32)> {
    let base = model.base();
    let x = f32::from_bits(base.x_bits);
    let y = f32::from_bits(base.y_bits);
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn runtime_live_entity_layer(model: &TypedRuntimeEntityModel) -> i32 {
    match model {
        TypedRuntimeEntityModel::Player(_) => 41,
        TypedRuntimeEntityModel::Unit(_) => 40,
        TypedRuntimeEntityModel::WorldLabel(_) => 39,
        TypedRuntimeEntityModel::WeatherState(_) => 38,
        TypedRuntimeEntityModel::Puddle(_) => 37,
        TypedRuntimeEntityModel::Fire(_) => 36,
    }
}

fn append_building_projection_objects(
    scene: &mut RenderModel,
    building_projection_by_build_pos: &BTreeMap<i32, BuildingProjection>,
    runtime_projection: &TypedBuildingRuntimeProjection,
    configured_block_projection: &ConfiguredBlockProjection,
) {
    const TILE_SIZE: f32 = 8.0;

    for (&build_pos, building) in building_projection_by_build_pos {
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

    append_runtime_building_markers(scene, &runtime_projection);

    append_runtime_build_config_content_icons(
        scene,
        "payload-source",
        &configured_block_projection.payload_source_content_by_build_pos,
    );
    append_runtime_build_config_content_icons(
        scene,
        "payload-router",
        &configured_block_projection.payload_router_sorted_content_by_build_pos,
    );
}

fn append_runtime_building_markers(
    scene: &mut RenderModel,
    projection: &TypedBuildingRuntimeProjection,
) {
    for building in projection.buildings() {
        match &building.value {
            TypedBuildingRuntimeValue::PayloadSource { command_pos, .. } => {
                append_runtime_payload_source_objects(scene, building, *command_pos);
            }
            TypedBuildingRuntimeValue::MassDriver {
                link: Some(target_build_pos),
                ..
            } if building.kind == TypedBuildingRuntimeKind::MassDriver => {
                append_runtime_driver_link_objects(
                    scene,
                    building.block_name.as_str(),
                    building.build_pos,
                    *target_build_pos,
                );
            }
            TypedBuildingRuntimeValue::PayloadMassDriver {
                link: Some(target_build_pos),
                ..
            } if building.kind == TypedBuildingRuntimeKind::PayloadMassDriver => {
                append_runtime_driver_link_objects(
                    scene,
                    building.block_name.as_str(),
                    building.build_pos,
                    *target_build_pos,
                );
            }
            TypedBuildingRuntimeValue::UnitAssembler {
                progress_bits,
                unit_count,
                block_count,
                block_sample,
                command_pos,
                payload_present,
                pay_rotation_bits,
            } => append_runtime_unit_assembler_objects(
                scene,
                building,
                *progress_bits,
                *unit_count,
                *block_count,
                block_sample.as_ref(),
                *command_pos,
                *payload_present,
                *pay_rotation_bits,
            ),
            _ => {}
        }
    }
}

fn append_runtime_payload_source_objects(
    scene: &mut RenderModel,
    building: &TypedBuildingRuntimeModel,
    command_pos: Option<(u32, u32)>,
) {
    let Some((command_x_bits, command_y_bits)) = command_pos else {
        return;
    };
    let (tile_x, tile_y) = unpack_runtime_point2(building.build_pos);
    scene.objects.push(RenderObject {
        id: format!(
            "marker:runtime-payload-source-command:{}:{tile_x}:{tile_y}:0x{command_x_bits:08x}:0x{command_y_bits:08x}",
            building.block_name,
        ),
        layer: 16,
        x: f32::from_bits(command_x_bits),
        y: f32::from_bits(command_y_bits),
    });
}

fn append_runtime_unit_assembler_objects(
    scene: &mut RenderModel,
    building: &TypedBuildingRuntimeModel,
    progress_bits: u32,
    unit_count: usize,
    block_count: usize,
    block_sample: Option<&ConfiguredContentRef>,
    command_pos: Option<(u32, u32)>,
    payload_present: bool,
    pay_rotation_bits: u32,
) {
    const TILE_SIZE: f32 = 8.0;

    let (tile_x, tile_y) = unpack_runtime_point2(building.build_pos);
    append_runtime_unit_assembler_area_objects(scene, building.block_name.as_str(), tile_x, tile_y);
    scene.objects.push(RenderObject {
        id: format!(
            "marker:runtime-unit-assembler-progress:{}:{tile_x}:{tile_y}:0x{progress_bits:08x}:{unit_count}:{block_count}:{}:{}:0x{pay_rotation_bits:08x}",
            building.block_name,
            block_sample
                .map(runtime_build_config_content_ref_label)
                .unwrap_or_else(|| "none".to_string()),
            if payload_present { 1 } else { 0 },
        ),
        layer: 16,
        x: tile_x as f32 * TILE_SIZE,
        y: tile_y as f32 * TILE_SIZE,
    });

    let Some((command_x_bits, command_y_bits)) = command_pos else {
        return;
    };
    scene.objects.push(RenderObject {
        id: format!(
            "marker:runtime-unit-assembler-command:{}:{tile_x}:{tile_y}:0x{command_x_bits:08x}:0x{command_y_bits:08x}",
            building.block_name,
        ),
        layer: 16,
        x: f32::from_bits(command_x_bits),
        y: f32::from_bits(command_y_bits),
    });
}

fn append_runtime_unit_assembler_area_objects(
    scene: &mut RenderModel,
    block_name: &str,
    tile_x: i32,
    tile_y: i32,
) {
    const TILE_SIZE: f32 = 8.0;

    let size = runtime_unit_assembler_area_size(block_name);
    let min_tile_x = tile_x - size / 2;
    let min_tile_y = tile_y - size / 2;
    let max_tile_x = tile_x + size / 2 + 1;
    let max_tile_y = tile_y + size / 2 + 1;

    for (edge, start_x, start_y, end_x, end_y) in [
        (
            "top",
            min_tile_x as f32 * TILE_SIZE,
            min_tile_y as f32 * TILE_SIZE,
            max_tile_x as f32 * TILE_SIZE,
            min_tile_y as f32 * TILE_SIZE,
        ),
        (
            "right",
            max_tile_x as f32 * TILE_SIZE,
            min_tile_y as f32 * TILE_SIZE,
            max_tile_x as f32 * TILE_SIZE,
            max_tile_y as f32 * TILE_SIZE,
        ),
        (
            "bottom",
            max_tile_x as f32 * TILE_SIZE,
            max_tile_y as f32 * TILE_SIZE,
            min_tile_x as f32 * TILE_SIZE,
            max_tile_y as f32 * TILE_SIZE,
        ),
        (
            "left",
            min_tile_x as f32 * TILE_SIZE,
            max_tile_y as f32 * TILE_SIZE,
            min_tile_x as f32 * TILE_SIZE,
            min_tile_y as f32 * TILE_SIZE,
        ),
    ] {
        let line_id = format!(
            "marker:line:runtime-unit-assembler-area:{block_name}:{tile_x}:{tile_y}:{edge}"
        );
        scene.objects.push(RenderObject {
            id: line_id.clone(),
            layer: 15,
            x: start_x,
            y: start_y,
        });
        scene.objects.push(RenderObject {
            id: format!("{line_id}:line-end"),
            layer: 15,
            x: end_x,
            y: end_y,
        });
    }
}

fn runtime_unit_assembler_area_size(block_name: &str) -> i32 {
    match block_name {
        "tank-assembler" | "ship-assembler" | "mech-assembler" => 5,
        _ => 5,
    }
}

fn append_runtime_driver_link_objects(
    scene: &mut RenderModel,
    block_name: &str,
    build_pos: i32,
    target_build_pos: i32,
) {
    const TILE_SIZE: f32 = 8.0;

    let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
    let (target_x, target_y) = unpack_runtime_point2(target_build_pos);
    let line_id = format!(
        "marker:line:runtime-driver-link:{block_name}:{tile_x}:{tile_y}:{target_x}:{target_y}"
    );
    scene.objects.push(RenderObject {
        id: line_id.clone(),
        layer: 15,
        x: tile_x as f32 * TILE_SIZE,
        y: tile_y as f32 * TILE_SIZE,
    });
    if build_pos != target_build_pos {
        scene.objects.push(RenderObject {
            id: format!("{line_id}:line-end"),
            layer: 15,
            x: target_x as f32 * TILE_SIZE,
            y: target_y as f32 * TILE_SIZE,
        });
    }
}

fn append_runtime_build_config_content_icons(
    scene: &mut RenderModel,
    family: &str,
    values: &BTreeMap<i32, Option<ConfiguredContentRef>>,
) {
    const TILE_SIZE: f32 = 8.0;

    for (&build_pos, content) in values {
        let Some(content) = content else {
            continue;
        };
        let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
        scene.objects.push(RenderObject {
            id: runtime_build_config_content_object_id(family, build_pos, content),
            layer: 15,
            x: tile_x as f32 * TILE_SIZE,
            y: tile_y as f32 * TILE_SIZE,
        });
    }
}

fn runtime_build_config_content_object_id(
    family: &str,
    build_pos: i32,
    content: &ConfiguredContentRef,
) -> String {
    let (tile_x, tile_y) = unpack_runtime_point2(build_pos);
    format!(
        "marker:runtime-build-config-icon:{family}:{tile_x}:{tile_y}:{}:{}",
        content.content_type, content.content_id
    )
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
    use mdt_render_ui::render_model::RenderPrimitive;
    use mdt_typeio::{write_object as write_typeio_object, TypeIoObject};
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

    fn encode_effect_payload(
        effect_id: i16,
        x: f32,
        y: f32,
        rotation: f32,
        color_rgba: u32,
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(18);
        payload.extend_from_slice(&effect_id.to_be_bytes());
        payload.extend_from_slice(&x.to_be_bytes());
        payload.extend_from_slice(&y.to_be_bytes());
        payload.extend_from_slice(&rotation.to_be_bytes());
        payload.extend_from_slice(&color_rgba.to_be_bytes());
        payload
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

    fn first_runtime_effect_icon(scene: &RenderModel) -> &RenderObject {
        scene
            .objects
            .iter()
            .find(|object| object.id.starts_with("marker:runtime-effect-icon:"))
            .expect("expected runtime effect icon")
    }

    fn first_runtime_effect_line(scene: &RenderModel) -> &RenderObject {
        scene
            .objects
            .iter()
            .find(|object| {
                object
                    .id
                    .starts_with("marker:line:runtime-effect-point-beam:")
                    && !object.id.ends_with(":line-end")
            })
            .expect("expected runtime effect line")
    }

    fn first_runtime_effect_line_end(scene: &RenderModel) -> &RenderObject {
        scene
            .objects
            .iter()
            .find(|object| {
                object
                    .id
                    .starts_with("marker:line:runtime-effect-point-beam:")
                    && object.id.ends_with(":line-end")
            })
            .expect("expected runtime effect line end")
    }

    fn runtime_effect_lines_with_prefix<'a>(
        scene: &'a RenderModel,
        prefix: &str,
    ) -> Vec<&'a RenderObject> {
        scene
            .objects
            .iter()
            .filter(|object| object.id.starts_with(prefix))
            .collect()
    }

    fn scene_object_by_id<'a>(scene: &'a RenderModel, id: &str) -> Option<&'a RenderObject> {
        scene.objects.iter().find(|object| object.id == id)
    }

    fn first_runtime_world_label_event_object(scene: &RenderModel) -> &RenderObject {
        scene
            .objects
            .iter()
            .find(|object| object.id.starts_with("world-label:event:"))
            .expect("expected runtime world-label event object")
    }

    #[test]
    fn runtime_rules_label_includes_high_signal_rule_fields() {
        let mut state = SessionState::default();
        state.rules_projection.waves = Some(true);
        state.rules_projection.pvp = Some(false);
        state.rules_projection.unit_cap = Some(180);
        state.rules_projection.default_team_id = Some(1);
        state.rules_projection.wave_team_id = Some(2);
        state.rules_projection.initial_wave_spacing = Some(90.0);

        assert_eq!(
            runtime_rules_label(&state),
            "sr0:srf0:so0:sof0:rule0:rf0:clr0:cmp0:wv1:pvp0:uc180:dt1:wt2:iws90:obj0:q0:par0:fg0:oor0:last-1"
        );
    }

    #[test]
    fn render_runtime_adapter_appends_visible_runtime_live_unit_object() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(crate::session_state::TypedRuntimeEntityModel::Unit(
                crate::session_state::TypedRuntimeUnitEntity {
                    base: crate::session_state::TypedRuntimeEntityBase {
                        entity_id: 202,
                        class_id: 4,
                        hidden: false,
                        is_local_player: false,
                        unit_kind: 2,
                        unit_value: 202,
                        x_bits: 30.0f32.to_bits(),
                        y_bits: 40.0f32.to_bits(),
                        last_seen_entity_snapshot_count: 3,
                    },
                    semantic: crate::session_state::EntityUnitSemanticProjection {
                        team_id: 2,
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
                    },
                    carried_item_stack: None,
                },
            ));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let unit = scene_object_by_id(&scene, "unit:202").expect("missing runtime unit object");
        assert_eq!(unit.layer, 40);
        assert_eq!(unit.x, 30.0);
        assert_eq!(unit.y, 40.0);
    }

    #[test]
    fn render_runtime_adapter_skips_hidden_runtime_live_unit_object() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(crate::session_state::TypedRuntimeEntityModel::Unit(
                crate::session_state::TypedRuntimeUnitEntity {
                    base: crate::session_state::TypedRuntimeEntityBase {
                        entity_id: 202,
                        class_id: 4,
                        hidden: true,
                        is_local_player: false,
                        unit_kind: 2,
                        unit_value: 202,
                        x_bits: 30.0f32.to_bits(),
                        y_bits: 40.0f32.to_bits(),
                        last_seen_entity_snapshot_count: 3,
                    },
                    semantic: crate::session_state::EntityUnitSemanticProjection {
                        team_id: 2,
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
                    },
                    carried_item_stack: None,
                },
            ));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene_object_by_id(&scene, "unit:202").is_none());
    }

    #[test]
    fn render_runtime_adapter_appends_visible_runtime_world_label_object() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(crate::session_state::TypedRuntimeEntityModel::WorldLabel(
                crate::session_state::TypedRuntimeWorldLabelEntity {
                    base: crate::session_state::TypedRuntimeEntityBase {
                        entity_id: 404,
                        class_id: 35,
                        hidden: false,
                        is_local_player: false,
                        unit_kind: 0,
                        unit_value: 0,
                        x_bits: 56.0f32.to_bits(),
                        y_bits: 72.0f32.to_bits(),
                        last_seen_entity_snapshot_count: 3,
                    },
                    semantic: crate::session_state::EntityWorldLabelSemanticProjection {
                        flags: 1,
                        font_size_bits: 12.0f32.to_bits(),
                        text: Some("runtime".to_string()),
                        z_bits: 0.5f32.to_bits(),
                    },
                },
            ));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let world_label = scene
            .objects
            .iter()
            .find(|object| object.id.starts_with("world-label:404"))
            .expect("missing runtime world-label object");
        assert_eq!(world_label.id, "world-label:404:text:72756e74696d65");
        assert_eq!(world_label.layer, 39);
        assert_eq!(world_label.x, 56.0);
        assert_eq!(world_label.y, 72.0);
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                RenderPrimitive::Text { id, text, .. }
                    if id == "world-label:404:text:72756e74696d65" && text == "runtime"
            )
        }));
    }

    #[test]
    fn render_runtime_adapter_renders_world_label_event_overlay_object() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::WorldLabel {
            reliable: false,
            label_id: Some(99),
            message: Some("runtime-event".to_string()),
            duration: 1.0,
            world_x: 48.0,
            world_y: 64.0,
        }]);
        assert_eq!(adapter.world_overlay().world_label_overlays.len(), 1);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let world_label = first_runtime_world_label_event_object(&scene);
        assert_eq!(
            world_label.id,
            "world-label:event:0:text:72756e74696d652d6576656e74"
        );
        assert_eq!(world_label.layer, 39);
        assert_eq!(world_label.x, 48.0);
        assert_eq!(world_label.y, 64.0);
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                RenderPrimitive::Text { id, text, .. }
                    if id == "world-label:event:0:text:72756e74696d652d6576656e74"
                        && text == "runtime-event"
            )
        }));
    }

    #[test]
    fn render_runtime_adapter_removes_world_label_event_overlay_on_remove_event() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::WorldLabel {
            reliable: true,
            label_id: Some(99),
            message: Some("runtime-event".to_string()),
            duration: 1.0,
            world_x: 32.0,
            world_y: 40.0,
        }]);
        assert_eq!(adapter.world_overlay().world_label_overlays.len(), 1);

        adapter.observe_events(&[ClientSessionEvent::RemoveWorldLabel { label_id: 99 }]);
        assert!(adapter.world_overlay().world_label_overlays.is_empty());

        adapter.apply(&mut scene, &mut hud, &input, &state);
        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("world-label:event:")));
    }

    #[test]
    fn render_runtime_adapter_expires_anonymous_world_label_event_overlay() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::WorldLabel {
            reliable: false,
            label_id: None,
            message: Some("ephemeral".to_string()),
            duration: 0.0,
            world_x: 20.0,
            world_y: 24.0,
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);
        first_runtime_world_label_event_object(&scene);

        adapter.observe_events(&[]);
        assert!(adapter.world_overlay().world_label_overlays.is_empty());

        let mut expired_scene = RenderModel::default();
        let mut expired_hud = HudModel::default();
        adapter.apply(&mut expired_scene, &mut expired_hud, &input, &state);
        assert!(!expired_scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("world-label:event:")));
    }

    #[test]
    fn render_runtime_adapter_skips_world_label_event_overlay_with_non_finite_duration() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::WorldLabel {
            reliable: false,
            label_id: Some(99),
            message: Some("runtime-event".to_string()),
            duration: f32::NAN,
            world_x: 48.0,
            world_y: 64.0,
        }]);
        assert!(adapter.world_overlay().world_label_overlays.is_empty());

        adapter.apply(&mut scene, &mut hud, &input, &state);
        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("world-label:event:")));
    }

    #[test]
    fn render_runtime_adapter_renders_create_bullet_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::CreateBullet {
            projection: crate::session_state::CreateBulletProjection {
                bullet_type_id: Some(17),
                team_id: 4,
                x_bits: 32.5f32.to_bits(),
                y_bits: 48.0f32.to_bits(),
                angle_bits: 90.0f32.to_bits(),
                damage_bits: 11.5f32.to_bits(),
                velocity_scl_bits: 1.25f32.to_bits(),
                lifetime_scl_bits: 0.75f32.to_bits(),
            },
        }]);
        assert_eq!(adapter.world_overlay().world_event_markers.len(), 1);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = scene_object_by_id(&scene, "marker:runtime-bullet:0:17:4")
            .expect("missing runtime bullet marker");
        assert_eq!(marker.x, 32.5);
        assert_eq!(marker.y, 48.0);
        assert_eq!(marker.layer, 28);
    }

    #[test]
    fn render_runtime_adapter_renders_logic_explosion_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::LogicExplosionObserved {
            team_id: 2,
            x: 16.0,
            y: 24.0,
            radius: 64.0,
            damage: 96.0,
            air: true,
            ground: false,
            pierce: true,
            effect: true,
        }]);
        assert_eq!(adapter.world_overlay().world_event_markers.len(), 1);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = scene_object_by_id(
            &scene,
            "marker:runtime-logic-explosion:0:2:0x42800000:1:1:0:1",
        )
        .expect("missing runtime logic explosion marker");
        assert_eq!(marker.x, 16.0);
        assert_eq!(marker.y, 24.0);
        assert_eq!(marker.layer, 28);
    }

    #[test]
    fn render_runtime_adapter_renders_sound_at_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::SoundAtRequested {
            sound_id: Some(11),
            x: 64.0,
            y: 96.0,
            volume: 0.8,
            pitch: 1.1,
        }]);
        assert_eq!(adapter.world_overlay().world_event_markers.len(), 1);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = scene_object_by_id(&scene, "marker:runtime-sound-at:0:11")
            .expect("missing runtime sound-at marker");
        assert_eq!(marker.x, 64.0);
        assert_eq!(marker.y, 96.0);
        assert_eq!(marker.layer, 28);
    }

    #[test]
    fn render_runtime_adapter_renders_tile_world_action_markers() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[
            ClientSessionEvent::UnitBlockSpawn {
                tile_pos: Some(pack_runtime_point2(1, 2)),
            },
            ClientSessionEvent::UnitTetherBlockSpawned {
                tile_pos: Some(pack_runtime_point2(2, 3)),
                unit_id: 44,
            },
            ClientSessionEvent::AutoDoorToggle {
                tile_pos: Some(pack_runtime_point2(3, 4)),
                open: true,
            },
            ClientSessionEvent::LandingPadLanded {
                tile_pos: Some(pack_runtime_point2(4, 5)),
            },
            ClientSessionEvent::AssemblerDroneSpawned {
                tile_pos: Some(pack_runtime_point2(5, 6)),
                unit_id: 77,
            },
            ClientSessionEvent::AssemblerUnitSpawned {
                tile_pos: Some(pack_runtime_point2(6, 7)),
            },
        ]);
        assert_eq!(adapter.world_overlay().world_event_markers.len(), 6);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        for (id, x, y) in [
            ("marker:runtime-unit-block-spawn:0:1:2", 8.0, 16.0),
            (
                "marker:runtime-unit-tether-block-spawned:1:2:3:44",
                16.0,
                24.0,
            ),
            ("marker:runtime-auto-door-toggle:2:3:4:1", 24.0, 32.0),
            ("marker:runtime-landing-pad-landed:3:4:5", 32.0, 40.0),
            (
                "marker:runtime-assembler-drone-spawned:4:5:6:77",
                40.0,
                48.0,
            ),
            ("marker:runtime-assembler-unit-spawned:5:6:7", 48.0, 56.0),
        ] {
            let marker = scene_object_by_id(&scene, id).expect("missing tile world-action marker");
            assert_eq!(marker.x, x);
            assert_eq!(marker.y, y);
            assert_eq!(marker.layer, 28);
        }
    }

    #[test]
    fn render_runtime_adapter_appends_local_runtime_player_when_scene_has_no_player_focus() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(crate::session_state::TypedRuntimeEntityModel::Player(
                crate::session_state::TypedRuntimePlayerEntity {
                    base: crate::session_state::TypedRuntimeEntityBase {
                        entity_id: 101,
                        class_id: 12,
                        hidden: false,
                        is_local_player: true,
                        unit_kind: 2,
                        unit_value: 101,
                        x_bits: 24.0f32.to_bits(),
                        y_bits: 32.0f32.to_bits(),
                        last_seen_entity_snapshot_count: 5,
                    },
                    semantic: crate::session_state::EntityPlayerSemanticProjection::default(),
                },
            ));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let player =
            scene_object_by_id(&scene, "player:101").expect("missing runtime local player");
        assert_eq!(player.layer, 41);
        assert_eq!(player.x, 24.0);
        assert_eq!(player.y, 32.0);
        assert_eq!(scene.player_focus_tile(8.0), Some((3, 4)));
    }

    #[test]
    fn render_runtime_adapter_preserves_existing_player_focus_when_runtime_local_player_exists() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        scene.objects.push(RenderObject {
            id: "player:7".to_string(),
            layer: 40,
            x: 24.0,
            y: 32.0,
        });
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(crate::session_state::TypedRuntimeEntityModel::Player(
                crate::session_state::TypedRuntimePlayerEntity {
                    base: crate::session_state::TypedRuntimeEntityBase {
                        entity_id: 101,
                        class_id: 12,
                        hidden: false,
                        is_local_player: true,
                        unit_kind: 2,
                        unit_value: 101,
                        x_bits: 80.0f32.to_bits(),
                        y_bits: 96.0f32.to_bits(),
                        last_seen_entity_snapshot_count: 5,
                    },
                    semantic: crate::session_state::EntityPlayerSemanticProjection::default(),
                },
            ));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene_object_by_id(&scene, "player:101").is_none());
        assert_eq!(scene.player_focus_tile(8.0), Some((3, 4)));
    }

    #[test]
    fn render_runtime_adapter_does_not_treat_unit_object_as_existing_player_focus() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        scene.objects.push(RenderObject {
            id: "unit:7".to_string(),
            layer: 40,
            x: 24.0,
            y: 32.0,
        });
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(crate::session_state::TypedRuntimeEntityModel::Player(
                crate::session_state::TypedRuntimePlayerEntity {
                    base: crate::session_state::TypedRuntimeEntityBase {
                        entity_id: 101,
                        class_id: 12,
                        hidden: false,
                        is_local_player: true,
                        unit_kind: 2,
                        unit_value: 101,
                        x_bits: 80.0f32.to_bits(),
                        y_bits: 96.0f32.to_bits(),
                        last_seen_entity_snapshot_count: 5,
                    },
                    semantic: crate::session_state::EntityPlayerSemanticProjection::default(),
                },
            ));

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let player =
            scene_object_by_id(&scene, "player:101").expect("missing runtime local player");
        assert_eq!(player.x, 80.0);
        assert_eq!(player.y, 96.0);
    }

    #[test]
    fn render_runtime_adapter_appends_runtime_ping_location_text_object() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.ping_location_projection.received_count = 1;
        state.ping_location_projection.last_player_id = Some(42);
        state.ping_location_projection.last_x_bits = Some(48.0f32.to_bits());
        state.ping_location_projection.last_y_bits = Some(56.0f32.to_bits());
        state.ping_location_projection.last_text = Some("watch here".to_string());

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = scene
            .objects
            .iter()
            .find(|object| object.id.starts_with("marker:text:runtime-ping:42:text:"))
            .expect("missing runtime ping marker");
        assert_eq!(marker.layer, 31);
        assert_eq!(marker.x, 48.0);
        assert_eq!(marker.y, 56.0);
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                RenderPrimitive::Text { id, text, .. }
                    if id == &marker.id && text == "watch here"
            )
        }));
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
                    config: ClientBuildPlanConfig::Point2Array(vec![(8, 9), (9, 10)]),
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

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(
            &mut scene,
            &mut hud,
            session.snapshot_input(),
            session.state(),
        );

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("plan:runtime-place:")));
        assert!(scene.objects.iter().any(|object| {
            object
                .id
                .starts_with("marker:line:runtime-plan-config-link:0:0:")
        }));
        assert!(scene.objects.iter().any(|object| {
            object
                .id
                .starts_with("marker:line:runtime-plan-config-link:0:1:")
        }));
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-break:")));
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:line:runtime-break-rect:")));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    ..
                } if *x0 == 44.0 && *y0 == 36.0 && *x1 == 68.0 && *y1 == 76.0
            )
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    ..
                } if *x0 == 44.0 && *y0 == 36.0 && *x1 == 76.0 && *y1 == 84.0
            )
        }));
        assert!(hud.status_text.contains("runtime_selected=0x0101"));
        assert!(hud.status_text.contains("runtime_plans=2"));
        assert!(hud.status_text.contains("runtime_cfg_int=0"));
        assert!(hud.status_text.contains("runtime_cfg_bool=0"));
        assert!(hud.status_text.contains("runtime_cfg_point2=0"));
        assert!(hud.status_text.contains("runtime_cfg_point2_array=1"));
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
    fn render_runtime_adapter_surfaces_build_plan_content_config_icons() {
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState {
            plans: Some(vec![ClientBuildPlan {
                tile: (6, 7),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 0,
                config: ClientBuildPlanConfig::Content {
                    content_type: 1,
                    content_id: 7,
                },
            }]),
            ..Default::default()
        };
        let state = SessionState::default();

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:runtime-build-config-icon:plan-content:6:7:1:7"
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Icon {
                    family,
                    variant,
                    layer,
                    x,
                    y,
                    ..
                } if *family
                    == mdt_render_ui::render_model::RenderIconPrimitiveFamily::RuntimeBuildConfig
                    && variant == "plan-content"
                    && *layer == 23
                    && *x == 48.0
                    && *y == 56.0
            )
        }));
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

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(
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
        let session = &hud
            .runtime_ui
            .as_ref()
            .expect("runtime_ui should be present")
            .session;
        assert_eq!(session.kick.reason_text.as_deref(), Some("server restart"));
        assert_eq!(session.kick.reason_ordinal, Some(15));
        assert_eq!(
            session.kick.hint_category.as_deref(),
            Some("ServerRestarting")
        );
        assert_eq!(
            session.kick.hint_text.as_deref(),
            Some("server is restarting; retry connection shortly.")
        );
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

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(
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
                block_name: Some("message".to_string()),
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
                turret_reload_counter_bits: None,
                turret_rotation_bits: None,
                item_turret_ammo_count: None,
                continuous_turret_last_length_bits: None,
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
        state.building_table_projection.last_block_name = Some("message".to_string());
        state.building_table_projection.last_rotation = Some(1);
        state.building_table_projection.last_team_id = Some(2);
        state.building_table_projection.last_config = Some(mdt_typeio::TypeIoObject::Int(7));
        state.building_table_projection.last_health_bits = Some(0x3f800000);
        state.building_table_projection.last_enabled = Some(true);
        state.building_table_projection.last_efficiency = Some(0x80);
        state.building_table_projection.last_optional_efficiency = Some(0x40);
        state.building_table_projection.last_update =
            Some(crate::session_state::BuildingProjectionUpdateKind::ConstructFinish);

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "block:runtime-building:12:6:258"));
    }

    #[test]
    fn render_runtime_adapter_surfaces_snapshot_input_command_mode_projection() {
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState {
            command_mode: mdt_input::CommandModeProjection {
                active: true,
                selected_units: vec![11, 22],
                command_buildings: vec![pack_runtime_point2(18, 40)],
                command_rect: Some(mdt_input::CommandModeRectProjection {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
                control_groups: vec![mdt_input::CommandModeControlGroupProjection {
                    index: 4,
                    unit_ids: vec![99],
                }],
                last_control_group_operation: None,
                last_target: Some(mdt_input::CommandModeTargetProjection {
                    unit_target: Some(mdt_input::CommandUnitRef {
                        kind: 2,
                        value: 808,
                    }),
                    ..Default::default()
                }),
                last_command_selection: Some(mdt_input::CommandModeCommandSelection {
                    command_id: Some(5),
                }),
                last_stance_selection: Some(mdt_input::CommandModeStanceSelection {
                    stance_id: Some(7),
                    enabled: true,
                }),
            },
            ..Default::default()
        };
        let mut state = SessionState::default();
        state.entity_table_projection.upsert_entity(
            11,
            1,
            false,
            0,
            0,
            8.0f32.to_bits(),
            16.0f32.to_bits(),
            false,
            1,
        );
        state.entity_table_projection.upsert_entity(
            22,
            1,
            false,
            0,
            0,
            24.0f32.to_bits(),
            32.0f32.to_bits(),
            false,
            1,
        );
        state.entity_table_projection.upsert_entity(
            808,
            1,
            false,
            0,
            0,
            40.0f32.to_bits(),
            48.0f32.to_bits(),
            false,
            1,
        );

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let runtime_ui = hud
            .runtime_ui
            .as_ref()
            .expect("runtime_ui observability should be present");
        assert!(runtime_ui.command_mode.active);
        assert_eq!(runtime_ui.command_mode.selected_units, vec![11, 22]);
        assert_eq!(
            runtime_ui.command_mode.command_buildings,
            vec![pack_runtime_point2(18, 40)]
        );
        assert_eq!(
            runtime_ui.command_mode.command_rect,
            Some(mdt_render_ui::RuntimeCommandRectObservability {
                x0: 1,
                y0: 2,
                x1: 3,
                y1: 4,
            })
        );
        assert_eq!(
            runtime_ui.command_mode.control_groups,
            vec![mdt_render_ui::RuntimeCommandControlGroupObservability {
                index: 4,
                unit_ids: vec![99],
            }]
        );
        assert_eq!(
            runtime_ui.command_mode.last_target,
            Some(mdt_render_ui::RuntimeCommandTargetObservability {
                build_target: None,
                unit_target: Some(mdt_render_ui::RuntimeCommandUnitRefObservability {
                    kind: 2,
                    value: 808,
                }),
                position_target: None,
                rect_target: None,
            })
        );
        assert_eq!(
            runtime_ui.command_mode.last_command_selection,
            Some(mdt_render_ui::RuntimeCommandSelectionObservability {
                command_id: Some(5),
            })
        );
        assert_eq!(
            runtime_ui.command_mode.last_stance_selection,
            Some(mdt_render_ui::RuntimeCommandStanceObservability {
                stance_id: Some(7),
                enabled: true,
            })
        );
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "marker:runtime-command-selected-unit:11"));
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "marker:runtime-command-selected-unit:22"));
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id == "marker:runtime-command-building:18:40"));
        assert!(scene.objects.iter().any(|object| {
            object.id
                == format!(
                    "marker:line:runtime-command-rect:top:{}:{}:{}:{}",
                    8.0f32.to_bits(),
                    16.0f32.to_bits(),
                    24.0f32.to_bits(),
                    16.0f32.to_bits()
                )
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id == format!("marker:runtime-command-unit-target:{}:{}", 2, 808)
        }));
        assert!(scene.objects.iter().any(|object| {
            object
                .id
                .starts_with("marker:line:runtime-command-target-link:0:")
        }));
        assert!(scene.objects.iter().any(|object| {
            object
                .id
                .starts_with("marker:line:runtime-command-building-target-link:0:")
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Rect {
                    family,
                    layer,
                    left,
                    top,
                    right,
                    bottom,
                    ..
                } if family == "runtime-command-rect"
                    && *layer == 29
                    && *left == 8.0
                    && *top == 16.0
                    && *right == 24.0
                    && *bottom == 32.0
            )
        }));
        assert!(!scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Line {
                    id,
                    ..
                } if id
                    == &format!(
                        "marker:line:runtime-command-rect:top:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        16.0f32.to_bits()
                    )
            )
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Icon {
                    family,
                    variant,
                    x,
                    y,
                    ..
                } if *family
                    == mdt_render_ui::render_model::RenderIconPrimitiveFamily::RuntimeCommand
                    && variant == "selected-unit"
                    && *x == 8.0
                    && *y == 16.0
            )
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    ..
                } if *x0 == 8.0 && *y0 == 16.0 && *x1 == 40.0 && *y1 == 48.0
            )
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    ..
                } if *x0 == 148.0 && *y0 == 324.0 && *x1 == 40.0 && *y1 == 48.0
            )
        }));
    }

    #[test]
    fn render_runtime_adapter_renders_command_mode_target_markers() {
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState {
            command_mode: mdt_input::CommandModeProjection {
                active: true,
                last_target: Some(mdt_input::CommandModeTargetProjection {
                    build_target: Some(pack_runtime_point2(7, 8)),
                    position_target: Some(mdt_input::CommandModePositionTarget {
                        x_bits: 96.0f32.to_bits(),
                        y_bits: 120.0f32.to_bits(),
                    }),
                    rect_target: Some(mdt_input::CommandModeRectProjection {
                        x0: 4,
                        y0: 5,
                        x1: 6,
                        y1: 7,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let state = SessionState::default();

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let build_target = scene_object_by_id(&scene, "marker:runtime-command-build-target:7:8")
            .expect("missing command build target marker");
        assert_eq!(build_target.x, 56.0);
        assert_eq!(build_target.y, 64.0);

        let position_target = scene_object_by_id(
            &scene,
            &format!(
                "marker:runtime-command-position-target:0x{:08x}:0x{:08x}",
                96.0f32.to_bits(),
                120.0f32.to_bits()
            ),
        )
        .expect("missing command position target marker");
        assert_eq!(position_target.x, 96.0);
        assert_eq!(position_target.y, 120.0);

        assert!(scene.objects.iter().any(|object| {
            object.id
                == format!(
                    "marker:line:runtime-command-target-rect:top:{}:{}:{}:{}",
                    32.0f32.to_bits(),
                    40.0f32.to_bits(),
                    48.0f32.to_bits(),
                    40.0f32.to_bits()
                )
        }));
    }

    #[test]
    fn render_runtime_adapter_skips_command_mode_overlays_when_projection_is_inactive() {
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState {
            command_mode: mdt_input::CommandModeProjection {
                active: false,
                selected_units: vec![11],
                command_buildings: vec![pack_runtime_point2(18, 40)],
                command_rect: Some(mdt_input::CommandModeRectProjection {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
                last_target: Some(mdt_input::CommandModeTargetProjection {
                    build_target: Some(pack_runtime_point2(7, 8)),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut state = SessionState::default();
        state.entity_table_projection.upsert_entity(
            11,
            1,
            false,
            0,
            0,
            8.0f32.to_bits(),
            16.0f32.to_bits(),
            false,
            1,
        );

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-command-")));
        assert!(!scene
            .objects
            .iter()
            .any(|object| { object.id.starts_with("marker:line:runtime-command-") }));
        assert!(!scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Icon { family, .. }
                    if *family
                        == mdt_render_ui::render_model::RenderIconPrimitiveFamily::RuntimeCommand
            )
        }));
        assert!(!scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Rect { family, .. }
                    if family == "runtime-command-rect" || family == "runtime-command-target-rect"
            )
        }));
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
        let mut adapter = RenderRuntimeAdapter::default();
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
        state.tile_config_projection.rollback_count = 1;
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
        let rollback_strip = &hud
            .build_ui
            .as_ref()
            .expect("build_ui observability should be present")
            .rollback_strip;
        assert_eq!(rollback_strip.applied_authoritative_count, 3);
        assert_eq!(rollback_strip.rollback_count, 1);
        assert_eq!(rollback_strip.last_build_tile, Some((9, 4)));
        assert!(rollback_strip.last_business_applied);
        assert!(rollback_strip.last_cleared_pending_local);
        assert!(rollback_strip.last_was_rollback);
        assert_eq!(rollback_strip.last_pending_local_match, Some(false));
        assert_eq!(
            rollback_strip.last_source,
            Some(mdt_render_ui::BuildConfigAuthoritySourceObservability::ConstructFinish)
        );
        assert_eq!(
            rollback_strip.last_configured_outcome,
            Some(mdt_render_ui::BuildConfigOutcomeObservability::Applied)
        );
        assert_eq!(
            rollback_strip.last_configured_block_name.as_deref(),
            Some("item-source")
        );
    }

    #[test]
    fn render_runtime_adapter_reports_configured_block_projection_in_hud() {
        let mut adapter = RenderRuntimeAdapter::default();
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
            .constructor_runtime_by_build_pos
            .insert(
                pack_runtime_point2(19, 41),
                crate::session_state::ConstructorRuntimeProjection {
                    progress_bits: 0x3f20_0000,
                    payload_present: true,
                    pay_rotation_bits: 0x4020_0000,
                    payload_build_block_id: Some(11),
                    payload_unit_class_id: None,
                },
            );
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
            .payload_source_runtime_by_build_pos
            .insert(
                pack_runtime_point2(21, 43),
                crate::session_state::PayloadSourceRuntimeProjection {
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    pay_vector_x_bits: 0x4120_0000,
                    pay_vector_y_bits: 0x41a0_0000,
                    pay_rotation_bits: 0x4000_0000,
                    payload_present: true,
                    payload_type: Some(1),
                    payload_build_block_id: Some(12),
                    payload_build_revision: Some(1),
                    payload_unit_class_id: None,
                    payload_unit_payload_len: None,
                    payload_unit_payload_sha256: None,
                },
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
            .payload_router_runtime_by_build_pos
            .insert(
                pack_runtime_point2(22, 44),
                crate::session_state::PayloadRouterRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    item_rotation_bits: 0x4040_0000,
                    payload_present: true,
                    payload_type: Some(0),
                    payload_kind: Some(crate::session_state::PayloadRouterPayloadKind::Unit),
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: Some(11),
                    payload_unit_revision: Some(2),
                    payload_serialized_len: 5,
                    payload_serialized_sha256: "0123456789abcdef".to_string(),
                    rec_dir: 3,
                },
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
            .payload_loader_runtime_by_build_pos
            .insert(
                pack_runtime_point2(25, 47),
                crate::session_state::PayloadLoaderRuntimeProjection {
                    exporting: false,
                    payload_present: true,
                    payload_type: Some(1),
                    pay_rotation_bits: 0x4000_0000,
                    payload_build_block_id: Some(12),
                    payload_build_revision: Some(3),
                    payload_unit_class_id: None,
                    payload_unit_payload_len: None,
                    payload_unit_payload_sha256: None,
                },
            );
        state
            .configured_block_projection
            .unit_factory_current_plan_by_build_pos
            .insert(pack_runtime_point2(24, 46), 7);
        state
            .configured_block_projection
            .unit_factory_runtime_by_build_pos
            .insert(
                pack_runtime_point2(24, 46),
                crate::session_state::UnitFactoryRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    command_id: Some(9),
                    payload_present: true,
                    pay_rotation_bits: 0x4000_0000,
                },
            );
        state
            .configured_block_projection
            .reconstructor_command_by_build_pos
            .insert(pack_runtime_point2(26, 48), Some(12));
        state
            .configured_block_projection
            .reconstructor_runtime_by_build_pos
            .insert(
                pack_runtime_point2(26, 48),
                crate::session_state::ReconstructorRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    payload_present: true,
                    pay_rotation_bits: 0x4000_0000,
                },
            );
        state
            .configured_block_projection
            .memory_values_bits_by_build_pos
            .insert(
                pack_runtime_point2(27, 49),
                vec![1.0f64.to_bits(), (-3.5f64).to_bits()],
            );
        state
            .configured_block_projection
            .unit_assembler_by_build_pos
            .insert(
                pack_runtime_point2(29, 51),
                crate::session_state::UnitAssemblerRuntimeProjection {
                    progress_bits: 0x3f00_0000,
                    unit_ids: vec![111, 222],
                    block_entry_count: 3,
                    block_sample: Some(ConfiguredContentRef {
                        content_type: 1,
                        content_id: 8,
                    }),
                    command_pos: Some((12.5f32.to_bits(), 20.0f32.to_bits())),
                    payload_present: true,
                    pay_rotation_bits: 0x4040_0000,
                },
            );
        state
            .configured_block_projection
            .mass_driver_link_by_build_pos
            .insert(
                pack_runtime_point2(30, 52),
                Some(pack_runtime_point2(20, 22)),
            );
        state
            .configured_block_projection
            .mass_driver_runtime_by_build_pos
            .insert(
                pack_runtime_point2(30, 52),
                crate::session_state::MassDriverRuntimeProjection {
                    rotation_bits: 0x4120_0000,
                    state_ordinal: 2,
                },
            );
        state
            .configured_block_projection
            .payload_mass_driver_link_by_build_pos
            .insert(
                pack_runtime_point2(31, 53),
                Some(pack_runtime_point2(24, 26)),
            );
        state
            .configured_block_projection
            .payload_mass_driver_runtime_by_build_pos
            .insert(
                pack_runtime_point2(31, 53),
                crate::session_state::PayloadMassDriverRuntimeProjection {
                    turret_rotation_bits: 0x4140_0000,
                    state_ordinal: 3,
                    reload_counter_bits: 0x3f20_0000,
                    charge_bits: 0x3f40_0000,
                    loaded: true,
                    charging: false,
                    payload_present: true,
                },
            );
        state
            .configured_block_projection
            .sorter_item_by_build_pos
            .insert(pack_runtime_point2(32, 54), Some(7));
        state
            .configured_block_projection
            .sorter_runtime_by_build_pos
            .insert(
                pack_runtime_point2(32, 54),
                crate::session_state::SorterRuntimeProjection {
                    legacy: true,
                    non_empty_side_mask: 0x05,
                    buffered_item_count: 3,
                },
            );
        for (build_pos, block_name) in [
            (pack_runtime_point2(18, 40), "message"),
            (pack_runtime_point2(19, 41), "constructor"),
            (pack_runtime_point2(21, 43), "payload-source"),
            (pack_runtime_point2(22, 44), "payload-router"),
            (pack_runtime_point2(23, 45), "power-node"),
            (pack_runtime_point2(24, 46), "ground-factory"),
            (pack_runtime_point2(25, 47), "payload-unloader"),
            (pack_runtime_point2(26, 48), "additive-reconstructor"),
            (pack_runtime_point2(27, 49), "memory-cell"),
            (pack_runtime_point2(28, 50), "build-tower"),
            (pack_runtime_point2(29, 51), "tank-assembler"),
            (pack_runtime_point2(30, 52), "mass-driver"),
            (pack_runtime_point2(31, 53), "payload-mass-driver"),
            (pack_runtime_point2(32, 54), "sorter"),
        ] {
            state.building_table_projection.by_build_pos.insert(
                build_pos,
                crate::session_state::BuildingProjection {
                    block_id: Some(1),
                    block_name: Some(block_name.to_string()),
                    rotation: None,
                    team_id: None,
                    io_version: None,
                    module_bitmask: None,
                    time_scale_bits: None,
                    time_scale_duration_bits: None,
                    last_disabler_pos: None,
                    legacy_consume_connected: None,
                    config: None,
                    health_bits: None,
                    enabled: None,
                    efficiency: None,
                    optional_efficiency: None,
                    visible_flags: None,
                    turret_reload_counter_bits: None,
                    turret_rotation_bits: None,
                    item_turret_ammo_count: None,
                    continuous_turret_last_length_bits: None,
                    build_turret_rotation_bits: (block_name == "build-tower")
                        .then_some(0x4210_0000),
                    build_turret_plans_present: (block_name == "build-tower").then_some(true),
                    build_turret_plan_count: (block_name == "build-tower").then_some(5),
                    last_update: crate::session_state::BuildingProjectionUpdateKind::TileConfig,
                },
            );
        }

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud
            .status_text
            .contains("runtime_configured=uc1@14:36=clear:is1@12:34=0:ls1@13:35=0"));
        assert!(hud.status_text.contains(":mg1@18:40=len5:"));
        assert!(hud.status_text.contains(":ct1@19:41=5:"));
        assert!(hud.status_text.contains(":il1@20:42=11223344:"));
        assert!(hud
            .status_text
            .contains(":pl1@25:47=imp:y1:t1:r40000000:b:12@r3:"));
        assert!(hud.status_text.contains(":ps1@21:43=b:7:"));
        assert!(hud.status_text.contains(":pr1@22:44=u:9:"));
        assert!(hud
            .status_text
            .contains(":uf1@24:46=cp7:p3f400000:c0x41480000:0x41900000:cmd9:y1:r40000000:"));
        assert!(hud
            .status_text
            .contains(":ua1@29:51=p3f000000:u2:b3:sb:8:c0x41480000:0x41a00000:y1:r40400000:"));
        assert!(hud.status_text.contains(":pn1@23:45=n2:24:46|25:47:"));
        assert!(hud.status_text.contains(":rc1@26:48=12"));
        let build_ui = hud
            .build_ui
            .as_ref()
            .expect("build_ui observability should be present");
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "message" && entry.sample == "18:40:len=5:text=hello"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "constructor"
                && entry.sample == "19:41:recipe=5:p3f200000:y1:r40200000:payload=b:11"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "payload-source"
                && entry.sample
                    == "21:43:content=b:7:command=0x41480000:0x41900000:payload=1:payload-type=1:vec=0x41200000:0x41a00000:rot=0x40000000:payload-ref=b:12@r1"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "payload-router"
                && entry.sample
                    == "22:44:content=u:9:progress=0x3f400000:item-rot=0x40400000:payload=1:payload-type=0:payload-kind=unit:payload-ref=uc:11@r2:payload-len=5:payload-sha=0123456789ab:rec-dir=3"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "power-node" && entry.sample == "23:45:links=24:46|25:47"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "payload-loader"
                && entry.sample
                    == "25:47:payload-unloader:mode=import:y1:payload-type=1:r40000000:payload=b:12@r3"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "unit-factory"
                && entry.sample
                    == "24:46:ground-factory:plan=7:progress=0x3f400000:command=0x41480000:0x41900000:command-id=9:payload=1:pay-rot=0x40000000"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "reconstructor"
                && entry.sample
                    == "26:48:additive-reconstructor:command=12:p3f400000:c0x41480000:0x41900000:y1:r40000000"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "memory"
                && entry.sample == "27:49:memory-cell:len=2:bits=3ff0000000000000-c00c000000000000"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "build-tower" && entry.sample == "28:50:rot=0x42100000:plans=5"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "unit-assembler"
                && entry.sample
                    == "29:51:tank-assembler:progress=0x3f000000:units=2:blocks=3:sample=b:8:command=0x41480000:0x41a00000:payload=1:pay-rot=0x40400000"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "mass-driver"
                && entry.sample == "30:52:link=20:22:rot=0x41200000:state=2"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "payload-mass-driver"
                && entry.sample
                    == "31:53:link=24:26:rot=0x41400000:state=3:reload=0x3f200000:charge=0x3f400000:loaded=1:charging=0:payload=1"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "sorter"
                && entry.sample == "32:54:item=7:legacy=1:sides=0x05:buffered=3"
        }));
        let payload_source_icon = scene
            .objects
            .iter()
            .find(|object| object.id == "marker:runtime-build-config-icon:payload-source:21:43:1:7")
            .expect("payload-source config icon should be present");
        assert_eq!(payload_source_icon.x, 168.0);
        assert_eq!(payload_source_icon.y, 344.0);
        let payload_source_command = scene
            .objects
            .iter()
            .find(|object| {
                object.id
                    == "marker:runtime-payload-source-command:payload-source:21:43:0x41480000:0x41900000"
            })
            .expect("payload-source command marker should be present");
        assert_eq!(payload_source_command.x, 12.5);
        assert_eq!(payload_source_command.y, 18.0);
        let payload_router_icon = scene
            .objects
            .iter()
            .find(|object| object.id == "marker:runtime-build-config-icon:payload-router:22:44:6:9")
            .expect("payload-router config icon should be present");
        assert_eq!(payload_router_icon.x, 176.0);
        assert_eq!(payload_router_icon.y, 352.0);
    }

    #[test]
    fn runtime_configured_payload_loader_family_label_compacts_payload_runtime_sample() {
        let values = BTreeMap::from([(
            pack_runtime_point2(25, 47),
            PayloadLoaderRuntimeProjection {
                exporting: true,
                payload_present: true,
                payload_type: Some(0),
                pay_rotation_bits: 0x4000_0000,
                payload_build_block_id: None,
                payload_build_revision: None,
                payload_unit_class_id: Some(9),
                payload_unit_payload_len: Some(128),
                payload_unit_payload_sha256: Some("0123456789abcdef".to_string()),
            },
        )]);

        assert_eq!(
            runtime_configured_payload_loader_family_label("pl", &values),
            "pl1@25:47=exp:y1:t0:r40000000:uc:9:l128:s0123456789ab"
        );
    }

    #[test]
    fn runtime_typed_build_config_value_label_formats_payload_loader_like_constructor_runtime() {
        let label = runtime_typed_build_config_value_label(
            TypedBuildingRuntimeKind::PayloadLoader,
            &TypedBuildingRuntimeValue::PayloadLoader {
                exporting: Some(false),
                payload_present: Some(true),
                payload_type: Some(0),
                pay_rotation_bits: Some(0x4040_0000),
                payload_build_block_id: None,
                payload_build_revision: None,
                payload_unit_class_id: Some(9),
                payload_unit_payload_len: Some(128),
                payload_unit_payload_sha256: Some("0123456789abcdef".to_string()),
            },
        );

        assert_eq!(
            label,
            "mode=import:y1:payload-type=0:r40400000:payload=uc:9:unit-len=128:unit-sha=0123456789ab"
        );
    }

    #[test]
    fn runtime_typed_build_config_value_label_formats_payload_source_runtime() {
        let label = runtime_typed_build_config_value_label(
            TypedBuildingRuntimeKind::PayloadSource,
            &TypedBuildingRuntimeValue::PayloadSource {
                configured_content: Some(ConfiguredContentRef {
                    content_type: 6,
                    content_id: 9,
                }),
                command_pos: Some((40.0f32.to_bits(), 60.0f32.to_bits())),
                pay_vector_x_bits: Some(0x4120_0000),
                pay_vector_y_bits: Some(0x41a0_0000),
                pay_rotation_bits: Some(0x4040_0000),
                payload_present: Some(true),
                payload_type: Some(1),
                payload_build_block_id: None,
                payload_build_revision: None,
                payload_unit_class_id: Some(11),
                payload_unit_payload_len: Some(128),
                payload_unit_payload_sha256: Some("0123456789abcdef".to_string()),
            },
        );

        assert_eq!(
            label,
            "content=u:9:command=0x42200000:0x42700000:payload=1:payload-type=1:vec=0x41200000:0x41a00000:rot=0x40400000:payload-ref=uc:11:unit-len=128:unit-sha=0123456789ab"
        );
    }

    #[test]
    fn runtime_typed_build_config_value_label_formats_payload_router_runtime() {
        let label = runtime_typed_build_config_value_label(
            TypedBuildingRuntimeKind::PayloadRouter,
            &TypedBuildingRuntimeValue::PayloadRouter {
                sorted_content: Some(ConfiguredContentRef {
                    content_type: 1,
                    content_id: 9,
                }),
                progress_bits: Some(0x3f40_0000),
                item_rotation_bits: Some(0x4040_0000),
                payload_present: Some(true),
                payload_type: Some(0),
                payload_kind: Some(PayloadRouterPayloadKind::Unit),
                payload_build_block_id: None,
                payload_build_revision: None,
                payload_unit_class_id: Some(11),
                payload_unit_revision: Some(2),
                payload_serialized_len: Some(5),
                payload_serialized_sha256: Some("0123456789abcdef".to_string()),
                rec_dir: Some(3),
            },
        );

        assert_eq!(
            label,
            "content=b:9:progress=0x3f400000:item-rot=0x40400000:payload=1:payload-type=0:payload-kind=unit:payload-ref=uc:11@r2:payload-len=5:payload-sha=0123456789ab:rec-dir=3"
        );
    }

    #[test]
    fn runtime_typed_build_config_value_label_formats_mass_driver_runtime() {
        let label = runtime_typed_build_config_value_label(
            TypedBuildingRuntimeKind::MassDriver,
            &TypedBuildingRuntimeValue::MassDriver {
                link: Some(pack_runtime_point2(20, 22)),
                rotation_bits: Some(0x4120_0000),
                state_ordinal: Some(2),
            },
        );

        assert_eq!(label, "link=20:22:rot=0x41200000:state=2");
    }

    #[test]
    fn runtime_typed_build_config_value_label_formats_payload_mass_driver_runtime() {
        let label = runtime_typed_build_config_value_label(
            TypedBuildingRuntimeKind::PayloadMassDriver,
            &TypedBuildingRuntimeValue::PayloadMassDriver {
                link: Some(pack_runtime_point2(24, 26)),
                turret_rotation_bits: Some(0x4140_0000),
                state_ordinal: Some(3),
                reload_counter_bits: Some(0x3f20_0000),
                charge_bits: Some(0x3f40_0000),
                loaded: Some(true),
                charging: Some(false),
                payload_present: Some(true),
            },
        );

        assert_eq!(
            label,
            "link=24:26:rot=0x41400000:state=3:reload=0x3f200000:charge=0x3f400000:loaded=1:charging=0:payload=1"
        );
    }

    #[test]
    fn runtime_typed_build_config_value_label_formats_sorter_runtime() {
        let label = runtime_typed_build_config_value_label(
            TypedBuildingRuntimeKind::Sorter,
            &TypedBuildingRuntimeValue::Sorter {
                item_id: Some(7),
                legacy: Some(true),
                non_empty_side_mask: Some(0x05),
                buffered_item_count: Some(3),
            },
        );

        assert_eq!(label, "item=7:legacy=1:sides=0x05:buffered=3");
    }

    #[test]
    fn render_runtime_adapter_renders_unit_assembler_and_driver_link_markers() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();

        state
            .configured_block_projection
            .unit_assembler_by_build_pos
            .insert(
                pack_runtime_point2(30, 40),
                crate::session_state::UnitAssemblerRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    unit_ids: vec![3, 7],
                    block_entry_count: 4,
                    block_sample: Some(ConfiguredContentRef {
                        content_type: 1,
                        content_id: 9,
                    }),
                    command_pos: Some((40.0f32.to_bits(), 60.0f32.to_bits())),
                    payload_present: false,
                    pay_rotation_bits: 0x4080_0000,
                },
            );
        state
            .configured_block_projection
            .mass_driver_link_by_build_pos
            .insert(
                pack_runtime_point2(12, 14),
                Some(pack_runtime_point2(20, 22)),
            );
        state
            .configured_block_projection
            .mass_driver_runtime_by_build_pos
            .insert(
                pack_runtime_point2(12, 14),
                crate::session_state::MassDriverRuntimeProjection {
                    rotation_bits: 0x4120_0000,
                    state_ordinal: 2,
                },
            );
        state
            .configured_block_projection
            .payload_mass_driver_link_by_build_pos
            .insert(
                pack_runtime_point2(16, 18),
                Some(pack_runtime_point2(24, 26)),
            );
        state
            .configured_block_projection
            .payload_mass_driver_runtime_by_build_pos
            .insert(
                pack_runtime_point2(16, 18),
                crate::session_state::PayloadMassDriverRuntimeProjection {
                    turret_rotation_bits: 0x4140_0000,
                    state_ordinal: 3,
                    reload_counter_bits: 0x3f20_0000,
                    charge_bits: 0x3f40_0000,
                    loaded: true,
                    charging: false,
                    payload_present: true,
                },
            );
        state
            .configured_block_projection
            .payload_mass_driver_link_by_build_pos
            .insert(
                pack_runtime_point2(28, 30),
                Some(pack_runtime_point2(32, 34)),
            );
        state
            .configured_block_projection
            .payload_mass_driver_runtime_by_build_pos
            .insert(
                pack_runtime_point2(28, 30),
                crate::session_state::PayloadMassDriverRuntimeProjection {
                    turret_rotation_bits: 0x4150_0000,
                    state_ordinal: 4,
                    reload_counter_bits: 0x3f60_0000,
                    charge_bits: 0x3f70_0000,
                    loaded: false,
                    charging: true,
                    payload_present: false,
                },
            );

        for (build_pos, block_name) in [
            (pack_runtime_point2(30, 40), "tank-assembler"),
            (pack_runtime_point2(12, 14), "mass-driver"),
            (pack_runtime_point2(16, 18), "payload-mass-driver"),
            (pack_runtime_point2(28, 30), "large-payload-mass-driver"),
        ] {
            state.building_table_projection.by_build_pos.insert(
                build_pos,
                crate::session_state::BuildingProjection {
                    block_id: Some(1),
                    block_name: Some(block_name.to_string()),
                    rotation: None,
                    team_id: None,
                    io_version: None,
                    module_bitmask: None,
                    time_scale_bits: None,
                    time_scale_duration_bits: None,
                    last_disabler_pos: None,
                    legacy_consume_connected: None,
                    config: None,
                    health_bits: None,
                    enabled: None,
                    efficiency: None,
                    optional_efficiency: None,
                    visible_flags: None,
                    turret_reload_counter_bits: None,
                    turret_rotation_bits: None,
                    item_turret_ammo_count: None,
                    continuous_turret_last_length_bits: None,
                    build_turret_rotation_bits: None,
                    build_turret_plans_present: None,
                    build_turret_plan_count: None,
                    last_update: crate::session_state::BuildingProjectionUpdateKind::TileConfig,
                },
            );
        }

        adapter.apply(&mut scene, &mut hud, &input, &state);

        let area_top = scene
            .objects
            .iter()
            .find(|object| {
                object.id == "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top"
            })
            .expect("unit-assembler area top line should be present");
        assert_eq!((area_top.x, area_top.y), (224.0, 304.0));
        let area_top_end = scene
            .objects
            .iter()
            .find(|object| {
                object.id
                    == "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top:line-end"
            })
            .expect("unit-assembler area top line end should be present");
        assert_eq!((area_top_end.x, area_top_end.y), (264.0, 304.0));
        assert!(scene.objects.iter().any(|object| {
            object.id
                == "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:0:0x40800000"
                && object.x == 240.0
                && object.y == 320.0
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id
                == "marker:runtime-unit-assembler-command:tank-assembler:30:40:0x42200000:0x42700000"
                && object.x == 40.0
                && object.y == 60.0
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:line:runtime-driver-link:mass-driver:12:14:20:22"
                && object.x == 96.0
                && object.y == 112.0
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:line:runtime-driver-link:mass-driver:12:14:20:22:line-end"
                && object.x == 160.0
                && object.y == 176.0
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:line:runtime-driver-link:payload-mass-driver:16:18:24:26"
        }));
        assert!(scene.objects.iter().any(|object| {
            object.id == "marker:line:runtime-driver-link:large-payload-mass-driver:28:30:32:34"
        }));
        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                mdt_render_ui::render_model::RenderPrimitive::Line {
                    id,
                    layer,
                    x0,
                    y0,
                    x1,
                    y1,
                } if id == "marker:line:runtime-driver-link:mass-driver:12:14:20:22"
                    && *layer == 15
                    && *x0 == 96.0
                    && *y0 == 112.0
                    && *x1 == 160.0
                    && *y1 == 176.0
            )
        }));
        let build_ui = hud
            .build_ui
            .as_ref()
            .expect("build_ui observability should be present");
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "mass-driver"
                && entry.sample == "12:14:link=20:22:rot=0x41200000:state=2"
        }));
        assert!(build_ui.inspector_entries.iter().any(|entry| {
            entry.family == "payload-mass-driver"
                && entry.sample
                    == "28:30:large-payload-mass-driver:link=32:34:rot=0x41500000:state=4:reload=0x3f600000:charge=0x3f700000:loaded=0:charging=1:payload=0"
        }));
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

        let max_ttl = runtime_effect_overlay_ttl_ticks(Some(13))
            .max(runtime_effect_overlay_ttl_ticks(Some(21)));
        for _ in 0..(max_ttl - 1) {
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
    fn render_runtime_adapter_renders_spawn_effect_as_runtime_effect_overlay_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::SpawnEffect {
            x: 12.0,
            y: 20.0,
            rotation: 45.0,
            unit_type_id: Some(9),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:-1:0x{:08x}:0x{:08x}:0",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        let icon = first_runtime_effect_icon(&scene);
        assert_eq!(
            icon.id,
            format!(
                "marker:runtime-effect-icon:content-icon:normal:-1:6:9:0x{:08x}:0x{:08x}",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(icon.x, 12.0);
        assert_eq!(icon.y, 20.0);
    }

    #[test]
    fn render_runtime_adapter_skips_effect_requested_outside_default_clip_view_bounds() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events_with_view(
            &[ClientSessionEvent::EffectRequested {
                effect_id: None,
                x: 200.0,
                y: 0.0,
                rotation: 0.0,
                color_rgba: 0x11223344,
                data_object: None,
            }],
            Some(RuntimeEffectClipView {
                center: (0.0, 0.0),
                size: (100.0, 100.0),
            }),
        );
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
    }

    #[test]
    fn render_runtime_adapter_skips_effect_requested_with_invalid_clip_view() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events_with_view(
            &[ClientSessionEvent::EffectRequested {
                effect_id: None,
                x: 0.0,
                y: 0.0,
                rotation: 0.0,
                color_rgba: 0x11223344,
                data_object: None,
            }],
            Some(RuntimeEffectClipView {
                center: (0.0, 0.0),
                size: (0.0, 100.0),
            }),
        );
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
    }

    #[test]
    fn render_runtime_adapter_keeps_point_beam_when_large_clip_overlaps_view_bounds() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events_with_view(
            &[ClientSessionEvent::EffectRequested {
                effect_id: Some(10),
                x: 180.0,
                y: 0.0,
                rotation: 45.0,
                color_rgba: 0x11223344,
                data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
            }],
            Some(RuntimeEffectClipView {
                center: (0.0, 0.0),
                size: (100.0, 100.0),
            }),
        );
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let line = first_runtime_effect_line(&scene);
        assert_eq!(line.x, 180.0);
        assert_eq!(line.y, 0.0);
    }

    #[test]
    fn render_runtime_adapter_uses_packet_origin_for_clip_culling_before_contract_origin_projection(
    ) {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events_with_view(
            &[ClientSessionEvent::EffectRequested {
                effect_id: Some(10),
                x: 400.0,
                y: 400.0,
                rotation: 0.0,
                color_rgba: 0x11223344,
                data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
            }],
            Some(RuntimeEffectClipView {
                center: (80.0, 160.0),
                size: (40.0, 40.0),
            }),
        );
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
        assert!(!scene.objects.iter().any(|object| object
            .id
            .starts_with("marker:line:runtime-effect-point-beam:")));
    }

    #[test]
    fn render_runtime_adapter_applies_clip_culling_to_effect_reliable_requested() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events_with_view(
            &[ClientSessionEvent::EffectReliableRequested {
                effect_id: Some(13),
                x: 400.0,
                y: 0.0,
                rotation: 0.0,
                color_rgba: 0x11223344,
            }],
            Some(RuntimeEffectClipView {
                center: (0.0, 0.0),
                size: (100.0, 100.0),
            }),
        );
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(!scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
    }

    #[test]
    fn render_runtime_adapter_renders_point_beam_executor_line_from_packet_origin_to_target() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(10),
            x: 12.0,
            y: 20.0,
            rotation: 45.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:10:0x{:08x}:0x{:08x}:1",
                80.0f32.to_bits(),
                160.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 80.0);
        assert_eq!(marker.y, 160.0);

        let line = first_runtime_effect_line(&scene);
        assert_eq!(
            line.id,
            format!(
                "marker:line:runtime-effect-point-beam:normal:10:0x{:08x}:0x{:08x}:0x{:08x}:0x{:08x}",
                12.0f32.to_bits(),
                20.0f32.to_bits(),
                80.0f32.to_bits(),
                160.0f32.to_bits()
            )
        );
        assert_eq!(line.x, 12.0);
        assert_eq!(line.y, 20.0);

        let line_end = first_runtime_effect_line_end(&scene);
        assert_eq!(
            line_end.id,
            format!(
                "marker:line:runtime-effect-point-beam:normal:10:0x{:08x}:0x{:08x}:0x{:08x}:0x{:08x}:line-end",
                12.0f32.to_bits(),
                20.0f32.to_bits(),
                80.0f32.to_bits(),
                160.0f32.to_bits()
            )
        );
        assert_eq!(line_end.x, 80.0);
        assert_eq!(line_end.y, 160.0);
    }

    #[test]
    fn render_runtime_adapter_renders_leg_destroy_executor_line_to_second_position() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(263),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ObjectArray(vec![
                mdt_typeio::TypeIoObject::Vec2 { x: 40.0, y: 60.0 },
                mdt_typeio::TypeIoObject::Vec2 { x: 72.0, y: 96.0 },
                mdt_typeio::TypeIoObject::Null,
            ])),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:263:0x{:08x}:0x{:08x}:1",
                72.0f32.to_bits(),
                96.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 72.0);
        assert_eq!(marker.y, 96.0);

        let leg_destroy_prefix = "marker:line:runtime-effect-leg-destroy:";
        let leg_destroy_lines = runtime_effect_lines_with_prefix(&scene, leg_destroy_prefix);
        assert_eq!(leg_destroy_lines.len(), 2);
        assert!(leg_destroy_lines.iter().any(|line| {
            !line.id.ends_with(":line-end")
                && line.id
                    == format!(
                        "marker:line:runtime-effect-leg-destroy:normal:263:0x{:08x}:0x{:08x}:0x{:08x}:0x{:08x}",
                        12.0f32.to_bits(),
                        20.0f32.to_bits(),
                        72.0f32.to_bits(),
                        96.0f32.to_bits()
                    )
                && line.x == 12.0
                && line.y == 20.0
        }));
        assert!(leg_destroy_lines.iter().any(|line| {
            line.id.ends_with(":line-end")
                && line.id
                    == format!(
                        "marker:line:runtime-effect-leg-destroy:normal:263:0x{:08x}:0x{:08x}:0x{:08x}:0x{:08x}:line-end",
                        12.0f32.to_bits(),
                        20.0f32.to_bits(),
                        72.0f32.to_bits(),
                        96.0f32.to_bits()
                    )
                && line.x == 72.0
                && line.y == 96.0
        }));
    }

    #[test]
    fn render_runtime_adapter_renders_regen_suppress_seek_executor_curve() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(178),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::Vec2 { x: 80.0, y: 160.0 }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let overlay = adapter
            .world_overlay()
            .effect_overlays
            .first()
            .expect("missing regen suppress seek overlay");
        let expected_marker = effect_contract_executor::marker_position_for_effect_overlay(
            overlay,
            overlay.source_x_bits,
            overlay.source_y_bits,
            overlay.x_bits,
            overlay.y_bits,
        )
        .expect("regen suppress seek should override marker position");
        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:178:0x{:08x}:0x{:08x}:1",
                expected_marker.0, expected_marker.1
            )
        );
        assert_eq!(marker.x.to_bits(), expected_marker.0);
        assert_eq!(marker.y.to_bits(), expected_marker.1);

        let seek_prefix = "marker:line:runtime-effect-regen-suppress-seek:";
        let seek_lines = runtime_effect_lines_with_prefix(&scene, seek_prefix);
        assert_eq!(seek_lines.len(), 12);
        assert!(seek_lines.iter().any(|line| {
            !line.id.ends_with(":line-end")
                && line.id.starts_with(
                    "marker:line:runtime-effect-regen-suppress-seek:normal:178:0x41400000:0x41a00000:"
                )
                && line.x == 12.0
                && line.y == 20.0
        }));
    }

    #[test]
    fn render_runtime_adapter_moves_regen_suppress_seek_geometry_with_parent_unit_source_follow() {
        let mut adapter = RenderRuntimeAdapter::default();
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
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(178),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);

        let mut first_scene = RenderModel::default();
        let mut first_hud = HudModel::default();
        adapter.apply(&mut first_scene, &mut first_hud, &input, &state);
        let first_marker = first_runtime_effect_marker(&first_scene);

        let seek_prefix = "marker:line:runtime-effect-regen-suppress-seek:";
        let first_points = runtime_effect_lines_with_prefix(&first_scene, seek_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();

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

        let mut second_scene = RenderModel::default();
        let mut second_hud = HudModel::default();
        adapter.apply(&mut second_scene, &mut second_hud, &input, &state);
        let second_marker = first_runtime_effect_marker(&second_scene);

        let second_points = runtime_effect_lines_with_prefix(&second_scene, seek_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();

        assert!(second_marker.x > first_marker.x);
        assert!(second_marker.y > first_marker.y);
        assert_eq!(first_points.len(), second_points.len());
        assert!(first_points
            .iter()
            .any(|point| { (point.0 - 12.0).abs() < 0.01 && (point.1 - 20.0).abs() < 0.01 }));
        assert!(second_points
            .iter()
            .any(|point| { (point.0 - 28.0).abs() < 0.01 && (point.1 - 44.0).abs() < 0.01 }));
    }

    #[test]
    fn render_runtime_adapter_renders_chain_lightning_executor_segments() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(261),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:261:0x{:08x}:0x{:08x}:1",
                80.0f32.to_bits(),
                160.0f32.to_bits()
            )
        );

        let chain_prefix = "marker:line:runtime-effect-chain-lightning:";
        let chain_lines = runtime_effect_lines_with_prefix(&scene, chain_prefix);
        assert!(chain_lines.len() >= 6);
        assert!(chain_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x == 12.0 && object.y == 20.0
        }));
        assert!(chain_lines.iter().any(|object| {
            object.id.ends_with(":line-end") && object.x == 80.0 && object.y == 160.0
        }));
    }

    #[test]
    fn render_runtime_adapter_moves_chain_lightning_geometry_with_parent_unit_source_follow() {
        let mut adapter = RenderRuntimeAdapter::default();
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
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(261),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);

        let mut first_scene = RenderModel::default();
        let mut first_hud = HudModel::default();
        adapter.apply(&mut first_scene, &mut first_hud, &input, &state);

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

        let mut second_scene = RenderModel::default();
        let mut second_hud = HudModel::default();
        adapter.apply(&mut second_scene, &mut second_hud, &input, &state);

        let chain_prefix = "marker:line:runtime-effect-chain-lightning:";
        let first_points = runtime_effect_lines_with_prefix(&first_scene, chain_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();
        let second_points = runtime_effect_lines_with_prefix(&second_scene, chain_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();

        assert_eq!(first_points.len(), second_points.len());
        assert!(first_points
            .iter()
            .any(|point| { (point.0 - 12.0).abs() < 0.01 && (point.1 - 20.0).abs() < 0.01 }));
        assert!(second_points
            .iter()
            .any(|point| { (point.0 - 28.0).abs() < 0.01 && (point.1 - 44.0).abs() < 0.01 }));
    }

    #[test]
    fn render_runtime_adapter_renders_chain_emp_executor_segments() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(262),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:262:0x{:08x}:0x{:08x}:1",
                80.0f32.to_bits(),
                160.0f32.to_bits()
            )
        );

        let chain_prefix = "marker:line:runtime-effect-chain-emp:";
        let chain_lines = runtime_effect_lines_with_prefix(&scene, chain_prefix);
        assert!(chain_lines.len() >= 6);
        assert!(chain_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x == 12.0 && object.y == 20.0
        }));
        assert!(chain_lines.iter().any(|object| {
            object.id.ends_with(":line-end") && object.x == 80.0 && object.y == 160.0
        }));
    }

    #[test]
    fn render_runtime_adapter_moves_chain_emp_geometry_with_parent_unit_source_follow() {
        let mut adapter = RenderRuntimeAdapter::default();
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
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(262),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);

        let mut first_scene = RenderModel::default();
        let mut first_hud = HudModel::default();
        adapter.apply(&mut first_scene, &mut first_hud, &input, &state);

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

        let mut second_scene = RenderModel::default();
        let mut second_hud = HudModel::default();
        adapter.apply(&mut second_scene, &mut second_hud, &input, &state);

        let chain_prefix = "marker:line:runtime-effect-chain-emp:";
        let first_points = runtime_effect_lines_with_prefix(&first_scene, chain_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();
        let second_points = runtime_effect_lines_with_prefix(&second_scene, chain_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();

        assert_eq!(first_points.len(), second_points.len());
        assert!(first_points
            .iter()
            .any(|point| { (point.0 - 12.0).abs() < 0.01 && (point.1 - 20.0).abs() < 0.01 }));
        assert!(second_points
            .iter()
            .any(|point| { (point.0 - 28.0).abs() < 0.01 && (point.1 - 44.0).abs() < 0.01 }));
    }

    #[test]
    fn render_runtime_adapter_renders_shield_break_executor_hexagon() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(256),
            x: 32.0,
            y: 48.0,
            rotation: 6.0,
            color_rgba: 0x11223344,
            data_object: None,
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:256:0x{:08x}:0x{:08x}:0",
                32.0f32.to_bits(),
                48.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 32.0);
        assert_eq!(marker.y, 48.0);

        let shield_prefix = "marker:line:runtime-effect-shield-break:";
        let shield_lines = runtime_effect_lines_with_prefix(&scene, shield_prefix);
        assert_eq!(shield_lines.len(), 12);
        assert!(shield_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x == 38.0 && object.y == 48.0
        }));
    }

    #[test]
    fn render_runtime_adapter_renders_point_hit_executor_circle() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(11),
            x: 32.0,
            y: 48.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: None,
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:11:0x{:08x}:0x{:08x}:0",
                32.0f32.to_bits(),
                48.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 32.0);
        assert_eq!(marker.y, 48.0);

        let point_hit_prefix = "marker:line:runtime-effect-point-hit:";
        let point_hit_lines = runtime_effect_lines_with_prefix(&scene, point_hit_prefix);
        assert_eq!(point_hit_lines.len(), 24);
        assert!(point_hit_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x > 32.0 && object.y == 48.0
        }));
    }

    #[test]
    fn render_runtime_adapter_renders_lightning_executor_polyline_segments() {
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
            data_object: Some(mdt_typeio::TypeIoObject::Vec2Array(vec![
                (10.0, 20.0),
                (30.0, 40.0),
                (50.0, 60.0),
            ])),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:13:0x{:08x}:0x{:08x}:1",
                50.0f32.to_bits(),
                60.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 50.0);
        assert_eq!(marker.y, 60.0);

        let lightning_prefix = "marker:line:runtime-effect-lightning:";
        let lightning_lines = runtime_effect_lines_with_prefix(&scene, lightning_prefix);
        assert_eq!(lightning_lines.len(), 4);
        assert!(lightning_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x == 10.0 && object.y == 20.0
        }));
        assert!(lightning_lines.iter().any(|object| {
            object.id.ends_with(":line-end") && object.x == 50.0 && object.y == 60.0
        }));
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
    fn render_runtime_adapter_renders_unit_spirit_executor_double_diamond() {
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

        let unit_spirit_prefix = "marker:line:runtime-effect-unit-spirit:";
        let unit_spirit_lines = runtime_effect_lines_with_prefix(&scene, unit_spirit_prefix);
        assert_eq!(unit_spirit_lines.len(), 16);
        assert!(unit_spirit_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x < 20.0 && object.y < 40.0
        }));
    }

    #[test]
    fn render_runtime_adapter_moves_unit_spirit_geometry_with_parent_unit_source_follow() {
        let mut adapter = RenderRuntimeAdapter::default();
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
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(8),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);

        let mut first_scene = RenderModel::default();
        let mut first_hud = HudModel::default();
        adapter.apply(&mut first_scene, &mut first_hud, &input, &state);

        let first_marker = first_runtime_effect_marker(&first_scene);
        assert_eq!(first_marker.x, 80.0);
        assert_eq!(first_marker.y, 160.0);

        let unit_spirit_prefix = "marker:line:runtime-effect-unit-spirit:";
        let mut first_points = runtime_effect_lines_with_prefix(&first_scene, unit_spirit_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();
        first_points.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        });

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

        let mut second_scene = RenderModel::default();
        let mut second_hud = HudModel::default();
        adapter.apply(&mut second_scene, &mut second_hud, &input, &state);

        let second_marker = first_runtime_effect_marker(&second_scene);
        assert_eq!(second_marker.x, 96.0);
        assert_eq!(second_marker.y, 184.0);

        let mut second_points = runtime_effect_lines_with_prefix(&second_scene, unit_spirit_prefix)
            .into_iter()
            .map(|object| (object.x, object.y))
            .collect::<Vec<_>>();
        second_points.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        });

        assert_eq!(first_points.len(), second_points.len());
        for (first, second) in first_points.iter().zip(second_points.iter()) {
            assert!(((second.0 - first.0) - 16.0).abs() < 0.01);
            assert!(((second.1 - first.1) - 24.0).abs() < 0.01);
        }
    }

    #[test]
    fn render_runtime_adapter_renders_item_transfer_executor_rings() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(9),
            x: 1.0,
            y: 2.0,
            rotation: 0.0,
            color_rgba: 0x55667788,
            data_object: Some(mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert!(marker.id.starts_with("marker:runtime-effect:normal:9:"));
        assert!(marker.x != 80.0 || marker.y != 160.0);

        let item_transfer_prefix = "marker:line:runtime-effect-item-transfer:";
        let item_transfer_lines = runtime_effect_lines_with_prefix(&scene, item_transfer_prefix);
        assert_eq!(item_transfer_lines.len(), 32);
        assert!(item_transfer_lines.iter().any(|object| {
            !object.id.ends_with(":line-end") && object.x < 20.0 && object.y < 40.0
        }));
    }

    #[test]
    fn render_runtime_adapter_moves_item_transfer_marker_with_parent_unit_source_follow() {
        let mut adapter = RenderRuntimeAdapter::default();
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
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(9),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x55667788,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);

        let mut first_scene = RenderModel::default();
        let mut first_hud = HudModel::default();
        adapter.apply(&mut first_scene, &mut first_hud, &input, &state);
        let first_marker = first_runtime_effect_marker(&first_scene);

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

        let mut second_scene = RenderModel::default();
        let mut second_hud = HudModel::default();
        adapter.apply(&mut second_scene, &mut second_hud, &input, &state);
        let second_marker = first_runtime_effect_marker(&second_scene);

        assert!(((second_marker.x - first_marker.x) - 16.0).abs() < 0.01);
        assert!(((second_marker.y - first_marker.y) - 24.0).abs() < 0.01);
    }

    #[test]
    fn render_runtime_adapter_projects_building_pos_effect_payload_to_tile_world_position() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(12),
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
            "marker:runtime-effect:normal:12:0x41000000:0x41800000:1"
        );
        assert_eq!(marker.x, 8.0);
        assert_eq!(marker.y, 16.0);
    }

    #[test]
    fn render_runtime_adapter_projects_drop_item_effect_payload_along_zero_rotation() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(142),
            x: 10.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ContentRaw {
                content_type: 0,
                content_id: 7,
            }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:142:0x{:08x}:0x{:08x}:1",
                30.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 30.0);
        assert_eq!(marker.y, 20.0);
    }

    #[test]
    fn render_runtime_adapter_renders_block_content_icon_effect_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(252),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 42,
            }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:252:0x{:08x}:0x{:08x}:1",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 12.0);
        assert_eq!(marker.y, 20.0);

        let icon = first_runtime_effect_icon(&scene);
        assert_eq!(
            icon.id,
            format!(
                "marker:runtime-effect-icon:block-content-icon:normal:252:1:42:0x{:08x}:0x{:08x}",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(icon.x, 12.0);
        assert_eq!(icon.y, 20.0);
    }

    #[test]
    fn render_runtime_adapter_renders_core_build_block_effect_icon_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(15),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ContentRaw {
                content_type: 1,
                content_id: 42,
            }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let icon = first_runtime_effect_icon(&scene);
        assert_eq!(
            icon.id,
            format!(
                "marker:runtime-effect-icon:block-content-icon:normal:15:1:42:0x{:08x}:0x{:08x}",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(icon.x, 12.0);
        assert_eq!(icon.y, 20.0);
    }

    #[test]
    fn render_runtime_adapter_renders_unit_assemble_effect_icon_marker() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(35),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ContentRaw {
                content_type: 6,
                content_id: 9,
            }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let icon = first_runtime_effect_icon(&scene);
        assert_eq!(
            icon.id,
            format!(
                "marker:runtime-effect-icon:content-icon:normal:35:6:9:0x{:08x}:0x{:08x}",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(icon.x, 12.0);
        assert_eq!(icon.y, 20.0);
    }

    #[test]
    fn render_runtime_adapter_renders_payload_deposit_effect_icon_from_source_toward_target() {
        let mut adapter = RenderRuntimeAdapter::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(26),
            x: 12.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ObjectArray(vec![
                mdt_typeio::TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 42,
                },
                mdt_typeio::TypeIoObject::Point2 { x: 10, y: 20 },
            ])),
        }]);

        let mut first_scene = RenderModel::default();
        let mut first_hud = HudModel::default();
        adapter.apply(&mut first_scene, &mut first_hud, &input, &state);

        let marker = first_runtime_effect_marker(&first_scene);
        assert_eq!(marker.x, 80.0);
        assert_eq!(marker.y, 160.0);
        let first_icon = first_runtime_effect_icon(&first_scene);
        assert_eq!(
            first_icon.id,
            format!(
                "marker:runtime-effect-icon:payload-deposit:normal:26:1:42:0x{:08x}:0x{:08x}",
                12.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(first_icon.x, 12.0);
        assert_eq!(first_icon.y, 20.0);

        adapter.observe_events(&[]);

        let mut second_scene = RenderModel::default();
        let mut second_hud = HudModel::default();
        adapter.apply(&mut second_scene, &mut second_hud, &input, &state);

        let second_icon = first_runtime_effect_icon(&second_scene);
        let payload_progress = 1.0 / f32::from(runtime_effect_overlay_ttl_ticks(Some(26)) - 1);
        let expected_second_x = 12.0 + (80.0 - 12.0) * payload_progress;
        let expected_second_y = 20.0 + (160.0 - 20.0) * payload_progress;
        assert_eq!(
            second_icon.id,
            format!(
                "marker:runtime-effect-icon:payload-deposit:normal:26:1:42:0x{:08x}:0x{:08x}",
                expected_second_x.to_bits(),
                expected_second_y.to_bits()
            )
        );
        assert_eq!(second_icon.x, expected_second_x);
        assert_eq!(second_icon.y, expected_second_y);
    }

    #[test]
    fn render_runtime_adapter_projects_drop_item_effect_payload_along_ninety_degrees() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(142),
            x: 10.0,
            y: 20.0,
            rotation: 90.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::ContentRaw {
                content_type: 0,
                content_id: 7,
            }),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:142:0x{:08x}:0x{:08x}:1",
                10.0f32.to_bits(),
                40.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 10.0);
        assert_eq!(marker.y, 40.0);
    }

    #[test]
    fn render_runtime_adapter_projects_float_length_effect_payload_to_ray_endpoint() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(200),
            x: 10.0,
            y: 20.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::Float(16.0)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:200:0x{:08x}:0x{:08x}:1",
                26.0f32.to_bits(),
                20.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 26.0);
        assert_eq!(marker.y, 20.0);

        let line_id = format!(
            "marker:line:runtime-effect-float-length:normal:200:0x{:08x}:0x{:08x}:0x{:08x}:0x{:08x}",
            10.0f32.to_bits(),
            20.0f32.to_bits(),
            26.0f32.to_bits(),
            20.0f32.to_bits()
        );
        let line = scene_object_by_id(&scene, &line_id).expect("expected float_length line");
        assert_eq!(line.x, 10.0);
        assert_eq!(line.y, 20.0);

        let line_end = scene_object_by_id(&scene, &format!("{line_id}:line-end"))
            .expect("expected float_length line end");
        assert_eq!(line_end.x, 26.0);
        assert_eq!(line_end.y, 20.0);
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
            x: 20.0,
            y: 24.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            "marker:runtime-effect:normal:257:0x41a00000:0x41c00000:1"
        );
        assert_eq!(marker.x, 20.0);
        assert_eq!(marker.y, 24.0);

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
            "marker:runtime-effect:normal:257:0x42000000:0x42100000:1"
        );
        assert_eq!(updated_marker.x, 32.0);
        assert_eq!(updated_marker.y, 36.0);
    }

    #[test]
    fn render_runtime_adapter_culls_hidden_unit_parent_effect_overlays_after_hidden_snapshot() {
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
            x: 20.0,
            y: 24.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);
        assert_eq!(adapter.world_overlay().effect_overlays.len(), 1);
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));

        state.apply_hidden_snapshot(
            crate::session_state::AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(404),
                sample_ids: vec![404],
            },
            BTreeSet::from([404]),
        );

        let mut hidden_scene = RenderModel::default();
        let mut hidden_hud = HudModel::default();
        adapter.apply(&mut hidden_scene, &mut hidden_hud, &input, &state);

        assert!(adapter.world_overlay().effect_overlays.is_empty());
        assert!(!hidden_scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
    }

    #[test]
    fn runtime_effect_overlay_ttl_ticks_match_rot_with_parent_effect_lifetimes() {
        assert_eq!(runtime_effect_overlay_ttl_ticks(Some(67)), 80);
        assert_eq!(runtime_effect_overlay_ttl_ticks(Some(68)), 40);
        assert_eq!(runtime_effect_overlay_ttl_ticks(Some(122)), 120);
    }

    #[test]
    fn runtime_effect_overlay_ttl_ticks_match_leg_destroy_lifetime() {
        assert_eq!(runtime_effect_overlay_ttl_ticks(Some(263)), 90);
    }

    #[test]
    fn runtime_effect_overlay_ttl_ticks_match_regen_suppress_seek_lifetime() {
        assert_eq!(runtime_effect_overlay_ttl_ticks(Some(178)), 140);
    }

    #[test]
    fn runtime_effect_overlay_ttl_ticks_match_drill_steam_lifetime() {
        assert_eq!(runtime_effect_overlay_ttl_ticks(Some(124)), 220);
    }

    #[test]
    fn render_runtime_adapter_hides_drill_steam_until_start_delay_elapses() {
        let mut adapter = RenderRuntimeAdapter::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(124),
            x: 32.0,
            y: 48.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: None,
        }]);

        let mut initial_scene = RenderModel::default();
        let mut initial_hud = HudModel::default();
        adapter.apply(&mut initial_scene, &mut initial_hud, &input, &state);
        assert!(!initial_scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));

        for _ in 0..29 {
            adapter.observe_events(&[]);
            let mut delayed_scene = RenderModel::default();
            let mut delayed_hud = HudModel::default();
            adapter.apply(&mut delayed_scene, &mut delayed_hud, &input, &state);
            assert!(!delayed_scene
                .objects
                .iter()
                .any(|object| object.id.starts_with("marker:runtime-effect:")));
        }

        adapter.observe_events(&[]);
        let mut visible_scene = RenderModel::default();
        let mut visible_hud = HudModel::default();
        adapter.apply(&mut visible_scene, &mut visible_hud, &input, &state);

        let marker = first_runtime_effect_marker(&visible_scene);
        assert_eq!(
            marker.id,
            "marker:runtime-effect:normal:124:0x42000000:0x42400000:0"
        );
        assert_eq!(marker.x, 32.0);
        assert_eq!(marker.y, 48.0);
    }

    #[test]
    fn render_runtime_adapter_renders_drill_steam_lines_after_start_delay_elapses() {
        let mut adapter = RenderRuntimeAdapter::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(124),
            x: 32.0,
            y: 48.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: None,
        }]);

        let mut initial_scene = RenderModel::default();
        let mut initial_hud = HudModel::default();
        adapter.apply(&mut initial_scene, &mut initial_hud, &input, &state);
        assert!(runtime_effect_lines_with_prefix(
            &initial_scene,
            "marker:line:runtime-effect-drill-steam:"
        )
        .is_empty());

        for _ in 0..29 {
            adapter.observe_events(&[]);
            let mut delayed_scene = RenderModel::default();
            let mut delayed_hud = HudModel::default();
            adapter.apply(&mut delayed_scene, &mut delayed_hud, &input, &state);
            assert!(runtime_effect_lines_with_prefix(
                &delayed_scene,
                "marker:line:runtime-effect-drill-steam:"
            )
            .is_empty());
        }

        adapter.observe_events(&[]);
        let mut visible_scene = RenderModel::default();
        let mut visible_hud = HudModel::default();
        adapter.apply(&mut visible_scene, &mut visible_hud, &input, &state);

        let steam_lines = runtime_effect_lines_with_prefix(
            &visible_scene,
            "marker:line:runtime-effect-drill-steam:",
        );
        assert_eq!(steam_lines.len(), 48);
        assert!(steam_lines
            .iter()
            .any(|object| object.x != 32.0 || object.y != 48.0));
    }

    #[test]
    fn render_runtime_adapter_renders_arc_shield_break_lines_for_parent_unit() {
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
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            crate::session_state::EntitySemanticProjectionEntry {
                class_id: 12,
                last_seen_entity_snapshot_count: 3,
                projection: crate::session_state::EntitySemanticProjection::Unit(
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

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(257),
            x: 12.0,
            y: 16.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(marker.x, 12.0);
        assert_eq!(marker.y, 16.0);

        let shield_prefix = "marker:line:runtime-effect-arc-shield-break:";
        let shield_lines = runtime_effect_lines_with_prefix(&scene, shield_prefix);
        assert_eq!(shield_lines.len(), 36);
        assert!(shield_lines.iter().any(|object| object.y > 28.0));
    }

    #[test]
    fn render_runtime_adapter_renders_unit_shield_break_lines_for_parent_unit() {
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
            effect_id: Some(260),
            x: 12.0,
            y: 16.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(marker.x, 12.0);
        assert_eq!(marker.y, 16.0);

        let shield_prefix = "marker:line:runtime-effect-unit-shield-break:";
        let shield_lines = runtime_effect_lines_with_prefix(&scene, shield_prefix);
        assert_eq!(shield_lines.len(), 40);
        assert!(shield_lines
            .iter()
            .any(|object| object.x == 26.0 && object.y == 16.0));
    }

    #[test]
    fn render_runtime_adapter_renders_green_laser_charge_lines_for_parent_unit() {
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
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            crate::session_state::EntitySemanticProjectionEntry {
                class_id: 12,
                last_seen_entity_snapshot_count: 3,
                projection: crate::session_state::EntitySemanticProjection::Unit(
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

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(67),
            x: 12.0,
            y: 16.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(marker.x, 12.0);
        assert_eq!(marker.y, 16.0);

        let charge_prefix = "marker:line:runtime-effect-green-laser-charge:";
        let charge_lines = runtime_effect_lines_with_prefix(&scene, charge_prefix);
        assert_eq!(charge_lines.len(), 32);
        assert!(charge_lines
            .iter()
            .any(|object| object.x == 12.0 && object.y == 56.0));
    }

    #[test]
    fn render_runtime_adapter_renders_green_laser_charge_small_lines_for_parent_unit() {
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
            effect_id: Some(68),
            x: 12.0,
            y: 16.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(marker.x, 12.0);
        assert_eq!(marker.y, 16.0);

        let charge_prefix = "marker:line:runtime-effect-green-laser-charge-small:";
        let charge_lines = runtime_effect_lines_with_prefix(&scene, charge_prefix);
        assert_eq!(charge_lines.len(), 24);
        assert!(charge_lines
            .iter()
            .any(|object| object.x == 62.0 && object.y == 16.0));
    }

    #[test]
    fn render_runtime_adapter_culls_hidden_non_local_parent_unit_effect_overlays() {
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
            x: 12.0,
            y: 16.0,
            rotation: 0.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert_eq!(adapter.world_overlay().effect_overlays.len(), 1);
        assert!(scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));

        state.hidden_snapshot_ids = BTreeSet::from([404]);
        state.last_hidden_snapshot = Some(crate::session_state::AppliedHiddenSnapshotIds {
            count: 1,
            first_id: Some(404),
            sample_ids: vec![404],
        });
        state.entity_table_projection.by_entity_id.insert(
            404,
            crate::session_state::EntityProjection {
                class_id: 12,
                hidden: true,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 88,
                x_bits: 12.0f32.to_bits(),
                y_bits: 16.0f32.to_bits(),
                last_seen_entity_snapshot_count: 4,
            },
        );

        let mut hidden_scene = RenderModel::default();
        let mut hidden_hud = HudModel::default();
        adapter.apply(&mut hidden_scene, &mut hidden_hud, &input, &state);

        assert!(adapter.world_overlay().effect_overlays.is_empty());
        assert!(!hidden_scene
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:runtime-effect:")));
    }

    #[test]
    fn render_runtime_adapter_preserves_snapshot_input_offset_for_missing_parent_unit() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let mut input = ClientSnapshotInputState {
            unit_id: Some(404),
            dead: false,
            position: Some((44.0, 60.0)),
            rotation: 0.0,
            ..Default::default()
        };
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(257),
            x: 46.0,
            y: 60.0,
            rotation: 15.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:257:0x{:08x}:0x{:08x}:1",
                46.0f32.to_bits(),
                60.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 46.0);
        assert_eq!(marker.y, 60.0);

        input.position = Some((50.0, 60.0));
        input.rotation = 90.0;
        let mut updated_scene = RenderModel::default();
        let mut updated_hud = HudModel::default();
        adapter.apply(&mut updated_scene, &mut updated_hud, &input, &state);

        let updated_marker = first_runtime_effect_marker(&updated_scene);
        assert_eq!(
            updated_marker.id,
            format!(
                "marker:runtime-effect:normal:257:0x{:08x}:0x{:08x}:1",
                50.0f32.to_bits(),
                62.0f32.to_bits()
            )
        );
        assert_eq!(updated_marker.x, 50.0);
        assert_eq!(updated_marker.y, 62.0);
    }

    #[test]
    fn render_runtime_adapter_preserves_world_player_offset_for_missing_parent_unit() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let mut input = ClientSnapshotInputState {
            unit_id: Some(404),
            dead: false,
            rotation: 0.0,
            ..Default::default()
        };
        let mut state = SessionState::default();
        state.world_player_x_bits = Some(44.0f32.to_bits());
        state.world_player_y_bits = Some(60.0f32.to_bits());

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(260),
            x: 46.0,
            y: 60.0,
            rotation: 15.0,
            color_rgba: 0x11223344,
            data_object: Some(mdt_typeio::TypeIoObject::UnitId(404)),
        }]);
        adapter.apply(&mut scene, &mut hud, &input, &state);

        let marker = first_runtime_effect_marker(&scene);
        assert_eq!(
            marker.id,
            format!(
                "marker:runtime-effect:normal:260:0x{:08x}:0x{:08x}:1",
                46.0f32.to_bits(),
                60.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 46.0);
        assert_eq!(marker.y, 60.0);

        state.world_player_x_bits = Some(50.0f32.to_bits());
        state.world_player_y_bits = Some(60.0f32.to_bits());
        input.rotation = 90.0;
        let mut updated_scene = RenderModel::default();
        let mut updated_hud = HudModel::default();
        adapter.apply(&mut updated_scene, &mut updated_hud, &input, &state);

        let updated_marker = first_runtime_effect_marker(&updated_scene);
        assert_eq!(
            updated_marker.id,
            format!(
                "marker:runtime-effect:normal:260:0x{:08x}:0x{:08x}:1",
                50.0f32.to_bits(),
                62.0f32.to_bits()
            )
        );
        assert_eq!(updated_marker.x, 50.0);
        assert_eq!(updated_marker.y, 62.0);
    }

    #[test]
    fn render_runtime_adapter_keeps_original_effect_position_when_parent_unit_missing_and_no_fallback(
    ) {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
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
                1.0f32.to_bits(),
                2.0f32.to_bits()
            )
        );
        assert_eq!(marker.x, 1.0);
        assert_eq!(marker.y, 2.0);
    }

    #[test]
    fn render_runtime_adapter_projects_packed_point2_array_first_effect_payload() {
        let mut adapter = RenderRuntimeAdapter::default();
        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = ClientSnapshotInputState::default();
        let state = SessionState::default();

        adapter.observe_events(&[ClientSessionEvent::EffectRequested {
            effect_id: Some(12),
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
                "marker:runtime-effect:normal:12:0x{:08x}:0x{:08x}:1",
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
        assert_eq!(
            runtime_effect_business_projection_label(Some(
                &EffectBusinessProjection::PositionTarget {
                    source_x_bits: 32.5f32.to_bits(),
                    source_y_bits: 48.0f32.to_bits(),
                    target_x_bits: 80.0f32.to_bits(),
                    target_y_bits: 160.0f32.to_bits(),
                }
            )),
            "target:0x42020000:0x42400000:0x42a00000:0x43200000"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(
                &EffectBusinessProjection::PayloadTargetContent {
                    source_x_bits: 12.0f32.to_bits(),
                    source_y_bits: 20.0f32.to_bits(),
                    target_x_bits: 84.0f32.to_bits(),
                    target_y_bits: 140.0f32.to_bits(),
                    content_type: 6,
                    content_id: 9,
                }
            )),
            "payloadTarget:0x41400000:0x41a00000:0x42a80000:0x430c0000:6:9"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(&EffectBusinessProjection::LengthRay {
                source_x_bits: 32.5f32.to_bits(),
                source_y_bits: 48.0f32.to_bits(),
                target_x_bits: 45.0f32.to_bits(),
                target_y_bits: 48.0f32.to_bits(),
                rotation_bits: 0.0f32.to_bits(),
                length_bits: 12.5f32.to_bits(),
            })),
            "ray:0x42020000:0x42400000:0x42340000:0x42400000:0x00000000:0x41480000"
        );
        assert_eq!(
            runtime_effect_business_projection_label(Some(
                &EffectBusinessProjection::LightningPath {
                    points: vec![
                        (10.0f32.to_bits(), 20.0f32.to_bits()),
                        (50.0f32.to_bits(), 60.0f32.to_bits()),
                    ],
                }
            )),
            "lightningPath:2:0x42480000:0x42700000"
        );
        assert_eq!(runtime_effect_business_projection_label(None), "none");
    }

    #[test]
    fn runtime_world_position_from_contract_effect_projection_uses_target_position() {
        assert_eq!(
            runtime_world_position_from_effect_business_projection(Some(
                &EffectBusinessProjection::PositionTarget {
                    source_x_bits: 32.5f32.to_bits(),
                    source_y_bits: 48.0f32.to_bits(),
                    target_x_bits: 80.0f32.to_bits(),
                    target_y_bits: 160.0f32.to_bits(),
                }
            )),
            Some(RuntimeWorldPositionObservability {
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
            })
        );
        assert_eq!(
            runtime_world_position_from_effect_business_projection(Some(
                &EffectBusinessProjection::PayloadTargetContent {
                    source_x_bits: 12.0f32.to_bits(),
                    source_y_bits: 20.0f32.to_bits(),
                    target_x_bits: 84.0f32.to_bits(),
                    target_y_bits: 140.0f32.to_bits(),
                    content_type: 6,
                    content_id: 9,
                }
            )),
            Some(RuntimeWorldPositionObservability {
                x_bits: 84.0f32.to_bits(),
                y_bits: 140.0f32.to_bits(),
            })
        );
        assert_eq!(
            runtime_world_position_from_effect_business_projection(Some(
                &EffectBusinessProjection::LengthRay {
                    source_x_bits: 32.5f32.to_bits(),
                    source_y_bits: 48.0f32.to_bits(),
                    target_x_bits: 45.0f32.to_bits(),
                    target_y_bits: 48.0f32.to_bits(),
                    rotation_bits: 0.0f32.to_bits(),
                    length_bits: 12.5f32.to_bits(),
                }
            )),
            Some(RuntimeWorldPositionObservability {
                x_bits: 45.0f32.to_bits(),
                y_bits: 48.0f32.to_bits(),
            })
        );
        assert_eq!(
            runtime_world_position_from_effect_business_projection(Some(
                &EffectBusinessProjection::LightningPath {
                    points: vec![
                        (10.0f32.to_bits(), 20.0f32.to_bits()),
                        (50.0f32.to_bits(), 60.0f32.to_bits()),
                    ],
                }
            )),
            Some(RuntimeWorldPositionObservability {
                x_bits: 50.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
            })
        );
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
    fn render_runtime_adapter_reports_effect_source_binding_end_to_end_in_hud() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        ingest_sample_world(&mut session);
        let local_player_entity_id = session
            .state()
            .entity_table_projection
            .local_player_entity_id
            .unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(8, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(&mut payload, &TypeIoObject::UnitId(local_player_entity_id));
        let packet = encode_packet(packet_id, &payload, false).unwrap();
        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_runtime_binding_state,
            Some(crate::session_state::EffectRuntimeBindingState::ParentFollow)
        );
        assert_eq!(
            session.state().last_effect_runtime_source_binding_state,
            Some(crate::session_state::EffectRuntimeBindingState::ParentFollow)
        );

        let mut adapter = RenderRuntimeAdapter::default();
        adapter.observe_events(&[event]);

        let mut scene = RenderModel::default();
        let mut hud = HudModel::default();
        let input = session.snapshot_input();
        adapter.apply(&mut scene, &mut hud, input, session.state());

        assert!(hud
            .status_text
            .contains("runtime_effect_binding=follow/follow"));
        let live_effect = &hud
            .runtime_ui
            .as_ref()
            .expect("runtime_ui observability should be present")
            .live
            .effect;
        assert_eq!(live_effect.active_overlay_count, 1);
        assert_eq!(live_effect.active_effect_id, Some(8));
        assert_eq!(
            live_effect.last_contract_name.as_deref(),
            Some("position_target")
        );
    }

    #[test]
    fn runtime_effect_binding_label_distinguishes_reject_and_unresolved_fallback() {
        let input = ClientSnapshotInputState::default();
        let mut reject_state = SessionState::default();
        reject_state.last_effect_runtime_binding_state =
            Some(EffectRuntimeBindingState::BindingRejected);
        assert_eq!(
            runtime_effect_binding_label(&input, &reject_state, &RuntimeWorldOverlay::default()),
            "reject/none"
        );

        let mut world_overlay = RuntimeWorldOverlay::default();
        world_overlay.effect_overlays.push(RuntimeEffectOverlay {
            effect_id: Some(257),
            source_x_bits: 1.0f32.to_bits(),
            source_y_bits: 2.0f32.to_bits(),
            source_binding: None,
            x_bits: 1.0f32.to_bits(),
            y_bits: 2.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("unit_parent"),
            binding: Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 1.0f32.to_bits(),
                spawn_y_bits: 2.0f32.to_bits(),
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
            content_ref: None,
            polyline_points: Vec::new(),
        });
        assert_eq!(
            runtime_effect_binding_label(&input, &SessionState::default(), &world_overlay),
            "fallback/none"
        );
    }

    #[test]
    fn runtime_effect_binding_label_prefers_session_pair_over_active_overlay() {
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.last_effect_runtime_binding_state = Some(EffectRuntimeBindingState::BindingRejected);
        state.last_effect_runtime_source_binding_state =
            Some(EffectRuntimeBindingState::ParentFollow);

        let mut world_overlay = RuntimeWorldOverlay::default();
        world_overlay.effect_overlays.push(RuntimeEffectOverlay {
            effect_id: Some(9),
            source_x_bits: 1.0f32.to_bits(),
            source_y_bits: 2.0f32.to_bits(),
            source_binding: Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 1.0f32.to_bits(),
                spawn_y_bits: 2.0f32.to_bits(),
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
            x_bits: 3.0f32.to_bits(),
            y_bits: 4.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 405,
                spawn_x_bits: 3.0f32.to_bits(),
                spawn_y_bits: 4.0f32.to_bits(),
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
            content_ref: None,
            polyline_points: Vec::new(),
        });

        assert_eq!(
            runtime_effect_binding_label(&input, &state, &world_overlay),
            "reject/follow"
        );
    }

    #[test]
    fn runtime_effect_binding_label_does_not_mix_session_target_with_overlay_source() {
        let input = ClientSnapshotInputState::default();
        let mut state = SessionState::default();
        state.last_effect_runtime_binding_state = Some(EffectRuntimeBindingState::BindingRejected);

        let mut world_overlay = RuntimeWorldOverlay::default();
        world_overlay.effect_overlays.push(RuntimeEffectOverlay {
            effect_id: Some(9),
            source_x_bits: 1.0f32.to_bits(),
            source_y_bits: 2.0f32.to_bits(),
            source_binding: Some(RuntimeEffectBinding::ParentUnit {
                unit_id: 404,
                spawn_x_bits: 1.0f32.to_bits(),
                spawn_y_bits: 2.0f32.to_bits(),
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
            x_bits: 3.0f32.to_bits(),
            y_bits: 4.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0,
            reliable: false,
            has_data: true,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("position_target"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        });

        assert_eq!(
            runtime_effect_binding_label(&input, &state, &world_overlay),
            "reject/none"
        );
    }

    #[test]
    fn runtime_effect_binding_label_reports_source_binding_family_target_source_pairs() {
        let input = ClientSnapshotInputState::default();

        for (target, source, expected) in [
            (
                EffectRuntimeBindingState::ParentFollow,
                EffectRuntimeBindingState::ParentFollow,
                "follow/follow",
            ),
            (
                EffectRuntimeBindingState::UnresolvedFallback,
                EffectRuntimeBindingState::UnresolvedFallback,
                "fallback/fallback",
            ),
            (
                EffectRuntimeBindingState::ParentFollow,
                EffectRuntimeBindingState::BindingRejected,
                "follow/reject",
            ),
        ] {
            let mut state = SessionState::default();
            state.last_effect_runtime_binding_state = Some(target);
            state.last_effect_runtime_source_binding_state = Some(source);

            assert_eq!(
                runtime_effect_binding_label(&input, &state, &RuntimeWorldOverlay::default()),
                expected
            );
        }
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
        state.entity_table_projection.upsert_local_player(
            404,
            2,
            999,
            20.0f32.to_bits(),
            33.0f32.to_bits(),
            false,
            3,
        );
        state.entity_table_projection.upsert_entity(
            903,
            35,
            false,
            0,
            0,
            10.0f32.to_bits(),
            12.0f32.to_bits(),
            false,
            2,
        );
        state.entity_table_projection.upsert_entity(
            904,
            35,
            false,
            0,
            0,
            40.0f32.to_bits(),
            60.0f32.to_bits(),
            false,
            4,
        );
        state.entity_semantic_projection.upsert(
            903,
            35,
            2,
            crate::session_state::EntitySemanticProjection::WorldLabel(
                crate::session_state::EntityWorldLabelSemanticProjection {
                    flags: 1,
                    font_size_bits: 8.0f32.to_bits(),
                    text: Some("older label".to_string()),
                    z_bits: 2.0f32.to_bits(),
                },
            ),
        );
        state.entity_semantic_projection.upsert(
            904,
            35,
            4,
            crate::session_state::EntitySemanticProjection::WorldLabel(
                crate::session_state::EntityWorldLabelSemanticProjection {
                    flags: 3,
                    font_size_bits: 12.0f32.to_bits(),
                    text: Some("world label".to_string()),
                    z_bits: 4.0f32.to_bits(),
                },
            ),
        );
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
        state.last_effect_id = Some(8);
        state.last_effect_x_bits = Some(22.0f32.to_bits());
        state.last_effect_y_bits = Some(30.0f32.to_bits());
        state.last_effect_data_kind = Some("Point2".to_string());
        state.last_effect_contract_name = Some("position_target".to_string());
        state.last_effect_reliable_contract_name = Some("unit_parent".to_string());
        state.last_effect_data_semantic =
            Some(crate::session_state::EffectDataSemantic::Point2 { x: 3, y: 4 });
        state.last_effect_data_business_hint = Some(
            crate::effect_data_runtime::EffectDataBusinessHint::PositionHint(
                mdt_typeio::TypeIoEffectPositionHint::Point2 {
                    x: 3,
                    y: 4,
                    path: vec![1, 0],
                },
            ),
        );
        state.last_effect_business_projection = Some(EffectBusinessProjection::WorldPosition {
            source: EffectBusinessPositionSource::Point2,
            x_bits: 24.0f32.to_bits(),
            y_bits: 32.0f32.to_bits(),
        });
        state.last_effect_business_path = Some(vec![1, 0]);
        state.last_effect_runtime_binding_state = Some(EffectRuntimeBindingState::ParentFollow);
        state.last_effect_runtime_source_binding_state =
            Some(EffectRuntimeBindingState::BindingRejected);
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
        state.last_spawn_effect_x_bits = Some(18.0f32.to_bits());
        state.last_spawn_effect_y_bits = Some(28.0f32.to_bits());
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
        state.rules_projection.unit_cap = Some(180);
        state.rules_projection.default_team_id = Some(1);
        state.rules_projection.wave_team_id = Some(2);
        state.rules_projection.initial_wave_spacing = Some(90.0);
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
        state.last_announce_message = Some("announce".to_string());
        state.received_info_message_count = 13;
        state.last_info_message = Some("info".to_string());
        state.received_info_toast_count = 14;
        state.received_warning_toast_count = 15;
        state.received_info_popup_count = 16;
        state.received_info_popup_reliable_count = 17;
        state.last_info_toast_message = Some("toast".to_string());
        state.last_warning_toast_text = Some("warning".to_string());
        state.last_info_popup_reliable = Some(true);
        state.last_info_popup_id = Some("popup-a".to_string());
        state.last_info_popup_message = Some("popup text".to_string());
        state.last_info_popup_duration_bits = Some(2.5f32.to_bits());
        state.last_info_popup_align = Some(1);
        state.last_info_popup_top = Some(2);
        state.last_info_popup_left = Some(3);
        state.last_info_popup_bottom = Some(4);
        state.last_info_popup_right = Some(5);
        state.received_menu_open_count = 16;
        state.received_follow_up_menu_open_count = 17;
        state.received_hide_follow_up_menu_count = 18;
        state.last_menu_open_id = Some(40);
        state.last_menu_open_title = Some("main".to_string());
        state.last_menu_open_message = Some("pick".to_string());
        state.last_menu_open_option_rows = 2;
        state.last_menu_open_first_row_len = 3;
        state.last_follow_up_menu_open_id = Some(41);
        state.last_follow_up_menu_open_title = Some("follow".to_string());
        state.last_follow_up_menu_open_message = Some("next".to_string());
        state.last_follow_up_menu_open_option_rows = 1;
        state.last_follow_up_menu_open_first_row_len = 2;
        state.last_hide_follow_up_menu_id = Some(41);
        state.received_server_message_count = 7;
        state.last_server_message = Some("server text".to_string());
        state.received_chat_message_count = 8;
        state.last_chat_message = Some("[cyan]hello".to_string());
        state.last_chat_unformatted = Some("hello".to_string());
        state.last_chat_sender_entity_id = Some(404);
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
            .insert(
                pack_runtime_point2(1, 1),
                std::collections::BTreeMap::from([(4, 6), (7, 8)]),
            );
        state
            .resource_delta_projection
            .building_items_by_build
            .insert(
                pack_runtime_point2(2, 2),
                std::collections::BTreeMap::from([(9, 10)]),
            );
        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                900,
                crate::session_state::ResourceUnitItemStack {
                    item_id: Some(6),
                    amount: 3,
                },
            );
        state
            .resource_delta_projection
            .authoritative_build_update_count = 4;
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
        state.received_wave_advance_signal_count = 2;
        state.last_wave_advance_signal_from = Some(7);
        state.last_wave_advance_signal_to = Some(8);
        state.last_wave_advance_signal_apply_count = Some(4);
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
                    block_name: Some("power-node".to_string()),
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
                    turret_reload_counter_bits: None,
                    turret_rotation_bits: None,
                    item_turret_ammo_count: None,
                    continuous_turret_last_length_bits: None,
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
            last_block_name: Some("power-node".to_string()),
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
            last_turret_reload_counter_bits: None,
            last_turret_rotation_bits: None,
            last_item_turret_ammo_count: None,
            last_continuous_turret_last_length_bits: None,
            last_build_turret_rotation_bits: Some(0x4210_0000),
            last_build_turret_plans_present: None,
            last_build_turret_plan_count: None,
            last_update: Some(crate::session_state::BuildingProjectionUpdateKind::TileConfig),
            last_removed: false,
            last_block_snapshot_head_conflict: false,
        };
        state.core_inventory_runtime_binding_kind = Some(
            crate::session_state::CoreInventoryRuntimeBindingKind::FirstCorePerTeamApproximation,
        );
        state.core_inventory_runtime_ambiguous_team_count = 1;
        state.core_inventory_runtime_ambiguous_team_sample = vec![1];
        state.core_inventory_runtime_missing_team_count = 1;
        state.core_inventory_runtime_missing_team_sample = vec![4];
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
            .contains("runtime_core_binding=first-core-per-team:a1@1:m1@4"));
        assert!(hud
            .status_text
            .contains("runtime_builder=q1:i2:f3:r4:o1:finish@100:99:place:local1"));
        assert!(hud
            .status_text
            .contains("runtime_builder_head=flight@100:99:place:b301:r1"));
        let build_ui = hud
            .build_ui
            .as_ref()
            .expect("build_ui observability should be present");
        assert_eq!(build_ui.selected_block_id, input.selected_block_id);
        assert_eq!(build_ui.selected_rotation, input.selected_rotation);
        assert_eq!(build_ui.building, input.building);
        assert_eq!(build_ui.queued_count, 1);
        assert_eq!(build_ui.inflight_count, 2);
        assert_eq!(build_ui.finished_count, 3);
        assert_eq!(build_ui.removed_count, 4);
        assert_eq!(build_ui.orphan_authoritative_count, 1);
        let head = build_ui
            .head
            .as_ref()
            .expect("queue head should be present");
        assert_eq!((head.x, head.y), (100, 99));
        assert!(!head.breaking);
        assert_eq!(head.block_id, Some(301));
        assert_eq!(head.rotation, Some(1));
        assert_eq!(head.stage, BuildQueueHeadStage::InFlight);
        assert!(build_ui.inspector_entries.is_empty());
        assert!(hud.status_text.contains("runtime_core_teams=1"));
        assert!(hud.status_text.contains("runtime_core_items=2"));
        assert!(hud
            .status_text
            .contains("runtime_buildings=1:b1:c1:config@100:99#301:rm0:on1:e128:oe64"));
        assert!(hud
            .status_text
            .contains(":turrnone:tnone:anone:lnone:trb0x42100000"));
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
            .contains("runtime_effect_data_hint=pos:point2:3:4@1/0"));
        assert!(hud
            .status_text
            .contains("runtime_effect_apply=pos:point2:0x41c00000:0x42000000"));
        assert!(hud.status_text.contains("runtime_effect_path=1/0"));
        assert!(hud
            .status_text
            .contains("runtime_effect_binding=follow/reject"));
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
            "runtime_rules=sr67:srf68:so69:sof70:rule71:rf72:clr73:cmp74:wv1:pvp0:uc180:dt1:wt2:iws90:obj2:q1:par1:fg2:oor75:last9"
        ));
        assert!(hud.status_text.contains(
            "runtime_ui_notice=hud9:hudr10:hide11:ann12:info13:toast14:warn15:popup16:popr17:clip51@copied#6:uri52@https_//exam~#19:https"
        ));
        assert!(hud
            .status_text
            .contains("runtime_ui_menu=menu16:fmenu17:hfm18:tin53@404:Digits:12345#16:n1:e1"));
        assert!(hud
            .status_text
            .contains("runtime_chat=srv7@server_text:msg8@[cyan]hello:rawhello:s404"));
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
        assert_eq!(runtime_ui.hud_text.announce_count, 12);
        assert_eq!(
            runtime_ui.hud_text.last_announce_message.as_deref(),
            Some("announce")
        );
        assert_eq!(runtime_ui.hud_text.info_message_count, 13);
        assert_eq!(
            runtime_ui.hud_text.last_info_message.as_deref(),
            Some("info")
        );
        assert_eq!(runtime_ui.toast.info_count, 14);
        assert_eq!(runtime_ui.toast.warning_count, 15);
        assert_eq!(runtime_ui.toast.last_info_message.as_deref(), Some("toast"));
        assert_eq!(
            runtime_ui.toast.last_warning_text.as_deref(),
            Some("warning")
        );
        assert_eq!(runtime_ui.toast.info_popup_count, 16);
        assert_eq!(runtime_ui.toast.info_popup_reliable_count, 17);
        assert_eq!(runtime_ui.toast.last_info_popup_reliable, Some(true));
        assert_eq!(
            runtime_ui.toast.last_info_popup_id.as_deref(),
            Some("popup-a")
        );
        assert_eq!(
            runtime_ui.toast.last_info_popup_message.as_deref(),
            Some("popup text")
        );
        assert_eq!(
            runtime_ui.toast.last_info_popup_duration_bits,
            Some(2.5f32.to_bits())
        );
        assert_eq!(runtime_ui.toast.last_info_popup_align, Some(1));
        assert_eq!(runtime_ui.toast.last_info_popup_top, Some(2));
        assert_eq!(runtime_ui.toast.last_info_popup_left, Some(3));
        assert_eq!(runtime_ui.toast.last_info_popup_bottom, Some(4));
        assert_eq!(runtime_ui.toast.last_info_popup_right, Some(5));
        assert_eq!(runtime_ui.toast.clipboard_count, 51);
        assert_eq!(
            runtime_ui.toast.last_clipboard_text.as_deref(),
            Some("copied")
        );
        assert_eq!(runtime_ui.toast.open_uri_count, 52);
        assert_eq!(
            runtime_ui.toast.last_open_uri.as_deref(),
            Some("https://example.com")
        );
        assert_eq!(runtime_ui.chat.server_message_count, 7);
        assert_eq!(
            runtime_ui.chat.last_server_message.as_deref(),
            Some("server text")
        );
        assert_eq!(runtime_ui.chat.chat_message_count, 8);
        assert_eq!(
            runtime_ui.chat.last_chat_message.as_deref(),
            Some("[cyan]hello")
        );
        assert_eq!(
            runtime_ui.chat.last_chat_unformatted.as_deref(),
            Some("hello")
        );
        assert_eq!(runtime_ui.chat.last_chat_sender_entity_id, Some(404));
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
        assert_eq!(runtime_ui.admin.trace_info_count, 56);
        assert_eq!(runtime_ui.admin.trace_info_parse_fail_count, 76);
        assert_eq!(runtime_ui.admin.last_trace_info_player_id, Some(123456));
        assert_eq!(runtime_ui.admin.debug_status_client_count, 57);
        assert_eq!(runtime_ui.admin.debug_status_client_parse_fail_count, 77);
        assert_eq!(runtime_ui.admin.debug_status_client_unreliable_count, 58);
        assert_eq!(
            runtime_ui
                .admin
                .debug_status_client_unreliable_parse_fail_count,
            78
        );
        assert_eq!(runtime_ui.admin.last_debug_status_value, Some(12));
        assert_eq!(
            runtime_ui.session.core_binding.kind,
            Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation)
        );
        assert_eq!(runtime_ui.session.core_binding.ambiguous_team_count, 1);
        assert_eq!(
            runtime_ui.session.core_binding.ambiguous_team_sample,
            vec![1]
        );
        assert_eq!(runtime_ui.session.core_binding.missing_team_count, 1);
        assert_eq!(runtime_ui.session.core_binding.missing_team_sample, vec![4]);
        assert_eq!(runtime_ui.session.resource_delta.remove_tile_count, 80);
        assert_eq!(runtime_ui.session.resource_delta.set_tile_count, 81);
        assert_eq!(runtime_ui.session.resource_delta.set_floor_count, 82);
        assert_eq!(runtime_ui.session.resource_delta.set_overlay_count, 83);
        assert_eq!(runtime_ui.session.resource_delta.set_item_count, 22);
        assert_eq!(runtime_ui.session.resource_delta.set_items_count, 23);
        assert_eq!(runtime_ui.session.resource_delta.set_liquid_count, 24);
        assert_eq!(runtime_ui.session.resource_delta.set_liquids_count, 25);
        assert_eq!(runtime_ui.session.resource_delta.clear_items_count, 84);
        assert_eq!(runtime_ui.session.resource_delta.clear_liquids_count, 85);
        assert_eq!(runtime_ui.session.resource_delta.set_tile_items_count, 26);
        assert_eq!(runtime_ui.session.resource_delta.set_tile_liquids_count, 27);
        assert_eq!(runtime_ui.session.resource_delta.take_items_count, 1);
        assert_eq!(runtime_ui.session.resource_delta.transfer_item_to_count, 2);
        assert_eq!(
            runtime_ui
                .session
                .resource_delta
                .transfer_item_to_unit_count,
            3
        );
        assert_eq!(
            runtime_ui.session.resource_delta.last_kind.as_deref(),
            Some("to_unit")
        );
        assert_eq!(runtime_ui.session.resource_delta.last_item_id, Some(6));
        assert_eq!(runtime_ui.session.resource_delta.last_amount, None);
        assert_eq!(runtime_ui.session.resource_delta.last_build_pos, None);
        assert_eq!(runtime_ui.session.resource_delta.last_unit, None);
        assert_eq!(
            runtime_ui.session.resource_delta.last_to_entity_id,
            Some(404)
        );
        assert_eq!(runtime_ui.session.resource_delta.build_count, 2);
        assert_eq!(runtime_ui.session.resource_delta.build_stack_count, 3);
        assert_eq!(runtime_ui.session.resource_delta.entity_count, 1);
        assert_eq!(
            runtime_ui
                .session
                .resource_delta
                .authoritative_build_update_count,
            4
        );
        assert_eq!(runtime_ui.session.resource_delta.delta_apply_count, 5);
        assert_eq!(runtime_ui.session.resource_delta.delta_skip_count, 6);
        assert_eq!(runtime_ui.session.resource_delta.delta_conflict_count, 7);
        assert_eq!(
            runtime_ui.session.resource_delta.last_changed_build_pos,
            Some(pack_runtime_point2(9, 9))
        );
        assert_eq!(
            runtime_ui.session.resource_delta.last_changed_entity_id,
            Some(900)
        );
        assert_eq!(
            runtime_ui.session.resource_delta.last_changed_item_id,
            Some(6)
        );
        assert_eq!(
            runtime_ui.session.resource_delta.last_changed_amount,
            Some(1)
        );
        assert_eq!(runtime_ui.menu.menu_open_count, 16);
        assert_eq!(runtime_ui.menu.follow_up_menu_open_count, 17);
        assert_eq!(runtime_ui.menu.hide_follow_up_menu_count, 18);
        assert_eq!(runtime_ui.menu.last_menu_open_id, Some(40));
        assert_eq!(
            runtime_ui.menu.last_menu_open_title.as_deref(),
            Some("main")
        );
        assert_eq!(
            runtime_ui.menu.last_menu_open_message.as_deref(),
            Some("pick")
        );
        assert_eq!(runtime_ui.menu.last_menu_open_option_rows, 2);
        assert_eq!(runtime_ui.menu.last_menu_open_first_row_len, 3);
        assert_eq!(runtime_ui.menu.last_follow_up_menu_open_id, Some(41));
        assert_eq!(
            runtime_ui.menu.last_follow_up_menu_open_title.as_deref(),
            Some("follow")
        );
        assert_eq!(
            runtime_ui.menu.last_follow_up_menu_open_message.as_deref(),
            Some("next")
        );
        assert_eq!(runtime_ui.menu.last_follow_up_menu_open_option_rows, 1);
        assert_eq!(runtime_ui.menu.last_follow_up_menu_open_first_row_len, 2);
        assert_eq!(runtime_ui.menu.last_hide_follow_up_menu_id, Some(41));
        assert_eq!(runtime_ui.menu.menu_choose_count, 29);
        assert_eq!(runtime_ui.menu.last_menu_choose_menu_id, Some(404));
        assert_eq!(runtime_ui.menu.last_menu_choose_option, Some(2));
        assert_eq!(runtime_ui.menu.text_input_result_count, 30);
        assert_eq!(runtime_ui.menu.last_text_input_result_id, Some(405));
        assert_eq!(
            runtime_ui.menu.last_text_input_result_text.as_deref(),
            Some("ok123")
        );
        assert_eq!(runtime_ui.rules.set_rules_count, 67);
        assert_eq!(runtime_ui.rules.set_rules_parse_fail_count, 68);
        assert_eq!(runtime_ui.rules.set_objectives_count, 69);
        assert_eq!(runtime_ui.rules.set_objectives_parse_fail_count, 70);
        assert_eq!(runtime_ui.rules.set_rule_count, 71);
        assert_eq!(runtime_ui.rules.set_rule_parse_fail_count, 72);
        assert_eq!(runtime_ui.rules.clear_objectives_count, 73);
        assert_eq!(runtime_ui.rules.complete_objective_count, 74);
        assert_eq!(runtime_ui.rules.waves, Some(true));
        assert_eq!(runtime_ui.rules.pvp, Some(false));
        assert_eq!(runtime_ui.rules.objective_count, 2);
        assert_eq!(runtime_ui.rules.qualified_objective_count, 1);
        assert_eq!(runtime_ui.rules.objective_parent_edge_count, 1);
        assert_eq!(runtime_ui.rules.objective_flag_count, 2);
        assert_eq!(runtime_ui.rules.complete_out_of_range_count, 75);
        assert_eq!(runtime_ui.rules.last_completed_index, Some(9));
        assert_eq!(runtime_ui.world_labels.label_count, 19);
        assert_eq!(runtime_ui.world_labels.reliable_label_count, 20);
        assert_eq!(runtime_ui.world_labels.remove_label_count, 21);
        assert_eq!(runtime_ui.world_labels.active_count, 2);
        assert_eq!(runtime_ui.world_labels.last_entity_id, Some(904));
        assert_eq!(
            runtime_ui.world_labels.last_text.as_deref(),
            Some("world label")
        );
        assert_eq!(runtime_ui.world_labels.last_flags, Some(3));
        assert_eq!(
            runtime_ui.world_labels.last_font_size_bits,
            Some(12.0f32.to_bits())
        );
        assert_eq!(runtime_ui.world_labels.last_z_bits, Some(4.0f32.to_bits()));
        assert_eq!(
            runtime_ui.world_labels.last_position,
            Some(RuntimeWorldPositionObservability {
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
            })
        );
        assert_eq!(runtime_ui.markers.create_count, 54);
        assert_eq!(runtime_ui.markers.remove_count, 55);
        assert_eq!(runtime_ui.markers.update_count, 56);
        assert_eq!(runtime_ui.markers.update_text_count, 57);
        assert_eq!(runtime_ui.markers.update_texture_count, 58);
        assert_eq!(runtime_ui.markers.decode_fail_count, 2);
        assert_eq!(runtime_ui.markers.last_marker_id, Some(808));
        assert_eq!(
            runtime_ui.markers.last_control_name.as_deref(),
            Some("flushText")
        );
        assert_eq!(runtime_ui.live.entity.entity_count, 3);
        assert_eq!(runtime_ui.live.entity.hidden_count, 0);
        assert_eq!(runtime_ui.live.entity.player_count, 1);
        assert_eq!(runtime_ui.live.entity.unit_count, 0);
        assert_eq!(runtime_ui.live.entity.last_entity_id, Some(904));
        assert_eq!(runtime_ui.live.entity.last_player_entity_id, Some(404));
        assert_eq!(runtime_ui.live.entity.last_unit_entity_id, None);
        assert_eq!(runtime_ui.live.entity.local_entity_id, Some(404));
        assert_eq!(runtime_ui.live.entity.local_unit_kind, Some(2));
        assert_eq!(runtime_ui.live.entity.local_unit_value, Some(999));
        assert_eq!(runtime_ui.live.entity.local_hidden, Some(false));
        assert_eq!(
            runtime_ui.live.entity.local_last_seen_entity_snapshot_count,
            Some(3)
        );
        assert_eq!(
            runtime_ui.live.entity.local_position,
            Some(RuntimeWorldPositionObservability {
                x_bits: 20.0f32.to_bits(),
                y_bits: 33.0f32.to_bits(),
            })
        );
        assert_eq!(runtime_ui.live.effect.effect_count, 11);
        assert_eq!(runtime_ui.live.effect.spawn_effect_count, 73);
        assert_eq!(runtime_ui.live.effect.last_effect_id, Some(8));
        assert_eq!(
            runtime_ui.live.effect.last_spawn_effect_unit_type_id,
            Some(19)
        );
        assert_eq!(runtime_ui.live.effect.last_kind.as_deref(), Some("Point2"));
        assert_eq!(
            runtime_ui.live.effect.last_contract_name.as_deref(),
            Some("position_target")
        );
        assert_eq!(
            runtime_ui
                .live
                .effect
                .last_reliable_contract_name
                .as_deref(),
            Some("unit_parent")
        );
        assert_eq!(
            runtime_ui.live.effect.last_position_source,
            Some(RuntimeLiveEffectPositionSource::BusinessProjection)
        );
        assert_eq!(
            runtime_ui.live.effect.last_position_hint,
            Some(RuntimeWorldPositionObservability {
                x_bits: 24.0f32.to_bits(),
                y_bits: 32.0f32.to_bits(),
            })
        );
        assert!(hud
            .status_text
            .contains("runtime_world_label=lbl19:lblr20:rml21:act2:last904:f3:fs1094713344:z1082130432:pos40.0:60.0:txtworld_label"));
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
            .contains("runtime_gameplay_signal=flag46:go47:ugo48:sc49:res50:wave2@7>8#4"));
    }

    #[test]
    fn runtime_world_label_observability_tracks_hidden_labels_as_inactive() {
        let mut state = SessionState::default();
        state.entity_table_projection.upsert_entity(
            903,
            35,
            false,
            0,
            0,
            10.0f32.to_bits(),
            12.0f32.to_bits(),
            false,
            2,
        );
        state.entity_table_projection.upsert_entity(
            904,
            35,
            false,
            0,
            0,
            40.0f32.to_bits(),
            60.0f32.to_bits(),
            false,
            4,
        );
        state.entity_table_projection.upsert_entity(
            905,
            35,
            false,
            0,
            0,
            70.0f32.to_bits(),
            90.0f32.to_bits(),
            true,
            5,
        );
        state.entity_semantic_projection.upsert(
            903,
            35,
            2,
            crate::session_state::EntitySemanticProjection::WorldLabel(
                crate::session_state::EntityWorldLabelSemanticProjection {
                    flags: 1,
                    font_size_bits: 8.0f32.to_bits(),
                    text: Some("older".to_string()),
                    z_bits: 2.0f32.to_bits(),
                },
            ),
        );
        state.entity_semantic_projection.upsert(
            904,
            35,
            4,
            crate::session_state::EntitySemanticProjection::WorldLabel(
                crate::session_state::EntityWorldLabelSemanticProjection {
                    flags: 3,
                    font_size_bits: 12.0f32.to_bits(),
                    text: Some("active".to_string()),
                    z_bits: 4.0f32.to_bits(),
                },
            ),
        );
        state.entity_semantic_projection.upsert(
            905,
            35,
            5,
            crate::session_state::EntitySemanticProjection::WorldLabel(
                crate::session_state::EntityWorldLabelSemanticProjection {
                    flags: 7,
                    font_size_bits: 16.0f32.to_bits(),
                    text: Some("hidden".to_string()),
                    z_bits: 6.0f32.to_bits(),
                },
            ),
        );

        let observability = runtime_world_label_observability(&state);

        assert_eq!(observability.active_count, 2);
        assert_eq!(observability.inactive_count, 1);
        assert_eq!(observability.last_entity_id, Some(904));
        assert_eq!(observability.last_text.as_deref(), Some("active"));
        assert_eq!(
            observability.last_position,
            Some(RuntimeWorldPositionObservability {
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
            })
        );
    }

    #[test]
    fn runtime_live_effect_observability_prefers_active_overlay_state() {
        let mut state = SessionState::default();
        state.received_effect_count = 11;
        state.received_spawn_effect_count = 73;
        state.last_effect_id = Some(8);
        state.last_spawn_effect_unit_type_id = Some(19);
        state.last_effect_data_kind = Some("Point2".to_string());
        state.last_effect_contract_name = Some("position_target".to_string());
        state.last_effect_reliable_contract_name = Some("unit_parent".to_string());
        state.last_effect_data_business_hint = Some(
            crate::effect_data_runtime::EffectDataBusinessHint::PositionHint(
                mdt_typeio::TypeIoEffectPositionHint::Point2 {
                    x: 3,
                    y: 4,
                    path: vec![1, 0],
                },
            ),
        );
        state.last_effect_business_projection = Some(EffectBusinessProjection::WorldPosition {
            source: EffectBusinessPositionSource::Point2,
            x_bits: 24.0f32.to_bits(),
            y_bits: 32.0f32.to_bits(),
        });
        let mut world_overlay = RuntimeWorldOverlay::default();
        world_overlay.effect_overlays.push(RuntimeEffectOverlay {
            effect_id: Some(13),
            source_x_bits: 22.0f32.to_bits(),
            source_y_bits: 30.0f32.to_bits(),
            source_binding: None,
            x_bits: 28.0f32.to_bits(),
            y_bits: 36.0f32.to_bits(),
            rotation_bits: 0.0f32.to_bits(),
            color_rgba: 0xffffffff,
            reliable: true,
            has_data: false,
            lifetime_ticks: 3,
            remaining_ticks: 3,
            contract_name: Some("lightning"),
            binding: None,
            content_ref: None,
            polyline_points: Vec::new(),
        });

        let observability = runtime_live_effect_summary_observability(&state, &world_overlay);

        assert_eq!(observability.active_overlay_count, 1);
        assert_eq!(observability.active_effect_id, Some(13));
        assert_eq!(
            observability.active_contract_name.as_deref(),
            Some("lightning")
        );
        assert_eq!(observability.active_reliable, Some(true));
        assert_eq!(
            observability.active_position,
            Some(RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            })
        );
        assert_eq!(observability.display_effect_id(), Some(13));
        assert_eq!(observability.display_contract_name(), Some("lightning"));
        assert_eq!(
            observability.display_reliable_contract_name(),
            Some("lightning")
        );
        assert_eq!(
            observability.display_position_source(),
            Some(RuntimeLiveEffectPositionSource::ActiveOverlay)
        );
        assert_eq!(
            observability.display_position(),
            Some(&RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            })
        );
    }

    #[test]
    fn render_runtime_adapter_prefers_authoritative_state_mirror_in_hud() {
        let mut adapter = RenderRuntimeAdapter::default();
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
        let mut adapter = RenderRuntimeAdapter::default();
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
    fn runtime_local_entity_label_includes_owned_unit_runtime_sync_surface() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            crate::session_state::EntityProjection {
                class_id: crate::session_state::EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 4,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            202,
            crate::session_state::EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 4,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            4,
            crate::session_state::EntitySemanticProjection::Unit(
                crate::session_state::EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 55,
                    health_bits: 0,
                    rotation_bits: 0,
                    shield_bits: 0,
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: Some(crate::session_state::EntityUnitRuntimeSyncProjection {
                        ammo_bits: 0x3f80_0000,
                        elevation_bits: 0x4000_0000,
                        flag_bits: 0x0000_0000_0000_002a,
                        base_rotation_bits: Some(0x4040_0000),
                    }),
                    controller_type: 0,
                    controller_value: Some(101),
                },
            ),
        );
        state.refresh_runtime_typed_entity_from_tables(101);
        state.refresh_runtime_typed_entity_from_tables(202);

        assert_eq!(
            runtime_local_entity_label(&state),
            "101:c12:u2:202:h0:ou202@am0x3f800000:el0x40000000:fg0x000000000000002a:br0x40400000"
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
        state.last_unit_control_target =
            Some(crate::session_state::UnitRefProjection { kind: 2, value: 88 });
        state.received_unit_building_control_select_count = 8;
        state.last_unit_building_control_select_target =
            Some(crate::session_state::UnitRefProjection { kind: 1, value: 77 });
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
        let mut adapter = RenderRuntimeAdapter::default();
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
        state.record_reconnect_projection(
            ReconnectPhaseProjection::Attempting,
            Some(ReconnectReasonKind::ConnectRedirect),
            Some("connectRedirect".to_string()),
            None,
            Some("server requested redirect".to_string()),
        );
        state.received_connect_redirect_count = 1;
        state.last_connect_redirect_ip = Some("127.0.0.1".to_string());
        state.last_connect_redirect_port = Some(6567);

        adapter.apply(&mut scene, &mut hud, &input, &state);

        assert!(hud.status_text.contains(
            "runtime_loading=defer0:replay0:drop0:qdrop0:sfail0:scfail0:efail0:rdy0@none:to4:cto3:rto1:ltcload@300000:rs5:rr2:wr2:kr1:lrkick:lwr@lw1:cl1:rd0:cc1:p7:d8:r9"
        ));
        let session = &hud
            .runtime_ui
            .as_ref()
            .expect("runtime_ui should be present")
            .session;
        assert_eq!(session.loading.timeout_count, 4);
        assert_eq!(session.loading.connect_or_loading_timeout_count, 3);
        assert_eq!(session.loading.ready_snapshot_timeout_count, 1);
        assert_eq!(
            session.loading.last_timeout_kind,
            Some(RuntimeSessionTimeoutKind::ConnectOrLoading)
        );
        assert_eq!(session.loading.last_timeout_idle_ms, Some(300000));
        assert_eq!(session.loading.reset_count, 5);
        assert_eq!(session.loading.reconnect_reset_count, 2);
        assert_eq!(session.loading.world_reload_count, 2);
        assert_eq!(session.loading.kick_reset_count, 1);
        assert_eq!(
            session.loading.last_reset_kind,
            Some(RuntimeSessionResetKind::Kick)
        );
        assert_eq!(
            session
                .loading
                .last_world_reload
                .as_ref()
                .map(|world_reload| world_reload.cleared_pending_packets),
            Some(7)
        );
        assert_eq!(
            session.reconnect.phase,
            RuntimeReconnectPhaseObservability::Attempting
        );
        assert_eq!(
            session.reconnect.reason_kind,
            Some(RuntimeReconnectReasonKind::ConnectRedirect)
        );
        assert_eq!(
            session.reconnect.reason_text.as_deref(),
            Some("connectRedirect")
        );
        assert_eq!(
            session.reconnect.hint_text.as_deref(),
            Some("server requested redirect")
        );
        assert_eq!(session.reconnect.redirect_count, 1);
        assert_eq!(
            session.reconnect.last_redirect_ip.as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(session.reconnect.last_redirect_port, Some(6567));
    }
}
