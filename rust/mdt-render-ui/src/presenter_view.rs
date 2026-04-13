use crate::{panel_model::PresenterViewWindow, RenderModel, RenderObject};

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

#[cfg(test)]
mod tests {
    use super::{
        crop_origin, crop_window, crop_window_to_focus, normalize_zoom, projected_window,
        visible_window_tile, world_rect_tile_coords, world_tile_coords,
        world_to_tile_index_floor, zoomed_view_tile_span, CropWindowMode,
    };
    use crate::{RenderModel, RenderObject, Viewport};

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
}
