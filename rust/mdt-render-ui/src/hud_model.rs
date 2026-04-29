use crate::presenter_view::compact_runtime_ui_text;
use std::fmt::Display;

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

fn percent_of_total(part: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        part.saturating_mul(100) / total
    }
}

fn percent_of(part: usize, total: usize) -> usize {
    percent_of_total(part, total)
}

fn optional_numeric_label<T: Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_i32_label(value: Option<i32>) -> String {
    optional_numeric_label(value)
}

fn optional_u8_label(value: Option<u8>) -> String {
    optional_numeric_label(value)
}

fn optional_i16_label(value: Option<i16>) -> String {
    optional_numeric_label(value)
}

fn optional_u32_label(value: Option<u32>) -> String {
    optional_numeric_label(value)
}

fn optional_usize_label(value: Option<usize>) -> String {
    optional_numeric_label(value)
}

fn optional_u64_label(value: Option<u64>) -> String {
    optional_numeric_label(value)
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

fn compact_prefix_label(value: Option<&str>, prefix_len: usize) -> String {
    value
        .map(|value| value.chars().take(prefix_len).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn compact_sha_label(value: Option<&str>) -> String {
    compact_prefix_label(value, 12)
}

fn world_position_status_label(value: Option<&RuntimeWorldPositionObservability>) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    let x = f32::from_bits(value.x_bits);
    let y = f32::from_bits(value.y_bits);
    if x.is_finite() && y.is_finite() {
        format!("{x:.1}:{y:.1}")
    } else {
        format!("0x{:08x}:0x{:08x}", value.x_bits, value.y_bits)
    }
}

fn live_effect_position_source_status_label(
    source: Option<RuntimeLiveEffectPositionSource>,
) -> &'static str {
    match source {
        Some(RuntimeLiveEffectPositionSource::ActiveOverlay) => "active",
        Some(RuntimeLiveEffectPositionSource::BusinessProjection) => "biz",
        Some(RuntimeLiveEffectPositionSource::EffectPacket) => "pkt",
        Some(RuntimeLiveEffectPositionSource::SpawnEffectPacket) => "spawn",
        None => "none",
    }
}

fn runtime_live_entity_status_label(entity: &RuntimeLiveEntitySummaryObservability) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}:tp{}/{}:{}:last{}/{}/{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_status_label(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
        entity.player_count,
        entity.unit_count,
        entity.ownership_label(),
        optional_i32_label(entity.last_entity_id),
        optional_i32_label(entity.last_player_entity_id),
        optional_i32_label(entity.last_unit_entity_id),
    )
}

fn runtime_live_effect_status_label(effect: &RuntimeLiveEffectSummaryObservability) -> String {
    format!(
        "{}/{}@{}:u{}:k{}:c{}/{}:h{}:p{}@{}",
        effect.effect_count,
        effect.spawn_effect_count,
        optional_i16_label(effect.display_effect_id()),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.display_contract_name()),
        compact_runtime_ui_text(effect.display_reliable_contract_name()),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_label(effect.display_position_source()),
        world_position_status_label(effect.display_position()),
    )
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

impl RuntimeUiObservability {
    pub fn status_label(&self) -> String {
        if self == &Self::default() {
            return "ui:hud=0/0/0@none/none:ann=0@none:info=0@none:toast=0/0@none/none:popup=0/0:clip0:uri0:choice=0/0:tin=0@none:none/none/none#0:nn:live=ent=0/0@none:unone/none:pnone:hn:snone:tp0/0:own=0/0:c0@none:lastnone/none/none:fx=0/0:ov0@none:unone:dnone:knone:cnone/none:bindnone:r?:hnone:pnone@none:ttlnone".to_string();
        }

        let hud_text = &self.hud_text;
        let toast = &self.toast;
        let menu = &self.menu;
        let text_input = &self.text_input;
        let live = &self.live;

        format!(
            "ui:hud={}/{}/{}@{}/{}:ann={}@{}:info={}@{}:toast={}/{}@{}/{}:popup={}/{}:clip{}:uri{}:choice={}/{}:tin={}@{}:{}/{}/{}#{}:n{}:e{}:live=ent={}:fx={}",
            hud_text.set_count,
            hud_text.set_reliable_count,
            hud_text.hide_count,
            compact_runtime_ui_text(hud_text.last_message.as_deref()),
            compact_runtime_ui_text(hud_text.last_reliable_message.as_deref()),
            hud_text.announce_count,
            compact_runtime_ui_text(hud_text.last_announce_message.as_deref()),
            hud_text.info_message_count,
            compact_runtime_ui_text(hud_text.last_info_message.as_deref()),
            toast.info_count,
            toast.warning_count,
            compact_runtime_ui_text(toast.last_info_message.as_deref()),
            compact_runtime_ui_text(toast.last_warning_text.as_deref()),
            toast.info_popup_count,
            toast.info_popup_reliable_count,
            toast.clipboard_count,
            toast.open_uri_count,
            menu.menu_choose_count,
            menu.text_input_result_count,
            text_input.open_count,
            optional_i32_label(text_input.last_id),
            compact_runtime_ui_text(text_input.last_title.as_deref()),
            compact_runtime_ui_text(text_input.last_message.as_deref()),
            compact_runtime_ui_text(text_input.last_default_text.as_deref()),
            text_input.last_length.unwrap_or_default(),
            optional_bool_label(text_input.last_numeric),
            optional_bool_label(text_input.last_allow_empty),
            runtime_live_entity_status_label(&live.entity),
            runtime_live_effect_status_label(&live.effect),
        )
    }
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
pub struct RuntimePayloadSubtreeStatusObservability {
    pub total_count: usize,
    pub dynamic_count: usize,
    pub payload_with_status_count: usize,
    pub first_status_id: Option<i16>,
    pub first_status_name: Option<String>,
    pub first_status_time_bits: Option<u32>,
    pub first_status_dynamic_field_count: Option<usize>,
}

impl RuntimePayloadSubtreeStatusObservability {
    pub fn detail_label(&self) -> String {
        let mut label = format!(
            "c={}:d={}:n={}:f={}",
            self.total_count,
            self.dynamic_count,
            self.payload_with_status_count,
            optional_i16_label(self.first_status_id),
        );
        if let Some(name) = self.first_status_name.as_deref() {
            if !name.is_empty() {
                label.push('/');
                label.push_str(name);
            }
        }
        label.push('@');
        match self.first_status_time_bits {
            Some(bits) => label.push_str(&f32::from_bits(bits).to_string()),
            None => label.push_str("none"),
        }
        if let Some(count) = self.first_status_dynamic_field_count {
            label.push_str(&format!(":fd={count}"));
        }
        label
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeUnitControllerAttackTargetObservability {
    pub kind: Option<String>,
    pub value: Option<i32>,
}

impl RuntimeUnitControllerAttackTargetObservability {
    pub fn detail_label(&self) -> String {
        match (self.kind.as_deref(), self.value) {
            (Some(kind), Some(value)) if !kind.is_empty() => format!("{kind}/{value}"),
            (Some(kind), None) if !kind.is_empty() => kind.to_string(),
            (None, Some(value)) => value.to_string(),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeUnitControllerCommandQueueObservability {
    pub total_count: usize,
    pub building_count: usize,
    pub unit_count: usize,
    pub position_count: usize,
    pub ignored_count: usize,
}

impl RuntimeUnitControllerCommandQueueObservability {
    pub fn detail_label(&self) -> String {
        format!(
            "q={}/{}/{}/{}/{}",
            self.total_count,
            self.building_count,
            self.unit_count,
            self.position_count,
            self.ignored_count,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeUnitControllerObservability {
    pub controller_type: Option<u8>,
    pub controller_value: Option<i32>,
    pub target_position_tile: Option<(i32, i32)>,
    pub attack_target: Option<RuntimeUnitControllerAttackTargetObservability>,
    pub command_id: Option<u8>,
    pub command_queue: Option<RuntimeUnitControllerCommandQueueObservability>,
    pub stance_id: Option<u8>,
    pub status_count: Option<usize>,
}

impl RuntimeUnitControllerObservability {
    pub fn detail_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some((x, y)) = self.target_position_tile {
            parts.push(format!("tp={x}:{y}"));
        }
        if let Some(target) = self
            .attack_target
            .as_ref()
            .map(Self::attack_target_label)
            .filter(|label| !label.is_empty())
        {
            parts.push(format!("atk={target}"));
        }
        if let Some(command_id) = self.command_id {
            parts.push(format!("cmd={command_id}"));
        }
        if let Some(command_queue) = self.command_queue.as_ref() {
            parts.push(command_queue.detail_label());
        }
        if let Some(stance_id) = self.stance_id {
            parts.push(format!("st={stance_id}"));
        }
        if let Some(status_count) = self.status_count {
            parts.push(format!("sts={status_count}"));
        }
        parts.join(":")
    }

    fn attack_target_label(
        attack_target: &RuntimeUnitControllerAttackTargetObservability,
    ) -> String {
        attack_target.detail_label()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeLiveEntitySummaryObservability {
    pub entity_count: usize,
    pub hidden_count: usize,
    pub player_count: usize,
    pub unit_count: usize,
    pub player_with_owned_unit_count: usize,
    pub owned_unit_count: usize,
    pub ownership_conflict_count: usize,
    pub ownership_conflict_unit_sample: Vec<i32>,
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
    pub local_owned_unit_payload_status: Option<RuntimePayloadSubtreeStatusObservability>,
    pub local_owned_carried_item_id: Option<i16>,
    pub local_owned_carried_item_amount: Option<i32>,
    pub local_owned_controller_type: Option<u8>,
    pub local_owned_controller_value: Option<i32>,
    pub local_owned_controller_detail: Option<String>,
    pub local_owned_controller_v2: Option<RuntimeUnitControllerObservability>,
    pub local_owned_unit_status_detail: Option<String>,
}

impl RuntimeLiveEntitySummaryObservability {
    pub fn ownership_conflict_unit_sample_label(&self) -> String {
        if self.ownership_conflict_unit_sample.is_empty() {
            "none".to_string()
        } else {
            self.ownership_conflict_unit_sample
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    pub fn ownership_label(&self) -> String {
        format!(
            "own={}/{}:c{}@{}",
            self.player_with_owned_unit_count,
            self.owned_unit_count,
            self.ownership_conflict_count,
            self.ownership_conflict_unit_sample_label(),
        )
    }

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

    pub fn local_owned_unit_payload_status_label(&self) -> String {
        let Some(status) = self.local_owned_unit_payload_status.as_ref() else {
            return "payload-status=none".to_string();
        };
        format!("payload-status={}", status.detail_label())
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
        let controller_type = self
            .local_owned_controller_v2
            .as_ref()
            .and_then(|controller| controller.controller_type)
            .or(self.local_owned_controller_type);
        let controller_value = self
            .local_owned_controller_v2
            .as_ref()
            .and_then(|controller| controller.controller_value)
            .or(self.local_owned_controller_value);
        let mut label = format!(
            "controller={}/{}",
            optional_u8_label(controller_type),
            optional_i32_label(controller_value),
        );
        let detail = self
            .local_owned_controller_v2
            .as_ref()
            .map(RuntimeUnitControllerObservability::detail_label)
            .filter(|detail| !detail.is_empty())
            .or_else(|| {
                self.local_owned_controller_detail
                    .as_ref()
                    .filter(|detail| !detail.is_empty())
                    .cloned()
            });
        if let Some(detail) = detail {
            label.push(':');
            label.push_str(&detail);
        }
        label
    }

    pub fn local_owned_unit_status_label(&self) -> String {
        match self.local_owned_unit_status_detail.as_deref() {
            Some(detail) if !detail.is_empty() => format!("status={detail}"),
            _ => "status=none".to_string(),
        }
    }

    pub fn detail_label(&self) -> String {
        format!(
            "local={} unit={}/{} pos={} hidden={} seen={} players={} units={} {} last={}/{}/{} owned={} {} {} {} {} {} {}",
            optional_i32_label(self.local_entity_id),
            optional_u8_label(self.local_unit_kind),
            optional_u32_label(self.local_unit_value),
            world_position_text(self.local_position.as_ref()),
            optional_bool_label(self.local_hidden),
            optional_u64_label(self.local_last_seen_entity_snapshot_count),
            self.player_count,
            self.unit_count,
            self.ownership_label(),
            optional_i32_label(self.last_entity_id),
            optional_i32_label(self.last_player_entity_id),
            optional_i32_label(self.last_unit_entity_id),
            optional_i32_label(self.local_owned_unit_entity_id),
            self.local_owned_unit_payload_label(),
            self.local_owned_unit_nested_label(),
            self.local_owned_unit_payload_status_label(),
            self.local_owned_unit_stack_label(),
            self.local_owned_unit_controller_label(),
            self.local_owned_unit_status_label(),
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
    pub last_effect_id: Option<i16>,
    pub last_spawn_effect_unit_type_id: Option<i16>,
    pub last_kind: Option<String>,
    pub last_contract_name: Option<String>,
    pub last_reliable_contract_name: Option<String>,
    pub last_business_hint: Option<String>,
    pub last_position_hint: Option<RuntimeWorldPositionObservability>,
    pub last_position_source: Option<RuntimeLiveEffectPositionSource>,
    pub session_target_binding_state: Option<String>,
    pub session_source_binding_state: Option<String>,
    pub overlay_target_binding_state: Option<String>,
    pub overlay_source_binding_state: Option<String>,
    pub target_follow_count: u64,
    pub target_reject_count: u64,
    pub target_fallback_count: u64,
    pub source_follow_count: u64,
    pub source_reject_count: u64,
    pub source_fallback_count: u64,
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

    pub fn binding_source_label(&self) -> &'static str {
        if self.session_target_binding_state.is_some() || self.session_source_binding_state.is_some()
        {
            "session"
        } else if self.overlay_target_binding_state.is_some()
            || self.overlay_source_binding_state.is_some()
        {
            "overlay"
        } else {
            "none"
        }
    }

    pub fn display_target_binding_state(&self) -> Option<&str> {
        if self.binding_source_label() == "session" {
            self.session_target_binding_state.as_deref()
        } else {
            self.overlay_target_binding_state.as_deref()
        }
    }

    pub fn display_source_binding_state(&self) -> Option<&str> {
        if self.binding_source_label() == "session" {
            self.session_source_binding_state.as_deref()
        } else {
            self.overlay_source_binding_state.as_deref()
        }
    }

    pub fn target_binding_counts_label(&self) -> String {
        format!(
            "{}/{}/{}",
            self.target_follow_count, self.target_reject_count, self.target_fallback_count
        )
    }

    pub fn source_binding_counts_label(&self) -> String {
        format!(
            "{}/{}/{}",
            self.source_follow_count, self.source_reject_count, self.source_fallback_count
        )
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
        HudMinimapSummary, HudModel, HudSummary, HudViewWindowSummary, RuntimeChatObservability,
        RuntimeHudTextObservability, RuntimeLiveEntitySummaryObservability,
        RuntimeMenuObservability, RuntimePayloadSubtreeStatusObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiNoticeLayerKind,
        RuntimeUiObservability, RuntimeUiPromptLayerKind, RuntimeUiStackForegroundSummaryKind,
        RuntimeUiStackDepthSummary, RuntimeUiStackSummary,
        RuntimeUnitControllerAttackTargetObservability,
        RuntimeUnitControllerCommandQueueObservability, RuntimeUnitControllerObservability,
        RuntimeWorldPositionObservability,
    };

    #[derive(Clone, Copy)]
    struct RuntimeUiDepthExpectations {
        prompt_depth: usize,
        notice_depth: usize,
        chat_depth: usize,
        active_group_count: usize,
        total_depth: usize,
    }

    #[derive(Clone, Copy)]
    struct HudVisibilityExpectations<'a> {
        map_tile_count: usize,
        known_tile_count: usize,
        unknown_tile_count: usize,
        known_tile_percent: usize,
        unknown_tile_percent: usize,
        visible_map_percent: usize,
        hidden_map_percent: usize,
        visibility_label: &'a str,
        overlay_label: &'a str,
        fog_label: &'a str,
    }

    #[derive(Clone, Copy)]
    struct MinimapWindowLabelExpectations<'a> {
        focus_tile_label: &'a str,
        tile_count: usize,
        origin_label: &'a str,
        size_label: &'a str,
        summary_label: &'a str,
        view_window_detail_label: &'a str,
        detail_label: &'a str,
    }

    fn hud_with_runtime_ui(runtime_ui: RuntimeUiObservability) -> HudModel {
        HudModel {
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        }
    }

    fn runtime_ui_stack_fixture() -> RuntimeUiObservability {
        RuntimeUiObservability {
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
        }
    }

    fn hud_minimap_summary(
        focus_tile: Option<(usize, usize)>,
        origin_x: usize,
        origin_y: usize,
        width: usize,
        height: usize,
    ) -> HudMinimapSummary {
        HudMinimapSummary {
            focus_tile,
            view_window: HudViewWindowSummary {
                origin_x,
                origin_y,
                width,
                height,
            },
        }
    }

    fn hud_summary_fixture() -> HudSummary {
        HudSummary {
            player_name: String::new(),
            team_id: 0,
            selected_block: String::new(),
            plan_count: 0,
            marker_count: 0,
            map_width: 0,
            map_height: 0,
            overlay_visible: false,
            fog_enabled: false,
            visible_tile_count: 0,
            hidden_tile_count: 0,
            minimap: hud_minimap_summary(None, 0, 0, 0, 0),
        }
    }

    fn assert_runtime_ui_stack_summary_depths(
        summary: &RuntimeUiStackSummary,
        expected: RuntimeUiDepthExpectations,
    ) {
        assert_eq!(summary.prompt_depth(), expected.prompt_depth);
        assert_eq!(summary.notice_depth(), expected.notice_depth);
        assert_eq!(summary.chat_depth(), expected.chat_depth);
        assert_eq!(summary.menu_depth(), expected.prompt_depth);
        assert_eq!(summary.hud_depth(), expected.notice_depth);
        assert_eq!(summary.dialog_depth(), expected.total_depth);
        assert_eq!(summary.active_group_count(), expected.active_group_count);
        assert_eq!(summary.total_depth(), expected.total_depth);
    }

    fn assert_runtime_ui_depth_summary(
        summary: &RuntimeUiStackDepthSummary,
        expected: RuntimeUiDepthExpectations,
    ) {
        assert_eq!(summary.prompt_depth, expected.prompt_depth);
        assert_eq!(summary.notice_depth, expected.notice_depth);
        assert_eq!(summary.chat_depth, expected.chat_depth);
        assert_eq!(summary.menu_depth(), expected.prompt_depth);
        assert_eq!(summary.hud_depth(), expected.notice_depth);
        assert_eq!(summary.dialog_depth(), expected.total_depth);
        assert_eq!(summary.active_group_count, expected.active_group_count);
        assert_eq!(summary.total_depth, expected.total_depth);
    }

    fn assert_hud_visibility(
        summary: &HudSummary,
        expected: HudVisibilityExpectations<'_>,
    ) {
        assert_eq!(summary.map_tile_count(), expected.map_tile_count);
        assert_eq!(summary.known_tile_count(), expected.known_tile_count);
        assert_eq!(summary.unknown_tile_count(), expected.unknown_tile_count);
        assert_eq!(summary.known_tile_percent(), expected.known_tile_percent);
        assert_eq!(summary.unknown_tile_percent(), expected.unknown_tile_percent);
        assert_eq!(summary.visible_map_percent(), expected.visible_map_percent);
        assert_eq!(summary.hidden_map_percent(), expected.hidden_map_percent);
        assert_eq!(summary.visibility_label(), expected.visibility_label);
        assert_eq!(summary.overlay_label(), expected.overlay_label);
        assert_eq!(summary.fog_label(), expected.fog_label);
    }

    fn assert_minimap_window_labels(
        minimap: &HudMinimapSummary,
        expected: MinimapWindowLabelExpectations<'_>,
    ) {
        assert_eq!(minimap.focus_tile_label(), expected.focus_tile_label);
        assert_eq!(minimap.view_window.tile_count(), expected.tile_count);
        assert_eq!(minimap.view_window.origin_label(), expected.origin_label);
        assert_eq!(minimap.view_window.size_label(), expected.size_label);
        assert_eq!(minimap.view_window.summary_label(), expected.summary_label);
        assert_eq!(
            minimap.view_window.detail_label(),
            expected.view_window_detail_label
        );
        assert_eq!(minimap.detail_label(), expected.detail_label);
    }

    #[test]
    fn runtime_ui_stack_summary_tracks_foreground_and_layer_order() {
        let hud = hud_with_runtime_ui(runtime_ui_stack_fixture());

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
        assert_runtime_ui_stack_summary_depths(
            &summary,
            RuntimeUiDepthExpectations {
                prompt_depth: 3,
                notice_depth: 4,
                chat_depth: 1,
                active_group_count: 3,
                total_depth: 8,
            },
        );
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
        let hud = hud_with_runtime_ui(runtime_ui_stack_fixture());

        let summary = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui summary");
        assert_runtime_ui_depth_summary(
            &summary,
            RuntimeUiDepthExpectations {
                prompt_depth: 3,
                notice_depth: 4,
                chat_depth: 1,
                active_group_count: 3,
                total_depth: 8,
            },
        );
        assert!(!summary.is_empty());
    }

    #[test]
    fn runtime_ui_stack_depth_summary_is_empty_for_default_runtime_ui() {
        let hud = hud_with_runtime_ui(RuntimeUiObservability::default());

        let summary = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui summary");
        assert_runtime_ui_depth_summary(
            &summary,
            RuntimeUiDepthExpectations {
                prompt_depth: 0,
                notice_depth: 0,
                chat_depth: 0,
                active_group_count: 0,
                total_depth: 0,
            },
        );
        assert!(summary.is_empty());
    }

    #[test]
    fn runtime_ui_stack_summary_keeps_hide_hud_history_out_of_active_notice_layers() {
        let hud = hud_with_runtime_ui(RuntimeUiObservability {
            hud_text: RuntimeHudTextObservability {
                hide_count: 1,
                ..RuntimeHudTextObservability::default()
            },
            ..RuntimeUiObservability::default()
        });

        let summary = hud
            .runtime_ui_stack_summary()
            .expect("runtime ui stack summary");
        assert_eq!(summary.notice_kind, None);
        assert!(summary.notice_layers.is_empty());
        assert_eq!(summary.notice_depth(), 0);
        assert_eq!(summary.hud_depth(), 0);
    }

    #[test]
    fn runtime_ui_stack_summary_drops_completed_prompt_layers_from_foreground() {
        let hud = hud_with_runtime_ui(RuntimeUiObservability {
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
        });

        let summary = hud
            .runtime_ui_stack_summary()
            .expect("runtime ui stack summary");
        assert_eq!(summary.foreground_kind, None);
        assert_eq!(summary.prompt_kind, None);
        assert!(summary.prompt_layers.is_empty());
        assert_runtime_ui_stack_summary_depths(
            &summary,
            RuntimeUiDepthExpectations {
                prompt_depth: 0,
                notice_depth: 0,
                chat_depth: 0,
                active_group_count: 0,
                total_depth: 0,
            },
        );

        let depth = hud
            .runtime_ui_stack_depth_summary()
            .expect("runtime ui depth summary");
        assert_runtime_ui_depth_summary(
            &depth,
            RuntimeUiDepthExpectations {
                prompt_depth: 0,
                notice_depth: 0,
                chat_depth: 0,
                active_group_count: 0,
                total_depth: 0,
            },
        );
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
    fn runtime_ui_status_label_preserves_empty_field_order_and_default_markers() {
        let runtime_ui = RuntimeUiObservability::default();

        assert_eq!(
            runtime_ui.status_label(),
            "ui:hud=0/0/0@none/none:ann=0@none:info=0@none:toast=0/0@none/none:popup=0/0:clip0:uri0:choice=0/0:tin=0@none:none/none/none#0:nn:live=ent=0/0@none:unone/none:pnone:hn:snone:tp0/0:own=0/0:c0@none:lastnone/none/none:fx=0/0:ov0@none:unone:dnone:knone:cnone/none:bindnone:r?:hnone:pnone@none:ttlnone"
        );
    }

    #[test]
    fn hud_summary_visibility_helpers_compute_counts_and_percentages() {
        let summary = HudSummary {
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
            minimap: hud_minimap_summary(Some((2, 3)), 1, 2, 4, 4),
            ..hud_summary_fixture()
        };

        assert_hud_visibility(
            &summary,
            HudVisibilityExpectations {
                map_tile_count: 100,
                known_tile_count: 40,
                unknown_tile_count: 60,
                known_tile_percent: 40,
                unknown_tile_percent: 60,
                visible_map_percent: 25,
                hidden_map_percent: 15,
                visibility_label: "mixed",
                overlay_label: "on",
                fog_label: "on",
            },
        );
        assert_minimap_window_labels(
            &summary.minimap,
            MinimapWindowLabelExpectations {
                focus_tile_label: "2:3",
                tile_count: 16,
                origin_label: "1:2",
                size_label: "4x4",
                summary_label: "origin=1:2 size=4x4",
                view_window_detail_label: "origin=1:2 size=4x4 area=16",
                detail_label: "focus=2:3 window-origin=1:2 window-size=4x4 window-area=16",
            },
        );
        assert_eq!(summary.minimap.summary_label(), "focus=2:3 window=1:2+4x4");
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
        let empty_summary = HudSummary {
            visible_tile_count: usize::MAX,
            hidden_tile_count: usize::MAX,
            ..hud_summary_fixture()
        };

        assert_hud_visibility(
            &empty_summary,
            HudVisibilityExpectations {
                map_tile_count: 0,
                known_tile_count: usize::MAX,
                unknown_tile_count: 0,
                known_tile_percent: 0,
                unknown_tile_percent: 0,
                visible_map_percent: 0,
                hidden_map_percent: 0,
                visibility_label: "empty",
                overlay_label: "off",
                fog_label: "off",
            },
        );
        assert_minimap_window_labels(
            &empty_summary.minimap,
            MinimapWindowLabelExpectations {
                focus_tile_label: "none",
                tile_count: 0,
                origin_label: "0:0",
                size_label: "0x0",
                summary_label: "origin=0:0 size=0x0",
                view_window_detail_label: "origin=0:0 size=0x0 area=0",
                detail_label: "focus=none window-origin=0:0 window-size=0x0 window-area=0",
            },
        );
        assert_eq!(
            empty_summary.minimap.summary_label(),
            "focus=none window=0:0+0x0"
        );
        assert_eq!(
            empty_summary.summary_label(),
            "team=0 block= plans=0 markers=0 vis=empty known=0 visible=0 overlay=off fog=off minimap=focus=none window=0:0+0x0"
        );
        assert_eq!(
            empty_summary.detail_label(),
            "player= team=0 block= plans=0 markers=0 map=0x0 tiles=0 vis=empty known=0 unknown=0 visible=0 hidden=0 overlay=off fog=off minimap=focus=none window-origin=0:0 window-size=0x0 window-area=0"
        );

        let overflowing_summary = HudSummary {
            map_width: usize::MAX,
            map_height: 2,
            visible_tile_count: 1,
            hidden_tile_count: 2,
            ..hud_summary_fixture()
        };

        assert_hud_visibility(
            &overflowing_summary,
            HudVisibilityExpectations {
                map_tile_count: usize::MAX,
                known_tile_count: 3,
                unknown_tile_count: usize::MAX - 3,
                known_tile_percent: 0,
                unknown_tile_percent: 1,
                visible_map_percent: 0,
                hidden_map_percent: 0,
                visibility_label: "mixed",
                overlay_label: "off",
                fog_label: "off",
            },
        );
        assert_eq!(
            overflowing_summary.detail_label(),
            "player= team=0 block= plans=0 markers=0 map=18446744073709551615x2 tiles=18446744073709551615 vis=mixed known=0 unknown=1 visible=0 hidden=0 overlay=off fog=off minimap=focus=none window-origin=0:0 window-size=0x0 window-area=0"
        );
    }

    #[test]
    fn hud_summary_visibility_label_covers_state_transitions() {
        let base = HudSummary {
            map_width: 2,
            map_height: 2,
            visible_tile_count: 0,
            hidden_tile_count: 0,
            ..hud_summary_fixture()
        };

        assert_eq!(base.visibility_label(), "unseen");
        assert_eq!(
            HudSummary {
                hidden_tile_count: 4,
                ..base.clone()
            }
            .visibility_label(),
            "hidden"
        );
        assert_eq!(
            HudSummary {
                visible_tile_count: 4,
                ..base.clone()
            }
            .visibility_label(),
            "clear"
        );
        assert_eq!(
            HudSummary {
                visible_tile_count: 2,
                hidden_tile_count: 2,
                ..base
            }
            .visibility_label(),
            "mapped"
        );
    }

    #[test]
    fn hud_summary_visibility_regression_reports_mapped_summary_when_all_tiles_are_known() {
        let summary = HudSummary {
            map_width: 5,
            map_height: 5,
            visible_tile_count: 12,
            hidden_tile_count: 13,
            ..hud_summary_fixture()
        };

        assert_hud_visibility(
            &summary,
            HudVisibilityExpectations {
                map_tile_count: 25,
                known_tile_count: 25,
                unknown_tile_count: 0,
                known_tile_percent: 100,
                unknown_tile_percent: 0,
                visible_map_percent: 48,
                hidden_map_percent: 52,
                visibility_label: "mapped",
                overlay_label: "off",
                fog_label: "off",
            },
        );
        assert_eq!(
            summary.summary_label(),
            "team=0 block= plans=0 markers=0 vis=mapped known=100 visible=48 overlay=off fog=off minimap=focus=none window=0:0+0x0"
        );
        assert_eq!(
            summary.detail_label(),
            "player= team=0 block= plans=0 markers=0 map=5x5 tiles=25 vis=mapped known=100 unknown=0 visible=48 hidden=52 overlay=off fog=off minimap=focus=none window-origin=0:0 window-size=0x0 window-area=0"
        );
    }

    #[test]
    fn runtime_live_entity_detail_label_surfaces_local_owned_unit_facets() {
        let entity = RuntimeLiveEntitySummaryObservability {
            entity_count: 12,
            hidden_count: 3,
            player_count: 2,
            unit_count: 1,
            player_with_owned_unit_count: 1,
            owned_unit_count: 2,
            ownership_conflict_count: 1,
            ownership_conflict_unit_sample: vec![202, 303],
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
            local_owned_unit_payload_sha256: Some("0123456789abcdef0123456789abcdef".to_string()),
            local_owned_unit_payload_nested_descendant_count: Some(2),
            local_owned_unit_payload_status: Some(RuntimePayloadSubtreeStatusObservability {
                total_count: 2,
                dynamic_count: 1,
                payload_with_status_count: 2,
                first_status_id: Some(13),
                first_status_name: Some("dynamic".to_string()),
                first_status_time_bits: Some(4.5f32.to_bits()),
                first_status_dynamic_field_count: Some(2),
            }),
            local_owned_carried_item_id: Some(6),
            local_owned_carried_item_amount: Some(4),
            local_owned_controller_type: Some(4),
            local_owned_controller_value: Some(101),
            local_owned_controller_detail: Some("legacy=1".to_string()),
            local_owned_controller_v2: Some(RuntimeUnitControllerObservability {
                controller_type: Some(4),
                controller_value: Some(101),
                target_position_tile: Some((12, 24)),
                attack_target: Some(RuntimeUnitControllerAttackTargetObservability {
                    kind: Some("b".to_string()),
                    value: Some(789),
                }),
                command_id: Some(4),
                command_queue: Some(RuntimeUnitControllerCommandQueueObservability {
                    total_count: 2,
                    building_count: 1,
                    unit_count: 0,
                    position_count: 0,
                    ignored_count: 1,
                }),
                stance_id: Some(9),
                status_count: Some(3),
            }),
            local_owned_unit_status_detail: Some("c=2:d=1:f=7/overdrive@5.5:fd=2".to_string()),
        };

        assert_eq!(entity.ownership_conflict_unit_sample_label(), "202,303");
        assert_eq!(entity.ownership_label(), "own=1/2:c1@202,303");
        assert_eq!(
            entity.local_owned_unit_payload_label(),
            "payload=count=2:unit=5/r7/l12:s0123456789ab"
        );
        assert_eq!(entity.local_owned_unit_nested_label(), "nested=2");
        assert_eq!(
            entity.local_owned_unit_payload_status_label(),
            "payload-status=c=2:d=1:n=2:f=13/dynamic@4.5:fd=2"
        );
        assert_eq!(entity.local_owned_unit_stack_label(), "stack=6x4");
        assert_eq!(
            entity.local_owned_unit_controller_label(),
            "controller=4/101:tp=12:24:atk=b/789:cmd=4:q=2/1/0/0/1:st=9:sts=3"
        );
        assert_eq!(
            entity.local_owned_unit_status_label(),
            "status=c=2:d=1:f=7/overdrive@5.5:fd=2"
        );
        assert_eq!(
            entity.detail_label(),
            "local=404 unit=2/999 pos=20.0:33.0 hidden=0 seen=7 players=2 units=1 own=1/2:c1@202,303 last=202/102/202 owned=202 payload=count=2:unit=5/r7/l12:s0123456789ab nested=2 payload-status=c=2:d=1:n=2:f=13/dynamic@4.5:fd=2 stack=6x4 controller=4/101:tp=12:24:atk=b/789:cmd=4:q=2/1/0/0/1:st=9:sts=3 status=c=2:d=1:f=7/overdrive@5.5:fd=2"
        );
    }

    #[test]
    fn runtime_live_entity_payload_status_label_defaults_to_none() {
        let entity = RuntimeLiveEntitySummaryObservability::default();

        assert_eq!(
            entity.local_owned_unit_payload_status_label(),
            "payload-status=none"
        );
    }

    #[test]
    fn runtime_unit_controller_observability_formats_detail_label() {
        let controller = RuntimeUnitControllerObservability {
            controller_type: Some(4),
            controller_value: Some(101),
            target_position_tile: Some((12, 24)),
            attack_target: Some(RuntimeUnitControllerAttackTargetObservability {
                kind: Some("b".to_string()),
                value: Some(789),
            }),
            command_id: Some(4),
            command_queue: Some(RuntimeUnitControllerCommandQueueObservability {
                total_count: 2,
                building_count: 1,
                unit_count: 0,
                position_count: 0,
                ignored_count: 1,
            }),
            stance_id: Some(9),
            status_count: Some(3),
        };

        assert_eq!(
            controller.detail_label(),
            "tp=12:24:atk=b/789:cmd=4:q=2/1/0/0/1:st=9:sts=3"
        );
    }

    #[test]
    fn runtime_live_entity_controller_label_falls_back_to_legacy_fields() {
        let entity = RuntimeLiveEntitySummaryObservability {
            local_owned_controller_type: Some(4),
            local_owned_controller_value: Some(101),
            local_owned_controller_detail: Some("cmd=4:q=2/1/0/0/1".to_string()),
            ..RuntimeLiveEntitySummaryObservability::default()
        };

        assert_eq!(
            entity.local_owned_unit_controller_label(),
            "controller=4/101:cmd=4:q=2/1/0/0/1"
        );
    }
}
