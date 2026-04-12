use crate::build_user_flow::build_build_user_flow_panel;
use crate::minimap_user_flow::build_minimap_user_flow_panel;
use crate::panel_model::{
    build_build_config_panel, build_build_interaction_panel, build_build_minimap_assist_panel,
    build_hud_status_panel, build_hud_visibility_panel, build_minimap_panel,
    build_runtime_admin_panel, build_runtime_bootstrap_panel, build_runtime_chat_panel,
    build_runtime_choice_panel, build_runtime_command_mode_panel, build_runtime_core_binding_panel,
    build_runtime_dialog_panel, build_runtime_dialog_stack_panel, build_runtime_kick_panel,
    build_runtime_live_effect_panel, build_runtime_live_entity_panel, build_runtime_loading_panel,
    build_runtime_marker_panel, build_runtime_menu_panel, build_runtime_notice_state_panel,
    build_runtime_prompt_panel, build_runtime_reconnect_panel, build_runtime_rules_panel,
    build_runtime_session_panel, build_runtime_ui_notice_panel, build_runtime_ui_stack_panel,
    build_runtime_world_label_panel, MinimapPanelModel, PresenterViewWindow,
    RuntimeDialogNoticeKind, RuntimeDialogPromptKind, RuntimeUiNoticePanelModel,
};
use crate::presenter_view::{
    crop_window_to_focus, normalize_zoom, projected_window, visible_window_tile,
    zoomed_view_tile_span,
};
use crate::render_model::{
    RenderIconPrimitiveFamily, RenderObjectSemanticFamily, RenderObjectSemanticKind,
    RenderPrimitive, RenderPrimitivePayload, RenderPrimitivePayloadValue,
};
use crate::{HudModel, RenderModel, ScenePresenter};
use std::collections::{BTreeMap, BTreeSet};

const TILE_SIZE: f32 = 8.0;
const ASCII_ICON_RUNTIME_EFFECT: char = 'E';
const ASCII_ICON_RUNTIME_EFFECT_MARKER: char = 'F';
const ASCII_ICON_BUILD_CONFIG: char = 'C';
const ASCII_ICON_RUNTIME_HEALTH: char = 'H';
const ASCII_ICON_RUNTIME_COMMAND: char = 'T';
const ASCII_ICON_RUNTIME_PLACE: char = 'P';
const ASCII_ICON_RUNTIME_UNIT_ASSEMBLER: char = 'A';
const ASCII_ICON_RUNTIME_BREAK: char = 'X';
const ASCII_ICON_RUNTIME_BULLET: char = 'B';
const ASCII_ICON_RUNTIME_LOGIC_EXPLOSION: char = 'L';
const ASCII_ICON_RUNTIME_SOUND_AT: char = 'S';
const ASCII_ICON_RUNTIME_TILE_ACTION: char = 'W';

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
        let primitives = scene.primitives();
        let text_primitive_ids = primitives
            .iter()
            .filter_map(|primitive| match primitive {
                RenderPrimitive::Text { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let icon_primitive_ids = primitives
            .iter()
            .filter_map(|primitive| match primitive {
                RenderPrimitive::Icon { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let rect_line_ids = primitives
            .iter()
            .filter_map(|primitive| match primitive {
                RenderPrimitive::Rect { line_ids, .. } => Some(line_ids.iter().map(String::as_str)),
                _ => None,
            })
            .flatten()
            .collect::<BTreeSet<_>>();
        let line_end_objects = scene
            .objects
            .iter()
            .filter_map(ascii_line_end_object_pair)
            .collect::<BTreeMap<_, _>>();
        let mut commands = scene
            .objects
            .iter()
            .filter(|object| {
                !text_primitive_ids.contains(object.id.as_str())
                    && !icon_primitive_ids.contains(object.id.as_str())
                    && !rect_line_ids.contains(object.id.as_str())
            })
            .filter_map(|object| ascii_render_command(object, &line_end_objects, window))
            .collect::<Vec<_>>();
        commands.extend(
            primitives
                .iter()
                .filter_map(|primitive| ascii_primitive_render_command(primitive, window)),
        );
        commands.sort_by_key(AsciiRenderCommand::layer);
        for command in commands {
            match command {
                AsciiRenderCommand::Point {
                    object,
                    local_x,
                    local_y,
                } => {
                    grid[local_y][local_x] = sprite_for_id(&object.id);
                }
                AsciiRenderCommand::Line {
                    start_tile,
                    end_tile,
                    sprite,
                    ..
                } => draw_ascii_line_segment(&mut grid, window, start_tile, end_tile, sprite),
                AsciiRenderCommand::Rect {
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                    sprite,
                    ..
                } => draw_ascii_rect_outline(
                    &mut grid,
                    window,
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                    sprite,
                ),
                AsciiRenderCommand::Icon {
                    local_x,
                    local_y,
                    sprite,
                    ..
                } => {
                    grid[local_y][local_x] = sprite;
                }
                AsciiRenderCommand::Text {
                    local_x,
                    local_y,
                    text,
                    ..
                } => draw_ascii_text(&mut grid, window, local_x, local_y, text),
            }
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
        if let Some(render_rect_text) = compose_render_rect_status_text(scene, window) {
            out.push_str(&format!("RENDER-RECT: {render_rect_text}\n"));
        }
        if let Some(render_rect_detail_text) = compose_render_rect_detail_text(scene, window) {
            out.push_str(&format!("RENDER-RECT-DETAIL: {render_rect_detail_text}\n"));
        }
        if let Some(render_icon_text) = compose_render_icon_status_text(scene, window) {
            out.push_str(&format!("RENDER-ICON: {render_icon_text}\n"));
        }
        if let Some(render_icon_detail_text) = compose_render_icon_detail_text(scene, window) {
            out.push_str(&format!("RENDER-ICON-DETAIL: {render_icon_detail_text}\n"));
        }
        if let Some(minimap_text) = compose_minimap_panel_text(scene, hud, window) {
            out.push_str(&format!("MINIMAP: {minimap_text}\n"));
        }
        if let Some(minimap_visibility_text) = compose_minimap_visibility_line(scene, hud, window) {
            out.push_str(&format!("MINIMAP-VIS: {minimap_visibility_text}\n"));
        }
        if let Some(visibility_minimap_text) = compose_visibility_minimap_text(scene, hud, window) {
            out.push_str(&format!("VIS-MINIMAP: {visibility_minimap_text}\n"));
        }
        if let Some(minimap_visibility_detail_text) =
            compose_minimap_visibility_detail_text(scene, hud, window)
        {
            out.push_str(&format!(
                "MINIMAP-VIS-DETAIL: {minimap_visibility_detail_text}\n"
            ));
        }
        if let Some(minimap_flow_text) = compose_minimap_flow_line(scene, hud, window) {
            out.push_str(&format!("MINIMAP-FLOW: {minimap_flow_text}\n"));
        }
        if let Some(minimap_kinds_text) = compose_minimap_kind_line(scene, hud) {
            out.push_str(&format!("MINIMAP-KINDS: {minimap_kinds_text}\n"));
        }
        for minimap_detail_text in compose_minimap_detail_lines(scene, hud) {
            out.push_str(&format!("MINIMAP-DETAIL: {minimap_detail_text}\n"));
        }
        if let Some(render_pipeline_text) = compose_render_pipeline_text(scene, window) {
            out.push_str(&format!("RENDER-PIPELINE: {render_pipeline_text}\n"));
        }
        for render_layer_text in compose_render_layer_lines(scene, window) {
            out.push_str(&format!("RENDER-LAYER: {render_layer_text}\n"));
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
        if let Some(build_minimap_aux_text) = compose_build_minimap_aux_text(scene, hud, window) {
            out.push_str(&format!("BUILD-MINIMAP-AUX: {build_minimap_aux_text}\n"));
        }
        if let Some(build_flow_text) = compose_build_flow_text(scene, hud, window) {
            out.push_str(&format!("BUILD-FLOW: {build_flow_text}\n"));
        }
        if let Some(build_flow_summary_text) = compose_build_flow_summary_text(scene, hud, window) {
            out.push_str(&format!("BUILD-FLOW-SUMMARY: {build_flow_summary_text}\n"));
        }
        if let Some(build_route_text) = compose_build_route_text(scene, hud, window) {
            out.push_str(&format!("BUILD-ROUTE: {build_route_text}\n"));
        }
        if let Some(build_flow_detail_text) = compose_build_flow_detail_text(scene, hud, window) {
            out.push_str(&format!("BUILD-FLOW-DETAIL: {build_flow_detail_text}\n"));
        }
        if let Some(build_text) = compose_build_ui_text(hud) {
            out.push_str(&format!("BUILD: {build_text}\n"));
        }
        if let Some(build_queue_text) = compose_build_ui_queue_text(hud) {
            out.push_str(&format!("BUILD-QUEUE: {build_queue_text}\n"));
        }
        for inspector_line in compose_build_ui_inspector_lines(hud) {
            out.push_str(&format!("BUILD-INSPECTOR: {inspector_line}\n"));
        }
        if let Some(overlay_semantics_text) = compose_overlay_semantics_text(scene) {
            out.push_str(&format!("OVERLAY-KINDS: {overlay_semantics_text}\n"));
        }
        if let Some(overlay_detail_text) = compose_overlay_detail_text(scene) {
            out.push_str(&format!("OVERLAY-DETAIL: {overlay_detail_text}\n"));
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
        if let Some(runtime_choice_text) = compose_runtime_choice_panel_text(hud) {
            out.push_str(&format!("RUNTIME-CHOICE: {runtime_choice_text}\n"));
        }
        if let Some(runtime_choice_detail_text) = compose_runtime_choice_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-CHOICE-DETAIL: {runtime_choice_detail_text}\n"
            ));
        }
        if let Some(runtime_prompt_text) = compose_runtime_prompt_panel_text(hud) {
            out.push_str(&format!("RUNTIME-PROMPT: {runtime_prompt_text}\n"));
        }
        if let Some(runtime_prompt_detail_text) = compose_runtime_prompt_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-PROMPT-DETAIL: {runtime_prompt_detail_text}\n"
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
        if let Some(runtime_stack_depth_text) = compose_runtime_stack_depth_text(hud) {
            out.push_str(&format!(
                "RUNTIME-STACK-DEPTH: {runtime_stack_depth_text}\n"
            ));
        }
        if let Some(runtime_stack_detail_text) = compose_runtime_stack_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-STACK-DETAIL: {runtime_stack_detail_text}\n"
            ));
        }
        if let Some(runtime_dialog_stack_text) = compose_runtime_dialog_stack_text(hud) {
            out.push_str(&format!(
                "RUNTIME-DIALOG-STACK: {runtime_dialog_stack_text}\n"
            ));
        }
        if let Some(runtime_command_text) = compose_runtime_command_mode_panel_text(hud) {
            out.push_str(&format!("RUNTIME-COMMAND: {runtime_command_text}\n"));
        }
        if let Some(runtime_command_detail_text) = compose_runtime_command_mode_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-COMMAND-DETAIL: {runtime_command_detail_text}\n"
            ));
        }
        if let Some(runtime_admin_text) = compose_runtime_admin_panel_text(hud) {
            out.push_str(&format!("RUNTIME-ADMIN: {runtime_admin_text}\n"));
        }
        if let Some(runtime_admin_detail_text) = compose_runtime_admin_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-ADMIN-DETAIL: {runtime_admin_detail_text}\n"
            ));
        }
        if let Some(runtime_rules_text) = compose_runtime_rules_panel_text(hud) {
            out.push_str(&format!("RUNTIME-RULES: {runtime_rules_text}\n"));
        }
        if let Some(runtime_rules_detail_text) = compose_runtime_rules_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-RULES-DETAIL: {runtime_rules_detail_text}\n"
            ));
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
        if let Some(runtime_marker_text) = compose_runtime_marker_panel_text(hud) {
            out.push_str(&format!("RUNTIME-MARKER: {runtime_marker_text}\n"));
        }
        if let Some(runtime_marker_detail_text) = compose_runtime_marker_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-MARKER-DETAIL: {runtime_marker_detail_text}\n"
            ));
        }
        if let Some(runtime_session_text) = compose_runtime_session_row_text(hud) {
            out.push_str(&format!("RUNTIME-SESSION: {runtime_session_text}\n"));
        }
        if let Some(runtime_session_detail_text) = compose_runtime_session_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-SESSION-DETAIL: {runtime_session_detail_text}\n"
            ));
        }
        if let Some(runtime_kick_text) = compose_runtime_kick_row_text(hud) {
            out.push_str(&format!("RUNTIME-KICK: {runtime_kick_text}\n"));
        }
        if let Some(runtime_kick_detail_text) = compose_runtime_kick_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-KICK-DETAIL: {runtime_kick_detail_text}\n"
            ));
        }
        if let Some(runtime_loading_text) = compose_runtime_loading_row_text(hud) {
            out.push_str(&format!("RUNTIME-LOADING: {runtime_loading_text}\n"));
        }
        if let Some(runtime_loading_detail_text) = compose_runtime_loading_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-LOADING-DETAIL: {runtime_loading_detail_text}\n"
            ));
        }
        if let Some(runtime_world_reload_detail_text) =
            compose_runtime_world_reload_detail_text(hud)
        {
            out.push_str(&format!(
                "RUNTIME-WORLD-RELOAD-DETAIL: {runtime_world_reload_detail_text}\n"
            ));
        }
        if let Some(runtime_core_binding_text) = compose_runtime_core_binding_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-CORE-BINDING: {runtime_core_binding_text}\n"
            ));
        }
        if let Some(runtime_core_binding_detail_text) =
            compose_runtime_core_binding_detail_text(hud)
        {
            out.push_str(&format!(
                "RUNTIME-CORE-BINDING-DETAIL: {runtime_core_binding_detail_text}\n"
            ));
        }
        if let Some(runtime_reconnect_text) = compose_runtime_reconnect_row_text(hud) {
            out.push_str(&format!("RUNTIME-RECONNECT: {runtime_reconnect_text}\n"));
        }
        if let Some(runtime_reconnect_detail_text) = compose_runtime_reconnect_detail_text(hud) {
            out.push_str(&format!(
                "RUNTIME-RECONNECT-DETAIL: {runtime_reconnect_detail_text}\n"
            ));
        }
        if let Some(runtime_live_entity_text) = compose_runtime_live_entity_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-LIVE-ENTITY: {runtime_live_entity_text}\n"
            ));
        }
        if let Some(runtime_live_entity_detail_text) =
            compose_runtime_live_entity_detail_row_text(hud)
        {
            out.push_str(&format!(
                "RUNTIME-LIVE-ENTITY-DETAIL: {runtime_live_entity_detail_text}\n"
            ));
        }
        if let Some(runtime_live_effect_text) = compose_runtime_live_effect_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-LIVE-EFFECT: {runtime_live_effect_text}\n"
            ));
        }
        if let Some(runtime_live_effect_detail_text) =
            compose_runtime_live_effect_detail_row_text(hud)
        {
            out.push_str(&format!(
                "RUNTIME-LIVE-EFFECT-DETAIL: {runtime_live_effect_detail_text}\n"
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

#[derive(Debug, Clone, Copy)]
enum AsciiRenderCommand<'a> {
    Point {
        object: &'a crate::RenderObject,
        local_x: usize,
        local_y: usize,
    },
    Line {
        layer: i32,
        start_tile: (i32, i32),
        end_tile: (i32, i32),
        sprite: char,
    },
    Rect {
        layer: i32,
        left_tile: i32,
        top_tile: i32,
        right_tile: i32,
        bottom_tile: i32,
        sprite: char,
    },
    Icon {
        layer: i32,
        local_x: usize,
        local_y: usize,
        sprite: char,
    },
    Text {
        layer: i32,
        local_x: usize,
        local_y: usize,
        text: &'a str,
    },
}

impl AsciiRenderCommand<'_> {
    fn layer(&self) -> i32 {
        match self {
            Self::Point { object, .. } => object.layer,
            Self::Line { layer, .. } | Self::Rect { layer, .. } | Self::Icon { layer, .. } => {
                *layer
            }
            Self::Text { layer, .. } => *layer,
        }
    }
}

fn ascii_render_command<'a>(
    object: &'a crate::RenderObject,
    line_end_objects: &BTreeMap<String, &'a crate::RenderObject>,
    window: PresenterViewWindow,
) -> Option<AsciiRenderCommand<'a>> {
    match RenderObjectSemanticKind::from_id(&object.id) {
        RenderObjectSemanticKind::MarkerLineEnd => None,
        RenderObjectSemanticKind::MarkerLine => {
            if let Some(line_end) = line_end_objects.get(&object.id) {
                let Some(start_tile) = ascii_world_object_tile(object) else {
                    return None;
                };
                let Some(end_tile) = ascii_world_object_tile(line_end) else {
                    return None;
                };
                return Some(AsciiRenderCommand::Line {
                    layer: object.layer,
                    start_tile,
                    end_tile,
                    sprite: sprite_for_id(&object.id),
                });
            }
            visible_window_tile(
                object,
                TILE_SIZE,
                window.origin_x,
                window.origin_y,
                window.width,
                window.height,
            )
            .map(|(object, local_x, local_y)| AsciiRenderCommand::Point {
                object,
                local_x,
                local_y,
            })
        }
        _ => visible_window_tile(
            object,
            TILE_SIZE,
            window.origin_x,
            window.origin_y,
            window.width,
            window.height,
        )
        .map(|(object, local_x, local_y)| AsciiRenderCommand::Point {
            object,
            local_x,
            local_y,
        }),
    }
}

fn ascii_line_end_object_pair(
    object: &crate::RenderObject,
) -> Option<(String, &crate::RenderObject)> {
    if RenderObjectSemanticKind::from_id(&object.id) != RenderObjectSemanticKind::MarkerLineEnd {
        return None;
    }
    object
        .id
        .strip_suffix(":line-end")
        .map(|base_id| (base_id.to_string(), object))
}

fn ascii_primitive_render_command<'a>(
    primitive: &'a RenderPrimitive,
    window: PresenterViewWindow,
) -> Option<AsciiRenderCommand<'a>> {
    match primitive {
        RenderPrimitive::Text {
            kind: _kind,
            layer,
            x,
            y,
            text,
            ..
        } => {
            let Some((tile_x, tile_y)) = finite_world_tile(*x, *y) else {
                return None;
            };
            let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
            if tile_x < window.origin_x
                || tile_y < window.origin_y
                || tile_x >= window.origin_x.saturating_add(window.width)
                || tile_y >= window.origin_y.saturating_add(window.height)
            {
                return None;
            }

            Some(AsciiRenderCommand::Text {
                layer: *layer,
                local_x: tile_x - window.origin_x,
                local_y: tile_y - window.origin_y,
                text: text.as_str(),
            })
        }
        RenderPrimitive::Rect {
            layer,
            left,
            top,
            right,
            bottom,
            ..
        } => {
            let Some((left_tile, top_tile, right_tile, bottom_tile)) =
                finite_world_rect_tiles(*left, *top, *right, *bottom)
            else {
                return None;
            };
            if right_tile < window.origin_x as i32
                || bottom_tile < window.origin_y as i32
                || left_tile >= window.origin_x.saturating_add(window.width) as i32
                || top_tile >= window.origin_y.saturating_add(window.height) as i32
            {
                return None;
            }
            Some(AsciiRenderCommand::Rect {
                layer: *layer,
                left_tile,
                top_tile,
                right_tile,
                bottom_tile,
                sprite: 'R',
            })
        }
        RenderPrimitive::Icon {
            family,
            layer,
            x,
            y,
            ..
        } => {
            let Some((tile_x, tile_y)) = finite_world_tile(*x, *y) else {
                return None;
            };
            let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
            if tile_x < window.origin_x
                || tile_y < window.origin_y
                || tile_x >= window.origin_x.saturating_add(window.width)
                || tile_y >= window.origin_y.saturating_add(window.height)
            {
                return None;
            }
            Some(AsciiRenderCommand::Icon {
                layer: *layer,
                local_x: tile_x - window.origin_x,
                local_y: tile_y - window.origin_y,
                sprite: ascii_sprite_for_icon(*family),
            })
        }
        _ => None,
    }
}

fn ascii_world_object_tile(object: &crate::RenderObject) -> Option<(i32, i32)> {
    finite_world_tile(object.x, object.y)
}

fn finite_world_tile(x: f32, y: f32) -> Option<(i32, i32)> {
    (x.is_finite() && y.is_finite()).then_some((
        crate::presenter_view::world_to_tile_index_floor(x, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(y, TILE_SIZE),
    ))
}

fn finite_world_rect_tiles(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Option<(i32, i32, i32, i32)> {
    (left.is_finite() && top.is_finite() && right.is_finite() && bottom.is_finite()).then_some((
        crate::presenter_view::world_to_tile_index_floor(left, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(top, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(right, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(bottom, TILE_SIZE),
    ))
}

fn draw_ascii_line_segment(
    grid: &mut [Vec<char>],
    window: PresenterViewWindow,
    start_tile: (i32, i32),
    end_tile: (i32, i32),
    sprite: char,
) {
    let (mut x0, mut y0) = start_tile;
    let (x1, y1) = end_tile;
    let dx = (x1 - x0).abs();
    let sx = if x0 <= x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 <= y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        draw_ascii_tile_if_visible(grid, window, x0, y0, sprite);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled_error = err.saturating_mul(2);
        if doubled_error >= dy {
            err += dy;
            x0 += sx;
        }
        if doubled_error <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_ascii_rect_outline(
    grid: &mut [Vec<char>],
    window: PresenterViewWindow,
    left_tile: i32,
    top_tile: i32,
    right_tile: i32,
    bottom_tile: i32,
    sprite: char,
) {
    draw_ascii_line_segment(
        grid,
        window,
        (left_tile, top_tile),
        (right_tile, top_tile),
        sprite,
    );
    draw_ascii_line_segment(
        grid,
        window,
        (right_tile, top_tile),
        (right_tile, bottom_tile),
        sprite,
    );
    draw_ascii_line_segment(
        grid,
        window,
        (right_tile, bottom_tile),
        (left_tile, bottom_tile),
        sprite,
    );
    draw_ascii_line_segment(
        grid,
        window,
        (left_tile, bottom_tile),
        (left_tile, top_tile),
        sprite,
    );
}

fn compose_render_rect_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut rect_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Rect {
                family,
                layer,
                left,
                top,
                right,
                bottom,
                ..
            } => finite_world_rect_tiles(left, top, right, bottom)
                .map(|tiles| (family, layer, left, top, right, bottom, tiles)),
            _ => None,
        })
        .filter_map(
            |(
                family,
                layer,
                left,
                top,
                right,
                bottom,
                (left_tile, top_tile, right_tile, bottom_tile),
            )| {
                if !render_rect_detail_is_visible(
                    window,
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                ) {
                    None
                } else {
                    Some((
                        family,
                        layer,
                        left,
                        top,
                        right,
                        bottom,
                        left_tile,
                        top_tile,
                        right_tile,
                        bottom_tile,
                    ))
                }
            },
        )
        .collect::<Vec<_>>();

    if rect_primitives.is_empty() {
        return None;
    }

    rect_primitives.sort_by_key(|(_, layer, _, _, _, _, _, _, _, _)| *layer);
    let mut parts = vec![format!("count={}", rect_primitives.len())];
    for (family, layer, left, top, right, bottom, _, _, _, _) in rect_primitives.into_iter().take(2)
    {
        parts.push(format!(
            "{family}@{layer}:{}:{}:{}:{}",
            left as i32, top as i32, right as i32, bottom as i32
        ));
    }
    Some(parts.join(" "))
}

fn compose_render_rect_detail_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut rect_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Rect {
                family,
                layer,
                left,
                top,
                right,
                bottom,
                line_ids,
                ..
            } => {
                let (left_tile, top_tile, right_tile, bottom_tile) =
                    finite_world_rect_tiles(*left, *top, *right, *bottom)?;
                if !render_rect_detail_is_visible(
                    window,
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                ) {
                    return None;
                }
                let payload = primitive.payload();
                let (block_name, tile_x, tile_y) =
                    render_rect_detail_payload_fields(payload.as_ref());
                Some((
                    *layer,
                    family.clone(),
                    *left as i32,
                    *top as i32,
                    *right as i32,
                    *bottom as i32,
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                    line_ids.len(),
                    block_name,
                    tile_x,
                    tile_y,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if rect_primitives.is_empty() {
        return None;
    }

    rect_primitives.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
    });

    let mut parts = vec![format!("count={}", rect_primitives.len())];
    for (
        layer,
        family,
        left,
        top,
        right,
        bottom,
        left_tile,
        top_tile,
        right_tile,
        bottom_tile,
        line_count,
        block_name,
        tile_x,
        tile_y,
    ) in rect_primitives
    {
        parts.push(format!(
            "{family}@{layer}:{left}:{top}:{right}:{bottom} payload[{}]",
            render_rect_detail_fields_text(
                left_tile,
                top_tile,
                right_tile,
                bottom_tile,
                line_count,
                block_name.as_deref(),
                tile_x,
                tile_y
            )
        ));
    }
    Some(parts.join(" | "))
}

fn compose_render_icon_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut icon_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Icon {
                family,
                variant,
                layer,
                x,
                y,
                ..
            } => finite_world_tile(x, y)
                .map(|(tile_x, tile_y)| (family, variant, layer, tile_x, tile_y)),
            _ => None,
        })
        .filter_map(|(family, variant, layer, tile_x, tile_y)| {
            if tile_x >= 0
                && tile_y >= 0
                && (tile_x as usize) >= window.origin_x
                && (tile_y as usize) >= window.origin_y
                && (tile_x as usize) < window.origin_x.saturating_add(window.width)
                && (tile_y as usize) < window.origin_y.saturating_add(window.height)
            {
                Some((family, variant, layer, tile_x, tile_y))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if icon_primitives.is_empty() {
        return None;
    }

    icon_primitives.sort_by_key(|(_, _, layer, _, _)| *layer);
    let mut parts = vec![format!("count={}", icon_primitives.len())];
    for (family, variant, layer, tile_x, tile_y) in icon_primitives.into_iter().take(2) {
        parts.push(format!(
            "{}/{}@{layer}:{tile_x}:{tile_y}",
            family.label(),
            variant
        ));
    }
    Some(parts.join(" "))
}

fn compose_render_icon_detail_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut icon_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Icon {
                family,
                variant,
                layer,
                x,
                y,
                ..
            } => {
                let payload = primitive.payload()?;
                let (tile_x, tile_y) = finite_world_tile(*x, *y)?;
                if !render_icon_detail_is_visible(window, tile_x, tile_y) {
                    return None;
                }

                Some((
                    *layer,
                    family.label(),
                    variant.clone(),
                    tile_x,
                    tile_y,
                    payload,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if icon_primitives.is_empty() {
        return None;
    }

    icon_primitives.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.cmp(&right.4))
    });

    let mut parts = vec![format!("count={}", icon_primitives.len())];
    for (layer, family_label, variant, tile_x, tile_y, payload) in icon_primitives {
        parts.push(format!(
            "{family_label}/{variant}@{layer}:{tile_x}:{tile_y} payload[{}]",
            render_primitive_payload_fields_text(&payload)
        ));
    }
    Some(parts.join(" | "))
}

fn render_icon_detail_is_visible(window: PresenterViewWindow, tile_x: i32, tile_y: i32) -> bool {
    tile_x >= 0
        && tile_y >= 0
        && (tile_x as usize) >= window.origin_x
        && (tile_y as usize) >= window.origin_y
        && (tile_x as usize) < window.origin_x.saturating_add(window.width)
        && (tile_y as usize) < window.origin_y.saturating_add(window.height)
}

fn render_rect_detail_is_visible(
    window: PresenterViewWindow,
    left_tile: i32,
    top_tile: i32,
    right_tile: i32,
    bottom_tile: i32,
) -> bool {
    !(right_tile < window.origin_x as i32
        || bottom_tile < window.origin_y as i32
        || left_tile >= window.origin_x.saturating_add(window.width) as i32
        || top_tile >= window.origin_y.saturating_add(window.height) as i32)
}

fn render_rect_detail_payload_fields(
    payload: Option<&RenderPrimitivePayload>,
) -> (Option<String>, Option<i32>, Option<i32>) {
    let block_name = payload
        .and_then(|payload| payload.field("block_name"))
        .and_then(|value| match value {
            RenderPrimitivePayloadValue::Text(value) => Some(value.clone()),
            _ => None,
        });
    let tile_x = payload
        .and_then(|payload| payload.field("tile_x"))
        .and_then(|value| match value {
            RenderPrimitivePayloadValue::I32(value) => Some(*value),
            _ => None,
        });
    let tile_y = payload
        .and_then(|payload| payload.field("tile_y"))
        .and_then(|value| match value {
            RenderPrimitivePayloadValue::I32(value) => Some(*value),
            _ => None,
        });
    (block_name, tile_x, tile_y)
}

fn render_rect_detail_fields_text(
    left_tile: i32,
    top_tile: i32,
    right_tile: i32,
    bottom_tile: i32,
    line_count: usize,
    block_name: Option<&str>,
    tile_x: Option<i32>,
    tile_y: Option<i32>,
) -> String {
    let width_tiles = (right_tile - left_tile).max(0);
    let height_tiles = (bottom_tile - top_tile).max(0);
    let mut parts = vec![
        format!("left_tile={left_tile}"),
        format!("top_tile={top_tile}"),
        format!("right_tile={right_tile}"),
        format!("bottom_tile={bottom_tile}"),
        format!("width_tiles={width_tiles}"),
        format!("height_tiles={height_tiles}"),
        format!("line_count={line_count}"),
    ];
    if let Some(block_name) = block_name {
        parts.push(format!("block_name={block_name}"));
    }
    if let Some(tile_x) = tile_x {
        parts.push(format!("tile_x={tile_x}"));
    }
    if let Some(tile_y) = tile_y {
        parts.push(format!("tile_y={tile_y}"));
    }
    parts.join(",")
}

fn render_primitive_payload_fields_text(payload: &RenderPrimitivePayload) -> String {
    payload
        .fields
        .iter()
        .map(|(name, value)| format!("{}={}", *name, render_primitive_payload_value_text(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn render_primitive_payload_value_text(value: &RenderPrimitivePayloadValue) -> String {
    match value {
        RenderPrimitivePayloadValue::Bool(value) => bool_flag(*value).to_string(),
        RenderPrimitivePayloadValue::I16(value) => value.to_string(),
        RenderPrimitivePayloadValue::I32(value) => value.to_string(),
        RenderPrimitivePayloadValue::I32List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        RenderPrimitivePayloadValue::U8(value) => value.to_string(),
        RenderPrimitivePayloadValue::U8List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        RenderPrimitivePayloadValue::U32(value) => format!("0x{value:08x}"),
        RenderPrimitivePayloadValue::Usize(value) => value.to_string(),
        RenderPrimitivePayloadValue::Text(value) => value.clone(),
        RenderPrimitivePayloadValue::TextList(values) => format!("[{}]", values.join(",")),
    }
}

fn draw_ascii_text(
    grid: &mut [Vec<char>],
    _window: PresenterViewWindow,
    local_x: usize,
    local_y: usize,
    text: &str,
) {
    for (row_offset, line) in text.lines().enumerate() {
        let y = local_y + row_offset;
        if y >= grid.len() {
            break;
        }

        for (col_offset, ch) in line.chars().enumerate() {
            let x = local_x + col_offset;
            if x >= grid[y].len() {
                break;
            }
            grid[y][x] = ch;
        }
    }
}

fn draw_ascii_tile_if_visible(
    grid: &mut [Vec<char>],
    window: PresenterViewWindow,
    tile_x: i32,
    tile_y: i32,
    sprite: char,
) {
    let Ok(tile_x) = usize::try_from(tile_x) else {
        return;
    };
    let Ok(tile_y) = usize::try_from(tile_y) else {
        return;
    };
    if tile_x < window.origin_x
        || tile_y < window.origin_y
        || tile_x >= window.origin_x.saturating_add(window.width)
        || tile_y >= window.origin_y.saturating_add(window.height)
    {
        return;
    }
    let local_x = tile_x - window.origin_x;
    let local_y = tile_y - window.origin_y;
    grid[local_y][local_x] = sprite;
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

    let zoom = normalize_zoom(scene.viewport.zoom);
    let window_width = zoomed_view_tile_span(max_width, zoom, base_window.width);
    let window_height = zoomed_view_tile_span(max_height, zoom, base_window.height);
    crop_window_to_focus(scene, TILE_SIZE, base_window, window_width, window_height)
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

fn ascii_sprite_for_icon(family: RenderIconPrimitiveFamily) -> char {
    match family {
        RenderIconPrimitiveFamily::RuntimeEffect => ASCII_ICON_RUNTIME_EFFECT,
        RenderIconPrimitiveFamily::RuntimeEffectMarker => ASCII_ICON_RUNTIME_EFFECT_MARKER,
        RenderIconPrimitiveFamily::RuntimeBuildConfig => ASCII_ICON_BUILD_CONFIG,
        RenderIconPrimitiveFamily::RuntimeConfig
        | RenderIconPrimitiveFamily::RuntimeConfigParseFail
        | RenderIconPrimitiveFamily::RuntimeConfigNoApply
        | RenderIconPrimitiveFamily::RuntimeConfigRollback
        | RenderIconPrimitiveFamily::RuntimeConfigPendingMismatch => ASCII_ICON_BUILD_CONFIG,
        RenderIconPrimitiveFamily::RuntimeHealth => ASCII_ICON_RUNTIME_HEALTH,
        RenderIconPrimitiveFamily::RuntimeCommand => ASCII_ICON_RUNTIME_COMMAND,
        RenderIconPrimitiveFamily::RuntimePlace => ASCII_ICON_RUNTIME_PLACE,
        RenderIconPrimitiveFamily::RuntimeUnitAssemblerProgress
        | RenderIconPrimitiveFamily::RuntimeUnitAssemblerCommand => {
            ASCII_ICON_RUNTIME_UNIT_ASSEMBLER
        }
        RenderIconPrimitiveFamily::RuntimeBreak => ASCII_ICON_RUNTIME_BREAK,
        RenderIconPrimitiveFamily::RuntimeBullet => ASCII_ICON_RUNTIME_BULLET,
        RenderIconPrimitiveFamily::RuntimeLogicExplosion => ASCII_ICON_RUNTIME_LOGIC_EXPLOSION,
        RenderIconPrimitiveFamily::RuntimeSoundAt => ASCII_ICON_RUNTIME_SOUND_AT,
        RenderIconPrimitiveFamily::RuntimeTileAction => ASCII_ICON_RUNTIME_TILE_ACTION,
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

fn compose_visibility_minimap_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let visibility = build_hud_visibility_panel(hud)?;
    let minimap = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "overlay={} fog={} known={}({}%) vis={}({}%/{}%) hid={}({}%/{}%) map={}x{} window={}:{}->{}:{} size={}x{} cover={}/{}({}%) focus={} in-window={}",
        bool_flag(visibility.overlay_visible),
        bool_flag(visibility.fog_enabled),
        visibility.known_tile_count,
        visibility.known_tile_percent,
        visibility.visible_tile_count,
        visibility.visible_known_percent,
        visibility.visible_map_percent(),
        visibility.hidden_tile_count,
        visibility.hidden_known_percent,
        visibility.hidden_map_percent(),
        minimap.map_width,
        minimap.map_height,
        minimap.window.origin_x,
        minimap.window.origin_y,
        minimap.window_last_x,
        minimap.window_last_y,
        minimap.window.width,
        minimap.window.height,
        minimap.window_tile_count,
        minimap.map_tile_count,
        minimap.window_coverage_percent,
        optional_focus_tile_text(minimap.focus_tile),
        optional_bool_label(minimap.focus_in_window),
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
    let menu = &runtime_ui.menu;
    let text_input = &runtime_ui.text_input;
    let live = &runtime_ui.live;
    Some(format!(
        "hud={}/{}/{}@{}/{} ann={}@{} info={}@{} toast={}/{}@{}/{} popup={}/{} clip={} uri={} choice={}/{} tin={}@{}:{}/{}/{}#{}:n{}:e{} live=ent={} fx={}",
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
        compose_live_entity_text(&live.entity),
        compose_live_effect_text(&live.effect),
    ))
}

fn compose_runtime_ui_notice_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_notice_panel(hud)?;
    Some(format!(
        "hud={}/{}/{}@{}/{} ann={}@{} info={}@{} toast={}/{}@{}/{} popup={}/{}@{}:{}/{} clip={}@{} uri={}@{}:{} tin={}@{}:{}/{}/{}#{}:n{}:e{}",
        panel.hud_set_count,
        panel.hud_set_reliable_count,
        panel.hud_hide_count,
        compact_runtime_ui_text(panel.hud_last_message.as_deref()),
        compact_runtime_ui_text(panel.hud_last_reliable_message.as_deref()),
        panel.announce_count,
        compact_runtime_ui_text(panel.last_announce_message.as_deref()),
        panel.info_message_count,
        compact_runtime_ui_text(panel.last_info_message.as_deref()),
        panel.toast_info_count,
        panel.toast_warning_count,
        compact_runtime_ui_text(panel.toast_last_info_message.as_deref()),
        compact_runtime_ui_text(panel.toast_last_warning_text.as_deref()),
        panel.info_popup_count,
        panel.info_popup_reliable_count,
        optional_bool_label(panel.last_info_popup_reliable),
        compact_runtime_ui_text(panel.last_info_popup_id.as_deref()),
        compact_runtime_ui_text(panel.last_info_popup_message.as_deref()),
        panel.clipboard_count,
        compact_runtime_ui_text(panel.last_clipboard_text.as_deref()),
        panel.open_uri_count,
        compact_runtime_ui_text(panel.last_open_uri.as_deref()),
        runtime_ui_uri_scheme(panel.last_open_uri.as_deref()),
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
        "active=1 hud-events={}/{}/{} hud-len={}/{} announce={} len={} info={} len={} toast-events={}/{} toast-len={}/{} popup={}/{} rel={} id-len={} msg-len={} dur={} box={}:{}/{}/{}/{} clip={} len={} uri={} len={} scheme={} text-input={} id={} title-len={} msg-len={} default-len={} limit={} numeric={} allow-empty={}",
        panel.hud_set_count,
        panel.hud_set_reliable_count,
        panel.hud_hide_count,
        runtime_ui_text_len(panel.hud_last_message.as_deref()),
        runtime_ui_text_len(panel.hud_last_reliable_message.as_deref()),
        panel.announce_count,
        runtime_ui_text_len(panel.last_announce_message.as_deref()),
        panel.info_message_count,
        runtime_ui_text_len(panel.last_info_message.as_deref()),
        panel.toast_info_count,
        panel.toast_warning_count,
        runtime_ui_text_len(panel.toast_last_info_message.as_deref()),
        runtime_ui_text_len(panel.toast_last_warning_text.as_deref()),
        panel.info_popup_count,
        panel.info_popup_reliable_count,
        optional_bool_label(panel.last_info_popup_reliable),
        runtime_ui_text_len(panel.last_info_popup_id.as_deref()),
        runtime_ui_text_len(panel.last_info_popup_message.as_deref()),
        optional_u32_label(panel.last_info_popup_duration_bits),
        optional_i32_label(panel.last_info_popup_align),
        optional_i32_label(panel.last_info_popup_top),
        optional_i32_label(panel.last_info_popup_left),
        optional_i32_label(panel.last_info_popup_bottom),
        optional_i32_label(panel.last_info_popup_right),
        panel.clipboard_count,
        runtime_ui_text_len(panel.last_clipboard_text.as_deref()),
        panel.open_uri_count,
        runtime_ui_text_len(panel.last_open_uri.as_deref()),
        runtime_ui_uri_scheme(panel.last_open_uri.as_deref()),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        runtime_ui_text_len(panel.text_input_last_message.as_deref()),
        runtime_ui_text_len(panel.text_input_last_default_text.as_deref()),
        optional_i32_label(panel.text_input_last_length),
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

fn compose_runtime_rules_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_rules_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "set-rules={} set-objectives={} set-rule={} clear-objectives={} complete-objective={}",
        panel.set_rules_count,
        panel.set_objectives_count,
        panel.set_rule_count,
        panel.clear_objectives_count,
        panel.complete_objective_count,
    ))
}

fn compose_runtime_menu_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_menu_panel(hud)?;
    Some(format!(
        "menu={}@{}:{}/{}#{}:{} follow={}@{}:{}/{}#{}:{} hide={}@{} tin={}@{}:{}/{}#{}:n{}:e{}",
        panel.menu_open_count,
        optional_i32_label(panel.last_menu_open_id),
        compact_runtime_ui_text(panel.last_menu_open_title.as_deref()),
        compact_runtime_ui_text(panel.last_menu_open_message.as_deref()),
        panel.last_menu_open_option_rows,
        panel.last_menu_open_first_row_len,
        panel.follow_up_menu_open_count,
        optional_i32_label(panel.last_follow_up_menu_open_id),
        compact_runtime_ui_text(panel.last_follow_up_menu_open_title.as_deref()),
        compact_runtime_ui_text(panel.last_follow_up_menu_open_message.as_deref()),
        panel.last_follow_up_menu_open_option_rows,
        panel.last_follow_up_menu_open_first_row_len,
        panel.hide_follow_up_menu_count,
        optional_i32_label(panel.last_hide_follow_up_menu_id),
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
        "active={} outstanding-follow-up={} menu={} title-len={} message-len={} rows={}/{} follow={} title-len={} message-len={} rows={}/{} hide-id={} text-input={} id={} title={} default-len={} numeric={} allow-empty={}",
        if panel.text_input_open_count > 0
            || panel.menu_open_count > 0
            || panel.outstanding_follow_up_count() > 0
        {
            1
        } else {
            0
        },
        panel.outstanding_follow_up_count(),
        optional_i32_label(panel.last_menu_open_id),
        panel.menu_title_len(),
        panel.menu_message_len(),
        panel.last_menu_open_option_rows,
        panel.last_menu_open_first_row_len,
        optional_i32_label(panel.last_follow_up_menu_open_id),
        panel.follow_up_title_len(),
        panel.follow_up_message_len(),
        panel.last_follow_up_menu_open_option_rows,
        panel.last_follow_up_menu_open_first_row_len,
        optional_i32_label(panel.last_hide_follow_up_menu_id),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        panel.default_text_len(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_choice_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_choice_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "menu-choose={}@{}/{} tin-result={}@{}/{}",
        panel.menu_choose_count,
        optional_i32_label(panel.last_menu_choose_menu_id),
        optional_i32_label(panel.last_menu_choose_option),
        panel.text_input_result_count,
        optional_i32_label(panel.last_text_input_result_id),
        compact_runtime_ui_text(panel.last_text_input_result_text.as_deref()),
    ))
}

fn compose_runtime_choice_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_choice_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "choose-menu={} choose-option={} result-id={} result-len={}",
        optional_i32_label(panel.last_menu_choose_menu_id),
        optional_i32_label(panel.last_menu_choose_option),
        optional_i32_label(panel.last_text_input_result_id),
        panel.text_input_result_len(),
    ))
}

fn compose_runtime_prompt_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_prompt_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let layers = panel.layer_labels().join(">");
    Some(format!(
        "kind={} active={} depth={} layers={} menu={} follow-up={} tin={}@{}:{}/{}/{}#{}:n{}:e{}",
        runtime_dialog_prompt_text(panel.kind),
        bool_flag(panel.is_active()),
        panel.depth(),
        if layers.is_empty() {
            "none"
        } else {
            layers.as_str()
        },
        panel.menu_open_count,
        panel.outstanding_follow_up_count(),
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

fn compose_runtime_prompt_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_prompt_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "menu-active={} follow-up-open={} follow-up-hide={} outstanding-follow-up={} text-input={} id={} title-len={} message-len={} default-len={} numeric={} allow-empty={}",
        bool_flag(panel.menu_active()),
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.outstanding_follow_up_count(),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        panel.prompt_message_len(),
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
    let prompt = build_runtime_prompt_panel(hud)?;
    let notice = build_runtime_notice_state_panel(hud)?;
    if panel.is_empty() && !notice.is_active() && notice.count == 0 && notice.text.is_none() {
        return None;
    }
    Some(format!(
        "prompt={} active={} layers=menu:{}/follow-up:{}/input:{} message-len={} default-len={} notice={} layers=hud:{}/reliable:{}/info:{}/warn:{} notice-len={}",
        runtime_dialog_prompt_text(prompt.kind),
        bool_flag(prompt.is_active()),
        bool_flag(prompt.menu_active()),
        panel.outstanding_follow_up_count(),
        prompt.text_input_open_count,
        panel.prompt_message_len(),
        panel.default_text_len(),
        runtime_dialog_notice_text(notice.kind),
        bool_flag(notice.hud_active),
        bool_flag(notice.reliable_hud_active),
        bool_flag(notice.toast_info_active),
        bool_flag(notice.toast_warning_active),
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
    let panel = build_runtime_dialog_stack_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "dialog=front:{} groups:{} total:{} prompt={}/menu:{}/follow-up:{}/input:{} notice={}/hud:{}/reliable:{}/info:{}/warn:{} chat=active:{}/server:{}/local:{} sender={}",
        panel.foreground_label(),
        panel.active_group_count(),
        panel.total_depth(),
        runtime_dialog_prompt_text(panel.prompt.kind),
        bool_flag(panel.prompt.menu_active()),
        panel.prompt.outstanding_follow_up_count(),
        panel.prompt.text_input_open_count,
        runtime_dialog_notice_text(panel.notice.kind),
        bool_flag(panel.notice.hud_active),
        bool_flag(panel.notice.reliable_hud_active),
        bool_flag(panel.notice.toast_info_active),
        bool_flag(panel.notice.toast_warning_active),
        bool_flag(!panel.chat.is_empty()),
        panel.chat.server_message_count,
        panel.chat.chat_message_count,
        optional_i32_label(panel.chat.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_stack_depth_text(hud: &HudModel) -> Option<String> {
    let summary = hud.runtime_ui_stack_depth_summary()?;
    if summary.is_empty() {
        return None;
    }
    Some(format!(
        "prompt={} notice={} chat={} menu={} hud={} dialog={} groups={} total={}",
        summary.prompt_depth,
        summary.notice_depth,
        summary.chat_depth,
        summary.menu_depth(),
        summary.hud_depth(),
        summary.dialog_depth(),
        summary.active_group_count,
        summary.total_depth,
    ))
}

fn compose_runtime_dialog_stack_text(hud: &HudModel) -> Option<String> {
    let summary = hud.runtime_ui_stack_summary()?;
    if summary.is_empty() {
        return None;
    }
    let prompt_layers = summary.prompt_layer_labels().join(">");
    let notice_layers = summary.notice_layer_labels().join(">");
    Some(format!(
        "front={} prompt={}@{} menu={} follow-up={} input={} notice={}@{} depths=menu:{}/hud:{}/dialog:{} chat={} server={} local={} tin={} sender={} total={}",
        summary.foreground_label(),
        summary.prompt_label(),
        if prompt_layers.is_empty() {
            "none"
        } else {
            prompt_layers.as_str()
        },
        summary.menu_open_count,
        summary.outstanding_follow_up_count,
        summary.text_input_open_count,
        summary.notice_label(),
        if notice_layers.is_empty() {
            "none"
        } else {
            notice_layers.as_str()
        },
        summary.menu_depth(),
        summary.hud_depth(),
        summary.dialog_depth(),
        bool_flag(summary.chat_active),
        summary.server_message_count,
        summary.chat_message_count,
        optional_i32_label(summary.text_input_last_id),
        optional_i32_label(summary.last_chat_sender_entity_id),
        summary.total_depth(),
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

fn compose_runtime_command_mode_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_command_mode_panel(hud)?;
    Some(format!(
        "sample={} groups={} first-building={} rect={} target={} command={} stance={}",
        command_i32_sample_text(&panel.selected_unit_sample),
        command_control_groups_text(&panel.control_groups),
        optional_i32_label(panel.first_command_building),
        command_rect_text(panel.command_rect),
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

fn compose_runtime_admin_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_admin_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "trace={} fail={} last-player={} debug={} fail={} unreliable={} fail={} last-value={}",
        panel.trace_info_count,
        panel.trace_info_parse_fail_count,
        optional_i32_label(panel.last_trace_info_player_id),
        panel.debug_status_client_count,
        panel.debug_status_client_parse_fail_count,
        panel.debug_status_client_unreliable_count,
        panel.debug_status_client_unreliable_parse_fail_count,
        optional_i32_label(panel.last_debug_status_value),
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

fn compose_runtime_marker_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_marker_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "create={} remove={} update={} text={} texture={} fail={} last={} control={}",
        panel.create_count,
        panel.remove_count,
        panel.update_count,
        panel.update_text_count,
        panel.update_texture_count,
        panel.decode_fail_count,
        optional_i32_label(panel.last_marker_id),
        compact_runtime_ui_text(panel.last_control_name.as_deref()),
    ))
}

fn compose_runtime_marker_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_marker_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "total={} mutate={} text={} texture={} fail={} last={} control-len={}",
        panel.total_count(),
        panel.mutate_count(),
        panel.update_text_count,
        panel.update_texture_count,
        panel.decode_fail_count,
        optional_i32_label(panel.last_marker_id),
        panel.control_name_len(),
    ))
}

fn compose_runtime_kick_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_kick_panel(hud)?;
    Some(compose_runtime_kick_panel_text(&panel))
}

fn compose_runtime_bootstrap_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_bootstrap_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(panel.summary_label())
}

fn compose_runtime_bootstrap_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_bootstrap_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(panel.detail_label())
}

fn compose_runtime_session_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    if let Some(bootstrap_text) = compose_runtime_bootstrap_row_text(hud) {
        segments.push(format!("bootstrap={bootstrap_text}"));
    }
    segments.push(format!(
        "resource={}",
        compose_runtime_resource_delta_panel_text(&panel.resource_delta)
    ));
    segments.push(format!(
        "kick={}",
        compose_runtime_kick_panel_text(&panel.kick)
    ));
    segments.push(format!(
        "loading={}",
        compose_runtime_loading_panel_text(&panel.loading)
    ));
    segments.push(format!(
        "reconnect={}",
        compose_runtime_reconnect_panel_text(&panel.reconnect)
    ));
    Some(segments.join("; "))
}

fn compose_runtime_session_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    if let Some(bootstrap_text) = compose_runtime_bootstrap_detail_text(hud) {
        segments.push(format!("bootstrap=[{bootstrap_text}]"));
    }
    segments.push(format!(
        "resource=[{}]",
        compose_runtime_resource_delta_detail_panel_text(&panel.resource_delta)
    ));
    segments.push(format!(
        "kick=[{}]",
        compose_runtime_kick_detail_panel_text(&panel.kick)
    ));
    segments.push(format!(
        "loading=[{}]",
        compose_runtime_loading_detail_panel_text(&panel.loading)
    ));
    segments.push(format!(
        "reconnect=[{}]",
        compose_runtime_reconnect_detail_panel_text(&panel.reconnect)
    ));
    Some(segments.join(" "))
}

fn compose_runtime_resource_delta_panel_text(
    resource_delta: &crate::panel_model::RuntimeResourceDeltaPanelModel,
) -> String {
    format!(
        "tiles={}/{}/{}/{} set={}/{}/{}/{} clear={}/{} tile={}/{} flow={}/{}/{} last={}@{}#{}:bp{}:u{}:eid{} proj={}/{}/{} auth={} delta={}/{}/{} chg={}/{}/{}/{}",
        resource_delta.remove_tile_count,
        resource_delta.set_tile_count,
        resource_delta.set_floor_count,
        resource_delta.set_overlay_count,
        resource_delta.set_item_count,
        resource_delta.set_items_count,
        resource_delta.set_liquid_count,
        resource_delta.set_liquids_count,
        resource_delta.clear_items_count,
        resource_delta.clear_liquids_count,
        resource_delta.set_tile_items_count,
        resource_delta.set_tile_liquids_count,
        resource_delta.take_items_count,
        resource_delta.transfer_item_to_count,
        resource_delta.transfer_item_to_unit_count,
        compact_runtime_ui_text(resource_delta.last_kind.as_deref()),
        optional_i16_label(resource_delta.last_item_id),
        optional_i32_label(resource_delta.last_amount),
        optional_i32_label(resource_delta.last_build_pos),
        command_unit_ref_text(resource_delta.last_unit),
        optional_i32_label(resource_delta.last_to_entity_id),
        resource_delta.build_count,
        resource_delta.build_stack_count,
        resource_delta.entity_count,
        resource_delta.authoritative_build_update_count,
        resource_delta.delta_apply_count,
        resource_delta.delta_skip_count,
        resource_delta.delta_conflict_count,
        optional_i32_label(resource_delta.last_changed_build_pos),
        optional_i32_label(resource_delta.last_changed_entity_id),
        optional_i16_label(resource_delta.last_changed_item_id),
        optional_i32_label(resource_delta.last_changed_amount),
    )
}

fn compose_runtime_resource_delta_detail_panel_text(
    resource_delta: &crate::panel_model::RuntimeResourceDeltaPanelModel,
) -> String {
    format!(
        "tile-rm={} tile-set={} floor-set={} overlay-set={} item-set={}/{} liquid-set={}/{} clear={}/{} tile-apply={}/{} flow={}/{}/{} last-kind={} item={} amount={} build={} unit={} to-entity={} projection={}/{}/{} authoritative={} delta={}/{}/{} changed={}/{}/{}/{}",
        resource_delta.remove_tile_count,
        resource_delta.set_tile_count,
        resource_delta.set_floor_count,
        resource_delta.set_overlay_count,
        resource_delta.set_item_count,
        resource_delta.set_items_count,
        resource_delta.set_liquid_count,
        resource_delta.set_liquids_count,
        resource_delta.clear_items_count,
        resource_delta.clear_liquids_count,
        resource_delta.set_tile_items_count,
        resource_delta.set_tile_liquids_count,
        resource_delta.take_items_count,
        resource_delta.transfer_item_to_count,
        resource_delta.transfer_item_to_unit_count,
        compact_runtime_ui_text(resource_delta.last_kind.as_deref()),
        optional_i16_label(resource_delta.last_item_id),
        optional_i32_label(resource_delta.last_amount),
        optional_i32_label(resource_delta.last_build_pos),
        command_unit_ref_text(resource_delta.last_unit),
        optional_i32_label(resource_delta.last_to_entity_id),
        resource_delta.build_count,
        resource_delta.build_stack_count,
        resource_delta.entity_count,
        resource_delta.authoritative_build_update_count,
        resource_delta.delta_apply_count,
        resource_delta.delta_skip_count,
        resource_delta.delta_conflict_count,
        optional_i32_label(resource_delta.last_changed_build_pos),
        optional_i32_label(resource_delta.last_changed_entity_id),
        optional_i16_label(resource_delta.last_changed_item_id),
        optional_i32_label(resource_delta.last_changed_amount),
    )
}

fn compose_runtime_loading_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    Some(compose_runtime_loading_panel_text(&panel))
}

fn compose_runtime_kick_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_kick_panel(hud)?;
    (!panel.is_empty()).then(|| compose_runtime_kick_detail_panel_text(&panel))
}

fn compose_runtime_loading_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    (!panel.is_empty()).then(|| compose_runtime_loading_detail_panel_text(&panel))
}

fn compose_runtime_core_binding_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_core_binding_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "kind={} ambiguous={}@{} missing={}@{}",
        panel.kind_label(),
        panel.ambiguous_team_count,
        team_u8_sample_text(&panel.ambiguous_team_sample),
        panel.missing_team_count,
        team_u8_sample_text(&panel.missing_team_sample),
    ))
}

fn compose_runtime_core_binding_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_core_binding_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "kind={} ambiguous-count={} ambiguous-sample={} missing-count={} missing-sample={}",
        panel.kind_label(),
        panel.ambiguous_team_count,
        team_u8_sample_text(&panel.ambiguous_team_sample),
        panel.missing_team_count,
        team_u8_sample_text(&panel.missing_team_sample),
    ))
}

fn compose_runtime_reconnect_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_reconnect_panel(hud)?;
    Some(compose_runtime_reconnect_panel_text(&panel))
}

fn compose_runtime_reconnect_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_reconnect_panel(hud)?;
    (!panel.is_empty()).then(|| compose_runtime_reconnect_detail_panel_text(&panel))
}

fn compose_runtime_live_entity_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(compose_live_entity_panel_text(&panel))
}

fn compose_runtime_live_entity_detail_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(panel.detail_label())
}

fn compose_runtime_live_effect_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_effect_panel(hud)?;
    Some(compose_live_effect_panel_text(&panel))
}

fn compose_runtime_live_effect_detail_row_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_effect_panel(hud)?;
    Some(format!(
        "hint={} source={} pos={} ttl={} data={} active-rel={} contract={} reliable={}",
        panel.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_text(panel.display_position_source()),
        world_position_text(panel.display_position()),
        live_effect_ttl_text(panel.display_overlay_ttl()),
        live_effect_data_shape_text(panel.last_data_len, panel.last_data_type_tag),
        live_effect_reliable_flag_text(panel.active_reliable),
        compact_runtime_ui_text(panel.display_contract_name()),
        compact_runtime_ui_text(panel.display_reliable_contract_name()),
    ))
}

fn compose_build_ui_text(hud: &HudModel) -> Option<String> {
    let build_ui = hud.build_ui.as_ref()?;
    Some(compose_build_ui_summary_text(build_ui))
}

fn compose_build_ui_queue_text(hud: &HudModel) -> Option<String> {
    let build_ui = hud.build_ui.as_ref()?;
    Some(compose_build_ui_queue_summary_text(build_ui))
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
        "overlay={} fog={} known={}({}%) vis={}({}%/{}%) hid={}({}%/{}%) unseen={}({}%) density=map:{}/{}({}%) window:{}/{}({}%) offscreen:{}/{}({}%)",
        if panel.overlay_visible { 1 } else { 0 },
        if panel.fog_enabled { 1 } else { 0 },
        panel.known_tile_count,
        panel.known_tile_percent,
        panel.visible_tile_count,
        panel.visible_known_percent,
        panel.visible_map_percent(),
        panel.hidden_tile_count,
        panel.hidden_known_percent,
        panel.hidden_map_percent(),
        panel.unknown_tile_count,
        panel.unknown_tile_percent,
        panel.tracked_object_count,
        panel.map_tile_count,
        panel.map_object_density_percent(),
        panel.window_tracked_object_count,
        panel.window_tile_count,
        panel.window_object_density_percent(),
        panel.outside_window_count,
        panel.tracked_object_count,
        panel.outside_object_percent(),
    ))
}

fn compose_minimap_flow_line(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_user_flow_panel(scene, hud, window)?;
    Some(format!(
        "next={} focus={} pan={} vis={} cover={} target={} overlay-targets={}",
        panel.next_action,
        panel.focus_state.label(),
        panel.pan_label(),
        panel.visibility_label(),
        panel.coverage_label(),
        panel.target_kind.label(),
        panel.overlay_target_count,
    ))
}

fn compose_minimap_visibility_detail_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let visibility = build_minimap_user_flow_panel(scene, hud, window)?;
    let minimap = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "visibility={} coverage={} density=map:{}% window:{}% offscreen:{}%",
        visibility.visibility_label(),
        visibility.coverage_label(),
        minimap.map_object_density_percent(),
        minimap.window_object_density_percent(),
        minimap.outside_object_percent(),
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
    lines.push(compose_minimap_window_kind_distribution_line(&panel));
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

fn compose_minimap_window_kind_distribution_line(panel: &MinimapPanelModel) -> String {
    format!(
        "window-kinds: tracked={} outside={} player={} marker={} plan={} block={} runtime={} terrain={} unknown={}",
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
    Some("@=player M=marker P=plan #=block R=runtime overlay .=terrain ?=unknown".to_string())
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

fn compose_build_minimap_aux_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    let window_tile_count = window.width.saturating_mul(window.height);
    Some(format!(
        "mode={} select={} queue={} place-ready={} cfg={}/{} top={} auth={} head={} auth-tile={} src={} focus={} in-window={} visible-map={} unknown-map={} window={} d{} tracked={} runtime={} runtime-share={}%",
        build_interaction_mode_text(panel.mode),
        build_interaction_selection_text(panel.selection_state),
        build_interaction_queue_text(panel.queue_state),
        if panel.place_ready { 1 } else { 0 },
        panel.config_family_count,
        panel.config_sample_count,
        compact_runtime_ui_text(panel.top_config_family.as_deref()),
        build_interaction_authority_text(panel.authority_state),
        build_flow_head_tile_text(panel.head_tile),
        build_config_tile_text(panel.authority_tile),
        build_config_rollback_source_text(panel.authority_source),
        optional_focus_tile_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        panel.visible_map_percent,
        panel.unknown_tile_percent,
        panel.window_coverage_percent,
        window_object_density_percent(panel.tracked_object_count, window_tile_count),
        panel.tracked_object_count,
        panel.runtime_count,
        panel.runtime_share_percent(),
    ))
}

fn window_object_density_percent(tracked_object_count: usize, window_tile_count: usize) -> usize {
    if window_tile_count == 0 {
        0
    } else {
        ((tracked_object_count as u128) * 100 / (window_tile_count as u128)) as usize
    }
}

fn compose_build_flow_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(format!(
        "next={} minimap={} focus={} pan={} target={} scope={} head={} auth={}",
        panel.next_action,
        panel.minimap_next_action,
        panel.focus_state.label(),
        panel.pan_label(),
        panel.target_kind.label(),
        panel.config_scope,
        build_flow_head_tile_text(panel.head_tile),
        build_interaction_authority_text(panel.authority_state),
    ))
}

fn compose_build_flow_detail_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(panel.detail_label())
}

fn compose_build_flow_summary_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(panel.summary_label())
}

fn compose_build_route_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    let blockers = panel.blocker_labels().join(">");
    let route = panel.route.join(">");
    Some(format!(
        "next={} minimap={} blockers={}@{} route={}@{}",
        panel.next_action,
        panel.minimap_next_action,
        panel.blocker_count(),
        if blockers.is_empty() {
            "none"
        } else {
            blockers.as_str()
        },
        panel.route_count(),
        route.as_str(),
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
        "sel={} rot={} building={} cfg={}",
        optional_i16_label(build_ui.selected_block_id),
        build_ui.selected_rotation,
        if build_ui.building { 1 } else { 0 },
        build_ui.inspector_entries.len(),
    )
}

fn compose_build_ui_queue_summary_text(build_ui: &crate::BuildUiObservability) -> String {
    format!(
        "queue={}/{}/{}/{}/{} head={}",
        build_ui.queued_count,
        build_ui.inflight_count,
        build_ui.finished_count,
        build_ui.removed_count,
        build_ui.orphan_authoritative_count,
        build_queue_head_text(build_ui.head.as_ref()),
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

fn build_flow_head_tile_text(value: Option<(i32, i32)>) -> String {
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
        "{}/{}:ov{}@{}:u{}:d{}:k{}:c{}/{}:r{}:h{}:p{}@{}:ttl{}",
        effect.effect_count,
        effect.spawn_effect_count,
        effect.active_overlay_count,
        optional_i16_label(effect.display_effect_id()),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        live_effect_data_shape_text(effect.last_data_len, effect.last_data_type_tag),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.display_contract_name()),
        compact_runtime_ui_text(effect.display_reliable_contract_name()),
        live_effect_reliable_flag_text(effect.active_reliable),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_text(effect.display_position_source()),
        world_position_text(effect.display_position()),
        live_effect_ttl_text(effect.display_overlay_ttl()),
    )
}

fn compose_live_effect_panel_text(
    effect: &crate::panel_model::RuntimeLiveEffectPanelModel,
) -> String {
    format!(
        "{}/{}:ov{}@{}:u{}:d{}:k{}:c{}/{}:r{}:h{}:p{}@{}:ttl{}",
        effect.effect_count,
        effect.spawn_effect_count,
        effect.active_overlay_count,
        optional_i16_label(effect.display_effect_id()),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        live_effect_data_shape_text(effect.last_data_len, effect.last_data_type_tag),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.display_contract_name()),
        compact_runtime_ui_text(effect.display_reliable_contract_name()),
        live_effect_reliable_flag_text(effect.active_reliable),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_text(effect.display_position_source()),
        world_position_text(effect.display_position()),
        live_effect_ttl_text(effect.display_overlay_ttl()),
    )
}

fn live_effect_ttl_text(ttl: Option<(u8, u8)>) -> String {
    match ttl {
        Some((remaining, total)) => format!("{remaining}/{total}"),
        None => "none".to_string(),
    }
}

fn live_effect_data_shape_text(data_len: Option<usize>, data_type_tag: Option<u8>) -> String {
    match (data_len, data_type_tag) {
        (Some(data_len), Some(data_type_tag)) => format!("{data_len}/{data_type_tag}"),
        (Some(data_len), None) => format!("{data_len}/none"),
        (None, Some(data_type_tag)) => format!("none/{data_type_tag}"),
        (None, None) => "none".to_string(),
    }
}

fn live_effect_reliable_flag_text(flag: Option<bool>) -> &'static str {
    match flag {
        Some(true) => "1",
        Some(false) => "0",
        None => "?",
    }
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

fn compose_runtime_kick_detail_panel_text(
    kick: &crate::panel_model::RuntimeKickPanelModel,
) -> String {
    format!(
        "reason-len={} ordinal={} category-len={} hint-len={}",
        runtime_ui_text_len(kick.reason_text.as_deref()),
        optional_i32_label(kick.reason_ordinal),
        runtime_ui_text_len(kick.hint_category.as_deref()),
        runtime_ui_text_len(kick.hint_text.as_deref()),
    )
}

fn compose_runtime_loading_detail_panel_text(
    loading: &crate::panel_model::RuntimeLoadingPanelModel,
) -> String {
    format!(
        "ready={}@{} timeout={}/{}/{} kind={} idle={} resets={}/{}/{}/{} last-reset={} world={}",
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

fn compose_runtime_reconnect_detail_panel_text(
    reconnect: &crate::panel_model::RuntimeReconnectPanelModel,
) -> String {
    format!(
        "phase={} transitions={} reason-kind={} reason-len={} ordinal={} hint-len={} redirect={}@{}:{}",
        runtime_reconnect_phase_text(reconnect.phase),
        reconnect.phase_transition_count,
        runtime_reconnect_reason_kind_text(reconnect.reason_kind),
        runtime_ui_text_len(reconnect.reason_text.as_deref()),
        optional_i32_label(reconnect.reason_ordinal),
        runtime_ui_text_len(reconnect.hint_text.as_deref()),
        reconnect.redirect_count,
        compact_runtime_ui_text(reconnect.last_redirect_ip.as_deref()),
        optional_i32_label(reconnect.last_redirect_port),
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

fn compose_runtime_world_reload_detail_text(hud: &HudModel) -> Option<String> {
    let loading = build_runtime_loading_panel(hud)?;
    let world_reload = loading.last_world_reload.as_ref()?;
    Some(runtime_world_reload_detail_text(world_reload))
}

fn runtime_world_reload_detail_text(
    world_reload: &crate::panel_model::RuntimeWorldReloadPanelModel,
) -> String {
    format!(
        "loaded={} client={} ready={} confirm={} pending={} deferred={} replayed={}",
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
    )
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

    Some(summary.family_and_detail_text())
}

fn compose_overlay_detail_text(scene: &RenderModel) -> Option<String> {
    let summary = scene.semantic_summary();
    summary.detail_text()
}

fn compose_render_pipeline_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let summary = render_pipeline_summary(scene, window)?;
    let window = summary.window?;
    let span_text = summary
        .layer_span
        .map(|(min, max)| format!("{min}..{max}"))
        .unwrap_or_else(|| "none".to_string());
    let focus_text = summary
        .focus_tile
        .map(|(x, y)| format!("{x}:{y}"))
        .unwrap_or_else(|| "none".to_string());

    Some(format!(
        "total={} visible={} clipped={} layers={} span={} focus={} window={}:{}+{}x{} kinds={}",
        summary.total_object_count,
        summary.visible_object_count,
        summary.clipped_object_count,
        summary.layers.len(),
        span_text,
        focus_text,
        window.origin_x,
        window.origin_y,
        window.width,
        window.height,
        summary.visible_semantics.family_and_detail_text(),
    ))
}

fn compose_render_layer_lines(scene: &RenderModel, window: PresenterViewWindow) -> Vec<String> {
    let Some(summary) = render_pipeline_summary(scene, window) else {
        return Vec::new();
    };

    let layer_count = summary.layers.len();
    summary
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let mut text = format!(
                "{}/{} layer={} objects={} player={} marker={} plan={} block={} runtime={} terrain={} unknown={}",
                index + 1,
                layer_count,
                layer.layer,
                layer.object_count,
                layer.player_count,
                layer.marker_count,
                layer.plan_count,
                layer.block_count,
                layer.runtime_count,
                layer.terrain_count,
                layer.unknown_count,
            );
            if let Some(detail_text) = layer.detail_text() {
                text.push_str(" detail=");
                text.push_str(&detail_text);
            }
            text
        })
        .collect()
}

fn render_pipeline_summary(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<crate::render_model::RenderPipelineSummary> {
    if scene.objects.is_empty() {
        return None;
    }

    Some(scene.pipeline_summary_for_window(
        TILE_SIZE,
        crate::RenderViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width,
            height: window.height,
        },
    ))
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
    value
        .map(str::chars)
        .map(Iterator::count)
        .unwrap_or_default()
}

fn runtime_ui_uri_scheme(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .and_then(|uri| uri.split_once(':').map(|(scheme, _)| scheme.trim()))
        .filter(|scheme| !scheme.is_empty())
        .map(|scheme| compact_runtime_ui_text(Some(scheme)))
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_ui_notice_panel_is_empty(panel: &RuntimeUiNoticePanelModel) -> bool {
    panel.hud_set_count == 0
        && panel.hud_set_reliable_count == 0
        && panel.hud_hide_count == 0
        && panel.hud_last_message.is_none()
        && panel.hud_last_reliable_message.is_none()
        && panel.announce_count == 0
        && panel.last_announce_message.is_none()
        && panel.info_message_count == 0
        && panel.last_info_message.is_none()
        && panel.toast_info_count == 0
        && panel.toast_warning_count == 0
        && panel.toast_last_info_message.is_none()
        && panel.toast_last_warning_text.is_none()
        && panel.info_popup_count == 0
        && panel.info_popup_reliable_count == 0
        && panel.last_info_popup_reliable.is_none()
        && panel.last_info_popup_id.is_none()
        && panel.last_info_popup_message.is_none()
        && panel.last_info_popup_duration_bits.is_none()
        && panel.last_info_popup_align.is_none()
        && panel.last_info_popup_top.is_none()
        && panel.last_info_popup_left.is_none()
        && panel.last_info_popup_bottom.is_none()
        && panel.last_info_popup_right.is_none()
        && panel.clipboard_count == 0
        && panel.last_clipboard_text.is_none()
        && panel.open_uri_count == 0
        && panel.last_open_uri.is_none()
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

fn team_u8_sample_text(values: &[u8]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
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
        Some(crate::RuntimeLiveEffectPositionSource::ActiveOverlay) => "active",
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
    let unit_target = command_unit_ref_text(value.unit_target);
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

fn command_unit_ref_text(value: Option<crate::RuntimeCommandUnitRefObservability>) -> String {
    value
        .map(|unit| format!("{}:{}", unit.kind, unit.value))
        .unwrap_or_else(|| "none".to_string())
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
    use super::{
        ascii_line_end_object_pair, ascii_primitive_render_command, ascii_render_command,
        AsciiScenePresenter,
    };
    use crate::{
        hud_model::{
            HudSummary, RuntimeBootstrapObservability, RuntimeReconnectObservability,
            RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind,
            RuntimeResourceDeltaObservability, RuntimeSessionObservability,
            RuntimeSessionResetKind, RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
        },
        panel_model::PresenterViewWindow,
        project_scene_models, project_scene_models_with_view_window,
        render_model::{RenderIconPrimitiveFamily, RenderObjectSemanticKind, RenderPrimitive},
        HudModel, RenderModel, RenderObject, RuntimeAdminObservability,
        RuntimeHudTextObservability, RuntimeMenuObservability, RuntimeRulesObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
        RuntimeWorldLabelObservability, ScenePresenter, Viewport,
    };
    use mdt_world::parse_world_bundle;
    use std::collections::BTreeMap;

    fn runtime_stack_test_scene() -> RenderModel {
        RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: Vec::new(),
        }
    }

    fn runtime_stack_test_hud(runtime_ui: RuntimeUiObservability) -> HudModel {
        HudModel {
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        }
    }

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
    fn ascii_presenter_rasterizes_marker_line_segments_into_visible_tiles() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:line:demo".to_string(),
                    layer: 1,
                    x: -8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:line:demo:line-end".to_string(),
                    layer: 1,
                    x: 16.0,
                    y: 0.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        assert_eq!(presenter.last_frame().lines().last(), Some("MMM."));
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
    fn ascii_presenter_applies_zoom_to_view_window_size() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 2.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:focus".to_string(),
                layer: 40,
                x: 32.0,
                y: 32.0,
            }],
        };
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::with_max_view_tiles(4, 4);

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        let grid_rows = frame.lines().rev().take(2).collect::<Vec<_>>();

        assert!(frame.contains("WINDOW: origin=(3, 3) size=2x2"));
        assert_eq!(grid_rows.len(), 2);
        assert!(grid_rows.iter().all(|row| row.len() == 2));
    }

    #[test]
    fn ascii_presenter_zoom_out_expands_view_window_up_to_map_bounds() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 0.5,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:focus".to_string(),
                layer: 40,
                x: 32.0,
                y: 32.0,
            }],
        };
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::with_max_view_tiles(4, 4);

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        let grid_rows = frame.lines().rev().take(8).collect::<Vec<_>>();

        assert!(!frame.contains("WINDOW: origin="));
        assert_eq!(grid_rows.len(), 8);
        assert!(grid_rows.iter().all(|row| row.len() == 8));
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
        assert_eq!(super::sprite_for_id("unit:1"), 'R');
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
        assert!(frame.contains(
            "OVERLAY-DETAIL: marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1"
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 2,
                        height: 2,
                    },
                },
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
        assert!(frame.contains(
            "MINIMAP-DETAIL: window-kinds: tracked=7 outside=0 player=1 marker=2 plan=0 block=0 runtime=4 terrain=0 unknown=0"
        ));
    }

    #[test]
    fn ascii_presenter_surfaces_render_pipeline_layer_summary_for_visible_window() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                crate::RenderObject {
                    id: "terrain:0".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                crate::RenderObject {
                    id: "marker:line:7".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "player:focus".to_string(),
                    layer: 40,
                    x: 8.0,
                    y: 8.0,
                },
                crate::RenderObject {
                    id: "plan:build:1:3:3:257".to_string(),
                    layer: 20,
                    x: 24.0,
                    y: 24.0,
                },
                crate::RenderObject {
                    id: "block:runtime-building:1:3:3".to_string(),
                    layer: 35,
                    x: 24.0,
                    y: 0.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::with_max_view_tiles(2, 2);

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "RENDER-PIPELINE: total=5 visible=3 clipped=2 layers=3 span=0..40 focus=1:1 window=0:0+2x2 kinds=players=1 markers=1 plans=0 blocks=0 runtime=0 terrain=1 unknown=0 detail=marker-line:1"
        ));
        assert!(frame.contains(
            "RENDER-LAYER: 1/3 layer=0 objects=1 player=0 marker=0 plan=0 block=0 runtime=0 terrain=1 unknown=0"
        ));
        assert!(frame.contains(
            "RENDER-LAYER: 2/3 layer=30 objects=1 player=0 marker=1 plan=0 block=0 runtime=0 terrain=0 unknown=0 detail=marker-line:1"
        ));
        assert!(frame.contains(
            "RENDER-LAYER: 3/3 layer=40 objects=1 player=1 marker=0 plan=0 block=0 runtime=0 terrain=0 unknown=0"
        ));
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
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability {
                    set_count: 9,
                    set_reliable_count: 10,
                    hide_count: 11,
                    last_message: Some("hud text".to_string()),
                    last_reliable_message: Some("hud rel".to_string()),
                    announce_count: 12,
                    last_announce_message: Some("announce".to_string()),
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
                    core_binding: crate::RuntimeCoreBindingObservability {
                        kind: Some(
                            crate::RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation,
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
                        last_unit: Some(crate::RuntimeCommandUnitRefObservability {
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
                        last_changed_build_pos: Some(999),
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
                    },
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
                        active_overlay_remaining_ticks: Some(3),
                        active_overlay_lifetime_ticks: Some(5),
                        last_effect_id: Some(8),
                        last_spawn_effect_unit_type_id: Some(19),
                        last_data_len: Some(9),
                        last_data_type_tag: Some(4),
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
            "MINIMAP-VIS: overlay=1 fog=1 known=144(3%) vis=120(83%/2%) hid=24(16%/0%) unseen=4656(97%) density=map:4/4800(0%) window:4/1(400%) offscreen:0/4(0%)"
        ));
        assert!(frame.contains(
            "VIS-MINIMAP: overlay=1 fog=1 known=144(3%) vis=120(83%/2%) hid=24(16%/0%) map=80x60 window=0:0->0:0 size=1x1 cover=1/4800(0%) focus=0:0 in-window=1"
        ));
        assert!(frame.contains(
            "MINIMAP-VIS-DETAIL: visibility=mixed coverage=offscreen density=map:0% window:400% offscreen:0%"
        ));
        assert!(frame.contains(
            "MINIMAP-KINDS: tracked=4 player=1 marker=1 plan=1 block=1 runtime=0 terrain=0 unknown=0"
        ));
        assert!(frame.contains(
            "MINIMAP-LEGEND: @=player M=marker P=plan #=block R=runtime overlay .=terrain ?=unknown"
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
        assert!(frame.contains("BUILD: sel=257 rot=2 building=1 cfg=2"));
        assert!(frame.contains("BUILD-QUEUE: queue=1/2/3/4/1 head=flight@100:99:place:b301:r1"));
        assert!(frame
            .contains("BUILD-INSPECTOR: family=message tracked=1 sample=18:40:len=5:text=hello"));
        assert!(frame.contains(
            "BUILD-INSPECTOR: family=power-node tracked=1 sample=23:45:links=24:46|25:47"
        ));
        assert!(frame.contains(
            "OVERLAY-KINDS: players=1 markers=1 plans=1 blocks=1 runtime=0 terrain=0 unknown=0"
        ));
        assert!(frame.contains("RUNTIME-UI: hud=9/10/11@hud_text/hud_rel"));
        assert!(frame.contains("ann=12@announce"));
        assert!(frame.contains("info=13@info"));
        assert!(frame.contains("toast=14/15@toast/warn"));
        assert!(frame.contains("popup=16/17"));
        assert!(frame.contains("choice=29/30"));
        assert!(frame.contains("tin=53@404:Digits/Only_numbers"));
        assert!(frame.contains(
            "RUNTIME-NOTICE: hud=9/10/11@hud_text/hud_rel ann=12@announce info=13@info toast=14/15@toast/warn popup=16/17@1:popup-a/popup_text clip=18@copied uri=19@https_//exam~:https tin=53@404:Digits/Only_numbers/12345#16:n1:e1"
        ));
        assert!(frame.contains(
            "RUNTIME-NOTICE-DETAIL: active=1 hud-events=9/10/11 hud-len=8/7 announce=12 len=8 info=13 len=4 toast-events=14/15 toast-len=5/4 popup=16/17 rel=1 id-len=7 msg-len=10 dur=1075838976 box=1:2/3/4/5 clip=18 len=6 uri=19 len=19 scheme=https text-input=53 id=404 title-len=6 msg-len=12 default-len=5 limit=16 numeric=1 allow-empty=1"
        ));
        assert!(frame
            .contains("RUNTIME-MENU: menu=16@40:main/pick#2:3 follow=17@41:follow/next#1:2 hide=18@41 tin=53@404:Digits/12345#16:n1:e1"));
        assert!(frame.contains(
            "RUNTIME-MENU-DETAIL: active=1 outstanding-follow-up=0 menu=40 title-len=4 message-len=4 rows=2/3 follow=41 title-len=6 message-len=4 rows=1/2 hide-id=41 text-input=53 id=404 title=Digits default-len=5 numeric=1 allow-empty=1"
        ));
        assert!(frame.contains("RUNTIME-CHOICE: menu-choose=29@404/2 tin-result=30@405/ok123"));
        assert!(frame.contains(
            "RUNTIME-CHOICE-DETAIL: choose-menu=404 choose-option=2 result-id=405 result-len=5"
        ));
        assert!(frame.contains(
            "RUNTIME-DIALOG: prompt=input act=1 menu=16/17/18 tin=53@404:Digits/Only_numbers/12345#16:n1:e1 notice=warn@warn total=48"
        ));
        assert!(frame.contains(
            "RUNTIME-DIALOG-DETAIL: prompt=input active=1 layers=menu:1/follow-up:0/input:53 message-len=12 default-len=5 notice=warn layers=hud:1/reliable:1/info:1/warn:1 notice-len=4"
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
            "RUNTIME-STACK-DEPTH: prompt=2 notice=4 chat=1 menu=2 hud=4 dialog=7 groups=3 total=7"
        ));
        assert!(frame.contains(
            "RUNTIME-STACK-DETAIL: dialog=front:input groups:3 total:7 prompt=input/menu:1/follow-up:0/input:53 notice=warn/hud:1/reliable:1/info:1/warn:1 chat=active:1/server:7/local:8 sender=404"
        ));
        assert!(frame.contains(
            "RUNTIME-DIALOG-STACK: front=input prompt=input@input>menu menu=16 follow-up=0 input=53 notice=warn@hud>reliable>info>warn depths=menu:2/hud:4/dialog:7 chat=1 server=7 local=8 tin=404 sender=404 total=7"
        ));
        assert!(frame.contains(
            "RUNTIME-COMMAND: act=1 sel=4@11,22,33 bld=2@327686 rect=-3:4:12:18 groups=2#3@11,4#1@99 target=b589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4 cmd=5 stance=7/0"
        ));
        assert!(frame.contains(
            "RUNTIME-COMMAND-DETAIL: sample=11,22,33 groups=2#3@11,4#1@99 first-building=327686 rect=-3:4:12:18 target=b589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4 command=5 stance=7/0"
        ));
        assert!(frame.contains("RUNTIME-ADMIN: trace=56@123456 fail=76 dbg=57/58@12 fail=231"));
        assert!(frame.contains(
            "RUNTIME-ADMIN-DETAIL: trace=56 fail=76 last-player=123456 debug=57 fail=77 unreliable=58 fail=78 last-value=12"
        ));
        assert!(frame.contains(
            "RUNTIME-RULES: mut=354 fail=210 set=67/69/71 clear=73 complete=74 state=wv1:pvp0 obj=2 qual=1 parents=1 flags=2 oor=75 last=9"
        ));
        assert!(frame.contains(
            "RUNTIME-RULES-DETAIL: set-rules=67 set-objectives=69 set-rule=71 clear-objectives=73 complete-objective=74"
        ));
        assert!(frame.contains(
            "RUNTIME-WORLD-LABEL: set=19 rel=20 remove=21 total=60 active=2 inactive=1 last=904 flags=3 font=1094713344@12.0 z=1082130432@4.0 pos=40.0:60.0 text=world label lines=1 len=11"
        ));
        assert!(frame.contains(
            "RUNTIME-WORLD-LABEL-DETAIL: set=19 rel=20 remove=21 active=2 inactive=1 last=904 flags=3 text-len=11 lines=1 font=1094713344@12.0 z=1082130432@4.0 pos=40.0:60.0"
        ));
        assert!(frame.contains(
            "RUNTIME-MARKER: create=54 remove=55 update=56 text=57 texture=58 fail=2 last=808 control=flushText"
        ));
        assert!(frame.contains(
            "RUNTIME-MARKER-DETAIL: total=280 mutate=165 text=57 texture=58 fail=2 last=808 control-len=9"
        ));
        assert!(frame.contains(
            "RUNTIME-SESSION: bootstrap=rules=rules-hash-1:tags=tags-hash-2:locales=locales-hash-3:teams=2:markers=3:chunks=4:patches=5:plans=6:fog=7; resource=tiles=80/81/82/83 set=22/23/24/25 clear=84/85 tile=26/27 flow=1/2/3 last=to_unit@6#none:bpnone:u2:808:eid404 proj=2/3/1 auth=4 delta=5/6/7 chg=999/900/6/1; kick=idInUse@7:IdInUse:wait_for_old~; loading=defer5 replay6 drop7 qdrop8 sfail9 scfail10 efail11 rdy12@1300 to2/1/1 ltready@20000 rs3/1/1/1 lrreload lwr@lw1:cl0:rd1:cc0:p4:d5:r6; reconnect=attempt#3 redirect redirect=1@127.0.0.1:6567 reason=connectRedir~#none hint=server_reque~"
        ));
        assert!(frame.contains(
            "RUNTIME-SESSION-DETAIL: bootstrap=[rules-label=rules-hash-1:tags-label=tags-hash-2:locales-label=locales-hash-3:team-count=2:marker-count=3:custom-chunk-count=4:content-patch-count=5:player-team-plan-count=6:static-fog-team-count=7] resource=[tile-rm=80 tile-set=81 floor-set=82 overlay-set=83 item-set=22/23 liquid-set=24/25 clear=84/85 tile-apply=26/27 flow=1/2/3 last-kind=to_unit item=6 amount=none build=none unit=2:808 to-entity=404 projection=2/3/1 authoritative=4 delta=5/6/7 changed=999/900/6/1] kick=[reason-len=7 ordinal=7 category-len=7 hint-len=20] loading=[ready=12@1300 timeout=2/1/1 kind=ready idle=20000 resets=3/1/1/1 last-reset=reload world=@lw1:cl0:rd1:cc0:p4:d5:r6] reconnect=[phase=attempt transitions=3 reason-kind=redirect reason-len=15 ordinal=none hint-len=25 redirect=1@127.0.0.1:6567]"
        ));
        assert!(frame.contains("RUNTIME-KICK: idInUse@7:IdInUse:wait_for_old~"));
        assert!(frame
            .contains("RUNTIME-KICK-DETAIL: reason-len=7 ordinal=7 category-len=7 hint-len=20"));
        assert!(frame.contains(
            "RUNTIME-LOADING: defer5 replay6 drop7 qdrop8 sfail9 scfail10 efail11 rdy12@1300 to2/1/1 ltready@20000 rs3/1/1/1 lrreload lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
        ));
        assert!(frame.contains(
            "RUNTIME-LOADING-DETAIL: ready=12@1300 timeout=2/1/1 kind=ready idle=20000 resets=3/1/1/1 last-reset=reload world=@lw1:cl0:rd1:cc0:p4:d5:r6"
        ));
        assert!(frame.contains(
            "RUNTIME-WORLD-RELOAD-DETAIL: loaded=1 client=0 ready=1 confirm=0 pending=4 deferred=5 replayed=6"
        ));
        assert!(frame
            .contains("RUNTIME-CORE-BINDING: kind=first-core-per-team ambiguous=1@1 missing=1@4"));
        assert!(frame.contains(
            "RUNTIME-CORE-BINDING-DETAIL: kind=first-core-per-team ambiguous-count=1 ambiguous-sample=1 missing-count=1 missing-sample=4"
        ));
        assert!(frame.contains(
            "RUNTIME-RECONNECT: attempt#3 redirect redirect=1@127.0.0.1:6567 reason=connectRedir~#none hint=server_reque~"
        ));
        assert!(frame.contains(
            "RUNTIME-RECONNECT-DETAIL: phase=attempt transitions=3 reason-kind=redirect reason-len=15 ordinal=none hint-len=25 redirect=1@127.0.0.1:6567"
        ));
        assert!(frame.contains(
            "RUNTIME-LIVE-ENTITY: 1/0@404:u2/999:p20.0:33.0:h0:s3:tp1/0:last404/404/none"
        ));
        assert!(frame.contains(
            "RUNTIME-LIVE-ENTITY-DETAIL: local=404 unit=2/999 pos=20.0:33.0 hidden=0 seen=3 players=1 units=0 last=404/404/none owned=202 payload=count=2:unit=5/r7/l12:s0123456789ab nested=2 stack=6x4 controller=4/101"
        ));
        assert!(frame.contains(
            "RUNTIME-LIVE-EFFECT: 11/73:ov1@13:u19:d9/4:kPoint2:clightning/lightning:r1:hpos:point2:3:4@1/0:pactive@28.0:36.0:ttl3/5"
        ));
        assert!(frame.contains(
            "RUNTIME-LIVE-EFFECT-DETAIL: hint=pos:point2:3:4@1/0 source=active pos=28.0:36.0 ttl=3/5 data=9/4 active-rel=1 contract=lightning reliable=lightning"
        ));
        assert!(frame.contains("live=ent=1/0@404:u2/999:p20.0:33.0:h0:s3"));
        assert!(frame.contains(
            "fx=11/73:ov1@13:u19:d9/4:kPoint2:clightning/lightning:r1:hpos:point2:3:4@1/0:pactive@28.0:36.0:ttl3/5"
        ));
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((1, 1)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
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
            "MINIMAP-VIS: overlay=0 fog=0 known=0(0%) vis=0(0%/0%) hid=0(0%/0%) unseen=4800(100%) density=map:3/4800(0%) window:3/4(75%) offscreen:0/3(0%)"
        ));
        assert!(frame.contains(
            "MINIMAP-FLOW: next=survey focus=inside pan=hold vis=unseen cover=offscreen target=player overlay-targets=0"
        ));
        assert!(frame.contains(
            "MINIMAP-KINDS: tracked=3 player=1 marker=0 plan=0 block=0 runtime=0 terrain=1 unknown=1"
        ));
        assert!(frame.contains(
            "MINIMAP-LEGEND: @=player M=marker P=plan #=block R=runtime overlay .=terrain ?=unknown"
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
            "BUILD-MINIMAP-AUX: mode=place select=head-aligned queue=mixed place-ready=1 cfg=4/8 top=gamma auth=rejected-missing-building head=10:12 auth-tile=10:12 src=tileConfig focus=1:1 in-window=1 visible-map=0 unknown-map=100 window=0 d75 tracked=3 runtime=0 runtime-share=0%"
        ));
        assert!(frame.contains(
            "BUILD-FLOW: next=resolve minimap=survey focus=inside pan=hold target=player scope=multi head=10:12 auth=rejected-missing-building"
        ));
        assert!(frame.contains(
            "BUILD-FLOW-SUMMARY: next=resolve minimap=survey focus=inside pan=hold target=player scope=multi"
        ));
        assert!(frame.contains(
            "BUILD-ROUTE: next=resolve minimap=survey blockers=2@resolve>survey route=3@resolve>survey>commit"
        ));
        assert!(frame.contains(
            "BUILD-FLOW-DETAIL: next=resolve minimap=survey focus=inside pan=hold target=player scope=multi blockers=resolve+survey route=resolve+survey+commit authority=rejected-missing-building head=10,12"
        ));
    }

    #[test]
    fn runtime_ui_uri_scheme_rejects_empty_and_colonless_values() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();

        for uri in ["", "noscheme", "://example.com"] {
            let mut runtime_ui = RuntimeUiObservability::default();
            runtime_ui.toast.last_open_uri = Some(uri.to_string());

            presenter.present(&scene, &runtime_stack_test_hud(runtime_ui));
            let frame = presenter.last_frame();

            assert!(frame.contains("RUNTIME-NOTICE-DETAIL:"));
            assert!(frame.contains("scheme=none"));
        }

        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.toast.last_open_uri = Some("https://example.com".to_string());

        presenter.present(&scene, &runtime_stack_test_hud(runtime_ui));
        let frame = presenter.last_frame();

        assert!(frame.contains("RUNTIME-NOTICE-DETAIL:"));
        assert!(frame.contains("scheme=https"));
    }

    #[test]
    fn runtime_ui_uri_scheme_trims_whitespace_around_the_uri() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.toast.last_open_uri = Some("  https://example.com  ".to_string());

        presenter.present(&scene, &runtime_stack_test_hud(runtime_ui));
        let frame = presenter.last_frame();

        assert!(frame.contains("RUNTIME-NOTICE-DETAIL:"));
        assert!(frame.contains("scheme=https"));
        assert!(!frame.contains("scheme=_https"));
    }

    #[test]
    fn runtime_ui_notice_panel_is_empty_rejects_single_active_field() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(
            &scene,
            &runtime_stack_test_hud(RuntimeUiObservability::default()),
        );
        assert!(!presenter.last_frame().contains("RUNTIME-NOTICE-DETAIL:"));

        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.toast.last_open_uri = Some("https://example.com".to_string());

        presenter.present(&scene, &runtime_stack_test_hud(runtime_ui));
        let frame = presenter.last_frame();

        assert!(frame.contains("RUNTIME-NOTICE-DETAIL:"));
        assert!(frame.contains("scheme=https"));
    }

    #[test]
    fn runtime_ui_text_len_counts_unicode_scalars_not_bytes() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.text_input.open_count = 1;
        runtime_ui.text_input.last_title = Some("é🙂a".to_string());

        presenter.present(&scene, &runtime_stack_test_hud(runtime_ui));

        let frame = presenter.last_frame();
        assert!(frame.contains("RUNTIME-NOTICE-DETAIL:"));
        assert!(frame.contains("title-len=3"));
        assert!(!frame.contains("title-len=7"));
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
        assert!(!presenter.last_frame().contains("RUNTIME-SESSION-DETAIL:"));
        assert!(!presenter.last_frame().contains("RUNTIME-NOTICE-DETAIL:"));
        assert!(!presenter.last_frame().contains("RUNTIME-COMMAND-DETAIL:"));
        assert!(!presenter.last_frame().contains("RUNTIME-DIALOG-STACK:"));
        assert!(!presenter.last_frame().contains("BUILD-FLOW-DETAIL:"));
        assert!(!presenter
            .last_frame()
            .contains("RUNTIME-WORLD-LABEL-DETAIL:"));
    }

    #[test]
    fn ascii_presenter_surfaces_runtime_stack_minimal_regression_cases() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();

        let mut chat_only = RuntimeUiObservability::default();
        chat_only.chat.server_message_count = 1;
        chat_only.chat.chat_message_count = 2;
        chat_only.chat.last_chat_sender_entity_id = Some(42);

        let mut menu_only = RuntimeUiObservability::default();
        menu_only.menu.menu_open_count = 1;

        let mut follow_up_only = RuntimeUiObservability::default();
        follow_up_only.menu.follow_up_menu_open_count = 1;

        let mut input_notice_chat = RuntimeUiObservability::default();
        input_notice_chat.text_input.open_count = 1;
        input_notice_chat.text_input.last_id = Some(404);
        input_notice_chat.toast.warning_count = 1;
        input_notice_chat.toast.last_warning_text = Some("warn".to_string());
        input_notice_chat.chat.server_message_count = 1;
        input_notice_chat.chat.chat_message_count = 1;
        input_notice_chat.chat.last_chat_sender_entity_id = Some(404);

        let cases = vec![
            (
                "chat-only",
                runtime_stack_test_hud(chat_only),
                "RUNTIME-STACK: front=chat prompt=0@none notice=none@none chat=1 groups=1 total=1 tin=none sender=42",
                "RUNTIME-STACK-DEPTH: prompt=0 notice=0 chat=1 menu=0 hud=0 dialog=1 groups=1 total=1",
                "RUNTIME-STACK-DETAIL: dialog=front:chat groups:1 total:1 prompt=none/menu:0/follow-up:0/input:0 notice=none/hud:0/reliable:0/info:0/warn:0 chat=active:1/server:1/local:2 sender=42",
            ),
            (
                "menu-only",
                runtime_stack_test_hud(menu_only),
                "RUNTIME-STACK: front=menu prompt=1@menu notice=none@none chat=0 groups=1 total=1 tin=none sender=none",
                "RUNTIME-STACK-DEPTH: prompt=1 notice=0 chat=0 menu=1 hud=0 dialog=1 groups=1 total=1",
                "RUNTIME-STACK-DETAIL: dialog=front:menu groups:1 total:1 prompt=menu/menu:1/follow-up:0/input:0 notice=none/hud:0/reliable:0/info:0/warn:0 chat=active:0/server:0/local:0 sender=none",
            ),
            (
                "follow-up-without-text-input",
                runtime_stack_test_hud(follow_up_only),
                "RUNTIME-STACK: front=follow-up prompt=1@follow-up notice=none@none chat=0 groups=1 total=1 tin=none sender=none",
                "RUNTIME-STACK-DEPTH: prompt=1 notice=0 chat=0 menu=1 hud=0 dialog=1 groups=1 total=1",
                "RUNTIME-STACK-DETAIL: dialog=front:follow-up groups:1 total:1 prompt=follow/menu:0/follow-up:1/input:0 notice=none/hud:0/reliable:0/info:0/warn:0 chat=active:0/server:0/local:0 sender=none",
            ),
            (
                "text-input+notice+chat",
                runtime_stack_test_hud(input_notice_chat),
                "RUNTIME-STACK: front=input prompt=1@input notice=warn@warn chat=1 groups=3 total=3 tin=404 sender=404",
                "RUNTIME-STACK-DEPTH: prompt=1 notice=1 chat=1 menu=1 hud=1 dialog=3 groups=3 total=3",
                "RUNTIME-STACK-DETAIL: dialog=front:input groups:3 total:3 prompt=input/menu:0/follow-up:0/input:1 notice=warn/hud:0/reliable:0/info:0/warn:1 chat=active:1/server:1/local:1 sender=404",
            ),
        ];

        for (name, hud, stack_line, depth_line, detail_line) in cases {
            presenter.present(&scene, &hud);
            let frame = presenter.last_frame();
            assert!(
                frame.contains(stack_line),
                "missing runtime stack line for {name} in {frame}",
            );
            assert!(
                frame.contains(depth_line),
                "missing runtime stack depth line for {name} in {frame}",
            );
            assert!(
                frame.contains(detail_line),
                "missing runtime stack detail line for {name} in {frame}",
            );
        }
    }

    #[test]
    fn ascii_presenter_drops_completed_prompt_history_from_stack_foreground() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();
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
        runtime_ui.chat.chat_message_count = 2;
        runtime_ui.chat.last_chat_sender_entity_id = Some(42);

        presenter.present(&scene, &runtime_stack_test_hud(runtime_ui));

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "RUNTIME-STACK: front=chat prompt=0@none notice=none@none chat=1 groups=1 total=1 tin=404 sender=42"
        ));
        assert!(frame.contains(
            "RUNTIME-STACK-DEPTH: prompt=0 notice=0 chat=1 menu=0 hud=0 dialog=1 groups=1 total=1"
        ));
        assert!(frame.contains(
            "RUNTIME-STACK-DETAIL: dialog=front:chat groups:1 total:1 prompt=none/menu:0/follow-up:0/input:1 notice=none/hud:0/reliable:0/info:0/warn:0 chat=active:1/server:1/local:2 sender=42"
        ));
        assert!(frame.contains(
            "RUNTIME-DIALOG-STACK: front=chat prompt=none@none menu=1 follow-up=0 input=1 notice=none@none depths=menu:0/hud:0/dialog:1 chat=1 server=1 local=2 tin=404 sender=42 total=1"
        ));
    }

    #[test]
    fn ascii_presenter_surfaces_runtime_prompt_rows() {
        let scene = runtime_stack_test_scene();
        let mut presenter = AsciiScenePresenter::default();
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.text_input.open_count = 53;
        runtime_ui.text_input.last_id = Some(404);
        runtime_ui.text_input.last_title = Some("Digits".to_string());
        runtime_ui.text_input.last_message = Some("Only numbers".to_string());
        runtime_ui.text_input.last_default_text = Some("12345".to_string());
        runtime_ui.text_input.last_length = Some(16);
        runtime_ui.text_input.last_numeric = Some(true);
        runtime_ui.text_input.last_allow_empty = Some(true);
        runtime_ui.menu.menu_open_count = 16;
        runtime_ui.menu.follow_up_menu_open_count = 17;
        runtime_ui.menu.hide_follow_up_menu_count = 15;

        let mut presenter_hud = runtime_stack_test_hud(runtime_ui);
        presenter_hud.title = "prompt".to_string();

        presenter.present(&scene, &presenter_hud);

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "RUNTIME-PROMPT: kind=input active=1 depth=3 layers=input>follow-up>menu menu=16 follow-up=2 tin=53@404:Digits/Only_numbers/12345#16:n1:e1"
        ));
        assert!(frame.contains(
            "RUNTIME-PROMPT-DETAIL: menu-active=1 follow-up-open=17 follow-up-hide=15 outstanding-follow-up=2 text-input=53 id=404 title-len=6 message-len=12 default-len=5 numeric=1 allow-empty=1"
        ));
    }

    #[test]
    fn ascii_presenter_renders_text_primitives_into_grid() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "world-label:7:text:48656c6c6f".to_string(),
                    layer: 39,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:text:8:text:4d61726b6572".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 8.0,
                },
            ],
        };
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("Hello"));
        assert!(frame.contains("Marker"));
    }

    #[test]
    fn ascii_presenter_ignores_non_finite_coordinates_in_ascii_filters() {
        let window = PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: 4,
            height: 4,
        };
        let line = RenderObject {
            id: "marker:line:demo".to_string(),
            layer: 1,
            x: f32::NAN,
            y: 0.0,
        };
        let line_end = RenderObject {
            id: "marker:line:demo:line-end".to_string(),
            layer: 1,
            x: 8.0,
            y: 0.0,
        };
        let line_objects = vec![line.clone(), line_end.clone()];
        let line_end_objects = line_objects
            .iter()
            .filter_map(ascii_line_end_object_pair)
            .collect::<BTreeMap<_, _>>();
        assert!(ascii_render_command(&line, &line_end_objects, window).is_none());

        let text_primitive = RenderPrimitive::Text {
            id: "world-label:7:text:48656c6c6f".to_string(),
            kind: RenderObjectSemanticKind::RuntimeWorldLabel,
            layer: 1,
            x: f32::INFINITY,
            y: 0.0,
            text: "Hello".to_string(),
        };
        assert!(ascii_primitive_render_command(&text_primitive, window).is_none());

        let rect_primitive = RenderPrimitive::Rect {
            id: "marker:line:runtime-command-rect".to_string(),
            family: "runtime-command-rect".to_string(),
            layer: 1,
            left: f32::NAN,
            top: f32::NAN,
            right: f32::NAN,
            bottom: f32::NAN,
            line_ids: vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ],
        };
        assert!(ascii_primitive_render_command(&rect_primitive, window).is_none());

        let icon_primitive = RenderPrimitive::Icon {
            id: "marker:runtime-health:1:2".to_string(),
            family: RenderIconPrimitiveFamily::RuntimeHealth,
            variant: "health".to_string(),
            layer: 1,
            x: f32::NAN,
            y: 8.0,
        };
        assert!(ascii_primitive_render_command(&icon_primitive, window).is_none());

        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-health:1:2".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:top:1:1:2:1".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:top:1:1:2:1:line-end".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:right:2:1:2:2".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:right:2:1:2:2:line-end".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:bottom:2:2:1:2".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:bottom:2:2:1:2:line-end".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:left:1:2:1:1".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
                RenderObject {
                    id: "marker:line:runtime-command-rect:left:1:2:1:1:line-end".to_string(),
                    layer: 1,
                    x: f32::NAN,
                    y: f32::NAN,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();
        presenter.present(&scene, &HudModel::default());
        let frame = presenter.last_frame();
        assert!(!frame.contains("RENDER-ICON:"));
        assert!(!frame.contains("RENDER-RECT:"));
    }

    #[test]
    fn ascii_presenter_surfaces_rect_primitive_summary_for_runtime_command_rects() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:top:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:top:{}:{}:{}:{}:line-end",
                        8.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 16.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:right:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 16.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:right:{}:{}:{}:{}:line-end",
                        24.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:bottom:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:bottom:{}:{}:{}:{}:line-end",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:left:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:left:{}:{}:{}:{}:line-end",
                        8.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 16.0,
                },
            ],
        };
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-RECT: count=1 runtime-command-rect@29:8:16:24:32"));
        assert!(frame.contains(
            "RENDER-RECT-DETAIL: count=1 | runtime-command-rect@29:8:16:24:32 payload[left_tile=1,top_tile=2,right_tile=3,bottom_tile=4,width_tiles=2,height_tiles=2,line_count=4]"
        ));
    }

    #[test]
    fn ascii_presenter_surfaces_rect_primitive_summary_for_runtime_break_rects() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}:line-end",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}",
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}:line-end",
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}",
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}:line-end",
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}:line-end",
                        32.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 40.0,
                },
            ],
        };
        let hud = HudModel::default();
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-RECT: count=1 runtime-break-rect@30:32:40:40:48"));
        assert!(frame.contains(
            "RENDER-RECT-DETAIL: count=1 | runtime-break-rect@30:32:40:40:48 payload[left_tile=4,top_tile=5,right_tile=5,bottom_tile=6,width_tiles=1,height_tiles=1,line_count=4]"
        ));
    }

    #[test]
    fn ascii_presenter_reports_runtime_icon_primitives_without_generic_point_fallback() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-effect-icon:content-icon:normal:-1:6:9:0x00000000:0x00000000"
                        .to_string(),
                    layer: 31,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-build-config-icon:payload-source:1:0:1:7".to_string(),
                    layer: 32,
                    x: 8.0,
                    y: 0.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "RENDER-ICON: count=2 runtime-effect-icon/content-icon@31:0:0 runtime-build-config-icon/payload-source@32:1:0"
        ));
        assert!(frame.contains(
            "RENDER-ICON-DETAIL: count=2 | runtime-effect-icon/content-icon@31:0:0 payload[content_id=9,content_type=6,delivery=normal,effect_id=-1,variant=content-icon,x_bits=0x00000000,y_bits=0x00000000] | runtime-build-config-icon/payload-source@32:1:0 payload[content_id=7,content_type=1,tile_x=1,tile_y=0,variant=payload-source]"
        ));
        assert_eq!(frame.lines().last(), Some("EC"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_health_icon_primitive() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-health:0:0".to_string(),
                layer: 32,
                x: 0.0,
                y: 0.0,
            }],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-ICON: count=1 runtime-health/health@32:0:0"));
        assert_eq!(frame.lines().last(), Some("H"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_command_icon_primitive() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-command-building:0:0".to_string(),
                layer: 29,
                x: 0.0,
                y: 0.0,
            }],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-ICON: count=1 runtime-command/building@29:0:0"));
        assert_eq!(frame.lines().last(), Some("T"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_command_selected_unit_icon_primitive() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-command-selected-unit:22".to_string(),
                layer: 29,
                x: 0.0,
                y: 0.0,
            }],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-ICON: count=1 runtime-command/selected-unit@29:0:0"));
        assert_eq!(frame.lines().last(), Some("T"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_place_icon_primitive() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "plan:runtime-place:0:8:9".to_string(),
                layer: 21,
                x: 0.0,
                y: 0.0,
            }],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-ICON: count=1 runtime-place/place@21:0:0"));
        assert_eq!(frame.lines().last(), Some("P"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_effect_marker_icon_primitive() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-effect:normal:13:0x41000000:0x41800000:1".to_string(),
                layer: 26,
                x: 0.0,
                y: 0.0,
            }],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-ICON: count=1 runtime-effect/normal@26:0:0"));
        assert!(frame.contains(
            "RENDER-ICON-DETAIL: count=1 | runtime-effect/normal@26:0:0 payload[delivery=normal,effect_id=13,has_data=1,variant=normal,x_bits=0x41000000,y_bits=0x41800000]"
        ));
        assert_eq!(frame.lines().last(), Some("F"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_unit_assembler_icon_primitives() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:0:0x40800000".to_string(),
                    layer: 16,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-unit-assembler-command:tank-assembler:30:40:0x42200000:0x42700000".to_string(),
                    layer: 16,
                    x: 8.0,
                    y: 0.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "RENDER-ICON: count=2 runtime-unit-assembler-progress/tank-assembler@16:0:0 runtime-unit-assembler-command/tank-assembler@16:1:0"
        ));
        assert!(frame.contains(
            "RENDER-ICON-DETAIL: count=2 | runtime-unit-assembler-command/tank-assembler@16:1:0 payload[tile_x=30,tile_y=40,variant=tank-assembler,x_bits=0x42200000,y_bits=0x42700000] | runtime-unit-assembler-progress/tank-assembler@16:0:0 payload[block_count=4,pay_rotation_bits=0x40800000,payload_present=0,progress_bits=0x3f400000,sample_id=9,sample_kind=b,sample_present=1,tile_x=30,tile_y=40,unit_count=2,variant=tank-assembler]"
        ));
        assert_eq!(frame.lines().last(), Some("AA"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_world_event_icon_primitives() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 40.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-break:0:3:4".to_string(),
                    layer: 14,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-bullet:1:17:4".to_string(),
                    layer: 28,
                    x: 8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-logic-explosion:2:2:0x42800000:1:1:0:1".to_string(),
                    layer: 28,
                    x: 16.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-sound-at:3:11".to_string(),
                    layer: 28,
                    x: 24.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-auto-door-toggle:4:3:4:1".to_string(),
                    layer: 28,
                    x: 32.0,
                    y: 0.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains(
            "RENDER-ICON: count=5 runtime-break/break@14:0:0 runtime-bullet/bullet@28:1:0"
        ));
        assert_eq!(frame.lines().last(), Some("XBLSW"));
    }

    #[test]
    fn ascii_presenter_reports_runtime_config_icon_primitives_without_generic_point_fallback() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 40.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-config:0:0:string".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-parse-fail:1:0:int".to_string(),
                    layer: 31,
                    x: 8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-noapply:2:0:content".to_string(),
                    layer: 32,
                    x: 16.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-rollback:3:0:unit".to_string(),
                    layer: 33,
                    x: 24.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-pending-mismatch:4:0:payload".to_string(),
                    layer: 34,
                    x: 32.0,
                    y: 0.0,
                },
            ],
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &HudModel::default());

        let frame = presenter.last_frame();
        assert!(frame.contains("RENDER-ICON: count=5"));
        assert!(frame.contains("runtime-config/string@30:0:0"));
        assert!(frame.contains("runtime-config-parse-fail/int@31:1:0"));
        assert!(frame.contains("RENDER-ICON-DETAIL: count=5"));
        assert!(frame
            .contains("runtime-config/string@30:0:0 payload[tile_x=0,tile_y=0,variant=string]"));
        assert!(frame.contains(
            "runtime-config-pending-mismatch/payload@34:4:0 payload[tile_x=4,tile_y=0,variant=payload]"
        ));
        assert_eq!(frame.lines().last(), Some("CCCCC"));
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
