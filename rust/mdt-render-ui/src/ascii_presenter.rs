use crate::panel_model::{
    build_build_config_panel, build_minimap_panel, build_runtime_menu_panel,
    build_runtime_rules_panel, build_runtime_ui_notice_panel, build_runtime_world_label_panel,
    PresenterViewWindow,
};
use crate::render_model::RenderObjectSemanticKind;
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
        let (window_x, window_y, window_width, window_height) =
            crop_window(scene, width, height, self.max_view_tiles);
        let mut grid = vec![vec![' '; window_width]; window_height];
        let mut objects = scene
            .objects
            .iter()
            .filter_map(|object| {
                visible_window_tile(object, window_x, window_y, window_width, window_height)
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
        if let Some(minimap_text) = compose_minimap_panel_text(
            scene,
            hud,
            PresenterViewWindow {
                origin_x: window_x,
                origin_y: window_y,
                width: window_width,
                height: window_height,
            },
        ) {
            out.push_str(&format!("MINIMAP: {minimap_text}\n"));
        }
        if let Some(minimap_kinds_text) = compose_minimap_kind_line(scene, hud) {
            out.push_str(&format!("MINIMAP-KINDS: {minimap_kinds_text}\n"));
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
        if let Some(runtime_menu_text) = compose_runtime_menu_panel_text(hud) {
            out.push_str(&format!("RUNTIME-MENU: {runtime_menu_text}\n"));
        }
        if let Some(runtime_rules_text) = compose_runtime_rules_panel_text(hud) {
            out.push_str(&format!("RUNTIME-RULES: {runtime_rules_text}\n"));
        }
        if let Some(runtime_world_label_text) = compose_runtime_world_label_panel_text(hud) {
            out.push_str(&format!(
                "RUNTIME-WORLD-LABEL: {runtime_world_label_text}\n"
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
        if window_width != width || window_height != height {
            out.push_str(&format!(
                "WINDOW: origin=({}, {}) size={}x{}\n",
                window_x, window_y, window_width, window_height
            ));
        }
        for y in (0..window_height).rev() {
            for x in 0..window_width {
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
) -> (usize, usize, usize, usize) {
    let Some((max_width, max_height)) = max_view_tiles else {
        return (0, 0, width, height);
    };
    if width <= max_width && height <= max_height {
        return (0, 0, width, height);
    }

    let focus = scene
        .objects
        .iter()
        .find(|object| object.semantic_kind() == RenderObjectSemanticKind::Player)
        .map(|object| {
            (
                world_to_tile_index_clamped(object.x, width),
                world_to_tile_index_clamped(object.y, height),
            )
        })
        .unwrap_or((width / 2, height / 2));

    let window_width = max_width.min(width);
    let window_height = max_height.min(height);
    let window_x = crop_origin(focus.0, width, window_width);
    let window_y = crop_origin(focus.1, height, window_height);
    (window_x, window_y, window_width, window_height)
}

fn crop_origin(focus: usize, bound: usize, window: usize) -> usize {
    let half = window / 2;
    focus.saturating_sub(half).min(bound.saturating_sub(window))
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

fn world_to_tile_index_clamped(world_position: f32, bound: usize) -> usize {
    if bound == 0 {
        return 0;
    }
    world_to_tile_index_floor(world_position).clamp(0, bound.saturating_sub(1) as i32) as usize
}

fn sprite_for_id(id: &str) -> char {
    match RenderObjectSemanticKind::from_id(id) {
        RenderObjectSemanticKind::Player => '@',
        RenderObjectSemanticKind::Runtime => 'R',
        RenderObjectSemanticKind::Marker => 'M',
        RenderObjectSemanticKind::Plan => 'P',
        RenderObjectSemanticKind::Block => '#',
        RenderObjectSemanticKind::Terrain => '.',
        RenderObjectSemanticKind::Unknown => '?',
    }
}

fn compose_hud_summary_text(hud: &HudModel) -> Option<String> {
    let summary = hud.summary.as_ref()?;
    Some(format!(
        "player={} team={} selected={} plans={} markers={} map={}x{} overlay={} fog={} vis={} hid={}",
        compact_runtime_ui_text(Some(summary.player_name.as_str())),
        summary.team_id,
        compact_runtime_ui_text(Some(summary.selected_block.as_str())),
        summary.plan_count,
        summary.marker_count,
        summary.map_width,
        summary.map_height,
        if summary.overlay_visible { 1 } else { 0 },
        if summary.fog_enabled { 1 } else { 0 },
        summary.visible_tile_count,
        summary.hidden_tile_count,
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

fn compose_runtime_world_label_panel_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_world_label_panel(hud)?;
    Some(format!(
        "set={} rel={} remove={} total={}",
        panel.label_count, panel.reliable_label_count, panel.remove_label_count, panel.total_count,
    ))
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
        "map={}x{} rect={}:{}->{}:{} focus={} tiles={}/{} known={} vis={}({}%) hid={}({}%) overlay={} fog={} objs={}",
        panel.map_width,
        panel.map_height,
        panel.window.origin_x,
        panel.window.origin_y,
        panel.window_last_x,
        panel.window_last_y,
        optional_focus_tile_text(panel.focus_tile),
        panel.window_tile_count,
        panel.map_tile_count,
        panel.known_tile_count,
        panel.visible_tile_count,
        percent_text(panel.visible_tile_count, panel.known_tile_count),
        panel.hidden_tile_count,
        percent_text(panel.hidden_tile_count, panel.known_tile_count),
        if panel.overlay_visible { 1 } else { 0 },
        if panel.fog_enabled { 1 } else { 0 },
        panel.tracked_object_count,
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
    Some(format!(
        "player={} marker={} plan={} block={} runtime={} terrain={} unknown={}",
        panel.player_count,
        panel.marker_count,
        panel.plan_count,
        panel.block_count,
        panel.runtime_count,
        panel.terrain_count,
        panel.unknown_count,
    ))
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

fn optional_focus_tile_text(value: Option<(usize, usize)>) -> String {
    match value {
        Some((x, y)) => format!("{x}:{y}"),
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

fn percent_text(part: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        part.saturating_mul(100) / total
    }
}

fn compose_live_entity_text(entity: &crate::RuntimeLiveEntitySummaryObservability) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_text(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
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

fn compose_overlay_semantics_text(scene: &RenderModel) -> Option<String> {
    let counts = overlay_semantic_counts(scene);
    let total = counts.iter().map(|(_, count)| count).sum::<usize>();
    if total == 0 {
        return None;
    }

    Some(format!(
        "players={} markers={} plans={} blocks={} runtime={} terrain={} unknown={}",
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Player),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Marker),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Plan),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Block),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Runtime),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Terrain),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Unknown),
    ))
}

fn overlay_semantic_counts(scene: &RenderModel) -> Vec<(RenderObjectSemanticKind, usize)> {
    let mut counts = Vec::with_capacity(6);
    for object in &scene.objects {
        let kind = object.semantic_kind();
        if let Some((_, count)) = counts.iter_mut().find(|(existing, _)| *existing == kind) {
            *count += 1;
        } else {
            counts.push((kind, 1));
        }
    }
    counts
}

fn overlay_semantic_count(
    counts: &[(RenderObjectSemanticKind, usize)],
    kind: RenderObjectSemanticKind,
) -> usize {
    counts
        .iter()
        .find(|(existing, _)| *existing == kind)
        .map(|(_, count)| *count)
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::AsciiScenePresenter;
    use crate::{
        hud_model::HudSummary, project_scene_models, HudModel, RenderModel, RenderObject,
        RuntimeHudTextObservability, RuntimeMenuObservability, RuntimeRulesObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
        RuntimeWorldLabelObservability, ScenePresenter, Viewport,
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
    fn ascii_presenter_uses_alias_semantic_mapping_for_focus_and_sprites() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
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
        assert_eq!(super::sprite_for_id("marker:runtime-health:1:2"), 'R');
        assert_eq!(super::sprite_for_id("hint:1"), 'M');
        assert_eq!(super::sprite_for_id("build-plan:1"), 'P');
        assert_eq!(super::sprite_for_id("building:1:2"), '#');
        assert_eq!(super::sprite_for_id("tile:1"), '.');
    }

    #[test]
    fn ascii_presenter_emits_structured_summary_and_runtime_ui_lines() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
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
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
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
                },
                live: crate::RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability {
                        entity_count: 1,
                        hidden_count: 0,
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
        assert!(frame.contains("map=80x60 overlay=1 fog=1 vis=120 hid=24"));
        assert!(frame.contains(
            "MINIMAP: map=80x60 rect=0:0->0:0 focus=0:0 tiles=1/4800 known=144 vis=120(83%) hid=24(16%) overlay=1 fog=1 objs=4"
        ));
        assert!(frame.contains(
            "MINIMAP-KINDS: player=1 marker=1 plan=1 block=1 runtime=0 terrain=0 unknown=0"
        ));
        assert!(frame.contains(
            "BUILD-CONFIG: sel=257 rot=2 mode=build pending=1/2 hist=3/4 orphan=1 head=flight@100:99:place:b301:r1 align=split families=2/2 tracked=2"
        ));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 1/2 message#1@18:40:len=5:text=hello"));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 2/2 power-node#1@23:45:links=24:46|25:47"));
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
            "RUNTIME-MENU: menu=16 fmenu=17 hide=18 tin=53@404:Digits/12345#16:n1:e1"
        ));
        assert!(frame.contains(
            "RUNTIME-RULES: mut=354 fail=210 set=67/69/71 clear=73 complete=74 state=wv1:pvp0 obj=2 qual=1 parents=1 flags=2 oor=75 last=9"
        ));
        assert!(frame.contains("RUNTIME-WORLD-LABEL: set=19 rel=20 remove=21 total=60"));
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
            "MINIMAP: map=80x60 rect=0:0->1:1 focus=1:1 tiles=4/4800 known=0 vis=0(0%) hid=0(0%) overlay=0 fog=0 objs=3"
        ));
        assert!(frame.contains(
            "MINIMAP-KINDS: player=1 marker=0 plan=0 block=0 runtime=0 terrain=1 unknown=1"
        ));
        assert!(frame.contains(
            "BUILD-CONFIG: sel=301 rot=1 mode=build pending=2/1 hist=4/5 orphan=6 head=queued@10:12:place:b301:r1 align=match families=3/4 tracked=8"
        ));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 1/4 gamma#4@four"));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 2/4 beta#2@two"));
        assert!(frame.contains("BUILD-CONFIG-ENTRY: 3/4 alpha#1@one"));
        assert!(frame.contains("BUILD-CONFIG-MORE: +1 hidden families beyond cap"));
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
