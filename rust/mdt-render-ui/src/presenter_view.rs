use crate::{
    panel_model::{MinimapPanelModel, PresenterViewWindow},
    render_model::RenderObjectSemanticKind,
    RenderModel, RenderObject,
};
use std::fmt::Display;

pub(crate) fn projected_window(
    scene: &RenderModel,
    viewport_width: usize,
    viewport_height: usize,
) -> PresenterViewWindow {
    scene
        .view_window
        .map(|window| PresenterViewWindow {
            origin_x: clamp_window_origin(window.origin_x, window.width, viewport_width),
            origin_y: clamp_window_origin(window.origin_y, window.height, viewport_height),
            width: window.width.min(viewport_width),
            height: window.height.min(viewport_height),
        })
        .unwrap_or(PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: viewport_width,
            height: viewport_height,
        })
}

pub(crate) enum CropWindowMode {
    MaxTiles,
    ZoomedTiles,
}

pub(crate) fn crop_window(
    scene: &RenderModel,
    tile_size: f32,
    width: usize,
    height: usize,
    max_view_tiles: Option<(usize, usize)>,
    mode: CropWindowMode,
) -> PresenterViewWindow {
    let base_window = projected_window(scene, width, height);
    let Some((max_width, max_height)) = max_view_tiles else {
        return base_window;
    };

    let zoom = normalize_zoom(scene.viewport.zoom);
    let window_width = zoomed_view_tile_span(max_width, zoom, base_window.width);
    let window_height = zoomed_view_tile_span(max_height, zoom, base_window.height);
    let fits = match mode {
        CropWindowMode::MaxTiles => {
            base_window.width <= max_width && base_window.height <= max_height
        }
        CropWindowMode::ZoomedTiles => {
            base_window.width <= window_width && base_window.height <= window_height
        }
    };
    if fits {
        return base_window;
    }

    crop_window_to_focus(scene, tile_size, base_window, window_width, window_height)
}

pub(crate) fn world_tile_coords(x: f32, y: f32, tile_size: f32) -> Option<(i32, i32)> {
    if !x.is_finite() || !y.is_finite() || !tile_size.is_finite() || tile_size <= 0.0 {
        return None;
    }

    Some((
        world_to_tile_index_floor(x, tile_size),
        world_to_tile_index_floor(y, tile_size),
    ))
}

pub(crate) fn world_rect_tile_coords(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    tile_size: f32,
) -> Option<(i32, i32, i32, i32)> {
    if !left.is_finite()
        || !top.is_finite()
        || !right.is_finite()
        || !bottom.is_finite()
        || !tile_size.is_finite()
        || tile_size <= 0.0
    {
        return None;
    }

    Some((
        world_to_tile_index_floor(left, tile_size),
        world_to_tile_index_floor(top, tile_size),
        world_to_tile_index_floor(right, tile_size),
        world_to_tile_index_floor(bottom, tile_size),
    ))
}

pub(crate) fn marker_line_end_base_id(object: &RenderObject) -> Option<&str> {
    if object.semantic_kind() != RenderObjectSemanticKind::MarkerLineEnd {
        return None;
    }

    object.id.strip_suffix(":line-end")
}

pub(crate) fn tile_in_window(
    tile_x: i32,
    tile_y: i32,
    window: PresenterViewWindow,
) -> Option<(usize, usize)> {
    let (Ok(tile_x), Ok(tile_y)) = (usize::try_from(tile_x), usize::try_from(tile_y)) else {
        return None;
    };
    if tile_x < window.origin_x
        || tile_y < window.origin_y
        || tile_x >= window.origin_x.saturating_add(window.width)
        || tile_y >= window.origin_y.saturating_add(window.height)
    {
        return None;
    }

    Some((tile_x - window.origin_x, tile_y - window.origin_y))
}

pub(crate) fn rect_in_window(
    left_tile: i32,
    top_tile: i32,
    right_tile: i32,
    bottom_tile: i32,
    window: PresenterViewWindow,
) -> bool {
    !(right_tile < window.origin_x as i32
        || bottom_tile < window.origin_y as i32
        || left_tile >= window.origin_x.saturating_add(window.width) as i32
        || top_tile >= window.origin_y.saturating_add(window.height) as i32)
}

pub(crate) fn render_pipeline_summary(
    scene: &RenderModel,
    window: PresenterViewWindow,
    tile_size: f32,
) -> Option<crate::render_model::RenderPipelineSummary> {
    if scene.objects.is_empty() {
        return None;
    }

    Some(scene.pipeline_summary_for_window(
        tile_size,
        crate::RenderViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width,
            height: window.height,
        },
    ))
}

pub(crate) fn semantic_detail_text(
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

pub(crate) fn build_queue_head_stage_text(stage: crate::BuildQueueHeadStage) -> &'static str {
    match stage {
        crate::BuildQueueHeadStage::Queued => "queued",
        crate::BuildQueueHeadStage::InFlight => "flight",
        crate::BuildQueueHeadStage::Finished => "finish",
        crate::BuildQueueHeadStage::Removed => "remove",
    }
}

pub(crate) fn build_queue_head_text(head: Option<&crate::BuildQueueHeadObservability>) -> String {
    let Some(head) = head else {
        return "none".to_string();
    };

    let mode = if head.breaking { "break" } else { "place" };
    format!(
        "{}@{}:{}:{mode}:b{}:r{}",
        build_queue_head_stage_text(head.stage),
        head.x,
        head.y,
        optional_i16_label(head.block_id),
        optional_u8_label(head.rotation),
    )
}

pub(crate) fn build_strip_queue_text(
    queue_text: &str,
    head_stage: Option<crate::BuildQueueHeadStage>,
    pending_count: usize,
) -> String {
    let queue_text = head_stage
        .map(build_queue_head_stage_text)
        .unwrap_or(queue_text);
    format!("{queue_text}/p{pending_count}")
}

pub(crate) fn build_strip_queue_fallback_text(
    head_stage: Option<crate::BuildQueueHeadStage>,
    queued_count: usize,
) -> String {
    head_stage
        .map(|stage| format!("{}/p{queued_count}", build_queue_head_stage_text(stage)))
        .unwrap_or_else(|| format!("queued/p{queued_count}"))
}

pub(crate) fn command_rect_text(value: Option<crate::RuntimeCommandRectObservability>) -> String {
    value
        .map(|rect| format!("{}:{}:{}:{}", rect.x0, rect.y0, rect.x1, rect.y1))
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn compact_runtime_ui_text(value: Option<&str>) -> String {
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

pub(crate) fn runtime_ui_text_len(value: Option<&str>) -> usize {
    value
        .map(str::chars)
        .map(Iterator::count)
        .unwrap_or_default()
}

pub(crate) fn runtime_ui_uri_scheme(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .and_then(|uri| uri.split_once(':').map(|(scheme, _)| scheme.trim()))
        .filter(|scheme| !scheme.is_empty())
        .map(|scheme| compact_runtime_ui_text(Some(scheme)))
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn runtime_layer_labels_text(labels: Vec<&str>) -> String {
    let labels = labels.join(">");
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels
    }
}

fn optional_i16_label(value: Option<i16>) -> String {
    optional_display_label(value)
}

fn optional_u8_label(value: Option<u8>) -> String {
    optional_display_label(value)
}

pub(crate) fn optional_display_label<T: Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn compose_minimap_window_distribution_text(panel: &MinimapPanelModel) -> String {
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

pub(crate) fn compose_minimap_window_kind_distribution_text(panel: &MinimapPanelModel) -> String {
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

pub(crate) fn crop_window_to_focus(
    scene: &RenderModel,
    tile_size: f32,
    base_window: PresenterViewWindow,
    window_width: usize,
    window_height: usize,
) -> PresenterViewWindow {
    let focus = scene.player_focus_tile(tile_size).unwrap_or((
        base_window.origin_x.saturating_add(base_window.width / 2),
        base_window.origin_y.saturating_add(base_window.height / 2),
    ));

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

pub(crate) fn crop_origin(focus: usize, origin: usize, bound: usize, window: usize) -> usize {
    let half = window / 2;
    let max_origin = bound.saturating_sub(window);
    let origin = origin.min(max_origin);
    focus.saturating_sub(half).clamp(origin, max_origin)
}

fn clamp_window_origin(origin: usize, window: usize, bound: usize) -> usize {
    if bound == 0 {
        return 0;
    }

    origin.min(bound.saturating_sub(window.max(1)))
}

pub(crate) fn visible_window_tile(
    object: &RenderObject,
    tile_size: f32,
    window_x: usize,
    window_y: usize,
    window_width: usize,
    window_height: usize,
) -> Option<(&RenderObject, usize, usize)> {
    if !tile_size.is_finite() || tile_size <= 0.0 || !object.x.is_finite() || !object.y.is_finite()
    {
        return None;
    }

    let (tile_x, tile_y) = world_tile_coords(object.x, object.y, tile_size)?;
    let window = PresenterViewWindow {
        origin_x: window_x,
        origin_y: window_y,
        width: window_width,
        height: window_height,
    };
    let (local_x, local_y) = tile_in_window(tile_x, tile_y, window)?;

    Some((object, local_x, local_y))
}

pub(crate) fn normalize_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    }
}

pub(crate) fn zoomed_view_tile_span(max_tiles: usize, zoom: f32, bound: usize) -> usize {
    let max_tiles = max_tiles.max(1);
    let desired = ((max_tiles as f32) / zoom).floor().max(1.0) as usize;
    desired.min(bound.max(1))
}

pub(crate) fn world_to_tile_index_floor(world_position: f32, tile_size: f32) -> i32 {
    if !world_position.is_finite() {
        return 0;
    }
    if !tile_size.is_finite() || tile_size <= 0.0 {
        return 0;
    }
    (world_position / tile_size).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::{
        build_queue_head_stage_text, build_queue_head_text, build_strip_queue_fallback_text,
        build_strip_queue_text, command_rect_text, compact_runtime_ui_text, crop_origin,
        crop_window_to_focus, marker_line_end_base_id, normalize_zoom, projected_window,
        rect_in_window, render_pipeline_summary, runtime_layer_labels_text, runtime_ui_text_len,
        runtime_ui_uri_scheme, semantic_detail_text, tile_in_window, visible_window_tile,
        world_rect_tile_coords, world_tile_coords, world_to_tile_index_floor,
        zoomed_view_tile_span, PresenterViewWindow,
    };
    use crate::{
        panel_model::RuntimeWorldLabelPanelModel, render_model::RenderSemanticDetailCount,
        BuildQueueHeadObservability, BuildQueueHeadStage, RenderModel, RenderObject,
        RuntimeWorldPositionObservability, Viewport,
    };

    const TILE_SIZE: f32 = 8.0;

    fn test_scene(
        view_window: Option<crate::RenderViewWindow>,
        objects: Vec<RenderObject>,
    ) -> RenderModel {
        RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window,
            objects,
        }
    }

    fn render_view_window(
        origin_x: usize,
        origin_y: usize,
        width: usize,
        height: usize,
    ) -> crate::RenderViewWindow {
        crate::RenderViewWindow {
            origin_x,
            origin_y,
            width,
            height,
        }
    }

    fn presenter_window(
        origin_x: usize,
        origin_y: usize,
        width: usize,
        height: usize,
    ) -> PresenterViewWindow {
        PresenterViewWindow {
            origin_x,
            origin_y,
            width,
            height,
        }
    }

    fn render_object(id: &str, layer: i32, x: f32, y: f32) -> RenderObject {
        RenderObject {
            id: id.to_string(),
            layer,
            x,
            y,
        }
    }

    #[test]
    fn crop_window_to_focus_clamps_to_projected_bounds() {
        let scene = test_scene(
            Some(render_view_window(3, 4, 8, 6)),
            vec![render_object("player:1", 0, 80.0, 56.0)],
        );

        let base = projected_window(&scene, 10, 10);
        let cropped = crop_window_to_focus(&scene, TILE_SIZE, base, 4, 4);

        assert_eq!(cropped.origin_x, 4);
        assert_eq!(cropped.origin_y, 2);
        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 4);
    }

    #[test]
    fn projected_window_clamps_scene_origin_to_viewport_bounds() {
        let scene = test_scene(Some(render_view_window(12, 13, 8, 6)), vec![]);

        let window = projected_window(&scene, 10, 10);

        assert_eq!(window.origin_x, 2);
        assert_eq!(window.origin_y, 4);
        assert_eq!(window.width, 8);
        assert_eq!(window.height, 6);
    }

    #[test]
    fn build_queue_head_text_formats_head_without_changing_semantics() {
        let head = BuildQueueHeadObservability {
            x: 100,
            y: 99,
            block_id: Some(301),
            rotation: Some(1),
            breaking: false,
            stage: BuildQueueHeadStage::InFlight,
        };

        assert_eq!(
            build_queue_head_text(Some(&head)),
            "flight@100:99:place:b301:r1"
        );
        assert_eq!(build_queue_head_text(None), "none");
    }

    #[test]
    fn projected_window_clamps_zero_sized_window_origin() {
        let scene = test_scene(Some(render_view_window(12, 13, 0, 0)), vec![]);

        let window = projected_window(&scene, 10, 10);

        assert_eq!(window.origin_x, 9);
        assert_eq!(window.origin_y, 9);
        assert_eq!(window.width, 0);
        assert_eq!(window.height, 0);
    }

    #[test]
    fn crop_origin_clamps_invalid_origin_to_window_bounds() {
        assert_eq!(crop_origin(7, 12, 8, 4), 4);
    }

    #[test]
    fn visible_window_tile_uses_tile_flooring_and_window_origin() {
        let object = render_object("plan:build", 1, 40.0, 24.0);

        let visible = visible_window_tile(&object, TILE_SIZE, 3, 2, 4, 4).unwrap();
        assert_eq!(visible.1, 2);
        assert_eq!(visible.2, 1);

        assert!(visible_window_tile(&object, TILE_SIZE, 6, 2, 4, 4).is_none());
        assert_eq!(world_to_tile_index_floor(40.0, TILE_SIZE), 5);
        assert_eq!(world_to_tile_index_floor(f32::NAN, TILE_SIZE), 0);
    }

    #[test]
    fn world_tile_coords_converts_finite_world_point_to_tiles() {
        assert_eq!(world_tile_coords(40.0, 24.0, TILE_SIZE), Some((5, 3)));
    }

    #[test]
    fn world_to_tile_index_floor_rejects_invalid_tile_size() {
        assert_eq!(world_to_tile_index_floor(40.0, 0.0), 0);
        assert_eq!(world_to_tile_index_floor(40.0, -8.0), 0);
        assert_eq!(world_to_tile_index_floor(40.0, f32::INFINITY), 0);
        assert_eq!(world_to_tile_index_floor(40.0, f32::NAN), 0);
    }

    #[test]
    fn world_tile_coords_rejects_non_finite_inputs() {
        assert_eq!(world_tile_coords(f32::NAN, 24.0, TILE_SIZE), None);
        assert_eq!(world_tile_coords(40.0, f32::INFINITY, TILE_SIZE), None);
        assert_eq!(world_tile_coords(40.0, 24.0, f32::NAN), None);
        assert_eq!(world_tile_coords(40.0, 24.0, 0.0), None);
    }

    #[test]
    fn world_rect_tile_coords_converts_finite_world_rect_to_tiles() {
        assert_eq!(
            world_rect_tile_coords(8.0, 16.0, 40.0, 56.0, TILE_SIZE),
            Some((1, 2, 5, 7))
        );
    }

    #[test]
    fn world_rect_tile_coords_rejects_non_finite_inputs() {
        assert_eq!(
            world_rect_tile_coords(f32::NAN, 16.0, 40.0, 56.0, TILE_SIZE),
            None
        );
        assert_eq!(world_rect_tile_coords(8.0, 16.0, 40.0, 56.0, 0.0), None);
    }

    #[test]
    fn marker_line_end_base_id_only_matches_marker_line_end_objects() {
        let line_end = render_object("marker:line:demo:line-end", 1, 8.0, 0.0);
        let line = render_object("marker:line:demo", 1, 0.0, 0.0);

        assert_eq!(marker_line_end_base_id(&line_end), Some("marker:line:demo"));
        assert_eq!(marker_line_end_base_id(&line), None);
    }

    #[test]
    fn tile_in_window_checks_bounds_and_negative_tiles() {
        let window = presenter_window(3, 2, 4, 4);
        assert_eq!(tile_in_window(3, 2, window), Some((0, 0)));
        assert_eq!(tile_in_window(6, 5, window), Some((3, 3)));
        assert_eq!(tile_in_window(2, 2, window), None);
        assert_eq!(tile_in_window(-1, 2, window), None);
    }

    #[test]
    fn rect_in_window_checks_overlap_using_tile_bounds() {
        let window = presenter_window(3, 2, 4, 4);
        assert!(rect_in_window(2, 2, 3, 3, window));
        assert!(rect_in_window(6, 5, 7, 6, window));
        assert!(!rect_in_window(0, 0, 2, 1, window));
    }

    #[test]
    fn render_pipeline_summary_returns_none_for_empty_scene() {
        let scene = test_scene(None, vec![]);

        assert_eq!(
            render_pipeline_summary(&scene, presenter_window(0, 0, 4, 4), TILE_SIZE,),
            None
        );
    }

    #[test]
    fn semantic_detail_text_joins_label_and_count_pairs() {
        assert_eq!(
            semantic_detail_text(&[
                RenderSemanticDetailCount {
                    label: "player",
                    count: 1,
                },
                RenderSemanticDetailCount {
                    label: "marker",
                    count: 2,
                },
            ]),
            Some("player:1,marker:2".to_string())
        );
        assert_eq!(semantic_detail_text(&[]), None);
    }

    #[test]
    fn build_strip_queue_helpers_match_stage_and_fallback_text() {
        assert_eq!(
            build_queue_head_stage_text(crate::BuildQueueHeadStage::Queued),
            "queued"
        );
        assert_eq!(
            build_strip_queue_text("mixed", Some(crate::BuildQueueHeadStage::InFlight), 3),
            "flight/p3"
        );
        assert_eq!(build_strip_queue_text("mixed", None, 3), "mixed/p3");
        assert_eq!(
            build_strip_queue_fallback_text(Some(crate::BuildQueueHeadStage::Finished), 3),
            "finish/p3"
        );
        assert_eq!(build_strip_queue_fallback_text(None, 3), "queued/p3");
    }

    #[test]
    fn command_rect_text_formats_present_value() {
        assert_eq!(
            command_rect_text(Some(crate::RuntimeCommandRectObservability {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            })),
            "-3:4:12:18"
        );
    }

    #[test]
    fn command_rect_text_returns_none_for_absent_value() {
        assert_eq!(command_rect_text(None), "none");
    }

    #[test]
    fn compact_runtime_ui_text_replaces_whitespace_and_truncates() {
        assert_eq!(
            compact_runtime_ui_text(Some("hello world:12\n345")),
            "hello_world_~"
        );
        assert_eq!(compact_runtime_ui_text(Some("")), "-");
        assert_eq!(compact_runtime_ui_text(None), "none");
    }

    #[test]
    fn visible_window_tile_rejects_non_finite_object_coordinates_and_tile_size() {
        let object = render_object("plan:build", 1, f32::NAN, 24.0);

        assert!(visible_window_tile(&object, TILE_SIZE, 0, 0, 4, 4).is_none());

        let object = render_object("plan:build", 1, 16.0, 24.0);

        assert!(visible_window_tile(&object, f32::INFINITY, 0, 0, 4, 4).is_none());
    }

    #[test]
    fn visible_window_tile_rejects_empty_window() {
        let object = render_object("plan:build", 1, 40.0, 24.0);

        assert!(visible_window_tile(&object, TILE_SIZE, 5, 3, 0, 4).is_none());
        assert!(visible_window_tile(&object, TILE_SIZE, 5, 3, 4, 0).is_none());
    }

    #[test]
    fn visible_window_tile_excludes_tiles_on_window_max_edge() {
        let right_edge = render_object("plan:right-edge", 1, 56.0, 40.0);
        let bottom_edge = render_object("plan:bottom-edge", 1, 40.0, 48.0);

        assert!(visible_window_tile(&right_edge, TILE_SIZE, 3, 2, 4, 4).is_none());
        assert!(visible_window_tile(&bottom_edge, TILE_SIZE, 3, 2, 4, 4).is_none());
    }

    #[test]
    fn zoom_helpers_fall_back_safely() {
        assert_eq!(normalize_zoom(0.0), 1.0);
        assert_eq!(normalize_zoom(-2.0), 1.0);
        assert_eq!(normalize_zoom(2.5), 2.5);

        assert_eq!(zoomed_view_tile_span(0, 2.0, 10), 1);
        assert_eq!(zoomed_view_tile_span(8, 2.0, 10), 4);
        assert_eq!(zoomed_view_tile_span(8, 0.5, 6), 6);
    }

    #[test]
    fn runtime_ui_uri_scheme_rejects_empty_and_colonless_values() {
        for uri in ["", "noscheme", "://example.com"] {
            assert_eq!(runtime_ui_uri_scheme(Some(uri)), "none");
        }
        assert_eq!(runtime_ui_uri_scheme(Some("https://example.com")), "https");
    }

    #[test]
    fn runtime_ui_uri_scheme_trims_whitespace_around_the_uri() {
        assert_eq!(
            runtime_ui_uri_scheme(Some("  https://example.com  ")),
            "https"
        );
    }

    #[test]
    fn runtime_ui_text_len_counts_unicode_scalars_not_bytes() {
        assert_eq!(runtime_ui_text_len(Some("你😀a")), 3);
        assert_eq!(runtime_ui_text_len(None), 0);
    }

    #[test]
    fn runtime_layer_labels_text_joins_labels_and_falls_back_to_none() {
        assert_eq!(
            runtime_layer_labels_text(vec!["input", "follow-up"]),
            "input>follow-up"
        );
        assert_eq!(runtime_layer_labels_text(Vec::new()), "none");
    }

    #[test]
    fn runtime_world_label_panel_model_is_empty_detects_default_state() {
        let panel = RuntimeWorldLabelPanelModel {
            label_count: 0,
            reliable_label_count: 0,
            remove_label_count: 0,
            total_count: 0,
            active_count: 0,
            inactive_count: 0,
            last_entity_id: None,
            last_text: None,
            last_flags: None,
            last_font_size_bits: None,
            last_z_bits: None,
            last_position: None,
        };

        assert!(panel.is_empty());
    }

    #[test]
    fn runtime_world_label_panel_model_is_empty_rejects_single_active_field() {
        let panel = RuntimeWorldLabelPanelModel {
            label_count: 0,
            reliable_label_count: 0,
            remove_label_count: 0,
            total_count: 0,
            active_count: 0,
            inactive_count: 0,
            last_entity_id: None,
            last_text: None,
            last_flags: None,
            last_font_size_bits: None,
            last_z_bits: None,
            last_position: Some(RuntimeWorldPositionObservability {
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
            }),
        };

        assert!(!panel.is_empty());
    }
}
