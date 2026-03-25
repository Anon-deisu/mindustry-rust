/// UI/HUD-specific view-model data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HudModel {
    pub title: String,
    pub wave_text: Option<String>,
    pub status_text: String,
    pub overlay_summary_text: Option<String>,
    pub fps: Option<f32>,
    pub summary: Option<HudSummary>,
    pub runtime_ui: Option<RuntimeUiObservability>,
    pub build_ui: Option<BuildUiObservability>,
}

/// Structured HUD summary that mirrors core status fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudSummary {
    pub player_name: String,
    pub team_id: u8,
    pub selected_block: String,
    pub plan_count: usize,
    pub marker_count: usize,
    pub map_width: usize,
    pub map_height: usize,
    pub overlay_visible: bool,
    pub fog_enabled: bool,
    pub visible_tile_count: usize,
    pub hidden_tile_count: usize,
    pub minimap: HudMinimapSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudMinimapSummary {
    pub focus_tile: Option<(usize, usize)>,
    pub view_window: HudViewWindowSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudViewWindowSummary {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
}

/// Structured runtime UI observability projection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeUiObservability {
    pub hud_text: RuntimeHudTextObservability,
    pub toast: RuntimeToastObservability,
    pub text_input: RuntimeTextInputObservability,
    pub chat: RuntimeChatObservability,
    pub admin: RuntimeAdminObservability,
    pub menu: RuntimeMenuObservability,
    pub command_mode: RuntimeCommandModeObservability,
    pub rules: RuntimeRulesObservability,
    pub world_labels: RuntimeWorldLabelObservability,
    pub session: RuntimeSessionObservability,
    pub live: RuntimeLiveSummaryObservability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuntimeUiStackDepthSummary {
    pub prompt_depth: usize,
    pub notice_depth: usize,
    pub chat_depth: usize,
    pub active_group_count: usize,
    pub total_depth: usize,
}

impl RuntimeUiStackDepthSummary {
    pub(crate) fn is_empty(&self) -> bool {
        self.total_depth == 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeHudTextObservability {
    pub set_count: u64,
    pub set_reliable_count: u64,
    pub hide_count: u64,
    pub last_message: Option<String>,
    pub last_reliable_message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeToastObservability {
    pub info_count: u64,
    pub warning_count: u64,
    pub last_info_message: Option<String>,
    pub last_warning_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextInputObservability {
    pub open_count: u64,
    pub last_id: Option<i32>,
    pub last_title: Option<String>,
    pub last_message: Option<String>,
    pub last_default_text: Option<String>,
    pub last_length: Option<i32>,
    pub last_numeric: Option<bool>,
    pub last_allow_empty: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeChatObservability {
    pub server_message_count: u64,
    pub last_server_message: Option<String>,
    pub chat_message_count: u64,
    pub last_chat_message: Option<String>,
    pub last_chat_unformatted: Option<String>,
    pub last_chat_sender_entity_id: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeMenuObservability {
    pub menu_open_count: u64,
    pub follow_up_menu_open_count: u64,
    pub hide_follow_up_menu_count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCommandModeObservability {
    pub active: bool,
    pub selected_units: Vec<i32>,
    pub command_buildings: Vec<i32>,
    pub command_rect: Option<RuntimeCommandRectObservability>,
    pub control_groups: Vec<RuntimeCommandControlGroupObservability>,
    pub last_target: Option<RuntimeCommandTargetObservability>,
    pub last_command_selection: Option<RuntimeCommandSelectionObservability>,
    pub last_stance_selection: Option<RuntimeCommandStanceObservability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCommandRectObservability {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandControlGroupObservability {
    pub index: u8,
    pub unit_ids: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCommandUnitRefObservability {
    pub kind: u8,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCommandTargetObservability {
    pub build_target: Option<i32>,
    pub unit_target: Option<RuntimeCommandUnitRefObservability>,
    pub position_target: Option<RuntimeWorldPositionObservability>,
    pub rect_target: Option<RuntimeCommandRectObservability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCommandSelectionObservability {
    pub command_id: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCommandStanceObservability {
    pub stance_id: Option<u8>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeAdminObservability {
    pub trace_info_count: u64,
    pub trace_info_parse_fail_count: u64,
    pub last_trace_info_player_id: Option<i32>,
    pub debug_status_client_count: u64,
    pub debug_status_client_parse_fail_count: u64,
    pub debug_status_client_unreliable_count: u64,
    pub debug_status_client_unreliable_parse_fail_count: u64,
    pub last_debug_status_value: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeRulesObservability {
    pub set_rules_count: u64,
    pub set_rules_parse_fail_count: u64,
    pub set_objectives_count: u64,
    pub set_objectives_parse_fail_count: u64,
    pub set_rule_count: u64,
    pub set_rule_parse_fail_count: u64,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeWorldLabelObservability {
    pub label_count: u64,
    pub reliable_label_count: u64,
    pub remove_label_count: u64,
    pub active_count: usize,
    pub last_entity_id: Option<i32>,
    pub last_text: Option<String>,
    pub last_flags: Option<u8>,
    pub last_font_size_bits: Option<u32>,
    pub last_z_bits: Option<u32>,
    pub last_position: Option<RuntimeWorldPositionObservability>,
}

/// Structured session/runtime lifecycle summary for kick/loading/reconnect state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSessionObservability {
    pub kick: RuntimeKickObservability,
    pub loading: RuntimeLoadingObservability,
    pub reconnect: RuntimeReconnectObservability,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeKickObservability {
    pub reason_text: Option<String>,
    pub reason_ordinal: Option<i32>,
    pub hint_category: Option<String>,
    pub hint_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeLoadingObservability {
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
    pub last_world_reload: Option<RuntimeWorldReloadObservability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSessionTimeoutKind {
    ConnectOrLoading,
    ReadySnapshotStall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSessionResetKind {
    Reconnect,
    WorldReload,
    Kick,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeWorldReloadObservability {
    pub had_loaded_world: bool,
    pub had_client_loaded: bool,
    pub was_ready_to_enter_world: bool,
    pub had_connect_confirm_sent: bool,
    pub cleared_pending_packets: usize,
    pub cleared_deferred_inbound_packets: usize,
    pub cleared_replayed_loading_events: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeReconnectObservability {
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeReconnectPhaseObservability {
    #[default]
    Idle,
    Scheduled,
    Attempting,
    Succeeded,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeReconnectReasonKind {
    ConnectRedirect,
    Kick,
    Timeout,
    ManualConnect,
}

/// Structured live runtime summary built from session entity/effect observability.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeLiveSummaryObservability {
    pub entity: RuntimeLiveEntitySummaryObservability,
    pub effect: RuntimeLiveEffectSummaryObservability,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeLiveEntitySummaryObservability {
    pub entity_count: usize,
    pub hidden_count: usize,
    pub player_count: usize,
    pub unit_count: usize,
    pub last_entity_id: Option<i32>,
    pub last_player_entity_id: Option<i32>,
    pub last_unit_entity_id: Option<i32>,
    pub local_entity_id: Option<i32>,
    pub local_unit_kind: Option<u8>,
    pub local_unit_value: Option<u32>,
    pub local_hidden: Option<bool>,
    pub local_last_seen_entity_snapshot_count: Option<u64>,
    pub local_position: Option<RuntimeWorldPositionObservability>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeLiveEffectSummaryObservability {
    pub effect_count: u64,
    pub spawn_effect_count: u64,
    pub last_effect_id: Option<i16>,
    pub last_spawn_effect_unit_type_id: Option<i16>,
    pub last_kind: Option<String>,
    pub last_contract_name: Option<String>,
    pub last_reliable_contract_name: Option<String>,
    pub last_business_hint: Option<String>,
    pub last_position_hint: Option<RuntimeWorldPositionObservability>,
    pub last_position_source: Option<RuntimeLiveEffectPositionSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLiveEffectPositionSource {
    BusinessProjection,
    EffectPacket,
    SpawnEffectPacket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWorldPositionObservability {
    pub x_bits: u32,
    pub y_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildConfigAuthoritySourceObservability {
    TileConfig,
    ConstructFinish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildConfigOutcomeObservability {
    Applied,
    RejectedMissingBuilding,
    RejectedMissingBlockMetadata,
    RejectedUnsupportedBlock,
    RejectedUnsupportedConfigType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildConfigRollbackStripObservability {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildUiObservability {
    pub selected_block_id: Option<i16>,
    pub selected_rotation: i32,
    pub building: bool,
    pub queued_count: usize,
    pub inflight_count: usize,
    pub finished_count: u64,
    pub removed_count: u64,
    pub orphan_authoritative_count: u64,
    pub head: Option<BuildQueueHeadObservability>,
    pub rollback_strip: BuildConfigRollbackStripObservability,
    pub inspector_entries: Vec<BuildConfigInspectorEntryObservability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildQueueHeadObservability {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub stage: BuildQueueHeadStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildQueueHeadStage {
    Queued,
    InFlight,
    Finished,
    Removed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildConfigInspectorEntryObservability {
    pub family: String,
    pub tracked_count: usize,
    pub sample: String,
}

impl HudModel {
    pub fn hidden() -> Self {
        Self::default()
    }

    pub(crate) fn runtime_ui_stack_depth_summary(&self) -> Option<RuntimeUiStackDepthSummary> {
        let runtime_ui = self.runtime_ui.as_ref()?;
        let prompt_depth = usize::from(runtime_ui.text_input.open_count > 0)
            + usize::from(outstanding_follow_up_count(&runtime_ui.menu) > 0)
            + usize::from(runtime_ui.menu.menu_open_count > 0);
        let notice_depth = usize::from(runtime_ui.hud_text.last_message.is_some())
            + usize::from(runtime_ui.hud_text.last_reliable_message.is_some())
            + usize::from(runtime_ui.toast.last_info_message.is_some())
            + usize::from(runtime_ui.toast.last_warning_text.is_some());
        let chat_depth = usize::from(runtime_chat_active(&runtime_ui.chat));
        let active_group_count =
            usize::from(prompt_depth > 0) + usize::from(notice_depth > 0) + chat_depth;

        Some(RuntimeUiStackDepthSummary {
            prompt_depth,
            notice_depth,
            chat_depth,
            active_group_count,
            total_depth: prompt_depth + notice_depth + chat_depth,
        })
    }

    pub fn is_hidden(&self) -> bool {
        self.title.is_empty()
            && self.wave_text.is_none()
            && self.status_text.is_empty()
            && self.overlay_summary_text.is_none()
            && self.fps.is_none()
            && self.summary.is_none()
            && self.runtime_ui.is_none()
            && self.build_ui.is_none()
    }

    pub fn is_visible(&self) -> bool {
        !self.is_hidden()
    }
}

fn outstanding_follow_up_count(menu: &RuntimeMenuObservability) -> u64 {
    menu.follow_up_menu_open_count
        .saturating_sub(menu.hide_follow_up_menu_count)
}

fn runtime_chat_active(chat: &RuntimeChatObservability) -> bool {
    chat.server_message_count > 0
        || chat.last_server_message.is_some()
        || chat.chat_message_count > 0
        || chat.last_chat_message.is_some()
        || chat.last_chat_unformatted.is_some()
        || chat.last_chat_sender_entity_id.is_some()
}

#[cfg(test)]
mod tests {
    use super::{
        HudModel, RuntimeChatObservability, RuntimeHudTextObservability, RuntimeMenuObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
    };

    #[test]
    fn runtime_ui_stack_depth_summary_tracks_prompt_notice_and_chat_layers() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability {
                    last_message: Some("hud".to_string()),
                    last_reliable_message: Some("reliable".to_string()),
                    ..RuntimeHudTextObservability::default()
                },
                toast: RuntimeToastObservability {
                    last_info_message: Some("info".to_string()),
                    last_warning_text: Some("warn".to_string()),
                    ..RuntimeToastObservability::default()
                },
                text_input: RuntimeTextInputObservability {
                    open_count: 2,
                    ..RuntimeTextInputObservability::default()
                },
                chat: RuntimeChatObservability {
                    server_message_count: 1,
                    chat_message_count: 2,
                    ..RuntimeChatObservability::default()
                },
                menu: RuntimeMenuObservability {
                    menu_open_count: 1,
                    follow_up_menu_open_count: 3,
                    hide_follow_up_menu_count: 1,
                },
                ..RuntimeUiObservability::default()
            }),
            ..HudModel::default()
        };

        let summary = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui summary");
        assert_eq!(summary.prompt_depth, 3);
        assert_eq!(summary.notice_depth, 4);
        assert_eq!(summary.chat_depth, 1);
        assert_eq!(summary.active_group_count, 3);
        assert_eq!(summary.total_depth, 8);
        assert!(!summary.is_empty());
    }

    #[test]
    fn runtime_ui_stack_depth_summary_is_empty_for_default_runtime_ui() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability::default()),
            ..HudModel::default()
        };

        let summary = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui summary");
        assert_eq!(summary.prompt_depth, 0);
        assert_eq!(summary.notice_depth, 0);
        assert_eq!(summary.chat_depth, 0);
        assert_eq!(summary.active_group_count, 0);
        assert_eq!(summary.total_depth, 0);
        assert!(summary.is_empty());
    }
}
