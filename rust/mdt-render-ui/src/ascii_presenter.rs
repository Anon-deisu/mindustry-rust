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
        if let Some(overlay_semantics_text) = compose_overlay_semantics_text(scene) {
            out.push_str(&format!("OVERLAY-KINDS: {overlay_semantics_text}\n"));
        }
        if let Some(runtime_ui_text) = compose_runtime_ui_text(hud) {
            out.push_str(&format!("RUNTIME-UI: {runtime_ui_text}\n"));
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
    Some(format!(
        "hud={}/{}/{}@{}/{} toast={}/{}@{}/{} tin={}@{}:{}/{}/{}#{}:n{}:e{}",
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
    ))
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
        RuntimeHudTextObservability, RuntimeTextInputObservability,
        RuntimeToastObservability, RuntimeUiObservability, ScenePresenter, Viewport,
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
            }),
        };
        let mut presenter = AsciiScenePresenter::default();

        presenter.present(&scene, &hud);

        let frame = presenter.last_frame();
        assert!(frame.contains("SUMMARY: player=operator team=2 selected=payload-rout~"));
        assert!(frame.contains("map=80x60 overlay=1 fog=1 vis=120 hid=24"));
        assert!(frame.contains(
            "OVERLAY-KINDS: players=1 markers=1 plans=1 blocks=1 runtime=0 terrain=0 unknown=0"
        ));
        assert!(frame.contains("RUNTIME-UI: hud=9/10/11@hud_text/hud_rel"));
        assert!(frame.contains("toast=14/15@toast/warn"));
        assert!(frame.contains("tin=53@404:Digits/Only_numbers"));
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
