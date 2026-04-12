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

impl HudSummary {
    pub fn map_tile_count(&self) -> usize {
        self.map_width.saturating_mul(self.map_height)
    }

    pub fn known_tile_count(&self) -> usize {
        self.visible_tile_count
            .saturating_add(self.hidden_tile_count)
    }

    pub fn unknown_tile_count(&self) -> usize {
        self.map_tile_count()
            .saturating_sub(self.known_tile_count())
    }

    pub fn known_tile_percent(&self) -> usize {
        percent_of(self.known_tile_count(), self.map_tile_count())
    }

    pub fn unknown_tile_percent(&self) -> usize {
        percent_of(self.unknown_tile_count(), self.map_tile_count())
    }

    pub fn visible_map_percent(&self) -> usize {
        percent_of(self.visible_tile_count, self.map_tile_count())
    }

    pub fn hidden_map_percent(&self) -> usize {
        percent_of(self.hidden_tile_count, self.map_tile_count())
    }

    pub fn visibility_label(&self) -> &'static str {
        if self.map_tile_count() == 0 {
            "empty"
        } else if self.known_tile_count() == 0 {
            "unseen"
        } else if self.visible_tile_count == 0 {
            "hidden"
        } else if self.unknown_tile_count() == 0 && self.hidden_tile_count == 0 {
            "clear"
        } else if self.unknown_tile_count() == 0 {
            "mapped"
        } else {
            "mixed"
        }
    }

    pub fn overlay_label(&self) -> &'static str {
        if self.overlay_visible {
            "on"
        } else {
            "off"
        }
    }

    pub fn fog_label(&self) -> &'static str {
        if self.fog_enabled {
            "on"
        } else {
            "off"
        }
    }

    pub fn summary_label(&self) -> String {
        format!(
            "team={} block={} plans={} markers={} vis={} known={} visible={} overlay={} fog={} minimap={}",
            self.team_id,
            self.selected_block,
            self.plan_count,
            self.marker_count,
            self.visibility_label(),
            self.known_tile_percent(),
            self.visible_map_percent(),
            self.overlay_label(),
            self.fog_label(),
            self.minimap.summary_label(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "player={} team={} block={} plans={} markers={} map={}x{} tiles={} vis={} known={} unknown={} visible={} hidden={} overlay={} fog={} minimap={}",
            self.player_name,
            self.team_id,
            self.selected_block,
            self.plan_count,
            self.marker_count,
            self.map_width,
            self.map_height,
            self.map_tile_count(),
            self.visibility_label(),
            self.known_tile_percent(),
            self.unknown_tile_percent(),
            self.visible_map_percent(),
            self.hidden_map_percent(),
            self.overlay_label(),
            self.fog_label(),
            self.minimap.detail_label(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudMinimapSummary {
    pub focus_tile: Option<(usize, usize)>,
    pub view_window: HudViewWindowSummary,
}

impl HudMinimapSummary {
    pub fn focus_tile_label(&self) -> String {
        self.focus_tile
            .map(|(x, y)| format!("{x}:{y}"))
            .unwrap_or_else(|| "none".to_string())
    }

    pub fn summary_label(&self) -> String {
        format!(
            "focus={} window={}+{}",
            self.focus_tile_label(),
            self.view_window.origin_label(),
            self.view_window.size_label(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "focus={} window-origin={} window-size={} window-area={}",
            self.focus_tile_label(),
            self.view_window.origin_label(),
            self.view_window.size_label(),
            self.view_window.tile_count(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudViewWindowSummary {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
}

impl HudViewWindowSummary {
    pub fn tile_count(&self) -> usize {
        self.width.saturating_mul(self.height)
    }

    pub fn origin_label(&self) -> String {
        format!("{}:{}", self.origin_x, self.origin_y)
    }

    pub fn size_label(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    pub fn summary_label(&self) -> String {
        format!("origin={} size={}", self.origin_label(), self.size_label(),)
    }

    pub fn detail_label(&self) -> String {
        format!(
            "origin={} size={} area={}",
            self.origin_label(),
            self.size_label(),
            self.tile_count(),
        )
    }
}

fn percent_of(part: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        part.saturating_mul(100) / total
    }
}

fn optional_i32_label(value: Option<i32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_u8_label(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_i16_label(value: Option<i16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_u32_label(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_usize_label(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_u64_label(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "none",
    }
}

fn world_position_text(value: Option<&RuntimeWorldPositionObservability>) -> String {
    value
        .map(|value| {
            format!(
                "{:.1}:{:.1}",
                f32::from_bits(value.x_bits),
                f32::from_bits(value.y_bits)
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn compact_sha_label(value: Option<&str>) -> String {
    value
        .map(|value| value.chars().take(12).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_string())
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
            && self.menu_open_count == 0
            && self.outstanding_follow_up_count == 0
            && self.text_input_open_count == 0
            && self.server_message_count == 0
            && self.chat_message_count == 0
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn summary_label(&self) -> String {
        format!(
            "fg={} prompt={} depth={} notice={} depth={} chat={} groups={}",
            self.foreground_label(),
            self.prompt_label(),
            self.prompt_depth(),
            self.notice_label(),
            self.notice_depth(),
            if self.chat_active { "on" } else { "off" },
            self.active_group_count(),
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn detail_label(&self) -> String {
        format!(
            "fg={} prompt={} layers=[{}] notice={} layers=[{}] chat={} groups={} depth={} menu={} hud={} dialog={} text-input={} server-msg={} chat-msg={} chat-sender={}",
            self.foreground_label(),
            self.prompt_label(),
            self.prompt_layer_labels().join(","),
            self.notice_label(),
            self.notice_layer_labels().join(","),
            if self.chat_active { "on" } else { "off" },
            self.active_group_count(),
            self.total_depth(),
            self.menu_depth(),
            self.hud_depth(),
            self.dialog_depth(),
            self.text_input_open_count,
            self.server_message_count,
            self.chat_message_count,
            self.last_chat_sender_entity_id
                .map(|entity_id| entity_id.to_string())
                .unwrap_or_else(|| "none".to_string()),
        )
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

    pub(crate) fn menu_depth(&self) -> usize {
        self.prompt_depth()
    }

    pub(crate) fn hud_depth(&self) -> usize {
        self.notice_depth()
    }

    pub(crate) fn dialog_depth(&self) -> usize {
        self.total_depth()
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

    pub(crate) fn menu_depth(&self) -> usize {
        self.prompt_depth
    }

    pub(crate) fn hud_depth(&self) -> usize {
        self.notice_depth
    }

    pub(crate) fn dialog_depth(&self) -> usize {
        self.total_depth
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

/// Structured bootstrap summary for world bootstrap rules/tags/locales and team hints.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBootstrapObservability {
    pub rules_label: String,
    pub tags_label: String,
    pub locales_label: String,
    pub team_count: usize,
    pub marker_count: usize,
    pub custom_chunk_count: usize,
    pub content_patch_count: usize,
    pub player_team_plan_count: usize,
    pub static_fog_team_count: usize,
}

impl RuntimeBootstrapObservability {
    pub fn is_empty(&self) -> bool {
        self.rules_label.is_empty()
            && self.tags_label.is_empty()
            && self.locales_label.is_empty()
            && self.team_count == 0
            && self.marker_count == 0
            && self.custom_chunk_count == 0
            && self.content_patch_count == 0
            && self.player_team_plan_count == 0
            && self.static_fog_team_count == 0
    }

    pub fn summary_label(&self) -> String {
        format!(
            "rules={}:tags={}:locales={}:teams={}:markers={}:chunks={}:patches={}:plans={}:fog={}",
            self.rules_label,
            self.tags_label,
            self.locales_label,
            self.team_count,
            self.marker_count,
            self.custom_chunk_count,
            self.content_patch_count,
            self.player_team_plan_count,
            self.static_fog_team_count,
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "rules-label={}:tags-label={}:locales-label={}:team-count={}:marker-count={}:custom-chunk-count={}:content-patch-count={}:player-team-plan-count={}:static-fog-team-count={}",
            self.rules_label,
            self.tags_label,
            self.locales_label,
            self.team_count,
            self.marker_count,
            self.custom_chunk_count,
            self.content_patch_count,
            self.player_team_plan_count,
            self.static_fog_team_count,
        )
    }
}

/// Structured session/runtime lifecycle summary for kick/loading/reconnect state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSessionObservability {
    pub bootstrap: RuntimeBootstrapObservability,
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
    pub local_owned_unit_entity_id: Option<i32>,
    pub local_owned_unit_payload_count: Option<i32>,
    pub local_owned_unit_payload_class_id: Option<u8>,
    pub local_owned_unit_payload_revision: Option<i16>,
    pub local_owned_unit_payload_body_len: Option<usize>,
    pub local_owned_unit_payload_sha256: Option<String>,
    pub local_owned_unit_payload_nested_descendant_count: Option<usize>,
    pub local_owned_carried_item_id: Option<i16>,
    pub local_owned_carried_item_amount: Option<i32>,
    pub local_owned_controller_type: Option<u8>,
    pub local_owned_controller_value: Option<i32>,
}

impl RuntimeLiveEntitySummaryObservability {
    pub fn local_owned_unit_payload_label(&self) -> String {
        format!(
            "payload=count={}:unit={}/r{}/l{}:s{}",
            optional_i32_label(self.local_owned_unit_payload_count),
            optional_u8_label(self.local_owned_unit_payload_class_id),
            optional_i16_label(self.local_owned_unit_payload_revision),
            optional_usize_label(self.local_owned_unit_payload_body_len),
            compact_sha_label(self.local_owned_unit_payload_sha256.as_deref()),
        )
    }

    pub fn local_owned_unit_nested_label(&self) -> String {
        format!(
            "nested={}",
            optional_usize_label(self.local_owned_unit_payload_nested_descendant_count),
        )
    }

    pub fn local_owned_unit_stack_label(&self) -> String {
        match (
            self.local_owned_carried_item_id,
            self.local_owned_carried_item_amount,
        ) {
            (None, None) => "stack=none".to_string(),
            (item_id, amount) => format!(
                "stack={}x{}",
                optional_i16_label(item_id),
                optional_i32_label(amount),
            ),
        }
    }

    pub fn local_owned_unit_controller_label(&self) -> String {
        format!(
            "controller={}/{}",
            optional_u8_label(self.local_owned_controller_type),
            optional_i32_label(self.local_owned_controller_value),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "local={} unit={}/{} pos={} hidden={} seen={} players={} units={} last={}/{}/{} owned={} {} {} {} {}",
            optional_i32_label(self.local_entity_id),
            optional_u8_label(self.local_unit_kind),
            optional_u32_label(self.local_unit_value),
            world_position_text(self.local_position.as_ref()),
            optional_bool_label(self.local_hidden),
            optional_u64_label(self.local_last_seen_entity_snapshot_count),
            self.player_count,
            self.unit_count,
            optional_i32_label(self.last_entity_id),
            optional_i32_label(self.last_player_entity_id),
            optional_i32_label(self.last_unit_entity_id),
            optional_i32_label(self.local_owned_unit_entity_id),
            self.local_owned_unit_payload_label(),
            self.local_owned_unit_nested_label(),
            self.local_owned_unit_stack_label(),
            self.local_owned_unit_controller_label(),
        )
    }
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
    pub active_overlay_remaining_ticks: Option<u8>,
    pub active_overlay_lifetime_ticks: Option<u8>,
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

    pub fn display_overlay_ttl(&self) -> Option<(u8, u8)> {
        match (
            self.active_overlay_remaining_ticks,
            self.active_overlay_lifetime_ticks,
        ) {
            (Some(remaining), Some(total)) => Some((remaining, total)),
            _ => None,
        }
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
    let id_active = runtime_ui.text_input.last_id.is_some_and(|text_input_id| {
        runtime_ui.menu.last_text_input_result_id != Some(text_input_id)
    });

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
        HudModel, RuntimeChatObservability, RuntimeHudTextObservability,
        RuntimeLiveEntitySummaryObservability, RuntimeMenuObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiNoticeLayerKind,
        RuntimeUiObservability, RuntimeUiPromptLayerKind, RuntimeUiStackForegroundSummaryKind,
        RuntimeUiStackSummary, RuntimeWorldPositionObservability,
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
        assert_eq!(summary.menu_depth(), 3);
        assert_eq!(summary.hud_depth(), 4);
        assert_eq!(summary.dialog_depth(), 8);
        assert_eq!(summary.active_group_count(), 3);
        assert_eq!(summary.total_depth(), 8);
        assert_eq!(
            summary.summary_label(),
            "fg=input prompt=input depth=3 notice=warn depth=4 chat=on groups=3"
        );
        assert_eq!(
            summary.detail_label(),
            "fg=input prompt=input layers=[input,follow-up,menu] notice=warn layers=[hud,reliable,info,warn] chat=on groups=3 depth=8 menu=3 hud=4 dialog=8 text-input=2 server-msg=1 chat-msg=2 chat-sender=77"
        );
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
        assert_eq!(summary.menu_depth(), 3);
        assert_eq!(summary.hud_depth(), 4);
        assert_eq!(summary.dialog_depth(), 8);
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
        assert_eq!(summary.menu_depth(), 0);
        assert_eq!(summary.hud_depth(), 0);
        assert_eq!(summary.dialog_depth(), 0);
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
        assert_eq!(summary.menu_depth(), 0);
        assert_eq!(summary.hud_depth(), 0);
        assert_eq!(summary.dialog_depth(), 0);
        assert_eq!(summary.total_depth(), 0);

        let depth = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui depth summary");
        assert!(depth.is_empty());
    }

    #[test]
    fn runtime_ui_stack_summary_with_recent_counts_is_not_empty() {
        let summary = RuntimeUiStackSummary {
            menu_open_count: 1,
            outstanding_follow_up_count: 1,
            text_input_open_count: 1,
            server_message_count: 1,
            chat_message_count: 1,
            ..RuntimeUiStackSummary::default()
        };

        assert!(!summary.is_empty());
        assert_eq!(
            summary.summary_label(),
            "fg=none prompt=none depth=0 notice=none depth=0 chat=off groups=0"
        );
        assert_eq!(
            summary.detail_label(),
            "fg=none prompt=none layers=[] notice=none layers=[] chat=off groups=0 depth=0 menu=0 hud=0 dialog=0 text-input=1 server-msg=1 chat-msg=1 chat-sender=none"
        );
    }

    #[test]
    fn hud_summary_visibility_helpers_compute_counts_and_percentages() {
        let summary = super::HudSummary {
            player_name: "operator".to_string(),
            team_id: 2,
            selected_block: "payload-router".to_string(),
            plan_count: 3,
            marker_count: 4,
            map_width: 10,
            map_height: 10,
            overlay_visible: true,
            fog_enabled: true,
            visible_tile_count: 25,
            hidden_tile_count: 15,
            minimap: super::HudMinimapSummary {
                focus_tile: Some((2, 3)),
                view_window: super::HudViewWindowSummary {
                    origin_x: 1,
                    origin_y: 2,
                    width: 4,
                    height: 4,
                },
            },
        };

        assert_eq!(summary.map_tile_count(), 100);
        assert_eq!(summary.known_tile_count(), 40);
        assert_eq!(summary.unknown_tile_count(), 60);
        assert_eq!(summary.known_tile_percent(), 40);
        assert_eq!(summary.unknown_tile_percent(), 60);
        assert_eq!(summary.visible_map_percent(), 25);
        assert_eq!(summary.hidden_map_percent(), 15);
        assert_eq!(summary.visibility_label(), "mixed");
        assert_eq!(summary.overlay_label(), "on");
        assert_eq!(summary.fog_label(), "on");
        assert_eq!(summary.minimap.focus_tile_label(), "2:3");
        assert_eq!(summary.minimap.view_window.tile_count(), 16);
        assert_eq!(summary.minimap.view_window.origin_label(), "1:2");
        assert_eq!(summary.minimap.view_window.size_label(), "4x4");
        assert_eq!(
            summary.minimap.view_window.summary_label(),
            "origin=1:2 size=4x4"
        );
        assert_eq!(
            summary.minimap.view_window.detail_label(),
            "origin=1:2 size=4x4 area=16"
        );
        assert_eq!(summary.minimap.summary_label(), "focus=2:3 window=1:2+4x4");
        assert_eq!(
            summary.minimap.detail_label(),
            "focus=2:3 window-origin=1:2 window-size=4x4 window-area=16"
        );
        assert_eq!(
            summary.summary_label(),
            "team=2 block=payload-router plans=3 markers=4 vis=mixed known=40 visible=25 overlay=on fog=on minimap=focus=2:3 window=1:2+4x4"
        );
        assert_eq!(
            summary.detail_label(),
            "player=operator team=2 block=payload-router plans=3 markers=4 map=10x10 tiles=100 vis=mixed known=40 unknown=60 visible=25 hidden=15 overlay=on fog=on minimap=focus=2:3 window-origin=1:2 window-size=4x4 window-area=16"
        );
    }

    #[test]
    fn hud_summary_visibility_helpers_fail_closed_on_empty_and_overflowing_maps() {
        let empty_summary = super::HudSummary {
            player_name: String::new(),
            team_id: 0,
            selected_block: String::new(),
            plan_count: 0,
            marker_count: 0,
            map_width: 0,
            map_height: 0,
            overlay_visible: false,
            fog_enabled: false,
            visible_tile_count: usize::MAX,
            hidden_tile_count: usize::MAX,
            minimap: super::HudMinimapSummary {
                focus_tile: None,
                view_window: super::HudViewWindowSummary {
                    origin_x: 0,
                    origin_y: 0,
                    width: 0,
                    height: 0,
                },
            },
        };

        assert_eq!(empty_summary.map_tile_count(), 0);
        assert_eq!(empty_summary.known_tile_count(), usize::MAX);
        assert_eq!(empty_summary.unknown_tile_count(), 0);
        assert_eq!(empty_summary.known_tile_percent(), 0);
        assert_eq!(empty_summary.unknown_tile_percent(), 0);
        assert_eq!(empty_summary.visible_map_percent(), 0);
        assert_eq!(empty_summary.hidden_map_percent(), 0);
        assert_eq!(empty_summary.visibility_label(), "empty");
        assert_eq!(empty_summary.overlay_label(), "off");
        assert_eq!(empty_summary.fog_label(), "off");
        assert_eq!(empty_summary.minimap.focus_tile_label(), "none");
        assert_eq!(empty_summary.minimap.view_window.tile_count(), 0);
        assert_eq!(
            empty_summary.minimap.summary_label(),
            "focus=none window=0:0+0x0"
        );
        assert_eq!(
            empty_summary.minimap.detail_label(),
            "focus=none window-origin=0:0 window-size=0x0 window-area=0"
        );
        assert_eq!(
            empty_summary.summary_label(),
            "team=0 block= plans=0 markers=0 vis=empty known=0 visible=0 overlay=off fog=off minimap=focus=none window=0:0+0x0"
        );
        assert_eq!(
            empty_summary.detail_label(),
            "player= team=0 block= plans=0 markers=0 map=0x0 tiles=0 vis=empty known=0 unknown=0 visible=0 hidden=0 overlay=off fog=off minimap=focus=none window-origin=0:0 window-size=0x0 window-area=0"
        );

        let overflowing_summary = super::HudSummary {
            player_name: String::new(),
            team_id: 0,
            selected_block: String::new(),
            plan_count: 0,
            marker_count: 0,
            map_width: usize::MAX,
            map_height: 2,
            overlay_visible: false,
            fog_enabled: false,
            visible_tile_count: 1,
            hidden_tile_count: 2,
            minimap: super::HudMinimapSummary {
                focus_tile: None,
                view_window: super::HudViewWindowSummary {
                    origin_x: 0,
                    origin_y: 0,
                    width: 0,
                    height: 0,
                },
            },
        };

        assert_eq!(overflowing_summary.map_tile_count(), usize::MAX);
        assert_eq!(overflowing_summary.known_tile_count(), 3);
        assert_eq!(overflowing_summary.unknown_tile_count(), usize::MAX - 3);
        assert_eq!(overflowing_summary.known_tile_percent(), 0);
        assert_eq!(overflowing_summary.visible_map_percent(), 0);
        assert_eq!(overflowing_summary.hidden_map_percent(), 0);
        assert_eq!(overflowing_summary.visibility_label(), "mixed");
        assert_eq!(
            overflowing_summary.detail_label(),
            "player= team=0 block= plans=0 markers=0 map=18446744073709551615x2 tiles=18446744073709551615 vis=mixed known=0 unknown=1 visible=0 hidden=0 overlay=off fog=off minimap=focus=none window-origin=0:0 window-size=0x0 window-area=0"
        );
    }

    #[test]
    fn hud_summary_visibility_label_covers_state_transitions() {
        let base = super::HudSummary {
            player_name: String::new(),
            team_id: 0,
            selected_block: String::new(),
            plan_count: 0,
            marker_count: 0,
            map_width: 2,
            map_height: 2,
            overlay_visible: false,
            fog_enabled: false,
            visible_tile_count: 0,
            hidden_tile_count: 0,
            minimap: super::HudMinimapSummary {
                focus_tile: None,
                view_window: super::HudViewWindowSummary {
                    origin_x: 0,
                    origin_y: 0,
                    width: 0,
                    height: 0,
                },
            },
        };

        assert_eq!(base.visibility_label(), "unseen");
        assert_eq!(
            super::HudSummary {
                hidden_tile_count: 4,
                ..base.clone()
            }
            .visibility_label(),
            "hidden"
        );
        assert_eq!(
            super::HudSummary {
                visible_tile_count: 4,
                ..base.clone()
            }
            .visibility_label(),
            "clear"
        );
        assert_eq!(
            super::HudSummary {
                visible_tile_count: 2,
                hidden_tile_count: 2,
                ..base
            }
            .visibility_label(),
            "mapped"
        );
    }

    #[test]
    fn runtime_live_entity_detail_label_surfaces_local_owned_unit_facets() {
        let entity = RuntimeLiveEntitySummaryObservability {
            entity_count: 12,
            hidden_count: 3,
            player_count: 2,
            unit_count: 1,
            last_entity_id: Some(202),
            last_player_entity_id: Some(102),
            last_unit_entity_id: Some(202),
            local_entity_id: Some(404),
            local_unit_kind: Some(2),
            local_unit_value: Some(999),
            local_hidden: Some(false),
            local_last_seen_entity_snapshot_count: Some(7),
            local_position: Some(RuntimeWorldPositionObservability {
                x_bits: 20.0f32.to_bits(),
                y_bits: 33.0f32.to_bits(),
            }),
            local_owned_unit_entity_id: Some(202),
            local_owned_unit_payload_count: Some(2),
            local_owned_unit_payload_class_id: Some(5),
            local_owned_unit_payload_revision: Some(7),
            local_owned_unit_payload_body_len: Some(12),
            local_owned_unit_payload_sha256: Some(
                "0123456789abcdef0123456789abcdef".to_string(),
            ),
            local_owned_unit_payload_nested_descendant_count: Some(2),
            local_owned_carried_item_id: Some(6),
            local_owned_carried_item_amount: Some(4),
            local_owned_controller_type: Some(4),
            local_owned_controller_value: Some(101),
        };

        assert_eq!(
            entity.local_owned_unit_payload_label(),
            "payload=count=2:unit=5/r7/l12:s0123456789ab"
        );
        assert_eq!(
            entity.local_owned_unit_nested_label(),
            "nested=2"
        );
        assert_eq!(
            entity.local_owned_unit_stack_label(),
            "stack=6x4"
        );
        assert_eq!(
            entity.local_owned_unit_controller_label(),
            "controller=4/101"
        );
        assert_eq!(
            entity.detail_label(),
            "local=404 unit=2/999 pos=20.0:33.0 hidden=0 seen=7 players=2 units=1 last=202/102/202 owned=202 payload=count=2:unit=5/r7/l12:s0123456789ab nested=2 stack=6x4 controller=4/101"
        );
    }
}
