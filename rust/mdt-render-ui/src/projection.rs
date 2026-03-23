use crate::{hud_model::HudSummary, HudModel, RenderModel, RenderObject, Viewport};
use mdt_world::{LoadedWorldSession, MarkerEntry, MarkerModel, PointMarkerModel, TeamPlanRef};

const TILE_SIZE: f32 = 8.0;
const TERRAIN_LAYER: i32 = 0;
const BLOCK_LAYER: i32 = 10;
const PLAN_LAYER: i32 = 20;
const MARKER_LAYER: i32 = 30;
const PLAYER_LAYER: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneVisibility {
    hud_visible: bool,
    overlay_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FogVisibility {
    enabled: bool,
    team_id: u8,
}

pub fn project_scene_models(
    session: &LoadedWorldSession<'_>,
    locale: &str,
) -> (RenderModel, HudModel) {
    project_scene_models_with_player_position(session, locale, None)
}

pub fn project_scene_models_with_player_position(
    session: &LoadedWorldSession<'_>,
    locale: &str,
    player_position: Option<(f32, f32)>,
) -> (RenderModel, HudModel) {
    let visibility = scene_visibility(session, locale);
    (
        project_render_model_with_player_position_visibility(
            session,
            player_position,
            visibility.overlay_visible,
        ),
        project_hud_model_with_visibility(session, locale, visibility),
    )
}

pub fn project_scene_models_with_view_window(
    session: &LoadedWorldSession<'_>,
    locale: &str,
    player_position: Option<(f32, f32)>,
    max_view_tiles: (usize, usize),
) -> (RenderModel, HudModel) {
    let visibility = scene_visibility(session, locale);
    (
        project_render_model_with_view_window_visibility(
            session,
            player_position,
            max_view_tiles,
            visibility.overlay_visible,
        ),
        project_hud_model_with_visibility(session, locale, visibility),
    )
}

pub fn project_render_model(session: &LoadedWorldSession<'_>) -> RenderModel {
    project_render_model_with_player_position(session, None)
}

pub fn project_render_model_with_player_position(
    session: &LoadedWorldSession<'_>,
    player_position: Option<(f32, f32)>,
) -> RenderModel {
    project_render_model_with_player_position_visibility(session, player_position, true)
}

fn project_render_model_with_player_position_visibility(
    session: &LoadedWorldSession<'_>,
    player_position: Option<(f32, f32)>,
    overlay_visible: bool,
) -> RenderModel {
    let graph = session.graph();
    let grid = graph.grid();
    let fog_visibility = fog_visibility(session);
    let mut objects = Vec::with_capacity(grid.tile_count() * 2 + 8);

    for tile in grid.iter_tiles() {
        if !tile_visible_under_fog(session, fog_visibility, tile.x as usize, tile.y as usize) {
            continue;
        }
        let world_x = tile.x as f32 * TILE_SIZE;
        let world_y = tile.y as f32 * TILE_SIZE;
        objects.push(RenderObject {
            id: format!("terrain:{}", tile.tile_index),
            layer: TERRAIN_LAYER,
            x: world_x,
            y: world_y,
        });

        if tile.block_id != 0 {
            objects.push(RenderObject {
                id: format!("block:{}:{}", tile.tile_index, tile.block_id),
                layer: BLOCK_LAYER,
                x: world_x,
                y: world_y,
            });
        }
    }

    if overlay_visible {
        for plan in session.player_team_plans() {
            if !tile_visible_under_fog(
                session,
                fog_visibility,
                plan.plan.x as usize,
                plan.plan.y as usize,
            ) {
                continue;
            }
            objects.push(project_team_plan(plan));
        }

        for marker in graph.markers() {
            if let Some((x, y)) = marker_world_position(marker) {
                let marker_tile_x = world_to_tile_index_clamped(x, graph.width());
                let marker_tile_y = world_to_tile_index_clamped(y, graph.height());
                if !tile_visible_under_fog(session, fog_visibility, marker_tile_x, marker_tile_y) {
                    continue;
                }
                objects.push(RenderObject {
                    id: format!("marker:{}", marker.id),
                    layer: MARKER_LAYER,
                    x,
                    y,
                });
            }
        }
    }

    let (player_x, player_y) = player_position.unwrap_or_else(|| session.state().player_position());
    objects.push(RenderObject {
        id: format!("player:{}", session.player().id),
        layer: PLAYER_LAYER,
        x: player_x,
        y: player_y,
    });

    RenderModel {
        viewport: Viewport {
            width: graph.width() as f32 * TILE_SIZE,
            height: graph.height() as f32 * TILE_SIZE,
            zoom: 1.0,
        },
        objects,
    }
}

pub fn project_render_model_with_view_window(
    session: &LoadedWorldSession<'_>,
    player_position: Option<(f32, f32)>,
    max_view_tiles: (usize, usize),
) -> RenderModel {
    project_render_model_with_view_window_visibility(session, player_position, max_view_tiles, true)
}

fn project_render_model_with_view_window_visibility(
    session: &LoadedWorldSession<'_>,
    player_position: Option<(f32, f32)>,
    max_view_tiles: (usize, usize),
    overlay_visible: bool,
) -> RenderModel {
    let graph = session.graph();
    let grid = graph.grid();
    let fog_visibility = fog_visibility(session);
    let (player_x, player_y) = player_position.unwrap_or_else(|| session.state().player_position());
    let (window_x, window_y, window_width, window_height) = view_window_bounds(
        graph.width(),
        graph.height(),
        (player_x, player_y),
        max_view_tiles,
    );
    let mut objects = Vec::with_capacity(window_width.saturating_mul(window_height) * 2 + 8);

    for y in window_y..window_y + window_height {
        for x in window_x..window_x + window_width {
            let Some(tile) = grid.tile(x, y) else {
                continue;
            };
            if !tile_visible_under_fog(session, fog_visibility, x, y) {
                continue;
            }
            let world_x = tile.x as f32 * TILE_SIZE;
            let world_y = tile.y as f32 * TILE_SIZE;
            objects.push(RenderObject {
                id: format!("terrain:{}", tile.tile_index),
                layer: TERRAIN_LAYER,
                x: world_x,
                y: world_y,
            });

            if tile.block_id != 0 {
                objects.push(RenderObject {
                    id: format!("block:{}:{}", tile.tile_index, tile.block_id),
                    layer: BLOCK_LAYER,
                    x: world_x,
                    y: world_y,
                });
            }
        }
    }

    if overlay_visible {
        for plan in session.player_team_plans() {
            if !tile_visible_under_fog(
                session,
                fog_visibility,
                plan.plan.x as usize,
                plan.plan.y as usize,
            ) {
                continue;
            }
            if tile_in_window(
                i32::from(plan.plan.x),
                i32::from(plan.plan.y),
                window_x,
                window_y,
                window_width,
                window_height,
            ) {
                objects.push(project_team_plan(plan));
            }
        }

        for marker in graph.markers() {
            if let Some((x, y)) = marker_world_position(marker) {
                let marker_tile_x = world_to_tile_index_clamped(x, graph.width());
                let marker_tile_y = world_to_tile_index_clamped(y, graph.height());
                if !tile_visible_under_fog(session, fog_visibility, marker_tile_x, marker_tile_y) {
                    continue;
                }
                if tile_in_window(
                    marker_tile_x as i32,
                    marker_tile_y as i32,
                    window_x,
                    window_y,
                    window_width,
                    window_height,
                ) {
                    objects.push(RenderObject {
                        id: format!("marker:{}", marker.id),
                        layer: MARKER_LAYER,
                        x,
                        y,
                    });
                }
            }
        }
    }

    objects.push(RenderObject {
        id: format!("player:{}", session.player().id),
        layer: PLAYER_LAYER,
        x: player_x,
        y: player_y,
    });

    RenderModel {
        viewport: Viewport {
            width: graph.width() as f32 * TILE_SIZE,
            height: graph.height() as f32 * TILE_SIZE,
            zoom: 1.0,
        },
        objects,
    }
}

pub fn project_hud_model(session: &LoadedWorldSession<'_>, locale: &str) -> HudModel {
    let visibility = scene_visibility(session, locale);
    project_hud_model_with_visibility(session, locale, visibility)
}

fn project_hud_model_with_visibility(
    session: &LoadedWorldSession<'_>,
    locale: &str,
    visibility: SceneVisibility,
) -> HudModel {
    if !visibility.hud_visible {
        return HudModel::hidden();
    }

    let graph = session.graph();
    let fog_visibility = fog_visibility(session);
    let (visible_tile_count, hidden_tile_count) = fog_tile_counts(session, fog_visibility);
    let title = session
        .display_title(locale)
        .unwrap_or(session.player().name.as_str())
        .to_string();
    let selected_block = session.selected_block_name().unwrap_or("none");
    let player_name = session.player().name.clone();
    let team_id = session.player().team_id;
    let plan_count = session.player_team_plans().len();
    let marker_count = graph.markers().count();
    let map_width = graph.width();
    let map_height = graph.height();
    let status_text = format!(
        "player={} team={} selected={} plans={} markers={} map={}x{} overlay={} fog={} vis={} hid={}",
        player_name,
        team_id,
        selected_block,
        plan_count,
        marker_count,
        map_width,
        map_height,
        if visibility.overlay_visible { 1 } else { 0 },
        if fog_visibility.enabled { 1 } else { 0 },
        visible_tile_count,
        hidden_tile_count
    );

    HudModel {
        title,
        status_text,
        fps: None,
        summary: Some(HudSummary {
            player_name,
            team_id,
            selected_block: selected_block.to_string(),
            plan_count,
            marker_count,
            map_width,
            map_height,
            overlay_visible: visibility.overlay_visible,
            fog_enabled: fog_visibility.enabled,
            visible_tile_count,
            hidden_tile_count,
        }),
        runtime_ui: None,
    }
}

fn scene_visibility(session: &LoadedWorldSession<'_>, locale: &str) -> SceneVisibility {
    let render_contract = session.enter_render_contract(locale);
    SceneVisibility {
        hud_visible: render_contract.hud.visible,
        overlay_visible: render_contract.overlay.visible,
    }
}

fn fog_visibility(session: &LoadedWorldSession<'_>) -> FogVisibility {
    FogVisibility {
        enabled: session.rules_flag("fog").unwrap_or(false)
            && session.graph().static_fog_chunk().is_some(),
        team_id: session.player().team_id,
    }
}

fn tile_visible_under_fog(
    session: &LoadedWorldSession<'_>,
    fog_visibility: FogVisibility,
    tile_x: usize,
    tile_y: usize,
) -> bool {
    !fog_visibility.enabled
        || session
            .graph()
            .fog_revealed(fog_visibility.team_id, tile_x, tile_y)
            .unwrap_or(true)
}

fn fog_tile_counts(session: &LoadedWorldSession<'_>, fog_visibility: FogVisibility) -> (usize, usize) {
    let grid = session.graph().grid();
    if !fog_visibility.enabled {
        return (grid.tile_count(), 0);
    }

    grid.iter_tiles().fold((0usize, 0usize), |(visible, hidden), tile| {
        if session
            .graph()
            .fog_revealed(fog_visibility.team_id, tile.x as usize, tile.y as usize)
            .unwrap_or(true)
        {
            (visible + 1, hidden)
        } else {
            (visible, hidden + 1)
        }
    })
}

fn project_team_plan(plan: TeamPlanRef<'_>) -> RenderObject {
    RenderObject {
        id: format!("plan:{}:{}:{}", plan.team_id, plan.plan.x, plan.plan.y),
        layer: PLAN_LAYER,
        x: plan.plan.x as f32 * TILE_SIZE,
        y: plan.plan.y as f32 * TILE_SIZE,
    }
}

fn marker_world_position(marker: &MarkerEntry) -> Option<(f32, f32)> {
    marker
        .marker
        .tile_coords()
        .map(|(x, y)| (x as f32 * TILE_SIZE, y as f32 * TILE_SIZE))
        .or_else(|| match &marker.marker {
            MarkerModel::Point(point) => Some(point_marker_world_position(point)),
            MarkerModel::Unknown(_) => None,
        })
}

fn point_marker_world_position(marker: &PointMarkerModel) -> (f32, f32) {
    (f32::from_bits(marker.x_bits), f32::from_bits(marker.y_bits))
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

fn view_window_bounds(
    width: usize,
    height: usize,
    player_position: (f32, f32),
    max_view_tiles: (usize, usize),
) -> (usize, usize, usize, usize) {
    let (max_width, max_height) = max_view_tiles;
    if width <= max_width && height <= max_height {
        return (0, 0, width, height);
    }

    let focus = (
        world_to_tile_index_clamped(player_position.0, width),
        world_to_tile_index_clamped(player_position.1, height),
    );
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

fn tile_in_window(
    tile_x: i32,
    tile_y: i32,
    window_x: usize,
    window_y: usize,
    window_width: usize,
    window_height: usize,
) -> bool {
    if tile_x < 0 || tile_y < 0 {
        return false;
    }

    let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
    tile_x >= window_x
        && tile_y >= window_y
        && tile_x < window_x.saturating_add(window_width)
        && tile_y < window_y.saturating_add(window_height)
}

#[cfg(test)]
mod tests {
    use super::{project_hud_model, project_render_model, project_render_model_with_view_window};
    use mdt_world::parse_world_bundle;

    #[test]
    fn projects_loaded_world_session_into_render_and_hud_models() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();

        let render = project_render_model(&session);
        let hud = project_hud_model(&session, "fr");

        assert_eq!(render.viewport.width, 64.0);
        assert_eq!(render.viewport.height, 64.0);
        assert!(render
            .objects
            .iter()
            .any(|object| object.id.starts_with("terrain:")));
        assert!(render.objects.iter().any(|object| object.id == "marker:11"));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id == format!("player:{}", session.player().id)));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id == "plan:1:1:2"));
        assert_eq!(hud.title, "Golden Deterministic");
        assert!(hud.status_text.contains("plans=1"));
        assert!(hud.status_text.contains("markers=2"));
        assert!(hud.status_text.contains("map=8x8"));
        assert!(hud.status_text.contains("overlay=1"));
        assert!(hud.status_text.contains("fog=1"));
    }

    #[test]
    fn runtime_player_position_override_moves_player_object() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();

        let render = super::project_render_model_with_player_position(&session, Some((80.0, 96.0)));
        let player = render
            .objects
            .iter()
            .find(|object| object.id == format!("player:{}", session.player().id))
            .unwrap();

        assert_eq!((player.x, player.y), (80.0, 96.0));
    }

    #[test]
    fn view_window_projection_omits_offscreen_tiles() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();

        let full = project_render_model(&session);
        let cropped = project_render_model_with_view_window(&session, Some((32.0, 32.0)), (4, 4));

        assert!(cropped.objects.len() < full.objects.len());
        assert!(!cropped
            .objects
            .iter()
            .any(|object| object.id == "terrain:0"));
        assert!(cropped
            .objects
            .iter()
            .any(|object| object.id == format!("player:{}", session.player().id)));
    }

    #[test]
    fn hud_projection_expresses_hidden_state_when_contract_hud_not_visible() {
        let mut bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        bundle.tag_pairs.retain(|(key, _)| key != "name");
        let session = bundle.loaded_session().unwrap();

        let contract = session.enter_render_contract("zz");
        assert!(!contract.hud.visible);

        let hud = project_hud_model(&session, "zz");
        assert!(hud.is_hidden());
        assert!(!hud.is_visible());
        assert_eq!(hud.title, "");
        assert_eq!(hud.status_text, "");
        assert_eq!(hud.summary, None);
    }

    #[test]
    fn hud_projection_populates_structured_summary() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();

        let hud = project_hud_model(&session, "fr");
        let summary = hud.summary.as_ref().expect("summary should be present");

        assert_eq!(summary.player_name, session.player().name);
        assert_eq!(summary.team_id, session.player().team_id);
        assert_eq!(summary.selected_block, "none");
        assert_eq!(summary.plan_count, session.player_team_plans().len());
        assert_eq!(summary.marker_count, session.graph().markers().count());
        assert_eq!(summary.map_width, session.graph().width());
        assert_eq!(summary.map_height, session.graph().height());
        assert_eq!(
            summary.overlay_visible,
            session.enter_render_contract("fr").overlay.visible
        );
        assert_eq!(summary.fog_enabled, true);
        assert_eq!(
            summary.visible_tile_count + summary.hidden_tile_count,
            session.graph().grid().tile_count()
        );
        assert!(summary.hidden_tile_count > 0);
    }

    #[test]
    fn scene_projection_omits_overlay_objects_when_contract_overlay_not_visible() {
        let mut bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        bundle.tag_pairs.retain(|(key, _)| key != "name");
        let session = bundle.loaded_session().unwrap();

        let contract = session.enter_render_contract("zz");
        assert!(!contract.overlay.visible);

        let (render, hud) = super::project_scene_models(&session, "zz");
        assert!(hud.is_hidden());
        assert!(!render
            .objects
            .iter()
            .any(|object| object.id.starts_with("plan:")));
        assert!(!render
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:")));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id == format!("player:{}", session.player().id)));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id.starts_with("terrain:")));
    }

    #[test]
    fn render_projection_omits_unrevealed_tiles_under_static_fog() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();
        let player_team_id = session.player().team_id;
        let hidden_tile = session
            .graph()
            .grid()
            .iter_tiles()
            .find(|tile| {
                session
                    .graph()
                    .fog_revealed(player_team_id, tile.x as usize, tile.y as usize)
                    == Some(false)
            })
            .map(|tile| tile.tile_index)
            .expect("expected at least one unrevealed tile in sample world");
        let revealed_tile = session
            .graph()
            .grid()
            .iter_tiles()
            .find(|tile| {
                session
                    .graph()
                    .fog_revealed(player_team_id, tile.x as usize, tile.y as usize)
                    == Some(true)
            })
            .map(|tile| tile.tile_index)
            .expect("expected at least one revealed tile in sample world");

        let render = project_render_model(&session);

        assert!(!render
            .objects
            .iter()
            .any(|object| object.id == format!("terrain:{hidden_tile}")));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id == format!("terrain:{revealed_tile}")));
    }

    #[test]
    fn view_window_bounds_is_stable_around_half_tile_positions() {
        let left = super::view_window_bounds(8, 8, (27.9, 32.0), (4, 4));
        let right = super::view_window_bounds(8, 8, (28.1, 32.0), (4, 4));

        assert_eq!(left, (1, 2, 4, 4));
        assert_eq!(left, right);
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
