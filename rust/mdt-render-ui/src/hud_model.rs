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
}

/// Structured runtime UI observability projection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeUiObservability {
    pub hud_text: RuntimeHudTextObservability,
    pub toast: RuntimeToastObservability,
    pub text_input: RuntimeTextInputObservability,
    pub admin: RuntimeAdminObservability,
    pub menu: RuntimeMenuObservability,
    pub rules: RuntimeRulesObservability,
    pub world_labels: RuntimeWorldLabelObservability,
    pub live: RuntimeLiveSummaryObservability,
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
pub struct RuntimeMenuObservability {
    pub menu_open_count: u64,
    pub follow_up_menu_open_count: u64,
    pub hide_follow_up_menu_count: u64,
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
