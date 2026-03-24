use crate::panel_model::{
    build_build_config_panel, build_build_interaction_panel, build_build_minimap_assist_panel,
    build_hud_status_panel, build_hud_visibility_panel, build_minimap_panel,
    build_runtime_admin_panel, build_runtime_chat_panel, build_runtime_command_mode_panel,
    build_runtime_dialog_panel, build_runtime_kick_panel, build_runtime_live_effect_panel,
    build_runtime_live_entity_panel, build_runtime_loading_panel, build_runtime_menu_panel,
    build_runtime_reconnect_panel, build_runtime_rules_panel, build_runtime_session_panel,
    build_runtime_ui_notice_panel, build_runtime_ui_stack_panel, build_runtime_world_label_panel,
    MinimapPanelModel, PresenterViewWindow, RuntimeDialogNoticeKind, RuntimeDialogPromptKind,
    RuntimeUiNoticePanelModel,
};
use crate::render_model::{RenderObjectSemanticFamily, RenderObjectSemanticKind};
use crate::{HudModel, RenderModel, ScenePresenter};

const TILE_SIZE: f32 = 8.0;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AsciiScenePresenter {
    last_frame: String,
    max_view_tiles: Option<(usize, usize)>,
}

impl AsciiScenePresenter {
    pub fn with_max_view_tiles(width: usize, height: usize) -> Self {
        Self {
            last_frame: String::new(),
            max_view_tiles: Some((width, height)),
        }
    }

    pub fn last_frame(&self) -> &str {
        &self.last_frame
    }

    fn compose_frame(&self, scene: &RenderModel, hud: &HudModel) -> String {
        let width = (scene.viewport.width / TILE_SIZE).round().max(0.0) as usize;
        let height = (scene.viewport.height / TILE_SIZE).round().max(0.0) as usize;
        let window = crop_window(scene, width, height, self.max_view_tiles);
        let mut grid = vec![vec![' '; window.width]; window.height];
        let mut objects = scene
            .objects
            .iter()
            .filter_map(|object| {
                visible_window_tile(
                    object,
                    window.origin_x,
                    window.origin_y,
                    window.width,
                    window.height,
                )
            })
            .collect::<Vec<_>>();
        objects.sort_by_key(|(object, _, _)| object.layer);
        for (object, local_x, local_y) in objects {
            grid[local_y][local_x] = sprite_for_id(&object.id);
        }

        let mut out = String::new();
        out.push_str(&format!("TITLE: {}\n", hud.title));
        if let Some(wave_text) = hud.wave_text.as_deref().filter(|text| !text.is_empty()) {
            out.push_str(&format!("WAVE: {wave_text}\n"));
        }
        out.push_str(&format!("STATUS: {}\n", hud.status_text));
        if let Some(summary_text) = compose_hud_summary_text(hud) {
            out.push_str(&format!("SUMMARY: {summary_text}\n"));
        }
        if let Some(visibility_text) = compose_hud_visibility_text(hud) {
            out.push_str(&format!("HUD-VIS: {visibility_text}\n"));
        }
        if let Some(detail_text) = compose_hud_detail_text(hud) {
            out.push_str(&format!("HUD-DETAIL: {detail_text}\n"));
        }
        if let Some(minimap_text) = compose_minimap_panel_text(scene, hud, window) {
            out.push_str(&format!("MINIMAP: {minimap_text}\n"));
        }
        if let Some(minimap_visibility_text) = compose_minimap_visibility_line(scene, hud, window) {
            out.push_str(&format!("MINIMAP-VIS: {minimap_visibility_text}\n"));
        }
        if let Some(minimap_kinds_text) = compose_minimap_kind_line(scene, hud) {
            out.push_str(&format!("MINIMAP-KINDS: {minimap_kinds_text}\n"));
        }
        for minimap_detail_text in compose_minimap_detail_lines(scene, hud) {
            out.push_str(&format!("MINIMAP-DETAIL: {minimap_detail_text}\n"));
        }
        if let Some(minimap_legend_text) = compose_minimap_legend_line(hud) {
            out.push_str(&format!("MINIMAP-LEGEND: {minimap_legend_text}\n"));
        }
        if let Some(build_config_text) = compose_build_config_panel_text(hud) {
            out.push_str(&format!("BUILD-CONFIG: {build_config_text}\n"));
        }
        for entry_line in compose_build_config_entry_lines(hud) {
            out.push_str(&format!("BUILD-CONFIG-ENTRY: {entry_line}\n"));
        }
        if let Some(build_config_more_text) = compose_build_config_more_line(hud) {
            out.push_str(&format!("BUILD-CONFIG-MORE: {build_config_more_text}\n"));
        }
        if let Some(build_config_rollback_text) = compose_build_config_rollback_text(hud) {
            out.push_str(&format!("BUILD-ROLLBACK: {build_config_rollback_text}\n"));
        }
        if let Some(build_interaction_text) = compose_build_interaction_text(hud) {
            out.push_str(&format!("BUILD-INTERACTION: {build_interaction_text}\n"));
        }
        if let Some(build_flow_text) = compose_build_flow_text(scene, hud, window) {
            out.push_str(&format!("BUILD-FLOW: {build_flow_text}\n"));
        }
        if let Some(build_text) = compose_build_ui_text(hud) {
            out.push_str(&format!("BUILD: {build_text}\n"));
        }
        for inspector_line in compose_build_ui_inspector_lines(hud) {
            out.push_str(&format!("BUILD-INSPECTOR: {inspector_line}\n"));
        }
        if let Some(overlay_semantics_text) = compose_overlay_semantics_text(scene) {
            out.push_str(&format!("OVERLAY-KINDS: {overlay_semantics_text}\n"));
        }
        if let Some(runtime_ui_text) = compose_runtime_ui_text(hud) {
            out.push_str(&format!("RUNTIME-UI: {runtime_ui_text}\n"));
        }
        if let Some(runtime_ui_notice_text) = compose_runtime_ui_notice_panel_text(hud) {
            out.push_str(&format!("RUNTIME-NOTICE: {runtime_ui_notice_text}\n"));
        }
        if let Some(runtime_ui_notice_detail_text) = compose_runtime_ui_notice_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-NOTICE-DETAIL: {runtime_ui_notice_detail_text}\n"
            ));
        }
        if let Some(runtime_menu_text) = compose_runtime_menu_panel_text(hud) {
            out.push_str(&format!("RUNTIME-MENU: {runtime_menu_text}\n"));
        }
        if let Some(runtime_menu_detail_text) = compose_runtime_menu_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-MENU-DETAIL: {runtime_menu_detail_text}\n"
            ));
        }
        if let Some(runtime_dialog_text) = compose_runtime_dialog_panel_text(hud) {
            out.push_str(&format!("RUNTIME-DIALOG: {runtime_dialog_text}\n"));
        }
        if let Some(runtime_dialog_detail_text) = compose_runtime_dialog_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-DIALOG-DETAIL: {runtime_dialog_detail_text}\n"
            ));
        }
        if let Some(runtime_chat_text) = compose_runtime_chat_panel_text(hud) {
            out.push_str(&format!("RUNTIME-CHAT: {runtime_chat_text}\n"));
        }
        if let Some(runtime_chat_detail_text) = compose_runtime_chat_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-CHAT-DETAIL: {runtime_chat_detail_text}\n"
            ));
        }
        if let Some(runtime_stack_text) = compose_runtime_stack_panel_text(hud) {
            out.push_str(&format!("RUNTIME-STACK: {runtime_stack_text}\n"));
        }
        if let Some(runtime_stack_detail_text) = compose_runtime_stack_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-STACK-DETAIL: {runtime_stack_detail_text}\n"
            ));
        }
        if let Some(runtime_command_text) = compose_runtime_command_mode_panel_text(hud) {
            out.push_str(&format!("RUNTIME-COMMAND: {runtime_command_text}\n"));
        }
        if let Some(runtime_admin_text) = compose_runtime_admin_panel_text(hud) {
            out.push_str(&format!("RUNTIME-ADMIN: {runtime_admin_text}\n"));
        }
        if let Some(runtime_rules_text) = compose_runtime_rules_panel_text(hud) {
            out.push_str(&format!("RUNTIME-RULES: {runtime_rules_text}\n"));
        }
        if let Some(runtime_world_label_text) = compose_runtime_world_label_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-WORLD-LABEL: {runtime_world_label_text}\n"
            ));
        }
        if let Some(runtime_world_label_detail_text) = compose_runtime_world_label_detail_text(hud)
        {
            out.push_str(&format!(
                "RUNTIME-WORLD-LABEL-DETAIL: {runtime_world_label_detail_text}\n"
            ));
        }
        if let Some(runtime_session_text) = compose_runtime_session_row_text(hud) {
            out.push_str(&format!("RUNTIME-SESSION: {runtime_session_text}\n"));
        }
        if let Some(runtime_kick_text) = compose_runtime_kick_row_text(hud) {
            out.push_str(&format!("RUNTIME-KICK: {runtime_kick_text}\n"));
        }
        if let Some(runtime_loading_text) = compose_runtime_loading_row_text(hud) {
            out.push_str(&format!("RUNTIME-LOADING: {runtime_loading_text}\n"));
        }
        if let Some(runtime_reconnect_text) = compose_runtime_reconnect_row_text(hud) {
            out.push_str(&format!("RUNTIME-RECONNECT: {runtime_reconnect_text}\n"));
        }
        if let Some(runtime_live_entity_text) = compose_runtime_live_entity_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-LIVE-ENTITY: {runtime_live_entity_text}\n"
            ));
        }
        if let Some(runtime_live_effect_text) = compose_runtime_live_effect_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-LIVE-EFFECT: {runtime_live_effect_text}\n"
            ));
        }
        if let Some(summary_text) = hud
            .overlay_summary_text
            .as_deref()
            .filter(|text| !text.is_empty())
        {
            out.push_str(&format!("OVERLAY: {summary_text}\n"));
        }
        out.push_str(&format!(
            "VIEWPORT: {}x{} zoom={:.2}\n",
            width, height, scene.viewport.zoom
        ));
        if window.origin_x != 0
            || window.origin_y != 0
            || window.width != width
            || window.height != height
        {
            out.push_str(&format!(
                "WINDOW: origin=({}, {}) size={}x{}\n",
                window.origin_x, window.origin_y, window.width, window.height
            ));
        }
        for y in (0..window.height).rev() {
            for x in 0..window.width {
                let ch = match grid[y][x] {
                    ' ' => '.',
                    other => other,
                };
                out.push(ch);
            }
            if y > 0 {
                out.push('\n');
            }
        }
        out
    }
}

impl ScenePresenter for AsciiScenePresenter {
    fn present(&mut self, scene: &RenderModel, hud: &HudModel) {
        self.last_frame = self.compose_frame(scene, hud);
    }
}

fn crop_window(
    scene: &RenderModel,
    width: usize,
    height: usize,
    max_view_tiles: Option<(usize, usize)>,
) -> PresenterViewWindow {
    let base_window = projected_window(scene, width, height);
    let Some((max_width, max_height)) = max_view_tiles else {
        return base_window;
    };
    if base_window.width <= max_width && base_window.height <= max_height {
        return base_window;
    }

    let focus = scene.player_focus_tile(TILE_SIZE).unwrap_or((
        base_window.origin_x.saturating_add(base_window.width / 2),
        base_window.origin_y.saturating_add(base_window.height / 2),
    ));

    let window_width = max_width.min(base_window.width);
    let window_height = max_height.min(base_window.height);
    PresenterViewWindow {
        origin_x: crop_origin(
            focus.0,
            base_window.origin_x,
            base_window.width,
            window_width,
        ),
        origin_y: crop_origin(
            focus.1,
            base_window.origin_y,
            base_window.height,
            window_height,
        ),
        width: window_width,
        height: window_height,
    }
}

fn projected_window(scene: &RenderModel, width: usize, height: usize) -> PresenterViewWindow {
    scene
        .view_window
        .map(|window| PresenterViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width.min(width),
            height: window.height.min(height),
        })
        .unwrap_or(PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width,
            height,
        })
}

fn crop_origin(focus: usize, origin: usize, bound: usize, window: usize) -> usize {
    let half = window / 2;
    focus
        .saturating_sub(half)
        .clamp(origin, origin.saturating_add(bound.saturating_sub(window)))
}

fn visible_window_tile(
    object: &crate::RenderObject,
    window_x: usize,
    window_y: usize,
    window_width: usize,
    window_height: usize,
) -> Option<(&crate::RenderObject, usize, usize)> {
    let tile_x = world_to_tile_index_floor(object.x) as isize;
    let tile_y = world_to_tile_index_floor(object.y) as isize;
    if tile_x < 0 || tile_y < 0 {
        return None;
    }

    let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
    if tile_x < window_x
        || tile_y < window_y
        || tile_x >= window_x.saturating_add(window_width)
        || tile_y >= window_y.saturating_add(window_height)
    {
        return None;
    }

    Some((object, tile_x - window_x, tile_y - window_y))
}

fn world_to_tile_index_floor(world_position: f32) -> i32 {
    if !world_position.is_finite() {
        return 0;
    }
    (world_position / TILE_SIZE).floor() as i32
}

fn sprite_for_id(id: &str) -> char {
    match RenderObjectSemanticKind::from_id(id).family() {
        RenderObjectSemanticFamily::Player => '@',
        RenderObjectSemanticFamily::Runtime => 'R',
        RenderObjectSemanticFamily::Marker => 'M',
        RenderObjectSemanticFamily::Plan => 'P',
        RenderObjectSemanticFamily::Block => '#',
        RenderObjectSemanticFamily::Terrain => '.',
        RenderObjectSemanticFamily::Unknown => '?',
    }
}

fn compose_hud_summary_text(hud: &HudModel) -> Option<String> {
    let summary = build_hud_status_panel(hud)?;
    Some(format!(
        "player={} team={} selected={} plans={} markers={} map={}x{}",
        compact_runtime_ui_text(Some(summary.player_name.as_str())),
        summary.team_id,
        compact_runtime_ui_text(Some(summary.selected_block.as_str())),
        summary.plan_count,
        summary.marker_count,
        summary.map_width,
        summary.map_height,
    ))
}

fn compose_hud_visibility_text(hud: &HudModel) -> Option<String> {
    let visibility = build_hud_visibility_panel(hud)?;
    Some(format!(
        "overlay={} fog={} known={}({}%) vis={}({}%) hid={}({}%) unseen={}({}%)",
        if visibility.overlay_visible { 1 } else { 0 },
        if visibility.fog_enabled { 1 } else { 0 },
        visibility.known_tile_count,
        visibility.known_tile_percent,
        visibility.visible_tile_count,
        visibility.visible_known_percent,
        visibility.hidden_tile_count,
        visibility.hidden_known_percent,
        visibility.unknown_tile_count,
        visibility.unknown_tile_percent,
    ))
}

fn compose_hud_detail_text(hud: &HudModel) -> Option<String> {
    let summary = build_hud_status_panel(hud)?;
    let visibility = build_hud_visibility_panel(hud)?;
    Some(format!(
        "player={} len={} selected={} len={} tiles={} vis-map={} hidden-map={}",
        compact_runtime_ui_text(Some(summary.player_name.as_str())),
        summary.player_name_len(),
        compact_runtime_ui_text(Some(summary.selected_block.as_str())),
        summary.selected_block_len(),
        summary.map_tile_count(),
        visibility.visible_map_percent(),
        visibility.hidden_map_percent(),
    ))
}

fn compose_runtime_ui_text(hud: &HudModel) -> Option<String> {
    let runtime_ui = hud.runtime_ui.as_ref()?;
    let hud_text = &runtime_ui.hud_text;
    let toast = &runtime_ui.toast;
    let text_input = &runtime_ui.text_input;
    let live = &runtime_ui.live;
    Some(format!(
        "hud={}/{}/{}@{}/{} toast={}/{}@{}/{} tin={}@{}:{}/{}/{}#{}:n{}:e{} live=ent={} fx={}",
        hud_text.set_count,
        hud_text.set_reliable_count,
        hud_text.hide_count,
        compact_runtime_ui_text(hud_text.last_message.as_deref()),
        compact_runtime_ui_text(hud_text.last_reliable_message.as_deref()),
        toast.info_count,
        toast.warning_count,
        compact_runtime_ui_text(toast.last_info_message.as_deref()),
        compact_runtime_ui_text(toast.last_warning_text.as_deref()),
        text_input.open_count,
        optional_i32_label(text_input.last_id),
        compact_runtime_ui_text(text_input.last_title.as_deref()),
        compact_runtime_ui_text(text_input.last_message.as_deref()),
        compact_runtime_ui_text(text_input.last_default_text.as_deref()),
        text_input.last_length.unwrap_or_default(),
        optional_bool_label(text_input.last_numeric),
        optional_bool_label(text_input.last_allow_empty),
        compose_live_entity_text(&live.entity),
        compose_live_effect_text(&live.effect),
    ))
}

fn compose_runtime_ui_notice_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_notice_panel(hud)?;
    Some(format!(
        "hud={}/{}/{}@{}/{} toast={}/{}@{}/{} tin={}@{}:{}/{}/{}#{}:n{}:e{}",
        panel.hud_set_count,
        panel.hud_set_reliable_count,
        panel.hud_hide_count,
        compact_runtime_ui_text(panel.hud_last_message.as_deref()),
        compact_runtime_ui_text(panel.hud_last_reliable_message.as_deref()),
        panel.toast_info_count,
        panel.toast_warning_count,
        compact_runtime_ui_text(panel.toast_last_info_message.as_deref()),
        compact_runtime_ui_text(panel.toast_last_warning_text.as_deref()),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_ui_notice_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_notice_panel(hud)?;
    if runtime_ui_notice_panel_is_empty(&panel) {
        return None;
    }
    Some(format!(
        "active=1 hud-events={}/{}/{} hud-len={}/{} toast-events={}/{} toast-len={}/{} text-input={} id={} title-len={} msg-len={} default-len={} numeric={} allow-empty={}",
        panel.hud_set_count,
        panel.hud_set_reliable_count,
        panel.hud_hide_count,
        runtime_ui_text_len(panel.hud_last_message.as_deref()),
        runtime_ui_text_len(panel.hud_last_reliable_message.as_deref()),
        panel.toast_info_count,
        panel.toast_warning_count,
        runtime_ui_text_len(panel.toast_last_info_message.as_deref()),
        runtime_ui_text_len(panel.toast_last_warning_text.as_deref()),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        runtime_ui_text_len(panel.text_input_last_message.as_deref()),
        runtime_ui_text_len(panel.text_input_last_default_text.as_deref()),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_rules_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_rules_panel(hud)?;
    Some(format!(
        "mut={} fail={} set={}/{}/{} clear={} complete={} state=wv{}:pvp{} obj={} qual={} parents={} flags={} oor={} last={}",
        panel.mutation_count,
        panel.parse_fail_count,
        panel.set_rules_count,
        panel.set_objectives_count,
        panel.set_rule_count,
        panel.clear_objectives_count,
        panel.complete_objective_count,
        optional_bool_label(panel.waves),
        optional_bool_label(panel.pvp),
        panel.objective_count,
        panel.qualified_objective_count,
        panel.objective_parent_edge_count,
        panel.objective_flag_count,
        panel.complete_out_of_range_count,
        optional_i32_label(panel.last_completed_index),
    ))
}

fn compose_runtime_menu_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_menu_panel(hud)?;
    Some(format!(
        "menu={} fmenu={} hide={} tin={}@{}:{}/{}#{}:n{}:e{}",
        panel.menu_open_count,
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_menu_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_menu_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "active={} outstanding-follow-up={} text-input={} id={} title={} default-len={} numeric={} allow-empty={}",
        if panel.text_input_open_count > 0
            || panel.menu_open_count > 0
            || panel.outstanding_follow_up_count() > 0
        {
            1
        } else {
            0
        },
        panel.outstanding_follow_up_count(),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        panel.default_text_len(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_dialog_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_dialog_panel(hud)?;
    Some(format!(
        "prompt={} act={} menu={}/{}/{} tin={}@{}:{}/{}/{}#{}:n{}:e{} notice={}@{} total={}",
        runtime_dialog_prompt_text(panel.prompt_kind),
        if panel.prompt_active { 1 } else { 0 },
        panel.menu_open_count,
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
        runtime_dialog_notice_text(panel.notice_kind),
        compact_runtime_ui_text(panel.notice_text.as_deref()),
        panel.notice_count,
    ))
}

fn compose_runtime_dialog_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_dialog_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "prompt={} active={} outstanding-follow-up={} message-len={} default-len={} notice={} notice-len={}",
        runtime_dialog_prompt_text(panel.prompt_kind),
        if panel.prompt_active { 1 } else { 0 },
        panel.outstanding_follow_up_count(),
        panel.prompt_message_len(),
        panel.default_text_len(),
        runtime_dialog_notice_text(panel.notice_kind),
        panel.notice_text_len(),
    ))
}

fn compose_runtime_chat_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_chat_panel(hud)?;
    Some(format!(
        "srv={} last-srv={} chat={} last-chat={} raw={} sender={}",
        panel.server_message_count,
        compact_runtime_ui_text(panel.last_server_message.as_deref()),
        panel.chat_message_count,
        compact_runtime_ui_text(panel.last_chat_message.as_deref()),
        compact_runtime_ui_text(panel.last_chat_unformatted.as_deref()),
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_chat_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_chat_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "server-len={} chat-len={} raw-len={} formatted-eq-raw={} sender={}",
        panel.last_server_message_len(),
        panel.last_chat_message_len(),
        panel.last_chat_unformatted_len(),
        optional_bool_label(panel.formatted_matches_unformatted()),
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_stack_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_stack_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let prompt_layers = panel.prompt_layer_labels().join(">");
    let notice_layers = panel.notice_layer_labels().join(">");
    Some(format!(
        "front={} prompt={}@{} notice={}@{} chat={} groups={} total={} tin={} sender={}",
        panel.foreground_label(),
        panel.prompt_depth(),
        if prompt_layers.is_empty() {
            "none"
        } else {
            prompt_layers.as_str()
        },
        runtime_dialog_notice_text(panel.notice_kind),
        if notice_layers.is_empty() {
            "none"
        } else {
            notice_layers.as_str()
        },
        panel.chat_depth(),
        panel.active_group_count(),
        panel.total_depth(),
        optional_i32_label(panel.text_input_last_id),
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_stack_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_stack_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "menu-active={} outstanding-follow-up={} text-input={} notice-depth={} server-chat={}/{} sender={}",
        if panel.menu_active { 1 } else { 0 },
        panel.outstanding_follow_up_count,
        panel.text_input_open_count,
        panel.notice_depth(),
        panel.server_message_count,
        panel.chat_message_count,
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_command_mode_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_command_mode_panel(hud)?;
    Some(format!(
        "act={} sel={}@{} bld={}@{} rect={} groups={} target={} cmd={} stance={}",
        if panel.active { 1 } else { 0 },
        panel.selected_unit_count,
        command_i32_sample_text(&panel.selected_unit_sample),
        panel.command_building_count,
        optional_i32_label(panel.first_command_building),
        command_rect_text(panel.command_rect),
        command_control_groups_text(&panel.control_groups),
        command_target_text(panel.last_target),
        optional_u8_label(
            panel
                .last_command_selection
                .and_then(|selection| selection.command_id)
        ),
        command_stance_text(panel.last_stance_selection),
    ))
}

fn compose_runtime_admin_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_admin_panel(hud)?;
    Some(format!(
        "trace={}@{} fail={} dbg={}/{}@{} fail={}",
        panel.trace_info_count,
        optional_i32_label(panel.last_trace_info_player_id),
        panel.trace_info_parse_fail_count,
        panel.debug_status_client_count,
        panel.debug_status_client_unreliable_count,
        optional_i32_label(panel.last_debug_status_value),
        panel.parse_fail_count,
    ))
}

fn compose_runtime_world_label_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_world_label_panel(hud)?;
    Some(format!(
        "set={} rel={} remove={} total={} active={} inactive={} last={} flags={} font={} z={} pos={} text={} lines={} len={}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.total_count,
        panel.active_count,
        panel.inactive_count(),
        optional_i32_label(panel.last_entity_id),
        optional_u8_label(panel.last_flags),
        runtime_world_label_scalar_text(panel.last_font_size_bits, panel.last_font_size()),
        runtime_world_label_scalar_text(panel.last_z_bits, panel.last_z()),
        world_position_text(panel.last_position.as_ref()),
        runtime_world_label_text_sample(panel.last_text.as_deref()),
        panel.last_text_line_count(),
        panel.last_text_len(),
    ))
}

fn compose_runtime_world_label_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_world_label_panel(hud)?;
    if panel.label_count == 0
        && panel.reliable_label_count == 0
        && panel.remove_label_count == 0
        && panel.active_count == 0
        && panel.last_entity_id.is_none()
        && panel.last_text.is_none()
        && panel.last_flags.is_none()
        && panel.last_font_size_bits.is_none()
        && panel.last_z_bits.is_none()
        && panel.last_position.is_none()
    {
        return None;
    }

    Some(format!(
        "set={} rel={} remove={} active={} inactive={} last={} flags={} text-len={} lines={} font={} z={} pos={}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.active_count,
        panel.inactive_count(),
        optional_i32_label(panel.last_entity_id),
        optional_u8_label(panel.last_flags),
        panel.last_text_len(),
        panel.last_text_line_count(),
        runtime_world_label_scalar_text(panel.last_font_size_bits, panel.last_font_size()),
        runtime_world_label_scalar_text(panel.last_z_bits, panel.last_z()),
        world_position_text(panel.last_position.as_ref()),
    ))
}

fn runtime_world_label_text_sample(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    let sample = value.chars().take(24).collect::<String>();
    if value.chars().count() > 24 {
        format!("{sample}...")
    } else {
        sample
    }
}

fn compose_runtime_kick_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_kick_panel(hud)?;
    Some(compose_runtime_kick_panel_text(&panel))
}

fn compose_runtime_session_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "kick={}; loading={}; reconnect={}",
        compose_runtime_kick_panel_text(&panel.kick),
        compose_runtime_loading_panel_text(&panel.loading),
        compose_runtime_reconnect_panel_text(&panel.reconnect),
    ))
}

fn compose_runtime_loading_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    Some(compose_runtime_loading_panel_text(&panel))
}

fn compose_runtime_reconnect_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_reconnect_panel(hud)?;
    Some(compose_runtime_reconnect_panel_text(&panel))
}

fn compose_runtime_live_entity_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(compose_live_entity_panel_text(&panel))
}

fn compose_runtime_live_effect_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_effect_panel(hud)?;
    Some(compose_live_effect_panel_text(&panel))
}

fn compose_build_ui_text(hud: &HudModel) -> Option<String> {
    let build_ui = hud.build_ui.as_ref()?;
    Some(compose_build_ui_summary_text(build_ui))
}

fn compose_minimap_panel_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "map={}x{} window={}:{}->{}:{} size={}x{} cover={}/{}({}%) focus={} in-window={} drift={}:{} edges={}/{}/{}/{}",
        panel.map_width,
        panel.map_height,
        panel.window.origin_x,
        panel.window.origin_y,
        panel.window_last_x,
        panel.window_last_y,
        panel.window.width,
        panel.window.height,
        panel.window_tile_count,
        panel.map_tile_count,
        panel.window_coverage_percent,
        optional_focus_tile_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        optional_signed_tile_text(panel.focus_offset_x),
        optional_signed_tile_text(panel.focus_offset_y),
        bool_flag(panel.window_clamped_left),
        bool_flag(panel.window_clamped_top),
        bool_flag(panel.window_clamped_right),
        bool_flag(panel.window_clamped_bottom),
    ))
}

fn compose_minimap_visibility_line(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "overlay={} fog={} known={}({}%) vis={}({}%) hid={}({}%) unseen={}({}%)",
        if panel.overlay_visible { 1 } else { 0 },
        if panel.fog_enabled { 1 } else { 0 },
        panel.known_tile_count,
        panel.known_tile_percent,
        panel.visible_tile_count,
        panel.visible_known_percent,
        panel.hidden_tile_count,
        panel.hidden_known_percent,
        panel.unknown_tile_count,
        panel.unknown_tile_percent,
    ))
}

fn compose_minimap_kind_line(scene: &RenderModel, hud: &HudModel) -> Option<String> {
    let panel = build_minimap_panel(
        scene,
        hud,
        PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: 0,
            height: 0,
        },
    )?;
    let mut text = format!(
        "tracked={} player={} marker={} plan={} block={} runtime={} terrain={} unknown={}",
        panel.tracked_object_count,
        panel.player_count,
        panel.marker_count,
        panel.plan_count,
        panel.block_count,
        panel.runtime_count,
        panel.terrain_count,
        panel.unknown_count,
    );
    if let Some(detail_text) = semantic_detail_text(&panel.detail_counts) {
        text.push_str(" detail=");
        text.push_str(&detail_text);
    }
    Some(text)
}

fn compose_minimap_detail_lines(scene: &RenderModel, hud: &HudModel) -> Vec<String> {
    let Some(panel) = build_minimap_panel(
        scene,
        hud,
        PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: 0,
            height: 0,
        },
    ) else {
        return Vec::new();
    };

    let detail_count = panel.detail_counts.len();
    let mut lines = panel
        .detail_counts
        .iter()
        .enumerate()
        .map(|(index, detail)| {
            format!(
                "{}/{} {}={}",
                index + 1,
                detail_count,
                detail.label,
                detail.count
            )
        })
        .collect::<Vec<_>>();
    lines.push(compose_minimap_window_distribution_line(&panel));
    lines
}

fn compose_minimap_window_distribution_line(panel: &MinimapPanelModel) -> String {
    format!(
        "miniwin:win{}:off{}@pl{}:mk{}:pn{}:bk{}:rt{}:tr{}:uk{}",
        panel.window_tracked_object_count,
        panel.outside_window_count,
        panel.window_player_count,
        panel.window_marker_count,
        panel.window_plan_count,
        panel.window_block_count,
        panel.window_runtime_count,
        panel.window_terrain_count,
        panel.window_unknown_count,
    )
}

fn compose_minimap_legend_line(hud: &HudModel) -> Option<String> {
    hud.summary.as_ref()?;
    Some("@=player M=marker P=plan #=block R=runtime .=terrain ?=unknown".to_string())
}

fn compose_build_config_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, 3)?;
    Some(format!(
        "sel={} rot={} mode={} pending={}/{} hist={}/{} orphan={} head={} align={} families={}/{} tracked={}",
        optional_i16_label(panel.selected_block_id),
        panel.selected_rotation,
        if panel.building { "build" } else { "idle" },
        panel.queued_count,
        panel.inflight_count,
        panel.finished_count,
        panel.removed_count,
        panel.orphan_authoritative_count,
        build_config_head_text(panel.head.as_ref()),
        build_config_alignment_text(panel.selected_matches_head),
        panel.entries.len(),
        panel.tracked_family_count,
        panel.tracked_sample_count,
    ))
}

fn compose_build_config_entry_lines(hud: &HudModel) -> Vec<String> {
    let Some(panel) = build_build_config_panel(hud, 3) else {
        return Vec::new();
    };
    panel
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                "{}/{} {}#{}@{}",
                index + 1,
                panel.tracked_family_count,
                compact_runtime_ui_text(Some(entry.family.as_str())),
                entry.tracked_count,
                compact_build_inspector_text(entry.sample.as_str(), 56),
            )
        })
        .collect()
}

fn compose_build_config_more_line(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, 3)?;
    (panel.truncated_family_count > 0).then(|| {
        format!(
            "+{} hidden families beyond cap",
            panel.truncated_family_count
        )
    })
}

fn compose_build_config_rollback_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, 3)?;
    let strip = &panel.rollback_strip;
    Some(format!(
        "authoritative={} rollback={} last={} src={} business={} clear={} last-rb={} pending={} outcome={} block={}",
        strip.applied_authoritative_count,
        strip.rollback_count,
        build_config_tile_text(strip.last_build_tile),
        build_config_rollback_source_text(strip.last_source),
        if strip.last_business_applied { 1 } else { 0 },
        if strip.last_cleared_pending_local { 1 } else { 0 },
        if strip.last_was_rollback { 1 } else { 0 },
        build_config_pending_match_text(strip.last_pending_local_match),
        build_config_outcome_text(strip.last_configured_outcome),
        compact_runtime_ui_text(strip.last_configured_block_name.as_deref()),
    ))
}

fn compose_build_interaction_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_interaction_panel(hud)?;
    Some(format!(
        "mode={} select={} queue={} pending={} place-ready={} cfg={}/{} top={} head={} auth={} pending={} src={} tile={} block={} orphan={}",
        build_interaction_mode_text(panel.mode),
        build_interaction_selection_text(panel.selection_state),
        build_interaction_queue_text(panel.queue_state),
        panel.pending_count,
        if panel.place_ready { 1 } else { 0 },
        panel.config_family_count,
        panel.config_sample_count,
        compact_runtime_ui_text(panel.top_config_family.as_deref()),
        build_config_head_text(panel.head.as_ref()),
        build_interaction_authority_text(panel.authority_state),
        build_config_pending_match_text(panel.authority_pending_match),
        build_interaction_authority_source_text(panel.authority_source),
        build_config_tile_text(panel.authority_tile),
        compact_runtime_ui_text(panel.authority_block_name.as_deref()),
        panel.orphan_authoritative_count,
    ))
}

fn compose_build_flow_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(format!(
        "next={} mode={} select={} queue={} place-ready={} focus={} vis={} cover={} scope={} auth={} runtime-share={}%",
        panel.next_action_label(),
        build_interaction_mode_text(panel.mode),
        build_interaction_selection_text(panel.selection_state),
        build_interaction_queue_text(panel.queue_state),
        if panel.place_ready { 1 } else { 0 },
        panel.focus_state_label(),
        panel.map_visibility_label(),
        panel.window_coverage_label(),
        panel.config_scope_label(),
        build_interaction_authority_text(panel.authority_state),
        panel.runtime_share_percent(),
    ))
}

fn compose_build_ui_inspector_lines(hud: &HudModel) -> Vec<String> {
    let Some(build_ui) = hud.build_ui.as_ref() else {
        return Vec::new();
    };

    build_ui
        .inspector_entries
        .iter()
        .map(|entry| {
            format!(
                "family={} tracked={} sample={}",
                compact_runtime_ui_text(Some(entry.family.as_str())),
                entry.tracked_count,
                compact_build_inspector_text(entry.sample.as_str(), 72),
            )
        })
        .collect()
}

fn compact_build_inspector_text(value: &str, limit: usize) -> String {
    let mut compact = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index == limit {
            compact.push('~');
            break;
        }
        compact.push(match ch {
            ' ' | '\t' | '\r' | '\n' => '_',
            _ => ch,
        });
    }
    if compact.is_empty() {
        "-".to_string()
    } else {
        compact
    }
}

fn compose_build_ui_summary_text(build_ui: &crate::BuildUiObservability) -> String {
    format!(
        "sel={} rot={} building={} queue={}/{}/{}/{}/{} head={} cfg={}",
        optional_i16_label(build_ui.selected_block_id),
        build_ui.selected_rotation,
        if build_ui.building { 1 } else { 0 },
        build_ui.queued_count,
        build_ui.inflight_count,
        build_ui.finished_count,
        build_ui.removed_count,
        build_ui.orphan_authoritative_count,
        build_queue_head_text(build_ui.head.as_ref()),
        build_ui.inspector_entries.len(),
    )
}

fn build_config_head_text(head: Option<&crate::panel_model::BuildConfigHeadModel>) -> String {
    let Some(head) = head else {
        return "none".to_string();
    };
    let stage = match head.stage {
        crate::BuildQueueHeadStage::Queued => "queued",
        crate::BuildQueueHeadStage::InFlight => "flight",
        crate::BuildQueueHeadStage::Finished => "finish",
        crate::BuildQueueHeadStage::Removed => "remove",
    };
    let mode = if head.breaking { "break" } else { "place" };
    format!(
        "{stage}@{}:{}:{mode}:b{}:r{}",
        head.x,
        head.y,
        optional_i16_label(head.block_id),
        optional_u8_label(head.rotation),
    )
}

fn build_config_tile_text(value: Option<(i32, i32)>) -> String {
    match value {
        Some((x, y)) => format!("{x}:{y}"),
        None => "none".to_string(),
    }
}

fn build_config_rollback_source_text(
    value: Option<crate::BuildConfigAuthoritySourceObservability>,
) -> &'static str {
    match value {
        Some(crate::BuildConfigAuthoritySourceObservability::TileConfig) => "tileConfig",
        Some(crate::BuildConfigAuthoritySourceObservability::ConstructFinish) => "constructFinish",
        None => "none",
    }
}

fn build_config_pending_match_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "match",
        Some(false) => "mismatch",
        None => "none",
    }
}

fn build_interaction_mode_text(value: crate::panel_model::BuildInteractionMode) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionMode::Idle => "idle",
        crate::panel_model::BuildInteractionMode::Place => "place",
        crate::panel_model::BuildInteractionMode::Break => "break",
    }
}

fn build_interaction_selection_text(
    value: crate::panel_model::BuildInteractionSelectionState,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionSelectionState::Unarmed => "unarmed",
        crate::panel_model::BuildInteractionSelectionState::Armed => "armed",
        crate::panel_model::BuildInteractionSelectionState::HeadAligned => "head-aligned",
        crate::panel_model::BuildInteractionSelectionState::HeadDiverged => "head-diverged",
        crate::panel_model::BuildInteractionSelectionState::BreakingHead => "break-head",
    }
}

fn build_interaction_queue_text(
    value: crate::panel_model::BuildInteractionQueueState,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionQueueState::Empty => "empty",
        crate::panel_model::BuildInteractionQueueState::Queued => "queued",
        crate::panel_model::BuildInteractionQueueState::InFlight => "inflight",
        crate::panel_model::BuildInteractionQueueState::Mixed => "mixed",
    }
}

fn build_interaction_authority_text(
    value: crate::panel_model::BuildInteractionAuthorityState,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionAuthorityState::None => "none",
        crate::panel_model::BuildInteractionAuthorityState::Applied => "applied",
        crate::panel_model::BuildInteractionAuthorityState::Cleared => "cleared",
        crate::panel_model::BuildInteractionAuthorityState::Rollback => "rollback",
        crate::panel_model::BuildInteractionAuthorityState::RejectedMissingBuilding => {
            "rejected-missing-building"
        }
        crate::panel_model::BuildInteractionAuthorityState::RejectedMissingBlockMetadata => {
            "rejected-missing-metadata"
        }
        crate::panel_model::BuildInteractionAuthorityState::RejectedUnsupportedBlock => {
            "rejected-unsupported-block"
        }
        crate::panel_model::BuildInteractionAuthorityState::RejectedUnsupportedConfigType => {
            "rejected-unsupported-config"
        }
    }
}

fn build_interaction_authority_source_text(
    value: Option<crate::BuildConfigAuthoritySourceObservability>,
) -> &'static str {
    match value {
        Some(crate::BuildConfigAuthoritySourceObservability::TileConfig) => "tileConfig",
        Some(crate::BuildConfigAuthoritySourceObservability::ConstructFinish) => "constructFinish",
        None => "none",
    }
}

fn build_config_outcome_text(
    value: Option<crate::BuildConfigOutcomeObservability>,
) -> &'static str {
    match value {
        Some(crate::BuildConfigOutcomeObservability::Applied) => "applied",
        Some(crate::BuildConfigOutcomeObservability::RejectedMissingBuilding) => {
            "rejected-missing-building"
        }
        Some(crate::BuildConfigOutcomeObservability::RejectedMissingBlockMetadata) => {
            "rejected-missing-block-metadata"
        }
        Some(crate::BuildConfigOutcomeObservability::RejectedUnsupportedBlock) => {
            "rejected-unsupported-block"
        }
        Some(crate::BuildConfigOutcomeObservability::RejectedUnsupportedConfigType) => {
            "rejected-unsupported-config-type"
        }
        None => "none",
    }
}

fn optional_focus_tile_text(value: Option<(usize, usize)>) -> String {
    match value {
        Some((x, y)) => format!("{x}:{y}"),
        None => "-".to_string(),
    }
}

fn optional_signed_tile_text(value: Option<isize>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

fn build_config_alignment_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "match",
        Some(false) => "split",
        None => "none",
    }
}

fn compose_live_entity_text(entity: &crate::RuntimeLiveEntitySummaryObservability) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}:tp{}/{}:last{}/{}/{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_text(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
        entity.player_count,
        entity.unit_count,
        optional_i32_label(entity.last_entity_id),
        optional_i32_label(entity.last_player_entity_id),
        optional_i32_label(entity.last_unit_entity_id),
    )
}

fn compose_live_entity_panel_text(
    entity: &crate::panel_model::RuntimeLiveEntityPanelModel,
) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}:tp{}/{}:last{}/{}/{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_text(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
        entity.player_count,
        entity.unit_count,
        optional_i32_label(entity.last_entity_id),
        optional_i32_label(entity.last_player_entity_id),
        optional_i32_label(entity.last_unit_entity_id),
    )
}

fn compose_live_effect_text(effect: &crate::RuntimeLiveEffectSummaryObservability) -> String {
    format!(
        "{}/{}@{}:u{}:k{}:c{}/{}:p{}@{}",
        effect.effect_count,
        effect.spawn_effect_count,
        optional_i16_label(effect.last_effect_id),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.last_contract_name.as_deref()),
        compact_runtime_ui_text(effect.last_reliable_contract_name.as_deref()),
        live_effect_position_source_text(effect.last_position_source),
        world_position_text(effect.last_position_hint.as_ref()),
    )
}

fn compose_live_effect_panel_text(
    effect: &crate::panel_model::RuntimeLiveEffectPanelModel,
) -> String {
    format!(
        "{}/{}@{}:u{}:k{}:c{}/{}:p{}@{}",
        effect.effect_count,
        effect.spawn_effect_count,
        optional_i16_label(effect.last_effect_id),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.last_contract_name.as_deref()),
        compact_runtime_ui_text(effect.last_reliable_contract_name.as_deref()),
        live_effect_position_source_text(effect.last_position_source),
        world_position_text(effect.last_position_hint.as_ref()),
    )
}

fn compose_runtime_kick_panel_text(kick: &crate::panel_model::RuntimeKickPanelModel) -> String {
    format!(
        "{}@{}:{}:{}",
        compact_runtime_ui_text(kick.reason_text.as_deref()),
        optional_i32_label(kick.reason_ordinal),
        compact_runtime_ui_text(kick.hint_category.as_deref()),
        compact_runtime_ui_text(kick.hint_text.as_deref()),
    )
}

fn compose_runtime_loading_panel_text(
    loading: &crate::panel_model::RuntimeLoadingPanelModel,
) -> String {
    format!(
        "defer{} replay{} drop{} qdrop{} sfail{} scfail{} efail{} rdy{}@{} to{}/{}/{} lt{}@{} rs{}/{}/{}/{} lr{} lwr{}",
        loading.deferred_inbound_packet_count,
        loading.replayed_inbound_packet_count,
        loading.dropped_loading_low_priority_packet_count,
        loading.dropped_loading_deferred_overflow_count,
        loading.failed_state_snapshot_parse_count,
        loading.failed_state_snapshot_core_data_parse_count,
        loading.failed_entity_snapshot_parse_count,
        loading.ready_inbound_liveness_anchor_count,
        optional_u64_label(loading.last_ready_inbound_liveness_anchor_at_ms),
        loading.timeout_count,
        loading.connect_or_loading_timeout_count,
        loading.ready_snapshot_timeout_count,
        runtime_session_timeout_kind_text(loading.last_timeout_kind),
        optional_u64_label(loading.last_timeout_idle_ms),
        loading.reset_count,
        loading.reconnect_reset_count,
        loading.world_reload_count,
        loading.kick_reset_count,
        runtime_session_reset_kind_text(loading.last_reset_kind),
        runtime_world_reload_panel_text(loading.last_world_reload.as_ref()),
    )
}

fn compose_runtime_reconnect_panel_text(
    reconnect: &crate::panel_model::RuntimeReconnectPanelModel,
) -> String {
    format!(
        "{}#{} {} redirect={}@{}:{} reason={}#{} hint={}",
        runtime_reconnect_phase_text(reconnect.phase),
        reconnect.phase_transition_count,
        runtime_reconnect_reason_kind_text(reconnect.reason_kind),
        reconnect.redirect_count,
        compact_runtime_ui_text(reconnect.last_redirect_ip.as_deref()),
        optional_i32_label(reconnect.last_redirect_port),
        compact_runtime_ui_text(reconnect.reason_text.as_deref()),
        optional_i32_label(reconnect.reason_ordinal),
        compact_runtime_ui_text(reconnect.hint_text.as_deref()),
    )
}

fn runtime_session_timeout_kind_text(
    kind: Option<crate::hud_model::RuntimeSessionTimeoutKind>,
) -> &'static str {
    match kind {
        Some(crate::hud_model::RuntimeSessionTimeoutKind::ConnectOrLoading) => "cload",
        Some(crate::hud_model::RuntimeSessionTimeoutKind::ReadySnapshotStall) => "ready",
        None => "none",
    }
}

fn runtime_session_reset_kind_text(
    kind: Option<crate::hud_model::RuntimeSessionResetKind>,
) -> &'static str {
    match kind {
        Some(crate::hud_model::RuntimeSessionResetKind::Reconnect) => "reconnect",
        Some(crate::hud_model::RuntimeSessionResetKind::WorldReload) => "reload",
        Some(crate::hud_model::RuntimeSessionResetKind::Kick) => "kick",
        None => "none",
    }
}

fn runtime_world_reload_panel_text(
    world_reload: Option<&crate::panel_model::RuntimeWorldReloadPanelModel>,
) -> String {
    match world_reload {
        Some(world_reload) => format!(
            "@lw{}:cl{}:rd{}:cc{}:p{}:d{}:r{}",
            if world_reload.had_loaded_world { 1 } else { 0 },
            if world_reload.had_client_loaded { 1 } else { 0 },
            if world_reload.was_ready_to_enter_world {
                1
            } else {
                0
            },
            if world_reload.had_connect_confirm_sent {
                1
            } else {
                0
            },
            world_reload.cleared_pending_packets,
            world_reload.cleared_deferred_inbound_packets,
            world_reload.cleared_replayed_loading_events,
        ),
        None => "none".to_string(),
    }
}

fn runtime_reconnect_phase_text(
    phase: crate::hud_model::RuntimeReconnectPhaseObservability,
) -> &'static str {
    match phase {
        crate::hud_model::RuntimeReconnectPhaseObservability::Idle => "idle",
        crate::hud_model::RuntimeReconnectPhaseObservability::Scheduled => "sched",
        crate::hud_model::RuntimeReconnectPhaseObservability::Attempting => "attempt",
        crate::hud_model::RuntimeReconnectPhaseObservability::Succeeded => "ok",
        crate::hud_model::RuntimeReconnectPhaseObservability::Aborted => "abort",
    }
}

fn runtime_reconnect_reason_kind_text(
    kind: Option<crate::hud_model::RuntimeReconnectReasonKind>,
) -> &'static str {
    match kind {
        Some(crate::hud_model::RuntimeReconnectReasonKind::ConnectRedirect) => "redirect",
        Some(crate::hud_model::RuntimeReconnectReasonKind::Kick) => "kick",
        Some(crate::hud_model::RuntimeReconnectReasonKind::Timeout) => "timeout",
        Some(crate::hud_model::RuntimeReconnectReasonKind::ManualConnect) => "manual",
        None => "none",
    }
}

fn compose_overlay_semantics_text(scene: &RenderModel) -> Option<String> {
    let summary = scene.semantic_summary();
    if summary.total_count == 0 {
        return None;
    }

    let mut text = format!(
        "players={} markers={} plans={} blocks={} runtime={} terrain={} unknown={}",
        summary.player_count,
        summary.marker_count,
        summary.plan_count,
        summary.block_count,
        summary.runtime_count,
        summary.terrain_count,
        summary.unknown_count,
    );
    if let Some(detail_text) = summary.detail_text() {
        text.push_str(" detail=");
        text.push_str(&detail_text);
    }
    Some(text)
}

fn semantic_detail_text(
    detail_counts: &[crate::render_model::RenderSemanticDetailCount],
) -> Option<String> {
    if detail_counts.is_empty() {
        return None;
    }

    Some(
        detail_counts
            .iter()
            .map(|detail| format!("{}:{}", detail.label, detail.count))
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn build_queue_head_text(head: Option<&crate::BuildQueueHeadObservability>) -> String {
    let Some(head) = head else {
        return "none".to_string();
    };

    let stage = match head.stage {
        crate::BuildQueueHeadStage::Queued => "queued",
        crate::BuildQueueHeadStage::InFlight => "flight",
        crate::BuildQueueHeadStage::Finished => "finish",
        crate::BuildQueueHeadStage::Removed => "remove",
    };
    let mode = if head.breaking { "break" } else { "place" };
    format!(
        "{stage}@{}:{}:{mode}:b{}:r{}",
        head.x,
        head.y,
        optional_i16_label(head.block_id),
        optional_u8_label(head.rotation),
    )
}

fn compact_runtime_ui_text(value: Option<&str>) -> String {
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

fn runtime_ui_text_len(value: Option<&str>) -> usize {
    value.map(str::chars).map(Iterator::count).unwrap_or_default()
}

fn runtime_ui_notice_panel_is_empty(panel: &RuntimeUiNoticePanelModel) -> bool {
    panel.hud_set_count == 0
        && panel.hud_set_reliable_count == 0
        && panel.hud_hide_count == 0
        && panel.hud_last_message.is_none()
        && panel.hud_last_reliable_message.is_none()
        && panel.toast_info_count == 0
        && panel.toast_warning_count == 0
        && panel.toast_last_info_message.is_none()
        && panel.toast_last_warning_text.is_none()
        && panel.text_input_open_count == 0
        && panel.text_input_last_id.is_none()
        && panel.text_input_last_title.is_none()
        && panel.text_input_last_message.is_none()
        && panel.text_input_last_default_text.is_none()
        && panel.text_input_last_length.is_none()
        && panel.text_input_last_numeric.is_none()
        && panel.text_input_last_allow_empty.is_none()
}

fn optional_i32_label(value: Option<i32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_i16_label(value: Option<i16>) -> String {
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

fn runtime_world_label_scalar_text(bits: Option<u32>, value: Option<f32>) -> String {
    match (bits, value) {
        (Some(bits), Some(value)) => format!("{bits}@{value:.1}"),
        (Some(bits), None) => bits.to_string(),
        (None, _) => "none".to_string(),
    }
}

fn optional_u64_label(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn world_position_text(value: Option<&crate::RuntimeWorldPositionObservability>) -> String {
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

fn live_effect_position_source_text(
    source: Option<crate::RuntimeLiveEffectPositionSource>,
) -> &'static str {
    match source {
        Some(crate::RuntimeLiveEffectPositionSource::BusinessProjection) => "biz",
        Some(crate::RuntimeLiveEffectPositionSource::EffectPacket) => "pkt",
        Some(crate::RuntimeLiveEffectPositionSource::SpawnEffectPacket) => "spawn",
        None => "none",
    }
}

fn optional_bool_label(value: Option<bool>) -> char {
    match value {
        Some(true) => '1',
        Some(false) => '0',
        None => 'n',
    }
}

fn bool_flag(value: bool) -> u8 {
    u8::from(value)
}

fn runtime_dialog_prompt_text(kind: Option<RuntimeDialogPromptKind>) -> &'static str {
    match kind {
        Some(RuntimeDialogPromptKind::Menu) => "menu",
        Some(RuntimeDialogPromptKind::FollowUpMenu) => "follow",
        Some(RuntimeDialogPromptKind::TextInput) => "input",
        None => "none",
    }
}

fn runtime_dialog_notice_text(kind: Option<RuntimeDialogNoticeKind>) -> &'static str {
    match kind {
        Some(RuntimeDialogNoticeKind::Hud) => "hud",
        Some(RuntimeDialogNoticeKind::HudReliable) => "hud-rel",
        Some(RuntimeDialogNoticeKind::ToastInfo) => "toast",
        Some(RuntimeDialogNoticeKind::ToastWarning) => "warn",
        None => "none",
    }
}

fn command_i32_sample_text(values: &[i32]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn command_rect_text(value: Option<crate::RuntimeCommandRectObservability>) -> String {
    value
        .map(|rect| format!("{}:{}:{}:{}", rect.x0, rect.y0, rect.x1, rect.y1))
        .unwrap_or_else(|| "none".to_string())
}

fn command_control_groups_text(
    groups: &[crate::panel_model::RuntimeCommandControlGroupPanelModel],
) -> String {
    if groups.is_empty() {
        return "none".to_string();
    }
    groups
        .iter()
        .map(|group| {
            format!(
                "{}#{}@{}",
                group.index,
                group.unit_count,
                optional_i32_label(group.first_unit_id)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn command_target_text(value: Option<crate::RuntimeCommandTargetObservability>) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    let unit_target = value
        .unit_target
        .map(|unit| format!("{}:{}", unit.kind, unit.value))
        .unwrap_or_else(|| "none".to_string());
    let position_target = value
        .position_target
        .map(|position| format!("0x{:08x}:0x{:08x}", position.x_bits, position.y_bits))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "b{}:u{}:p{}:r{}",
        optional_i32_label(value.build_target),
        unit_target,
        position_target,
        command_rect_text(value.rect_target)
    )
}

fn command_stance_text(value: Option<crate::RuntimeCommandStanceObservability>) -> String {
    value
        .map(|stance| {
            format!(
                "{}/{}",
                optional_u8_label(stance.stance_id),
                if stance.enabled { 1 } else { 0 }
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::AsciiScenePresenter;
    use crate::{
        hud_model::{
            HudSummary, RuntimeReconnectObservability, RuntimeReconnectPhaseObservability,
            RuntimeReconnectReasonKind, RuntimeSessionObservability, RuntimeSessionResetKind,
            RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
        },
        project_scene_models, project_scene_models_with_view_window, HudModel, RenderModel,
        RenderObject, RuntimeAdminObservability, RuntimeHudTextObservability,
        RuntimeMenuObservability, RuntimeRulesObservability, RuntimeTextInputObservability,
        RuntimeToastObservability, RuntimeUiObservability, RuntimeWorldLabelObservability,
        ScenePresenter, Viewport,
    };
    use mdt_world::parse_world_bundle;

    #[test]
    fn ascii_presenter_renders_projected_scene_layers() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();
        let (scene, hud) = project_scene_models(&session, "fr");
        let contract = session.enter_render_contract("fr");
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("TITLE: Golden Deterministic"));
        assert!(frame.contains(&format!(
            "WAVE: {}",
            contract.hud.wave_text.as_deref().unwrap_or_default()
        )));
        assert!(frame.contains(&format!(
            "STATUS: {}",
            contract.hud.status_text.as_deref().unwrap_or_default()
        )));
        assert!(frame.contains(&format!(
            "OVERLAY: {}",
            contract.overlay.summary_text.as_deref().unwrap_or_default()
        )));
        assert!(frame.contains("@"));
        assert!(frame.contains("M"));
        assert!(frame.contains("P"));
        assert!(frame.contains("#"));
    }

    #[test]
    fn ascii_presenter_can_crop_around_player_focus() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();
        let (scene, hud) = project_scene_models(&session, "fr");
        let mut presenter = AsciiScenePresenter::with_max_view_tiles(4, 4);

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("WINDOW: origin="));
    }

    #[test]
    fn ascii_presenter_honors_projected_view_window_without_local_crop() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();
        let (scene, hud) =
            project_scene_models_with_view_window(&session, "fr", Some((32.0, 32.0)), (4, 4));
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        let grid_rows = frame.lines().rev().take(4).collect::<Vec<_>>();

        assert!(frame.contains("WINDOW: origin=(2, 2) size=4x4"));
        assert_eq!(grid_rows.len(), 4);
        assert!(grid_rows.iter().all(|row| row.len() == 4));
    }

    #[test]
    fn ascii_presenter_uses_alias_semantic_mapping_for_focus_and_sprites() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "unit:focus".to_string(),
                    layer: 40,
                    x: 56.0,
                    y: 56.0,
                },
                RenderObject {
                    id: "marker:runtime-health:1:2".to_string(),
                    layer: 35,
                    x: 48.0,
                    y: 40.0,
                },
                RenderObject {
                    id: "hint:marker".to_string(),
                    layer: 30,
                    x: 40.0,
                    y: 40.0,
                },
                RenderObject {
                    id: "build-plan:1".to_string(),
                    layer: 20,
                    x: 32.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "building:3:4".to_string(),
                    layer: 10,
                    x: 48.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "tile:3".to_string(),
                    layer: 0,
                    x: 40.0,
                    y: 32.0,
                },
            ],
        };
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::with_max_view_tiles(4, 4);

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("WINDOW: origin=(4, 4) size=4x4"));
        assert!(frame.contains("@"));
        assert!(frame.contains("R"));
        assert!(frame.contains("M"));
        assert!(frame.contains("P"));
        assert!(frame.contains("#"));
    }

    #[test]
    fn ascii_presenter_keeps_crop_stable_around_half_tile_player_motion() {
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::with_max_view_tiles(4, 4);
        let base_scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "terrain:stable".to_string(),
                    layer: 0,
                    x: 8.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "unit:focus".to_string(),
                    layer: 40,
                    x: 27.9,
                    y: 32.0,
                },
            ],
        };
        let mut moved_scene = base_scene.clone();
        moved_scene
            .objects
            .iter_mut()
            .find(|object| object.id == "unit:focus")
            .unwrap()
            .x = 28.1;

        presenter.present(&base_scene, &hud);
        let first = presenter.last_frame().to_string();
        presenter.present(&moved_scene, &hud);
        let second = presenter.last_frame().to_string();

        assert_eq!(first, second);
    }

    #[test]
    fn sprite_for_id_supports_alias_prefixes() {
        assert_eq!(super::sprite_for_id("unit:1"), '@');
        assert_eq!(super::sprite_for_id("marker:line:7"), 'M');
        assert_eq!(super::sprite_for_id("marker:line:7:line-end"), 'M');
        assert_eq!(super::sprite_for_id("marker:runtime-health:1:2"), 'R');
        assert_eq!(
            super::sprite_for_id("marker:runtime-config-rollback:1:2:string"),
            'R'
        );
        assert_eq!(super::sprite_for_id("hint:1"), 'M');
        assert_eq!(super::sprite_for_id("build-plan:1"), 'P');
        assert_eq!(super::sprite_for_id("building:1:2"), '#');
        assert_eq!(super::sprite_for_id("tile:1"), '.');
    }

    #[test]
    fn ascii_presenter_surfaces_overlay_detail_semantics() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                crate::RenderObject {
                    id: "player:1".to_string(),
                    layer: 1,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:line:7".to_string(),
                    layer: 2,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:line:7:line-end".to_string(),
                    layer: 3,
                    x: 8.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:runtime-config:3:2:string".to_string(),
                    layer: 4,
                    x: 0.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "block:runtime-building:1:2:3".to_string(),
                    layer: 5,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "plan:runtime-place:0:4:5".to_string(),
                    layer: 6,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "terrain:runtime-deconstruct:9:4".to_string(),
                    layer: 7,
                    x: 8.0,
                    y: 8.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "OVERLAY-KINDS: players=1 markers=2 plans=0 blocks=0 runtime=4 terrain=0 unknown=0"
        ));
        assert!(frame.contains(
            "detail=marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1"
        ));
    }

    #[test]
    fn ascii_presenter_surfaces_minimap_detail_semantics() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                crate::RenderObject {
                    id: "player:1".to_string(),
                    layer: 1,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:line:7".to_string(),
                    layer: 1,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:line:7:line-end".to_string(),
                    layer: 1,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "marker:runtime-config:3:2:string".to_string(),
                    layer: 1,
                    x: 8.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "block:runtime-building:1:2:3".to_string(),
                    layer: 1,
                    x: 0.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "plan:runtime-place:0:4:5".to_string(),
                    layer: 1,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "terrain:runtime-deconstruct:9:4".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 8.0,
                },
            ],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 2,
                map_height: 2,
                overlay_visible: true,
                fog_enabled: false,
                visible_tile_count: 4,
                hidden_tile_count: 0,
            }),
            ..HudModel::default()
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "MINIMAP-KINDS: tracked=7 player=1 marker=2 plan=0 block=0 runtime=4 terrain=0 unknown=0 detail=marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1"
        ));
        assert!(frame.contains("MINIMAP-DETAIL: 1/6 marker-line=1"));
    }

    #[test]
    fn ascii_presenter_emits_structured_summary_and_runtime_ui_lines() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                crate::RenderObject {
                    id: "player:1".to_string(),
                    layer: 1,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:7".to_string(),
                    layer: 2,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "plan:1:2:3".to_string(),
                    layer: 3,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "block:9:4".to_string(),
                    layer: 4,
                    x: 0.0,
                    y: 0.0,
                },
            ],
        };
        let hud = HudModel {
            title: "demo".to_string(),
            wave_text: Some("Wave 3".to_string()),
            status_text: "base".to_string(),
            overlay_summary_text: Some("Plans 2".to_string()),
            fps: None,
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
                chat: crate::RuntimeChatObservability {
                    server_message_count: 7,
                    last_server_message: Some("server text".to_string()),
                    chat_message_count: 8,
                    last_chat_message: Some("[cyan]hello".to_string()),
                    last_chat_unformatted: Some("hello".to_string()),
                    last_chat_sender_entity_id: Some(404),
                },
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
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
                },
                command_mode: crate::RuntimeCommandModeObservability {
                    active: true,
                    selected_units: vec![11, 22, 33, 44],
                    command_buildings: vec![
                        ((5 & 0xffff) << 16) | (6 & 0xffff),
                        ((-7 & 0xffff) << 16) | (8 & 0xffff),
                    ],
                    command_rect: Some(crate::RuntimeCommandRectObservability {
                        x0: -3,
                        y0: 4,
                        x1: 12,
                        y1: 18,
                    }),
                    control_groups: vec![
                        crate::RuntimeCommandControlGroupObservability {
                            index: 2,
                            unit_ids: vec![11, 22, 33],
                        },
                        crate::RuntimeCommandControlGroupObservability {
                            index: 4,
                            unit_ids: vec![99],
                        },
                    ],
                    last_target: Some(crate::RuntimeCommandTargetObservability {
                        build_target: Some(((9 & 0xffff) << 16) | (10 & 0xffff)),
                        unit_target: Some(crate::RuntimeCommandUnitRefObservability {
                            kind: 2,
                            value: 808,
                        }),
                        position_target: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 48.0f32.to_bits(),
                            y_bits: 96.0f32.to_bits(),
                        }),
                        rect_target: Some(crate::RuntimeCommandRectObservability {
                            x0: 1,
                            y0: 2,
                            x1: 3,
                            y1: 4,
                        }),
                    }),
                    last_command_selection: Some(crate::RuntimeCommandSelectionObservability {
                        command_id: Some(5),
                    }),
                    last_stance_selection: Some(crate::RuntimeCommandStanceObservability {
                        stance_id: Some(7),
                        enabled: false,
                    }),
                },
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
                world_labels: RuntimeWorldLabelObservability {
                    label_count: 19,
                    reliable_label_count: 20,
                    remove_label_count: 21,
                    active_count: 2,
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
                live: crate::RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability {
                        entity_count: 1,
                        hidden_count: 0,
                        player_count: 1,
                        unit_count: 0,
                        last_entity_id: Some(404),
                        last_player_entity_id: Some(404),
                        last_unit_entity_id: None,
                        local_entity_id: Some(404),
                        local_unit_kind: Some(2),
                        local_unit_value: Some(999),
                        local_hidden: Some(false),
                        local_last_seen_entity_snapshot_count: Some(3),
                        local_position: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 20.0f32.to_bits(),
                            y_bits: 33.0f32.to_bits(),
                        }),
                    },
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
            build_ui: Some(crate::BuildUiObservability {
                selected_block_id: Some(257),
                selected_rotation: 2,
                building: true,
                queued_count: 1,
                inflight_count: 2,
                finished_count: 3,
                removed_count: 4,
                orphan_authoritative_count: 1,
                head: Some(crate::BuildQueueHeadObservability {
                    x: 100,
                    y: 99,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: crate::BuildQueueHeadStage::InFlight,
                }),
                rollback_strip: crate::BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 3,
                    rollback_count: 1,
                    last_build_tile: Some((23, 45)),
                    last_business_applied: true,
                    last_cleared_pending_local: true,
                    last_was_rollback: true,
                    last_pending_local_match: Some(false),
                    last_source: Some(
                        crate::BuildConfigAuthoritySourceObservability::ConstructFinish,
                    ),
                    last_configured_outcome: Some(crate::BuildConfigOutcomeObservability::Applied),
                    last_configured_block_name: Some("power-node".to_string()),
                },
                inspector_entries: vec![
                    crate::BuildConfigInspectorEntryObservability {
                        family: "message".to_string(),
                        tracked_count: 1,
                        sample: "18:40:len=5:text=hello".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "power-node".to_string(),
                        tracked_count: 1,
                        sample: "23:45:links=24:46|25:47".to_string(),
                    },
                ],
            }),
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("SUMMARY: player=operator team=2 selected=payload-rout~"));
        assert!(frame.contains("plans=3 markers=4 map=80x60"));
        assert!(frame.contains(
            "HUD-DETAIL: player=operator len=8 selected=payload-rout~ len=14 tiles=4800 vis-map=2 hidden-map=0"
        ));
        assert!(frame.contains(
            "MINIMAP: map=80x60 window=0:0->0:0 size=1x1 cover=1/4800(0%) focus=0:0 in-window=1 drift=0:0 edges=1/1/0/0"
        ));
        assert!(frame.contains(
            "MINIMAP-VIS: overlay=1 fog=1 known=144(3%) vis=120(83%) hid=24(16%) unseen=4656(97%)"
        ));
        assert!(frame.contains(
            "MINIMAP-KINDS: tracked=4 player=1 marker=1 plan=1 block=1 runtime=0 terrain=0 unknown=0"
        ));
        assert!(frame.contains(
            "MINIMAP-LEGEND: @=player M=marker P=plan #=block R=runtime .=terrain ?=unknown"
        ));
        assert!(frame.contains(
            "BUILD-CONFIG: sel=257 rot=2 mode=build pending=1/2 hist=3/4 orphan=1 head=flight@100:99:place:b301:r1 align=split families=2/2 tracked=2"
        ));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 1/2 message#1@18:40:len=5:text=hello"));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 2/2 power-node#1@23:45:links=24:46|25:47"));
        assert!(frame.contains(
            "BUILD-ROLLBACK: authoritative=3 rollback=1 last=23:45 src=constructFinish business=1 clear=1 last-rb=1 pending=mismatch outcome=applied block=power-node"
        ));
        assert!(frame.contains(
            "BUILD-INTERACTION: mode=place select=head-diverged queue=mixed pending=3 place-ready=1 cfg=2/2 top=message head=flight@100:99:place:b301:r1 auth=rollback pending=mismatch src=constructFinish tile=23:45 block=power-node orphan=1"
        ));
        assert!(frame.contains(
            "BUILD: sel=257 rot=2 building=1 queue=1/2/3/4/1 head=flight@100:99:place:b301:r1 cfg=2"
        ));
        assert!(frame
            .contains("BUILD-INSPECTOR: family=message tracked=1 sample=18:40:len=5:text=hello"));
        assert!(frame.contains(
            "BUILD-INSPECTOR: family=power-node tracked=1 sample=23:45:links=24:46|25:47"
        ));
        assert!(frame.contains(
            "OVERLAY-KINDS: players=1 markers=1 plans=1 blocks=1 runtime=0 terrain=0 unknown=0"
        ));
        assert!(frame.contains("RUNTIME-UI: hud=9/10/11@hud_text/hud_rel"));
        assert!(frame.contains("toast=14/15@toast/warn"));
        assert!(frame.contains("tin=53@404:Digits/Only_numbers"));
        assert!(frame.contains(
            "RUNTIME-NOTICE: hud=9/10/11@hud_text/hud_rel toast=14/15@toast/warn tin=53@404:Digits/Only_numbers/12345#16:n1:e1"
        ));
        assert!(frame.contains(
            "RUNTIME-NOTICE-DETAIL: active=1 hud-events=9/10/11 hud-len=8/7 toast-events=14/15 toast-len=5/4 text-input=53 id=404 title-len=6 msg-len=12 default-len=5 numeric=1 allow-empty=1"
        ));
        assert!(frame
            .contains("RUNTIME-MENU: menu=16 fmenu=17 hide=18 tin=53@404:Digits/12345#16:n1:e1"));
        assert!(frame.contains(
            "RUNTIME-MENU-DETAIL: active=1 outstanding-follow-up=0 text-input=53 id=404 title=Digits default-len=5 numeric=1 allow-empty=1"
        ));
        assert!(frame.contains(
            "RUNTIME-DIALOG: prompt=input act=1 menu=16/17/18 tin=53@404:Digits/Only_numbers/12345#16:n1:e1 notice=warn@warn total=48"
        ));
        assert!(frame.contains(
            "RUNTIME-DIALOG-DETAIL: prompt=input active=1 outstanding-follow-up=0 message-len=12 default-len=5 notice=warn notice-len=4"
        ));
        assert!(frame.contains(
            "RUNTIME-CHAT: srv=7 last-srv=server_text chat=8 last-chat=[cyan]hello raw=hello sender=404"
        ));
        assert!(frame.contains(
            "RUNTIME-CHAT-DETAIL: server-len=11 chat-len=11 raw-len=5 formatted-eq-raw=0 sender=404"
        ));
        assert!(frame.contains(
            "RUNTIME-STACK: front=input prompt=2@input>menu notice=warn@hud>reliable>info>warn chat=1 groups=3 total=7 tin=404 sender=404"
        ));
        assert!(frame.contains(
            "RUNTIME-STACK-DETAIL: menu-active=1 outstanding-follow-up=0 text-input=53 notice-depth=4 server-chat=7/8 sender=404"
        ));
        assert!(frame.contains(
            "RUNTIME-COMMAND: act=1 sel=4@11,22,33 bld=2@327686 rect=-3:4:12:18 groups=2#3@11,4#1@99 target=b589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4 cmd=5 stance=7/0"
        ));
        assert!(frame.contains("RUNTIME-ADMIN: trace=56@123456 fail=76 dbg=57/58@12 fail=231"));
        assert!(frame.contains(
            "RUNTIME-RULES: mut=354 fail=210 set=67/69/71 clear=73 complete=74 state=wv1:pvp0 obj=2 qual=1 parents=1 flags=2 oor=75 last=9"
        ));
        assert!(frame.contains(
            "RUNTIME-WORLD-LABEL: set=19 rel=20 remove=21 total=60 active=2 inactive=58 last=904 flags=3 font=1094713344@12.0 z=1082130432@4.0 pos=40.0:60.0 text=world label lines=1 len=11"
        ));
        assert!(frame.contains(
            "RUNTIME-WORLD-LABEL-DETAIL: set=19 rel=20 remove=21 active=2 inactive=58 last=904 flags=3 text-len=11 lines=1 font=1094713344@12.0 z=1082130432@4.0 pos=40.0:60.0"
        ));
        assert!(frame.contains(
            "RUNTIME-SESSION: kick=idInUse@7:IdInUse:wait_for_old~; loading=defer5 replay6 drop7 qdrop8 sfail9 scfail10 efail11 rdy12@1300 to2/1/1 ltready@20000 rs3/1/1/1 lrreload lwr@lw1:cl0:rd1:cc0:p4:d5:r6; reconnect=attempt#3 redirect redirect=1@127.0.0.1:6567 reason=connectRedir~#none hint=server_reque~"
        ));
        assert!(frame.contains("RUNTIME-KICK: idInUse@7:IdInUse:wait_for_old~"));
        assert!(frame.contains(
            "RUNTIME-LOADING: defer5 replay6 drop7 qdrop8 sfail9 scfail10 efail11 rdy12@1300 to2/1/1 ltready@20000 rs3/1/1/1 lrreload lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
        ));
        assert!(frame.contains(
            "RUNTIME-RECONNECT: attempt#3 redirect redirect=1@127.0.0.1:6567 reason=connectRedir~#none hint=server_reque~"
        ));
        assert!(frame.contains(
            "RUNTIME-LIVE-ENTITY: 1/0@404:u2/999:p20.0:33.0:h0:s3:tp1/0:last404/404/none"
        ));
        assert!(frame.contains(
            "RUNTIME-LIVE-EFFECT: 11/73@8:u19:kPoint2:cposition_tar~/unit_parent:pbiz@24.0:32.0"
        ));
        assert!(frame.contains("live=ent=1/0@404:u2/999:p20.0:33.0:h0:s3"));
        assert!(frame.contains("fx=11/73@8:u19:kPoint2:cposition_tar~/unit_parent:pbiz@24.0:32.0"));
    }

    #[test]
    fn ascii_presenter_surfaces_minimap_and_build_config_overflow_context() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                crate::RenderObject {
                    id: "player:1".to_string(),
                    layer: 1,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "terrain:1".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "unknown".to_string(),
                    layer: 2,
                    x: 8.0,
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
            }),
            build_ui: Some(crate::BuildUiObservability {
                selected_block_id: Some(301),
                selected_rotation: 1,
                building: true,
                queued_count: 2,
                inflight_count: 1,
                finished_count: 4,
                removed_count: 5,
                orphan_authoritative_count: 6,
                head: Some(crate::BuildQueueHeadObservability {
                    x: 10,
                    y: 12,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: crate::BuildQueueHeadStage::Queued,
                }),
                rollback_strip: crate::BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 4,
                    rollback_count: 2,
                    last_build_tile: Some((10, 12)),
                    last_business_applied: true,
                    last_cleared_pending_local: false,
                    last_was_rollback: false,
                    last_pending_local_match: Some(true),
                    last_source: Some(crate::BuildConfigAuthoritySourceObservability::TileConfig),
                    last_configured_outcome: Some(
                        crate::BuildConfigOutcomeObservability::RejectedMissingBuilding,
                    ),
                    last_configured_block_name: Some("alpha".to_string()),
                },
                inspector_entries: vec![
                    crate::BuildConfigInspectorEntryObservability {
                        family: "alpha".to_string(),
                        tracked_count: 1,
                        sample: "one".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "gamma".to_string(),
                        tracked_count: 4,
                        sample: "four".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "beta".to_string(),
                        tracked_count: 2,
                        sample: "two".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "delta".to_string(),
                        tracked_count: 1,
                        sample: "three".to_string(),
                    },
                ],
            }),
            ..HudModel::default()
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "MINIMAP: map=80x60 window=0:0->1:1 size=2x2 cover=4/4800(0%) focus=1:1 in-window=1 drift=1:1 edges=1/1/0/0"
        ));
        assert!(frame.contains(
            "MINIMAP-VIS: overlay=0 fog=0 known=0(0%) vis=0(0%) hid=0(0%) unseen=4800(100%)"
        ));
        assert!(frame.contains(
            "MINIMAP-KINDS: tracked=3 player=1 marker=0 plan=0 block=0 runtime=0 terrain=1 unknown=1"
        ));
        assert!(frame.contains(
            "MINIMAP-LEGEND: @=player M=marker P=plan #=block R=runtime .=terrain ?=unknown"
        ));
        assert!(frame.contains(
            "BUILD-CONFIG: sel=301 rot=1 mode=build pending=2/1 hist=4/5 orphan=6 head=queued@10:12:place:b301:r1 align=match families=3/4 tracked=8"
        ));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 1/4 gamma#4@four"));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 2/4 beta#2@two"));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 3/4 alpha#1@one"));
        assert!(frame.contains("BUILD-CONFIG-MORE: +1 hidden families beyond cap"));
        assert!(frame.contains(
            "BUILD-ROLLBACK: authoritative=4 rollback=2 last=10:12 src=tileConfig business=1 clear=0 last-rb=0 pending=match outcome=rejected-missing-building block=alpha"
        ));
        assert!(frame.contains(
            "BUILD-INTERACTION: mode=place select=head-aligned queue=mixed pending=3 place-ready=1 cfg=4/8 top=gamma head=queued@10:12:place:b301:r1 auth=rejected-missing-building pending=match src=tileConfig tile=10:12 block=alpha orphan=6"
        ));
        assert!(frame.contains(
            "BUILD-FLOW: next=resolve mode=place select=head-aligned queue=mixed place-ready=1 focus=inside vis=unseen cover=offscreen scope=multi auth=rejected-missing-building runtime-share=0%"
        ));
    }

    #[test]
    fn ascii_presenter_omits_runtime_session_row_for_empty_default_state() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: Vec::new(),
        };
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability::default()),
            ..HudModel::default()
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        assert!(!presenter.last_frame().contains("RUNTIME-SESSION:"));
        assert!(!presenter
            .last_frame()
            .contains("RUNTIME-NOTICE-DETAIL:"));
        assert!(!presenter
            .last_frame()
            .contains("RUNTIME-WORLD-LABEL-DETAIL:"));
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        let cleaned = text
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect::<String>();
        assert_eq!(cleaned.len() % 2, 0);

        cleaned
            .as_bytes()
            .chunks(2)
            .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
            .collect()
    }
}
