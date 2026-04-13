use crate::{
    panel_model::{MinimapPanelModel, PresenterViewWindow},
    render_model::{RenderPrimitivePayload, RenderPrimitivePayloadValue},
    BuildQueueHeadStage, RenderModel, RenderObject,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropWindowMode {
    PreserveBaseWithinMax,
    PreserveBaseWithinZoomed,
}

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

pub(crate) fn crop_window(
    scene: &RenderModel,
    tile_size: f32,
    viewport_width: usize,
    viewport_height: usize,
    max_view_tiles: Option<(usize, usize)>,
    mode: CropWindowMode,
) -> PresenterViewWindow {
    let base_window = projected_window(scene, viewport_width, viewport_height);
    let Some((max_width, max_height)) = max_view_tiles else {
        return base_window;
    };
    if matches!(mode, CropWindowMode::PreserveBaseWithinMax)
        && base_window.width <= max_width
        && base_window.height <= max_height
    {
        return base_window;
    }

    let zoom = normalize_zoom(scene.viewport.zoom);
    let window_width = zoomed_view_tile_span(max_width, zoom, base_window.width);
    let window_height = zoomed_view_tile_span(max_height, zoom, base_window.height);
    if base_window.width <= window_width && base_window.height <= window_height {
        return base_window;
    }

    crop_window_to_focus(scene, tile_size, base_window, window_width, window_height)
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
    focus
        .saturating_sub(half)
        .clamp(origin, max_origin)
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

    let tile_x = world_to_tile_index_floor(object.x, tile_size) as isize;
    let tile_y = world_to_tile_index_floor(object.y, tile_size) as isize;
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

pub(crate) fn render_line_is_visible(
    window: PresenterViewWindow,
    start_tile_x: i32,
    start_tile_y: i32,
    end_tile_x: i32,
    end_tile_y: i32,
) -> bool {
    let left_tile = start_tile_x.min(end_tile_x);
    let top_tile = start_tile_y.min(end_tile_y);
    let right_tile = start_tile_x.max(end_tile_x);
    let bottom_tile = start_tile_y.max(end_tile_y);
    render_rect_detail_is_visible(window, left_tile, top_tile, right_tile, bottom_tile)
}

pub(crate) fn render_rect_detail_is_visible(
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

pub(crate) fn tile_local_coords(
    tile_x: i32,
    tile_y: i32,
    window: PresenterViewWindow,
) -> Option<(usize, usize)> {
    let Ok(tile_x) = usize::try_from(tile_x) else {
        return None;
    };
    let Ok(tile_y) = usize::try_from(tile_y) else {
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

pub(crate) fn format_build_strip_queue_status_text(
    head_stage: Option<BuildQueueHeadStage>,
    pending_count: usize,
    idle_queue_text: Option<String>,
) -> String {
    if let Some(stage) = head_stage {
        format_build_queue_stage_text(stage, pending_count)
    } else if let Some(queue_text) = idle_queue_text {
        format!("{queue_text}/p{pending_count}")
    } else {
        format!("queued@{pending_count}")
    }
}

fn format_build_queue_stage_text(stage: BuildQueueHeadStage, pending_count: usize) -> String {
    let stage_text = match stage {
        BuildQueueHeadStage::Queued => "queued",
        BuildQueueHeadStage::InFlight => "flight",
        BuildQueueHeadStage::Finished => "finish",
        BuildQueueHeadStage::Removed => "remove",
    };
    format!("{stage_text}@{pending_count}")
}

pub(crate) fn compose_minimap_window_distribution_text(panel: &MinimapPanelModel) -> String {
    format!(
        "miniwin:tracked={}:outside={}:player={}:marker={}:plan={}:block={}:runtime={}:terrain={}:unknown={}",
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
        "miniwin-kinds: tracked={} outside={} player={} marker={} plan={} block={} runtime={} terrain={} unknown={}",
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

pub(crate) fn render_rect_detail_payload_fields(
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

#[cfg(test)]
mod tests {
    use super::{
        compose_minimap_window_distribution_text, compose_minimap_window_kind_distribution_text,
        crop_origin, crop_window, crop_window_to_focus, format_build_strip_queue_status_text,
        normalize_zoom, projected_window, render_line_is_visible, render_rect_detail_is_visible,
        render_rect_detail_payload_fields, tile_local_coords, visible_window_tile,
        world_rect_tile_coords, world_tile_coords, world_to_tile_index_floor,
        zoomed_view_tile_span, CropWindowMode,
    };
    use crate::{
        panel_model::{MinimapPanelModel, PresenterViewWindow},
        render_model::{RenderPrimitivePayload, RenderPrimitivePayloadValue},
        BuildQueueHeadStage, RenderModel, RenderObject, Viewport,
    };
    use std::collections::BTreeMap;

    const TILE_SIZE: f32 = 8.0;

    #[test]
    fn crop_window_to_focus_clamps_to_projected_bounds() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window: Some(crate::RenderViewWindow {
                origin_x: 3,
                origin_y: 4,
                width: 8,
                height: 6,
            }),
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 0,
                x: 80.0,
                y: 56.0,
            }],
        };

        let base = projected_window(&scene, 10, 10);
        let cropped = crop_window_to_focus(&scene, TILE_SIZE, base, 4, 4);

        assert_eq!(cropped.origin_x, 4);
        assert_eq!(cropped.origin_y, 2);
        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 4);
    }

    #[test]
    fn crop_window_preserves_base_window_when_mode_uses_max_bounds() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 2.0,
            },
            view_window: Some(crate::RenderViewWindow {
                origin_x: 1,
                origin_y: 1,
                width: 4,
                height: 4,
            }),
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 0,
                x: 24.0,
                y: 24.0,
            }],
        };

        let cropped = crop_window(
            &scene,
            TILE_SIZE,
            10,
            10,
            Some((4, 4)),
            CropWindowMode::PreserveBaseWithinMax,
        );

        assert_eq!(cropped.origin_x, 1);
        assert_eq!(cropped.origin_y, 1);
        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 4);
    }

    #[test]
    fn crop_window_applies_zoomed_bounds_when_mode_uses_zoomed_limit() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 2.0,
            },
            view_window: Some(crate::RenderViewWindow {
                origin_x: 1,
                origin_y: 1,
                width: 4,
                height: 4,
            }),
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 0,
                x: 24.0,
                y: 24.0,
            }],
        };

        let cropped = crop_window(
            &scene,
            TILE_SIZE,
            10,
            10,
            Some((4, 4)),
            CropWindowMode::PreserveBaseWithinZoomed,
        );

        assert_eq!(cropped.origin_x, 2);
        assert_eq!(cropped.origin_y, 2);
        assert_eq!(cropped.width, 2);
        assert_eq!(cropped.height, 2);
    }

    #[test]
    fn projected_window_clamps_scene_origin_to_viewport_bounds() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window: Some(crate::RenderViewWindow {
                origin_x: 12,
                origin_y: 13,
                width: 8,
                height: 6,
            }),
            objects: vec![],
        };

        let window = projected_window(&scene, 10, 10);

        assert_eq!(window.origin_x, 2);
        assert_eq!(window.origin_y, 4);
        assert_eq!(window.width, 8);
        assert_eq!(window.height, 6);
    }

    #[test]
    fn projected_window_clamps_zero_sized_window_origin() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window: Some(crate::RenderViewWindow {
                origin_x: 12,
                origin_y: 13,
                width: 0,
                height: 0,
            }),
            objects: vec![],
        };

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
        let object = RenderObject {
            id: "plan:build".to_string(),
            layer: 1,
            x: 40.0,
            y: 24.0,
        };

        let visible = visible_window_tile(&object, TILE_SIZE, 3, 2, 4, 4).unwrap();
        assert_eq!(visible.1, 2);
        assert_eq!(visible.2, 1);

        assert!(visible_window_tile(&object, TILE_SIZE, 6, 2, 4, 4).is_none());
        assert_eq!(world_to_tile_index_floor(40.0, TILE_SIZE), 5);
        assert_eq!(world_to_tile_index_floor(f32::NAN, TILE_SIZE), 0);
    }

    #[test]
    fn world_to_tile_index_floor_rejects_invalid_tile_size() {
        assert_eq!(world_to_tile_index_floor(40.0, 0.0), 0);
        assert_eq!(world_to_tile_index_floor(40.0, -8.0), 0);
        assert_eq!(world_to_tile_index_floor(40.0, f32::INFINITY), 0);
        assert_eq!(world_to_tile_index_floor(40.0, f32::NAN), 0);
    }

    #[test]
    fn world_tile_coords_rejects_non_finite_positions_and_invalid_tile_size() {
        assert_eq!(world_tile_coords(40.0, 24.0, TILE_SIZE), Some((5, 3)));
        assert_eq!(world_tile_coords(f32::NAN, 24.0, TILE_SIZE), None);
        assert_eq!(world_tile_coords(40.0, f32::INFINITY, TILE_SIZE), None);
        assert_eq!(world_tile_coords(40.0, 24.0, 0.0), None);
    }

    #[test]
    fn world_rect_tile_coords_rejects_non_finite_positions_and_invalid_tile_size() {
        assert_eq!(
            world_rect_tile_coords(8.0, 16.0, 24.0, 32.0, TILE_SIZE),
            Some((1, 2, 3, 4))
        );
        assert_eq!(
            world_rect_tile_coords(f32::NAN, 16.0, 24.0, 32.0, TILE_SIZE),
            None
        );
        assert_eq!(
            world_rect_tile_coords(8.0, 16.0, 24.0, f32::NEG_INFINITY, TILE_SIZE),
            None
        );
        assert_eq!(world_rect_tile_coords(8.0, 16.0, 24.0, 32.0, 0.0), None);
    }

    #[test]
    fn render_rect_detail_is_visible_tracks_window_overlap() {
        let window = PresenterViewWindow {
            origin_x: 2,
            origin_y: 3,
            width: 4,
            height: 3,
        };

        assert!(render_rect_detail_is_visible(window, 1, 2, 3, 4));
        assert!(!render_rect_detail_is_visible(window, -2, -1, 1, 2));
        assert!(!render_rect_detail_is_visible(window, 6, 3, 7, 4));
    }

    #[test]
    fn render_line_is_visible_uses_bounding_rect_overlap() {
        let window = PresenterViewWindow {
            origin_x: 2,
            origin_y: 3,
            width: 4,
            height: 3,
        };

        assert!(render_line_is_visible(window, -1, 4, 2, 4));
        assert!(!render_line_is_visible(window, -3, -2, -1, -1));
    }

    #[test]
    fn tile_local_coords_maps_visible_tiles_to_local_offsets() {
        let window = PresenterViewWindow {
            origin_x: 2,
            origin_y: 3,
            width: 4,
            height: 3,
        };

        assert_eq!(tile_local_coords(2, 3, window), Some((0, 0)));
        assert_eq!(tile_local_coords(5, 5, window), Some((3, 2)));
        assert_eq!(tile_local_coords(6, 3, window), None);
        assert_eq!(tile_local_coords(-1, 3, window), None);
    }

    #[test]
    fn format_build_strip_queue_status_text_prefers_head_stage() {
        assert_eq!(
            format_build_strip_queue_status_text(
                Some(BuildQueueHeadStage::InFlight),
                3,
                Some("idle".to_string())
            ),
            "flight@3"
        );
    }

    #[test]
    fn format_build_strip_queue_status_text_handles_idle_and_fallback_states() {
        assert_eq!(
            format_build_strip_queue_status_text(None, 4, Some("armed".to_string())),
            "armed/p4"
        );
        assert_eq!(
            format_build_strip_queue_status_text(None, 2, None),
            "queued@2"
        );
    }

    #[test]
    fn compose_minimap_window_distribution_text_preserves_compact_field_order() {
        let panel = sample_minimap_panel();
        assert_eq!(
            compose_minimap_window_distribution_text(&panel),
            "miniwin:tracked=12:outside=5:player=1:marker=2:plan=3:block=4:runtime=5:terrain=6:unknown=7"
        );
    }

    #[test]
    fn compose_minimap_window_kind_distribution_text_preserves_spaced_field_order() {
        let panel = sample_minimap_panel();
        assert_eq!(
            compose_minimap_window_kind_distribution_text(&panel),
            "miniwin-kinds: tracked=12 outside=5 player=1 marker=2 plan=3 block=4 runtime=5 terrain=6 unknown=7"
        );
    }

    #[test]
    fn render_rect_detail_payload_fields_extracts_named_values() {
        let payload = RenderPrimitivePayload {
            label: "rect".to_string(),
            fields: BTreeMap::from([
                (
                    "block_name",
                    RenderPrimitivePayloadValue::Text("duo".to_string()),
                ),
                ("tile_x", RenderPrimitivePayloadValue::I32(4)),
                ("tile_y", RenderPrimitivePayloadValue::I32(9)),
            ]),
        };

        assert_eq!(
            render_rect_detail_payload_fields(Some(&payload)),
            (Some("duo".to_string()), Some(4), Some(9))
        );
    }

    #[test]
    fn render_rect_detail_payload_fields_ignores_missing_or_mistyped_values() {
        let payload = RenderPrimitivePayload {
            label: "rect".to_string(),
            fields: BTreeMap::from([
                ("block_name", RenderPrimitivePayloadValue::I32(1)),
                ("tile_x", RenderPrimitivePayloadValue::Text("4".to_string())),
            ]),
        };

        assert_eq!(
            render_rect_detail_payload_fields(Some(&payload)),
            (None, None, None)
        );
        assert_eq!(render_rect_detail_payload_fields(None), (None, None, None));
    }

    #[test]
    fn visible_window_tile_rejects_non_finite_object_coordinates_and_tile_size() {
        let object = RenderObject {
            id: "plan:build".to_string(),
            layer: 1,
            x: f32::NAN,
            y: 24.0,
        };

        assert!(visible_window_tile(&object, TILE_SIZE, 0, 0, 4, 4).is_none());

        let object = RenderObject {
            id: "plan:build".to_string(),
            layer: 1,
            x: 16.0,
            y: 24.0,
        };

        assert!(visible_window_tile(&object, f32::INFINITY, 0, 0, 4, 4).is_none());
    }

    #[test]
    fn visible_window_tile_rejects_empty_window() {
        let object = RenderObject {
            id: "plan:build".to_string(),
            layer: 1,
            x: 40.0,
            y: 24.0,
        };

        assert!(visible_window_tile(&object, TILE_SIZE, 5, 3, 0, 4).is_none());
        assert!(visible_window_tile(&object, TILE_SIZE, 5, 3, 4, 0).is_none());
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

    fn sample_minimap_panel() -> MinimapPanelModel {
        MinimapPanelModel {
            map_width: 0,
            map_height: 0,
            window: PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 0,
                height: 0,
            },
            window_last_x: 0,
            window_last_y: 0,
            window_clamped_left: false,
            window_clamped_top: false,
            window_clamped_right: false,
            window_clamped_bottom: false,
            window_tile_count: 0,
            window_coverage_percent: 0,
            map_tile_count: 0,
            known_tile_count: 0,
            known_tile_percent: 0,
            unknown_tile_count: 0,
            unknown_tile_percent: 0,
            focus_tile: None,
            focus_in_window: None,
            focus_offset_x: None,
            focus_offset_y: None,
            overlay_visible: false,
            fog_enabled: false,
            visible_tile_count: 0,
            visible_known_percent: 0,
            hidden_tile_count: 0,
            hidden_known_percent: 0,
            tracked_object_count: 0,
            window_tracked_object_count: 12,
            outside_window_count: 5,
            player_count: 0,
            window_player_count: 1,
            marker_count: 0,
            window_marker_count: 2,
            plan_count: 0,
            window_plan_count: 3,
            block_count: 0,
            window_block_count: 4,
            runtime_count: 0,
            window_runtime_count: 5,
            terrain_count: 0,
            window_terrain_count: 6,
            unknown_count: 0,
            window_unknown_count: 7,
            detail_counts: Vec::new(),
        }
    }
}
