use crate::{
    hud_model::{
        runtime_menu_prompt_active, runtime_text_input_prompt_active,
        RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind, RuntimeSessionResetKind,
        RuntimeSessionTimeoutKind,
    },
    render_model::{RenderObjectSemanticFamily, RenderSemanticDetailCount},
    BuildConfigAuthoritySourceObservability, BuildConfigOutcomeObservability, BuildQueueHeadStage,
    HudModel, RenderModel,
};

const MINIMAP_TILE_SIZE: f32 = 8.0;

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
    pub window_clamped_left: bool,
    pub window_clamped_top: bool,
    pub window_clamped_right: bool,
    pub window_clamped_bottom: bool,
    pub window_tile_count: usize,
    pub window_coverage_percent: usize,
    pub map_tile_count: usize,
    pub known_tile_count: usize,
    pub known_tile_percent: usize,
    pub unknown_tile_count: usize,
    pub unknown_tile_percent: usize,
    pub focus_tile: Option<(usize, usize)>,
    pub focus_in_window: Option<bool>,
    pub focus_offset_x: Option<isize>,
    pub focus_offset_y: Option<isize>,
    pub overlay_visible: bool,
    pub fog_enabled: bool,
    pub visible_tile_count: usize,
    pub visible_known_percent: usize,
    pub hidden_tile_count: usize,
    pub hidden_known_percent: usize,
    pub tracked_object_count: usize,
    pub window_tracked_object_count: usize,
    pub outside_window_count: usize,
    pub player_count: usize,
    pub window_player_count: usize,
    pub marker_count: usize,
    pub window_marker_count: usize,
    pub plan_count: usize,
    pub window_plan_count: usize,
    pub block_count: usize,
    pub window_block_count: usize,
    pub runtime_count: usize,
    pub window_runtime_count: usize,
    pub terrain_count: usize,
    pub window_terrain_count: usize,
    pub unknown_count: usize,
    pub window_unknown_count: usize,
    pub detail_counts: Vec<RenderSemanticDetailCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct WindowSemanticCounts {
    total_count: usize,
    player_count: usize,
    marker_count: usize,
    plan_count: usize,
    block_count: usize,
    runtime_count: usize,
    terrain_count: usize,
    unknown_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudStatusPanelModel {
    pub player_name: String,
    pub team_id: u8,
    pub selected_block: String,
    pub plan_count: usize,
    pub marker_count: usize,
    pub map_width: usize,
    pub map_height: usize,
}

impl HudStatusPanelModel {
    pub fn map_tile_count(&self) -> usize {
        self.map_width.saturating_mul(self.map_height)
    }

    pub fn player_name_len(&self) -> usize {
        text_char_count(Some(self.player_name.as_str()))
    }

    pub fn selected_block_len(&self) -> usize {
        text_char_count(Some(self.selected_block.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudVisibilityPanelModel {
    pub overlay_visible: bool,
    pub fog_enabled: bool,
    pub visible_tile_count: usize,
    pub hidden_tile_count: usize,
    pub known_tile_count: usize,
    pub known_tile_percent: usize,
    pub visible_known_percent: usize,
    pub hidden_known_percent: usize,
    pub unknown_tile_count: usize,
    pub unknown_tile_percent: usize,
}

impl HudVisibilityPanelModel {
    pub fn visible_map_percent(&self) -> usize {
        percent_of(
            self.visible_tile_count,
            self.known_tile_count
                .saturating_add(self.unknown_tile_count),
        )
    }

    pub fn hidden_map_percent(&self) -> usize {
        percent_of(
            self.hidden_tile_count,
            self.known_tile_count
                .saturating_add(self.unknown_tile_count),
        )
    }
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
pub struct BuildMinimapAssistPanelModel {
    pub mode: BuildInteractionMode,
    pub selection_state: BuildInteractionSelectionState,
    pub queue_state: BuildInteractionQueueState,
    pub place_ready: bool,
    pub config_family_count: usize,
    pub config_sample_count: usize,
    pub top_config_family: Option<String>,
    pub authority_state: BuildInteractionAuthorityState,
    pub head_tile: Option<(i32, i32)>,
    pub authority_tile: Option<(i32, i32)>,
    pub authority_source: Option<BuildConfigAuthoritySourceObservability>,
    pub focus_tile: Option<(usize, usize)>,
    pub focus_in_window: Option<bool>,
    pub visible_map_percent: usize,
    pub unknown_tile_percent: usize,
    pub window_coverage_percent: usize,
    pub tracked_object_count: usize,
    pub runtime_count: usize,
}

#[cfg_attr(not(test), allow(dead_code))]
impl BuildMinimapAssistPanelModel {
    pub fn focus_state_label(&self) -> &'static str {
        match (self.focus_tile, self.focus_in_window) {
            (Some(_), Some(true)) => "inside",
            (Some(_), Some(false)) => "outside",
            (Some(_), None) => "tracked",
            (None, _) => "none",
        }
    }

    pub fn map_visibility_label(&self) -> &'static str {
        if self.unknown_tile_percent == 100 {
            "unseen"
        } else if self.visible_map_percent == 0 {
            "hidden"
        } else if self.unknown_tile_percent == 0 {
            "mapped"
        } else {
            "mixed"
        }
    }

    pub fn window_coverage_label(&self) -> &'static str {
        if self.window_coverage_percent == 0 {
            "offscreen"
        } else if self.window_coverage_percent == 100 {
            "full"
        } else {
            "partial"
        }
    }

    pub fn window_object_density_percent(&self, window_tile_count: usize) -> usize {
        percent_of(self.tracked_object_count, window_tile_count)
    }

    pub fn config_scope_label(&self) -> &'static str {
        match self.config_family_count {
            0 => "none",
            1 => "single",
            _ => "multi",
        }
    }

    pub fn runtime_share_percent(&self) -> usize {
        percent_of(self.runtime_count, self.tracked_object_count)
    }

    pub fn next_action_label(&self) -> &'static str {
        match self.mode {
            BuildInteractionMode::Idle => "idle",
            BuildInteractionMode::Break => {
                if self.focus_tile.is_none() || matches!(self.focus_in_window, Some(false)) {
                    "refocus"
                } else {
                    "break"
                }
            }
            BuildInteractionMode::Place => {
                if !self.place_ready {
                    "arm"
                } else if matches!(
                    self.selection_state,
                    BuildInteractionSelectionState::HeadDiverged
                ) {
                    "realign"
                } else if matches!(self.queue_state, BuildInteractionQueueState::Empty) {
                    "seed"
                } else if self.authority_needs_attention() {
                    "resolve"
                } else if self.focus_tile.is_none() || matches!(self.focus_in_window, Some(false)) {
                    "refocus"
                } else if matches!(self.map_visibility_label(), "unseen" | "hidden") {
                    "survey"
                } else {
                    "commit"
                }
            }
        }
    }

    fn authority_needs_attention(&self) -> bool {
        !matches!(
            self.authority_state,
            BuildInteractionAuthorityState::None | BuildInteractionAuthorityState::Applied
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeUiNoticePanelModel {
    pub hud_set_count: u64,
    pub hud_set_reliable_count: u64,
    pub hud_hide_count: u64,
    pub hud_last_message: Option<String>,
    pub hud_last_reliable_message: Option<String>,
    pub announce_count: u64,
    pub last_announce_message: Option<String>,
    pub info_message_count: u64,
    pub last_info_message: Option<String>,
    pub toast_info_count: u64,
    pub toast_warning_count: u64,
    pub toast_last_info_message: Option<String>,
    pub toast_last_warning_text: Option<String>,
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
    pub text_input_open_count: u64,
    pub text_input_last_id: Option<i32>,
    pub text_input_last_title: Option<String>,
    pub text_input_last_default_text: Option<String>,
    pub text_input_last_length: Option<i32>,
    pub text_input_last_numeric: Option<bool>,
    pub text_input_last_allow_empty: Option<bool>,
}

impl RuntimeMenuPanelModel {
    pub fn is_empty(&self) -> bool {
        self.menu_open_count == 0
            && self.follow_up_menu_open_count == 0
            && self.hide_follow_up_menu_count == 0
            && self.last_menu_open_id.is_none()
            && self.last_menu_open_title.is_none()
            && self.last_menu_open_message.is_none()
            && self.last_menu_open_option_rows == 0
            && self.last_menu_open_first_row_len == 0
            && self.last_follow_up_menu_open_id.is_none()
            && self.last_follow_up_menu_open_title.is_none()
            && self.last_follow_up_menu_open_message.is_none()
            && self.last_follow_up_menu_open_option_rows == 0
            && self.last_follow_up_menu_open_first_row_len == 0
            && self.last_hide_follow_up_menu_id.is_none()
            && self.text_input_open_count == 0
            && self.text_input_last_id.is_none()
            && self.text_input_last_title.is_none()
            && self.text_input_last_default_text.is_none()
            && self.text_input_last_length.is_none()
            && self.text_input_last_numeric.is_none()
            && self.text_input_last_allow_empty.is_none()
    }

    pub fn outstanding_follow_up_count(&self) -> u64 {
        self.follow_up_menu_open_count
            .saturating_sub(self.hide_follow_up_menu_count)
    }

    pub fn default_text_len(&self) -> usize {
        text_char_count(self.text_input_last_default_text.as_deref())
    }

    pub fn menu_title_len(&self) -> usize {
        text_char_count(self.last_menu_open_title.as_deref())
    }

    pub fn menu_message_len(&self) -> usize {
        text_char_count(self.last_menu_open_message.as_deref())
    }

    pub fn follow_up_title_len(&self) -> usize {
        text_char_count(self.last_follow_up_menu_open_title.as_deref())
    }

    pub fn follow_up_message_len(&self) -> usize {
        text_char_count(self.last_follow_up_menu_open_message.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeChoicePanelModel {
    pub menu_choose_count: u64,
    pub last_menu_choose_menu_id: Option<i32>,
    pub last_menu_choose_option: Option<i32>,
    pub text_input_result_count: u64,
    pub last_text_input_result_id: Option<i32>,
    pub last_text_input_result_text: Option<String>,
}

impl RuntimeChoicePanelModel {
    pub fn is_empty(&self) -> bool {
        self.menu_choose_count == 0
            && self.last_menu_choose_menu_id.is_none()
            && self.last_menu_choose_option.is_none()
            && self.text_input_result_count == 0
            && self.last_text_input_result_id.is_none()
            && self.last_text_input_result_text.is_none()
    }

    pub fn text_input_result_len(&self) -> usize {
        text_char_count(self.last_text_input_result_text.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeChatPanelModel {
    pub server_message_count: u64,
    pub last_server_message: Option<String>,
    pub chat_message_count: u64,
    pub last_chat_message: Option<String>,
    pub last_chat_unformatted: Option<String>,
    pub last_chat_sender_entity_id: Option<i32>,
}

impl RuntimeChatPanelModel {
    pub fn is_empty(&self) -> bool {
        self.server_message_count == 0
            && self.last_server_message.is_none()
            && self.chat_message_count == 0
            && self.last_chat_message.is_none()
            && self.last_chat_unformatted.is_none()
            && self.last_chat_sender_entity_id.is_none()
    }

    pub fn last_server_message_len(&self) -> usize {
        text_char_count(self.last_server_message.as_deref())
    }

    pub fn last_chat_message_len(&self) -> usize {
        text_char_count(self.last_chat_message.as_deref())
    }

    pub fn last_chat_unformatted_len(&self) -> usize {
        text_char_count(self.last_chat_unformatted.as_deref())
    }

    pub fn formatted_matches_unformatted(&self) -> Option<bool> {
        match (
            self.last_chat_message.as_deref(),
            self.last_chat_unformatted.as_deref(),
        ) {
            (Some(formatted), Some(unformatted)) => Some(formatted == unformatted),
            _ => None,
        }
    }
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

impl RuntimeDialogPanelModel {
    pub fn is_empty(&self) -> bool {
        self.prompt_kind.is_none()
            && !self.prompt_active
            && self.menu_open_count == 0
            && self.follow_up_menu_open_count == 0
            && self.hide_follow_up_menu_count == 0
            && self.text_input_open_count == 0
            && self.text_input_last_id.is_none()
            && self.text_input_last_title.is_none()
            && self.text_input_last_message.is_none()
            && self.text_input_last_default_text.is_none()
            && self.text_input_last_length.is_none()
            && self.text_input_last_numeric.is_none()
            && self.text_input_last_allow_empty.is_none()
            && self.notice_kind.is_none()
            && self.notice_text.is_none()
            && self.notice_count == 0
    }

    pub fn outstanding_follow_up_count(&self) -> u64 {
        self.follow_up_menu_open_count
            .saturating_sub(self.hide_follow_up_menu_count)
    }

    pub fn prompt_message_len(&self) -> usize {
        text_char_count(self.text_input_last_message.as_deref())
    }

    pub fn default_text_len(&self) -> usize {
        text_char_count(self.text_input_last_default_text.as_deref())
    }

    pub fn notice_text_len(&self) -> usize {
        text_char_count(self.notice_text.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePromptPanelModel {
    pub kind: Option<RuntimeDialogPromptKind>,
    pub menu_active: bool,
    pub text_input_active: bool,
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
}

#[cfg_attr(not(test), allow(dead_code))]
impl RuntimePromptPanelModel {
    pub fn is_empty(&self) -> bool {
        self.kind.is_none()
            && !self.menu_active
            && !self.text_input_active
            && self.menu_open_count == 0
            && self.follow_up_menu_open_count == 0
            && self.hide_follow_up_menu_count == 0
            && self.text_input_open_count == 0
            && self.text_input_last_id.is_none()
            && self.text_input_last_title.is_none()
            && self.text_input_last_message.is_none()
            && self.text_input_last_default_text.is_none()
            && self.text_input_last_length.is_none()
            && self.text_input_last_numeric.is_none()
            && self.text_input_last_allow_empty.is_none()
    }

    pub fn is_active(&self) -> bool {
        self.text_input_active || self.outstanding_follow_up_count() > 0 || self.menu_active
    }

    pub fn menu_active(&self) -> bool {
        self.menu_active
    }

    pub fn text_input_active(&self) -> bool {
        self.text_input_active
    }

    pub fn outstanding_follow_up_count(&self) -> u64 {
        self.follow_up_menu_open_count
            .saturating_sub(self.hide_follow_up_menu_count)
    }

    pub fn layer_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.text_input_active() {
            labels.push("input");
        }
        if self.outstanding_follow_up_count() > 0 {
            labels.push("follow-up");
        }
        if self.menu_active() {
            labels.push("menu");
        }
        labels
    }

    pub fn depth(&self) -> usize {
        self.layer_labels().len()
    }

    pub fn prompt_message_len(&self) -> usize {
        text_char_count(self.text_input_last_message.as_deref())
    }

    pub fn default_text_len(&self) -> usize {
        text_char_count(self.text_input_last_default_text.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeNoticeStatePanelModel {
    pub kind: Option<RuntimeDialogNoticeKind>,
    pub text: Option<String>,
    pub count: u64,
    pub hud_active: bool,
    pub reliable_hud_active: bool,
    pub toast_info_active: bool,
    pub toast_warning_active: bool,
}

#[cfg_attr(not(test), allow(dead_code))]
impl RuntimeNoticeStatePanelModel {
    pub fn is_empty(&self) -> bool {
        self.kind.is_none()
            && self.text.is_none()
            && self.count == 0
            && !self.hud_active
            && !self.reliable_hud_active
            && !self.toast_info_active
            && !self.toast_warning_active
    }

    pub fn is_active(&self) -> bool {
        self.depth() > 0
    }

    pub fn layer_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.hud_active {
            labels.push("hud");
        }
        if self.reliable_hud_active {
            labels.push("reliable");
        }
        if self.toast_info_active {
            labels.push("info");
        }
        if self.toast_warning_active {
            labels.push("warn");
        }
        labels
    }

    pub fn depth(&self) -> usize {
        self.layer_labels().len()
    }

    pub fn text_len(&self) -> usize {
        text_char_count(self.text.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeUiStackForegroundKind {
    Menu,
    FollowUpMenu,
    TextInput,
    Chat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDialogStackPanelModel {
    pub foreground_kind: Option<RuntimeUiStackForegroundKind>,
    pub prompt: RuntimePromptPanelModel,
    pub notice: RuntimeNoticeStatePanelModel,
    pub chat: RuntimeChatPanelModel,
}

impl RuntimeDialogStackPanelModel {
    pub fn is_empty(&self) -> bool {
        self.prompt.is_empty() && self.notice.is_empty() && self.chat.is_empty()
    }

    pub fn foreground_label(&self) -> &'static str {
        match self.foreground_kind {
            Some(RuntimeUiStackForegroundKind::Menu) => "menu",
            Some(RuntimeUiStackForegroundKind::FollowUpMenu) => "follow-up",
            Some(RuntimeUiStackForegroundKind::TextInput) => "input",
            Some(RuntimeUiStackForegroundKind::Chat) => "chat",
            None => "none",
        }
    }

    pub fn prompt_depth(&self) -> usize {
        self.prompt.depth()
    }

    pub fn notice_depth(&self) -> usize {
        self.notice.depth()
    }

    pub fn chat_depth(&self) -> usize {
        usize::from(!self.chat.is_empty())
    }

    pub fn active_group_count(&self) -> usize {
        usize::from(self.prompt_depth() > 0)
            + usize::from(self.notice_depth() > 0)
            + self.chat_depth()
    }

    pub fn total_depth(&self) -> usize {
        self.prompt_depth() + self.notice_depth() + self.chat_depth()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeUiStackPanelModel {
    pub foreground_kind: Option<RuntimeUiStackForegroundKind>,
    pub menu_active: bool,
    pub outstanding_follow_up_count: u64,
    pub text_input_active: bool,
    pub text_input_open_count: u64,
    pub text_input_last_id: Option<i32>,
    pub notice_kind: Option<RuntimeDialogNoticeKind>,
    pub hud_notice_active: bool,
    pub reliable_hud_notice_active: bool,
    pub toast_info_active: bool,
    pub toast_warning_active: bool,
    pub chat_active: bool,
    pub server_message_count: u64,
    pub chat_message_count: u64,
    pub last_chat_sender_entity_id: Option<i32>,
}

impl RuntimeUiStackPanelModel {
    pub fn is_empty(&self) -> bool {
        self.foreground_kind.is_none()
            && !self.menu_active
            && self.outstanding_follow_up_count == 0
            && !self.text_input_active
            && self.text_input_open_count == 0
            && self.text_input_last_id.is_none()
            && self.notice_kind.is_none()
            && !self.hud_notice_active
            && !self.reliable_hud_notice_active
            && !self.toast_info_active
            && !self.toast_warning_active
            && !self.chat_active
            && self.server_message_count == 0
            && self.chat_message_count == 0
            && self.last_chat_sender_entity_id.is_none()
    }

    pub fn foreground_label(&self) -> &'static str {
        match self.foreground_kind {
            Some(RuntimeUiStackForegroundKind::Menu) => "menu",
            Some(RuntimeUiStackForegroundKind::FollowUpMenu) => "follow-up",
            Some(RuntimeUiStackForegroundKind::TextInput) => "input",
            Some(RuntimeUiStackForegroundKind::Chat) => "chat",
            None => "none",
        }
    }

    pub fn prompt_layer_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.text_input_active {
            labels.push("input");
        }
        if self.outstanding_follow_up_count > 0 {
            labels.push("follow-up");
        }
        if self.menu_active {
            labels.push("menu");
        }
        labels
    }

    pub fn notice_layer_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.hud_notice_active {
            labels.push("hud");
        }
        if self.reliable_hud_notice_active {
            labels.push("reliable");
        }
        if self.toast_info_active {
            labels.push("info");
        }
        if self.toast_warning_active {
            labels.push("warn");
        }
        labels
    }

    pub fn prompt_depth(&self) -> usize {
        self.prompt_layer_labels().len()
    }

    pub fn notice_depth(&self) -> usize {
        self.notice_layer_labels().len()
    }

    pub fn chat_depth(&self) -> usize {
        usize::from(self.chat_active)
    }

    pub fn total_depth(&self) -> usize {
        self.prompt_depth() + self.notice_depth() + self.chat_depth()
    }

    pub fn active_group_count(&self) -> usize {
        usize::from(self.prompt_depth() > 0)
            + usize::from(self.notice_depth() > 0)
            + usize::from(self.chat_active)
    }
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

impl RuntimeAdminPanelModel {
    pub fn is_empty(&self) -> bool {
        self.trace_info_count == 0
            && self.trace_info_parse_fail_count == 0
            && self.last_trace_info_player_id.is_none()
            && self.debug_status_client_count == 0
            && self.debug_status_client_parse_fail_count == 0
            && self.debug_status_client_unreliable_count == 0
            && self.debug_status_client_unreliable_parse_fail_count == 0
            && self.last_debug_status_value.is_none()
            && self.parse_fail_count == 0
    }
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

impl RuntimeRulesPanelModel {
    pub fn is_empty(&self) -> bool {
        self.mutation_count == 0
            && self.parse_fail_count == 0
            && self.set_rules_count == 0
            && self.set_objectives_count == 0
            && self.set_rule_count == 0
            && self.clear_objectives_count == 0
            && self.complete_objective_count == 0
            && self.waves.is_none()
            && self.pvp.is_none()
            && self.objective_count == 0
            && self.qualified_objective_count == 0
            && self.objective_parent_edge_count == 0
            && self.objective_flag_count == 0
            && self.complete_out_of_range_count == 0
            && self.last_completed_index.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorldLabelPanelModel {
    pub label_count: u64,
    pub reliable_label_count: u64,
    pub remove_label_count: u64,
    pub total_count: u64,
    pub active_count: usize,
    pub inactive_count: usize,
    pub last_entity_id: Option<i32>,
    pub last_text: Option<String>,
    pub last_flags: Option<u8>,
    pub last_font_size_bits: Option<u32>,
    pub last_z_bits: Option<u32>,
    pub last_position: Option<crate::RuntimeWorldPositionObservability>,
}

impl RuntimeWorldLabelPanelModel {
    pub fn inactive_count(&self) -> usize {
        self.inactive_count
    }

    pub fn last_text_len(&self) -> usize {
        self.last_text
            .as_deref()
            .map(|text| text.chars().count())
            .unwrap_or(0)
    }

    pub fn last_text_line_count(&self) -> usize {
        self.last_text
            .as_deref()
            .map(|text| text.split('\n').count())
            .unwrap_or(0)
    }

    pub fn last_font_size(&self) -> Option<f32> {
        finite_f32_bits(self.last_font_size_bits)
    }

    pub fn last_z(&self) -> Option<f32> {
        finite_f32_bits(self.last_z_bits)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMarkerPanelModel {
    pub create_count: u64,
    pub remove_count: u64,
    pub update_count: u64,
    pub update_text_count: u64,
    pub update_texture_count: u64,
    pub decode_fail_count: u64,
    pub last_marker_id: Option<i32>,
    pub last_control_name: Option<String>,
}

impl RuntimeMarkerPanelModel {
    pub fn is_empty(&self) -> bool {
        self.create_count == 0
            && self.remove_count == 0
            && self.update_count == 0
            && self.update_text_count == 0
            && self.update_texture_count == 0
            && self.decode_fail_count == 0
            && self.last_marker_id.is_none()
            && self.last_control_name.is_none()
    }

    pub fn total_count(&self) -> u64 {
        self.create_count
            .saturating_add(self.remove_count)
            .saturating_add(self.update_count)
            .saturating_add(self.update_text_count)
            .saturating_add(self.update_texture_count)
    }

    pub fn mutate_count(&self) -> u64 {
        self.create_count
            .saturating_add(self.remove_count)
            .saturating_add(self.update_count)
    }

    pub fn control_name_len(&self) -> usize {
        text_char_count(self.last_control_name.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCoreBindingPanelModel {
    pub kind: Option<crate::RuntimeCoreBindingKindObservability>,
    pub ambiguous_team_count: usize,
    pub ambiguous_team_sample: Vec<u8>,
    pub missing_team_count: usize,
    pub missing_team_sample: Vec<u8>,
}

impl RuntimeCoreBindingPanelModel {
    pub fn is_empty(&self) -> bool {
        self.kind.is_none()
            && self.ambiguous_team_count == 0
            && self.ambiguous_team_sample.is_empty()
            && self.missing_team_count == 0
            && self.missing_team_sample.is_empty()
    }

    pub fn kind_label(&self) -> &'static str {
        self.kind.map(|kind| kind.label()).unwrap_or("none")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBootstrapPanelModel {
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

impl From<&crate::hud_model::RuntimeBootstrapObservability> for RuntimeBootstrapPanelModel {
    fn from(value: &crate::hud_model::RuntimeBootstrapObservability) -> Self {
        Self {
            rules_label: value.rules_label.clone(),
            tags_label: value.tags_label.clone(),
            locales_label: value.locales_label.clone(),
            team_count: value.team_count,
            marker_count: value.marker_count,
            custom_chunk_count: value.custom_chunk_count,
            content_patch_count: value.content_patch_count,
            player_team_plan_count: value.player_team_plan_count,
            static_fog_team_count: value.static_fog_team_count,
        }
    }
}

impl RuntimeBootstrapPanelModel {
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

impl MinimapPanelModel {
    pub fn visible_map_percent(&self) -> usize {
        percent_of(self.visible_tile_count, self.map_tile_count)
    }

    pub fn hidden_map_percent(&self) -> usize {
        percent_of(self.hidden_tile_count, self.map_tile_count)
    }

    pub fn map_object_density_percent(&self) -> usize {
        percent_of(self.tracked_object_count, self.map_tile_count)
    }

    pub fn window_object_density_percent(&self) -> usize {
        percent_of(self.window_tracked_object_count, self.window_tile_count)
    }

    pub fn outside_object_percent(&self) -> usize {
        percent_of(self.outside_window_count, self.tracked_object_count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSessionPanelModel {
    pub bootstrap: RuntimeBootstrapPanelModel,
    pub core_binding: RuntimeCoreBindingPanelModel,
    pub resource_delta: RuntimeResourceDeltaPanelModel,
    pub kick: RuntimeKickPanelModel,
    pub loading: RuntimeLoadingPanelModel,
    pub reconnect: RuntimeReconnectPanelModel,
}

impl RuntimeSessionPanelModel {
    pub fn is_empty(&self) -> bool {
        self.bootstrap.is_empty()
            && self.resource_delta.is_empty()
            && self.kick.is_empty()
            && self.loading.is_empty()
            && self.reconnect.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeResourceDeltaPanelModel {
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
    pub last_unit: Option<crate::RuntimeCommandUnitRefObservability>,
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

impl RuntimeResourceDeltaPanelModel {
    pub fn is_empty(&self) -> bool {
        self.remove_tile_count == 0
            && self.set_tile_count == 0
            && self.set_floor_count == 0
            && self.set_overlay_count == 0
            && self.set_item_count == 0
            && self.set_items_count == 0
            && self.set_liquid_count == 0
            && self.set_liquids_count == 0
            && self.clear_items_count == 0
            && self.clear_liquids_count == 0
            && self.set_tile_items_count == 0
            && self.set_tile_liquids_count == 0
            && self.take_items_count == 0
            && self.transfer_item_to_count == 0
            && self.transfer_item_to_unit_count == 0
            && self.last_kind.is_none()
            && self.last_item_id.is_none()
            && self.last_amount.is_none()
            && self.last_build_pos.is_none()
            && self.last_unit.is_none()
            && self.last_to_entity_id.is_none()
            && self.build_count == 0
            && self.build_stack_count == 0
            && self.entity_count == 0
            && self.authoritative_build_update_count == 0
            && self.delta_apply_count == 0
            && self.delta_skip_count == 0
            && self.delta_conflict_count == 0
            && self.last_changed_build_pos.is_none()
            && self.last_changed_entity_id.is_none()
            && self.last_changed_item_id.is_none()
            && self.last_changed_amount.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeKickPanelModel {
    pub reason_text: Option<String>,
    pub reason_ordinal: Option<i32>,
    pub hint_category: Option<String>,
    pub hint_text: Option<String>,
}

impl RuntimeKickPanelModel {
    pub fn is_empty(&self) -> bool {
        self.reason_text.is_none()
            && self.reason_ordinal.is_none()
            && self.hint_category.is_none()
            && self.hint_text.is_none()
    }
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

impl RuntimeLoadingPanelModel {
    pub fn is_empty(&self) -> bool {
        self.deferred_inbound_packet_count == 0
            && self.replayed_inbound_packet_count == 0
            && self.dropped_loading_low_priority_packet_count == 0
            && self.dropped_loading_deferred_overflow_count == 0
            && self.failed_state_snapshot_parse_count == 0
            && self.failed_state_snapshot_core_data_parse_count == 0
            && self.failed_entity_snapshot_parse_count == 0
            && self.ready_inbound_liveness_anchor_count == 0
            && self.last_ready_inbound_liveness_anchor_at_ms.is_none()
            && self.timeout_count == 0
            && self.connect_or_loading_timeout_count == 0
            && self.ready_snapshot_timeout_count == 0
            && self.last_timeout_kind.is_none()
            && self.last_timeout_idle_ms.is_none()
            && self.reset_count == 0
            && self.reconnect_reset_count == 0
            && self.world_reload_count == 0
            && self.kick_reset_count == 0
            && self.last_reset_kind.is_none()
            && self.last_world_reload.is_none()
    }
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

impl RuntimeReconnectPanelModel {
    pub fn is_empty(&self) -> bool {
        self.phase == RuntimeReconnectPhaseObservability::Idle
            && self.phase_transition_count == 0
            && self.reason_kind.is_none()
            && self.reason_text.is_none()
            && self.reason_ordinal.is_none()
            && self.hint_text.is_none()
            && self.redirect_count == 0
            && self.last_redirect_ip.is_none()
            && self.last_redirect_port.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveEntityPanelModel {
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
    pub local_position: Option<crate::RuntimeWorldPositionObservability>,
}

impl RuntimeLiveEntityPanelModel {
    pub fn local_owned_unit_payload_label(&self) -> String {
        format!(
            "payload=unit={}/{}",
            optional_u8_label(self.local_unit_kind),
            optional_u32_label(self.local_unit_value),
        )
    }

    pub fn local_owned_unit_nested_label(&self) -> String {
        format!(
            "nested=snapshot={}",
            optional_u64_label(self.local_last_seen_entity_snapshot_count),
        )
    }

    pub fn local_owned_unit_stack_label(&self) -> String {
        format!(
            "stack=entities={} hidden={} players={} units={} last={}/{}/{}",
            self.entity_count,
            self.hidden_count,
            self.player_count,
            self.unit_count,
            optional_i32_label(self.last_entity_id),
            optional_i32_label(self.last_player_entity_id),
            optional_i32_label(self.last_unit_entity_id),
        )
    }

    pub fn local_owned_unit_controller_label(&self) -> String {
        format!(
            "controller=entity={} pos={} hidden={}",
            optional_i32_label(self.local_entity_id),
            world_position_text(self.local_position.as_ref()),
            optional_bool_label(self.local_hidden),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "local={} {} {} {} {}",
            optional_i32_label(self.local_entity_id),
            self.local_owned_unit_payload_label(),
            self.local_owned_unit_nested_label(),
            self.local_owned_unit_stack_label(),
            self.local_owned_unit_controller_label(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveEffectPanelModel {
    pub effect_count: u64,
    pub spawn_effect_count: u64,
    pub active_overlay_count: usize,
    pub active_effect_id: Option<i16>,
    pub active_contract_name: Option<String>,
    pub active_reliable: Option<bool>,
    pub active_position: Option<crate::RuntimeWorldPositionObservability>,
    pub last_effect_id: Option<i16>,
    pub last_spawn_effect_unit_type_id: Option<i16>,
    pub last_kind: Option<String>,
    pub last_contract_name: Option<String>,
    pub last_reliable_contract_name: Option<String>,
    pub last_business_hint: Option<String>,
    pub last_position_hint: Option<crate::RuntimeWorldPositionObservability>,
    pub last_position_source: Option<crate::RuntimeLiveEffectPositionSource>,
}

impl RuntimeLiveEffectPanelModel {
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

    pub fn display_position_source(&self) -> Option<crate::RuntimeLiveEffectPositionSource> {
        if self.active_position.is_some() {
            Some(crate::RuntimeLiveEffectPositionSource::ActiveOverlay)
        } else {
            self.last_position_source
        }
    }

    pub fn display_position(&self) -> Option<&crate::RuntimeWorldPositionObservability> {
        self.active_position
            .as_ref()
            .or(self.last_position_hint.as_ref())
    }
}

pub fn build_hud_status_panel(hud: &HudModel) -> Option<HudStatusPanelModel> {
    let summary = hud.summary.as_ref()?;
    Some(HudStatusPanelModel {
        player_name: summary.player_name.clone(),
        team_id: summary.team_id,
        selected_block: summary.selected_block.clone(),
        plan_count: summary.plan_count,
        marker_count: summary.marker_count,
        map_width: summary.map_width,
        map_height: summary.map_height,
    })
}

pub fn build_hud_visibility_panel(hud: &HudModel) -> Option<HudVisibilityPanelModel> {
    let summary = hud.summary.as_ref()?;
    let map_tile_count = summary.map_width.saturating_mul(summary.map_height);
    let known_tile_count = summary
        .visible_tile_count
        .saturating_add(summary.hidden_tile_count);
    let unknown_tile_count = map_tile_count.saturating_sub(known_tile_count);
    Some(HudVisibilityPanelModel {
        overlay_visible: summary.overlay_visible,
        fog_enabled: summary.fog_enabled,
        visible_tile_count: summary.visible_tile_count,
        hidden_tile_count: summary.hidden_tile_count,
        known_tile_count,
        known_tile_percent: percent_of(known_tile_count, map_tile_count),
        visible_known_percent: percent_of(summary.visible_tile_count, known_tile_count),
        hidden_known_percent: percent_of(summary.hidden_tile_count, known_tile_count),
        unknown_tile_count,
        unknown_tile_percent: percent_of(unknown_tile_count, map_tile_count),
    })
}

pub fn build_minimap_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<MinimapPanelModel> {
    let summary = hud.summary.as_ref()?;
    let window = resolve_presenter_window(
        scene,
        summary.map_width,
        summary.map_height,
        summary.minimap.view_window,
        window,
    );
    let semantics = scene.semantic_summary();
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
    let focus_tile = summary.minimap.focus_tile;
    let window_semantics = minimap_window_semantic_counts(scene, window);
    let window_mid_x = window.origin_x.saturating_add(window_last_x) / 2;
    let window_mid_y = window.origin_y.saturating_add(window_last_y) / 2;
    let focus_in_window = focus_tile.map(|(focus_x, focus_y)| {
        focus_x >= window.origin_x
            && focus_x <= window_last_x
            && focus_y >= window.origin_y
            && focus_y <= window_last_y
    });
    let (focus_offset_x, focus_offset_y) = focus_tile
        .map(|(focus_x, focus_y)| {
            (
                focus_x as isize - window_mid_x as isize,
                focus_y as isize - window_mid_y as isize,
            )
        })
        .unzip();

    Some(MinimapPanelModel {
        map_width: summary.map_width,
        map_height: summary.map_height,
        window,
        window_last_x,
        window_last_y,
        window_clamped_left: window.origin_x == 0,
        window_clamped_top: window.origin_y == 0,
        window_clamped_right: window_last_x.saturating_add(1) >= summary.map_width,
        window_clamped_bottom: window_last_y.saturating_add(1) >= summary.map_height,
        window_tile_count,
        window_coverage_percent: percent_of(window_tile_count, map_tile_count),
        map_tile_count,
        known_tile_count,
        known_tile_percent: percent_of(known_tile_count, map_tile_count),
        unknown_tile_count,
        unknown_tile_percent: percent_of(unknown_tile_count, map_tile_count),
        focus_tile,
        focus_in_window,
        focus_offset_x,
        focus_offset_y,
        overlay_visible: summary.overlay_visible,
        fog_enabled: summary.fog_enabled,
        visible_tile_count: summary.visible_tile_count,
        visible_known_percent: percent_of(summary.visible_tile_count, known_tile_count),
        hidden_tile_count: summary.hidden_tile_count,
        hidden_known_percent: percent_of(summary.hidden_tile_count, known_tile_count),
        tracked_object_count: semantics.total_count,
        window_tracked_object_count: window_semantics.total_count,
        outside_window_count: semantics
            .total_count
            .saturating_sub(window_semantics.total_count),
        player_count: semantics.player_count,
        window_player_count: window_semantics.player_count,
        marker_count: semantics.marker_count,
        window_marker_count: window_semantics.marker_count,
        plan_count: semantics.plan_count,
        window_plan_count: window_semantics.plan_count,
        block_count: semantics.block_count,
        window_block_count: window_semantics.block_count,
        runtime_count: semantics.runtime_count,
        window_runtime_count: window_semantics.runtime_count,
        terrain_count: semantics.terrain_count,
        window_terrain_count: window_semantics.terrain_count,
        unknown_count: semantics.unknown_count,
        window_unknown_count: window_semantics.unknown_count,
        detail_counts: semantics.detail_counts,
    })
}

fn clamp_presenter_window_to_map(
    window: PresenterViewWindow,
    map_width: usize,
    map_height: usize,
) -> PresenterViewWindow {
    let width = window.width.min(map_width);
    let height = window.height.min(map_height);
    let max_origin_x = map_width.saturating_sub(width);
    let max_origin_y = map_height.saturating_sub(height);

    PresenterViewWindow {
        origin_x: window.origin_x.min(max_origin_x),
        origin_y: window.origin_y.min(max_origin_y),
        width,
        height,
    }
}

fn minimap_window_semantic_counts(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> WindowSemanticCounts {
    let mut counts = WindowSemanticCounts::default();
    let window_last_x = window
        .origin_x
        .saturating_add(window.width.saturating_sub(1));
    let window_last_y = window
        .origin_y
        .saturating_add(window.height.saturating_sub(1));

    for object in &scene.objects {
        let tile_x = world_to_tile_index_floor(object.x, MINIMAP_TILE_SIZE);
        let tile_y = world_to_tile_index_floor(object.y, MINIMAP_TILE_SIZE);
        if tile_x < window.origin_x as i32
            || tile_x > window_last_x as i32
            || tile_y < window.origin_y as i32
            || tile_y > window_last_y as i32
        {
            continue;
        }

        counts.total_count += 1;
        match object.semantic_family() {
            RenderObjectSemanticFamily::Player => counts.player_count += 1,
            RenderObjectSemanticFamily::Marker => counts.marker_count += 1,
            RenderObjectSemanticFamily::Plan => counts.plan_count += 1,
            RenderObjectSemanticFamily::Block => counts.block_count += 1,
            RenderObjectSemanticFamily::Runtime => counts.runtime_count += 1,
            RenderObjectSemanticFamily::Terrain => counts.terrain_count += 1,
            RenderObjectSemanticFamily::Unknown => counts.unknown_count += 1,
        }
    }

    counts
}

fn world_to_tile_index_floor(world_position: f32, tile_size: f32) -> i32 {
    if !world_position.is_finite() || !tile_size.is_finite() || tile_size <= 0.0 {
        return -1;
    }
    (world_position / tile_size).floor() as i32
}

fn percent_of(part: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        part.saturating_mul(100) / total
    }
}

fn text_char_count(value: Option<&str>) -> usize {
    value.map(|text| text.chars().count()).unwrap_or(0)
}

fn finite_f32_bits(bits: Option<u32>) -> Option<f32> {
    bits.map(f32::from_bits).filter(|value| value.is_finite())
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

pub fn build_build_minimap_assist_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<BuildMinimapAssistPanelModel> {
    let interaction = build_build_interaction_panel(hud)?;
    let minimap = build_minimap_panel(scene, hud, window)?;
    Some(BuildMinimapAssistPanelModel {
        mode: interaction.mode,
        selection_state: interaction.selection_state,
        queue_state: interaction.queue_state,
        place_ready: interaction.place_ready,
        config_family_count: interaction.config_family_count,
        config_sample_count: interaction.config_sample_count,
        top_config_family: interaction.top_config_family,
        authority_state: interaction.authority_state,
        head_tile: interaction.head.as_ref().map(|head| (head.x, head.y)),
        authority_tile: interaction.authority_tile,
        authority_source: interaction.authority_source,
        focus_tile: minimap.focus_tile,
        focus_in_window: minimap.focus_in_window,
        visible_map_percent: minimap.visible_map_percent(),
        unknown_tile_percent: minimap.unknown_tile_percent,
        window_coverage_percent: minimap.window_coverage_percent,
        tracked_object_count: minimap.tracked_object_count,
        runtime_count: minimap.runtime_count,
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
        announce_count: runtime_ui.hud_text.announce_count,
        last_announce_message: runtime_ui.hud_text.last_announce_message.clone(),
        info_message_count: runtime_ui.hud_text.info_message_count,
        last_info_message: runtime_ui.hud_text.last_info_message.clone(),
        toast_info_count: runtime_ui.toast.info_count,
        toast_warning_count: runtime_ui.toast.warning_count,
        toast_last_info_message: runtime_ui.toast.last_info_message.clone(),
        toast_last_warning_text: runtime_ui.toast.last_warning_text.clone(),
        info_popup_count: runtime_ui.toast.info_popup_count,
        info_popup_reliable_count: runtime_ui.toast.info_popup_reliable_count,
        last_info_popup_reliable: runtime_ui.toast.last_info_popup_reliable,
        last_info_popup_id: runtime_ui.toast.last_info_popup_id.clone(),
        last_info_popup_message: runtime_ui.toast.last_info_popup_message.clone(),
        last_info_popup_duration_bits: runtime_ui.toast.last_info_popup_duration_bits,
        last_info_popup_align: runtime_ui.toast.last_info_popup_align,
        last_info_popup_top: runtime_ui.toast.last_info_popup_top,
        last_info_popup_left: runtime_ui.toast.last_info_popup_left,
        last_info_popup_bottom: runtime_ui.toast.last_info_popup_bottom,
        last_info_popup_right: runtime_ui.toast.last_info_popup_right,
        clipboard_count: runtime_ui.toast.clipboard_count,
        last_clipboard_text: runtime_ui.toast.last_clipboard_text.clone(),
        open_uri_count: runtime_ui.toast.open_uri_count,
        last_open_uri: runtime_ui.toast.last_open_uri.clone(),
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
        last_menu_open_id: runtime_ui.menu.last_menu_open_id,
        last_menu_open_title: runtime_ui.menu.last_menu_open_title.clone(),
        last_menu_open_message: runtime_ui.menu.last_menu_open_message.clone(),
        last_menu_open_option_rows: runtime_ui.menu.last_menu_open_option_rows,
        last_menu_open_first_row_len: runtime_ui.menu.last_menu_open_first_row_len,
        last_follow_up_menu_open_id: runtime_ui.menu.last_follow_up_menu_open_id,
        last_follow_up_menu_open_title: runtime_ui.menu.last_follow_up_menu_open_title.clone(),
        last_follow_up_menu_open_message: runtime_ui.menu.last_follow_up_menu_open_message.clone(),
        last_follow_up_menu_open_option_rows: runtime_ui.menu.last_follow_up_menu_open_option_rows,
        last_follow_up_menu_open_first_row_len: runtime_ui
            .menu
            .last_follow_up_menu_open_first_row_len,
        last_hide_follow_up_menu_id: runtime_ui.menu.last_hide_follow_up_menu_id,
        text_input_open_count: runtime_ui.text_input.open_count,
        text_input_last_id: runtime_ui.text_input.last_id,
        text_input_last_title: runtime_ui.text_input.last_title.clone(),
        text_input_last_default_text: runtime_ui.text_input.last_default_text.clone(),
        text_input_last_length: runtime_ui.text_input.last_length,
        text_input_last_numeric: runtime_ui.text_input.last_numeric,
        text_input_last_allow_empty: runtime_ui.text_input.last_allow_empty,
    })
}

pub fn build_runtime_choice_panel(hud: &HudModel) -> Option<RuntimeChoicePanelModel> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    Some(RuntimeChoicePanelModel {
        menu_choose_count: runtime_ui.menu.menu_choose_count,
        last_menu_choose_menu_id: runtime_ui.menu.last_menu_choose_menu_id,
        last_menu_choose_option: runtime_ui.menu.last_menu_choose_option,
        text_input_result_count: runtime_ui.menu.text_input_result_count,
        last_text_input_result_id: runtime_ui.menu.last_text_input_result_id,
        last_text_input_result_text: runtime_ui.menu.last_text_input_result_text.clone(),
    })
}

pub fn build_runtime_prompt_panel(hud: &HudModel) -> Option<RuntimePromptPanelModel> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    let menu = build_runtime_menu_panel(hud)?;
    let outstanding_follow_up_count = menu.outstanding_follow_up_count();
    let menu_active = runtime_menu_prompt_active(&runtime_ui.menu);
    let text_input_active = runtime_text_input_prompt_active(runtime_ui);
    let kind = if text_input_active {
        Some(RuntimeDialogPromptKind::TextInput)
    } else if outstanding_follow_up_count > 0 {
        Some(RuntimeDialogPromptKind::FollowUpMenu)
    } else if menu_active {
        Some(RuntimeDialogPromptKind::Menu)
    } else {
        None
    };

    Some(RuntimePromptPanelModel {
        kind,
        menu_active,
        text_input_active,
        menu_open_count: menu.menu_open_count,
        follow_up_menu_open_count: menu.follow_up_menu_open_count,
        hide_follow_up_menu_count: menu.hide_follow_up_menu_count,
        text_input_open_count: menu.text_input_open_count,
        text_input_last_id: menu.text_input_last_id,
        text_input_last_title: menu.text_input_last_title,
        text_input_last_message: runtime_ui.text_input.last_message.clone(),
        text_input_last_default_text: menu.text_input_last_default_text,
        text_input_last_length: menu.text_input_last_length,
        text_input_last_numeric: menu.text_input_last_numeric,
        text_input_last_allow_empty: menu.text_input_last_allow_empty,
    })
}

pub fn build_runtime_chat_panel(hud: &HudModel) -> Option<RuntimeChatPanelModel> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    Some(RuntimeChatPanelModel {
        server_message_count: runtime_ui.chat.server_message_count,
        last_server_message: runtime_ui.chat.last_server_message.clone(),
        chat_message_count: runtime_ui.chat.chat_message_count,
        last_chat_message: runtime_ui.chat.last_chat_message.clone(),
        last_chat_unformatted: runtime_ui.chat.last_chat_unformatted.clone(),
        last_chat_sender_entity_id: runtime_ui.chat.last_chat_sender_entity_id,
    })
}

pub fn build_runtime_notice_state_panel(hud: &HudModel) -> Option<RuntimeNoticeStatePanelModel> {
    let notice = build_runtime_ui_notice_panel(hud)?;
    let hud_active = notice.hud_last_message.is_some();
    let reliable_hud_active = notice.hud_last_reliable_message.is_some();
    let toast_info_active = notice.toast_last_info_message.is_some();
    let toast_warning_active = notice.toast_last_warning_text.is_some();
    let kind = if toast_warning_active {
        Some(RuntimeDialogNoticeKind::ToastWarning)
    } else if toast_info_active {
        Some(RuntimeDialogNoticeKind::ToastInfo)
    } else if reliable_hud_active {
        Some(RuntimeDialogNoticeKind::HudReliable)
    } else if hud_active {
        Some(RuntimeDialogNoticeKind::Hud)
    } else {
        None
    };
    let text = match kind {
        Some(RuntimeDialogNoticeKind::ToastWarning) => notice.toast_last_warning_text,
        Some(RuntimeDialogNoticeKind::ToastInfo) => notice.toast_last_info_message,
        Some(RuntimeDialogNoticeKind::HudReliable) => notice.hud_last_reliable_message,
        Some(RuntimeDialogNoticeKind::Hud) => notice.hud_last_message,
        None => None,
    };

    Some(RuntimeNoticeStatePanelModel {
        kind,
        text,
        count: notice
            .hud_set_count
            .saturating_add(notice.hud_set_reliable_count)
            .saturating_add(notice.toast_info_count)
            .saturating_add(notice.toast_warning_count),
        hud_active,
        reliable_hud_active,
        toast_info_active,
        toast_warning_active,
    })
}

pub fn build_runtime_dialog_panel(hud: &HudModel) -> Option<RuntimeDialogPanelModel> {
    let prompt = build_runtime_prompt_panel(hud)?;
    let notice = build_runtime_notice_state_panel(hud)?;

    Some(RuntimeDialogPanelModel {
        prompt_kind: prompt.kind,
        prompt_active: prompt.is_active(),
        menu_open_count: prompt.menu_open_count,
        follow_up_menu_open_count: prompt.follow_up_menu_open_count,
        hide_follow_up_menu_count: prompt.hide_follow_up_menu_count,
        text_input_open_count: prompt.text_input_open_count,
        text_input_last_id: prompt.text_input_last_id,
        text_input_last_title: prompt.text_input_last_title,
        text_input_last_message: prompt.text_input_last_message,
        text_input_last_default_text: prompt.text_input_last_default_text,
        text_input_last_length: prompt.text_input_last_length,
        text_input_last_numeric: prompt.text_input_last_numeric,
        text_input_last_allow_empty: prompt.text_input_last_allow_empty,
        notice_kind: notice.kind,
        notice_text: notice.text,
        notice_count: notice.count,
    })
}

pub fn build_runtime_dialog_stack_panel(hud: &HudModel) -> Option<RuntimeDialogStackPanelModel> {
    let prompt = build_runtime_prompt_panel(hud)?;
    let notice = build_runtime_notice_state_panel(hud)?;
    let chat = build_runtime_chat_panel(hud)?;
    let outstanding_follow_up_count = prompt.outstanding_follow_up_count();
    let foreground_kind = if prompt.text_input_active() {
        Some(RuntimeUiStackForegroundKind::TextInput)
    } else if outstanding_follow_up_count > 0 {
        Some(RuntimeUiStackForegroundKind::FollowUpMenu)
    } else if prompt.menu_active() {
        Some(RuntimeUiStackForegroundKind::Menu)
    } else if !chat.is_empty() {
        Some(RuntimeUiStackForegroundKind::Chat)
    } else {
        None
    };

    Some(RuntimeDialogStackPanelModel {
        foreground_kind,
        prompt,
        notice,
        chat,
    })
}

pub fn build_runtime_ui_stack_panel(hud: &HudModel) -> Option<RuntimeUiStackPanelModel> {
    let dialog_stack = build_runtime_dialog_stack_panel(hud)?;
    let chat_active = !dialog_stack.chat.is_empty();

    Some(RuntimeUiStackPanelModel {
        foreground_kind: dialog_stack.foreground_kind,
        menu_active: dialog_stack.prompt.menu_active(),
        outstanding_follow_up_count: dialog_stack.prompt.outstanding_follow_up_count(),
        text_input_active: dialog_stack.prompt.text_input_active(),
        text_input_open_count: dialog_stack.prompt.text_input_open_count,
        text_input_last_id: dialog_stack.prompt.text_input_last_id,
        notice_kind: dialog_stack.notice.kind,
        hud_notice_active: dialog_stack.notice.hud_active,
        reliable_hud_notice_active: dialog_stack.notice.reliable_hud_active,
        toast_info_active: dialog_stack.notice.toast_info_active,
        toast_warning_active: dialog_stack.notice.toast_warning_active,
        chat_active,
        server_message_count: dialog_stack.chat.server_message_count,
        chat_message_count: dialog_stack.chat.chat_message_count,
        last_chat_sender_entity_id: dialog_stack.chat.last_chat_sender_entity_id,
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
        active_count: world_labels.active_count,
        inactive_count: world_labels.inactive_count,
        last_entity_id: world_labels.last_entity_id,
        last_text: world_labels.last_text.clone(),
        last_flags: world_labels.last_flags,
        last_font_size_bits: world_labels.last_font_size_bits,
        last_z_bits: world_labels.last_z_bits,
        last_position: world_labels.last_position,
    })
}

pub fn build_runtime_marker_panel(hud: &HudModel) -> Option<RuntimeMarkerPanelModel> {
    let markers = &hud.runtime_ui.as_ref()?.markers;
    Some(RuntimeMarkerPanelModel {
        create_count: markers.create_count,
        remove_count: markers.remove_count,
        update_count: markers.update_count,
        update_text_count: markers.update_text_count,
        update_texture_count: markers.update_texture_count,
        decode_fail_count: markers.decode_fail_count,
        last_marker_id: markers.last_marker_id,
        last_control_name: markers.last_control_name.clone(),
    })
}

pub fn build_runtime_core_binding_panel(hud: &HudModel) -> Option<RuntimeCoreBindingPanelModel> {
    let core_binding = &hud.runtime_ui.as_ref()?.session.core_binding;
    Some(RuntimeCoreBindingPanelModel {
        kind: core_binding.kind,
        ambiguous_team_count: core_binding.ambiguous_team_count,
        ambiguous_team_sample: core_binding.ambiguous_team_sample.clone(),
        missing_team_count: core_binding.missing_team_count,
        missing_team_sample: core_binding.missing_team_sample.clone(),
    })
}

pub fn build_runtime_bootstrap_panel(hud: &HudModel) -> Option<RuntimeBootstrapPanelModel> {
    Some(build_runtime_session_panel(hud)?.bootstrap)
}

pub fn build_runtime_kick_panel(hud: &HudModel) -> Option<RuntimeKickPanelModel> {
    Some(build_runtime_session_panel(hud)?.kick)
}

pub fn build_runtime_loading_panel(hud: &HudModel) -> Option<RuntimeLoadingPanelModel> {
    Some(build_runtime_session_panel(hud)?.loading)
}

pub fn build_runtime_reconnect_panel(hud: &HudModel) -> Option<RuntimeReconnectPanelModel> {
    Some(build_runtime_session_panel(hud)?.reconnect)
}

pub fn build_runtime_session_panel(hud: &HudModel) -> Option<RuntimeSessionPanelModel> {
    let session = &hud.runtime_ui.as_ref()?.session;
    Some(RuntimeSessionPanelModel {
        bootstrap: RuntimeBootstrapPanelModel::from(&session.bootstrap),
        core_binding: RuntimeCoreBindingPanelModel {
            kind: session.core_binding.kind,
            ambiguous_team_count: session.core_binding.ambiguous_team_count,
            ambiguous_team_sample: session.core_binding.ambiguous_team_sample.clone(),
            missing_team_count: session.core_binding.missing_team_count,
            missing_team_sample: session.core_binding.missing_team_sample.clone(),
        },
        resource_delta: RuntimeResourceDeltaPanelModel {
            remove_tile_count: session.resource_delta.remove_tile_count,
            set_tile_count: session.resource_delta.set_tile_count,
            set_floor_count: session.resource_delta.set_floor_count,
            set_overlay_count: session.resource_delta.set_overlay_count,
            set_item_count: session.resource_delta.set_item_count,
            set_items_count: session.resource_delta.set_items_count,
            set_liquid_count: session.resource_delta.set_liquid_count,
            set_liquids_count: session.resource_delta.set_liquids_count,
            clear_items_count: session.resource_delta.clear_items_count,
            clear_liquids_count: session.resource_delta.clear_liquids_count,
            set_tile_items_count: session.resource_delta.set_tile_items_count,
            set_tile_liquids_count: session.resource_delta.set_tile_liquids_count,
            take_items_count: session.resource_delta.take_items_count,
            transfer_item_to_count: session.resource_delta.transfer_item_to_count,
            transfer_item_to_unit_count: session.resource_delta.transfer_item_to_unit_count,
            last_kind: session.resource_delta.last_kind.clone(),
            last_item_id: session.resource_delta.last_item_id,
            last_amount: session.resource_delta.last_amount,
            last_build_pos: session.resource_delta.last_build_pos,
            last_unit: session.resource_delta.last_unit,
            last_to_entity_id: session.resource_delta.last_to_entity_id,
            build_count: session.resource_delta.build_count,
            build_stack_count: session.resource_delta.build_stack_count,
            entity_count: session.resource_delta.entity_count,
            authoritative_build_update_count: session
                .resource_delta
                .authoritative_build_update_count,
            delta_apply_count: session.resource_delta.delta_apply_count,
            delta_skip_count: session.resource_delta.delta_skip_count,
            delta_conflict_count: session.resource_delta.delta_conflict_count,
            last_changed_build_pos: session.resource_delta.last_changed_build_pos,
            last_changed_entity_id: session.resource_delta.last_changed_entity_id,
            last_changed_item_id: session.resource_delta.last_changed_item_id,
            last_changed_amount: session.resource_delta.last_changed_amount,
        },
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
        player_count: entity.player_count,
        unit_count: entity.unit_count,
        last_entity_id: entity.last_entity_id,
        last_player_entity_id: entity.last_player_entity_id,
        last_unit_entity_id: entity.last_unit_entity_id,
        local_entity_id: entity.local_entity_id,
        local_unit_kind: entity.local_unit_kind,
        local_unit_value: entity.local_unit_value,
        local_hidden: entity.local_hidden,
        local_last_seen_entity_snapshot_count: entity.local_last_seen_entity_snapshot_count,
        local_position: entity.local_position,
    })
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

fn optional_u32_label(value: Option<u32>) -> String {
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

fn world_position_text(value: Option<&crate::RuntimeWorldPositionObservability>) -> String {
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

pub fn build_runtime_live_effect_panel(hud: &HudModel) -> Option<RuntimeLiveEffectPanelModel> {
    let effect = &hud.runtime_ui.as_ref()?.live.effect;
    Some(RuntimeLiveEffectPanelModel {
        effect_count: effect.effect_count,
        spawn_effect_count: effect.spawn_effect_count,
        active_overlay_count: effect.active_overlay_count,
        active_effect_id: effect.active_effect_id,
        active_contract_name: effect.active_contract_name.clone(),
        active_reliable: effect.active_reliable,
        active_position: effect.active_position,
        last_effect_id: effect.last_effect_id,
        last_spawn_effect_unit_type_id: effect.last_spawn_effect_unit_type_id,
        last_kind: effect.last_kind.clone(),
        last_contract_name: effect.last_contract_name.clone(),
        last_reliable_contract_name: effect.last_reliable_contract_name.clone(),
        last_business_hint: effect.last_business_hint.clone(),
        last_position_hint: effect.last_position_hint,
        last_position_source: effect.last_position_source,
    })
}

fn resolve_presenter_window(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    summary_window: crate::hud_model::HudViewWindowSummary,
    window: PresenterViewWindow,
) -> PresenterViewWindow {
    if window.width != 0 || window.height != 0 {
        return clamp_presenter_window_to_map(window, map_width, map_height);
    }

    let scene_window = scene
        .view_window
        .map(|view_window| clamp_render_view_window_to_map(view_window, map_width, map_height))
        .unwrap_or_else(|| clamp_hud_view_window_to_map(summary_window, map_width, map_height));

    if scene_window.width != 0 || scene_window.height != 0 {
        scene_window
    } else {
        window
    }
}

fn clamp_render_view_window_to_map(
    window: crate::render_model::RenderViewWindow,
    map_width: usize,
    map_height: usize,
) -> PresenterViewWindow {
    clamp_presenter_window_to_map(
        PresenterViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width,
            height: window.height,
        },
        map_width,
        map_height,
    )
}

fn clamp_hud_view_window_to_map(
    window: crate::hud_model::HudViewWindowSummary,
    map_width: usize,
    map_height: usize,
) -> PresenterViewWindow {
    clamp_presenter_window_to_map(
        PresenterViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width,
            height: window.height,
        },
        map_width,
        map_height,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_build_config_panel, build_build_interaction_panel, build_build_minimap_assist_panel,
        build_hud_status_panel, build_hud_visibility_panel, build_minimap_panel,
        build_runtime_bootstrap_panel,
        build_runtime_admin_panel, build_runtime_chat_panel, build_runtime_choice_panel,
        build_runtime_command_mode_panel, build_runtime_core_binding_panel,
        build_runtime_dialog_panel, build_runtime_dialog_stack_panel, build_runtime_kick_panel,
        build_runtime_live_effect_panel, build_runtime_live_entity_panel,
        build_runtime_loading_panel, build_runtime_marker_panel, build_runtime_menu_panel,
        build_runtime_notice_state_panel, build_runtime_prompt_panel,
        build_runtime_reconnect_panel, build_runtime_rules_panel, build_runtime_session_panel,
        build_runtime_ui_notice_panel, build_runtime_ui_stack_panel,
        build_runtime_world_label_panel, BuildInteractionAuthorityState, BuildInteractionMode,
        BuildInteractionQueueState, BuildInteractionSelectionState, BuildMinimapAssistPanelModel,
        PresenterViewWindow, RuntimeCoreBindingPanelModel, RuntimeDialogNoticeKind,
        RuntimeDialogPromptKind, RuntimeMarkerPanelModel, RuntimeUiStackForegroundKind,
        RuntimeWorldLabelPanelModel,
    };
    use crate::{
        hud_model::{
            HudSummary, RuntimeCommandControlGroupObservability, RuntimeCommandModeObservability,
            RuntimeCommandRectObservability, RuntimeCommandSelectionObservability,
            RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
            RuntimeCommandUnitRefObservability, RuntimeCoreBindingKindObservability,
            RuntimeCoreBindingObservability, RuntimeReconnectObservability,
            RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind,
            RuntimeBootstrapObservability, RuntimeResourceDeltaObservability,
            RuntimeSessionObservability,
            RuntimeSessionResetKind, RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
        },
        BuildConfigAuthoritySourceObservability, BuildConfigInspectorEntryObservability,
        BuildConfigOutcomeObservability, BuildConfigRollbackStripObservability,
        BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudModel,
        RenderModel, RenderObject, RenderSemanticDetailCount, RuntimeAdminObservability,
        RuntimeHudTextObservability, RuntimeLiveSummaryObservability, RuntimeMenuObservability,
        RuntimeRulesObservability, RuntimeTextInputObservability, RuntimeToastObservability,
        RuntimeUiObservability, RuntimeWorldLabelObservability, Viewport,
    };

    fn pack_point2(x: i32, y: i32) -> i32 {
        ((x & 0xffff) << 16) | (y & 0xffff)
    }

    fn runtime_stack_test_hud(runtime_ui: RuntimeUiObservability) -> HudModel {
        HudModel {
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        }
    }

    fn runtime_world_label_test_hud(
        label_count: u64,
        reliable_label_count: u64,
        remove_label_count: u64,
    ) -> HudModel {
        HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                world_labels: RuntimeWorldLabelObservability {
                    label_count,
                    reliable_label_count,
                    remove_label_count,
                    ..RuntimeWorldLabelObservability::default()
                },
                ..RuntimeUiObservability::default()
            }),
            ..HudModel::default()
        }
    }

    #[test]
    fn builds_hud_status_and_visibility_panels_from_summary() {
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
            }),
            ..HudModel::default()
        };

        let status_panel = build_hud_status_panel(&hud).expect("expected hud status panel");
        assert_eq!(status_panel.player_name, "operator");
        assert_eq!(status_panel.team_id, 2);
        assert_eq!(status_panel.selected_block, "payload-router");
        assert_eq!(status_panel.plan_count, 3);
        assert_eq!(status_panel.marker_count, 4);
        assert_eq!(status_panel.map_width, 80);
        assert_eq!(status_panel.map_height, 60);

        let visibility_panel =
            build_hud_visibility_panel(&hud).expect("expected hud visibility panel");
        assert!(visibility_panel.overlay_visible);
        assert!(visibility_panel.fog_enabled);
        assert_eq!(visibility_panel.visible_tile_count, 120);
        assert_eq!(visibility_panel.hidden_tile_count, 24);
        assert_eq!(visibility_panel.known_tile_count, 144);
        assert_eq!(visibility_panel.known_tile_percent, 3);
        assert_eq!(visibility_panel.visible_known_percent, 83);
        assert_eq!(visibility_panel.hidden_known_percent, 16);
        assert_eq!(visibility_panel.unknown_tile_count, 4656);
        assert_eq!(visibility_panel.unknown_tile_percent, 97);
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((5, 3)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
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
        assert!(!panel.window_clamped_left);
        assert!(!panel.window_clamped_top);
        assert!(!panel.window_clamped_right);
        assert!(!panel.window_clamped_bottom);
        assert_eq!(panel.window_tile_count, 56);
        assert_eq!(panel.window_coverage_percent, 1);
        assert_eq!(panel.map_tile_count, 4800);
        assert_eq!(panel.known_tile_count, 144);
        assert_eq!(panel.known_tile_percent, 3);
        assert_eq!(panel.unknown_tile_count, 4656);
        assert_eq!(panel.unknown_tile_percent, 97);
        assert_eq!(panel.focus_tile, Some((5, 3)));
        assert_eq!(panel.focus_in_window, Some(true));
        assert_eq!(panel.focus_offset_x, Some(0));
        assert_eq!(panel.focus_offset_y, Some(-1));
        assert_eq!(panel.visible_known_percent, 83);
        assert_eq!(panel.hidden_known_percent, 16);
        assert_eq!(panel.tracked_object_count, 5);
        assert_eq!(panel.window_tracked_object_count, 3);
        assert_eq!(panel.outside_window_count, 2);
        assert_eq!(panel.marker_count, 1);
        assert_eq!(panel.window_marker_count, 0);
        assert_eq!(panel.plan_count, 1);
        assert_eq!(panel.window_plan_count, 0);
        assert_eq!(panel.block_count, 1);
        assert_eq!(panel.window_block_count, 1);
        assert_eq!(panel.runtime_count, 1);
        assert_eq!(panel.window_runtime_count, 1);
        assert_eq!(panel.terrain_count, 0);
        assert_eq!(panel.window_terrain_count, 0);
        assert_eq!(panel.unknown_count, 0);
        assert_eq!(panel.window_unknown_count, 0);
        assert_eq!(panel.window_player_count, 1);
        assert_eq!(
            panel.detail_counts,
            vec![RenderSemanticDetailCount {
                label: "runtime-config",
                count: 1,
            }]
        );
    }

    #[test]
    fn builds_minimap_panel_clamps_scene_view_window_at_bottom_right() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(crate::render_model::RenderViewWindow {
                origin_x: 78,
                origin_y: 58,
                width: 8,
                height: 7,
            }),
            objects: vec![],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 80,
                map_height: 60,
                overlay_visible: false,
                fog_enabled: false,
                visible_tile_count: 0,
                hidden_tile_count: 0,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: None,
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 0,
                        height: 0,
                    },
                },
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 0,
                height: 0,
            },
        )
        .unwrap();

        assert_eq!(panel.window.origin_x, 72);
        assert_eq!(panel.window.origin_y, 53);
        assert_eq!(panel.window_last_x, 79);
        assert_eq!(panel.window_last_y, 59);
        assert!(panel.window_clamped_right);
        assert!(panel.window_clamped_bottom);
    }

    #[test]
    fn builds_minimap_panel_clamps_summary_view_window_at_bottom_right() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 80,
                map_height: 60,
                overlay_visible: false,
                fog_enabled: false,
                visible_tile_count: 0,
                hidden_tile_count: 0,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: None,
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 78,
                        origin_y: 58,
                        width: 8,
                        height: 7,
                    },
                },
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 0,
                height: 0,
            },
        )
        .unwrap();

        assert_eq!(panel.window.origin_x, 72);
        assert_eq!(panel.window.origin_y, 53);
        assert_eq!(panel.window_last_x, 79);
        assert_eq!(panel.window_last_y, 59);
        assert!(panel.window_clamped_right);
        assert!(panel.window_clamped_bottom);
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
    fn builds_build_minimap_assist_panel_from_interaction_and_minimap_panels() {
        let scene = RenderModel {
            viewport: crate::Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:1".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "terrain:2".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "unknown".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
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
                overlay_visible: false,
                fog_enabled: false,
                visible_tile_count: 0,
                hidden_tile_count: 0,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
            }),
            build_ui: Some(BuildUiObservability {
                selected_block_id: Some(301),
                selected_rotation: 1,
                building: true,
                queued_count: 2,
                inflight_count: 1,
                finished_count: 4,
                removed_count: 5,
                orphan_authoritative_count: 6,
                head: Some(BuildQueueHeadObservability {
                    x: 10,
                    y: 12,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: BuildQueueHeadStage::Queued,
                }),
                rollback_strip: BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 4,
                    rollback_count: 2,
                    last_build_tile: Some((10, 12)),
                    last_business_applied: true,
                    last_cleared_pending_local: false,
                    last_was_rollback: false,
                    last_pending_local_match: Some(true),
                    last_source: Some(BuildConfigAuthoritySourceObservability::TileConfig),
                    last_configured_outcome: Some(
                        BuildConfigOutcomeObservability::RejectedMissingBuilding,
                    ),
                    last_configured_block_name: Some("gamma".to_string()),
                },
                inspector_entries: vec![
                    BuildConfigInspectorEntryObservability {
                        family: "alpha".to_string(),
                        tracked_count: 1,
                        sample: "one".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "gamma".to_string(),
                        tracked_count: 4,
                        sample: "four".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "beta".to_string(),
                        tracked_count: 2,
                        sample: "two".to_string(),
                    },
                ],
            }),
            ..HudModel::default()
        };

        let panel = build_build_minimap_assist_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 2,
                height: 2,
            },
        )
        .expect("expected build minimap assist panel");

        assert_eq!(panel.mode, BuildInteractionMode::Place);
        assert_eq!(
            panel.selection_state,
            BuildInteractionSelectionState::HeadAligned
        );
        assert_eq!(panel.queue_state, BuildInteractionQueueState::Mixed);
        assert!(panel.place_ready);
        assert_eq!(panel.config_family_count, 3);
        assert_eq!(panel.config_sample_count, 7);
        assert_eq!(panel.top_config_family.as_deref(), Some("gamma"));
        assert_eq!(
            panel.authority_state,
            BuildInteractionAuthorityState::RejectedMissingBuilding
        );
        assert_eq!(panel.head_tile, Some((10, 12)));
        assert_eq!(panel.authority_tile, Some((10, 12)));
        assert_eq!(
            panel.authority_source,
            Some(BuildConfigAuthoritySourceObservability::TileConfig)
        );
        assert_eq!(panel.focus_tile, Some((0, 0)));
        assert_eq!(panel.focus_in_window, Some(true));
        assert_eq!(panel.visible_map_percent, 0);
        assert_eq!(panel.unknown_tile_percent, 100);
        assert_eq!(panel.window_coverage_percent, 0);
        assert_eq!(panel.tracked_object_count, 3);
        assert_eq!(panel.runtime_count, 0);
        assert_eq!(panel.focus_state_label(), "inside");
        assert_eq!(panel.map_visibility_label(), "unseen");
        assert_eq!(panel.window_coverage_label(), "offscreen");
        assert_eq!(panel.window_object_density_percent(4), 75);
        assert_eq!(panel.config_scope_label(), "multi");
        assert_eq!(panel.runtime_share_percent(), 0);
        assert_eq!(panel.next_action_label(), "resolve");
    }

    #[test]
    fn build_minimap_assist_next_action_prioritizes_operator_flow() {
        let mut panel = BuildMinimapAssistPanelModel {
            mode: BuildInteractionMode::Place,
            selection_state: BuildInteractionSelectionState::Armed,
            queue_state: BuildInteractionQueueState::Empty,
            place_ready: false,
            config_family_count: 0,
            config_sample_count: 0,
            top_config_family: None,
            authority_state: BuildInteractionAuthorityState::None,
            head_tile: None,
            authority_tile: None,
            authority_source: None,
            focus_tile: Some((4, 6)),
            focus_in_window: Some(true),
            visible_map_percent: 100,
            unknown_tile_percent: 0,
            window_coverage_percent: 25,
            tracked_object_count: 4,
            runtime_count: 1,
        };

        assert_eq!(panel.next_action_label(), "arm");
        assert_eq!(panel.window_object_density_percent(4), 100);

        panel.place_ready = true;
        panel.selection_state = BuildInteractionSelectionState::HeadDiverged;
        assert_eq!(panel.next_action_label(), "realign");

        panel.selection_state = BuildInteractionSelectionState::HeadAligned;
        assert_eq!(panel.next_action_label(), "seed");

        panel.queue_state = BuildInteractionQueueState::Queued;
        panel.authority_state = BuildInteractionAuthorityState::Rollback;
        assert_eq!(panel.next_action_label(), "resolve");

        panel.authority_state = BuildInteractionAuthorityState::Applied;
        panel.focus_in_window = Some(false);
        assert_eq!(panel.next_action_label(), "refocus");

        panel.focus_in_window = Some(true);
        panel.visible_map_percent = 0;
        panel.unknown_tile_percent = 100;
        assert_eq!(panel.next_action_label(), "survey");

        panel.visible_map_percent = 55;
        panel.unknown_tile_percent = 0;
        assert_eq!(panel.next_action_label(), "commit");

        panel.mode = BuildInteractionMode::Break;
        assert_eq!(panel.next_action_label(), "break");

        panel.focus_tile = None;
        assert_eq!(panel.next_action_label(), "refocus");

        panel.mode = BuildInteractionMode::Idle;
        assert_eq!(panel.next_action_label(), "idle");
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
                    announce_count: 12,
                    last_announce_message: Some("ann".to_string()),
                    info_message_count: 13,
                    last_info_message: Some("info".to_string()),
                    ..RuntimeHudTextObservability::default()
                },
                toast: RuntimeToastObservability {
                    info_count: 14,
                    warning_count: 15,
                    last_info_message: Some("toast".to_string()),
                    last_warning_text: Some("warn".to_string()),
                    info_popup_count: 16,
                    info_popup_reliable_count: 17,
                    last_info_popup_reliable: Some(true),
                    last_info_popup_id: Some("popup-a".to_string()),
                    last_info_popup_message: Some("popup text".to_string()),
                    last_info_popup_duration_bits: Some(2.5f32.to_bits()),
                    last_info_popup_align: Some(1),
                    last_info_popup_top: Some(2),
                    last_info_popup_left: Some(3),
                    last_info_popup_bottom: Some(4),
                    last_info_popup_right: Some(5),
                    clipboard_count: 18,
                    last_clipboard_text: Some("copied".to_string()),
                    open_uri_count: 19,
                    last_open_uri: Some("https://example.com".to_string()),
                    ..RuntimeToastObservability::default()
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
                chat: crate::RuntimeChatObservability {
                    server_message_count: 7,
                    last_server_message: Some("server text".to_string()),
                    chat_message_count: 8,
                    last_chat_message: Some("[cyan]hello".to_string()),
                    last_chat_unformatted: Some("hello".to_string()),
                    last_chat_sender_entity_id: Some(404),
                },
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
        assert_eq!(panel.announce_count, 12);
        assert_eq!(panel.last_announce_message.as_deref(), Some("ann"));
        assert_eq!(panel.info_message_count, 13);
        assert_eq!(panel.last_info_message.as_deref(), Some("info"));
        assert_eq!(panel.toast_info_count, 14);
        assert_eq!(panel.toast_warning_count, 15);
        assert_eq!(panel.toast_last_info_message.as_deref(), Some("toast"));
        assert_eq!(panel.toast_last_warning_text.as_deref(), Some("warn"));
        assert_eq!(panel.info_popup_count, 16);
        assert_eq!(panel.info_popup_reliable_count, 17);
        assert_eq!(panel.last_info_popup_reliable, Some(true));
        assert_eq!(panel.last_info_popup_id.as_deref(), Some("popup-a"));
        assert_eq!(panel.last_info_popup_message.as_deref(), Some("popup text"));
        assert_eq!(panel.last_info_popup_duration_bits, Some(2.5f32.to_bits()));
        assert_eq!(panel.last_info_popup_align, Some(1));
        assert_eq!(panel.last_info_popup_top, Some(2));
        assert_eq!(panel.last_info_popup_left, Some(3));
        assert_eq!(panel.last_info_popup_bottom, Some(4));
        assert_eq!(panel.last_info_popup_right, Some(5));
        assert_eq!(panel.clipboard_count, 18);
        assert_eq!(panel.last_clipboard_text.as_deref(), Some("copied"));
        assert_eq!(panel.open_uri_count, 19);
        assert_eq!(panel.last_open_uri.as_deref(), Some("https://example.com"));
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
    fn builds_runtime_chat_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability {
                    server_message_count: 7,
                    last_server_message: Some("server text".to_string()),
                    chat_message_count: 8,
                    last_chat_message: Some("[cyan]hello".to_string()),
                    last_chat_unformatted: Some("hello".to_string()),
                    last_chat_sender_entity_id: Some(404),
                },
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_chat_panel(&hud).expect("expected runtime chat panel");

        assert_eq!(panel.server_message_count, 7);
        assert_eq!(panel.last_server_message.as_deref(), Some("server text"));
        assert_eq!(panel.chat_message_count, 8);
        assert_eq!(panel.last_chat_message.as_deref(), Some("[cyan]hello"));
        assert_eq!(panel.last_chat_unformatted.as_deref(), Some("hello"));
        assert_eq!(panel.last_chat_sender_entity_id, Some(404));
    }

    #[test]
    fn builds_runtime_rules_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
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
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
        assert!(!panel.is_empty());
    }

    #[test]
    fn builds_runtime_world_label_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability {
                    label_count: 19,
                    reliable_label_count: 20,
                    remove_label_count: 21,
                    active_count: 2,
                    inactive_count: 1,
                    last_entity_id: Some(904),
                    last_text: Some("world label".to_string()),
                    last_flags: Some(3),
                    last_font_size_bits: Some(12.0f32.to_bits()),
                    last_z_bits: Some(4.0f32.to_bits()),
                    last_position: Some(crate::RuntimeWorldPositionObservability {
                        x_bits: 40.0f32.to_bits(),
                        y_bits: 60.0f32.to_bits(),
                    }),
                },
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
        assert_eq!(panel.active_count, 2);
        assert_eq!(panel.inactive_count, 1);
        assert_eq!(panel.last_entity_id, Some(904));
        assert_eq!(panel.last_text.as_deref(), Some("world label"));
        assert_eq!(panel.last_flags, Some(3));
        assert_eq!(panel.last_font_size_bits, Some(12.0f32.to_bits()));
        assert_eq!(panel.last_z_bits, Some(4.0f32.to_bits()));
        assert_eq!(panel.inactive_count(), 1);
        assert_eq!(panel.last_text_len(), 11);
        assert_eq!(panel.last_text_line_count(), 1);
        assert_eq!(panel.last_font_size(), Some(12.0));
        assert_eq!(panel.last_z(), Some(4.0));
        assert_eq!(
            panel.last_position,
            Some(crate::RuntimeWorldPositionObservability {
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
            })
        );
    }

    #[test]
    fn build_runtime_world_label_panel_saturates_total_count() {
        let saturated_panel =
            build_runtime_world_label_panel(&runtime_world_label_test_hud(u64::MAX - 1, 1, 1))
                .expect("expected runtime world-label panel");
        assert_eq!(saturated_panel.label_count, u64::MAX - 1);
        assert_eq!(saturated_panel.reliable_label_count, 1);
        assert_eq!(saturated_panel.remove_label_count, 1);
        assert_eq!(saturated_panel.total_count, u64::MAX);

        let exact_panel =
            build_runtime_world_label_panel(&runtime_world_label_test_hud(u64::MAX - 2, 1, 1))
                .expect("expected runtime world-label panel");
        assert_eq!(exact_panel.label_count, u64::MAX - 2);
        assert_eq!(exact_panel.reliable_label_count, 1);
        assert_eq!(exact_panel.remove_label_count, 1);
        assert_eq!(exact_panel.total_count, u64::MAX);
    }

    #[test]
    fn runtime_world_label_panel_derived_metrics_handle_multiline_and_non_finite_bits() {
        let panel = RuntimeWorldLabelPanelModel {
            label_count: 1,
            reliable_label_count: 2,
            remove_label_count: 3,
            total_count: 6,
            active_count: 1,
            inactive_count: 4,
            last_entity_id: Some(9),
            last_text: Some("alpha\nbeta\n".to_string()),
            last_flags: Some(7),
            last_font_size_bits: Some(f32::NAN.to_bits()),
            last_z_bits: Some(f32::INFINITY.to_bits()),
            last_position: None,
        };

        assert_eq!(panel.inactive_count(), 4);
        assert_eq!(panel.last_text_len(), 11);
        assert_eq!(panel.last_text_line_count(), 3);
        assert_eq!(panel.last_font_size(), None);
        assert_eq!(panel.last_z(), None);
    }

    #[test]
    fn builds_runtime_marker_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability {
                    create_count: 54,
                    remove_count: 55,
                    update_count: 56,
                    update_text_count: 57,
                    update_texture_count: 58,
                    decode_fail_count: 2,
                    last_marker_id: Some(808),
                    last_control_name: Some("flushText".to_string()),
                },
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_marker_panel(&hud).expect("expected runtime marker panel");

        assert_eq!(panel.create_count, 54);
        assert_eq!(panel.remove_count, 55);
        assert_eq!(panel.update_count, 56);
        assert_eq!(panel.update_text_count, 57);
        assert_eq!(panel.update_texture_count, 58);
        assert_eq!(panel.decode_fail_count, 2);
        assert_eq!(panel.last_marker_id, Some(808));
        assert_eq!(panel.last_control_name.as_deref(), Some("flushText"));
        assert_eq!(panel.total_count(), 280);
        assert_eq!(panel.mutate_count(), 165);
        assert_eq!(panel.control_name_len(), 9);
        assert!(!panel.is_empty());
    }

    #[test]
    fn runtime_marker_panel_derived_metrics_handle_default_state() {
        let panel = RuntimeMarkerPanelModel {
            create_count: 0,
            remove_count: 0,
            update_count: 0,
            update_text_count: 0,
            update_texture_count: 0,
            decode_fail_count: 0,
            last_marker_id: None,
            last_control_name: None,
        };

        assert!(panel.is_empty());
        assert_eq!(panel.total_count(), 0);
        assert_eq!(panel.mutate_count(), 0);
        assert_eq!(panel.control_name_len(), 0);
    }

    #[test]
    fn builds_runtime_live_entity_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability {
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
        assert_eq!(panel.player_count, 2);
        assert_eq!(panel.unit_count, 1);
        assert_eq!(panel.last_entity_id, Some(202));
        assert_eq!(panel.last_player_entity_id, Some(102));
        assert_eq!(panel.last_unit_entity_id, Some(202));
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
        assert_eq!(
            panel.local_owned_unit_payload_label(),
            "payload=unit=2/999"
        );
        assert_eq!(
            panel.local_owned_unit_nested_label(),
            "nested=snapshot=7"
        );
        assert_eq!(
            panel.local_owned_unit_stack_label(),
            "stack=entities=12 hidden=3 players=2 units=1 last=202/102/202"
        );
        assert_eq!(
            panel.local_owned_unit_controller_label(),
            "controller=entity=404 pos=20.0:33.0 hidden=0"
        );
        assert_eq!(
            panel.detail_label(),
            "local=404 payload=unit=2/999 nested=snapshot=7 stack=entities=12 hidden=3 players=2 units=1 last=202/102/202 controller=entity=404 pos=20.0:33.0 hidden=0"
        );
    }

    #[test]
    fn builds_runtime_live_effect_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability::default(),
                    effect: crate::RuntimeLiveEffectSummaryObservability {
                        effect_count: 11,
                        spawn_effect_count: 73,
                        active_overlay_count: 1,
                        active_effect_id: Some(13),
                        active_contract_name: Some("lightning".to_string()),
                        active_reliable: Some(true),
                        active_position: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 28.0f32.to_bits(),
                            y_bits: 36.0f32.to_bits(),
                        }),
                        last_effect_id: Some(8),
                        last_spawn_effect_unit_type_id: Some(19),
                        last_kind: Some("Point2".to_string()),
                        last_contract_name: Some("position_target".to_string()),
                        last_reliable_contract_name: Some("unit_parent".to_string()),
                        last_business_hint: Some("pos:point2:3:4@1/0".to_string()),
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
        assert_eq!(panel.active_overlay_count, 1);
        assert_eq!(panel.active_effect_id, Some(13));
        assert_eq!(panel.active_contract_name.as_deref(), Some("lightning"));
        assert_eq!(panel.active_reliable, Some(true));
        assert_eq!(
            panel.active_position,
            Some(crate::RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            })
        );
        assert_eq!(panel.last_effect_id, Some(8));
        assert_eq!(panel.last_spawn_effect_unit_type_id, Some(19));
        assert_eq!(panel.last_kind.as_deref(), Some("Point2"));
        assert_eq!(panel.last_contract_name.as_deref(), Some("position_target"));
        assert_eq!(
            panel.last_reliable_contract_name.as_deref(),
            Some("unit_parent")
        );
        assert_eq!(
            panel.last_business_hint.as_deref(),
            Some("pos:point2:3:4@1/0")
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
        assert_eq!(panel.display_effect_id(), Some(13));
        assert_eq!(panel.display_contract_name(), Some("lightning"));
        assert_eq!(panel.display_reliable_contract_name(), Some("lightning"));
        assert_eq!(
            panel.display_position_source(),
            Some(crate::RuntimeLiveEffectPositionSource::ActiveOverlay)
        );
        assert_eq!(
            panel.display_position(),
            Some(&crate::RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            })
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
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
                    last_menu_open_id: Some(40),
                    last_menu_open_title: Some("main".to_string()),
                    last_menu_open_message: Some("pick".to_string()),
                    last_menu_open_option_rows: 2,
                    last_menu_open_first_row_len: 3,
                    last_follow_up_menu_open_id: Some(41),
                    last_follow_up_menu_open_title: Some("follow".to_string()),
                    last_follow_up_menu_open_message: Some("next".to_string()),
                    last_follow_up_menu_open_option_rows: 1,
                    last_follow_up_menu_open_first_row_len: 2,
                    last_hide_follow_up_menu_id: Some(41),
                    menu_choose_count: 29,
                    last_menu_choose_menu_id: Some(404),
                    last_menu_choose_option: Some(2),
                    text_input_result_count: 30,
                    last_text_input_result_id: Some(405),
                    last_text_input_result_text: Some("ok123".to_string()),
                    ..RuntimeMenuObservability::default()
                },
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let panel = build_runtime_menu_panel(&hud).expect("expected runtime menu panel");

        assert_eq!(panel.menu_open_count, 16);
        assert_eq!(panel.follow_up_menu_open_count, 17);
        assert_eq!(panel.hide_follow_up_menu_count, 18);
        assert_eq!(panel.last_menu_open_id, Some(40));
        assert_eq!(panel.last_menu_open_title.as_deref(), Some("main"));
        assert_eq!(panel.last_menu_open_message.as_deref(), Some("pick"));
        assert_eq!(panel.last_menu_open_option_rows, 2);
        assert_eq!(panel.last_menu_open_first_row_len, 3);
        assert_eq!(panel.last_follow_up_menu_open_id, Some(41));
        assert_eq!(
            panel.last_follow_up_menu_open_title.as_deref(),
            Some("follow")
        );
        assert_eq!(
            panel.last_follow_up_menu_open_message.as_deref(),
            Some("next")
        );
        assert_eq!(panel.last_follow_up_menu_open_option_rows, 1);
        assert_eq!(panel.last_follow_up_menu_open_first_row_len, 2);
        assert_eq!(panel.last_hide_follow_up_menu_id, Some(41));
        assert_eq!(panel.text_input_open_count, 53);
        assert_eq!(panel.text_input_last_id, Some(404));
        assert_eq!(panel.text_input_last_title.as_deref(), Some("Digits"));
        assert_eq!(panel.text_input_last_default_text.as_deref(), Some("12345"));
        assert_eq!(panel.text_input_last_length, Some(16));
        assert_eq!(panel.text_input_last_numeric, Some(true));
        assert_eq!(panel.text_input_last_allow_empty, Some(true));

        let choice = build_runtime_choice_panel(&hud).expect("expected runtime choice panel");
        assert_eq!(choice.menu_choose_count, 29);
        assert_eq!(choice.last_menu_choose_menu_id, Some(404));
        assert_eq!(choice.last_menu_choose_option, Some(2));
        assert_eq!(choice.text_input_result_count, 30);
        assert_eq!(choice.last_text_input_result_id, Some(405));
        assert_eq!(choice.last_text_input_result_text.as_deref(), Some("ok123"));
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
                    ..RuntimeHudTextObservability::default()
                },
                toast: RuntimeToastObservability {
                    info_count: 14,
                    warning_count: 15,
                    last_info_message: Some("toast".to_string()),
                    last_warning_text: Some("warn".to_string()),
                    ..RuntimeToastObservability::default()
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
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
                    ..RuntimeMenuObservability::default()
                },
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
        assert_eq!(
            panel.text_input_last_message.as_deref(),
            Some("Only numbers")
        );
        assert_eq!(panel.text_input_last_default_text.as_deref(), Some("12345"));
        assert_eq!(panel.text_input_last_length, Some(16));
        assert_eq!(panel.text_input_last_numeric, Some(true));
        assert_eq!(panel.text_input_last_allow_empty, Some(true));
        assert_eq!(
            panel.notice_kind,
            Some(RuntimeDialogNoticeKind::ToastWarning)
        );
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
                chat: crate::RuntimeChatObservability::default(),
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
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
                chat: crate::RuntimeChatObservability::default(),
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
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
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
        assert!(!panel.is_empty());
    }

    #[test]
    fn runtime_admin_and_rules_panels_report_empty_for_default_runtime_ui() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability::default()),
            ..HudModel::default()
        };

        let admin = build_runtime_admin_panel(&hud).expect("expected runtime admin panel");
        let rules = build_runtime_rules_panel(&hud).expect("expected runtime rules panel");

        assert!(admin.is_empty());
        assert!(rules.is_empty());
    }

    #[test]
    fn builds_runtime_session_panel_from_runtime_ui_observability() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability {
                    bootstrap: RuntimeBootstrapObservability {
                        rules_label: "rules-hash-1".to_string(),
                        tags_label: "tags-hash-2".to_string(),
                        locales_label: "locales-hash-3".to_string(),
                        team_count: 2,
                        marker_count: 3,
                        custom_chunk_count: 4,
                        content_patch_count: 5,
                        player_team_plan_count: 6,
                        static_fog_team_count: 7,
                    },
                    core_binding: RuntimeCoreBindingObservability {
                        kind: Some(
                            RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation,
                        ),
                        ambiguous_team_count: 1,
                        ambiguous_team_sample: vec![1],
                        missing_team_count: 1,
                        missing_team_sample: vec![4],
                    },
                    resource_delta: RuntimeResourceDeltaObservability {
                        remove_tile_count: 80,
                        set_tile_count: 81,
                        set_floor_count: 82,
                        set_overlay_count: 83,
                        set_item_count: 22,
                        set_items_count: 23,
                        set_liquid_count: 24,
                        set_liquids_count: 25,
                        clear_items_count: 84,
                        clear_liquids_count: 85,
                        set_tile_items_count: 26,
                        set_tile_liquids_count: 27,
                        take_items_count: 1,
                        transfer_item_to_count: 2,
                        transfer_item_to_unit_count: 3,
                        last_kind: Some("to_unit".to_string()),
                        last_item_id: Some(6),
                        last_amount: None,
                        last_build_pos: None,
                        last_unit: Some(RuntimeCommandUnitRefObservability {
                            kind: 2,
                            value: 808,
                        }),
                        last_to_entity_id: Some(404),
                        build_count: 2,
                        build_stack_count: 3,
                        entity_count: 1,
                        authoritative_build_update_count: 4,
                        delta_apply_count: 5,
                        delta_skip_count: 6,
                        delta_conflict_count: 7,
                        last_changed_build_pos: Some(pack_point2(9, 9)),
                        last_changed_entity_id: Some(900),
                        last_changed_item_id: Some(6),
                        last_changed_amount: Some(1),
                    },
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
        let core_binding =
            build_runtime_core_binding_panel(&hud).expect("expected runtime core binding panel");
        let bootstrap =
            build_runtime_bootstrap_panel(&hud).expect("expected runtime bootstrap panel");

        assert_eq!(panel.bootstrap, bootstrap);
        assert_eq!(bootstrap.rules_label, "rules-hash-1");
        assert_eq!(bootstrap.tags_label, "tags-hash-2");
        assert_eq!(bootstrap.locales_label, "locales-hash-3");
        assert_eq!(bootstrap.team_count, 2);
        assert_eq!(bootstrap.marker_count, 3);
        assert_eq!(bootstrap.custom_chunk_count, 4);
        assert_eq!(bootstrap.content_patch_count, 5);
        assert_eq!(bootstrap.player_team_plan_count, 6);
        assert_eq!(bootstrap.static_fog_team_count, 7);
        assert_eq!(
            bootstrap.summary_label(),
            "rules=rules-hash-1:tags=tags-hash-2:locales=locales-hash-3:teams=2:markers=3:chunks=4:patches=5:plans=6:fog=7"
        );
        assert_eq!(
            bootstrap.detail_label(),
            "rules-label=rules-hash-1:tags-label=tags-hash-2:locales-label=locales-hash-3:team-count=2:marker-count=3:custom-chunk-count=4:content-patch-count=5:player-team-plan-count=6:static-fog-team-count=7"
        );
        assert!(!bootstrap.is_empty());
        assert_eq!(
            panel.core_binding.kind,
            Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation)
        );
        assert_eq!(panel.core_binding.ambiguous_team_count, 1);
        assert_eq!(panel.core_binding.ambiguous_team_sample, vec![1]);
        assert_eq!(panel.core_binding.missing_team_count, 1);
        assert_eq!(panel.core_binding.missing_team_sample, vec![4]);
        assert_eq!(panel.resource_delta.take_items_count, 1);
        assert_eq!(panel.resource_delta.transfer_item_to_count, 2);
        assert_eq!(panel.resource_delta.transfer_item_to_unit_count, 3);
        assert_eq!(panel.resource_delta.last_kind.as_deref(), Some("to_unit"));
        assert_eq!(
            panel.resource_delta.last_unit,
            Some(RuntimeCommandUnitRefObservability {
                kind: 2,
                value: 808,
            })
        );
        assert_eq!(panel.resource_delta.last_to_entity_id, Some(404));
        assert_eq!(panel.resource_delta.build_count, 2);
        assert_eq!(panel.resource_delta.delta_conflict_count, 7);
        assert!(!panel.resource_delta.is_empty());
        assert_eq!(
            core_binding,
            RuntimeCoreBindingPanelModel {
                kind: Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation,),
                ambiguous_team_count: 1,
                ambiguous_team_sample: vec![1],
                missing_team_count: 1,
                missing_team_sample: vec![4],
            }
        );
        assert!(!core_binding.is_empty());
        assert_eq!(core_binding.kind_label(), "first-core-per-team");
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

    #[test]
    fn builds_runtime_session_subpanels_independently() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability {
                    bootstrap: RuntimeBootstrapObservability::default(),
                    core_binding: RuntimeCoreBindingObservability::default(),
                    resource_delta: RuntimeResourceDeltaObservability::default(),
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

        let kick = build_runtime_kick_panel(&hud).expect("expected runtime kick panel");
        assert_eq!(kick.reason_text.as_deref(), Some("idInUse"));
        assert_eq!(kick.reason_ordinal, Some(7));

        let loading = build_runtime_loading_panel(&hud).expect("expected runtime loading panel");
        assert_eq!(loading.timeout_count, 2);
        assert_eq!(loading.last_timeout_idle_ms, Some(20000));

        let reconnect =
            build_runtime_reconnect_panel(&hud).expect("expected runtime reconnect panel");
        assert_eq!(
            reconnect.phase,
            RuntimeReconnectPhaseObservability::Attempting
        );
        assert_eq!(
            reconnect.reason_kind,
            Some(RuntimeReconnectReasonKind::ConnectRedirect)
        );
        assert_eq!(reconnect.last_redirect_port, Some(6567));
    }

    #[test]
    fn runtime_session_panel_reports_empty_state_only_for_default_observability() {
        let empty_hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };
        let empty_panel =
            build_runtime_session_panel(&empty_hud).expect("expected runtime session panel");
        let empty_bootstrap =
            build_runtime_bootstrap_panel(&empty_hud).expect("expected runtime bootstrap panel");
        let empty_core_binding = build_runtime_core_binding_panel(&empty_hud)
            .expect("expected runtime core binding panel");
        assert!(empty_panel.is_empty());
        assert!(empty_panel.bootstrap.is_empty());
        assert!(empty_bootstrap.is_empty());
        assert!(empty_core_binding.is_empty());
        assert!(empty_panel.resource_delta.is_empty());
        assert!(empty_panel.kick.is_empty());
        assert!(empty_panel.loading.is_empty());
        assert!(empty_panel.reconnect.is_empty());

        let active_hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability::default(),
                toast: RuntimeToastObservability::default(),
                text_input: RuntimeTextInputObservability::default(),
                chat: crate::RuntimeChatObservability::default(),
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability::default(),
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability {
                    bootstrap: RuntimeBootstrapObservability::default(),
                    core_binding: RuntimeCoreBindingObservability::default(),
                    resource_delta: RuntimeResourceDeltaObservability {
                        take_items_count: 1,
                        transfer_item_to_count: 2,
                        transfer_item_to_unit_count: 3,
                        last_kind: Some("to_unit".to_string()),
                        last_item_id: Some(6),
                        last_unit: Some(RuntimeCommandUnitRefObservability {
                            kind: 2,
                            value: 808,
                        }),
                        last_to_entity_id: Some(404),
                        build_count: 2,
                        build_stack_count: 3,
                        entity_count: 1,
                        authoritative_build_update_count: 4,
                        delta_apply_count: 5,
                        delta_skip_count: 6,
                        delta_conflict_count: 7,
                        last_changed_build_pos: Some(pack_point2(9, 9)),
                        last_changed_entity_id: Some(900),
                        last_changed_item_id: Some(6),
                        last_changed_amount: Some(1),
                        ..RuntimeResourceDeltaObservability::default()
                    },
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
        let active_panel =
            build_runtime_session_panel(&active_hud).expect("expected runtime session panel");
        assert!(!active_panel.is_empty());
        assert!(!active_panel.resource_delta.is_empty());
        assert!(!active_panel.kick.is_empty());
        assert!(!active_panel.loading.is_empty());
        assert!(!active_panel.reconnect.is_empty());
    }

    #[test]
    fn panel_helpers_surface_hud_and_minimap_lengths_and_map_percents() {
        let hud = HudModel {
            summary: Some(HudSummary {
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((2, 3)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 10,
                        height: 10,
                    },
                },
            }),
            ..HudModel::default()
        };
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 0,
                x: 16.0,
                y: 24.0,
            }],
        };

        let status = build_hud_status_panel(&hud).expect("expected hud status panel");
        assert_eq!(status.map_tile_count(), 100);
        assert_eq!(status.player_name_len(), 8);
        assert_eq!(status.selected_block_len(), 14);

        let visibility = build_hud_visibility_panel(&hud).expect("expected hud visibility panel");
        assert_eq!(visibility.visible_map_percent(), 25);
        assert_eq!(visibility.hidden_map_percent(), 15);

        let minimap = build_minimap_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 4,
            },
        )
        .expect("expected minimap panel");
        assert_eq!(minimap.visible_map_percent(), 25);
        assert_eq!(minimap.hidden_map_percent(), 15);
        assert_eq!(minimap.map_object_density_percent(), 1);
        assert_eq!(minimap.window_object_density_percent(), 6);
        assert_eq!(minimap.outside_object_percent(), 0);
        assert_eq!(minimap.focus_tile, Some((2, 3)));
        assert_eq!(minimap.focus_in_window, Some(true));
    }

    #[test]
    fn panel_helpers_surface_runtime_menu_chat_and_dialog_derivations() {
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability {
                    set_count: 9,
                    set_reliable_count: 10,
                    hide_count: 11,
                    last_message: Some("hud text".to_string()),
                    last_reliable_message: Some("hud rel".to_string()),
                    ..RuntimeHudTextObservability::default()
                },
                toast: RuntimeToastObservability {
                    info_count: 14,
                    warning_count: 15,
                    last_info_message: Some("toast".to_string()),
                    last_warning_text: Some("warn".to_string()),
                    ..RuntimeToastObservability::default()
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
                chat: crate::RuntimeChatObservability {
                    server_message_count: 7,
                    last_server_message: Some("server text".to_string()),
                    chat_message_count: 8,
                    last_chat_message: Some("[cyan]hello".to_string()),
                    last_chat_unformatted: Some("hello".to_string()),
                    last_chat_sender_entity_id: Some(404),
                },
                admin: RuntimeAdminObservability::default(),
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 15,
                    ..RuntimeMenuObservability::default()
                },
                command_mode: RuntimeCommandModeObservability::default(),
                rules: RuntimeRulesObservability::default(),
                world_labels: RuntimeWorldLabelObservability::default(),
                markers: crate::hud_model::RuntimeMarkerObservability::default(),
                session: RuntimeSessionObservability::default(),
                live: RuntimeLiveSummaryObservability::default(),
            }),
            ..HudModel::default()
        };

        let menu = build_runtime_menu_panel(&hud).expect("expected runtime menu panel");
        assert!(!menu.is_empty());
        assert_eq!(menu.outstanding_follow_up_count(), 2);
        assert_eq!(menu.default_text_len(), 5);

        let chat = build_runtime_chat_panel(&hud).expect("expected runtime chat panel");
        assert!(!chat.is_empty());
        assert_eq!(chat.last_server_message_len(), 11);
        assert_eq!(chat.last_chat_message_len(), 11);
        assert_eq!(chat.last_chat_unformatted_len(), 5);
        assert_eq!(chat.formatted_matches_unformatted(), Some(false));

        let prompt = build_runtime_prompt_panel(&hud).expect("expected runtime prompt panel");
        assert!(!prompt.is_empty());
        assert_eq!(prompt.kind, Some(RuntimeDialogPromptKind::TextInput));
        assert!(prompt.is_active());
        assert!(prompt.menu_active());
        assert_eq!(prompt.outstanding_follow_up_count(), 2);
        assert_eq!(prompt.layer_labels(), vec!["input", "follow-up", "menu"]);
        assert_eq!(prompt.depth(), 3);
        assert_eq!(prompt.prompt_message_len(), 12);
        assert_eq!(prompt.default_text_len(), 5);

        let notice =
            build_runtime_notice_state_panel(&hud).expect("expected runtime notice state panel");
        assert!(!notice.is_empty());
        assert!(notice.is_active());
        assert_eq!(notice.kind, Some(RuntimeDialogNoticeKind::ToastWarning));
        assert_eq!(
            notice.layer_labels(),
            vec!["hud", "reliable", "info", "warn"]
        );
        assert_eq!(notice.depth(), 4);
        assert_eq!(notice.text_len(), 4);

        let dialog = build_runtime_dialog_panel(&hud).expect("expected runtime dialog panel");
        assert!(!dialog.is_empty());
        assert_eq!(dialog.outstanding_follow_up_count(), 2);
        assert_eq!(dialog.prompt_message_len(), 12);
        assert_eq!(dialog.default_text_len(), 5);
        assert_eq!(dialog.notice_text_len(), 4);

        let dialog_stack =
            build_runtime_dialog_stack_panel(&hud).expect("expected runtime dialog stack panel");
        assert!(!dialog_stack.is_empty());
        assert_eq!(
            dialog_stack.foreground_kind,
            Some(RuntimeUiStackForegroundKind::TextInput)
        );
        assert_eq!(dialog_stack.foreground_label(), "input");
        assert_eq!(
            dialog_stack.prompt.kind,
            Some(RuntimeDialogPromptKind::TextInput)
        );
        assert_eq!(
            dialog_stack.notice.kind,
            Some(RuntimeDialogNoticeKind::ToastWarning)
        );
        assert_eq!(dialog_stack.chat.last_chat_sender_entity_id, Some(404));
        assert_eq!(dialog_stack.prompt_depth(), 3);
        assert_eq!(dialog_stack.notice_depth(), 4);
        assert_eq!(dialog_stack.chat_depth(), 1);
        assert_eq!(dialog_stack.active_group_count(), 3);
        assert_eq!(dialog_stack.total_depth(), 8);

        let stack = build_runtime_ui_stack_panel(&hud).expect("expected runtime stack panel");
        assert!(!stack.is_empty());
        assert_eq!(
            stack.foreground_kind,
            Some(RuntimeUiStackForegroundKind::TextInput)
        );
        assert_eq!(stack.foreground_label(), "input");
        assert_eq!(
            stack.prompt_layer_labels(),
            vec!["input", "follow-up", "menu"]
        );
        assert_eq!(
            stack.notice_layer_labels(),
            vec!["hud", "reliable", "info", "warn"]
        );
        assert_eq!(stack.prompt_depth(), 3);
        assert_eq!(stack.notice_depth(), 4);
        assert_eq!(stack.chat_depth(), 1);
        assert_eq!(stack.active_group_count(), 3);
        assert_eq!(stack.total_depth(), 8);
    }

    #[test]
    fn builds_runtime_ui_stack_panel_for_minimal_presenter_regression_cases() {
        let mut chat_only = RuntimeUiObservability::default();
        chat_only.chat.server_message_count = 1;
        chat_only.chat.chat_message_count = 2;
        chat_only.chat.last_chat_sender_entity_id = Some(42);
        let chat_only_dialog =
            build_runtime_dialog_stack_panel(&runtime_stack_test_hud(chat_only.clone()))
                .expect("dialog stack");
        assert_eq!(
            chat_only_dialog.foreground_kind,
            Some(RuntimeUiStackForegroundKind::Chat)
        );
        assert_eq!(
            chat_only_dialog.prompt.layer_labels(),
            Vec::<&'static str>::new()
        );
        assert_eq!(
            chat_only_dialog.notice.layer_labels(),
            Vec::<&'static str>::new()
        );
        assert_eq!(chat_only_dialog.chat_depth(), 1);
        assert_eq!(chat_only_dialog.active_group_count(), 1);
        assert_eq!(chat_only_dialog.total_depth(), 1);
        let chat_only =
            build_runtime_ui_stack_panel(&runtime_stack_test_hud(chat_only)).expect("stack");
        assert_eq!(
            chat_only.foreground_kind,
            Some(RuntimeUiStackForegroundKind::Chat)
        );
        assert!(chat_only.prompt_layer_labels().is_empty());
        assert!(chat_only.notice_layer_labels().is_empty());
        assert_eq!(chat_only.chat_depth(), 1);
        assert_eq!(chat_only.active_group_count(), 1);
        assert_eq!(chat_only.total_depth(), 1);

        let mut menu_only = RuntimeUiObservability::default();
        menu_only.menu.menu_open_count = 1;
        let menu_only_dialog =
            build_runtime_dialog_stack_panel(&runtime_stack_test_hud(menu_only.clone()))
                .expect("dialog stack");
        assert_eq!(
            menu_only_dialog.foreground_kind,
            Some(RuntimeUiStackForegroundKind::Menu)
        );
        assert_eq!(menu_only_dialog.prompt.layer_labels(), vec!["menu"]);
        assert_eq!(
            menu_only_dialog.notice.layer_labels(),
            Vec::<&'static str>::new()
        );
        assert_eq!(menu_only_dialog.chat_depth(), 0);
        assert_eq!(menu_only_dialog.active_group_count(), 1);
        assert_eq!(menu_only_dialog.total_depth(), 1);
        let menu_only =
            build_runtime_ui_stack_panel(&runtime_stack_test_hud(menu_only)).expect("stack");
        assert_eq!(
            menu_only.foreground_kind,
            Some(RuntimeUiStackForegroundKind::Menu)
        );
        assert_eq!(menu_only.prompt_layer_labels(), vec!["menu"]);
        assert!(menu_only.notice_layer_labels().is_empty());
        assert_eq!(menu_only.chat_depth(), 0);
        assert_eq!(menu_only.active_group_count(), 1);
        assert_eq!(menu_only.total_depth(), 1);

        let mut follow_up_only = RuntimeUiObservability::default();
        follow_up_only.menu.follow_up_menu_open_count = 1;
        let follow_up_only_dialog =
            build_runtime_dialog_stack_panel(&runtime_stack_test_hud(follow_up_only.clone()))
                .expect("dialog stack");
        assert_eq!(
            follow_up_only_dialog.foreground_kind,
            Some(RuntimeUiStackForegroundKind::FollowUpMenu)
        );
        assert_eq!(
            follow_up_only_dialog.prompt.layer_labels(),
            vec!["follow-up"]
        );
        assert_eq!(
            follow_up_only_dialog.notice.layer_labels(),
            Vec::<&'static str>::new()
        );
        assert_eq!(follow_up_only_dialog.chat_depth(), 0);
        assert_eq!(follow_up_only_dialog.active_group_count(), 1);
        assert_eq!(follow_up_only_dialog.total_depth(), 1);
        let follow_up_only =
            build_runtime_ui_stack_panel(&runtime_stack_test_hud(follow_up_only)).expect("stack");
        assert_eq!(
            follow_up_only.foreground_kind,
            Some(RuntimeUiStackForegroundKind::FollowUpMenu)
        );
        assert_eq!(follow_up_only.prompt_layer_labels(), vec!["follow-up"]);
        assert!(follow_up_only.notice_layer_labels().is_empty());
        assert_eq!(follow_up_only.chat_depth(), 0);
        assert_eq!(follow_up_only.active_group_count(), 1);
        assert_eq!(follow_up_only.total_depth(), 1);

        let mut input_notice_chat = RuntimeUiObservability::default();
        input_notice_chat.text_input.open_count = 1;
        input_notice_chat.text_input.last_id = Some(404);
        input_notice_chat.toast.warning_count = 1;
        input_notice_chat.toast.last_warning_text = Some("warn".to_string());
        input_notice_chat.chat.server_message_count = 1;
        input_notice_chat.chat.chat_message_count = 1;
        input_notice_chat.chat.last_chat_sender_entity_id = Some(404);
        let input_notice_chat_dialog =
            build_runtime_dialog_stack_panel(&runtime_stack_test_hud(input_notice_chat.clone()))
                .expect("dialog stack");
        assert_eq!(
            input_notice_chat_dialog.foreground_kind,
            Some(RuntimeUiStackForegroundKind::TextInput)
        );
        assert_eq!(
            input_notice_chat_dialog.prompt.layer_labels(),
            vec!["input"]
        );
        assert_eq!(
            input_notice_chat_dialog.notice.kind,
            Some(RuntimeDialogNoticeKind::ToastWarning)
        );
        assert_eq!(input_notice_chat_dialog.notice.layer_labels(), vec!["warn"]);
        assert_eq!(input_notice_chat_dialog.chat_depth(), 1);
        assert_eq!(input_notice_chat_dialog.active_group_count(), 3);
        assert_eq!(input_notice_chat_dialog.total_depth(), 3);
        let input_notice_chat =
            build_runtime_ui_stack_panel(&runtime_stack_test_hud(input_notice_chat))
                .expect("stack");
        assert_eq!(
            input_notice_chat.foreground_kind,
            Some(RuntimeUiStackForegroundKind::TextInput)
        );
        assert_eq!(input_notice_chat.prompt_layer_labels(), vec!["input"]);
        assert_eq!(
            input_notice_chat.notice_kind,
            Some(RuntimeDialogNoticeKind::ToastWarning)
        );
        assert_eq!(input_notice_chat.notice_layer_labels(), vec!["warn"]);
        assert_eq!(input_notice_chat.chat_depth(), 1);
        assert_eq!(input_notice_chat.active_group_count(), 3);
        assert_eq!(input_notice_chat.total_depth(), 3);
    }

    #[test]
    fn completed_prompt_history_does_not_keep_stack_prompt_layers_active() {
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.text_input.open_count = 1;
        runtime_ui.text_input.last_id = Some(404);
        runtime_ui.menu.menu_open_count = 1;
        runtime_ui.menu.last_menu_open_id = Some(11);
        runtime_ui.menu.menu_choose_count = 1;
        runtime_ui.menu.last_menu_choose_menu_id = Some(11);
        runtime_ui.menu.text_input_result_count = 1;
        runtime_ui.menu.last_text_input_result_id = Some(404);
        runtime_ui.chat.server_message_count = 1;
        runtime_ui.chat.last_chat_sender_entity_id = Some(42);

        let dialog_stack =
            build_runtime_dialog_stack_panel(&runtime_stack_test_hud(runtime_ui.clone()))
                .expect("dialog stack");
        assert_eq!(
            dialog_stack.foreground_kind,
            Some(RuntimeUiStackForegroundKind::Chat)
        );
        assert!(dialog_stack.prompt.layer_labels().is_empty());
        assert_eq!(dialog_stack.prompt_depth(), 0);
        assert_eq!(dialog_stack.chat_depth(), 1);
        assert_eq!(dialog_stack.total_depth(), 1);

        let stack =
            build_runtime_ui_stack_panel(&runtime_stack_test_hud(runtime_ui)).expect("stack");
        assert_eq!(
            stack.foreground_kind,
            Some(RuntimeUiStackForegroundKind::Chat)
        );
        assert!(stack.prompt_layer_labels().is_empty());
        assert_eq!(stack.prompt_depth(), 0);
        assert_eq!(stack.chat_depth(), 1);
        assert_eq!(stack.total_depth(), 1);
    }
}
