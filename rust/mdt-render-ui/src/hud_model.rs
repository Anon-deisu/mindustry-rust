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
    pub markers: RuntimeMarkerObservability,
    pub session: RuntimeSessionObservability,
    pub live: RuntimeLiveSummaryObservability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeUiPromptLayerKind {
    Menu,
    FollowUpMenu,
    TextInput,
}

impl RuntimeUiPromptLayerKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::FollowUpMenu => "follow-up",
            Self::TextInput => "input",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeUiNoticeLayerKind {
    Hud,
    HudReliable,
    ToastInfo,
    ToastWarning,
}

impl RuntimeUiNoticeLayerKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Hud => "hud",
            Self::HudReliable => "reliable",
            Self::ToastInfo => "info",
            Self::ToastWarning => "warn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeUiStackForegroundSummaryKind {
    Menu,
    FollowUpMenu,
    TextInput,
    Chat,
}

impl RuntimeUiStackForegroundSummaryKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::FollowUpMenu => "follow-up",
            Self::TextInput => "input",
            Self::Chat => "chat",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RuntimeUiStackSummary {
    pub foreground_kind: Option<RuntimeUiStackForegroundSummaryKind>,
    pub prompt_kind: Option<RuntimeUiPromptLayerKind>,
    pub prompt_layers: Vec<RuntimeUiPromptLayerKind>,
    pub notice_kind: Option<RuntimeUiNoticeLayerKind>,
    pub notice_layers: Vec<RuntimeUiNoticeLayerKind>,
    pub chat_active: bool,
    pub menu_open_count: u64,
    pub outstanding_follow_up_count: u64,
    pub text_input_open_count: u64,
    pub text_input_last_id: Option<i32>,
    pub server_message_count: u64,
    pub chat_message_count: u64,
    pub last_chat_sender_entity_id: Option<i32>,
}

impl RuntimeUiStackSummary {
    pub(crate) fn is_empty(&self) -> bool {
        self.total_depth() == 0
            && self.foreground_kind.is_none()
            && self.text_input_last_id.is_none()
            && self.last_chat_sender_entity_id.is_none()
    }

    pub(crate) fn foreground_label(&self) -> &'static str {
        self.foreground_kind
            .map(RuntimeUiStackForegroundSummaryKind::label)
            .unwrap_or("none")
    }

    pub(crate) fn prompt_label(&self) -> &'static str {
        self.prompt_kind
            .map(RuntimeUiPromptLayerKind::label)
            .unwrap_or("none")
    }

    pub(crate) fn notice_label(&self) -> &'static str {
        self.notice_kind
            .map(RuntimeUiNoticeLayerKind::label)
            .unwrap_or("none")
    }

    pub(crate) fn prompt_layer_labels(&self) -> Vec<&'static str> {
        self.prompt_layers.iter().map(|kind| kind.label()).collect()
    }

    pub(crate) fn notice_layer_labels(&self) -> Vec<&'static str> {
        self.notice_layers.iter().map(|kind| kind.label()).collect()
    }

    pub(crate) fn prompt_depth(&self) -> usize {
        self.prompt_layers.len()
    }

    pub(crate) fn notice_depth(&self) -> usize {
        self.notice_layers.len()
    }

    pub(crate) fn chat_depth(&self) -> usize {
        usize::from(self.chat_active)
    }

    pub(crate) fn active_group_count(&self) -> usize {
        usize::from(self.prompt_depth() > 0)
            + usize::from(self.notice_depth() > 0)
            + self.chat_depth()
    }

    pub(crate) fn total_depth(&self) -> usize {
        self.prompt_depth() + self.notice_depth() + self.chat_depth()
    }
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
    pub announce_count: u64,
    pub last_announce_message: Option<String>,
    pub info_message_count: u64,
    pub last_info_message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeToastObservability {
    pub info_count: u64,
    pub warning_count: u64,
    pub last_info_message: Option<String>,
    pub last_warning_text: Option<String>,
    pub info_popup_count: u64,
    pub info_popup_reliable_count: u64,
    pub last_info_popup_reliable: Option<bool>,
    pub last_info_popup_id: Option<String>,
    pub last_info_popup_message: Option<String>,
    pub last_info_popup_duration_bits: Option<u32>,
    pub last_info_popup_align: Option<i32>,
    pub last_info_popup_top: Option<i32>,
    pub last_info_popup_left: Option<i32>,
    pub last_info_popup_bottom: Option<i32>,
    pub last_info_popup_right: Option<i32>,
    pub clipboard_count: u64,
    pub last_clipboard_text: Option<String>,
    pub open_uri_count: u64,
    pub last_open_uri: Option<String>,
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
    pub last_menu_open_id: Option<i32>,
    pub last_menu_open_title: Option<String>,
    pub last_menu_open_message: Option<String>,
    pub last_menu_open_option_rows: usize,
    pub last_menu_open_first_row_len: usize,
    pub last_follow_up_menu_open_id: Option<i32>,
    pub last_follow_up_menu_open_title: Option<String>,
    pub last_follow_up_menu_open_message: Option<String>,
    pub last_follow_up_menu_open_option_rows: usize,
    pub last_follow_up_menu_open_first_row_len: usize,
    pub last_hide_follow_up_menu_id: Option<i32>,
    pub menu_choose_count: u64,
    pub last_menu_choose_menu_id: Option<i32>,
    pub last_menu_choose_option: Option<i32>,
    pub text_input_result_count: u64,
    pub last_text_input_result_id: Option<i32>,
    pub last_text_input_result_text: Option<String>,
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
    pub inactive_count: usize,
    pub last_entity_id: Option<i32>,
    pub last_text: Option<String>,
    pub last_flags: Option<u8>,
    pub last_font_size_bits: Option<u32>,
    pub last_z_bits: Option<u32>,
    pub last_position: Option<RuntimeWorldPositionObservability>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeMarkerObservability {
    pub create_count: u64,
    pub remove_count: u64,
    pub update_count: u64,
    pub update_text_count: u64,
    pub update_texture_count: u64,
    pub decode_fail_count: u64,
    pub last_marker_id: Option<i32>,
    pub last_control_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCoreBindingKindObservability {
    FirstCorePerTeamApproximation,
}

impl RuntimeCoreBindingKindObservability {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::FirstCorePerTeamApproximation => "first-core-per-team",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCoreBindingObservability {
    pub kind: Option<RuntimeCoreBindingKindObservability>,
    pub ambiguous_team_count: usize,
    pub ambiguous_team_sample: Vec<u8>,
    pub missing_team_count: usize,
    pub missing_team_sample: Vec<u8>,
}

/// Structured session/runtime lifecycle summary for kick/loading/reconnect state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSessionObservability {
    pub core_binding: RuntimeCoreBindingObservability,
    pub resource_delta: RuntimeResourceDeltaObservability,
    pub kick: RuntimeKickObservability,
    pub loading: RuntimeLoadingObservability,
    pub reconnect: RuntimeReconnectObservability,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeResourceDeltaObservability {
    pub remove_tile_count: u64,
    pub set_tile_count: u64,
    pub set_floor_count: u64,
    pub set_overlay_count: u64,
    pub set_item_count: u64,
    pub set_items_count: u64,
    pub set_liquid_count: u64,
    pub set_liquids_count: u64,
    pub clear_items_count: u64,
    pub clear_liquids_count: u64,
    pub set_tile_items_count: u64,
    pub set_tile_liquids_count: u64,
    pub take_items_count: u64,
    pub transfer_item_to_count: u64,
    pub transfer_item_to_unit_count: u64,
    pub last_kind: Option<String>,
    pub last_item_id: Option<i16>,
    pub last_amount: Option<i32>,
    pub last_build_pos: Option<i32>,
    pub last_unit: Option<RuntimeCommandUnitRefObservability>,
    pub last_to_entity_id: Option<i32>,
    pub build_count: usize,
    pub build_stack_count: usize,
    pub entity_count: usize,
    pub authoritative_build_update_count: u64,
    pub delta_apply_count: u64,
    pub delta_skip_count: u64,
    pub delta_conflict_count: u64,
    pub last_changed_build_pos: Option<i32>,
    pub last_changed_entity_id: Option<i32>,
    pub last_changed_item_id: Option<i16>,
    pub last_changed_amount: Option<i32>,
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
    pub active_overlay_count: usize,
    pub active_effect_id: Option<i16>,
    pub active_contract_name: Option<String>,
    pub active_reliable: Option<bool>,
    pub active_position: Option<RuntimeWorldPositionObservability>,
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
    ActiveOverlay,
    BusinessProjection,
    EffectPacket,
    SpawnEffectPacket,
}

impl RuntimeLiveEffectSummaryObservability {
    pub fn display_effect_id(&self) -> Option<i16> {
        self.active_effect_id.or(self.last_effect_id)
    }

    pub fn display_contract_name(&self) -> Option<&str> {
        self.active_contract_name
            .as_deref()
            .or(self.last_contract_name.as_deref())
    }

    pub fn display_reliable_contract_name(&self) -> Option<&str> {
        if self.active_reliable == Some(true) {
            self.active_contract_name.as_deref()
        } else {
            self.last_reliable_contract_name.as_deref()
        }
    }

    pub fn display_position_source(&self) -> Option<RuntimeLiveEffectPositionSource> {
        if self.active_position.is_some() {
            Some(RuntimeLiveEffectPositionSource::ActiveOverlay)
        } else {
            self.last_position_source
        }
    }

    pub fn display_position(&self) -> Option<&RuntimeWorldPositionObservability> {
        self.active_position
            .as_ref()
            .or(self.last_position_hint.as_ref())
    }
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
        let summary = self.runtime_ui_stack_summary()?;

        Some(RuntimeUiStackDepthSummary {
            prompt_depth: summary.prompt_depth(),
            notice_depth: summary.notice_depth(),
            chat_depth: summary.chat_depth(),
            active_group_count: summary.active_group_count(),
            total_depth: summary.total_depth(),
        })
    }

    pub(crate) fn runtime_ui_stack_summary(&self) -> Option<RuntimeUiStackSummary> {
        let runtime_ui = self.runtime_ui.as_ref()?;
        let prompt_layers = runtime_prompt_layers(runtime_ui);
        let notice_layers = runtime_notice_layers(runtime_ui);
        let prompt_kind = prompt_layers.first().copied();
        let notice_kind = notice_layers.last().copied();
        let chat_active = runtime_chat_active(&runtime_ui.chat);
        let foreground_kind = match prompt_kind {
            Some(RuntimeUiPromptLayerKind::TextInput) => {
                Some(RuntimeUiStackForegroundSummaryKind::TextInput)
            }
            Some(RuntimeUiPromptLayerKind::FollowUpMenu) => {
                Some(RuntimeUiStackForegroundSummaryKind::FollowUpMenu)
            }
            Some(RuntimeUiPromptLayerKind::Menu) => Some(RuntimeUiStackForegroundSummaryKind::Menu),
            None if chat_active => Some(RuntimeUiStackForegroundSummaryKind::Chat),
            None => None,
        };

        Some(RuntimeUiStackSummary {
            foreground_kind,
            prompt_kind,
            prompt_layers,
            notice_kind,
            notice_layers,
            chat_active,
            menu_open_count: runtime_ui.menu.menu_open_count,
            outstanding_follow_up_count: outstanding_follow_up_count(&runtime_ui.menu),
            text_input_open_count: runtime_ui.text_input.open_count,
            text_input_last_id: runtime_ui.text_input.last_id,
            server_message_count: runtime_ui.chat.server_message_count,
            chat_message_count: runtime_ui.chat.chat_message_count,
            last_chat_sender_entity_id: runtime_ui.chat.last_chat_sender_entity_id,
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

pub(crate) fn runtime_menu_prompt_active(menu: &RuntimeMenuObservability) -> bool {
    let count_active = menu.menu_open_count.saturating_sub(menu.menu_choose_count) > 0;
    let id_active = menu
        .last_menu_open_id
        .is_some_and(|menu_id| menu.last_menu_choose_menu_id != Some(menu_id));

    count_active || id_active
}

pub(crate) fn runtime_text_input_prompt_active(runtime_ui: &RuntimeUiObservability) -> bool {
    let count_active = runtime_ui
        .text_input
        .open_count
        .saturating_sub(runtime_ui.menu.text_input_result_count)
        > 0;
    let id_active = runtime_ui
        .text_input
        .last_id
        .is_some_and(|text_input_id| runtime_ui.menu.last_text_input_result_id != Some(text_input_id));

    count_active || id_active
}

fn runtime_prompt_layers(runtime_ui: &RuntimeUiObservability) -> Vec<RuntimeUiPromptLayerKind> {
    let mut layers = Vec::new();
    if runtime_text_input_prompt_active(runtime_ui) {
        layers.push(RuntimeUiPromptLayerKind::TextInput);
    }
    if outstanding_follow_up_count(&runtime_ui.menu) > 0 {
        layers.push(RuntimeUiPromptLayerKind::FollowUpMenu);
    }
    if runtime_menu_prompt_active(&runtime_ui.menu) {
        layers.push(RuntimeUiPromptLayerKind::Menu);
    }
    layers
}

fn runtime_notice_layers(runtime_ui: &RuntimeUiObservability) -> Vec<RuntimeUiNoticeLayerKind> {
    let mut layers = Vec::new();
    if runtime_ui.hud_text.last_message.is_some() {
        layers.push(RuntimeUiNoticeLayerKind::Hud);
    }
    if runtime_ui.hud_text.last_reliable_message.is_some() {
        layers.push(RuntimeUiNoticeLayerKind::HudReliable);
    }
    if runtime_ui.toast.last_info_message.is_some() {
        layers.push(RuntimeUiNoticeLayerKind::ToastInfo);
    }
    if runtime_ui.toast.last_warning_text.is_some() {
        layers.push(RuntimeUiNoticeLayerKind::ToastWarning);
    }
    layers
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
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiNoticeLayerKind,
        RuntimeUiObservability, RuntimeUiPromptLayerKind, RuntimeUiStackForegroundSummaryKind,
    };

    #[test]
    fn runtime_ui_stack_summary_tracks_foreground_and_layer_order() {
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
                    last_id: Some(404),
                    ..RuntimeTextInputObservability::default()
                },
                chat: RuntimeChatObservability {
                    server_message_count: 1,
                    chat_message_count: 2,
                    last_chat_sender_entity_id: Some(77),
                    ..RuntimeChatObservability::default()
                },
                menu: RuntimeMenuObservability {
                    menu_open_count: 1,
                    follow_up_menu_open_count: 3,
                    hide_follow_up_menu_count: 1,
                    ..RuntimeMenuObservability::default()
                },
                ..RuntimeUiObservability::default()
            }),
            ..HudModel::default()
        };

        let summary = hud
            .runtime_ui_stack_summary()
            .expect("runtime ui stack summary");
        assert_eq!(
            summary.foreground_kind,
            Some(RuntimeUiStackForegroundSummaryKind::TextInput)
        );
        assert_eq!(
            summary.prompt_kind,
            Some(RuntimeUiPromptLayerKind::TextInput)
        );
        assert_eq!(
            summary.prompt_layers,
            vec![
                RuntimeUiPromptLayerKind::TextInput,
                RuntimeUiPromptLayerKind::FollowUpMenu,
                RuntimeUiPromptLayerKind::Menu,
            ]
        );
        assert_eq!(
            summary.notice_kind,
            Some(RuntimeUiNoticeLayerKind::ToastWarning)
        );
        assert_eq!(
            summary.notice_layers,
            vec![
                RuntimeUiNoticeLayerKind::Hud,
                RuntimeUiNoticeLayerKind::HudReliable,
                RuntimeUiNoticeLayerKind::ToastInfo,
                RuntimeUiNoticeLayerKind::ToastWarning,
            ]
        );
        assert_eq!(
            summary.prompt_layer_labels(),
            vec!["input", "follow-up", "menu"]
        );
        assert_eq!(
            summary.notice_layer_labels(),
            vec!["hud", "reliable", "info", "warn"]
        );
        assert_eq!(summary.foreground_label(), "input");
        assert_eq!(summary.prompt_label(), "input");
        assert_eq!(summary.notice_label(), "warn");
        assert_eq!(summary.outstanding_follow_up_count, 2);
        assert_eq!(summary.text_input_last_id, Some(404));
        assert_eq!(summary.last_chat_sender_entity_id, Some(77));
        assert_eq!(summary.prompt_depth(), 3);
        assert_eq!(summary.notice_depth(), 4);
        assert_eq!(summary.chat_depth(), 1);
        assert_eq!(summary.active_group_count(), 3);
        assert_eq!(summary.total_depth(), 8);
        assert!(!summary.is_empty());
    }

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
                    ..RuntimeMenuObservability::default()
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

    #[test]
    fn runtime_ui_stack_summary_drops_completed_prompt_layers_from_foreground() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                text_input: RuntimeTextInputObservability {
                    open_count: 1,
                    last_id: Some(404),
                    ..RuntimeTextInputObservability::default()
                },
                menu: RuntimeMenuObservability {
                    menu_open_count: 1,
                    last_menu_open_id: Some(11),
                    menu_choose_count: 1,
                    last_menu_choose_menu_id: Some(11),
                    text_input_result_count: 1,
                    last_text_input_result_id: Some(404),
                    ..RuntimeMenuObservability::default()
                },
                ..RuntimeUiObservability::default()
            }),
            ..HudModel::default()
        };

        let summary = hud
            .runtime_ui_stack_summary()
            .expect("runtime ui stack summary");
        assert_eq!(summary.foreground_kind, None);
        assert_eq!(summary.prompt_kind, None);
        assert!(summary.prompt_layers.is_empty());
        assert_eq!(summary.prompt_depth(), 0);
        assert_eq!(summary.total_depth(), 0);

        let depth = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui depth summary");
        assert!(depth.is_empty());
    }
}
