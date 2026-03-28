use crate::{
    hud_model::{HudMinimapSummary, HudSummary, HudViewWindowSummary},
    render_model::encode_render_text,
    HudModel, RenderModel, RenderObject, RenderViewWindow, Viewport,
};
use mdt_world::{LineMarkerModel, LoadedWorldSession, MarkerEntry, MarkerModel, TeamPlanRef};

const TILE_SIZE: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionLayer {
    Terrain,
    Block,
    Plan,
    Marker,
    Player,
}

impl ProjectionLayer {
    fn value(self) -> i32 {
        match self {
            Self::Terrain => 0,
            Self::Block => 10,
            Self::Plan => 20,
            Self::Marker => 30,
            Self::Player => 40,
        }
    }
}

impl From<ProjectionLayer> for i32 {
    fn from(layer: ProjectionLayer) -> Self {
        layer.value()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SceneVisibility {
    hud_visible: bool,
    overlay_visible: bool,
    hud_title_text: Option<String>,
    hud_wave_text: Option<String>,
    hud_status_text: Option<String>,
    overlay_summary_text: Option<String>,
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
        project_hud_model_with_visibility(session, locale, visibility, player_position, None),
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
        project_hud_model_with_visibility(
            session,
            locale,
            visibility,
            player_position,
            Some(max_view_tiles),
        ),
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
            layer: i32::from(ProjectionLayer::Terrain),
            x: world_x,
            y: world_y,
        });

        if tile.block_id != 0 {
            objects.push(RenderObject {
                id: format!("block:{}:{}", tile.tile_index, tile.block_id),
                layer: i32::from(ProjectionLayer::Block),
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
            for object in project_marker_objects(marker) {
                let marker_tile_x = world_to_tile_index_clamped(object.x, graph.width());
                let marker_tile_y = world_to_tile_index_clamped(object.y, graph.height());
                if !tile_visible_under_fog(session, fog_visibility, marker_tile_x, marker_tile_y) {
                    continue;
                }
                objects.push(object);
            }
        }
    }

    let (player_x, player_y) = player_position.unwrap_or_else(|| session.state().player_position());
    objects.push(RenderObject {
        id: format!("player:{}", session.player().id),
        layer: i32::from(ProjectionLayer::Player),
        x: player_x,
        y: player_y,
    });

    RenderModel {
        viewport: Viewport {
            width: graph.width() as f32 * TILE_SIZE,
            height: graph.height() as f32 * TILE_SIZE,
            zoom: 1.0,
        },
        view_window: Some(RenderViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: graph.width(),
            height: graph.height(),
        }),
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
                layer: i32::from(ProjectionLayer::Terrain),
                x: world_x,
                y: world_y,
            });

            if tile.block_id != 0 {
                objects.push(RenderObject {
                    id: format!("block:{}:{}", tile.tile_index, tile.block_id),
                    layer: i32::from(ProjectionLayer::Block),
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
            for object in project_marker_objects(marker) {
                let marker_tile_x = world_to_tile_index_clamped(object.x, graph.width());
                let marker_tile_y = world_to_tile_index_clamped(object.y, graph.height());
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
                    objects.push(object);
                }
            }
        }
    }

    objects.push(RenderObject {
        id: format!("player:{}", session.player().id),
        layer: i32::from(ProjectionLayer::Player),
        x: player_x,
        y: player_y,
    });

    RenderModel {
        viewport: Viewport {
            width: graph.width() as f32 * TILE_SIZE,
            height: graph.height() as f32 * TILE_SIZE,
            zoom: 1.0,
        },
        view_window: Some(RenderViewWindow {
            origin_x: window_x,
            origin_y: window_y,
            width: window_width,
            height: window_height,
        }),
        objects,
    }
}

pub fn project_hud_model(session: &LoadedWorldSession<'_>, locale: &str) -> HudModel {
    let visibility = scene_visibility(session, locale);
    project_hud_model_with_visibility(session, locale, visibility, None, None)
}

fn project_hud_model_with_visibility(
    session: &LoadedWorldSession<'_>,
    locale: &str,
    visibility: SceneVisibility,
    player_position: Option<(f32, f32)>,
    max_view_tiles: Option<(usize, usize)>,
) -> HudModel {
    if !visibility.hud_visible && !visibility.overlay_visible {
        return HudModel::hidden();
    }

    let graph = session.graph();
    let fog_visibility = fog_visibility(session);
    let (visible_tile_count, hidden_tile_count) = fog_tile_counts(session, fog_visibility);
    let title = if visibility.hud_visible {
        visibility
            .hud_title_text
            .clone()
            .or_else(|| {
                session
                    .display_title(locale)
                    .map(std::string::ToString::to_string)
            })
            .unwrap_or_else(|| session.player().name.clone())
    } else {
        String::new()
    };
    let selected_block = session.selected_block_name().unwrap_or("none");
    let player_name = session.player().name.clone();
    let team_id = session.player().team_id;
    let plan_count = session.player_team_plans().len();
    let marker_count = graph.markers().count();
    let map_width = graph.width();
    let map_height = graph.height();
    let minimap = project_hud_minimap_summary(session, player_position, max_view_tiles);
    let status_text = if visibility.hud_visible {
        visibility.hud_status_text.clone().unwrap_or_else(|| {
            format!(
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
            )
        })
    } else {
        String::new()
    };

    HudModel {
        title,
        wave_text: visibility
            .hud_visible
            .then(|| visibility.hud_wave_text.clone())
            .flatten(),
        status_text,
        overlay_summary_text: visibility
            .overlay_visible
            .then(|| visibility.overlay_summary_text.clone())
            .flatten(),
        fps: None,
        summary: visibility.hud_visible.then_some(HudSummary {
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
            minimap,
        }),
        runtime_ui: None,
        build_ui: None,
    }
}

fn project_hud_minimap_summary(
    session: &LoadedWorldSession<'_>,
    player_position: Option<(f32, f32)>,
    max_view_tiles: Option<(usize, usize)>,
) -> HudMinimapSummary {
    let graph = session.graph();
    let map_width = graph.width();
    let map_height = graph.height();
    let (player_x, player_y) = player_position.unwrap_or_else(|| session.state().player_position());
    let focus_tile =
        (map_width > 0 && map_height > 0 && player_x.is_finite() && player_y.is_finite())
            .then_some((
                world_to_tile_index_clamped(player_x, map_width),
                world_to_tile_index_clamped(player_y, map_height),
            ));
    let (origin_x, origin_y, width, height) = max_view_tiles
        .map(|max_view_tiles| {
            view_window_bounds(map_width, map_height, (player_x, player_y), max_view_tiles)
        })
        .unwrap_or((0, 0, map_width, map_height));

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

fn scene_visibility(session: &LoadedWorldSession<'_>, locale: &str) -> SceneVisibility {
    let render_contract = session.enter_render_contract(locale);
    SceneVisibility {
        hud_visible: render_contract.hud.visible,
        overlay_visible: render_contract.overlay.visible,
        hud_title_text: render_contract.hud.title_text.clone(),
        hud_wave_text: render_contract.hud.wave_text.clone(),
        hud_status_text: render_contract.hud.status_text.clone(),
        overlay_summary_text: render_contract.overlay.summary_text.clone(),
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

fn fog_tile_counts(
    session: &LoadedWorldSession<'_>,
    fog_visibility: FogVisibility,
) -> (usize, usize) {
    let grid = session.graph().grid();
    if !fog_visibility.enabled {
        return (grid.tile_count(), 0);
    }

    grid.iter_tiles()
        .fold((0usize, 0usize), |(visible, hidden), tile| {
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
        id: format!(
            "plan:build:{}:{}:{}:{}",
            plan.team_id, plan.plan.x, plan.plan.y, plan.plan.block_id
        ),
        layer: i32::from(ProjectionLayer::Plan),
        x: plan.plan.x as f32 * TILE_SIZE,
        y: plan.plan.y as f32 * TILE_SIZE,
    }
}

fn project_marker_objects(marker: &MarkerEntry) -> Vec<RenderObject> {
    let mut objects = Vec::with_capacity(2);
    let marker_kind = marker_kind_id_segment(marker);
    let start_world = marker_world_position(marker);
    if let Some((x, y)) = start_world {
        let marker_id = marker_text_payload(marker)
            .filter(|text| !text.is_empty())
            .map_or_else(
                || marker.id.to_string(),
                |text| format!("{}:text:{}", marker.id, encode_render_text(text)),
            );
        objects.push(RenderObject {
            id: format!("marker:{marker_kind}:{marker_id}"),
            layer: i32::from(ProjectionLayer::Marker),
            x,
            y,
        });
    }

    if let MarkerModel::Line(line) = &marker.marker {
        if let Some((x, y)) = line_marker_end_world_position(line) {
            if start_world != Some((x, y)) {
                objects.push(RenderObject {
                    id: format!("marker:{marker_kind}:{}:line-end", marker.id),
                    layer: i32::from(ProjectionLayer::Marker),
                    x,
                    y,
                });
            }
        }
    }

    objects
}

fn marker_text_payload(marker: &MarkerEntry) -> Option<&str> {
    match &marker.marker {
        MarkerModel::Text(text) => Some(text.text.as_str()),
        MarkerModel::ShapeText(text) => Some(text.text.as_str()),
        _ => None,
    }
}

fn marker_kind_id_segment(marker: &MarkerEntry) -> &'static str {
    match &marker.marker {
        MarkerModel::Point(_) => "point",
        MarkerModel::Text(_) => "text",
        MarkerModel::Shape(_) => "shape",
        MarkerModel::ShapeText(_) => "shape-text",
        MarkerModel::Line(_) => "line",
        MarkerModel::Texture(_) => "texture",
        MarkerModel::Quad(_) => "quad",
        MarkerModel::Unknown(_) => "unknown",
    }
}

fn marker_world_position(marker: &MarkerEntry) -> Option<(f32, f32)> {
    finite_world_position(marker.marker.world_position()).or_else(|| {
        marker
            .marker
            .tile_coords()
            .map(|(x, y)| (x as f32 * TILE_SIZE, y as f32 * TILE_SIZE))
    })
}

fn line_marker_end_world_position(line: &LineMarkerModel) -> Option<(f32, f32)> {
    finite_line_marker_world_position(line).or_else(|| {
        line.end_tile_coords()
            .map(|(x, y)| (x as f32 * TILE_SIZE, y as f32 * TILE_SIZE))
    })
}

fn finite_line_marker_world_position(line: &LineMarkerModel) -> Option<(f32, f32)> {
    finite_world_position(Some(line.end_world_position()))
}

fn finite_world_position(position: Option<(f32, f32)>) -> Option<(f32, f32)> {
    match position {
        Some((x, y)) if x.is_finite() && y.is_finite() => Some((x, y)),
        _ => None,
    }
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
    use crate::render_model::{
        RenderObjectSemanticKind, RenderPrimitive, RenderPrimitivePayloadValue,
    };
    use crate::{RenderModel, RenderViewWindow};
    use mdt_world::{
        parse_world_bundle, LineMarkerModel, MarkerEntry, MarkerModel, PointMarkerModel,
        TextMarkerModel,
    };

    #[test]
    fn projects_loaded_world_session_into_render_and_hud_models() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();

        let render = project_render_model(&session);
        let hud = project_hud_model(&session, "fr");
        let contract = session.enter_render_contract("fr");
        let expected_plan_ids = session
            .player_team_plans()
            .into_iter()
            .map(|plan| {
                format!(
                    "plan:build:{}:{}:{}:{}",
                    plan.team_id, plan.plan.x, plan.plan.y, plan.plan.block_id
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(render.viewport.width, 64.0);
        assert_eq!(render.viewport.height, 64.0);
        assert!(render
            .objects
            .iter()
            .any(|object| object.id.starts_with("terrain:")));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id.starts_with("marker:")));
        assert!(render
            .objects
            .iter()
            .any(|object| object.id == format!("player:{}", session.player().id)));
        assert!(render.objects.iter().any(|object| expected_plan_ids
            .iter()
            .any(|expected| object.id == *expected)));
        assert_eq!(hud.title, "Golden Deterministic");
        assert_eq!(hud.wave_text.as_deref(), contract.hud.wave_text.as_deref());
        assert_eq!(
            hud.status_text,
            contract.hud.status_text.as_deref().unwrap_or_default()
        );
        assert_eq!(
            hud.overlay_summary_text.as_deref(),
            contract.overlay.summary_text.as_deref()
        );
        let summary = hud.summary.as_ref().expect("summary should be present");
        assert_eq!(summary.plan_count, 1);
        assert_eq!(summary.marker_count, 2);
        assert_eq!(summary.map_width, 8);
        assert_eq!(summary.map_height, 8);
        assert!(summary.overlay_visible);
        assert!(summary.fog_enabled);
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
        assert_eq!(
            cropped.view_window,
            Some(RenderViewWindow {
                origin_x: 2,
                origin_y: 2,
                width: 4,
                height: 4,
            })
        );
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
    fn line_marker_projects_start_and_end_anchors() {
        let marker = MarkerEntry {
            id: 77,
            marker: MarkerModel::Line(LineMarkerModel {
                class_tag: "Line".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0f32.to_bits(),
                y_bits: 24.0f32.to_bits(),
                end_x_bits: 40.0f32.to_bits(),
                end_y_bits: 56.0f32.to_bits(),
                stroke_bits: 1.0f32.to_bits(),
                outline: true,
                color1: Some("ffd37f".to_string()),
                color2: Some("ffd37f".to_string()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].id, "marker:line:77");
        assert_eq!((objects[0].x, objects[0].y), (16.0, 24.0));
        assert_eq!(objects[1].id, "marker:line:77:line-end");
        assert_eq!((objects[1].x, objects[1].y), (40.0, 56.0));
    }

    #[test]
    fn line_marker_dedupes_line_end_when_both_anchors_share_world_position() {
        let marker = MarkerEntry {
            id: 78,
            marker: MarkerModel::Line(LineMarkerModel {
                class_tag: "Line".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0004f32.to_bits(),
                y_bits: 24.0004f32.to_bits(),
                end_x_bits: 16.0004f32.to_bits(),
                end_y_bits: 24.0004f32.to_bits(),
                stroke_bits: 1.0f32.to_bits(),
                outline: true,
                color1: Some("ffd37f".to_string()),
                color2: Some("ffd37f".to_string()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "marker:line:78");
        assert_eq!((objects[0].x, objects[0].y), (16.0004, 24.0004));
    }

    #[test]
    fn project_marker_objects_keeps_same_tile_nonzero_line_segments() {
        let marker = MarkerEntry {
            id: 79,
            marker: MarkerModel::Line(LineMarkerModel {
                class_tag: "Line".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0004f32.to_bits(),
                y_bits: 24.0004f32.to_bits(),
                end_x_bits: 16.0007f32.to_bits(),
                end_y_bits: 24.0007f32.to_bits(),
                stroke_bits: 1.0f32.to_bits(),
                outline: true,
                color1: Some("ffd37f".to_string()),
                color2: Some("ffd37f".to_string()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].id, "marker:line:79");
        assert_eq!((objects[0].x, objects[0].y), (16.0004, 24.0004));
        assert_eq!(objects[1].id, "marker:line:79:line-end");
        assert_eq!((objects[1].x, objects[1].y), (16.0007, 24.0007));
    }

    #[test]
    fn point_marker_projects_kind_specific_id_prefix() {
        let marker = MarkerEntry {
            id: 42,
            marker: MarkerModel::Point(PointMarkerModel {
                class_tag: "Point".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0f32.to_bits(),
                y_bits: 24.0f32.to_bits(),
                radius_bits: 1.0f32.to_bits(),
                stroke_bits: 0.5f32.to_bits(),
                color: Some("ffffff".to_string()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "marker:point:42");
        assert_eq!((objects[0].x, objects[0].y), (16.0, 24.0));
    }

    #[test]
    fn text_marker_projects_encoded_payload_for_text_primitives() {
        let marker = MarkerEntry {
            id: 43,
            marker: MarkerModel::Text(TextMarkerModel {
                class_tag: "Text".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0f32.to_bits(),
                y_bits: 24.0f32.to_bits(),
                text: "Hello".to_string(),
                font_size_bits: 12.0f32.to_bits(),
                flags: 0,
                text_align: 0,
                line_align: 0,
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "marker:text:43:text:48656c6c6f");
        assert_eq!((objects[0].x, objects[0].y), (16.0, 24.0));

        let scene = RenderModel {
            viewport: Default::default(),
            view_window: None,
            objects,
        };

        let primitives = scene.primitives();
        assert_eq!(
            primitives,
            vec![RenderPrimitive::Text {
                id: "marker:text:43:text:48656c6c6f".to_string(),
                kind: RenderObjectSemanticKind::MarkerText,
                layer: 30,
                x: 16.0,
                y: 24.0,
                text: "Hello".to_string(),
            }]
        );
        let payload = primitives[0].payload().expect("text payload");
        assert_eq!(payload.label, "marker-text");
        assert_eq!(
            payload.field("text"),
            Some(&RenderPrimitivePayloadValue::Text("Hello".to_string()))
        );
    }

    #[test]
    fn team_plan_projection_carries_build_semantic_and_block_id() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();
        let plan = session
            .player_team_plans()
            .into_iter()
            .next()
            .expect("expected a sample build plan");
        let expected_id = format!(
            "plan:build:{}:{}:{}:{}",
            plan.team_id, plan.plan.x, plan.plan.y, plan.plan.block_id
        );
        let expected_position = (plan.plan.x as f32 * 8.0, plan.plan.y as f32 * 8.0);

        let projected = super::project_team_plan(plan);

        assert_eq!(projected.id, expected_id);
        assert_eq!((projected.x, projected.y), expected_position);
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
        assert_eq!(hud.wave_text, None);
        assert_eq!(hud.status_text, "");
        assert_eq!(hud.overlay_summary_text, None);
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

    #[test]
    fn hud_minimap_focus_tile_is_none_for_non_finite_player_position() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();
        let visibility = super::scene_visibility(&session, "fr");

        let hud = super::project_hud_model_with_visibility(
            &session,
            "fr",
            visibility,
            Some((f32::NAN, f32::INFINITY)),
            None,
        );
        let summary = hud.summary.as_ref().expect("summary should be present");

        assert_eq!(summary.minimap.focus_tile, None);
    }

    #[test]
    fn projection_layers_remain_named_and_stable() {
        assert_eq!(i32::from(super::ProjectionLayer::Terrain), 0);
        assert_eq!(i32::from(super::ProjectionLayer::Block), 10);
        assert_eq!(i32::from(super::ProjectionLayer::Plan), 20);
        assert_eq!(i32::from(super::ProjectionLayer::Marker), 30);
        assert_eq!(i32::from(super::ProjectionLayer::Player), 40);
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
