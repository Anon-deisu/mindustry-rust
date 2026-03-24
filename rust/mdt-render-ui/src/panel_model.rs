use crate::{
    hud_model::{
        RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind, RuntimeSessionResetKind,
        RuntimeSessionTimeoutKind,
    },
    render_model::RenderObjectSemanticKind,
    BuildConfigAuthoritySourceObservability, BuildConfigOutcomeObservability, BuildQueueHeadStage,
    HudModel, RenderModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenterViewWindow {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapPanelModel {
    pub map_width: usize,
    pub map_height: usize,
    pub window: PresenterViewWindow,
    pub window_last_x: usize,
    pub window_last_y: usize,
    pub window_tile_count: usize,
    pub window_coverage_percent: usize,
    pub map_tile_count: usize,
    pub known_tile_count: usize,
    pub known_tile_percent: usize,
    pub unknown_tile_count: usize,
    pub unknown_tile_percent: usize,
    pub focus_tile: Option<(usize, usize)>,
    pub focus_in_window: Option<bool>,
    pub overlay_visible: bool,
    pub fog_enabled: bool,
    pub visible_tile_count: usize,
    pub visible_known_percent: usize,
    pub hidden_tile_count: usize,
    pub hidden_known_percent: usize,
    pub tracked_object_count: usize,
    pub player_count: usize,
    pub marker_count: usize,
    pub plan_count: usize,
    pub block_count: usize,
    pub runtime_count: usize,
    pub terrain_count: usize,
    pub unknown_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigPanelModel {
    pub selected_block_id: Option<i16>,
    pub selected_rotation: i32,
    pub building: bool,
    pub queued_count: usize,
    pub inflight_count: usize,
    pub pending_count: usize,
    pub finished_count: u64,
    pub removed_count: u64,
    pub orphan_authoritative_count: u64,
    pub tracked_family_count: usize,
    pub tracked_sample_count: usize,
    pub truncated_family_count: usize,
    pub selected_matches_head: Option<bool>,
    pub head: Option<BuildConfigHeadModel>,
    pub rollback_strip: BuildConfigRollbackStripModel,
    pub entries: Vec<BuildConfigPanelEntryModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigHeadModel {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub stage: BuildQueueHeadStage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigPanelEntryModel {
    pub family: String,
    pub tracked_count: usize,
    pub sample: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigRollbackStripModel {
    pub applied_authoritative_count: u64,
    pub rollback_count: u64,
    pub last_build_tile: Option<(i32, i32)>,
    pub last_business_applied: bool,
    pub last_cleared_pending_local: bool,
    pub last_was_rollback: bool,
    pub last_pending_local_match: Option<bool>,
    pub last_source: Option<BuildConfigAuthoritySourceObservability>,
    pub last_configured_outcome: Option<BuildConfigOutcomeObservability>,
    pub last_configured_block_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildInteractionMode {
    Idle,
    Place,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildInteractionSelectionState {
    Unarmed,
    Armed,
    HeadAligned,
    HeadDiverged,
    BreakingHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildInteractionQueueState {
    Empty,
    Queued,
    InFlight,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildInteractionAuthorityState {
    None,
    Applied,
    Cleared,
    Rollback,
    RejectedMissingBuilding,
    RejectedMissingBlockMetadata,
    RejectedUnsupportedBlock,
    RejectedUnsupportedConfigType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInteractionPanelModel {
    pub mode: BuildInteractionMode,
    pub selection_state: BuildInteractionSelectionState,
    pub queue_state: BuildInteractionQueueState,
    pub selected_block_id: Option<i16>,
    pub selected_rotation: i32,
    pub pending_count: usize,
    pub orphan_authoritative_count: u64,
    pub place_ready: bool,
    pub config_available: bool,
    pub config_family_count: usize,
    pub config_sample_count: usize,
    pub top_config_family: Option<String>,
    pub head: Option<BuildConfigHeadModel>,
    pub authority_state: BuildInteractionAuthorityState,
    pub authority_pending_match: Option<bool>,
    pub authority_source: Option<BuildConfigAuthoritySourceObservability>,
    pub authority_tile: Option<(i32, i32)>,
    pub authority_block_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeUiNoticePanelModel {
    pub hud_set_count: u64,
    pub hud_set_reliable_count: u64,
    pub hud_hide_count: u64,
    pub hud_last_message: Option<String>,
    pub hud_last_reliable_message: Option<String>,
    pub toast_info_count: u64,
    pub toast_warning_count: u64,
    pub toast_last_info_message: Option<String>,
    pub toast_last_warning_text: Option<String>,
    pub text_input_open_count: u64,
    pub text_input_last_id: Option<i32>,
    pub text_input_last_title: Option<String>,
    pub text_input_last_message: Option<String>,
    pub text_input_last_default_text: Option<String>,
    pub text_input_last_length: Option<i32>,
    pub text_input_last_numeric: Option<bool>,
    pub text_input_last_allow_empty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMenuPanelModel {
    pub menu_open_count: u64,
    pub follow_up_menu_open_count: u64,
    pub hide_follow_up_menu_count: u64,
    pub text_input_open_count: u64,
    pub text_input_last_id: Option<i32>,
    pub text_input_last_title: Option<String>,
    pub text_input_last_default_text: Option<String>,
    pub text_input_last_length: Option<i32>,
    pub text_input_last_numeric: Option<bool>,
    pub text_input_last_allow_empty: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDialogPromptKind {
    Menu,
    FollowUpMenu,
    TextInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDialogNoticeKind {
    Hud,
    HudReliable,
    ToastInfo,
    ToastWarning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDialogPanelModel {
    pub prompt_kind: Option<RuntimeDialogPromptKind>,
    pub prompt_active: bool,
    pub menu_open_count: u64,
    pub follow_up_menu_open_count: u64,
    pub hide_follow_up_menu_count: u64,
    pub text_input_open_count: u64,
    pub text_input_last_id: Option<i32>,
    pub text_input_last_title: Option<String>,
    pub text_input_last_message: Option<String>,
    pub text_input_last_default_text: Option<String>,
    pub text_input_last_length: Option<i32>,
    pub text_input_last_numeric: Option<bool>,
    pub text_input_last_allow_empty: Option<bool>,
    pub notice_kind: Option<RuntimeDialogNoticeKind>,
    pub notice_text: Option<String>,
    pub notice_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandModePanelModel {
    pub active: bool,
    pub selected_unit_count: usize,
    pub selected_unit_sample: Vec<i32>,
    pub command_building_count: usize,
    pub first_command_building: Option<i32>,
    pub command_rect: Option<crate::RuntimeCommandRectObservability>,
    pub control_groups: Vec<RuntimeCommandControlGroupPanelModel>,
    pub last_target: Option<crate::RuntimeCommandTargetObservability>,
    pub last_command_selection: Option<crate::RuntimeCommandSelectionObservability>,
    pub last_stance_selection: Option<crate::RuntimeCommandStanceObservability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandControlGroupPanelModel {
    pub index: u8,
    pub unit_count: usize,
    pub first_unit_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAdminPanelModel {
    pub trace_info_count: u64,
    pub trace_info_parse_fail_count: u64,
    pub last_trace_info_player_id: Option<i32>,
    pub debug_status_client_count: u64,
    pub debug_status_client_parse_fail_count: u64,
    pub debug_status_client_unreliable_count: u64,
    pub debug_status_client_unreliable_parse_fail_count: u64,
    pub last_debug_status_value: Option<i32>,
    pub parse_fail_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRulesPanelModel {
    pub mutation_count: u64,
    pub parse_fail_count: u64,
    pub set_rules_count: u64,
    pub set_objectives_count: u64,
    pub set_rule_count: u64,
    pub clear_objectives_count: u64,
    pub complete_objective_count: u64,
    pub waves: Option<bool>,
    pub pvp: Option<bool>,
    pub objective_count: usize,
    pub qualified_objective_count: usize,
    pub objective_parent_edge_count: usize,
    pub objective_flag_count: usize,
    pub complete_out_of_range_count: u64,
    pub last_completed_index: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorldLabelPanelModel {
    pub label_count: u64,
    pub reliable_label_count: u64,
    pub remove_label_count: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSessionPanelModel {
    pub kick: RuntimeKickPanelModel,
    pub loading: RuntimeLoadingPanelModel,
    pub reconnect: RuntimeReconnectPanelModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeKickPanelModel {
    pub reason_text: Option<String>,
    pub reason_ordinal: Option<i32>,
    pub hint_category: Option<String>,
    pub hint_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLoadingPanelModel {
    pub deferred_inbound_packet_count: u64,
    pub replayed_inbound_packet_count: u64,
    pub dropped_loading_low_priority_packet_count: u64,
    pub dropped_loading_deferred_overflow_count: u64,
    pub failed_state_snapshot_parse_count: u64,
    pub failed_state_snapshot_core_data_parse_count: u64,
    pub failed_entity_snapshot_parse_count: u64,
    pub ready_inbound_liveness_anchor_count: u64,
    pub last_ready_inbound_liveness_anchor_at_ms: Option<u64>,
    pub timeout_count: u64,
    pub connect_or_loading_timeout_count: u64,
    pub ready_snapshot_timeout_count: u64,
    pub last_timeout_kind: Option<RuntimeSessionTimeoutKind>,
    pub last_timeout_idle_ms: Option<u64>,
    pub reset_count: u64,
    pub reconnect_reset_count: u64,
    pub world_reload_count: u64,
    pub kick_reset_count: u64,
    pub last_reset_kind: Option<RuntimeSessionResetKind>,
    pub last_world_reload: Option<RuntimeWorldReloadPanelModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorldReloadPanelModel {
    pub had_loaded_world: bool,
    pub had_client_loaded: bool,
    pub was_ready_to_enter_world: bool,
    pub had_connect_confirm_sent: bool,
    pub cleared_pending_packets: usize,
    pub cleared_deferred_inbound_packets: usize,
    pub cleared_replayed_loading_events: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReconnectPanelModel {
    pub phase: RuntimeReconnectPhaseObservability,
    pub phase_transition_count: u64,
    pub reason_kind: Option<RuntimeReconnectReasonKind>,
    pub reason_text: Option<String>,
    pub reason_ordinal: Option<i32>,
    pub hint_text: Option<String>,
    pub redirect_count: u64,
    pub last_redirect_ip: Option<String>,
    pub last_redirect_port: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveEntityPanelModel {
    pub entity_count: usize,
    pub hidden_count: usize,
    pub local_entity_id: Option<i32>,
    pub local_unit_kind: Option<u8>,
    pub local_unit_value: Option<u32>,
    pub local_hidden: Option<bool>,
    pub local_last_seen_entity_snapshot_count: Option<u64>,
    pub local_position: Option<crate::RuntimeWorldPositionObservability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveEffectPanelModel {
    pub effect_count: u64,
    pub spawn_effect_count: u64,
    pub last_effect_id: Option<i16>,
    pub last_spawn_effect_unit_type_id: Option<i16>,
    pub last_kind: Option<String>,
    pub last_contract_name: Option<String>,
    pub last_reliable_contract_name: Option<String>,
    pub last_position_hint: Option<crate::RuntimeWorldPositionObservability>,
    pub last_position_source: Option<crate::RuntimeLiveEffectPositionSource>,
}

pub fn build_minimap_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<MinimapPanelModel> {
    let summary = hud.summary.as_ref()?;
    let window = resolve_presenter_window(scene, summary.map_width, summary.map_height, window);
    let player_count = semantic_count(scene, RenderObjectSemanticKind::Player);
    let marker_count = semantic_count(scene, RenderObjectSemanticKind::Marker);
    let plan_count = semantic_count(scene, RenderObjectSemanticKind::Plan);
    let block_count = semantic_count(scene, RenderObjectSemanticKind::Block);
    let runtime_count = semantic_count(scene, RenderObjectSemanticKind::Runtime);
    let terrain_count = semantic_count(scene, RenderObjectSemanticKind::Terrain);
    let unknown_count = semantic_count(scene, RenderObjectSemanticKind::Unknown);
    let window_last_x = window
        .origin_x
        .saturating_add(window.width.saturating_sub(1));
    let window_last_y = window
        .origin_y
        .saturating_add(window.height.saturating_sub(1));
    let window_tile_count = window.width.saturating_mul(window.height);
    let map_tile_count = summary.map_width.saturating_mul(summary.map_height);
    let known_tile_count = summary
        .visible_tile_count
        .saturating_add(summary.hidden_tile_count);
    let unknown_tile_count = map_tile_count.saturating_sub(known_tile_count);
    let focus_tile = scene.player_focus_tile(8.0);
    let focus_in_window = focus_tile.map(|(focus_x, focus_y)| {
        focus_x >= window.origin_x
            && focus_x <= window_last_x
            && focus_y >= window.origin_y
            && focus_y <= window_last_y
    });

    Some(MinimapPanelModel {
        map_width: summary.map_width,
        map_height: summary.map_height,
        window,
        window_last_x,
        window_last_y,
        window_tile_count,
        window_coverage_percent: percent_of(window_tile_count, map_tile_count),
        map_tile_count,
        known_tile_count,
        known_tile_percent: percent_of(known_tile_count, map_tile_count),
        unknown_tile_count,
        unknown_tile_percent: percent_of(unknown_tile_count, map_tile_count),
        focus_tile,
        focus_in_window,
        overlay_visible: summary.overlay_visible,
        fog_enabled: summary.fog_enabled,
        visible_tile_count: summary.visible_tile_count,
        visible_known_percent: percent_of(summary.visible_tile_count, known_tile_count),
        hidden_tile_count: summary.hidden_tile_count,
        hidden_known_percent: percent_of(summary.hidden_tile_count, known_tile_count),
        tracked_object_count: player_count
            .saturating_add(marker_count)
            .saturating_add(plan_count)
            .saturating_add(block_count)
            .saturating_add(runtime_count)
            .saturating_add(terrain_count)
            .saturating_add(unknown_count),
        player_count,
        marker_count,
        plan_count,
        block_count,
        runtime_count,
        terrain_count,
        unknown_count,
    })
}

fn percent_of(part: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        part.saturating_mul(100) / total
    }
}

pub fn build_build_config_panel(
    hud: &HudModel,
    max_entries: usize,
) -> Option<BuildConfigPanelModel> {
    let build_ui = hud.build_ui.as_ref()?;
    let mut entries = build_ui.inspector_entries.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .tracked_count
            .cmp(&left.tracked_count)
            .then_with(|| left.family.cmp(&right.family))
            .then_with(|| left.sample.cmp(&right.sample))
    });
    let tracked_family_count = entries.len();
    let tracked_sample_count = entries.iter().map(|entry| entry.tracked_count).sum();
    let capped_entries = entries
        .into_iter()
        .take(max_entries)
        .map(|entry| BuildConfigPanelEntryModel {
            family: entry.family.clone(),
            tracked_count: entry.tracked_count,
            sample: entry.sample.clone(),
        })
        .collect::<Vec<_>>();
    Some(BuildConfigPanelModel {
        selected_block_id: build_ui.selected_block_id,
        selected_rotation: build_ui.selected_rotation,
        building: build_ui.building,
        queued_count: build_ui.queued_count,
        inflight_count: build_ui.inflight_count,
        pending_count: build_ui
            .queued_count
            .saturating_add(build_ui.inflight_count),
        finished_count: build_ui.finished_count,
        removed_count: build_ui.removed_count,
        orphan_authoritative_count: build_ui.orphan_authoritative_count,
        tracked_family_count,
        tracked_sample_count,
        truncated_family_count: tracked_family_count.saturating_sub(capped_entries.len()),
        selected_matches_head: build_ui.head.as_ref().and_then(|head| {
            build_ui.selected_block_id.map(|selected_block_id| {
                head.block_id == Some(selected_block_id)
                    && head
                        .rotation
                        .map(|rotation| i32::from(rotation) == build_ui.selected_rotation)
                        .unwrap_or(true)
            })
        }),
        head: build_ui.head.as_ref().map(|head| BuildConfigHeadModel {
            x: head.x,
            y: head.y,
            breaking: head.breaking,
            block_id: head.block_id,
            rotation: head.rotation,
            stage: head.stage,
        }),
        rollback_strip: BuildConfigRollbackStripModel {
            applied_authoritative_count: build_ui.rollback_strip.applied_authoritative_count,
            rollback_count: build_ui.rollback_strip.rollback_count,
            last_build_tile: build_ui.rollback_strip.last_build_tile,
            last_business_applied: build_ui.rollback_strip.last_business_applied,
            last_cleared_pending_local: build_ui.rollback_strip.last_cleared_pending_local,
            last_was_rollback: build_ui.rollback_strip.last_was_rollback,
            last_pending_local_match: build_ui.rollback_strip.last_pending_local_match,
            last_source: build_ui.rollback_strip.last_source,
            last_configured_outcome: build_ui.rollback_strip.last_configured_outcome,
            last_configured_block_name: build_ui.rollback_strip.last_configured_block_name.clone(),
        },
        entries: capped_entries,
    })
}

pub fn build_build_interaction_panel(hud: &HudModel) -> Option<BuildInteractionPanelModel> {
    let build_ui = hud.build_ui.as_ref()?;
    let config_panel = build_build_config_panel(hud, 1)?;
    let mode = build_interaction_mode(build_ui, config_panel.head.as_ref());
    let selection_state =
        build_interaction_selection_state(build_ui, &config_panel, config_panel.head.as_ref());
    Some(BuildInteractionPanelModel {
        mode,
        selection_state,
        queue_state: build_interaction_queue_state(
            config_panel.queued_count,
            config_panel.inflight_count,
        ),
        selected_block_id: build_ui.selected_block_id,
        selected_rotation: build_ui.selected_rotation,
        pending_count: config_panel.pending_count,
        orphan_authoritative_count: build_ui.orphan_authoritative_count,
        place_ready: matches!(mode, BuildInteractionMode::Place)
            && build_ui.building
            && build_ui.selected_block_id.is_some(),
        config_available: config_panel.tracked_family_count > 0,
        config_family_count: config_panel.tracked_family_count,
        config_sample_count: config_panel.tracked_sample_count,
        top_config_family: config_panel
            .entries
            .first()
            .map(|entry| entry.family.clone()),
        head: config_panel.head.clone(),
        authority_state: build_interaction_authority_state(&config_panel.rollback_strip),
        authority_pending_match: config_panel.rollback_strip.last_pending_local_match,
        authority_source: config_panel.rollback_strip.last_source,
        authority_tile: config_panel.rollback_strip.last_build_tile,
        authority_block_name: config_panel
            .rollback_strip
            .last_configured_block_name
            .clone(),
    })
}

fn build_interaction_mode(
    build_ui: &crate::BuildUiObservability,
    head: Option<&BuildConfigHeadModel>,
) -> BuildInteractionMode {
    if head.map(|head| head.breaking).unwrap_or(false) {
        BuildInteractionMode::Break
    } else if build_ui.building && build_ui.selected_block_id.is_some() {
        BuildInteractionMode::Place
    } else {
        BuildInteractionMode::Idle
    }
}

fn build_interaction_selection_state(
    build_ui: &crate::BuildUiObservability,
    config_panel: &BuildConfigPanelModel,
    head: Option<&BuildConfigHeadModel>,
) -> BuildInteractionSelectionState {
    if head.map(|head| head.breaking).unwrap_or(false) {
        BuildInteractionSelectionState::BreakingHead
    } else if !build_ui.building || build_ui.selected_block_id.is_none() {
        BuildInteractionSelectionState::Unarmed
    } else {
        match config_panel.selected_matches_head {
            Some(true) => BuildInteractionSelectionState::HeadAligned,
            Some(false) => BuildInteractionSelectionState::HeadDiverged,
            None => BuildInteractionSelectionState::Armed,
        }
    }
}

fn build_interaction_queue_state(
    queued_count: usize,
    inflight_count: usize,
) -> BuildInteractionQueueState {
    match (queued_count > 0, inflight_count > 0) {
        (false, false) => BuildInteractionQueueState::Empty,
        (true, false) => BuildInteractionQueueState::Queued,
        (false, true) => BuildInteractionQueueState::InFlight,
        (true, true) => BuildInteractionQueueState::Mixed,
    }
}

fn build_interaction_authority_state(
    strip: &BuildConfigRollbackStripModel,
) -> BuildInteractionAuthorityState {
    match strip.last_configured_outcome {
        Some(BuildConfigOutcomeObservability::RejectedMissingBuilding) => {
            BuildInteractionAuthorityState::RejectedMissingBuilding
        }
        Some(BuildConfigOutcomeObservability::RejectedMissingBlockMetadata) => {
            BuildInteractionAuthorityState::RejectedMissingBlockMetadata
        }
        Some(BuildConfigOutcomeObservability::RejectedUnsupportedBlock) => {
            BuildInteractionAuthorityState::RejectedUnsupportedBlock
        }
        Some(BuildConfigOutcomeObservability::RejectedUnsupportedConfigType) => {
            BuildInteractionAuthorityState::RejectedUnsupportedConfigType
        }
        Some(BuildConfigOutcomeObservability::Applied) | None => {
            if strip.last_was_rollback {
                BuildInteractionAuthorityState::Rollback
            } else if strip.last_cleared_pending_local {
                BuildInteractionAuthorityState::Cleared
            } else if strip.last_business_applied || strip.last_source.is_some() {
                BuildInteractionAuthorityState::Applied
            } else {
                BuildInteractionAuthorityState::None
            }
        }
    }
}

pub fn build_runtime_ui_notice_panel(hud: &HudModel) -> Option<RuntimeUiNoticePanelModel> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    Some(RuntimeUiNoticePanelModel {
        hud_set_count: runtime_ui.hud_text.set_count,
        hud_set_reliable_count: runtime_ui.hud_text.set_reliable_count,
        hud_hide_count: runtime_ui.hud_text.hide_count,
        hud_last_message: runtime_ui.hud_text.last_message.clone(),
        hud_last_reliable_message: runtime_ui.hud_text.last_reliable_message.clone(),
        toast_info_count: runtime_ui.toast.info_count,
        toast_warning_count: runtime_ui.toast.warning_count,
        toast_last_info_message: runtime_ui.toast.last_info_message.clone(),
        toast_last_warning_text: runtime_ui.toast.last_warning_text.clone(),
        text_input_open_count: runtime_ui.text_input.open_count,
        text_input_last_id: runtime_ui.text_input.last_id,
        text_input_last_title: runtime_ui.text_input.last_title.clone(),
        text_input_last_message: runtime_ui.text_input.last_message.clone(),
        text_input_last_default_text: runtime_ui.text_input.last_default_text.clone(),
        text_input_last_length: runtime_ui.text_input.last_length,
        text_input_last_numeric: runtime_ui.text_input.last_numeric,
        text_input_last_allow_empty: runtime_ui.text_input.last_allow_empty,
    })
}

pub fn build_runtime_menu_panel(hud: &HudModel) -> Option<RuntimeMenuPanelModel> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    Some(RuntimeMenuPanelModel {
        menu_open_count: runtime_ui.menu.menu_open_count,
        follow_up_menu_open_count: runtime_ui.menu.follow_up_menu_open_count,
        hide_follow_up_menu_count: runtime_ui.menu.hide_follow_up_menu_count,
        text_input_open_count: runtime_ui.text_input.open_count,
        text_input_last_id: runtime_ui.text_input.last_id,
        text_input_last_title: runtime_ui.text_input.last_title.clone(),
        text_input_last_default_text: runtime_ui.text_input.last_default_text.clone(),
        text_input_last_length: runtime_ui.text_input.last_length,
        text_input_last_numeric: runtime_ui.text_input.last_numeric,
        text_input_last_allow_empty: runtime_ui.text_input.last_allow_empty,
    })
}

pub fn build_runtime_dialog_panel(hud: &HudModel) -> Option<RuntimeDialogPanelModel> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    let prompt_kind = if runtime_ui.text_input.open_count > 0 {
        Some(RuntimeDialogPromptKind::TextInput)
    } else if runtime_ui.menu.follow_up_menu_open_count > runtime_ui.menu.hide_follow_up_menu_count
    {
        Some(RuntimeDialogPromptKind::FollowUpMenu)
    } else if runtime_ui.menu.menu_open_count > 0 {
        Some(RuntimeDialogPromptKind::Menu)
    } else {
        None
    };
    let notice_kind = if runtime_ui.toast.last_warning_text.is_some() {
        Some(RuntimeDialogNoticeKind::ToastWarning)
    } else if runtime_ui.toast.last_info_message.is_some() {
        Some(RuntimeDialogNoticeKind::ToastInfo)
    } else if runtime_ui.hud_text.last_reliable_message.is_some() {
        Some(RuntimeDialogNoticeKind::HudReliable)
    } else if runtime_ui.hud_text.last_message.is_some() {
        Some(RuntimeDialogNoticeKind::Hud)
    } else {
        None
    };
    let notice_text = match notice_kind {
        Some(RuntimeDialogNoticeKind::ToastWarning) => runtime_ui.toast.last_warning_text.clone(),
        Some(RuntimeDialogNoticeKind::ToastInfo) => runtime_ui.toast.last_info_message.clone(),
        Some(RuntimeDialogNoticeKind::HudReliable) => {
            runtime_ui.hud_text.last_reliable_message.clone()
        }
        Some(RuntimeDialogNoticeKind::Hud) => runtime_ui.hud_text.last_message.clone(),
        None => None,
    };

    Some(RuntimeDialogPanelModel {
        prompt_kind,
        prompt_active: runtime_ui.text_input.open_count > 0
            || runtime_ui.menu.follow_up_menu_open_count > runtime_ui.menu.hide_follow_up_menu_count,
        menu_open_count: runtime_ui.menu.menu_open_count,
        follow_up_menu_open_count: runtime_ui.menu.follow_up_menu_open_count,
        hide_follow_up_menu_count: runtime_ui.menu.hide_follow_up_menu_count,
        text_input_open_count: runtime_ui.text_input.open_count,
        text_input_last_id: runtime_ui.text_input.last_id,
        text_input_last_title: runtime_ui.text_input.last_title.clone(),
        text_input_last_message: runtime_ui.text_input.last_message.clone(),
        text_input_last_default_text: runtime_ui.text_input.last_default_text.clone(),
        text_input_last_length: runtime_ui.text_input.last_length,
        text_input_last_numeric: runtime_ui.text_input.last_numeric,
        text_input_last_allow_empty: runtime_ui.text_input.last_allow_empty,
        notice_kind,
        notice_text,
        notice_count: runtime_ui
            .hud_text
            .set_count
            .saturating_add(runtime_ui.hud_text.set_reliable_count)
            .saturating_add(runtime_ui.toast.info_count)
            .saturating_add(runtime_ui.toast.warning_count),
    })
}

pub fn build_runtime_command_mode_panel(hud: &HudModel) -> Option<RuntimeCommandModePanelModel> {
    let command_mode = &hud.runtime_ui.as_ref()?.command_mode;
    if !command_mode.active
        && command_mode.selected_units.is_empty()
        && command_mode.command_buildings.is_empty()
        && command_mode.command_rect.is_none()
        && command_mode.control_groups.is_empty()
        && command_mode.last_target.is_none()
        && command_mode.last_command_selection.is_none()
        && command_mode.last_stance_selection.is_none()
    {
        return None;
    }
    Some(RuntimeCommandModePanelModel {
        active: command_mode.active,
        selected_unit_count: command_mode.selected_units.len(),
        selected_unit_sample: command_mode
            .selected_units
            .iter()
            .copied()
            .take(3)
            .collect(),
        command_building_count: command_mode.command_buildings.len(),
        first_command_building: command_mode.command_buildings.first().copied(),
        command_rect: command_mode.command_rect,
        control_groups: command_mode
            .control_groups
            .iter()
            .map(|group| RuntimeCommandControlGroupPanelModel {
                index: group.index,
                unit_count: group.unit_ids.len(),
                first_unit_id: group.unit_ids.first().copied(),
            })
            .collect(),
        last_target: command_mode.last_target,
        last_command_selection: command_mode.last_command_selection,
        last_stance_selection: command_mode.last_stance_selection,
    })
}

pub fn build_runtime_admin_panel(hud: &HudModel) -> Option<RuntimeAdminPanelModel> {
    let admin = &hud.runtime_ui.as_ref()?.admin;
    Some(RuntimeAdminPanelModel {
        trace_info_count: admin.trace_info_count,
        trace_info_parse_fail_count: admin.trace_info_parse_fail_count,
        last_trace_info_player_id: admin.last_trace_info_player_id,
        debug_status_client_count: admin.debug_status_client_count,
        debug_status_client_parse_fail_count: admin.debug_status_client_parse_fail_count,
        debug_status_client_unreliable_count: admin.debug_status_client_unreliable_count,
        debug_status_client_unreliable_parse_fail_count: admin
            .debug_status_client_unreliable_parse_fail_count,
        last_debug_status_value: admin.last_debug_status_value,
        parse_fail_count: admin
            .trace_info_parse_fail_count
            .saturating_add(admin.debug_status_client_parse_fail_count)
            .saturating_add(admin.debug_status_client_unreliable_parse_fail_count),
    })
}

pub fn build_runtime_rules_panel(hud: &HudModel) -> Option<RuntimeRulesPanelModel> {
    let rules = &hud.runtime_ui.as_ref()?.rules;
    Some(RuntimeRulesPanelModel {
        mutation_count: rules
            .set_rules_count
            .saturating_add(rules.set_objectives_count)
            .saturating_add(rules.set_rule_count)
            .saturating_add(rules.clear_objectives_count)
            .saturating_add(rules.complete_objective_count),
        parse_fail_count: rules
            .set_rules_parse_fail_count
            .saturating_add(rules.set_objectives_parse_fail_count)
            .saturating_add(rules.set_rule_parse_fail_count),
        set_rules_count: rules.set_rules_count,
        set_objectives_count: rules.set_objectives_count,
        set_rule_count: rules.set_rule_count,
        clear_objectives_count: rules.clear_objectives_count,
        complete_objective_count: rules.complete_objective_count,
        waves: rules.waves,
        pvp: rules.pvp,
        objective_count: rules.objective_count,
        qualified_objective_count: rules.qualified_objective_count,
        objective_parent_edge_count: rules.objective_parent_edge_count,
        objective_flag_count: rules.objective_flag_count,
        complete_out_of_range_count: rules.complete_out_of_range_count,
        last_completed_index: rules.last_completed_index,
    })
}

pub fn build_runtime_world_label_panel(hud: &HudModel) -> Option<RuntimeWorldLabelPanelModel> {
    let world_labels = &hud.runtime_ui.as_ref()?.world_labels;
    Some(RuntimeWorldLabelPanelModel {
        label_count: world_labels.label_count,
        reliable_label_count: world_labels.reliable_label_count,
        remove_label_count: world_labels.remove_label_count,
        total_count: world_labels
            .label_count
            .saturating_add(world_labels.reliable_label_count)
            .saturating_add(world_labels.remove_label_count),
    })
}

pub fn build_runtime_session_panel(hud: &HudModel) -> Option<RuntimeSessionPanelModel> {
    let session = &hud.runtime_ui.as_ref()?.session;
    Some(RuntimeSessionPanelModel {
        kick: RuntimeKickPanelModel {
            reason_text: session.kick.reason_text.clone(),
            reason_ordinal: session.kick.reason_ordinal,
            hint_category: session.kick.hint_category.clone(),
            hint_text: session.kick.hint_text.clone(),
        },
        loading: RuntimeLoadingPanelModel {
            deferred_inbound_packet_count: session.loading.deferred_inbound_packet_count,
            replayed_inbound_packet_count: session.loading.replayed_inbound_packet_count,
            dropped_loading_low_priority_packet_count: session
                .loading
                .dropped_loading_low_priority_packet_count,
            dropped_loading_deferred_overflow_count: session
                .loading
                .dropped_loading_deferred_overflow_count,
            failed_state_snapshot_parse_count: session.loading.failed_state_snapshot_parse_count,
            failed_state_snapshot_core_data_parse_count: session
                .loading
                .failed_state_snapshot_core_data_parse_count,
            failed_entity_snapshot_parse_count: session.loading.failed_entity_snapshot_parse_count,
            ready_inbound_liveness_anchor_count: session
                .loading
                .ready_inbound_liveness_anchor_count,
            last_ready_inbound_liveness_anchor_at_ms: session
                .loading
                .last_ready_inbound_liveness_anchor_at_ms,
            timeout_count: session.loading.timeout_count,
            connect_or_loading_timeout_count: session.loading.connect_or_loading_timeout_count,
            ready_snapshot_timeout_count: session.loading.ready_snapshot_timeout_count,
            last_timeout_kind: session.loading.last_timeout_kind,
            last_timeout_idle_ms: session.loading.last_timeout_idle_ms,
            reset_count: session.loading.reset_count,
            reconnect_reset_count: session.loading.reconnect_reset_count,
            world_reload_count: session.loading.world_reload_count,
            kick_reset_count: session.loading.kick_reset_count,
            last_reset_kind: session.loading.last_reset_kind,
            last_world_reload: session
                .loading
                .last_world_reload
                .as_ref()
                .map(|world_reload| RuntimeWorldReloadPanelModel {
                    had_loaded_world: world_reload.had_loaded_world,
                    had_client_loaded: world_reload.had_client_loaded,
                    was_ready_to_enter_world: world_reload.was_ready_to_enter_world,
                    had_connect_confirm_sent: world_reload.had_connect_confirm_sent,
                    cleared_pending_packets: world_reload.cleared_pending_packets,
                    cleared_deferred_inbound_packets: world_reload.cleared_deferred_inbound_packets,
                    cleared_replayed_loading_events: world_reload.cleared_replayed_loading_events,
                }),
        },
        reconnect: RuntimeReconnectPanelModel {
            phase: session.reconnect.phase,
            phase_transition_count: session.reconnect.phase_transition_count,
            reason_kind: session.reconnect.reason_kind,
            reason_text: session.reconnect.reason_text.clone(),
            reason_ordinal: session.reconnect.reason_ordinal,
            hint_text: session.reconnect.hint_text.clone(),
            redirect_count: session.reconnect.redirect_count,
            last_redirect_ip: session.reconnect.last_redirect_ip.clone(),
            last_redirect_port: session.reconnect.last_redirect_port,
        },
    })
}

pub fn build_runtime_live_entity_panel(hud: &HudModel) -> Option<RuntimeLiveEntityPanelModel> {
    let entity = &hud.runtime_ui.as_ref()?.live.entity;
    Some(RuntimeLiveEntityPanelModel {
        entity_count: entity.entity_count,
        hidden_count: entity.hidden_count,
        local_entity_id: entity.local_entity_id,
        local_unit_kind: entity.local_unit_kind,
        local_unit_value: entity.local_unit_value,
        local_hidden: entity.local_hidden,
        local_last_seen_entity_snapshot_count: entity.local_last_seen_entity_snapshot_count,
        local_position: entity.local_position,
    })
}

pub fn build_runtime_live_effect_panel(hud: &HudModel) -> Option<RuntimeLiveEffectPanelModel> {
    let effect = &hud.runtime_ui.as_ref()?.live.effect;
    Some(RuntimeLiveEffectPanelModel {
        effect_count: effect.effect_count,
        spawn_effect_count: effect.spawn_effect_count,
        last_effect_id: effect.last_effect_id,
        last_spawn_effect_unit_type_id: effect.last_spawn_effect_unit_type_id,
        last_kind: effect.last_kind.clone(),
        last_contract_name: effect.last_contract_name.clone(),
        last_reliable_contract_name: effect.last_reliable_contract_name.clone(),
        last_position_hint: effect.last_position_hint,
        last_position_source: effect.last_position_source,
    })
}

fn semantic_count(scene: &RenderModel, kind: RenderObjectSemanticKind) -> usize {
    scene
        .objects
        .iter()
        .filter(|object| object.semantic_family() == kind.family())
        .count()
}

fn resolve_presenter_window(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    window: PresenterViewWindow,
) -> PresenterViewWindow {
    if window.width != 0 || window.height != 0 {
        return window;
    }

    scene
        .view_window
        .map(|view_window| PresenterViewWindow {
            origin_x: view_window.origin_x.min(map_width),
            origin_y: view_window.origin_y.min(map_height),
            width: view_window.width.min(map_width),
            height: view_window.height.min(map_height),
        })
        .unwrap_or(window)
}

#[cfg(test)]
mod tests {
    use super::{
        build_build_config_panel, build_build_interaction_panel, build_minimap_panel,
        build_runtime_admin_panel, build_runtime_command_mode_panel,
        build_runtime_dialog_panel, build_runtime_live_effect_panel,
        build_runtime_live_entity_panel, build_runtime_menu_panel, build_runtime_rules_panel,
        build_runtime_session_panel, build_runtime_ui_notice_panel,
        build_runtime_world_label_panel, BuildInteractionAuthorityState, BuildInteractionMode,
        BuildInteractionQueueState, BuildInteractionSelectionState, PresenterViewWindow,
        RuntimeDialogNoticeKind, RuntimeDialogPromptKind,
    };
    use crate::{
        hud_model::{
            RuntimeCommandControlGroupObservability, RuntimeCommandModeObservability,
            RuntimeCommandRectObservability, RuntimeCommandSelectionObservability,
            RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
            RuntimeCommandUnitRefObservability,
            HudSummary, RuntimeReconnectObservability, RuntimeReconnectPhaseObservability,
            RuntimeReconnectReasonKind, RuntimeSessionObservability, RuntimeSessionResetKind,
            RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
        },
        BuildConfigAuthoritySourceObservability, BuildConfigInspectorEntryObservability,
        BuildConfigOutcomeObservability, BuildConfigRollbackStripObservability,
        BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudModel,
        RenderModel, RenderObject, RuntimeAdminObservability, RuntimeHudTextObservability,
        RuntimeLiveSummaryObservability, RuntimeMenuObservability, RuntimeRulesObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
        RuntimeWorldLabelObservability, Viewport,
    };

    fn pack_point2(x: i32, y: i32) -> i32 {
        ((x & 0xffff) << 16) | (y & 0xffff)
    }

    #[test]
    fn builds_minimap_panel_from_summary_window_and_scene_semantics() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:focus".to_string(),
                    layer: 10,
                    x: 40.0,
                    y: 24.0,
                },
                RenderObject {
                    id: "marker:1".to_string(),
                    layer: 11,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "plan:2".to_string(),
                    layer: 12,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "block:3".to_string(),
                    layer: 13,
                    x: 16.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config:3:2:string".to_string(),
                    layer: 14,
                    x: 24.0,
                    y: 24.0,
                },
            ],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 3,
                marker_count: 4,
                map_width: 80,
                map_height: 60,
                overlay_visible: true,
                fog_enabled: true,
                visible_tile_count: 120,
                hidden_tile_count: 24,
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 2,
                origin_y: 1,
                width: 8,
                height: 7,
            },
        )
        .unwrap();

        assert_eq!(panel.map_width, 80);
        assert_eq!(panel.map_height, 60);
        assert_eq!(panel.window_last_x, 9);
        assert_eq!(panel.window_last_y, 7);
        assert_eq!(panel.window_tile_count, 56);
        assert_eq!(panel.window_coverage_percent, 1);
        assert_eq!(panel.map_tile_count, 4800);
        assert_eq!(panel.known_tile_count, 144);
        assert_eq!(panel.known_tile_percent, 3);
        assert_eq!(panel.unknown_tile_count, 4656);
        assert_eq!(panel.unknown_tile_percent, 97);
        assert_eq!(panel.focus_tile, Some((5, 3)));
        assert_eq!(panel.focus_in_window, Some(true));
        assert_eq!(panel.visible_known_percent, 83);
        assert_eq!(panel.hidden_known_percent, 16);
        assert_eq!(panel.tracked_object_count, 5);
        assert_eq!(panel.marker_count, 1);
        assert_eq!(panel.plan_count, 1);
        assert_eq!(panel.block_count, 1);
        assert_eq!(panel.runtime_count, 1);
        assert_eq!(panel.terrain_count, 0);
        assert_eq!(panel.unknown_count, 0);
    }

    #[test]
    fn builds_build_config_panel_with_capped_and_sorted_entries() {
        let hud = HudModel {
            build_ui: Some(BuildUiObservability {
                selected_block_id: Some(257),
                selected_rotation: 2,
                building: true,
                queued_count: 1,
                inflight_count: 2,
                finished_count: 3,
                removed_count: 4,
                orphan_authoritative_count: 5,
                head: Some(BuildQueueHeadObservability {
                    x: 10,
                    y: 11,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: BuildQueueHeadStage::InFlight,
                }),
                rollback_strip: BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 7,
                    rollback_count: 2,
                    last_build_tile: Some((23, 45)),
                    last_business_applied: true,
                    last_cleared_pending_local: true,
                    last_was_rollback: true,
                    last_pending_local_match: Some(false),
                    last_source: Some(BuildConfigAuthoritySourceObservability::ConstructFinish),
                    last_configured_outcome: Some(BuildConfigOutcomeObservability::Applied),
                    last_configured_block_name: Some("power-node".to_string()),
                },
                inspector_entries: vec![
                    BuildConfigInspectorEntryObservability {
                        family: "message".to_string(),
                        tracked_count: 1,
                        sample: "18:40:len=5:text=hello".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "power-node".to_string(),
                        tracked_count: 3,
                        sample: "23:45:links=24:46|25:47".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "battery".to_string(),
                        tracked_count: 1,
                        sample: "20:41:cap=120".to_string(),
                    },
                ],
            }),
            ..HudModel::default()
        };

        let panel = build_build_config_panel(&hud, 2).unwrap();
        assert_eq!(panel.selected_block_id, Some(257));
        assert_eq!(panel.pending_count, 3);
        assert_eq!(panel.tracked_family_count, 3);
        assert_eq!(panel.tracked_sample_count, 5);
        assert_eq!(panel.truncated_family_count, 1);
        assert_eq!(panel.selected_matches_head, Some(false));
        assert_eq!(
            panel.head.as_ref().map(|head| head.stage),
            Some(BuildQueueHeadStage::InFlight)
        );
        assert_eq!(panel.rollback_strip.applied_authoritative_count, 7);
        assert_eq!(panel.rollback_strip.rollback_count, 2);
        assert_eq!(panel.rollback_strip.last_build_tile, Some((23, 45)));
        assert_eq!(
            panel.rollback_strip.last_source,
            Some(BuildConfigAuthoritySourceObservability::ConstructFinish)
        );
        assert_eq!(
            panel.rollback_strip.last_configured_outcome,
            Some(BuildConfigOutcomeObservability::Applied)
        );
        assert_eq!(
            panel.rollback_strip.last_configured_block_name.as_deref(),
            Some("power-node")
        );
        assert_eq!(panel.entries.len(), 2);
        assert_eq!(panel.entries[0].family, "power-node");
        assert_eq!(panel.entries[1].family, "battery");
    }

    #[test]
    fn builds_build_interaction_panel_from_build_ui_observability() {
        let hud = HudModel {
            build_ui: Some(BuildUiObservability {
                selected_block_id: Some(257),
                selected_rotation: 2,
                building: true,
                queued_count: 1,
                inflight_count: 2,
                finished_count: 3,
                removed_count: 4,
                orphan_authoritative_count: 5,
                head: Some(BuildQueueHeadObservability {
                    x: 10,
                    y: 11,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: BuildQueueHeadStage::InFlight,
                }),
                rollback_strip: BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 7,
                    rollback_count: 2,
                    last_build_tile: Some((23, 45)),
                    last_business_applied: true,
                    last_cleared_pending_local: true,
                    last_was_rollback: true,
                    last_pending_local_match: Some(false),
                    last_source: Some(BuildConfigAuthoritySourceObservability::ConstructFinish),
                    last_configured_outcome: Some(BuildConfigOutcomeObservability::Applied),
                    last_configured_block_name: Some("power-node".to_string()),
                },
                inspector_entries: vec![
                    BuildConfigInspectorEntryObservability {
                        family: "message".to_string(),
                        tracked_count: 1,
                        sample: "18:40:len=5:text=hello".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "power-node".to_string(),
                        tracked_count: 1,
                        sample: "23:45:links=24:46|25:47".to_string(),
                    },
                ],
            }),
            ..HudModel::default()
        };

        let panel = build_build_interaction_panel(&hud).expect("expected build interaction panel");

        assert_eq!(panel.mode, BuildInteractionMode::Place);
        assert_eq!(
            panel.selection_state,
            BuildInteractionSelectionState::HeadDiverged
        );
        assert_eq!(panel.queue_state, BuildInteractionQueueState::Mixed);
        assert_eq!(panel.selected_block_id, Some(257));
        assert_eq!(panel.selected_rotation, 2);
        assert_eq!(panel.pending_count, 3);
        assert_eq!(panel.orphan_authoritative_count, 5);
        assert!(panel.place_ready);
        assert!(panel.config_available);
        assert_eq!(panel.config_family_count, 2);
        assert_eq!(panel.config_sample_count, 2);
        assert_eq!(panel.top_config_family.as_deref(), Some("message"));
        assert_eq!(
            panel.head.as_ref().map(|head| head.stage),
            Some(BuildQueueHeadStage::InFlight)
        );
        assert_eq!(
            panel.authority_state,
            BuildInteractionAuthorityState::Rollback
        );
        assert_eq!(panel.authority_pending_match, Some(false));
        assert_eq!(
            panel.authority_source,
            Some(BuildConfigAuthoritySourceObservability::ConstructFinish)
        );
        assert_eq!(panel.authority_tile, Some((23, 45)));
        assert_eq!(panel.authority_block_name.as_deref(), Some("power-node"));
    }

    #[test]
    fn builds_build_interaction_panel_for_break_head_without_selection() {
        let hud = HudModel {
            build_ui: Some(BuildUiObservability {
                selected_block_id: None,
                selected_rotation: 0,
                building: false,
                queued_count: 2,
                inflight_count: 0,
                finished_count: 0,
                removed_count: 0,
                orphan_authoritative_count: 0,
                head: Some(BuildQueueHeadObservability {
                    x: 7,
                    y: 8,
                    breaking: true,
                    block_id: None,
                    rotation: None,
                    stage: BuildQueueHeadStage::Queued,
                }),
                rollback_strip: BuildConfigRollbackStripObservability::default(),
                inspector_entries: Vec::new(),
            }),
            ..HudModel::default()
        };

        let panel = build_build_interaction_panel(&hud).expect("expected build interaction panel");

        assert_eq!(panel.mode, BuildInteractionMode::Break);
        assert_eq!(
            panel.selection_state,
            BuildInteractionSelectionState::BreakingHead
        );
        assert_eq!(panel.queue_state, BuildInteractionQueueState::Queued);
        assert!(!panel.place_ready);
        assert!(!panel.config_available);
        assert_eq!(panel.config_family_count, 0);
        assert_eq!(panel.authority_state, BuildInteractionAuthorityState::None);
    }

    #[test]
    fn builds_runtime_ui_notice_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability {
                    set_count: 9,
                    set_reliable_count: 10,
                    hide_count: 11,
                    last_message: Some("hud text".to_string()),
                    last_reliable_message: Some("hud rel".to_string()),
                },
                toast: RuntimeToastObservability {
                    info_count: 14,
                    warning_count: 15,
                    last_info_message: Some("toast".to_string()),
                    last_warning_text: Some("warn".to_string()),
                },
                text_input: RuntimeTextInputObservability {
                    open_count: 53,
                    last_id: Some(404),
                    last_title: Some("Digits".to_string()),
                    last_message: Some("Only numbers".to_string()),
                    last_default_text: Some("12345".to_string()),
                    last_length: Some(16),
                    last_numeric: Some(true),
                    last_allow_empty: Some(true),
                },
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_ui_notice_panel(&hud).expect("expected runtime ui notice panel");

        assert_eq!(panel.hud_set_count, 9);
        assert_eq!(panel.hud_set_reliable_count, 10);
        assert_eq!(panel.hud_hide_count, 11);
        assert_eq!(panel.hud_last_message.as_deref(), Some("hud text"));
        assert_eq!(panel.hud_last_reliable_message.as_deref(), Some("hud rel"));
        assert_eq!(panel.toast_info_count, 14);
        assert_eq!(panel.toast_warning_count, 15);
        assert_eq!(panel.toast_last_info_message.as_deref(), Some("toast"));
        assert_eq!(panel.toast_last_warning_text.as_deref(), Some("warn"));
        assert_eq!(panel.text_input_open_count, 53);
        assert_eq!(panel.text_input_last_id, Some(404));
        assert_eq!(panel.text_input_last_title.as_deref(), Some("Digits"));
        assert_eq!(
            panel.text_input_last_message.as_deref(),
            Some("Only numbers")
        );
        assert_eq!(panel.text_input_last_default_text.as_deref(), Some("12345"));
        assert_eq!(panel.text_input_last_length, Some(16));
        assert_eq!(panel.text_input_last_numeric, Some(true));
        assert_eq!(panel.text_input_last_allow_empty, Some(true));
    }

    #[test]
    fn builds_runtime_rules_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability {
                    set_rules_count: 67,
                    set_rules_parse_fail_count: 68,
                    set_objectives_count: 69,
                    set_objectives_parse_fail_count: 70,
                    set_rule_count: 71,
                    set_rule_parse_fail_count: 72,
                    clear_objectives_count: 73,
                    complete_objective_count: 74,
                    waves: Some(true),
                    pvp: Some(false),
                    objective_count: 2,
                    qualified_objective_count: 1,
                    objective_parent_edge_count: 1,
                    objective_flag_count: 2,
                    complete_out_of_range_count: 75,
                    last_completed_index: Some(9),
                },
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_rules_panel(&hud).expect("expected runtime rules panel");

        assert_eq!(panel.mutation_count, 354);
        assert_eq!(panel.parse_fail_count, 210);
        assert_eq!(panel.set_rules_count, 67);
        assert_eq!(panel.set_objectives_count, 69);
        assert_eq!(panel.set_rule_count, 71);
        assert_eq!(panel.clear_objectives_count, 73);
        assert_eq!(panel.complete_objective_count, 74);
        assert_eq!(panel.waves, Some(true));
        assert_eq!(panel.pvp, Some(false));
        assert_eq!(panel.objective_count, 2);
        assert_eq!(panel.qualified_objective_count, 1);
        assert_eq!(panel.objective_parent_edge_count, 1);
        assert_eq!(panel.objective_flag_count, 2);
        assert_eq!(panel.complete_out_of_range_count, 75);
        assert_eq!(panel.last_completed_index, Some(9));
    }

    #[test]
    fn builds_runtime_world_label_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability {
                    label_count: 19,
                    reliable_label_count: 20,
                    remove_label_count: 21,
                },
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel =
            build_runtime_world_label_panel(&hud).expect("expected runtime world-label panel");

        assert_eq!(panel.label_count, 19);
        assert_eq!(panel.reliable_label_count, 20);
        assert_eq!(panel.remove_label_count, 21);
        assert_eq!(panel.total_count, 60);
    }

    #[test]
    fn builds_runtime_live_entity_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability {
                        entity_count: 12,
                        hidden_count: 3,
                        local_entity_id: Some(404),
                        local_unit_kind: Some(2),
                        local_unit_value: Some(999),
                        local_hidden: Some(false),
                        local_last_seen_entity_snapshot_count: Some(7),
                        local_position: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 20.0f32.to_bits(),
                            y_bits: 33.0f32.to_bits(),
                        }),
                    },
                    effect: crate::RuntimeLiveEffectSummaryObservability::default(),
                },
            }),
            ..HudModel::default()
        };

        let panel =
            build_runtime_live_entity_panel(&hud).expect("expected runtime live entity panel");

        assert_eq!(panel.entity_count, 12);
        assert_eq!(panel.hidden_count, 3);
        assert_eq!(panel.local_entity_id, Some(404));
        assert_eq!(panel.local_unit_kind, Some(2));
        assert_eq!(panel.local_unit_value, Some(999));
        assert_eq!(panel.local_hidden, Some(false));
        assert_eq!(panel.local_last_seen_entity_snapshot_count, Some(7));
        assert_eq!(
            panel.local_position,
            Some(crate::RuntimeWorldPositionObservability {
                x_bits: 20.0f32.to_bits(),
                y_bits: 33.0f32.to_bits(),
            })
        );
    }

    #[test]
    fn builds_runtime_live_effect_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability::default(),
                    effect: crate::RuntimeLiveEffectSummaryObservability {
                        effect_count: 11,
                        spawn_effect_count: 73,
                        last_effect_id: Some(8),
                        last_spawn_effect_unit_type_id: Some(19),
                        last_kind: Some("Point2".to_string()),
                        last_contract_name: Some("position_target".to_string()),
                        last_reliable_contract_name: Some("unit_parent".to_string()),
                        last_position_hint: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 24.0f32.to_bits(),
                            y_bits: 32.0f32.to_bits(),
                        }),
                        last_position_source: Some(
                            crate::RuntimeLiveEffectPositionSource::BusinessProjection,
                        ),
                    },
                },
            }),
            ..HudModel::default()
        };

        let panel =
            build_runtime_live_effect_panel(&hud).expect("expected runtime live effect panel");

        assert_eq!(panel.effect_count, 11);
        assert_eq!(panel.spawn_effect_count, 73);
        assert_eq!(panel.last_effect_id, Some(8));
        assert_eq!(panel.last_spawn_effect_unit_type_id, Some(19));
        assert_eq!(panel.last_kind.as_deref(), Some("Point2"));
        assert_eq!(panel.last_contract_name.as_deref(), Some("position_target"));
        assert_eq!(
            panel.last_reliable_contract_name.as_deref(),
            Some("unit_parent")
        );
        assert_eq!(
            panel.last_position_hint,
            Some(crate::RuntimeWorldPositionObservability {
                x_bits: 24.0f32.to_bits(),
                y_bits: 32.0f32.to_bits(),
            })
        );
        assert_eq!(
            panel.last_position_source,
            Some(crate::RuntimeLiveEffectPositionSource::BusinessProjection)
        );
    }

    #[test]
    fn builds_runtime_menu_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability {
                    open_count: 53,
                    last_id: Some(404),
                    last_title: Some("Digits".to_string()),
                    last_message: Some("Only numbers".to_string()),
                    last_default_text: Some("12345".to_string()),
                    last_length: Some(16),
                    last_numeric: Some(true),
                    last_allow_empty: Some(true),
                },
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
                },
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_menu_panel(&hud).expect("expected runtime menu panel");

        assert_eq!(panel.menu_open_count, 16);
        assert_eq!(panel.follow_up_menu_open_count, 17);
        assert_eq!(panel.hide_follow_up_menu_count, 18);
        assert_eq!(panel.text_input_open_count, 53);
        assert_eq!(panel.text_input_last_id, Some(404));
        assert_eq!(panel.text_input_last_title.as_deref(), Some("Digits"));
        assert_eq!(panel.text_input_last_default_text.as_deref(), Some("12345"));
        assert_eq!(panel.text_input_last_length, Some(16));
        assert_eq!(panel.text_input_last_numeric, Some(true));
        assert_eq!(panel.text_input_last_allow_empty, Some(true));
    }

    #[test]
    fn builds_runtime_dialog_panel_prioritizes_text_input_and_warning_notice() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability {
                    set_count: 9,
                    set_reliable_count: 10,
                    hide_count: 11,
                    last_message: Some("hud text".to_string()),
                    last_reliable_message: Some("hud rel".to_string()),
                },
                toast: RuntimeToastObservability {
                    info_count: 14,
                    warning_count: 15,
                    last_info_message: Some("toast".to_string()),
                    last_warning_text: Some("warn".to_string()),
                },
                text_input: RuntimeTextInputObservability {
                    open_count: 53,
                    last_id: Some(404),
                    last_title: Some("Digits".to_string()),
                    last_message: Some("Only numbers".to_string()),
                    last_default_text: Some("12345".to_string()),
                    last_length: Some(16),
                    last_numeric: Some(true),
                    last_allow_empty: Some(true),
                },
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
                },
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_dialog_panel(&hud).expect("expected runtime dialog panel");

        assert_eq!(panel.prompt_kind, Some(RuntimeDialogPromptKind::TextInput));
        assert!(panel.prompt_active);
        assert_eq!(panel.menu_open_count, 16);
        assert_eq!(panel.follow_up_menu_open_count, 17);
        assert_eq!(panel.hide_follow_up_menu_count, 18);
        assert_eq!(panel.text_input_open_count, 53);
        assert_eq!(panel.text_input_last_id, Some(404));
        assert_eq!(panel.text_input_last_title.as_deref(), Some("Digits"));
        assert_eq!(panel.text_input_last_message.as_deref(), Some("Only numbers"));
        assert_eq!(panel.text_input_last_default_text.as_deref(), Some("12345"));
        assert_eq!(panel.text_input_last_length, Some(16));
        assert_eq!(panel.text_input_last_numeric, Some(true));
        assert_eq!(panel.text_input_last_allow_empty, Some(true));
        assert_eq!(panel.notice_kind, Some(RuntimeDialogNoticeKind::ToastWarning));
        assert_eq!(panel.notice_text.as_deref(), Some("warn"));
        assert_eq!(panel.notice_count, 48);
    }

    #[test]
    fn builds_runtime_command_mode_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability {
                    active: true,
                    selected_units: vec![11, 22, 33, 44],
                    command_buildings: vec![pack_point2(5, 6), pack_point2(-7, 8)],
                    command_rect: Some(RuntimeCommandRectObservability {
                        x0: -3,
                        y0: 4,
                        x1: 12,
                        y1: 18,
                    }),
                    control_groups: vec![
                        RuntimeCommandControlGroupObservability {
                            index: 2,
                            unit_ids: vec![11, 22, 33],
                        },
                        RuntimeCommandControlGroupObservability {
                            index: 4,
                            unit_ids: vec![99],
                        },
                    ],
                    last_target: Some(RuntimeCommandTargetObservability {
                        build_target: Some(pack_point2(9, 10)),
                        unit_target: Some(RuntimeCommandUnitRefObservability {
                            kind: 2,
                            value: 808,
                        }),
                        position_target: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 48.0f32.to_bits(),
                            y_bits: 96.0f32.to_bits(),
                        }),
                        rect_target: Some(RuntimeCommandRectObservability {
                            x0: 1,
                            y0: 2,
                            x1: 3,
                            y1: 4,
                        }),
                    }),
                    last_command_selection: Some(RuntimeCommandSelectionObservability {
                        command_id: Some(5),
                    }),
                    last_stance_selection: Some(RuntimeCommandStanceObservability {
                        stance_id: Some(7),
                        enabled: false,
                    }),
                },
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel =
            build_runtime_command_mode_panel(&hud).expect("expected runtime command-mode panel");

        assert!(panel.active);
        assert_eq!(panel.selected_unit_count, 4);
        assert_eq!(panel.selected_unit_sample, vec![11, 22, 33]);
        assert_eq!(panel.command_building_count, 2);
        assert_eq!(panel.first_command_building, Some(pack_point2(5, 6)));
        assert_eq!(
            panel.command_rect,
            Some(RuntimeCommandRectObservability {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            })
        );
        assert_eq!(panel.control_groups.len(), 2);
        assert_eq!(panel.control_groups[0].index, 2);
        assert_eq!(panel.control_groups[0].unit_count, 3);
        assert_eq!(panel.control_groups[0].first_unit_id, Some(11));
        assert_eq!(panel.control_groups[1].index, 4);
        assert_eq!(panel.control_groups[1].unit_count, 1);
        assert_eq!(panel.control_groups[1].first_unit_id, Some(99));
        assert_eq!(
            panel.last_target,
            Some(RuntimeCommandTargetObservability {
                build_target: Some(pack_point2(9, 10)),
                unit_target: Some(RuntimeCommandUnitRefObservability {
                    kind: 2,
                    value: 808,
                }),
                position_target: Some(crate::RuntimeWorldPositionObservability {
                    x_bits: 48.0f32.to_bits(),
                    y_bits: 96.0f32.to_bits(),
                }),
                rect_target: Some(RuntimeCommandRectObservability {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
            })
        );
        assert_eq!(
            panel.last_command_selection,
            Some(RuntimeCommandSelectionObservability {
                command_id: Some(5),
            })
        );
        assert_eq!(
            panel.last_stance_selection,
            Some(RuntimeCommandStanceObservability {
                stance_id: Some(7),
                enabled: false,
            })
        );
    }

    #[test]
    fn omits_runtime_command_mode_panel_for_empty_default_state() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        assert_eq!(build_runtime_command_mode_panel(&hud), None);
    }

    #[test]
    fn builds_runtime_admin_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability {
                    trace_info_count: 56,
                    trace_info_parse_fail_count: 76,
                    last_trace_info_player_id: Some(123456),
                    debug_status_client_count: 57,
                    debug_status_client_parse_fail_count: 77,
                    debug_status_client_unreliable_count: 58,
                    debug_status_client_unreliable_parse_fail_count: 78,
                    last_debug_status_value: Some(12),
                },
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_admin_panel(&hud).expect("expected runtime admin panel");

        assert_eq!(panel.trace_info_count, 56);
        assert_eq!(panel.trace_info_parse_fail_count, 76);
        assert_eq!(panel.last_trace_info_player_id, Some(123456));
        assert_eq!(panel.debug_status_client_count, 57);
        assert_eq!(panel.debug_status_client_parse_fail_count, 77);
        assert_eq!(panel.debug_status_client_unreliable_count, 58);
        assert_eq!(panel.debug_status_client_unreliable_parse_fail_count, 78);
        assert_eq!(panel.last_debug_status_value, Some(12));
        assert_eq!(panel.parse_fail_count, 231);
    }

    #[test]
    fn builds_runtime_session_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                session: RuntimeSessionObservability {
                    kick: crate::hud_model::RuntimeKickObservability {
                        reason_text: Some("idInUse".to_string()),
                        reason_ordinal: Some(7),
                        hint_category: Some("IdInUse".to_string()),
                        hint_text: Some("wait for old session".to_string()),
                    },
                    loading: crate::hud_model::RuntimeLoadingObservability {
                        deferred_inbound_packet_count: 5,
                        replayed_inbound_packet_count: 6,
                        dropped_loading_low_priority_packet_count: 7,
                        dropped_loading_deferred_overflow_count: 8,
                        failed_state_snapshot_parse_count: 9,
                        failed_state_snapshot_core_data_parse_count: 10,
                        failed_entity_snapshot_parse_count: 11,
                        ready_inbound_liveness_anchor_count: 12,
                        last_ready_inbound_liveness_anchor_at_ms: Some(1300),
                        timeout_count: 2,
                        connect_or_loading_timeout_count: 1,
                        ready_snapshot_timeout_count: 1,
                        last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
                        last_timeout_idle_ms: Some(20000),
                        reset_count: 3,
                        reconnect_reset_count: 1,
                        world_reload_count: 1,
                        kick_reset_count: 1,
                        last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
                        last_world_reload: Some(RuntimeWorldReloadObservability {
                            had_loaded_world: true,
                            had_client_loaded: false,
                            was_ready_to_enter_world: true,
                            had_connect_confirm_sent: false,
                            cleared_pending_packets: 4,
                            cleared_deferred_inbound_packets: 5,
                            cleared_replayed_loading_events: 6,
                        }),
                    },
                    reconnect: RuntimeReconnectObservability {
                        phase: RuntimeReconnectPhaseObservability::Attempting,
                        phase_transition_count: 3,
                        reason_kind: Some(RuntimeReconnectReasonKind::ConnectRedirect),
                        reason_text: Some("connectRedirect".to_string()),
                        reason_ordinal: None,
                        hint_text: Some("server requested redirect".to_string()),
                        redirect_count: 1,
                        last_redirect_ip: Some("127.0.0.1".to_string()),
                        last_redirect_port: Some(6567),
                    },
                },
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_session_panel(&hud).expect("expected runtime session panel");

        assert_eq!(panel.kick.reason_text.as_deref(), Some("idInUse"));
        assert_eq!(panel.kick.reason_ordinal, Some(7));
        assert_eq!(panel.kick.hint_category.as_deref(), Some("IdInUse"));
        assert_eq!(
            panel.loading.last_timeout_kind,
            Some(RuntimeSessionTimeoutKind::ReadySnapshotStall)
        );
        assert_eq!(panel.loading.last_timeout_idle_ms, Some(20000));
        assert_eq!(
            panel.loading.last_reset_kind,
            Some(RuntimeSessionResetKind::WorldReload)
        );
        assert_eq!(
            panel
                .loading
                .last_world_reload
                .as_ref()
                .map(|world_reload| world_reload.cleared_pending_packets),
            Some(4)
        );
        assert_eq!(
            panel.reconnect.phase,
            RuntimeReconnectPhaseObservability::Attempting
        );
        assert_eq!(panel.reconnect.phase_transition_count, 3);
        assert_eq!(
            panel.reconnect.reason_kind,
            Some(RuntimeReconnectReasonKind::ConnectRedirect)
        );
        assert_eq!(
            panel.reconnect.last_redirect_ip.as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(panel.reconnect.last_redirect_port, Some(6567));
    }
}
