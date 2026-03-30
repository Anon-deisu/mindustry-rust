use crate::{
    hud_model::{HudMinimapSummary, HudSummary, HudViewWindowSummary},
    render_model::encode_render_text,
    HudModel, RenderModel, RenderObject, RenderViewWindow, Viewport,
};
use mdt_world::{
    LineMarkerModel, LoadedWorldSession, MarkerEntry, MarkerModel, QuadMarkerModel, TeamPlanRef,
};

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
            let plan_x = i32::from(plan.plan.x);
            let plan_y = i32::from(plan.plan.y);
            if visible_tile_coords_under_fog(
                session,
                fog_visibility,
                tile_coords_in_bounds(plan_x, plan_y, graph.width(), graph.height()),
            )
            .is_some()
            {
                objects.push(project_team_plan(plan));
            }
        }

        for marker in graph.markers() {
            for object in project_marker_objects(marker) {
                if visible_tile_coords_under_fog(
                    session,
                    fog_visibility,
                    world_position_tile_coords_in_bounds(
                        object.x,
                        object.y,
                        graph.width(),
                        graph.height(),
                    ),
                )
                .is_some()
                {
                    objects.push(object);
                }
            }
        }
    }

    let (player_x, player_y) = player_position.unwrap_or_else(|| session.state().player_position());
    if player_x.is_finite() && player_y.is_finite() {
        objects.push(RenderObject {
            id: format!("player:{}", session.player().id),
            layer: i32::from(ProjectionLayer::Player),
            x: player_x,
            y: player_y,
        });
    }

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
            let plan_x = i32::from(plan.plan.x);
            let plan_y = i32::from(plan.plan.y);
            if visible_tile_coords_under_fog(
                session,
                fog_visibility,
                tile_coords_in_bounds(plan_x, plan_y, graph.width(), graph.height()),
            )
            .is_some()
                && tile_in_window(
                    plan_x,
                    plan_y,
                    window_x,
                    window_y,
                    window_width,
                    window_height,
                )
            {
                objects.push(project_team_plan(plan));
            }
        }

        for marker in graph.markers() {
            for object in project_marker_objects(marker) {
                if let Some((marker_tile_x, marker_tile_y)) = visible_tile_coords_under_fog(
                    session,
                    fog_visibility,
                    world_position_tile_coords_in_bounds(
                        object.x,
                        object.y,
                        graph.width(),
                        graph.height(),
                    ),
                ) {
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
    }

    if player_x.is_finite() && player_y.is_finite() {
        objects.push(RenderObject {
            id: format!("player:{}", session.player().id),
            layer: i32::from(ProjectionLayer::Player),
            x: player_x,
            y: player_y,
        });
    }

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
        || fog_reveal_is_visible(session.graph().fog_revealed(
            fog_visibility.team_id,
            tile_x,
            tile_y,
        ))
}

fn visible_tile_coords_under_fog(
    session: &LoadedWorldSession<'_>,
    fog_visibility: FogVisibility,
    tile_coords: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let (tile_x, tile_y) = tile_coords?;
    tile_visible_under_fog(session, fog_visibility, tile_x, tile_y).then_some((tile_x, tile_y))
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
            if fog_reveal_is_visible(session.graph().fog_revealed(
                fog_visibility.team_id,
                tile.x as usize,
                tile.y as usize,
            )) {
                (visible + 1, hidden)
            } else {
                (visible, hidden + 1)
            }
        })
}

fn fog_reveal_is_visible(revealed: Option<bool>) -> bool {
    matches!(revealed, Some(true))
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
    let mut objects = Vec::with_capacity(9);
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

    if let MarkerModel::Quad(quad) = &marker.marker {
        for (edge_index, (start, end)) in quad_marker_edge_world_positions(quad).enumerate() {
            objects.push(RenderObject {
                id: format!("marker:line:quad:{}:{edge_index}", marker.id),
                layer: i32::from(ProjectionLayer::Marker),
                x: start.0,
                y: start.1,
            });
            objects.push(RenderObject {
                id: format!("marker:line:quad:{}:{edge_index}:line-end", marker.id),
                layer: i32::from(ProjectionLayer::Marker),
                x: end.0,
                y: end.1,
            });
        }
    }

    objects
}

#[cfg_attr(not(test), allow(dead_code))]
fn marker_projection_summary(marker: &MarkerEntry) -> String {
    let objects = project_marker_objects(marker);
    let line_end_count = objects
        .iter()
        .filter(|object| object.id.ends_with(":line-end"))
        .count();

    format!(
        "kind={} objects={} line-end={}",
        marker_kind_id_segment(marker),
        objects.len(),
        line_end_count
    )
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
    finite_world_position(marker.marker.world_position())
        .or_else(|| unknown_marker_world_position(&marker.marker))
        .or_else(|| {
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

fn quad_marker_edge_world_positions(
    quad: &QuadMarkerModel,
) -> impl Iterator<Item = ((f32, f32), (f32, f32))> + '_ {
    let vertices = quad
        .vertices_bits
        .chunks_exact(6)
        .map(|vertex| {
            finite_world_position(Some((f32::from_bits(vertex[0]), f32::from_bits(vertex[1]))))
        })
        .collect::<Vec<_>>();
    let len = vertices.len();
    (0..len).filter_map(move |index| {
        let start = vertices.get(index).copied().flatten()?;
        let end = vertices.get((index + 1) % len).copied().flatten()?;
        (start != end).then_some((start, end))
    })
}

fn finite_line_marker_world_position(line: &LineMarkerModel) -> Option<(f32, f32)> {
    finite_world_position(Some(line.end_world_position()))
}

fn unknown_marker_world_position(marker: &MarkerModel) -> Option<(f32, f32)> {
    let MarkerModel::Unknown(unknown) = marker else {
        return None;
    };
    finite_world_position(Some((
        f32::from_bits(unknown.x_bits?),
        f32::from_bits(unknown.y_bits?),
    )))
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

fn world_to_tile_index_in_bounds(world_position: f32, bound: usize) -> Option<usize> {
    if bound == 0 || !world_position.is_finite() {
        return None;
    }
    let tile = (world_position / TILE_SIZE).floor();
    if tile < 0.0 || tile >= bound as f32 {
        return None;
    }
    Some(tile as usize)
}

fn world_position_tile_coords_in_bounds(
    world_x: f32,
    world_y: f32,
    width: usize,
    height: usize,
) -> Option<(usize, usize)> {
    Some((
        world_to_tile_index_in_bounds(world_x, width)?,
        world_to_tile_index_in_bounds(world_y, height)?,
    ))
}

fn world_to_tile_index_clamped(world_position: f32, bound: usize) -> usize {
    if bound == 0 {
        return 0;
    }
    world_to_tile_index_floor(world_position).clamp(0, bound.saturating_sub(1) as i32) as usize
}

fn tile_coords_in_bounds(
    tile_x: i32,
    tile_y: i32,
    width: usize,
    height: usize,
) -> Option<(usize, usize)> {
    let tile_x = usize::try_from(tile_x).ok()?;
    let tile_y = usize::try_from(tile_y).ok()?;
    (tile_x < width && tile_y < height).then_some((tile_x, tile_y))
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

    let focus = if player_position.0.is_finite() && player_position.1.is_finite() {
        (
            world_to_tile_index_clamped(player_position.0, width),
            world_to_tile_index_clamped(player_position.1, height),
        )
    } else {
        (width / 2, height / 2)
    };
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
    use super::{
        fog_reveal_is_visible, marker_projection_summary, project_hud_model, project_render_model,
        project_render_model_with_player_position, project_render_model_with_view_window,
    };
    use crate::render_model::{
        RenderObjectSemanticKind, RenderPrimitive, RenderPrimitivePayloadValue,
    };
    use crate::{RenderModel, RenderViewWindow};
    use mdt_world::{
        parse_world_bundle, LineMarkerModel, MarkerEntry, MarkerModel, PointMarkerModel,
        QuadMarkerModel, ShapeMarkerModel, TextMarkerModel, TextureMarkerModel, UnknownMarkerModel,
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
    fn render_projection_treats_missing_fog_reveal_data_as_hidden() {
        assert!(!fog_reveal_is_visible(None));
        assert!(!fog_reveal_is_visible(Some(false)));
        assert!(fog_reveal_is_visible(Some(true)));
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
    fn shape_marker_projects_kind_specific_id_prefix_and_position() {
        let marker = MarkerEntry {
            id: 44,
            marker: MarkerModel::Shape(ShapeMarkerModel {
                class_tag: "Shape".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0f32.to_bits(),
                y_bits: 24.0f32.to_bits(),
                radius_bits: 8.0f32.to_bits(),
                rotation_bits: 0.0f32.to_bits(),
                stroke_bits: 1.0f32.to_bits(),
                start_angle_bits: 0.0f32.to_bits(),
                end_angle_bits: 360.0f32.to_bits(),
                fill: false,
                outline: true,
                sides: 4,
                color: Some("ffd37f".to_string()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "marker:shape:44");
        assert_eq!((objects[0].x, objects[0].y), (16.0, 24.0));
    }

    #[test]
    fn texture_marker_projects_kind_specific_id_prefix_and_position() {
        let marker = MarkerEntry {
            id: 45,
            marker: MarkerModel::Texture(TextureMarkerModel {
                class_tag: "Texture".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: 16.0f32.to_bits(),
                y_bits: 24.0f32.to_bits(),
                rotation_bits: 0.0f32.to_bits(),
                width_bits: 8.0f32.to_bits(),
                height_bits: 8.0f32.to_bits(),
                texture: mdt_world::MarkerTextureRef {
                    kind: "atlas".to_string(),
                    value: "block-1".to_string(),
                },
                color: Some("ffffffff".to_string()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "marker:texture:45");
        assert_eq!((objects[0].x, objects[0].y), (16.0, 24.0));
    }

    #[test]
    fn quad_marker_projects_edge_segments_into_line_primitives() {
        let mut vertices_bits = vec![0u32; 24];
        for (index, (x, y)) in [(8.0f32, 16.0f32), (24.0, 16.0), (24.0, 32.0), (8.0, 32.0)]
            .into_iter()
            .enumerate()
        {
            let base = index * 6;
            vertices_bits[base] = x.to_bits();
            vertices_bits[base + 1] = y.to_bits();
        }
        let marker = MarkerEntry {
            id: 46,
            marker: MarkerModel::Quad(QuadMarkerModel {
                class_tag: "Quad".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                texture: mdt_world::MarkerTextureRef {
                    kind: "atlas".to_string(),
                    value: "block-1".to_string(),
                },
                vertices_bits,
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects[0].id, "marker:quad:46");
        assert_eq!((objects[0].x, objects[0].y), (8.0, 16.0));
        assert_eq!(objects.len(), 9);
        assert_eq!(objects[1].id, "marker:line:quad:46:0");
        assert_eq!((objects[1].x, objects[1].y), (8.0, 16.0));
        assert_eq!(objects[2].id, "marker:line:quad:46:0:line-end");
        assert_eq!((objects[2].x, objects[2].y), (24.0, 16.0));
        assert_eq!(objects[7].id, "marker:line:quad:46:3");
        assert_eq!((objects[7].x, objects[7].y), (8.0, 32.0));
        assert_eq!(objects[8].id, "marker:line:quad:46:3:line-end");
        assert_eq!((objects[8].x, objects[8].y), (8.0, 16.0));

        let scene = RenderModel {
            viewport: Default::default(),
            view_window: None,
            objects,
        };

        assert_eq!(
            scene.primitives(),
            vec![
                RenderPrimitive::Line {
                    id: "marker:line:quad:46:0".to_string(),
                    layer: 30,
                    x0: 8.0,
                    y0: 16.0,
                    x1: 24.0,
                    y1: 16.0,
                },
                RenderPrimitive::Line {
                    id: "marker:line:quad:46:1".to_string(),
                    layer: 30,
                    x0: 24.0,
                    y0: 16.0,
                    x1: 24.0,
                    y1: 32.0,
                },
                RenderPrimitive::Line {
                    id: "marker:line:quad:46:2".to_string(),
                    layer: 30,
                    x0: 24.0,
                    y0: 32.0,
                    x1: 8.0,
                    y1: 32.0,
                },
                RenderPrimitive::Line {
                    id: "marker:line:quad:46:3".to_string(),
                    layer: 30,
                    x0: 8.0,
                    y0: 32.0,
                    x1: 8.0,
                    y1: 16.0,
                },
            ]
        );
    }

    #[test]
    fn marker_projection_summary_reports_kind_object_count_and_line_end_count() {
        let point_marker = MarkerEntry {
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
        let line_marker = MarkerEntry {
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
        let mut vertices_bits = vec![0u32; 24];
        for (index, (x, y)) in [(8.0f32, 16.0f32), (24.0, 16.0), (24.0, 32.0), (8.0, 32.0)]
            .into_iter()
            .enumerate()
        {
            let base = index * 6;
            vertices_bits[base] = x.to_bits();
            vertices_bits[base + 1] = y.to_bits();
        }
        let quad_marker = MarkerEntry {
            id: 46,
            marker: MarkerModel::Quad(QuadMarkerModel {
                class_tag: "Quad".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                texture: mdt_world::MarkerTextureRef {
                    kind: "atlas".to_string(),
                    value: "block-1".to_string(),
                },
                vertices_bits,
            }),
        };

        assert_eq!(
            marker_projection_summary(&point_marker),
            "kind=point objects=1 line-end=0"
        );
        assert_eq!(
            marker_projection_summary(&line_marker),
            "kind=line objects=2 line-end=1"
        );
        assert_eq!(
            marker_projection_summary(&quad_marker),
            "kind=quad objects=9 line-end=4"
        );
    }

    #[test]
    fn unknown_marker_projects_anchor_when_coordinates_are_present() {
        let marker = MarkerEntry {
            id: 47,
            marker: MarkerModel::Unknown(UnknownMarkerModel {
                class_tag: Some("MysteryMarker".to_string()),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: Some(0),
                x_bits: Some(16.0f32.to_bits()),
                y_bits: Some(24.0f32.to_bits()),
            }),
        };

        let objects = super::project_marker_objects(&marker);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "marker:unknown:47");
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
    fn render_projection_drops_out_of_bounds_plans_under_fog() {
        let mut bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let player_team_id = {
            let session = bundle.loaded_session().unwrap();
            session.player().team_id as u32
        };
        let group = bundle
            .team_plan_groups
            .iter_mut()
            .find(|group| group.team_id == player_team_id)
            .expect("expected player team plan group in sample world");
        group.plan_count += 1;
        group.plans.push(mdt_world::TeamPlan {
            x: -1,
            y: 0,
            rotation: 0,
            block_id: 0x0101,
            config: mdt_world::TypeIoValue::Null,
            config_bytes: Vec::new(),
            config_sha256: "out-of-bounds-plan".to_string(),
        });

        let session = bundle.loaded_session().unwrap();
        let render = project_render_model(&session);

        assert!(!render
            .objects
            .iter()
            .any(|object| object.id == format!("plan:build:{player_team_id}:-1:0:257")));
    }

    #[test]
    fn render_projection_drops_out_of_bounds_markers_under_fog() {
        let mut bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        bundle.markers.push(MarkerEntry {
            id: 999,
            marker: MarkerModel::Point(PointMarkerModel {
                class_tag: "OutOfBounds".to_string(),
                world: true,
                minimap: true,
                autoscale: false,
                draw_layer_bits: 0,
                x_bits: (-8.0f32).to_bits(),
                y_bits: 8.0f32.to_bits(),
                radius_bits: 1.0f32.to_bits(),
                stroke_bits: 0.5f32.to_bits(),
                color: Some("ffffff".to_string()),
            }),
        });

        let session = bundle.loaded_session().unwrap();
        let render = project_render_model_with_view_window(&session, None, (4, 4));

        assert!(!render
            .objects
            .iter()
            .any(|object| object.id == "marker:point:999"));
    }

    #[test]
    fn project_render_model_rejects_non_finite_player_position() {
        let bundle = parse_world_bundle(&decode_hex(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        )))
        .unwrap();
        let session = bundle.loaded_session().unwrap();

        let render =
            project_render_model_with_player_position(&session, Some((f32::NAN, f32::INFINITY)));

        assert!(!render
            .objects
            .iter()
            .any(|object| object.id == format!("player:{}", session.player().id)));
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
    fn view_window_bounds_handles_non_finite_player_position_without_origin_drift() {
        let bounds = super::view_window_bounds(8, 8, (f32::NAN, f32::INFINITY), (4, 4));

        assert_eq!(bounds, (2, 2, 4, 4));
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
