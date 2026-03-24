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

#[cfg(test)]
mod tests {
    use super::AsciiScenePresenter;
    use crate::{project_scene_models, RenderModel, RenderObject, ScenePresenter, Viewport};
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
        let hud = crate::HudModel::default();
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
        let hud = crate::HudModel::default();
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
