use crate::{
    hud_model::{HudModel, HudSummary, RuntimeUiStackDepthSummary, RuntimeUiStackSummary},
    panel_model::{
        build_runtime_admin_panel,
        build_runtime_bootstrap_panel,
        build_runtime_core_binding_panel,
        build_runtime_live_effect_panel, build_runtime_live_entity_panel,
        build_runtime_kick_panel,
        build_runtime_loading_panel,
        build_runtime_menu_panel,
        build_runtime_marker_panel,
        build_runtime_reconnect_panel,
        build_runtime_rules_panel,
        build_runtime_session_panel, build_runtime_world_label_panel, HudVisibilityPanelModel,
        MinimapPanelModel,
        PresenterViewWindow, RuntimeAdminPanelModel, RuntimeChatPanelModel,
        RuntimeChoicePanelModel,
        RuntimeCommandControlGroupPanelModel, RuntimeCommandModePanelModel,
        RuntimeCoreBindingPanelModel, RuntimeDialogNoticeKind, RuntimeDialogPanelModel,
        RuntimeDialogPromptKind, RuntimeDialogStackPanelModel, RuntimeKickPanelModel,
        RuntimeBootstrapPanelModel, RuntimeLoadingPanelModel, RuntimeLiveEffectPanelModel,
        RuntimeLiveEntityPanelModel,
        RuntimeMarkerPanelModel, RuntimeMenuPanelModel, RuntimeNoticeStatePanelModel,
        RuntimePromptPanelModel,
        RuntimeReconnectPanelModel, RuntimeResourceDeltaPanelModel,
        RuntimeRulesPanelModel, RuntimeSessionPanelModel,
        RuntimeUiNoticePanelModel, RuntimeUiStackPanelModel, RuntimeWorldLabelPanelModel,
        RuntimeWorldReloadPanelModel,
    },
    render_model::{RenderPrimitivePayload, RenderPrimitivePayloadValue, RenderSemanticDetailCount},
    BuildQueueHeadStage, RenderModel, RenderObject,
    RuntimeCommandRecentControlGroupOperationObservability, RuntimeCommandRectObservability,
    RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind,
    RuntimeSessionResetKind, RuntimeSessionTimeoutKind,
    RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
    RuntimeCommandUnitRefObservability, RuntimeLiveEffectPositionSource,
    RuntimeLiveEffectSummaryObservability,
    RuntimeLiveEntitySummaryObservability,
    RuntimeWorldPositionObservability,
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

pub(crate) fn format_build_config_alignment_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "match",
        Some(false) => "split",
        None => "none",
    }
}

pub(crate) fn compose_minimap_window_distribution_text(panel: &MinimapPanelModel) -> String {
    format_minimap_window_counts_text("miniwin:", ":", panel)
}

pub(crate) fn compose_minimap_window_kind_distribution_text(panel: &MinimapPanelModel) -> String {
    format_minimap_window_counts_text("miniwin-kinds: ", " ", panel)
}

pub(crate) fn format_minimap_kind_text(panel: &MinimapPanelModel) -> String {
    format!(
        "minikind:obj{}@pl{}:mk{}:pn{}:bk{}:rt{}:tr{}:uk{}",
        panel.tracked_object_count,
        panel.player_count,
        panel.marker_count,
        panel.plan_count,
        panel.block_count,
        panel.runtime_count,
        panel.terrain_count,
        panel.unknown_count,
    )
}

fn format_minimap_window_counts_text(
    prefix: &str,
    separator: &str,
    panel: &MinimapPanelModel,
) -> String {
    format!(
        "{prefix}tracked={}{}outside={}{}player={}{}marker={}{}plan={}{}block={}{}runtime={}{}terrain={}{}unknown={}",
        panel.window_tracked_object_count,
        separator,
        panel.outside_window_count,
        separator,
        panel.window_player_count,
        separator,
        panel.window_marker_count,
        separator,
        panel.window_plan_count,
        separator,
        panel.window_block_count,
        separator,
        panel.window_runtime_count,
        separator,
        panel.window_terrain_count,
        separator,
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

pub(crate) fn format_render_primitive_payload_fields_with<F>(
    payload: &RenderPrimitivePayload,
    mut format_value: F,
) -> String
where
    F: FnMut(&str, &RenderPrimitivePayloadValue) -> String,
{
    let mut parts = Vec::new();
    if let Some(variant) = payload.field("variant") {
        parts.push(format!("variant={}", format_value("variant", variant)));
    }
    for (field_name, field_value) in &payload.fields {
        if *field_name == "variant" {
            continue;
        }
        parts.push(format!(
            "{field_name}={}",
            format_value(field_name, field_value)
        ));
    }
    parts.join(",")
}

pub(crate) fn format_render_primitive_payload_value_with<Fb, Fu32>(
    field_name: &str,
    value: &RenderPrimitivePayloadValue,
    format_bool: Fb,
    format_u32: Fu32,
) -> String
where
    Fb: FnOnce(bool) -> String,
    Fu32: FnOnce(&str, u32) -> String,
{
    match value {
        RenderPrimitivePayloadValue::Bool(value) => format_bool(*value),
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
        RenderPrimitivePayloadValue::U32(value) => format_u32(field_name, *value),
        RenderPrimitivePayloadValue::Usize(value) => value.to_string(),
        RenderPrimitivePayloadValue::Text(value) => value.clone(),
        RenderPrimitivePayloadValue::TextList(values) => format!("[{}]", values.join(",")),
    }
}

pub(crate) fn format_render_line_signature(
    label: &str,
    layer: i32,
    start_tile_x: i32,
    start_tile_y: i32,
    end_tile_x: i32,
    end_tile_y: i32,
) -> String {
    format!("{label}@{layer}:{start_tile_x}:{start_tile_y}->{end_tile_x}:{end_tile_y}")
}

pub(crate) fn format_render_rect_signature(
    family: &str,
    layer: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> String {
    format!("{family}@{layer}:{left}:{top}:{right}:{bottom}")
}

pub(crate) fn format_render_icon_signature(
    family_label: &str,
    variant: &str,
    layer: i32,
    tile_x: i32,
    tile_y: i32,
) -> String {
    format!("{family_label}/{variant}@{layer}:{tile_x}:{tile_y}")
}

pub(crate) fn format_world_position_status_text(
    value: Option<&RuntimeWorldPositionObservability>,
) -> String {
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

pub(crate) fn format_runtime_world_label_sample_text(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    let sanitized = value.replace(' ', "_");
    let sample = sanitized.chars().take(24).collect::<String>();
    if sanitized.chars().count() > 24 {
        format!("{sample}~")
    } else {
        sample
    }
}

pub(crate) fn format_runtime_world_label_scalar_text(
    bits: Option<u32>,
    value: Option<f32>,
) -> String {
    match (bits, value) {
        (Some(bits), Some(value)) => format!("{bits}@{value:.1}"),
        (Some(bits), None) => bits.to_string(),
        (None, _) => "none".to_string(),
    }
}

pub(crate) fn runtime_ui_uri_scheme(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .and_then(|uri| uri.split_once(':').map(|(scheme, _)| scheme.trim()))
        .filter(|scheme| !scheme.is_empty())
        .map(|scheme| compact_runtime_ui_text(Some(scheme)))
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn runtime_ui_notice_panel_is_empty(panel: &RuntimeUiNoticePanelModel) -> bool {
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

pub(crate) fn format_runtime_ui_notice_panel_text(panel: &RuntimeUiNoticePanelModel) -> String {
    format!(
        "notice:hud={}/{}/{}@{}/{}:ann={}@{}:info={}@{}:toast={}/{}@{}/{}:popup={}/{}@{}:{}/{}:clip={}@{}:uri={}@{}:{}:tin={}@{}:{}/{}/{}#{}:n{}:e{}",
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
        format_optional_bool_flag(panel.last_info_popup_reliable),
        compact_runtime_ui_text(panel.last_info_popup_id.as_deref()),
        compact_runtime_ui_text(panel.last_info_popup_message.as_deref()),
        panel.clipboard_count,
        compact_runtime_ui_text(panel.last_clipboard_text.as_deref()),
        panel.open_uri_count,
        compact_runtime_ui_text(panel.last_open_uri.as_deref()),
        runtime_ui_uri_scheme(panel.last_open_uri.as_deref()),
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
    )
}

pub(crate) fn format_runtime_ui_notice_detail_text(
    panel: &RuntimeUiNoticePanelModel,
) -> Option<String> {
    if runtime_ui_notice_panel_is_empty(panel) {
        return None;
    }

    Some(format!(
        "noticed:a1:h{}/{}/{}:l{}/{}:ann{}:a{}:info{}:i{}:t{}/{}:l{}/{}:popup{}/{}:r{}:pid{}:pm{}:pd{}:pb{}:{}:{}:{}:{}:clip{}:{}:uri{}:{}:{}:tin{}:id{}:t{}:m{}:d{}:n{}:e{}",
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
        format_optional_bool_flag(panel.last_info_popup_reliable),
        runtime_ui_text_len(panel.last_info_popup_id.as_deref()),
        runtime_ui_text_len(panel.last_info_popup_message.as_deref()),
        format_optional_u32_text(panel.last_info_popup_duration_bits),
        format_optional_i32_text(panel.last_info_popup_align),
        format_optional_i32_text(panel.last_info_popup_top),
        format_optional_i32_text(panel.last_info_popup_left),
        format_optional_i32_text(panel.last_info_popup_bottom),
        format_optional_i32_text(panel.last_info_popup_right),
        panel.clipboard_count,
        runtime_ui_text_len(panel.last_clipboard_text.as_deref()),
        panel.open_uri_count,
        runtime_ui_text_len(panel.last_open_uri.as_deref()),
        runtime_ui_uri_scheme(panel.last_open_uri.as_deref()),
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        runtime_ui_text_len(panel.text_input_last_message.as_deref()),
        runtime_ui_text_len(panel.text_input_last_default_text.as_deref()),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
    ))
}

pub(crate) fn format_hud_visibility_detail_text(
    summary: &HudSummary,
    visibility: &HudVisibilityPanelModel,
) -> String {
    format!(
        "hudvisd:s={}:ov={}:fg={}:k={}/{}:v={}/{}:h={}/{}:u={}/{}",
        summary.visibility_label(),
        summary.overlay_label(),
        summary.fog_label(),
        visibility.known_tile_count,
        summary.map_tile_count(),
        visibility.visible_tile_count,
        visibility.known_tile_count,
        visibility.hidden_tile_count,
        visibility.known_tile_count,
        visibility.unknown_tile_count,
        summary.map_tile_count(),
    )
}

struct HudVisibilityMetrics {
    overlay_visible: u8,
    fog_enabled: u8,
    known_tile_count: usize,
    known_tile_percent: usize,
    visible_tile_count: usize,
    visible_known_percent: usize,
    hidden_tile_count: usize,
    hidden_known_percent: usize,
    unknown_tile_count: usize,
    unknown_tile_percent: usize,
    visible_map_percent: usize,
    hidden_map_percent: usize,
}

fn hud_visibility_metrics(visibility: &HudVisibilityPanelModel) -> HudVisibilityMetrics {
    HudVisibilityMetrics {
        overlay_visible: u8::from(visibility.overlay_visible),
        fog_enabled: u8::from(visibility.fog_enabled),
        known_tile_count: visibility.known_tile_count,
        known_tile_percent: visibility.known_tile_percent,
        visible_tile_count: visibility.visible_tile_count,
        visible_known_percent: visibility.visible_known_percent,
        hidden_tile_count: visibility.hidden_tile_count,
        hidden_known_percent: visibility.hidden_known_percent,
        unknown_tile_count: visibility.unknown_tile_count,
        unknown_tile_percent: visibility.unknown_tile_percent,
        visible_map_percent: visibility.visible_map_percent(),
        hidden_map_percent: visibility.hidden_map_percent(),
    }
}

pub(crate) fn format_hud_visibility_text(visibility: &HudVisibilityPanelModel) -> String {
    let metrics = hud_visibility_metrics(visibility);
    format!(
        "overlay={} fog={} known={}({}%) vis={}({}%) hid={}({}%) unseen={}({}%) vis-map={}% hid-map={}%",
        metrics.overlay_visible,
        metrics.fog_enabled,
        metrics.known_tile_count,
        metrics.known_tile_percent,
        metrics.visible_tile_count,
        metrics.visible_known_percent,
        metrics.hidden_tile_count,
        metrics.hidden_known_percent,
        metrics.unknown_tile_count,
        metrics.unknown_tile_percent,
        metrics.visible_map_percent,
        metrics.hidden_map_percent,
    )
}

pub(crate) fn format_hud_visibility_status_text(visibility: &HudVisibilityPanelModel) -> String {
    let metrics = hud_visibility_metrics(visibility);
    format!(
        "hudvis:ov{}:fg{}:k{}p{}:v{}p{}:h{}p{}:u{}p{}:vm{}:hm{}",
        metrics.overlay_visible,
        metrics.fog_enabled,
        metrics.known_tile_count,
        metrics.known_tile_percent,
        metrics.visible_tile_count,
        metrics.visible_known_percent,
        metrics.hidden_tile_count,
        metrics.hidden_known_percent,
        metrics.unknown_tile_count,
        metrics.unknown_tile_percent,
        metrics.visible_map_percent,
        metrics.hidden_map_percent,
    )
}

pub(crate) fn format_minimap_visibility_detail_text(minimap: &MinimapPanelModel) -> String {
    format!(
        "minivisd:v={}:c={}:md{}:wd{}:od{}:vp={}",
        minimap.visibility_label(),
        minimap.coverage_label(),
        minimap.map_object_density_percent(),
        minimap.window_object_density_percent(),
        minimap.outside_object_percent(),
        minimap.viewport_band(),
    )
}

pub(crate) fn format_visibility_minimap_text(
    visibility: &HudVisibilityPanelModel,
    minimap: &MinimapPanelModel,
) -> String {
    format!(
        "overlay={} fog={} known={}({}%) vis={}({}%/{}%) hid={}({}%/{}%) map={}x{} window={}:{}->{}:{} size={}x{} cover={}/{}({}%) focus={} in-window={}",
        u8::from(visibility.overlay_visible),
        u8::from(visibility.fog_enabled),
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
        format_optional_focus_tile_text(minimap.focus_tile),
        format_optional_bool_flag(minimap.focus_in_window),
    )
}

pub(crate) fn format_minimap_density_visibility_text(panel: &MinimapPanelModel) -> String {
    format!(
        "minidv:ov{}:fg{}:cov{}:mapd{}:wind{}:out{}",
        u8::from(panel.overlay_visible),
        u8::from(panel.fog_enabled),
        panel.window_coverage_percent,
        panel.map_object_density_percent(),
        panel.window_object_density_percent(),
        panel.outside_object_percent(),
    )
}

pub(crate) fn format_minimap_detail_lines(panel: &MinimapPanelModel) -> Vec<String> {
    let detail_count = panel.detail_counts.len();
    let mut lines = panel
        .detail_counts
        .iter()
        .enumerate()
        .map(|(index, detail)| {
            format!(
                "minid:{}/{}:{}={}",
                index + 1,
                detail_count,
                detail.label,
                detail.count
            )
        })
        .collect::<Vec<_>>();
    lines.push(format_minimap_density_visibility_text(panel));
    lines
}

pub(crate) fn format_minimap_edge_detail_text(panel: &MinimapPanelModel) -> String {
    panel.edge_detail_label()
}

pub(crate) fn format_semantic_detail_text(
    detail_counts: &[RenderSemanticDetailCount],
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

pub(crate) fn format_minimap_legend_text(summary: &HudSummary) -> String {
    format!(
        "legend:pl@/mkM/pnP/bk#/rtR/tr./uk?:vis={}:ov{}:fg{}",
        summary.visibility_label(),
        u8::from(summary.overlay_visible),
        u8::from(summary.fog_enabled),
    )
}

pub(crate) fn format_runtime_dialog_prompt_text(
    kind: Option<RuntimeDialogPromptKind>,
) -> &'static str {
    match kind {
        Some(RuntimeDialogPromptKind::Menu) => "menu",
        Some(RuntimeDialogPromptKind::FollowUpMenu) => "follow",
        Some(RuntimeDialogPromptKind::TextInput) => "input",
        None => "none",
    }
}

pub(crate) fn format_runtime_dialog_notice_text(
    kind: Option<RuntimeDialogNoticeKind>,
) -> &'static str {
    match kind {
        Some(RuntimeDialogNoticeKind::Hud) => "hud",
        Some(RuntimeDialogNoticeKind::HudReliable) => "hud-rel",
        Some(RuntimeDialogNoticeKind::ToastInfo) => "toast",
        Some(RuntimeDialogNoticeKind::ToastWarning) => "warn",
        None => "none",
    }
}

pub(crate) fn format_runtime_dialog_panel_text(panel: &RuntimeDialogPanelModel) -> String {
    format!(
        "dialog:p={}:a{}:m{}/f{}/h{}:tin{}@{}:{}/{}/{}#{}:n{}:e{}:n={}@{}:c{}",
        format_runtime_dialog_prompt_text(panel.prompt_kind),
        u8::from(panel.prompt_active),
        panel.menu_open_count,
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
        format_runtime_dialog_notice_text(panel.notice_kind),
        compact_runtime_ui_text(panel.notice_text.as_deref()),
        panel.notice_count,
    )
}

pub(crate) fn format_runtime_dialog_detail_text(
    panel: &RuntimeDialogPanelModel,
    prompt: &RuntimePromptPanelModel,
    notice: &RuntimeNoticeStatePanelModel,
) -> String {
    format!(
        "dialogd:p={}:a{}:m{}:fo{}:tin{}:msg{}:def{}:n={}:h{}:r{}:i{}:w{}:l{}",
        format_runtime_dialog_prompt_text(prompt.kind),
        u8::from(prompt.is_active()),
        u8::from(prompt.menu_active()),
        panel.outstanding_follow_up_count(),
        prompt.text_input_open_count,
        panel.prompt_message_len(),
        panel.default_text_len(),
        format_runtime_dialog_notice_text(notice.kind),
        u8::from(notice.hud_active),
        u8::from(notice.reliable_hud_active),
        u8::from(notice.toast_info_active),
        u8::from(notice.toast_warning_active),
        panel.notice_text_len(),
    )
}

pub(crate) fn format_runtime_prompt_panel_text(panel: &RuntimePromptPanelModel) -> String {
    let layers = panel.layer_labels().join(">");
    format!(
        "prompt:k={}:a{}:d{}:l={}:m{}:fo{}:tin{}@{}:{}/{}/{}#{}:n{}:e{}",
        format_runtime_dialog_prompt_text(panel.kind),
        u8::from(panel.is_active()),
        panel.depth(),
        if layers.is_empty() {
            "none"
        } else {
            layers.as_str()
        },
        panel.menu_open_count,
        panel.outstanding_follow_up_count(),
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
    )
}

pub(crate) fn format_runtime_prompt_panel_text_if_nonempty(
    panel: &RuntimePromptPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_prompt_panel_text(panel))
}

pub(crate) fn format_runtime_prompt_detail_text(panel: &RuntimePromptPanelModel) -> String {
    format!(
        "pd:ma{}:fm{}:fh{}:fo{}:tin{}:id{}:t{}:m{}:d{}:n{}:e{}",
        u8::from(panel.menu_active()),
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.outstanding_follow_up_count(),
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        panel.prompt_message_len(),
        panel.default_text_len(),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
    )
}

pub(crate) fn format_runtime_prompt_detail_text_if_nonempty(
    panel: &RuntimePromptPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_prompt_detail_text(panel))
}

pub(crate) fn format_runtime_notice_state_panel_text(
    panel: &RuntimeNoticeStatePanelModel,
) -> String {
    let notice_text = format!(
        "{}@{}",
        format_runtime_dialog_notice_text(panel.kind),
        compact_runtime_ui_text(panel.text.as_deref())
    );
    let layers = panel.layer_labels();
    let source = layers.last().copied().unwrap_or("none");
    let active_layers = if layers.is_empty() {
        "none".to_string()
    } else {
        layers.join(">")
    };
    format!(
        "notice-state:n={}:src={}:layers={}:c{}",
        notice_text, source, active_layers, panel.count
    )
}

pub(crate) fn format_runtime_notice_state_detail_text(
    panel: &RuntimeNoticeStatePanelModel,
) -> String {
    let notice_text = format!(
        "{}@{}",
        format_runtime_dialog_notice_text(panel.kind),
        compact_runtime_ui_text(panel.text.as_deref())
    );
    let layers = panel.layer_labels().join(">");
    let source = panel.layer_labels().last().copied().unwrap_or("none");
    format!(
        "nstated:n={}:src={}:c{}:d{}:l{}:layers={}",
        notice_text,
        source,
        panel.count,
        panel.depth(),
        panel.text_len(),
        if layers.is_empty() {
            "none"
        } else {
            layers.as_str()
        },
    )
}

pub(crate) fn format_runtime_stack_panel_text(panel: &RuntimeUiStackPanelModel) -> String {
    let prompt_layers = panel.prompt_layer_labels().join(">");
    let notice_layers = panel.notice_layer_labels().join(">");
    format!(
        "stack:f={}:p{}@{}:n={}@{}:c{}:g{}:t{}:tin{}:s{}",
        panel.foreground_label(),
        panel.prompt_depth(),
        if prompt_layers.is_empty() {
            "none"
        } else {
            prompt_layers.as_str()
        },
        format_runtime_dialog_notice_text(panel.notice_kind),
        if notice_layers.is_empty() {
            "none"
        } else {
            notice_layers.as_str()
        },
        panel.chat_depth(),
        panel.active_group_count(),
        panel.total_depth(),
        format_optional_i32_text(panel.text_input_last_id),
        format_optional_i32_text(panel.last_chat_sender_entity_id),
    )
}

pub(crate) fn format_runtime_stack_panel_text_if_nonempty(
    panel: &RuntimeUiStackPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_stack_panel_text(panel))
}

pub(crate) fn format_runtime_stack_detail_text(panel: &RuntimeDialogStackPanelModel) -> String {
    format!(
        "stackd:f={}:g{}:t{}:p={}:m{}:fo{}:i{}:n={}:h{}:r{}:i{}:w{}:c{}:{}/{}:sid{}",
        panel.foreground_label(),
        panel.active_group_count(),
        panel.total_depth(),
        format_runtime_dialog_prompt_text(panel.prompt.kind),
        u8::from(panel.prompt.menu_active()),
        panel.prompt.outstanding_follow_up_count(),
        panel.prompt.text_input_open_count,
        format_runtime_dialog_notice_text(panel.notice.kind),
        u8::from(panel.notice.hud_active),
        u8::from(panel.notice.reliable_hud_active),
        u8::from(panel.notice.toast_info_active),
        u8::from(panel.notice.toast_warning_active),
        u8::from(!panel.chat.is_empty()),
        panel.chat.server_message_count,
        panel.chat.chat_message_count,
        format_optional_i32_text(panel.chat.last_chat_sender_entity_id),
    )
}

pub(crate) fn format_runtime_stack_detail_text_if_nonempty(
    panel: &RuntimeDialogStackPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_stack_detail_text(panel))
}

pub(crate) fn format_runtime_live_entity_summary_text(
    entity: &RuntimeLiveEntitySummaryObservability,
) -> String {
    format_runtime_live_entity_body_text(
        entity.entity_count,
        entity.hidden_count,
        entity.local_entity_id,
        entity.local_unit_kind,
        entity.local_unit_value,
        entity.local_position.as_ref(),
        entity.local_hidden,
        entity.local_last_seen_entity_snapshot_count,
        entity.player_count,
        entity.unit_count,
        entity.last_entity_id,
        entity.last_player_entity_id,
        entity.last_unit_entity_id,
    )
}

pub(crate) fn format_runtime_live_entity_panel_text(
    entity: &RuntimeLiveEntityPanelModel,
) -> String {
    format!(
        "liveent:{}",
        format_runtime_live_entity_body_text(
            entity.entity_count,
            entity.hidden_count,
            entity.local_entity_id,
            entity.local_unit_kind,
            entity.local_unit_value,
            entity.local_position.as_ref(),
            entity.local_hidden,
            entity.local_last_seen_entity_snapshot_count,
            entity.player_count,
            entity.unit_count,
            entity.last_entity_id,
            entity.last_player_entity_id,
            entity.last_unit_entity_id,
        )
    )
}

pub(crate) fn format_runtime_live_entity_detail_text(
    entity: &RuntimeLiveEntityPanelModel,
) -> String {
    format!("liveentd:{}", entity.detail_label())
}

pub(crate) fn format_runtime_live_effect_summary_text(
    effect: &RuntimeLiveEffectSummaryObservability,
) -> String {
    format_runtime_live_effect_body_text(
        effect.effect_count,
        effect.spawn_effect_count,
        effect.active_overlay_count,
        effect.display_effect_id(),
        effect.last_spawn_effect_unit_type_id,
        effect.last_data_len,
        effect.last_data_type_tag,
        effect.last_kind.as_deref(),
        effect.display_contract_name(),
        effect.display_reliable_contract_name(),
        effect.binding_label.as_deref(),
        effect.active_reliable,
        effect.last_business_hint.as_deref(),
        effect.display_position_source(),
        effect.display_position(),
        effect.display_overlay_ttl(),
    )
}

pub(crate) fn format_runtime_live_effect_panel_text(
    effect: &RuntimeLiveEffectPanelModel,
) -> String {
    format!(
        "livefx:{}",
        format_runtime_live_effect_body_text(
            effect.effect_count,
            effect.spawn_effect_count,
            effect.active_overlay_count,
            effect.display_effect_id(),
            effect.last_spawn_effect_unit_type_id,
            effect.last_data_len,
            effect.last_data_type_tag,
            effect.last_kind.as_deref(),
            effect.display_contract_name(),
            effect.display_reliable_contract_name(),
            effect.binding_label.as_deref(),
            effect.active_reliable,
            effect.last_business_hint.as_deref(),
            effect.display_position_source(),
            effect.display_position(),
            effect.display_overlay_ttl(),
        )
    )
}

pub(crate) fn format_runtime_live_effect_detail_text(
    effect: &RuntimeLiveEffectPanelModel,
) -> String {
    format!(
        "livefxd:hint{}:src{}:pos{}:ttl{}:data{}:arel{}:ctr{}:rel{}:bind{}",
        effect.last_business_hint.as_deref().unwrap_or("none"),
        format_live_effect_position_source_text(effect.display_position_source()),
        format_world_position_status_text(effect.display_position()),
        format_live_effect_ttl_text(effect.display_overlay_ttl()),
        format_live_effect_data_shape_text(effect.last_data_len, effect.last_data_type_tag),
        format_live_effect_reliable_flag_text(effect.active_reliable),
        compact_runtime_ui_text(effect.display_contract_name()),
        compact_runtime_ui_text(effect.display_reliable_contract_name()),
        effect.binding_detail.as_deref().unwrap_or("none"),
    )
}

fn format_runtime_live_entity_body_text(
    entity_count: usize,
    hidden_count: usize,
    local_entity_id: Option<i32>,
    local_unit_kind: Option<u8>,
    local_unit_value: Option<u32>,
    local_position: Option<&RuntimeWorldPositionObservability>,
    local_hidden: Option<bool>,
    local_last_seen_entity_snapshot_count: Option<u64>,
    player_count: usize,
    unit_count: usize,
    last_entity_id: Option<i32>,
    last_player_entity_id: Option<i32>,
    last_unit_entity_id: Option<i32>,
) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}:tp{}/{}:last{}/{}/{}",
        entity_count,
        hidden_count,
        format_optional_i32_text(local_entity_id),
        format_optional_u8_text(local_unit_kind),
        format_optional_u32_text(local_unit_value),
        format_world_position_status_text(local_position),
        format_optional_bool_flag(local_hidden),
        format_optional_u64_text(local_last_seen_entity_snapshot_count),
        player_count,
        unit_count,
        format_optional_i32_text(last_entity_id),
        format_optional_i32_text(last_player_entity_id),
        format_optional_i32_text(last_unit_entity_id),
    )
}

fn format_runtime_live_effect_body_text(
    effect_count: u64,
    spawn_effect_count: u64,
    active_overlay_count: usize,
    display_effect_id: Option<i16>,
    last_spawn_effect_unit_type_id: Option<i16>,
    last_data_len: Option<usize>,
    last_data_type_tag: Option<u8>,
    last_kind: Option<&str>,
    display_contract_name: Option<&str>,
    display_reliable_contract_name: Option<&str>,
    binding_label: Option<&str>,
    active_reliable: Option<bool>,
    last_business_hint: Option<&str>,
    display_position_source: Option<RuntimeLiveEffectPositionSource>,
    display_position: Option<&RuntimeWorldPositionObservability>,
    display_overlay_ttl: Option<(u8, u8)>,
) -> String {
    format!(
        "{}/{}:ov{}@{}:u{}:d{}:k{}:c{}/{}:bind{}:r{}:h{}:p{}@{}:ttl{}",
        effect_count,
        spawn_effect_count,
        active_overlay_count,
        format_optional_i16_text(display_effect_id),
        format_optional_i16_text(last_spawn_effect_unit_type_id),
        format_live_effect_data_shape_text(last_data_len, last_data_type_tag),
        compact_runtime_ui_text(last_kind),
        compact_runtime_ui_text(display_contract_name),
        compact_runtime_ui_text(display_reliable_contract_name),
        binding_label.unwrap_or("none"),
        format_live_effect_reliable_flag_text(active_reliable),
        last_business_hint.unwrap_or("none"),
        format_live_effect_position_source_text(display_position_source),
        format_world_position_status_text(display_position),
        format_live_effect_ttl_text(display_overlay_ttl),
    )
}

pub(crate) fn format_runtime_chat_panel_text(panel: &RuntimeChatPanelModel) -> String {
    format!(
        "chat:srv{}@{}:msg{}@{}:raw{}:s{}",
        panel.server_message_count,
        compact_runtime_ui_text(panel.last_server_message.as_deref()),
        panel.chat_message_count,
        compact_runtime_ui_text(panel.last_chat_message.as_deref()),
        compact_runtime_ui_text(panel.last_chat_unformatted.as_deref()),
        format_optional_i32_text(panel.last_chat_sender_entity_id),
    )
}

pub(crate) fn format_runtime_chat_detail_text(panel: &RuntimeChatPanelModel) -> String {
    format!(
        "chatd:s{}:c{}:r{}:eq{}:sid{}",
        panel.last_server_message_len(),
        panel.last_chat_message_len(),
        panel.last_chat_unformatted_len(),
        format_optional_bool_flag(panel.formatted_matches_unformatted()),
        format_optional_i32_text(panel.last_chat_sender_entity_id),
    )
}

pub(crate) fn format_runtime_chat_detail_text_if_nonempty(
    panel: &RuntimeChatPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_chat_detail_text(panel))
}

pub(crate) fn format_runtime_bootstrap_summary_text_if_nonempty(
    panel: &RuntimeBootstrapPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| panel.summary_label())
}

pub(crate) fn format_runtime_bootstrap_detail_text_if_nonempty(
    panel: &RuntimeBootstrapPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| panel.detail_label())
}

pub(crate) fn format_runtime_choice_panel_text(panel: &RuntimeChoicePanelModel) -> String {
    format!(
        "choice:mc{}@{}/{}:tir{}@{}/{}",
        panel.menu_choose_count,
        format_optional_i32_text(panel.last_menu_choose_menu_id),
        format_optional_i32_text(panel.last_menu_choose_option),
        panel.text_input_result_count,
        format_optional_i32_text(panel.last_text_input_result_id),
        compact_runtime_ui_text(panel.last_text_input_result_text.as_deref()),
    )
}

pub(crate) fn format_runtime_choice_panel_text_if_nonempty(
    panel: &RuntimeChoicePanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_choice_panel_text(panel))
}

pub(crate) fn format_runtime_choice_detail_text(panel: &RuntimeChoicePanelModel) -> String {
    format!(
        "choiced:mid{}:opt{}:rid{}:rlen{}",
        format_optional_i32_text(panel.last_menu_choose_menu_id),
        format_optional_i32_text(panel.last_menu_choose_option),
        format_optional_i32_text(panel.last_text_input_result_id),
        panel.text_input_result_len(),
    )
}

pub(crate) fn format_runtime_choice_detail_text_if_nonempty(
    panel: &RuntimeChoicePanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_choice_detail_text(panel))
}

pub(crate) fn format_runtime_menu_detail_text(panel: &RuntimeMenuPanelModel) -> String {
    format!(
        "menud:a{}:fo{}:m{}:{}:{}:{}:{}:fm{}:{}:{}:{}:{}:hid{}:tin{}:id{}:t{}:d{}:n{}:e{}",
        if panel.text_input_open_count > 0
            || panel.menu_open_count > 0
            || panel.outstanding_follow_up_count() > 0
        {
            1
        } else {
            0
        },
        panel.outstanding_follow_up_count(),
        format_optional_i32_text(panel.last_menu_open_id),
        panel.menu_title_len(),
        panel.menu_message_len(),
        panel.last_menu_open_option_rows,
        panel.last_menu_open_first_row_len,
        format_optional_i32_text(panel.last_follow_up_menu_open_id),
        panel.follow_up_title_len(),
        panel.follow_up_message_len(),
        panel.last_follow_up_menu_open_option_rows,
        panel.last_follow_up_menu_open_first_row_len,
        format_optional_i32_text(panel.last_hide_follow_up_menu_id),
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        panel.default_text_len(),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
    )
}

pub(crate) fn format_runtime_menu_detail_text_if_nonempty(
    panel: &RuntimeMenuPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_menu_detail_text(panel))
}

pub(crate) fn format_runtime_menu_panel_text(panel: &RuntimeMenuPanelModel) -> String {
    format!(
        "menu:m{}@{}:{}/{}#{}:{}:fm{}@{}:{}/{}#{}:{}:h{}@{}:tin{}@{}:{}/{}#{}:n{}:e{}",
        panel.menu_open_count,
        format_optional_i32_text(panel.last_menu_open_id),
        compact_runtime_ui_text(panel.last_menu_open_title.as_deref()),
        compact_runtime_ui_text(panel.last_menu_open_message.as_deref()),
        panel.last_menu_open_option_rows,
        panel.last_menu_open_first_row_len,
        panel.follow_up_menu_open_count,
        format_optional_i32_text(panel.last_follow_up_menu_open_id),
        compact_runtime_ui_text(panel.last_follow_up_menu_open_title.as_deref()),
        compact_runtime_ui_text(panel.last_follow_up_menu_open_message.as_deref()),
        panel.last_follow_up_menu_open_option_rows,
        panel.last_follow_up_menu_open_first_row_len,
        panel.hide_follow_up_menu_count,
        format_optional_i32_text(panel.last_hide_follow_up_menu_id),
        panel.text_input_open_count,
        format_optional_i32_text(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        format_optional_bool_flag(panel.text_input_last_numeric),
        format_optional_bool_flag(panel.text_input_last_allow_empty),
    )
}

pub(crate) fn format_runtime_rules_panel_text(panel: &RuntimeRulesPanelModel) -> String {
    format!(
        "rules:mut{}:fail{}:wv{}:pvp{}:obj{}:q{}:par{}:fg{}:oor{}:last{}",
        panel.mutation_count,
        panel.parse_fail_count,
        format_optional_bool_flag(panel.waves),
        format_optional_bool_flag(panel.pvp),
        panel.objective_count,
        panel.qualified_objective_count,
        panel.objective_parent_edge_count,
        panel.objective_flag_count,
        panel.complete_out_of_range_count,
        format_optional_i32_text(panel.last_completed_index),
    )
}

pub(crate) fn format_runtime_rules_detail_text(panel: &RuntimeRulesPanelModel) -> String {
    format!(
        "rulesd:set{}:obj{}:rule{}:clr{}:done{}",
        panel.set_rules_count,
        panel.set_objectives_count,
        panel.set_rule_count,
        panel.clear_objectives_count,
        panel.complete_objective_count,
    )
}

pub(crate) fn format_runtime_rules_detail_text_if_nonempty(
    panel: &RuntimeRulesPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_rules_detail_text(panel))
}

pub(crate) fn format_runtime_admin_panel_text(panel: &RuntimeAdminPanelModel) -> String {
    format!(
        "admin:t{}@{}:f{}:dbg{}/{}@{}:f{}",
        panel.trace_info_count,
        format_optional_i32_text(panel.last_trace_info_player_id),
        panel.trace_info_parse_fail_count,
        panel.debug_status_client_count,
        panel.debug_status_client_unreliable_count,
        format_optional_i32_text(panel.last_debug_status_value),
        panel.parse_fail_count,
    )
}

pub(crate) fn format_runtime_admin_detail_text(panel: &RuntimeAdminPanelModel) -> String {
    format!(
        "admind:tr{}/{}@{}:dbg{}/{}:udbg{}/{}:last{}",
        panel.trace_info_count,
        panel.trace_info_parse_fail_count,
        format_optional_i32_text(panel.last_trace_info_player_id),
        panel.debug_status_client_count,
        panel.debug_status_client_parse_fail_count,
        panel.debug_status_client_unreliable_count,
        panel.debug_status_client_unreliable_parse_fail_count,
        format_optional_i32_text(panel.last_debug_status_value),
    )
}

pub(crate) fn format_runtime_admin_detail_text_if_nonempty(
    panel: &RuntimeAdminPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_admin_detail_text(panel))
}

pub(crate) fn format_runtime_world_label_panel_text(
    panel: &RuntimeWorldLabelPanelModel,
) -> String {
    format!(
        "wlabel:set{}:rel{}:rm{}:tot{}:act{}:inact{}:last{}:f{}:fs{}:z{}:pos{}:txt{}:l{}:n{}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.total_count,
        panel.active_count,
        panel.inactive_count(),
        format_optional_i32_text(panel.last_entity_id),
        format_optional_u8_text(panel.last_flags),
        format_runtime_world_label_scalar_text(panel.last_font_size_bits, panel.last_font_size()),
        format_runtime_world_label_scalar_text(panel.last_z_bits, panel.last_z()),
        format_world_position_status_text(panel.last_position.as_ref()),
        format_runtime_world_label_sample_text(panel.last_text.as_deref()),
        panel.last_text_line_count(),
        panel.last_text_len(),
    )
}

pub(crate) fn format_runtime_world_label_detail_text(
    panel: &RuntimeWorldLabelPanelModel,
) -> String {
    format!(
        "wlabeld:set{}:rel{}:rm{}:tot{}:act{}:in{}:last{}:f{}:txt{}x{}:fs{}:z{}:p{}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.total_count,
        panel.active_count,
        panel.inactive_count(),
        format_optional_i32_text(panel.last_entity_id),
        format_optional_u8_text(panel.last_flags),
        panel.last_text_len(),
        panel.last_text_line_count(),
        format_runtime_world_label_scalar_text(panel.last_font_size_bits, panel.last_font_size()),
        format_runtime_world_label_scalar_text(panel.last_z_bits, panel.last_z()),
        format_world_position_status_text(panel.last_position.as_ref()),
    )
}

pub(crate) fn format_runtime_world_label_detail_text_if_nonempty(
    panel: &RuntimeWorldLabelPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_world_label_detail_text(panel))
}

pub(crate) fn format_runtime_world_reload_panel_text(
    world_reload: Option<&RuntimeWorldReloadPanelModel>,
) -> String {
    match world_reload {
        Some(world_reload) => format!(
            "@lw{}:cl{}:rd{}:cc{}:p{}:d{}:r{}",
            u8::from(world_reload.had_loaded_world),
            u8::from(world_reload.had_client_loaded),
            u8::from(world_reload.was_ready_to_enter_world),
            u8::from(world_reload.had_connect_confirm_sent),
            world_reload.cleared_pending_packets,
            world_reload.cleared_deferred_inbound_packets,
            world_reload.cleared_replayed_loading_events,
        ),
        None => "none".to_string(),
    }
}

pub(crate) fn format_runtime_world_reload_detail_text(
    world_reload: &RuntimeWorldReloadPanelModel,
) -> String {
    format!(
        "reloadd:lw{}:cl{}:rd{}:cc{}:p{}:d{}:r{}",
        u8::from(world_reload.had_loaded_world),
        u8::from(world_reload.had_client_loaded),
        u8::from(world_reload.was_ready_to_enter_world),
        u8::from(world_reload.had_connect_confirm_sent),
        world_reload.cleared_pending_packets,
        world_reload.cleared_deferred_inbound_packets,
        world_reload.cleared_replayed_loading_events,
    )
}

pub(crate) fn format_runtime_marker_panel_text(panel: &RuntimeMarkerPanelModel) -> String {
    format!(
        "marker:cr{}:rm{}:up{}:txt{}:tex{}:f{}:last{}:ctl{}",
        panel.create_count,
        panel.remove_count,
        panel.update_count,
        panel.update_text_count,
        panel.update_texture_count,
        panel.decode_fail_count,
        format_optional_i32_text(panel.last_marker_id),
        compact_runtime_ui_text(panel.last_control_name.as_deref()),
    )
}

pub(crate) fn format_runtime_marker_panel_text_if_nonempty(
    panel: &RuntimeMarkerPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_marker_panel_text(panel))
}

pub(crate) fn format_runtime_marker_detail_text(panel: &RuntimeMarkerPanelModel) -> String {
    format!(
        "markerd:tot{}:mut{}:txt{}:tex{}:f{}:last{}:c{}",
        panel.total_count(),
        panel.mutate_count(),
        panel.update_text_count,
        panel.update_texture_count,
        panel.decode_fail_count,
        format_optional_i32_text(panel.last_marker_id),
        panel.control_name_len(),
    )
}

pub(crate) fn format_runtime_marker_detail_text_if_nonempty(
    panel: &RuntimeMarkerPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_marker_detail_text(panel))
}

pub(crate) fn format_runtime_kick_panel_text(kick: &RuntimeKickPanelModel) -> String {
    format!(
        "{}@{}:{}:{}",
        compact_runtime_ui_text(kick.reason_text.as_deref()),
        format_optional_i32_text(kick.reason_ordinal),
        compact_runtime_ui_text(kick.hint_category.as_deref()),
        compact_runtime_ui_text(kick.hint_text.as_deref()),
    )
}

pub(crate) fn format_runtime_kick_detail_text(kick: &RuntimeKickPanelModel) -> String {
    format!(
        "kickd:r{}:o{}:c{}:h{}",
        runtime_ui_text_len(kick.reason_text.as_deref()),
        format_optional_i32_text(kick.reason_ordinal),
        runtime_ui_text_len(kick.hint_category.as_deref()),
        runtime_ui_text_len(kick.hint_text.as_deref()),
    )
}

pub(crate) fn format_runtime_kick_detail_text_if_nonempty(
    kick: &RuntimeKickPanelModel,
) -> Option<String> {
    (!kick.is_empty()).then(|| format_runtime_kick_detail_text(kick))
}

pub(crate) fn format_runtime_resource_delta_panel_text(
    resource_delta: &RuntimeResourceDeltaPanelModel,
) -> String {
    format!(
        "resd:tile{}/{}/{}/{}:set{}/{}/{}/{}:clr{}/{}:tile{}/{}:flow{}/{}/{}@{}:{}:{}:{}:{}:{}:proj{}/{}/{}:au{}:d{}/{}/{}:chg{}/{}/{}/{}",
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
        format_optional_i16_text(resource_delta.last_item_id),
        format_optional_i32_text(resource_delta.last_amount),
        format_optional_i32_text(resource_delta.last_build_pos),
        format_runtime_command_unit_ref_text(resource_delta.last_unit),
        format_optional_i32_text(resource_delta.last_to_entity_id),
        resource_delta.build_count,
        resource_delta.build_stack_count,
        resource_delta.entity_count,
        resource_delta.authoritative_build_update_count,
        resource_delta.delta_apply_count,
        resource_delta.delta_skip_count,
        resource_delta.delta_conflict_count,
        format_optional_i32_text(resource_delta.last_changed_build_pos),
        format_optional_i32_text(resource_delta.last_changed_entity_id),
        format_optional_i16_text(resource_delta.last_changed_item_id),
        format_optional_i32_text(resource_delta.last_changed_amount),
    )
}

pub(crate) fn format_runtime_resource_delta_panel_text_if_nonempty(
    resource_delta: &RuntimeResourceDeltaPanelModel,
) -> Option<String> {
    (!resource_delta.is_empty()).then(|| format_runtime_resource_delta_panel_text(resource_delta))
}

pub(crate) fn format_runtime_resource_delta_detail_text(
    resource_delta: &RuntimeResourceDeltaPanelModel,
) -> String {
    format!(
        "resdd:rm{}:st{}:sf{}:so{}:set{}/{}/{}/{}:clr{}/{}:tile{}/{}:flow{}/{}/{}:last{}:{}:{}:{}:{}:{}:proj{}/{}/{}:au{}:d{}/{}/{}:chg{}/{}/{}/{}",
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
        format_optional_i16_text(resource_delta.last_item_id),
        format_optional_i32_text(resource_delta.last_amount),
        format_optional_i32_text(resource_delta.last_build_pos),
        format_runtime_command_unit_ref_text(resource_delta.last_unit),
        format_optional_i32_text(resource_delta.last_to_entity_id),
        resource_delta.build_count,
        resource_delta.build_stack_count,
        resource_delta.entity_count,
        resource_delta.authoritative_build_update_count,
        resource_delta.delta_apply_count,
        resource_delta.delta_skip_count,
        resource_delta.delta_conflict_count,
        format_optional_i32_text(resource_delta.last_changed_build_pos),
        format_optional_i32_text(resource_delta.last_changed_entity_id),
        format_optional_i16_text(resource_delta.last_changed_item_id),
        format_optional_i32_text(resource_delta.last_changed_amount),
    )
}

pub(crate) fn format_runtime_resource_delta_detail_text_if_nonempty(
    resource_delta: &RuntimeResourceDeltaPanelModel,
) -> Option<String> {
    (!resource_delta.is_empty()).then(|| format_runtime_resource_delta_detail_text(resource_delta))
}

pub(crate) fn format_runtime_reconnect_panel_text(
    reconnect: &RuntimeReconnectPanelModel,
) -> String {
    format!(
        "{}{}:{}@{}/{}:{}:{}@{}:{}",
        format_runtime_reconnect_phase_text(reconnect.phase),
        reconnect.phase_transition_count,
        format_runtime_reconnect_reason_kind_text(reconnect.reason_kind),
        reconnect.redirect_count,
        compact_runtime_ui_text(reconnect.last_redirect_ip.as_deref()),
        format_optional_i32_text(reconnect.last_redirect_port),
        compact_runtime_ui_text(reconnect.reason_text.as_deref()),
        format_optional_i32_text(reconnect.reason_ordinal),
        compact_runtime_ui_text(reconnect.hint_text.as_deref()),
    )
}

pub(crate) fn format_runtime_reconnect_row_text(
    reconnect: &RuntimeReconnectPanelModel,
) -> String {
    format!("reconnect:{}", format_runtime_reconnect_panel_text(reconnect))
}

pub(crate) fn format_runtime_reconnect_detail_text(
    reconnect: &RuntimeReconnectPanelModel,
) -> String {
    format!(
        "reconnectd:{}#{}:{}:r{}@{}:h{}:rd{}@{}:{}",
        format_runtime_reconnect_phase_text(reconnect.phase),
        reconnect.phase_transition_count,
        format_runtime_reconnect_reason_kind_text(reconnect.reason_kind),
        runtime_ui_text_len(reconnect.reason_text.as_deref()),
        format_optional_i32_text(reconnect.reason_ordinal),
        runtime_ui_text_len(reconnect.hint_text.as_deref()),
        reconnect.redirect_count,
        compact_runtime_ui_text(reconnect.last_redirect_ip.as_deref()),
        format_optional_i32_text(reconnect.last_redirect_port),
    )
}

pub(crate) fn format_runtime_reconnect_detail_text_if_nonempty(
    reconnect: &RuntimeReconnectPanelModel,
) -> Option<String> {
    (!reconnect.is_empty()).then(|| format_runtime_reconnect_detail_text(reconnect))
}

pub(crate) fn format_runtime_core_binding_panel_text(
    panel: &RuntimeCoreBindingPanelModel,
) -> String {
    format!(
        "core:{}:a{}@{}:m{}@{}",
        panel.kind_label(),
        panel.ambiguous_team_count,
        format_u8_list_text(&panel.ambiguous_team_sample),
        panel.missing_team_count,
        format_u8_list_text(&panel.missing_team_sample),
    )
}

pub(crate) fn format_runtime_core_binding_panel_text_if_nonempty(
    panel: &RuntimeCoreBindingPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_core_binding_panel_text(panel))
}

pub(crate) fn format_runtime_core_binding_detail_text(
    panel: &RuntimeCoreBindingPanelModel,
) -> String {
    format!(
        "cored:{}:a{}@{}:m{}@{}",
        panel.kind_label(),
        panel.ambiguous_team_count,
        format_u8_list_text(&panel.ambiguous_team_sample),
        panel.missing_team_count,
        format_u8_list_text(&panel.missing_team_sample),
    )
}

pub(crate) fn format_runtime_core_binding_detail_text_if_nonempty(
    panel: &RuntimeCoreBindingPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_core_binding_detail_text(panel))
}

pub(crate) fn format_runtime_loading_panel_text(
    loading: &RuntimeLoadingPanelModel,
) -> String {
    format!(
        "defer{}:replay{}:drop{}:qdrop{}:sfail{}:scfail{}:efail{}:rdy{}@{}:to{}:cto{}:rto{}:lt{}@{}:rs{}:rr{}:wr{}:kr{}:lr{}:lwr{}",
        loading.deferred_inbound_packet_count,
        loading.replayed_inbound_packet_count,
        loading.dropped_loading_low_priority_packet_count,
        loading.dropped_loading_deferred_overflow_count,
        loading.failed_state_snapshot_parse_count,
        loading.failed_state_snapshot_core_data_parse_count,
        loading.failed_entity_snapshot_parse_count,
        loading.ready_inbound_liveness_anchor_count,
        format_optional_u64_text(loading.last_ready_inbound_liveness_anchor_at_ms),
        loading.timeout_count,
        loading.connect_or_loading_timeout_count,
        loading.ready_snapshot_timeout_count,
        format_runtime_session_timeout_kind_text(loading.last_timeout_kind),
        format_optional_u64_text(loading.last_timeout_idle_ms),
        loading.reset_count,
        loading.reconnect_reset_count,
        loading.world_reload_count,
        loading.kick_reset_count,
        format_runtime_session_reset_kind_text(loading.last_reset_kind),
        format_runtime_world_reload_panel_text(loading.last_world_reload.as_ref()),
    )
}

pub(crate) fn format_runtime_loading_row_text(loading: &RuntimeLoadingPanelModel) -> String {
    format!("loading:{}", format_runtime_loading_panel_text(loading))
}

pub(crate) fn format_runtime_loading_detail_text(
    loading: &RuntimeLoadingPanelModel,
) -> String {
    format!(
        "loadingd:rdy{}@{}:to{}/{}/{}:{}@{}:rs{}/{}/{}/{}:{}:{}",
        loading.ready_inbound_liveness_anchor_count,
        format_optional_u64_text(loading.last_ready_inbound_liveness_anchor_at_ms),
        loading.timeout_count,
        loading.connect_or_loading_timeout_count,
        loading.ready_snapshot_timeout_count,
        format_runtime_session_timeout_kind_text(loading.last_timeout_kind),
        format_optional_u64_text(loading.last_timeout_idle_ms),
        loading.reset_count,
        loading.reconnect_reset_count,
        loading.world_reload_count,
        loading.kick_reset_count,
        format_runtime_session_reset_kind_text(loading.last_reset_kind),
        format_runtime_world_reload_panel_text(loading.last_world_reload.as_ref()),
    )
}

pub(crate) fn format_runtime_loading_detail_text_if_nonempty(
    loading: &RuntimeLoadingPanelModel,
) -> Option<String> {
    (!loading.is_empty()).then(|| format_runtime_loading_detail_text(loading))
}

pub(crate) fn format_runtime_world_reload_text_if_loading_nonempty(
    loading: &RuntimeLoadingPanelModel,
) -> Option<String> {
    (!loading.is_empty())
        .then(|| format_runtime_world_reload_panel_text(loading.last_world_reload.as_ref()))
}

pub(crate) fn format_runtime_world_reload_detail_text_from_loading(
    loading: &RuntimeLoadingPanelModel,
) -> Option<String> {
    loading
        .last_world_reload
        .as_ref()
        .map(format_runtime_world_reload_detail_text)
}

pub(crate) fn format_runtime_session_panel_text(panel: &RuntimeSessionPanelModel) -> String {
    let mut segments = Vec::new();
    if !panel.bootstrap.is_empty() {
        segments.push(format!("bootstrap={}", panel.bootstrap.summary_label()));
    }
    if !panel.core_binding.is_empty() {
        segments.push(format!(
            "cb={}",
            format_runtime_core_binding_panel_text(&panel.core_binding)
        ));
    }
    segments.push(format!(
        "rd={}",
        format_runtime_resource_delta_panel_text(&panel.resource_delta)
    ));
    segments.push(format!("k={}", format_runtime_kick_panel_text(&panel.kick)));
    segments.push(format!("l={}", format_runtime_loading_panel_text(&panel.loading)));
    segments.push(format!(
        "r={}",
        format_runtime_reconnect_panel_text(&panel.reconnect)
    ));
    format!("sess:{}", segments.join(";"))
}

pub(crate) fn format_runtime_session_panel_text_if_nonempty(
    panel: &RuntimeSessionPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_session_panel_text(panel))
}

pub(crate) fn format_runtime_session_detail_text(panel: &RuntimeSessionPanelModel) -> String {
    let mut segments = Vec::new();
    if !panel.bootstrap.is_empty() {
        segments.push(format!("bootstrap({})", panel.bootstrap.detail_label()));
    }
    if !panel.core_binding.is_empty() {
        segments.push(format!(
            "cb({})",
            format_runtime_core_binding_detail_text(&panel.core_binding)
        ));
    }
    segments.push(format!(
        "rd({})",
        format_runtime_resource_delta_detail_text(&panel.resource_delta)
    ));
    segments.push(format!("k({})", format_runtime_kick_detail_text(&panel.kick)));
    segments.push(format!("l({})", format_runtime_loading_detail_text(&panel.loading)));
    segments.push(format!(
        "r({})",
        format_runtime_reconnect_detail_text(&panel.reconnect)
    ));
    format!("sessd:{}", segments.join(":"))
}

pub(crate) fn format_runtime_session_detail_text_if_nonempty(
    panel: &RuntimeSessionPanelModel,
) -> Option<String> {
    (!panel.is_empty()).then(|| format_runtime_session_detail_text(panel))
}

pub(crate) fn format_runtime_session_banner_text(
    panel: &RuntimeSessionPanelModel,
) -> Option<String> {
    if !panel.kick.is_empty() {
        return Some(format!(
            "KICK {}",
            format_runtime_kick_panel_text(&panel.kick)
        ));
    }
    let mut segments = Vec::new();
    if let Some(world_reload) = panel.loading.last_world_reload.as_ref() {
        segments.push(format!(
            "RELOAD {}",
            format_runtime_world_reload_panel_text(Some(world_reload))
        ));
    }
    if !panel.reconnect.is_empty() {
        segments.push(format!(
            "RECONNECT {}",
            format_runtime_reconnect_panel_text(&panel.reconnect)
        ));
    }
    if !panel.loading.is_empty() {
        segments.push(format!(
            "LOADING {}",
            format_runtime_loading_panel_text(&panel.loading)
        ));
    }
    (!segments.is_empty()).then(|| segments.join(" | "))
}

pub(crate) fn compose_runtime_session_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeSessionPanelModel) -> Option<String>,
{
    let panel = build_runtime_session_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_world_label_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeWorldLabelPanelModel) -> Option<String>,
{
    let panel = build_runtime_world_label_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_bootstrap_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeBootstrapPanelModel) -> Option<String>,
{
    let panel = build_runtime_bootstrap_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_admin_text_from_hud<F>(hud: &HudModel, formatter: F) -> Option<String>
where
    F: FnOnce(&RuntimeAdminPanelModel) -> Option<String>,
{
    let panel = build_runtime_admin_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_marker_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeMarkerPanelModel) -> Option<String>,
{
    let panel = build_runtime_marker_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_resource_delta_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeResourceDeltaPanelModel) -> Option<String>,
{
    let panel = build_runtime_session_panel(hud)?;
    formatter(&panel.resource_delta)
}

pub(crate) fn compose_runtime_menu_text_from_hud<F>(hud: &HudModel, formatter: F) -> Option<String>
where
    F: FnOnce(&RuntimeMenuPanelModel) -> Option<String>,
{
    let panel = build_runtime_menu_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_rules_text_from_hud<F>(hud: &HudModel, formatter: F) -> Option<String>
where
    F: FnOnce(&RuntimeRulesPanelModel) -> Option<String>,
{
    let panel = build_runtime_rules_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_loading_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeLoadingPanelModel) -> Option<String>,
{
    let panel = build_runtime_loading_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_kick_text_from_hud<F>(hud: &HudModel, formatter: F) -> Option<String>
where
    F: FnOnce(&RuntimeKickPanelModel) -> Option<String>,
{
    let panel = build_runtime_kick_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_core_binding_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeCoreBindingPanelModel) -> Option<String>,
{
    let panel = build_runtime_core_binding_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_reconnect_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeReconnectPanelModel) -> Option<String>,
{
    let panel = build_runtime_reconnect_panel(hud)?;
    formatter(&panel)
}

pub(crate) fn compose_runtime_live_entity_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeLiveEntityPanelModel) -> String,
{
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(formatter(&panel))
}

pub(crate) fn compose_runtime_live_effect_text_from_hud<F>(
    hud: &HudModel,
    formatter: F,
) -> Option<String>
where
    F: FnOnce(&RuntimeLiveEffectPanelModel) -> String,
{
    let panel = build_runtime_live_effect_panel(hud)?;
    Some(formatter(&panel))
}

pub(crate) fn format_runtime_stack_depth_text(summary: &RuntimeUiStackDepthSummary) -> String {
    format!(
        "sdepth:p{}:n{}:c{}:m{}:h{}:d{}:g{}:t{}",
        summary.prompt_depth,
        summary.notice_depth,
        summary.chat_depth,
        summary.menu_depth(),
        summary.hud_depth(),
        summary.dialog_depth(),
        summary.active_group_count,
        summary.total_depth,
    )
}

pub(crate) fn format_runtime_stack_depth_text_if_nonempty(
    summary: &RuntimeUiStackDepthSummary,
) -> Option<String> {
    (!summary.is_empty()).then(|| format_runtime_stack_depth_text(summary))
}

pub(crate) fn format_runtime_dialog_stack_summary_text(
    summary: &RuntimeUiStackSummary,
) -> String {
    let prompt_layers = summary.prompt_layer_labels().join(">");
    let notice_layers = summary.notice_layer_labels().join(">");
    format!(
        "stackx:f={}:p={}@{}:m{}:fo{}:i{}:n={}@{}:md{}:hd{}:c{}:{}/{}:tin{}:s{}:dd{}:t{}",
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
        u8::from(summary.chat_active),
        summary.server_message_count,
        summary.chat_message_count,
        format_optional_i32_text(summary.text_input_last_id),
        format_optional_i32_text(summary.last_chat_sender_entity_id),
        summary.dialog_depth(),
        summary.total_depth(),
    )
}

pub(crate) fn format_runtime_dialog_stack_summary_text_if_nonempty(
    summary: &RuntimeUiStackSummary,
) -> Option<String> {
    (!summary.is_empty()).then(|| format_runtime_dialog_stack_summary_text(summary))
}

pub(crate) fn format_runtime_command_i32_list_text(values: &[i32]) -> String {
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

pub(crate) fn format_runtime_command_rect_text(
    value: Option<RuntimeCommandRectObservability>,
) -> String {
    value
        .map(|rect| format!("{}:{}:{}:{}", rect.x0, rect.y0, rect.x1, rect.y1))
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn format_runtime_command_control_groups_text(
    groups: &[RuntimeCommandControlGroupPanelModel],
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
                format_optional_i32_text(group.first_unit_id)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn format_runtime_command_control_group_operation_text(
    value: Option<RuntimeCommandRecentControlGroupOperationObservability>,
) -> &'static str {
    value.map(|operation| operation.label()).unwrap_or("none")
}

pub(crate) fn format_runtime_command_unit_ref_text(
    value: Option<RuntimeCommandUnitRefObservability>,
) -> String {
    value
        .map(|unit| format!("{}:{}", unit.kind, unit.value))
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn format_runtime_command_target_text(
    value: Option<RuntimeCommandTargetObservability>,
) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    let unit_target = format_runtime_command_unit_ref_text(value.unit_target);
    let position_target = value
        .position_target
        .map(|position| format!("0x{:08x}:0x{:08x}", position.x_bits, position.y_bits))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "b{}:u{}:p{}:r{}",
        format_optional_i32_text(value.build_target),
        unit_target,
        position_target,
        format_runtime_command_rect_text(value.rect_target)
    )
}

pub(crate) fn format_runtime_command_stance_text(
    value: Option<RuntimeCommandStanceObservability>,
) -> String {
    value
        .map(|stance| {
            format!(
                "{}/{}",
                format_optional_u8_text(stance.stance_id),
                if stance.enabled { 1 } else { 0 }
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn format_runtime_command_mode_panel_text(
    panel: &RuntimeCommandModePanelModel,
) -> String {
    format!(
        "cmd:act{}:sel{}@{}:bld{}@{}:rect{}:grp{}:op{}:t{}:c{}:s{}",
        if panel.active { 1 } else { 0 },
        panel.selected_unit_count,
        format_runtime_command_i32_list_text(&panel.selected_unit_sample),
        panel.command_building_count,
        format_optional_i32_text(panel.first_command_building),
        format_runtime_command_rect_text(panel.command_rect),
        format_runtime_command_control_groups_text(&panel.control_groups),
        format_runtime_command_control_group_operation_text(panel.last_control_group_operation),
        format_runtime_command_target_text(panel.last_target),
        format_optional_u8_text(
            panel
                .last_command_selection
                .and_then(|selection| selection.command_id),
        ),
        format_runtime_command_stance_text(panel.last_stance_selection),
    )
}

pub(crate) fn format_runtime_command_mode_detail_text(
    panel: &RuntimeCommandModePanelModel,
) -> String {
    format!(
        "cmdd:sample{}:grp{}:op{}:bld{}:rect{}:t{}:c{}:s{}",
        format_runtime_command_i32_list_text(&panel.selected_unit_sample),
        format_runtime_command_control_groups_text(&panel.control_groups),
        format_runtime_command_control_group_operation_text(panel.last_control_group_operation),
        format_optional_i32_text(panel.first_command_building),
        format_runtime_command_rect_text(panel.command_rect),
        format_runtime_command_target_text(panel.last_target),
        format_optional_u8_text(
            panel
                .last_command_selection
                .and_then(|selection| selection.command_id),
        ),
        format_runtime_command_stance_text(panel.last_stance_selection),
    )
}

pub(crate) fn format_runtime_command_group_lines(
    panel: &RuntimeCommandModePanelModel,
) -> Vec<String> {
    let group_count = panel.control_groups.len();
    panel
        .control_groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            format!(
                "cmdg:{}/{}:g{}#{}@{}",
                index + 1,
                group_count,
                group.index,
                group.unit_count,
                format_optional_i32_text(group.first_unit_id)
            )
        })
        .collect()
}

fn format_optional_i32_text(value: Option<i32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn format_optional_i16_text(value: Option<i16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn format_optional_u64_text(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn format_optional_u32_text(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn format_runtime_reconnect_phase_text(phase: RuntimeReconnectPhaseObservability) -> &'static str {
    match phase {
        RuntimeReconnectPhaseObservability::Idle => "idle",
        RuntimeReconnectPhaseObservability::Scheduled => "sched",
        RuntimeReconnectPhaseObservability::Attempting => "attempt",
        RuntimeReconnectPhaseObservability::Succeeded => "ok",
        RuntimeReconnectPhaseObservability::Aborted => "abort",
    }
}

fn format_runtime_reconnect_reason_kind_text(
    kind: Option<RuntimeReconnectReasonKind>,
) -> &'static str {
    match kind {
        Some(RuntimeReconnectReasonKind::ConnectRedirect) => "redirect",
        Some(RuntimeReconnectReasonKind::Kick) => "kick",
        Some(RuntimeReconnectReasonKind::Timeout) => "timeout",
        Some(RuntimeReconnectReasonKind::ManualConnect) => "manual",
        None => "none",
    }
}

fn format_runtime_session_timeout_kind_text(
    kind: Option<RuntimeSessionTimeoutKind>,
) -> &'static str {
    match kind {
        Some(RuntimeSessionTimeoutKind::ConnectOrLoading) => "cload",
        Some(RuntimeSessionTimeoutKind::ReadySnapshotStall) => "ready",
        None => "none",
    }
}

fn format_runtime_session_reset_kind_text(kind: Option<RuntimeSessionResetKind>) -> &'static str {
    match kind {
        Some(RuntimeSessionResetKind::Reconnect) => "reconnect",
        Some(RuntimeSessionResetKind::WorldReload) => "reload",
        Some(RuntimeSessionResetKind::Kick) => "kick",
        None => "none",
    }
}

fn format_u8_list_text(values: &[u8]) -> String {
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

pub(crate) fn format_optional_bool_flag(value: Option<bool>) -> char {
    match value {
        Some(true) => '1',
        Some(false) => '0',
        None => 'n',
    }
}

pub(crate) fn format_optional_focus_tile_text(value: Option<(usize, usize)>) -> String {
    match value {
        Some((x, y)) => format!("{x}:{y}"),
        None => "-".to_string(),
    }
}

pub(crate) fn format_optional_signed_tile_text(value: Option<isize>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

pub(crate) fn format_optional_u8_text(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn format_live_effect_position_source_text(
    source: Option<RuntimeLiveEffectPositionSource>,
) -> &'static str {
    match source {
        Some(RuntimeLiveEffectPositionSource::ActiveOverlay) => "active",
        Some(RuntimeLiveEffectPositionSource::BusinessProjection) => "biz",
        Some(RuntimeLiveEffectPositionSource::EffectPacket) => "pkt",
        Some(RuntimeLiveEffectPositionSource::SpawnEffectPacket) => "spawn",
        None => "none",
    }
}

pub(crate) fn format_render_text_signature(
    kind_label: &str,
    layer: i32,
    tile_x: i32,
    tile_y: i32,
) -> String {
    format!("{kind_label}@{layer}:{tile_x}:{tile_y}")
}

pub(crate) fn format_counted_preview_text<I>(total: usize, preview_items: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let preview_items = preview_items.into_iter().collect::<Vec<_>>();
    let mut parts = vec![format!("count={total}")];
    parts.extend(preview_items.iter().cloned());
    if total > preview_items.len() {
        parts.push(format!("more={}", total - preview_items.len()));
    }
    parts.join(" ")
}

pub(crate) fn format_counted_detail_text<I>(
    total: usize,
    item_separator: &str,
    detail_items: I,
) -> String
where
    I: IntoIterator<Item = String>,
{
    let detail_items = detail_items.into_iter().collect::<Vec<_>>();
    if detail_items.is_empty() {
        return format!("count={total}");
    }
    format!("count={total}{item_separator}{}", detail_items.join(item_separator))
}

pub(crate) fn format_live_effect_ttl_text(ttl: Option<(u8, u8)>) -> String {
    match ttl {
        Some((remaining, total)) => format!("{remaining}/{total}"),
        None => "none".to_string(),
    }
}

pub(crate) fn format_live_effect_data_shape_text(
    data_len: Option<usize>,
    data_type_tag: Option<u8>,
) -> String {
    match (data_len, data_type_tag) {
        (Some(data_len), Some(data_type_tag)) => format!("{data_len}/{data_type_tag}"),
        (Some(data_len), None) => format!("{data_len}/none"),
        (None, Some(data_type_tag)) => format!("none/{data_type_tag}"),
        (None, None) => "none".to_string(),
    }
}

pub(crate) fn format_live_effect_reliable_flag_text(flag: Option<bool>) -> &'static str {
    match flag {
        Some(true) => "1",
        Some(false) => "0",
        None => "?",
    }
}

pub(crate) fn format_render_rect_detail_fields(
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

#[cfg(test)]
mod tests {
    use super::{
        compose_minimap_window_distribution_text, compose_minimap_window_kind_distribution_text,
        crop_origin, crop_window, crop_window_to_focus, format_build_strip_queue_status_text,
        format_build_config_alignment_text,
        format_counted_detail_text, format_counted_preview_text,
        format_minimap_detail_lines, format_minimap_edge_detail_text,
        format_hud_visibility_detail_text, format_hud_visibility_status_text,
        format_hud_visibility_text, format_minimap_kind_text,
        format_minimap_density_visibility_text,
        format_minimap_legend_text, format_semantic_detail_text,
        format_optional_focus_tile_text, format_optional_signed_tile_text,
        format_visibility_minimap_text,
        format_runtime_command_control_group_operation_text,
        format_runtime_command_group_lines,
        format_runtime_command_control_groups_text, format_runtime_command_i32_list_text,
        format_runtime_command_mode_detail_text, format_runtime_command_mode_panel_text,
        format_runtime_ui_notice_detail_text, format_runtime_ui_notice_panel_text,
        format_runtime_notice_state_detail_text, format_runtime_notice_state_panel_text,
        format_runtime_command_rect_text, format_runtime_command_stance_text,
        format_runtime_command_target_text, format_runtime_command_unit_ref_text,
        format_runtime_dialog_stack_summary_text,
        format_runtime_dialog_detail_text, format_runtime_dialog_panel_text,
        format_runtime_dialog_notice_text, format_runtime_dialog_prompt_text,
        format_runtime_choice_panel_text, format_runtime_choice_panel_text_if_nonempty,
        format_runtime_choice_detail_text, format_runtime_choice_detail_text_if_nonempty,
        format_runtime_menu_panel_text,
        format_runtime_menu_detail_text, format_runtime_menu_detail_text_if_nonempty,
        format_runtime_rules_panel_text,
        format_runtime_rules_detail_text, format_runtime_rules_detail_text_if_nonempty,
        format_runtime_world_label_sample_text,
        format_runtime_world_label_scalar_text,
        format_runtime_world_label_detail_text, format_runtime_world_label_detail_text_if_nonempty,
        format_runtime_world_label_panel_text,
        format_runtime_world_reload_detail_text,
        format_runtime_world_reload_detail_text_from_loading,
        format_runtime_world_reload_panel_text,
        format_runtime_world_reload_text_if_loading_nonempty,
        format_runtime_core_binding_detail_text,
        format_runtime_core_binding_detail_text_if_nonempty,
        format_runtime_core_binding_panel_text,
        format_runtime_core_binding_panel_text_if_nonempty,
        format_runtime_live_effect_detail_text, format_runtime_live_effect_panel_text,
        format_runtime_live_effect_summary_text,
        format_runtime_live_entity_detail_text, format_runtime_live_entity_panel_text,
        format_runtime_live_entity_summary_text,
        format_runtime_loading_row_text,
        format_runtime_loading_detail_text, format_runtime_loading_detail_text_if_nonempty,
        format_runtime_loading_panel_text,
        format_runtime_kick_detail_text, format_runtime_kick_detail_text_if_nonempty,
        format_runtime_kick_panel_text, format_runtime_marker_detail_text,
        format_runtime_marker_detail_text_if_nonempty, format_runtime_marker_panel_text,
        format_runtime_marker_panel_text_if_nonempty,
        format_runtime_reconnect_detail_text,
        format_runtime_reconnect_detail_text_if_nonempty,
        format_runtime_reconnect_panel_text, format_runtime_reconnect_row_text,
        format_runtime_resource_delta_detail_text,
        format_runtime_resource_delta_detail_text_if_nonempty,
        format_runtime_resource_delta_panel_text,
        format_runtime_resource_delta_panel_text_if_nonempty,
        format_runtime_session_banner_text,
        format_runtime_session_detail_text, format_runtime_session_detail_text_if_nonempty,
        format_runtime_session_panel_text, format_runtime_session_panel_text_if_nonempty,
        format_runtime_prompt_detail_text, format_runtime_prompt_detail_text_if_nonempty,
        format_runtime_prompt_panel_text, format_runtime_prompt_panel_text_if_nonempty,
        format_runtime_chat_detail_text, format_runtime_chat_detail_text_if_nonempty,
        format_runtime_chat_panel_text,
        format_runtime_bootstrap_detail_text_if_nonempty,
        format_runtime_bootstrap_summary_text_if_nonempty,
        format_runtime_admin_detail_text, format_runtime_admin_detail_text_if_nonempty,
        format_runtime_admin_panel_text,
        format_optional_i16_text, format_optional_u8_text, format_optional_bool_flag,
        format_runtime_stack_depth_text, format_runtime_stack_detail_text,
        format_runtime_stack_panel_text,
        format_live_effect_data_shape_text, format_live_effect_reliable_flag_text,
        format_live_effect_ttl_text,
        format_live_effect_position_source_text, format_render_icon_signature,
        format_render_line_signature,
        format_render_primitive_payload_fields_with, format_render_primitive_payload_value_with,
        format_render_rect_detail_fields, format_render_rect_signature,
        format_render_text_signature,
        format_world_position_status_text, normalize_zoom, projected_window,
        render_line_is_visible, render_rect_detail_is_visible, render_rect_detail_payload_fields,
        tile_local_coords, visible_window_tile, world_rect_tile_coords, world_tile_coords,
        world_to_tile_index_floor, zoomed_view_tile_span, CropWindowMode,
    };
    use crate::{
        hud_model::{
            HudMinimapSummary, HudSummary, HudViewWindowSummary,
            RuntimeUiNoticeLayerKind, RuntimeUiPromptLayerKind,
            RuntimeUiStackDepthSummary, RuntimeUiStackForegroundSummaryKind, RuntimeUiStackSummary,
        },
        panel_model::{
            HudVisibilityPanelModel,
            MinimapPanelModel, PresenterViewWindow, RuntimeAdminPanelModel,
            RuntimeChatPanelModel,
            RuntimeChoicePanelModel,
            RuntimeCommandControlGroupPanelModel, RuntimeCommandModePanelModel,
            RuntimeCoreBindingPanelModel,
            RuntimeDialogNoticeKind, RuntimeDialogPanelModel, RuntimeDialogPromptKind,
            RuntimeDialogStackPanelModel, RuntimeKickPanelModel, RuntimeLoadingPanelModel,
            RuntimeLiveEffectPanelModel,
            RuntimeLiveEntityPanelModel,
            RuntimeMarkerPanelModel, RuntimeMenuPanelModel,
            RuntimeNoticeStatePanelModel, RuntimePromptPanelModel,
            RuntimeReconnectPanelModel, RuntimeRulesPanelModel,
            RuntimeResourceDeltaPanelModel, RuntimeSessionPanelModel, RuntimeBootstrapPanelModel,
            RuntimeUiNoticePanelModel,
            RuntimeUiStackForegroundKind, RuntimeUiStackPanelModel, RuntimeWorldLabelPanelModel,
            RuntimeWorldReloadPanelModel,
        },
        render_model::{RenderPrimitivePayload, RenderPrimitivePayloadValue, RenderSemanticDetailCount},
        BuildQueueHeadStage, RenderModel, RenderObject,
        RuntimeCommandRecentControlGroupOperationObservability, RuntimeCommandRectObservability,
        RuntimeCoreBindingKindObservability,
        RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind,
        RuntimeSessionResetKind, RuntimeSessionTimeoutKind,
        RuntimeCommandSelectionObservability,
        RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
        RuntimeCommandUnitRefObservability, RuntimeLiveEffectPositionSource,
        RuntimeLiveEffectSummaryObservability,
        RuntimeLiveEntitySummaryObservability,
        RuntimeWorldPositionObservability, Viewport,
    };
    use std::collections::BTreeMap;

    const TILE_SIZE: f32 = 8.0;

    fn sample_runtime_ui_notice_panel() -> RuntimeUiNoticePanelModel {
        RuntimeUiNoticePanelModel {
            hud_set_count: 1,
            hud_set_reliable_count: 2,
            hud_hide_count: 3,
            hud_last_message: Some("hud".to_string()),
            hud_last_reliable_message: Some("rel".to_string()),
            announce_count: 4,
            last_announce_message: Some("ann".to_string()),
            info_message_count: 5,
            last_info_message: Some("info".to_string()),
            toast_info_count: 6,
            toast_warning_count: 7,
            toast_last_info_message: Some("toasti".to_string()),
            toast_last_warning_text: Some("toastw".to_string()),
            info_popup_count: 8,
            info_popup_reliable_count: 9,
            last_info_popup_reliable: Some(true),
            last_info_popup_id: Some("pid".to_string()),
            last_info_popup_message: Some("popup".to_string()),
            last_info_popup_duration_bits: Some(16),
            last_info_popup_align: Some(17),
            last_info_popup_top: Some(18),
            last_info_popup_left: Some(19),
            last_info_popup_bottom: Some(20),
            last_info_popup_right: Some(21),
            clipboard_count: 10,
            last_clipboard_text: Some("clip".to_string()),
            open_uri_count: 11,
            last_open_uri: Some("mindustry://join".to_string()),
            text_input_open_count: 12,
            text_input_last_id: Some(22),
            text_input_last_title: Some("title".to_string()),
            text_input_last_message: Some("msg".to_string()),
            text_input_last_default_text: Some("def".to_string()),
            text_input_last_length: Some(23),
            text_input_last_numeric: Some(false),
            text_input_last_allow_empty: Some(true),
        }
    }

    fn sample_runtime_live_entity_panel() -> RuntimeLiveEntityPanelModel {
        RuntimeLiveEntityPanelModel {
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
            local_position: Some(RuntimeWorldPositionObservability {
                x_bits: 20.0f32.to_bits(),
                y_bits: 33.0f32.to_bits(),
            }),
            local_owned_unit_entity_id: Some(202),
            local_owned_unit_payload_count: Some(2),
            local_owned_unit_payload_class_id: Some(5),
            local_owned_unit_payload_revision: Some(7),
            local_owned_unit_payload_body_len: Some(12),
            local_owned_unit_payload_sha256: Some("0123456789abcdef".to_string()),
            local_owned_unit_payload_nested_descendant_count: Some(2),
            local_owned_carried_item_id: Some(6),
            local_owned_carried_item_amount: Some(4),
            local_owned_controller_type: Some(4),
            local_owned_controller_value: Some(101),
        }
    }

    fn sample_runtime_live_effect_panel() -> RuntimeLiveEffectPanelModel {
        RuntimeLiveEffectPanelModel {
            effect_count: 11,
            spawn_effect_count: 73,
            active_overlay_count: 1,
            binding_label: Some("target:parent-follow/source:parent-follow".to_string()),
            binding_detail: Some("source=session session=target:parent-follow/source:parent-follow overlay=target:parent-follow/source:parent-follow active=1 target_counts=1/0/0 source_counts=1/0/0".to_string()),
            active_effect_id: Some(13),
            active_contract_name: Some("lightning".to_string()),
            active_reliable: Some(true),
            active_position: Some(RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            }),
            active_overlay_remaining_ticks: Some(3),
            active_overlay_lifetime_ticks: Some(5),
            last_effect_id: None,
            last_spawn_effect_unit_type_id: Some(19),
            last_data_len: Some(9),
            last_data_type_tag: Some(4),
            last_kind: Some("Point2".to_string()),
            last_contract_name: Some("lightning".to_string()),
            last_reliable_contract_name: Some("lightning".to_string()),
            last_business_hint: Some("pos:point2:3:4@1/0".to_string()),
            last_position_hint: Some(RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            }),
            last_position_source: Some(RuntimeLiveEffectPositionSource::BusinessProjection),
        }
    }

    fn sample_runtime_live_effect_summary() -> RuntimeLiveEffectSummaryObservability {
        RuntimeLiveEffectSummaryObservability {
            effect_count: 11,
            spawn_effect_count: 73,
            active_overlay_count: 1,
            binding_label: Some("target:parent-follow/source:parent-follow".to_string()),
            binding_detail: Some("source=session session=target:parent-follow/source:parent-follow overlay=target:parent-follow/source:parent-follow active=1 target_counts=1/0/0 source_counts=1/0/0".to_string()),
            active_effect_id: Some(13),
            active_contract_name: Some("lightning".to_string()),
            active_reliable: Some(true),
            active_position: Some(RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            }),
            active_overlay_remaining_ticks: Some(3),
            active_overlay_lifetime_ticks: Some(5),
            last_effect_id: None,
            last_spawn_effect_unit_type_id: Some(19),
            last_data_len: Some(9),
            last_data_type_tag: Some(4),
            last_kind: Some("Point2".to_string()),
            last_contract_name: Some("lightning".to_string()),
            last_reliable_contract_name: Some("lightning".to_string()),
            last_business_hint: Some("pos:point2:3:4@1/0".to_string()),
            last_position_hint: Some(RuntimeWorldPositionObservability {
                x_bits: 28.0f32.to_bits(),
                y_bits: 36.0f32.to_bits(),
            }),
            last_position_source: Some(RuntimeLiveEffectPositionSource::BusinessProjection),
        }
    }

    fn sample_hud_summary() -> HudSummary {
        HudSummary {
            player_name: "player".to_string(),
            team_id: 4,
            selected_block: "duo".to_string(),
            plan_count: 3,
            marker_count: 2,
            map_width: 20,
            map_height: 10,
            overlay_visible: true,
            fog_enabled: false,
            visible_tile_count: 80,
            hidden_tile_count: 40,
            minimap: HudMinimapSummary {
                focus_tile: Some((4, 5)),
                view_window: HudViewWindowSummary {
                    origin_x: 1,
                    origin_y: 2,
                    width: 8,
                    height: 6,
                },
            },
        }
    }

    fn sample_hud_visibility_panel() -> HudVisibilityPanelModel {
        HudVisibilityPanelModel {
            overlay_visible: true,
            fog_enabled: false,
            visible_tile_count: 80,
            hidden_tile_count: 40,
            known_tile_count: 120,
            known_tile_percent: 60,
            visible_known_percent: 67,
            hidden_known_percent: 33,
            unknown_tile_count: 80,
            unknown_tile_percent: 40,
        }
    }

    fn sample_runtime_resource_delta_panel() -> RuntimeResourceDeltaPanelModel {
        RuntimeResourceDeltaPanelModel {
            remove_tile_count: 1,
            set_tile_count: 2,
            set_floor_count: 3,
            set_overlay_count: 4,
            set_item_count: 5,
            set_items_count: 6,
            set_liquid_count: 7,
            set_liquids_count: 8,
            clear_items_count: 9,
            clear_liquids_count: 10,
            set_tile_items_count: 11,
            set_tile_liquids_count: 12,
            take_items_count: 13,
            transfer_item_to_count: 14,
            transfer_item_to_unit_count: 15,
            last_kind: Some("to unit".to_string()),
            last_item_id: Some(16),
            last_amount: Some(17),
            last_build_pos: Some(18),
            last_unit: Some(RuntimeCommandUnitRefObservability { kind: 2, value: 19 }),
            last_to_entity_id: Some(20),
            build_count: 21,
            build_stack_count: 22,
            entity_count: 23,
            authoritative_build_update_count: 24,
            delta_apply_count: 25,
            delta_skip_count: 26,
            delta_conflict_count: 27,
            last_changed_build_pos: Some(28),
            last_changed_entity_id: Some(29),
            last_changed_item_id: Some(30),
            last_changed_amount: Some(31),
        }
    }

    fn empty_runtime_resource_delta_panel() -> RuntimeResourceDeltaPanelModel {
        RuntimeResourceDeltaPanelModel {
            remove_tile_count: 0,
            set_tile_count: 0,
            set_floor_count: 0,
            set_overlay_count: 0,
            set_item_count: 0,
            set_items_count: 0,
            set_liquid_count: 0,
            set_liquids_count: 0,
            clear_items_count: 0,
            clear_liquids_count: 0,
            set_tile_items_count: 0,
            set_tile_liquids_count: 0,
            take_items_count: 0,
            transfer_item_to_count: 0,
            transfer_item_to_unit_count: 0,
            last_kind: None,
            last_item_id: None,
            last_amount: None,
            last_build_pos: None,
            last_unit: None,
            last_to_entity_id: None,
            build_count: 0,
            build_stack_count: 0,
            entity_count: 0,
            authoritative_build_update_count: 0,
            delta_apply_count: 0,
            delta_skip_count: 0,
            delta_conflict_count: 0,
            last_changed_build_pos: None,
            last_changed_entity_id: None,
            last_changed_item_id: None,
            last_changed_amount: None,
        }
    }

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
    fn format_runtime_dialog_text_helpers_map_variants() {
        assert_eq!(
            format_runtime_dialog_prompt_text(Some(RuntimeDialogPromptKind::Menu)),
            "menu"
        );
        assert_eq!(
            format_runtime_dialog_prompt_text(Some(RuntimeDialogPromptKind::FollowUpMenu)),
            "follow"
        );
        assert_eq!(
            format_runtime_dialog_prompt_text(Some(RuntimeDialogPromptKind::TextInput)),
            "input"
        );
        assert_eq!(format_runtime_dialog_prompt_text(None), "none");

        assert_eq!(
            format_runtime_dialog_notice_text(Some(RuntimeDialogNoticeKind::Hud)),
            "hud"
        );
        assert_eq!(
            format_runtime_dialog_notice_text(Some(RuntimeDialogNoticeKind::HudReliable)),
            "hud-rel"
        );
        assert_eq!(
            format_runtime_dialog_notice_text(Some(RuntimeDialogNoticeKind::ToastInfo)),
            "toast"
        );
        assert_eq!(
            format_runtime_dialog_notice_text(Some(RuntimeDialogNoticeKind::ToastWarning)),
            "warn"
        );
        assert_eq!(format_runtime_dialog_notice_text(None), "none");
    }

    #[test]
    fn format_runtime_world_label_sample_text_truncates_spaces_and_long_text() {
        assert_eq!(format_runtime_world_label_sample_text(None), "none");
        assert_eq!(
            format_runtime_world_label_sample_text(Some("world label")),
            "world_label"
        );
        assert_eq!(
            format_runtime_world_label_sample_text(Some(
                "123456789012345678901234567890"
            )),
            "123456789012345678901234~"
        );
    }

    #[test]
    fn format_runtime_world_label_scalar_text_handles_missing_and_finite_values() {
        assert_eq!(format_runtime_world_label_scalar_text(None, None), "none");
        assert_eq!(
            format_runtime_world_label_scalar_text(Some(1094713344), Some(12.0)),
            "1094713344@12.0"
        );
        assert_eq!(
            format_runtime_world_label_scalar_text(Some(1094713344), None),
            "1094713344"
        );
    }

    #[test]
    fn format_runtime_world_label_panel_text_preserves_field_order() {
        let panel = RuntimeWorldLabelPanelModel {
            label_count: 19,
            reliable_label_count: 20,
            remove_label_count: 21,
            total_count: 60,
            active_count: 2,
            inactive_count: 1,
            last_entity_id: Some(904),
            last_flags: Some(3),
            last_font_size_bits: Some(1094713344),
            last_z_bits: Some(1082130432),
            last_position: Some(RuntimeWorldPositionObservability {
                x_bits: 0x4220_0000,
                y_bits: 0x4270_0000,
            }),
            last_text: Some("world label".to_string()),
        };

        assert_eq!(
            format_runtime_world_label_panel_text(&panel),
            "wlabel:set19:rel20:rm21:tot60:act2:inact1:last904:f3:fs1094713344@12.0:z1082130432@4.0:pos40.0:60.0:txtworld_label:l1:n11"
        );
    }

    #[test]
    fn format_runtime_world_label_detail_text_preserves_field_order() {
        let panel = RuntimeWorldLabelPanelModel {
            label_count: 19,
            reliable_label_count: 20,
            remove_label_count: 21,
            total_count: 60,
            active_count: 2,
            inactive_count: 1,
            last_entity_id: Some(904),
            last_flags: Some(3),
            last_font_size_bits: Some(1094713344),
            last_z_bits: Some(1082130432),
            last_position: Some(RuntimeWorldPositionObservability {
                x_bits: 0x4220_0000,
                y_bits: 0x4270_0000,
            }),
            last_text: Some("world label".to_string()),
        };

        assert_eq!(
            format_runtime_world_label_detail_text(&panel),
            "wlabeld:set19:rel20:rm21:tot60:act2:in1:last904:f3:txt11x1:fs1094713344@12.0:z1082130432@4.0:p40.0:60.0"
        );
    }

    #[test]
    fn format_runtime_world_label_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let empty_panel = RuntimeWorldLabelPanelModel {
            label_count: 0,
            reliable_label_count: 0,
            remove_label_count: 0,
            total_count: 9,
            active_count: 0,
            inactive_count: 9,
            last_entity_id: None,
            last_text: None,
            last_flags: None,
            last_font_size_bits: None,
            last_z_bits: None,
            last_position: None,
        };
        assert_eq!(
            format_runtime_world_label_detail_text_if_nonempty(&empty_panel),
            None
        );

        let panel = RuntimeWorldLabelPanelModel {
            label_count: 19,
            reliable_label_count: 20,
            remove_label_count: 21,
            total_count: 60,
            active_count: 2,
            inactive_count: 1,
            last_entity_id: Some(904),
            last_text: Some("world label".to_string()),
            last_flags: Some(3),
            last_font_size_bits: Some(1094713344),
            last_z_bits: Some(1082130432),
            last_position: Some(RuntimeWorldPositionObservability {
                x_bits: 0x4220_0000,
                y_bits: 0x4270_0000,
            }),
        };
        assert_eq!(
            format_runtime_world_label_detail_text_if_nonempty(&panel),
            Some(
                "wlabeld:set19:rel20:rm21:tot60:act2:in1:last904:f3:txt11x1:fs1094713344@12.0:z1082130432@4.0:p40.0:60.0"
                    .to_string()
            )
        );
    }

    #[test]
    fn format_runtime_world_reload_panel_text_preserves_field_order_and_none() {
        assert_eq!(format_runtime_world_reload_panel_text(None), "none");

        let panel = RuntimeWorldReloadPanelModel {
            had_loaded_world: true,
            had_client_loaded: false,
            was_ready_to_enter_world: true,
            had_connect_confirm_sent: false,
            cleared_pending_packets: 5,
            cleared_deferred_inbound_packets: 6,
            cleared_replayed_loading_events: 7,
        };

        assert_eq!(
            format_runtime_world_reload_panel_text(Some(&panel)),
            "@lw1:cl0:rd1:cc0:p5:d6:r7"
        );
    }

    #[test]
    fn format_runtime_world_reload_detail_text_preserves_field_order() {
        let panel = RuntimeWorldReloadPanelModel {
            had_loaded_world: true,
            had_client_loaded: false,
            was_ready_to_enter_world: true,
            had_connect_confirm_sent: false,
            cleared_pending_packets: 5,
            cleared_deferred_inbound_packets: 6,
            cleared_replayed_loading_events: 7,
        };

        assert_eq!(
            format_runtime_world_reload_detail_text(&panel),
            "reloadd:lw1:cl0:rd1:cc0:p5:d6:r7"
        );
    }

    #[test]
    fn format_runtime_world_reload_text_if_loading_nonempty_handles_empty_none_and_present() {
        let present = RuntimeLoadingPanelModel {
            deferred_inbound_packet_count: 1,
            replayed_inbound_packet_count: 2,
            dropped_loading_low_priority_packet_count: 3,
            dropped_loading_deferred_overflow_count: 4,
            failed_state_snapshot_parse_count: 5,
            failed_state_snapshot_core_data_parse_count: 6,
            failed_entity_snapshot_parse_count: 7,
            ready_inbound_liveness_anchor_count: 8,
            last_ready_inbound_liveness_anchor_at_ms: Some(9),
            timeout_count: 10,
            connect_or_loading_timeout_count: 11,
            ready_snapshot_timeout_count: 12,
            last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
            last_timeout_idle_ms: Some(13),
            reset_count: 14,
            reconnect_reset_count: 15,
            world_reload_count: 16,
            kick_reset_count: 17,
            last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
            last_world_reload: Some(RuntimeWorldReloadPanelModel {
                had_loaded_world: true,
                had_client_loaded: false,
                was_ready_to_enter_world: true,
                had_connect_confirm_sent: false,
                cleared_pending_packets: 4,
                cleared_deferred_inbound_packets: 5,
                cleared_replayed_loading_events: 6,
            }),
        };

        assert_eq!(
            format_runtime_world_reload_text_if_loading_nonempty(&present),
            Some("@lw1:cl0:rd1:cc0:p4:d5:r6".to_string())
        );
        assert_eq!(
            format_runtime_world_reload_text_if_loading_nonempty(&RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 1,
                replayed_inbound_packet_count: 0,
                dropped_loading_low_priority_packet_count: 0,
                dropped_loading_deferred_overflow_count: 0,
                failed_state_snapshot_parse_count: 0,
                failed_state_snapshot_core_data_parse_count: 0,
                failed_entity_snapshot_parse_count: 0,
                ready_inbound_liveness_anchor_count: 0,
                last_ready_inbound_liveness_anchor_at_ms: None,
                timeout_count: 0,
                connect_or_loading_timeout_count: 0,
                ready_snapshot_timeout_count: 0,
                last_timeout_kind: None,
                last_timeout_idle_ms: None,
                reset_count: 0,
                reconnect_reset_count: 0,
                world_reload_count: 0,
                kick_reset_count: 0,
                last_reset_kind: None,
                last_world_reload: None,
            }),
            Some("none".to_string())
        );
        assert_eq!(
            format_runtime_world_reload_text_if_loading_nonempty(&RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 0,
                replayed_inbound_packet_count: 0,
                dropped_loading_low_priority_packet_count: 0,
                dropped_loading_deferred_overflow_count: 0,
                failed_state_snapshot_parse_count: 0,
                failed_state_snapshot_core_data_parse_count: 0,
                failed_entity_snapshot_parse_count: 0,
                ready_inbound_liveness_anchor_count: 0,
                last_ready_inbound_liveness_anchor_at_ms: None,
                timeout_count: 0,
                connect_or_loading_timeout_count: 0,
                ready_snapshot_timeout_count: 0,
                last_timeout_kind: None,
                last_timeout_idle_ms: None,
                reset_count: 0,
                reconnect_reset_count: 0,
                world_reload_count: 0,
                kick_reset_count: 0,
                last_reset_kind: None,
                last_world_reload: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_world_reload_detail_text_from_loading_handles_missing_and_present() {
        let present = RuntimeLoadingPanelModel {
            deferred_inbound_packet_count: 1,
            replayed_inbound_packet_count: 2,
            dropped_loading_low_priority_packet_count: 3,
            dropped_loading_deferred_overflow_count: 4,
            failed_state_snapshot_parse_count: 5,
            failed_state_snapshot_core_data_parse_count: 6,
            failed_entity_snapshot_parse_count: 7,
            ready_inbound_liveness_anchor_count: 8,
            last_ready_inbound_liveness_anchor_at_ms: Some(9),
            timeout_count: 10,
            connect_or_loading_timeout_count: 11,
            ready_snapshot_timeout_count: 12,
            last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
            last_timeout_idle_ms: Some(13),
            reset_count: 14,
            reconnect_reset_count: 15,
            world_reload_count: 16,
            kick_reset_count: 17,
            last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
            last_world_reload: Some(RuntimeWorldReloadPanelModel {
                had_loaded_world: true,
                had_client_loaded: false,
                was_ready_to_enter_world: true,
                had_connect_confirm_sent: false,
                cleared_pending_packets: 4,
                cleared_deferred_inbound_packets: 5,
                cleared_replayed_loading_events: 6,
            }),
        };

        assert_eq!(
            format_runtime_world_reload_detail_text_from_loading(&present),
            Some("reloadd:lw1:cl0:rd1:cc0:p4:d5:r6".to_string())
        );
        assert_eq!(
            format_runtime_world_reload_detail_text_from_loading(&RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 1,
                replayed_inbound_packet_count: 0,
                dropped_loading_low_priority_packet_count: 0,
                dropped_loading_deferred_overflow_count: 0,
                failed_state_snapshot_parse_count: 0,
                failed_state_snapshot_core_data_parse_count: 0,
                failed_entity_snapshot_parse_count: 0,
                ready_inbound_liveness_anchor_count: 0,
                last_ready_inbound_liveness_anchor_at_ms: None,
                timeout_count: 0,
                connect_or_loading_timeout_count: 0,
                ready_snapshot_timeout_count: 0,
                last_timeout_kind: None,
                last_timeout_idle_ms: None,
                reset_count: 0,
                reconnect_reset_count: 0,
                world_reload_count: 0,
                kick_reset_count: 0,
                last_reset_kind: None,
                last_world_reload: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_marker_panel_text_preserves_field_order() {
        let panel = RuntimeMarkerPanelModel {
            create_count: 11,
            remove_count: 12,
            update_count: 13,
            update_text_count: 7,
            update_texture_count: 8,
            decode_fail_count: 2,
            last_marker_id: Some(904),
            last_control_name: Some("logic control".to_string()),
        };

        assert_eq!(
            format_runtime_marker_panel_text(&panel),
            "marker:cr11:rm12:up13:txt7:tex8:f2:last904:ctllogic_contro~"
        );
    }

    #[test]
    fn format_runtime_marker_panel_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeMarkerPanelModel {
            create_count: 11,
            remove_count: 12,
            update_count: 13,
            update_text_count: 7,
            update_texture_count: 8,
            decode_fail_count: 2,
            last_marker_id: Some(904),
            last_control_name: Some("logic control".to_string()),
        };

        assert_eq!(
            format_runtime_marker_panel_text_if_nonempty(&panel),
            Some("marker:cr11:rm12:up13:txt7:tex8:f2:last904:ctllogic_contro~".to_string())
        );
        assert_eq!(
            format_runtime_marker_panel_text_if_nonempty(&RuntimeMarkerPanelModel {
                create_count: 0,
                remove_count: 0,
                update_count: 0,
                update_text_count: 0,
                update_texture_count: 0,
                decode_fail_count: 0,
                last_marker_id: None,
                last_control_name: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_marker_detail_text_preserves_field_order() {
        let panel = RuntimeMarkerPanelModel {
            create_count: 11,
            remove_count: 12,
            update_count: 13,
            update_text_count: 7,
            update_texture_count: 8,
            decode_fail_count: 2,
            last_marker_id: Some(904),
            last_control_name: Some("logic control".to_string()),
        };

        assert_eq!(
            format_runtime_marker_detail_text(&panel),
            "markerd:tot51:mut36:txt7:tex8:f2:last904:c13"
        );
    }

    #[test]
    fn format_runtime_marker_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeMarkerPanelModel {
            create_count: 11,
            remove_count: 12,
            update_count: 13,
            update_text_count: 7,
            update_texture_count: 8,
            decode_fail_count: 2,
            last_marker_id: Some(904),
            last_control_name: Some("logic control".to_string()),
        };

        assert_eq!(
            format_runtime_marker_detail_text_if_nonempty(&panel),
            Some("markerd:tot51:mut36:txt7:tex8:f2:last904:c13".to_string())
        );
        assert_eq!(
            format_runtime_marker_detail_text_if_nonempty(&RuntimeMarkerPanelModel {
                create_count: 0,
                remove_count: 0,
                update_count: 0,
                update_text_count: 0,
                update_texture_count: 0,
                decode_fail_count: 0,
                last_marker_id: None,
                last_control_name: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_kick_panel_text_preserves_field_order() {
        let panel = RuntimeKickPanelModel {
            reason_text: Some("manual reconnect".to_string()),
            reason_ordinal: Some(7),
            hint_category: Some("network".to_string()),
            hint_text: Some("check vpn".to_string()),
        };

        assert_eq!(
            format_runtime_kick_panel_text(&panel),
            "manual_recon~@7:network:check_vpn"
        );
    }

    #[test]
    fn format_runtime_kick_detail_text_preserves_field_order() {
        let panel = RuntimeKickPanelModel {
            reason_text: Some("manual reconnect".to_string()),
            reason_ordinal: Some(7),
            hint_category: Some("network".to_string()),
            hint_text: Some("check vpn".to_string()),
        };

        assert_eq!(
            format_runtime_kick_detail_text(&panel),
            "kickd:r16:o7:c7:h9"
        );
    }

    #[test]
    fn format_runtime_kick_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeKickPanelModel {
            reason_text: Some("manual reconnect".to_string()),
            reason_ordinal: Some(7),
            hint_category: Some("network".to_string()),
            hint_text: Some("check vpn".to_string()),
        };

        assert_eq!(
            format_runtime_kick_detail_text_if_nonempty(&panel),
            Some("kickd:r16:o7:c7:h9".to_string())
        );
        assert_eq!(
            format_runtime_kick_detail_text_if_nonempty(&RuntimeKickPanelModel {
                reason_text: None,
                reason_ordinal: None,
                hint_category: None,
                hint_text: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_resource_delta_panel_text_preserves_field_order() {
        let panel = sample_runtime_resource_delta_panel();

        assert_eq!(
            format_runtime_resource_delta_panel_text(&panel),
            "resd:tile1/2/3/4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15@to_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31"
        );
    }

    #[test]
    fn format_runtime_resource_delta_panel_text_if_nonempty_handles_empty_and_nonempty() {
        assert_eq!(
            format_runtime_resource_delta_panel_text_if_nonempty(&sample_runtime_resource_delta_panel()),
            Some(
                "resd:tile1/2/3/4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15@to_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_resource_delta_panel_text_if_nonempty(&empty_runtime_resource_delta_panel()),
            None
        );
    }

    #[test]
    fn format_runtime_resource_delta_detail_text_preserves_field_order() {
        let panel = sample_runtime_resource_delta_panel();

        assert_eq!(
            format_runtime_resource_delta_detail_text(&panel),
            "resdd:rm1:st2:sf3:so4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15:lastto_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31"
        );
    }

    #[test]
    fn format_runtime_resource_delta_detail_text_if_nonempty_handles_empty_and_nonempty() {
        assert_eq!(
            format_runtime_resource_delta_detail_text_if_nonempty(&sample_runtime_resource_delta_panel()),
            Some(
                "resdd:rm1:st2:sf3:so4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15:lastto_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_resource_delta_detail_text_if_nonempty(&empty_runtime_resource_delta_panel()),
            None
        );
    }

    #[test]
    fn format_runtime_reconnect_panel_text_preserves_field_order() {
        let panel = RuntimeReconnectPanelModel {
            phase: RuntimeReconnectPhaseObservability::Attempting,
            phase_transition_count: 3,
            reason_kind: Some(RuntimeReconnectReasonKind::ManualConnect),
            reason_text: Some("manual".to_string()),
            reason_ordinal: Some(7),
            hint_text: Some("retry".to_string()),
            redirect_count: 4,
            last_redirect_ip: Some("1.2.3.4".to_string()),
            last_redirect_port: Some(6567),
        };

        assert_eq!(
            format_runtime_reconnect_panel_text(&panel),
            "attempt3:manual@4/1.2.3.4:6567:manual@7:retry"
        );
        assert_eq!(
            format_runtime_reconnect_row_text(&panel),
            "reconnect:attempt3:manual@4/1.2.3.4:6567:manual@7:retry"
        );
    }

    #[test]
    fn format_runtime_reconnect_detail_text_preserves_field_order() {
        let panel = RuntimeReconnectPanelModel {
            phase: RuntimeReconnectPhaseObservability::Attempting,
            phase_transition_count: 3,
            reason_kind: Some(RuntimeReconnectReasonKind::ManualConnect),
            reason_text: Some("manual".to_string()),
            reason_ordinal: Some(7),
            hint_text: Some("retry".to_string()),
            redirect_count: 4,
            last_redirect_ip: Some("1.2.3.4".to_string()),
            last_redirect_port: Some(6567),
        };

        assert_eq!(
            format_runtime_reconnect_detail_text(&panel),
            "reconnectd:attempt#3:manual:r6@7:h5:rd4@1.2.3.4:6567"
        );
    }

    #[test]
    fn format_runtime_reconnect_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeReconnectPanelModel {
            phase: RuntimeReconnectPhaseObservability::Attempting,
            phase_transition_count: 3,
            reason_kind: Some(RuntimeReconnectReasonKind::ManualConnect),
            reason_text: Some("manual".to_string()),
            reason_ordinal: Some(7),
            hint_text: Some("retry".to_string()),
            redirect_count: 4,
            last_redirect_ip: Some("1.2.3.4".to_string()),
            last_redirect_port: Some(6567),
        };

        assert_eq!(
            format_runtime_reconnect_detail_text_if_nonempty(&panel),
            Some("reconnectd:attempt#3:manual:r6@7:h5:rd4@1.2.3.4:6567".to_string())
        );
        assert_eq!(
            format_runtime_reconnect_detail_text_if_nonempty(&RuntimeReconnectPanelModel {
                phase: RuntimeReconnectPhaseObservability::Idle,
                phase_transition_count: 0,
                reason_kind: None,
                reason_text: None,
                reason_ordinal: None,
                hint_text: None,
                redirect_count: 0,
                last_redirect_ip: None,
                last_redirect_port: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_core_binding_panel_text_preserves_field_order() {
        let panel = RuntimeCoreBindingPanelModel {
            kind: Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation),
            ambiguous_team_count: 1,
            ambiguous_team_sample: vec![2, 3],
            missing_team_count: 4,
            missing_team_sample: vec![5],
        };

        assert_eq!(
            format_runtime_core_binding_panel_text(&panel),
            "core:first-core-per-team:a1@2,3:m4@5"
        );
    }

    #[test]
    fn format_runtime_core_binding_panel_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeCoreBindingPanelModel {
            kind: Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation),
            ambiguous_team_count: 1,
            ambiguous_team_sample: vec![2, 3],
            missing_team_count: 4,
            missing_team_sample: vec![5],
        };

        assert_eq!(
            format_runtime_core_binding_panel_text_if_nonempty(&panel),
            Some("core:first-core-per-team:a1@2,3:m4@5".to_string())
        );
        assert_eq!(
            format_runtime_core_binding_panel_text_if_nonempty(&RuntimeCoreBindingPanelModel {
                kind: None,
                ambiguous_team_count: 0,
                ambiguous_team_sample: vec![],
                missing_team_count: 0,
                missing_team_sample: vec![],
            }),
            None
        );
    }

    #[test]
    fn format_runtime_core_binding_detail_text_preserves_field_order() {
        let panel = RuntimeCoreBindingPanelModel {
            kind: Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation),
            ambiguous_team_count: 1,
            ambiguous_team_sample: vec![2, 3],
            missing_team_count: 4,
            missing_team_sample: vec![5],
        };

        assert_eq!(
            format_runtime_core_binding_detail_text(&panel),
            "cored:first-core-per-team:a1@2,3:m4@5"
        );
    }

    #[test]
    fn format_runtime_core_binding_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeCoreBindingPanelModel {
            kind: Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation),
            ambiguous_team_count: 1,
            ambiguous_team_sample: vec![2, 3],
            missing_team_count: 4,
            missing_team_sample: vec![5],
        };

        assert_eq!(
            format_runtime_core_binding_detail_text_if_nonempty(&panel),
            Some("cored:first-core-per-team:a1@2,3:m4@5".to_string())
        );
        assert_eq!(
            format_runtime_core_binding_detail_text_if_nonempty(&RuntimeCoreBindingPanelModel {
                kind: None,
                ambiguous_team_count: 0,
                ambiguous_team_sample: vec![],
                missing_team_count: 0,
                missing_team_sample: vec![],
            }),
            None
        );
    }

    #[test]
    fn format_runtime_loading_panel_text_preserves_field_order() {
        let panel = RuntimeLoadingPanelModel {
            deferred_inbound_packet_count: 1,
            replayed_inbound_packet_count: 2,
            dropped_loading_low_priority_packet_count: 3,
            dropped_loading_deferred_overflow_count: 4,
            failed_state_snapshot_parse_count: 5,
            failed_state_snapshot_core_data_parse_count: 6,
            failed_entity_snapshot_parse_count: 7,
            ready_inbound_liveness_anchor_count: 8,
            last_ready_inbound_liveness_anchor_at_ms: Some(9),
            timeout_count: 10,
            connect_or_loading_timeout_count: 11,
            ready_snapshot_timeout_count: 12,
            last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
            last_timeout_idle_ms: Some(13),
            reset_count: 14,
            reconnect_reset_count: 15,
            world_reload_count: 16,
            kick_reset_count: 17,
            last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
            last_world_reload: Some(RuntimeWorldReloadPanelModel {
                had_loaded_world: true,
                had_client_loaded: false,
                was_ready_to_enter_world: true,
                had_connect_confirm_sent: false,
                cleared_pending_packets: 4,
                cleared_deferred_inbound_packets: 5,
                cleared_replayed_loading_events: 6,
            }),
        };

        assert_eq!(
            format_runtime_loading_panel_text(&panel),
            "defer1:replay2:drop3:qdrop4:sfail5:scfail6:efail7:rdy8@9:to10:cto11:rto12:ltready@13:rs14:rr15:wr16:kr17:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
        );
        assert_eq!(
            format_runtime_loading_row_text(&panel),
            "loading:defer1:replay2:drop3:qdrop4:sfail5:scfail6:efail7:rdy8@9:to10:cto11:rto12:ltready@13:rs14:rr15:wr16:kr17:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
        );
    }

    #[test]
    fn format_runtime_loading_detail_text_preserves_field_order() {
        let panel = RuntimeLoadingPanelModel {
            deferred_inbound_packet_count: 1,
            replayed_inbound_packet_count: 2,
            dropped_loading_low_priority_packet_count: 3,
            dropped_loading_deferred_overflow_count: 4,
            failed_state_snapshot_parse_count: 5,
            failed_state_snapshot_core_data_parse_count: 6,
            failed_entity_snapshot_parse_count: 7,
            ready_inbound_liveness_anchor_count: 8,
            last_ready_inbound_liveness_anchor_at_ms: Some(9),
            timeout_count: 10,
            connect_or_loading_timeout_count: 11,
            ready_snapshot_timeout_count: 12,
            last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
            last_timeout_idle_ms: Some(13),
            reset_count: 14,
            reconnect_reset_count: 15,
            world_reload_count: 16,
            kick_reset_count: 17,
            last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
            last_world_reload: Some(RuntimeWorldReloadPanelModel {
                had_loaded_world: true,
                had_client_loaded: false,
                was_ready_to_enter_world: true,
                had_connect_confirm_sent: false,
                cleared_pending_packets: 4,
                cleared_deferred_inbound_packets: 5,
                cleared_replayed_loading_events: 6,
            }),
        };

        assert_eq!(
            format_runtime_loading_detail_text(&panel),
            "loadingd:rdy8@9:to10/11/12:ready@13:rs14/15/16/17:reload:@lw1:cl0:rd1:cc0:p4:d5:r6"
        );
    }

    #[test]
    fn format_runtime_loading_detail_text_if_nonempty_handles_empty_and_nonempty() {
        assert_eq!(
            format_runtime_loading_detail_text_if_nonempty(&RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 0,
                replayed_inbound_packet_count: 0,
                dropped_loading_low_priority_packet_count: 0,
                dropped_loading_deferred_overflow_count: 0,
                failed_state_snapshot_parse_count: 0,
                failed_state_snapshot_core_data_parse_count: 0,
                failed_entity_snapshot_parse_count: 0,
                ready_inbound_liveness_anchor_count: 0,
                last_ready_inbound_liveness_anchor_at_ms: None,
                timeout_count: 0,
                connect_or_loading_timeout_count: 0,
                ready_snapshot_timeout_count: 0,
                last_timeout_kind: None,
                last_timeout_idle_ms: None,
                reset_count: 0,
                reconnect_reset_count: 0,
                world_reload_count: 0,
                kick_reset_count: 0,
                last_reset_kind: None,
                last_world_reload: None,
            }),
            None
        );

        let panel = RuntimeLoadingPanelModel {
            deferred_inbound_packet_count: 1,
            replayed_inbound_packet_count: 2,
            dropped_loading_low_priority_packet_count: 3,
            dropped_loading_deferred_overflow_count: 4,
            failed_state_snapshot_parse_count: 5,
            failed_state_snapshot_core_data_parse_count: 6,
            failed_entity_snapshot_parse_count: 7,
            ready_inbound_liveness_anchor_count: 8,
            last_ready_inbound_liveness_anchor_at_ms: Some(9),
            timeout_count: 10,
            connect_or_loading_timeout_count: 11,
            ready_snapshot_timeout_count: 12,
            last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
            last_timeout_idle_ms: Some(13),
            reset_count: 14,
            reconnect_reset_count: 15,
            world_reload_count: 16,
            kick_reset_count: 17,
            last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
            last_world_reload: Some(RuntimeWorldReloadPanelModel {
                had_loaded_world: true,
                had_client_loaded: false,
                was_ready_to_enter_world: true,
                had_connect_confirm_sent: false,
                cleared_pending_packets: 4,
                cleared_deferred_inbound_packets: 5,
                cleared_replayed_loading_events: 6,
            }),
        };
        assert_eq!(
            format_runtime_loading_detail_text_if_nonempty(&panel),
            Some(
                "loadingd:rdy8@9:to10/11/12:ready@13:rs14/15/16/17:reload:@lw1:cl0:rd1:cc0:p4:d5:r6"
                    .to_string()
            )
        );
    }

    fn sample_runtime_session_panel() -> RuntimeSessionPanelModel {
        RuntimeSessionPanelModel {
            bootstrap: RuntimeBootstrapPanelModel {
                rules_label: "rules".to_string(),
                tags_label: "tags".to_string(),
                locales_label: "loc".to_string(),
                team_count: 1,
                marker_count: 2,
                custom_chunk_count: 3,
                content_patch_count: 4,
                player_team_plan_count: 5,
                static_fog_team_count: 6,
            },
            core_binding: RuntimeCoreBindingPanelModel {
                kind: Some(RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation),
                ambiguous_team_count: 1,
                ambiguous_team_sample: vec![2],
                missing_team_count: 3,
                missing_team_sample: vec![4],
            },
            resource_delta: RuntimeResourceDeltaPanelModel {
                remove_tile_count: 1,
                set_tile_count: 2,
                set_floor_count: 3,
                set_overlay_count: 4,
                set_item_count: 5,
                set_items_count: 6,
                set_liquid_count: 7,
                set_liquids_count: 8,
                clear_items_count: 9,
                clear_liquids_count: 10,
                set_tile_items_count: 11,
                set_tile_liquids_count: 12,
                take_items_count: 13,
                transfer_item_to_count: 14,
                transfer_item_to_unit_count: 15,
                last_kind: Some("to unit".to_string()),
                last_item_id: Some(16),
                last_amount: Some(17),
                last_build_pos: Some(18),
                last_unit: Some(RuntimeCommandUnitRefObservability { kind: 2, value: 19 }),
                last_to_entity_id: Some(20),
                build_count: 21,
                build_stack_count: 22,
                entity_count: 23,
                authoritative_build_update_count: 24,
                delta_apply_count: 25,
                delta_skip_count: 26,
                delta_conflict_count: 27,
                last_changed_build_pos: Some(28),
                last_changed_entity_id: Some(29),
                last_changed_item_id: Some(30),
                last_changed_amount: Some(31),
            },
            kick: RuntimeKickPanelModel {
                reason_text: Some("manual".to_string()),
                reason_ordinal: Some(7),
                hint_category: Some("net".to_string()),
                hint_text: Some("retry".to_string()),
            },
            loading: RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 1,
                replayed_inbound_packet_count: 2,
                dropped_loading_low_priority_packet_count: 3,
                dropped_loading_deferred_overflow_count: 4,
                failed_state_snapshot_parse_count: 5,
                failed_state_snapshot_core_data_parse_count: 6,
                failed_entity_snapshot_parse_count: 7,
                ready_inbound_liveness_anchor_count: 8,
                last_ready_inbound_liveness_anchor_at_ms: Some(9),
                timeout_count: 10,
                connect_or_loading_timeout_count: 11,
                ready_snapshot_timeout_count: 12,
                last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
                last_timeout_idle_ms: Some(13),
                reset_count: 14,
                reconnect_reset_count: 15,
                world_reload_count: 16,
                kick_reset_count: 17,
                last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
                last_world_reload: Some(RuntimeWorldReloadPanelModel {
                    had_loaded_world: true,
                    had_client_loaded: false,
                    was_ready_to_enter_world: true,
                    had_connect_confirm_sent: false,
                    cleared_pending_packets: 4,
                    cleared_deferred_inbound_packets: 5,
                    cleared_replayed_loading_events: 6,
                }),
            },
            reconnect: RuntimeReconnectPanelModel {
                phase: RuntimeReconnectPhaseObservability::Attempting,
                phase_transition_count: 3,
                reason_kind: Some(RuntimeReconnectReasonKind::ManualConnect),
                reason_text: Some("manual".to_string()),
                reason_ordinal: Some(7),
                hint_text: Some("retry".to_string()),
                redirect_count: 4,
                last_redirect_ip: Some("1.2.3.4".to_string()),
                last_redirect_port: Some(6567),
            },
        }
    }

    fn empty_runtime_session_panel() -> RuntimeSessionPanelModel {
        RuntimeSessionPanelModel {
            bootstrap: RuntimeBootstrapPanelModel::default(),
            core_binding: RuntimeCoreBindingPanelModel {
                kind: None,
                ambiguous_team_count: 0,
                ambiguous_team_sample: vec![],
                missing_team_count: 0,
                missing_team_sample: vec![],
            },
            resource_delta: RuntimeResourceDeltaPanelModel {
                remove_tile_count: 0,
                set_tile_count: 0,
                set_floor_count: 0,
                set_overlay_count: 0,
                set_item_count: 0,
                set_items_count: 0,
                set_liquid_count: 0,
                set_liquids_count: 0,
                clear_items_count: 0,
                clear_liquids_count: 0,
                set_tile_items_count: 0,
                set_tile_liquids_count: 0,
                take_items_count: 0,
                transfer_item_to_count: 0,
                transfer_item_to_unit_count: 0,
                last_kind: None,
                last_item_id: None,
                last_amount: None,
                last_build_pos: None,
                last_unit: None,
                last_to_entity_id: None,
                build_count: 0,
                build_stack_count: 0,
                entity_count: 0,
                authoritative_build_update_count: 0,
                delta_apply_count: 0,
                delta_skip_count: 0,
                delta_conflict_count: 0,
                last_changed_build_pos: None,
                last_changed_entity_id: None,
                last_changed_item_id: None,
                last_changed_amount: None,
            },
            kick: RuntimeKickPanelModel {
                reason_text: None,
                reason_ordinal: None,
                hint_category: None,
                hint_text: None,
            },
            loading: RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 0,
                replayed_inbound_packet_count: 0,
                dropped_loading_low_priority_packet_count: 0,
                dropped_loading_deferred_overflow_count: 0,
                failed_state_snapshot_parse_count: 0,
                failed_state_snapshot_core_data_parse_count: 0,
                failed_entity_snapshot_parse_count: 0,
                ready_inbound_liveness_anchor_count: 0,
                last_ready_inbound_liveness_anchor_at_ms: None,
                timeout_count: 0,
                connect_or_loading_timeout_count: 0,
                ready_snapshot_timeout_count: 0,
                last_timeout_kind: None,
                last_timeout_idle_ms: None,
                reset_count: 0,
                reconnect_reset_count: 0,
                world_reload_count: 0,
                kick_reset_count: 0,
                last_reset_kind: None,
                last_world_reload: None,
            },
            reconnect: RuntimeReconnectPanelModel {
                phase: RuntimeReconnectPhaseObservability::Idle,
                phase_transition_count: 0,
                reason_kind: None,
                reason_text: None,
                reason_ordinal: None,
                hint_text: None,
                redirect_count: 0,
                last_redirect_ip: None,
                last_redirect_port: None,
            },
        }
    }

    #[test]
    fn format_runtime_session_panel_text_if_nonempty_handles_empty_and_nonempty() {
        assert_eq!(
            format_runtime_session_panel_text_if_nonempty(&sample_runtime_session_panel()),
            Some(
                "sess:bootstrap=rules=rules:tags=tags:locales=loc:teams=1:markers=2:chunks=3:patches=4:plans=5:fog=6;cb=core:first-core-per-team:a1@2:m3@4;rd=resd:tile1/2/3/4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15@to_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31;k=manual@7:net:retry;l=defer1:replay2:drop3:qdrop4:sfail5:scfail6:efail7:rdy8@9:to10:cto11:rto12:ltready@13:rs14:rr15:wr16:kr17:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6;r=attempt3:manual@4/1.2.3.4:6567:manual@7:retry"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_session_panel_text_if_nonempty(&empty_runtime_session_panel()),
            None
        );
    }

    #[test]
    fn format_runtime_session_panel_text_preserves_segment_order() {
        let panel = sample_runtime_session_panel();

        assert_eq!(
            format_runtime_session_panel_text(&panel),
            "sess:bootstrap=rules=rules:tags=tags:locales=loc:teams=1:markers=2:chunks=3:patches=4:plans=5:fog=6;cb=core:first-core-per-team:a1@2:m3@4;rd=resd:tile1/2/3/4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15@to_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31;k=manual@7:net:retry;l=defer1:replay2:drop3:qdrop4:sfail5:scfail6:efail7:rdy8@9:to10:cto11:rto12:ltready@13:rs14:rr15:wr16:kr17:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6;r=attempt3:manual@4/1.2.3.4:6567:manual@7:retry"
        );
    }

    #[test]
    fn format_runtime_session_detail_text_if_nonempty_handles_empty_and_nonempty() {
        assert_eq!(
            format_runtime_session_detail_text_if_nonempty(&sample_runtime_session_panel()),
            Some(
                "sessd:bootstrap(rules-label=rules:tags-label=tags:locales-label=loc:team-count=1:marker-count=2:custom-chunk-count=3:content-patch-count=4:player-team-plan-count=5:static-fog-team-count=6):cb(cored:first-core-per-team:a1@2:m3@4):rd(resdd:rm1:st2:sf3:so4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15:lastto_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31):k(kickd:r6:o7:c3:h5):l(loadingd:rdy8@9:to10/11/12:ready@13:rs14/15/16/17:reload:@lw1:cl0:rd1:cc0:p4:d5:r6):r(reconnectd:attempt#3:manual:r6@7:h5:rd4@1.2.3.4:6567)"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_session_detail_text_if_nonempty(&empty_runtime_session_panel()),
            None
        );
    }

    #[test]
    fn format_runtime_session_detail_text_preserves_segment_order() {
        let panel = sample_runtime_session_panel();

        assert_eq!(
            format_runtime_session_detail_text(&panel),
            "sessd:bootstrap(rules-label=rules:tags-label=tags:locales-label=loc:team-count=1:marker-count=2:custom-chunk-count=3:content-patch-count=4:player-team-plan-count=5:static-fog-team-count=6):cb(cored:first-core-per-team:a1@2:m3@4):rd(resdd:rm1:st2:sf3:so4:set5/6/7/8:clr9/10:tile11/12:flow13/14/15:lastto_unit:16:17:18:2:19:20:proj21/22/23:au24:d25/26/27:chg28/29/30/31):k(kickd:r6:o7:c3:h5):l(loadingd:rdy8@9:to10/11/12:ready@13:rs14/15/16/17:reload:@lw1:cl0:rd1:cc0:p4:d5:r6):r(reconnectd:attempt#3:manual:r6@7:h5:rd4@1.2.3.4:6567)"
        );
    }

    #[test]
    fn format_runtime_session_banner_text_prefers_kick() {
        let panel = RuntimeSessionPanelModel {
            bootstrap: RuntimeBootstrapPanelModel::default(),
            core_binding: RuntimeCoreBindingPanelModel {
                kind: None,
                ambiguous_team_count: 0,
                ambiguous_team_sample: vec![],
                missing_team_count: 0,
                missing_team_sample: vec![],
            },
            resource_delta: RuntimeResourceDeltaPanelModel {
                remove_tile_count: 0,
                set_tile_count: 0,
                set_floor_count: 0,
                set_overlay_count: 0,
                set_item_count: 0,
                set_items_count: 0,
                set_liquid_count: 0,
                set_liquids_count: 0,
                clear_items_count: 0,
                clear_liquids_count: 0,
                set_tile_items_count: 0,
                set_tile_liquids_count: 0,
                take_items_count: 0,
                transfer_item_to_count: 0,
                transfer_item_to_unit_count: 0,
                last_kind: None,
                last_item_id: None,
                last_amount: None,
                last_build_pos: None,
                last_unit: None,
                last_to_entity_id: None,
                build_count: 0,
                build_stack_count: 0,
                entity_count: 0,
                authoritative_build_update_count: 0,
                delta_apply_count: 0,
                delta_skip_count: 0,
                delta_conflict_count: 0,
                last_changed_build_pos: None,
                last_changed_entity_id: None,
                last_changed_item_id: None,
                last_changed_amount: None,
            },
            kick: RuntimeKickPanelModel {
                reason_text: Some("manual".to_string()),
                reason_ordinal: Some(7),
                hint_category: Some("net".to_string()),
                hint_text: Some("retry".to_string()),
            },
            loading: RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 1,
                replayed_inbound_packet_count: 2,
                dropped_loading_low_priority_packet_count: 3,
                dropped_loading_deferred_overflow_count: 4,
                failed_state_snapshot_parse_count: 5,
                failed_state_snapshot_core_data_parse_count: 6,
                failed_entity_snapshot_parse_count: 7,
                ready_inbound_liveness_anchor_count: 8,
                last_ready_inbound_liveness_anchor_at_ms: Some(9),
                timeout_count: 10,
                connect_or_loading_timeout_count: 11,
                ready_snapshot_timeout_count: 12,
                last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
                last_timeout_idle_ms: Some(13),
                reset_count: 14,
                reconnect_reset_count: 15,
                world_reload_count: 16,
                kick_reset_count: 17,
                last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
                last_world_reload: Some(RuntimeWorldReloadPanelModel {
                    had_loaded_world: true,
                    had_client_loaded: false,
                    was_ready_to_enter_world: true,
                    had_connect_confirm_sent: false,
                    cleared_pending_packets: 4,
                    cleared_deferred_inbound_packets: 5,
                    cleared_replayed_loading_events: 6,
                }),
            },
            reconnect: RuntimeReconnectPanelModel {
                phase: RuntimeReconnectPhaseObservability::Attempting,
                phase_transition_count: 3,
                reason_kind: Some(RuntimeReconnectReasonKind::ManualConnect),
                reason_text: Some("manual".to_string()),
                reason_ordinal: Some(7),
                hint_text: Some("retry".to_string()),
                redirect_count: 4,
                last_redirect_ip: Some("1.2.3.4".to_string()),
                last_redirect_port: Some(6567),
            },
        };

        assert_eq!(
            format_runtime_session_banner_text(&panel),
            Some("KICK manual@7:net:retry".to_string())
        );
    }

    #[test]
    fn format_runtime_session_banner_text_joins_reload_reconnect_and_loading() {
        let panel = RuntimeSessionPanelModel {
            bootstrap: RuntimeBootstrapPanelModel::default(),
            core_binding: RuntimeCoreBindingPanelModel {
                kind: None,
                ambiguous_team_count: 0,
                ambiguous_team_sample: vec![],
                missing_team_count: 0,
                missing_team_sample: vec![],
            },
            resource_delta: RuntimeResourceDeltaPanelModel {
                remove_tile_count: 0,
                set_tile_count: 0,
                set_floor_count: 0,
                set_overlay_count: 0,
                set_item_count: 0,
                set_items_count: 0,
                set_liquid_count: 0,
                set_liquids_count: 0,
                clear_items_count: 0,
                clear_liquids_count: 0,
                set_tile_items_count: 0,
                set_tile_liquids_count: 0,
                take_items_count: 0,
                transfer_item_to_count: 0,
                transfer_item_to_unit_count: 0,
                last_kind: None,
                last_item_id: None,
                last_amount: None,
                last_build_pos: None,
                last_unit: None,
                last_to_entity_id: None,
                build_count: 0,
                build_stack_count: 0,
                entity_count: 0,
                authoritative_build_update_count: 0,
                delta_apply_count: 0,
                delta_skip_count: 0,
                delta_conflict_count: 0,
                last_changed_build_pos: None,
                last_changed_entity_id: None,
                last_changed_item_id: None,
                last_changed_amount: None,
            },
            kick: RuntimeKickPanelModel {
                reason_text: None,
                reason_ordinal: None,
                hint_category: None,
                hint_text: None,
            },
            loading: RuntimeLoadingPanelModel {
                deferred_inbound_packet_count: 1,
                replayed_inbound_packet_count: 2,
                dropped_loading_low_priority_packet_count: 3,
                dropped_loading_deferred_overflow_count: 4,
                failed_state_snapshot_parse_count: 5,
                failed_state_snapshot_core_data_parse_count: 6,
                failed_entity_snapshot_parse_count: 7,
                ready_inbound_liveness_anchor_count: 8,
                last_ready_inbound_liveness_anchor_at_ms: Some(9),
                timeout_count: 10,
                connect_or_loading_timeout_count: 11,
                ready_snapshot_timeout_count: 12,
                last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
                last_timeout_idle_ms: Some(13),
                reset_count: 14,
                reconnect_reset_count: 15,
                world_reload_count: 16,
                kick_reset_count: 17,
                last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
                last_world_reload: Some(RuntimeWorldReloadPanelModel {
                    had_loaded_world: true,
                    had_client_loaded: false,
                    was_ready_to_enter_world: true,
                    had_connect_confirm_sent: false,
                    cleared_pending_packets: 4,
                    cleared_deferred_inbound_packets: 5,
                    cleared_replayed_loading_events: 6,
                }),
            },
            reconnect: RuntimeReconnectPanelModel {
                phase: RuntimeReconnectPhaseObservability::Attempting,
                phase_transition_count: 3,
                reason_kind: Some(RuntimeReconnectReasonKind::ManualConnect),
                reason_text: Some("manual".to_string()),
                reason_ordinal: Some(7),
                hint_text: Some("retry".to_string()),
                redirect_count: 4,
                last_redirect_ip: Some("1.2.3.4".to_string()),
                last_redirect_port: Some(6567),
            },
        };

        assert_eq!(
            format_runtime_session_banner_text(&panel),
            Some("RELOAD @lw1:cl0:rd1:cc0:p4:d5:r6 | RECONNECT attempt3:manual@4/1.2.3.4:6567:manual@7:retry | LOADING defer1:replay2:drop3:qdrop4:sfail5:scfail6:efail7:rdy8@9:to10:cto11:rto12:ltready@13:rs14:rr15:wr16:kr17:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6".to_string())
        );
    }

    #[test]
    fn format_runtime_dialog_panel_text_preserves_field_order() {
        let panel = RuntimeDialogPanelModel {
            prompt_kind: Some(RuntimeDialogPromptKind::TextInput),
            prompt_active: true,
            menu_open_count: 16,
            follow_up_menu_open_count: 17,
            hide_follow_up_menu_count: 18,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
            notice_kind: Some(RuntimeDialogNoticeKind::ToastWarning),
            notice_text: Some("warn".to_string()),
            notice_count: 48,
        };

        assert_eq!(
            format_runtime_dialog_panel_text(&panel),
            "dialog:p=input:a1:m16/f17/h18:tin53@404:Digits/Only_numbers/12345#16:n1:e1:n=warn@warn:c48"
        );
    }

    #[test]
    fn format_runtime_dialog_detail_text_preserves_field_order() {
        let panel = RuntimeDialogPanelModel {
            prompt_kind: Some(RuntimeDialogPromptKind::TextInput),
            prompt_active: true,
            menu_open_count: 1,
            follow_up_menu_open_count: 0,
            hide_follow_up_menu_count: 0,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
            notice_kind: Some(RuntimeDialogNoticeKind::ToastWarning),
            notice_text: Some("warn".to_string()),
            notice_count: 48,
        };
        let prompt = RuntimePromptPanelModel {
            kind: Some(RuntimeDialogPromptKind::TextInput),
            menu_active: true,
            text_input_active: true,
            menu_open_count: 1,
            follow_up_menu_open_count: 0,
            hide_follow_up_menu_count: 0,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };
        let notice = RuntimeNoticeStatePanelModel {
            kind: Some(RuntimeDialogNoticeKind::ToastWarning),
            text: Some("warn".to_string()),
            count: 48,
            hud_active: true,
            reliable_hud_active: true,
            toast_info_active: true,
            toast_warning_active: true,
        };

        assert_eq!(
            format_runtime_dialog_detail_text(&panel, &prompt, &notice),
            "dialogd:p=input:a1:m1:fo0:tin53:msg12:def5:n=warn:h1:r1:i1:w1:l4"
        );
    }

    #[test]
    fn format_runtime_ui_notice_panel_text_preserves_field_order() {
        let panel = sample_runtime_ui_notice_panel();

        assert_eq!(
            format_runtime_ui_notice_panel_text(&panel),
            "notice:hud=1/2/3@hud/rel:ann=4@ann:info=5@info:toast=6/7@toasti/toastw:popup=8/9@1:pid/popup:clip=10@clip:uri=11@mindustry_//~:mindustry:tin=12@22:title/msg/def#23:n0:e1"
        );
    }

    #[test]
    fn format_runtime_ui_notice_detail_text_preserves_field_order() {
        let panel = sample_runtime_ui_notice_panel();

        assert_eq!(
            format_runtime_ui_notice_detail_text(&panel),
            Some(
                "noticed:a1:h1/2/3:l3/3:ann4:a3:info5:i4:t6/7:l6/6:popup8/9:r1:pid3:pm5:pd16:pb17:18:19:20:21:clip10:4:uri11:16:mindustry:tin12:id22:t5:m3:d3:n0:e1"
                    .to_string()
            )
        );
    }

    #[test]
    fn format_runtime_ui_notice_detail_text_omits_empty_panel() {
        let panel = RuntimeUiNoticePanelModel {
            hud_set_count: 0,
            hud_set_reliable_count: 0,
            hud_hide_count: 0,
            hud_last_message: None,
            hud_last_reliable_message: None,
            announce_count: 0,
            last_announce_message: None,
            info_message_count: 0,
            last_info_message: None,
            toast_info_count: 0,
            toast_warning_count: 0,
            toast_last_info_message: None,
            toast_last_warning_text: None,
            info_popup_count: 0,
            info_popup_reliable_count: 0,
            last_info_popup_reliable: None,
            last_info_popup_id: None,
            last_info_popup_message: None,
            last_info_popup_duration_bits: None,
            last_info_popup_align: None,
            last_info_popup_top: None,
            last_info_popup_left: None,
            last_info_popup_bottom: None,
            last_info_popup_right: None,
            clipboard_count: 0,
            last_clipboard_text: None,
            open_uri_count: 0,
            last_open_uri: None,
            text_input_open_count: 0,
            text_input_last_id: None,
            text_input_last_title: None,
            text_input_last_message: None,
            text_input_last_default_text: None,
            text_input_last_length: None,
            text_input_last_numeric: None,
            text_input_last_allow_empty: None,
        };

        assert_eq!(format_runtime_ui_notice_detail_text(&panel), None);
    }

    #[test]
    fn format_hud_visibility_detail_text_preserves_field_order() {
        let summary = sample_hud_summary();
        let visibility = sample_hud_visibility_panel();

        assert_eq!(
            format_hud_visibility_detail_text(&summary, &visibility),
            "hudvisd:s=mixed:ov=on:fg=off:k=120/200:v=80/120:h=40/120:u=80/200"
        );
    }

    #[test]
    fn format_hud_visibility_text_preserves_field_order() {
        let visibility = sample_hud_visibility_panel();

        assert_eq!(
            format_hud_visibility_text(&visibility),
            "overlay=1 fog=0 known=120(60%) vis=80(67%) hid=40(33%) unseen=80(40%) vis-map=40% hid-map=20%"
        );
    }

    #[test]
    fn format_hud_visibility_status_text_preserves_field_order() {
        let visibility = sample_hud_visibility_panel();

        assert_eq!(
            format_hud_visibility_status_text(&visibility),
            "hudvis:ov1:fg0:k120p60:v80p67:h40p33:u80p40:vm40:hm20"
        );
    }

    #[test]
    fn format_visibility_minimap_text_preserves_field_order() {
        let visibility = sample_hud_visibility_panel();
        let mut minimap = sample_minimap_panel();
        minimap.map_width = 20;
        minimap.map_height = 10;
        minimap.window = PresenterViewWindow {
            origin_x: 1,
            origin_y: 2,
            width: 8,
            height: 6,
        };
        minimap.window_last_x = 8;
        minimap.window_last_y = 7;
        minimap.window_tile_count = 48;
        minimap.window_coverage_percent = 24;
        minimap.map_tile_count = 200;
        minimap.focus_tile = Some((4, 5));
        minimap.focus_in_window = Some(true);

        assert_eq!(
            format_visibility_minimap_text(&visibility, &minimap),
            "overlay=1 fog=0 known=120(60%) vis=80(67%/40%) hid=40(33%/20%) map=20x10 window=1:2->8:7 size=8x6 cover=48/200(24%) focus=4:5 in-window=1"
        );
    }

    #[test]
    fn format_optional_focus_tile_text_handles_some_and_none() {
        assert_eq!(format_optional_focus_tile_text(Some((4, 5))), "4:5");
        assert_eq!(format_optional_focus_tile_text(None), "-");
    }

    #[test]
    fn format_optional_signed_tile_text_handles_some_and_none() {
        assert_eq!(format_optional_signed_tile_text(Some(-2)), "-2");
        assert_eq!(format_optional_signed_tile_text(Some(3)), "3");
        assert_eq!(format_optional_signed_tile_text(None), "-");
    }

    #[test]
    fn format_minimap_legend_text_preserves_field_order() {
        let summary = sample_hud_summary();

        assert_eq!(
            format_minimap_legend_text(&summary),
            "legend:pl@/mkM/pnP/bk#/rtR/tr./uk?:vis=mixed:ov1:fg0"
        );
    }

    #[test]
    fn format_runtime_notice_state_panel_text_preserves_field_order() {
        let panel = RuntimeNoticeStatePanelModel {
            kind: Some(RuntimeDialogNoticeKind::ToastWarning),
            text: Some("warn".to_string()),
            count: 48,
            hud_active: true,
            reliable_hud_active: true,
            toast_info_active: true,
            toast_warning_active: true,
        };

        assert_eq!(
            format_runtime_notice_state_panel_text(&panel),
            "notice-state:n=warn@warn:src=warn:layers=hud>reliable>info>warn:c48"
        );
    }

    #[test]
    fn format_runtime_notice_state_detail_text_preserves_field_order() {
        let panel = RuntimeNoticeStatePanelModel {
            kind: Some(RuntimeDialogNoticeKind::ToastWarning),
            text: Some("warn".to_string()),
            count: 48,
            hud_active: true,
            reliable_hud_active: true,
            toast_info_active: true,
            toast_warning_active: true,
        };

        assert_eq!(
            format_runtime_notice_state_detail_text(&panel),
            "nstated:n=warn@warn:src=warn:c48:d4:l4:layers=hud>reliable>info>warn"
        );
    }

    #[test]
    fn format_runtime_prompt_panel_text_preserves_field_order() {
        let panel = RuntimePromptPanelModel {
            kind: Some(RuntimeDialogPromptKind::TextInput),
            menu_active: true,
            text_input_active: true,
            menu_open_count: 16,
            follow_up_menu_open_count: 17,
            hide_follow_up_menu_count: 18,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_prompt_panel_text(&panel),
            "prompt:k=input:a1:d2:l=input>menu:m16:fo0:tin53@404:Digits/Only_numbers/12345#16:n1:e1"
        );
    }

    #[test]
    fn format_runtime_prompt_panel_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimePromptPanelModel {
            kind: Some(RuntimeDialogPromptKind::TextInput),
            menu_active: true,
            text_input_active: true,
            menu_open_count: 16,
            follow_up_menu_open_count: 17,
            hide_follow_up_menu_count: 18,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_prompt_panel_text_if_nonempty(&panel),
            Some(
                "prompt:k=input:a1:d2:l=input>menu:m16:fo0:tin53@404:Digits/Only_numbers/12345#16:n1:e1"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_prompt_panel_text_if_nonempty(&RuntimePromptPanelModel {
                kind: None,
                menu_active: false,
                text_input_active: false,
                menu_open_count: 0,
                follow_up_menu_open_count: 0,
                hide_follow_up_menu_count: 0,
                text_input_open_count: 0,
                text_input_last_id: None,
                text_input_last_title: None,
                text_input_last_message: None,
                text_input_last_default_text: None,
                text_input_last_length: None,
                text_input_last_numeric: None,
                text_input_last_allow_empty: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_prompt_detail_text_preserves_field_order() {
        let panel = RuntimePromptPanelModel {
            kind: Some(RuntimeDialogPromptKind::TextInput),
            menu_active: true,
            text_input_active: true,
            menu_open_count: 16,
            follow_up_menu_open_count: 17,
            hide_follow_up_menu_count: 18,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_prompt_detail_text(&panel),
            "pd:ma1:fm17:fh18:fo0:tin53:id404:t6:m12:d5:n1:e1"
        );
    }

    #[test]
    fn format_runtime_prompt_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimePromptPanelModel {
            kind: Some(RuntimeDialogPromptKind::TextInput),
            menu_active: true,
            text_input_active: true,
            menu_open_count: 16,
            follow_up_menu_open_count: 17,
            hide_follow_up_menu_count: 18,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_message: Some("Only numbers".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_prompt_detail_text_if_nonempty(&panel),
            Some("pd:ma1:fm17:fh18:fo0:tin53:id404:t6:m12:d5:n1:e1".to_string())
        );
        assert_eq!(
            format_runtime_prompt_detail_text_if_nonempty(&RuntimePromptPanelModel {
                kind: None,
                menu_active: false,
                text_input_active: false,
                menu_open_count: 0,
                follow_up_menu_open_count: 0,
                hide_follow_up_menu_count: 0,
                text_input_open_count: 0,
                text_input_last_id: None,
                text_input_last_title: None,
                text_input_last_message: None,
                text_input_last_default_text: None,
                text_input_last_length: None,
                text_input_last_numeric: None,
                text_input_last_allow_empty: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_live_entity_summary_text_preserves_field_order() {
        let summary = RuntimeLiveEntitySummaryObservability {
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
            local_position: Some(RuntimeWorldPositionObservability {
                x_bits: 20.0f32.to_bits(),
                y_bits: 33.0f32.to_bits(),
            }),
            local_owned_unit_entity_id: None,
            local_owned_unit_payload_count: None,
            local_owned_unit_payload_class_id: None,
            local_owned_unit_payload_revision: None,
            local_owned_unit_payload_body_len: None,
            local_owned_unit_payload_sha256: None,
            local_owned_unit_payload_nested_descendant_count: None,
            local_owned_carried_item_id: None,
            local_owned_carried_item_amount: None,
            local_owned_controller_type: None,
            local_owned_controller_value: None,
        };

        assert_eq!(
            format_runtime_live_entity_summary_text(&summary),
            "1/0@404:u2/999:p20.0:33.0:h0:s3:tp1/0:last404/404/none"
        );
    }

    #[test]
    fn format_runtime_live_entity_panel_text_preserves_field_order() {
        let panel = sample_runtime_live_entity_panel();

        assert_eq!(
            format_runtime_live_entity_panel_text(&panel),
            "liveent:1/0@404:u2/999:p20.0:33.0:h0:s3:tp1/0:last404/404/none"
        );
    }

    #[test]
    fn format_runtime_live_entity_detail_text_preserves_field_order() {
        let panel = sample_runtime_live_entity_panel();

        assert_eq!(
            format_runtime_live_entity_detail_text(&panel),
            "liveentd:local=404 unit=2/999 pos=20.0:33.0 hidden=0 seen=3 players=1 units=0 last=404/404/none owned=202 payload=count=2:unit=5/r7/l12:s0123456789ab nested=2 stack=6x4 controller=4/101"
        );
    }

    #[test]
    fn format_runtime_live_effect_detail_text_preserves_field_order() {
        let panel = sample_runtime_live_effect_panel();

        assert_eq!(
            format_runtime_live_effect_detail_text(&panel),
            "livefxd:hintpos:point2:3:4@1/0:srcactive:pos28.0:36.0:ttl3/5:data9/4:arel1:ctrlightning:rellightning:bindsource=session session=target:parent-follow/source:parent-follow overlay=target:parent-follow/source:parent-follow active=1 target_counts=1/0/0 source_counts=1/0/0"
        );
    }

    #[test]
    fn format_runtime_live_effect_summary_text_preserves_field_order() {
        let summary = sample_runtime_live_effect_summary();

        assert_eq!(
            format_runtime_live_effect_summary_text(&summary),
            "11/73:ov1@13:u19:d9/4:kPoint2:clightning/lightning:bindtarget:parent-follow/source:parent-follow:r1:hpos:point2:3:4@1/0:pactive@28.0:36.0:ttl3/5"
        );
    }

    #[test]
    fn format_runtime_live_effect_panel_text_preserves_field_order() {
        let panel = sample_runtime_live_effect_panel();

        assert_eq!(
            format_runtime_live_effect_panel_text(&panel),
            "livefx:11/73:ov1@13:u19:d9/4:kPoint2:clightning/lightning:bindtarget:parent-follow/source:parent-follow:r1:hpos:point2:3:4@1/0:pactive@28.0:36.0:ttl3/5"
        );
    }

    #[test]
    fn format_runtime_chat_panel_text_preserves_field_order() {
        let panel = RuntimeChatPanelModel {
            server_message_count: 7,
            last_server_message: Some("server text".to_string()),
            chat_message_count: 8,
            last_chat_message: Some("[cyan]hello".to_string()),
            last_chat_unformatted: Some("hello".to_string()),
            last_chat_sender_entity_id: Some(404),
        };

        assert_eq!(
            format_runtime_chat_panel_text(&panel),
            "chat:srv7@server_text:msg8@[cyan]hello:rawhello:s404"
        );
    }

    #[test]
    fn format_runtime_chat_detail_text_preserves_field_order() {
        let panel = RuntimeChatPanelModel {
            server_message_count: 7,
            last_server_message: Some("server text".to_string()),
            chat_message_count: 8,
            last_chat_message: Some("[cyan]hello".to_string()),
            last_chat_unformatted: Some("hello".to_string()),
            last_chat_sender_entity_id: Some(404),
        };

        assert_eq!(
            format_runtime_chat_detail_text(&panel),
            "chatd:s11:c11:r5:eq0:sid404"
        );
    }

    #[test]
    fn format_runtime_chat_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeChatPanelModel {
            server_message_count: 7,
            last_server_message: Some("server text".to_string()),
            chat_message_count: 8,
            last_chat_message: Some("[cyan]hello".to_string()),
            last_chat_unformatted: Some("hello".to_string()),
            last_chat_sender_entity_id: Some(404),
        };

        assert_eq!(
            format_runtime_chat_detail_text_if_nonempty(&panel),
            Some("chatd:s11:c11:r5:eq0:sid404".to_string())
        );
        assert_eq!(
            format_runtime_chat_detail_text_if_nonempty(&RuntimeChatPanelModel {
                server_message_count: 0,
                last_server_message: None,
                chat_message_count: 0,
                last_chat_message: None,
                last_chat_unformatted: None,
                last_chat_sender_entity_id: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_bootstrap_summary_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeBootstrapPanelModel {
            rules_label: "rules".to_string(),
            tags_label: "tags".to_string(),
            locales_label: "loc".to_string(),
            team_count: 1,
            marker_count: 2,
            custom_chunk_count: 3,
            content_patch_count: 4,
            player_team_plan_count: 5,
            static_fog_team_count: 6,
        };

        assert_eq!(
            format_runtime_bootstrap_summary_text_if_nonempty(&panel),
            Some(
                "rules=rules:tags=tags:locales=loc:teams=1:markers=2:chunks=3:patches=4:plans=5:fog=6"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_bootstrap_summary_text_if_nonempty(&RuntimeBootstrapPanelModel::default()),
            None
        );
    }

    #[test]
    fn format_runtime_bootstrap_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeBootstrapPanelModel {
            rules_label: "rules".to_string(),
            tags_label: "tags".to_string(),
            locales_label: "loc".to_string(),
            team_count: 1,
            marker_count: 2,
            custom_chunk_count: 3,
            content_patch_count: 4,
            player_team_plan_count: 5,
            static_fog_team_count: 6,
        };

        assert_eq!(
            format_runtime_bootstrap_detail_text_if_nonempty(&panel),
            Some(
                "rules-label=rules:tags-label=tags:locales-label=loc:team-count=1:marker-count=2:custom-chunk-count=3:content-patch-count=4:player-team-plan-count=5:static-fog-team-count=6"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_bootstrap_detail_text_if_nonempty(&RuntimeBootstrapPanelModel::default()),
            None
        );
    }

    #[test]
    fn format_runtime_choice_panel_text_preserves_field_order() {
        let panel = RuntimeChoicePanelModel {
            menu_choose_count: 29,
            last_menu_choose_menu_id: Some(404),
            last_menu_choose_option: Some(2),
            text_input_result_count: 30,
            last_text_input_result_id: Some(405),
            last_text_input_result_text: Some("ok123".to_string()),
        };

        assert_eq!(
            format_runtime_choice_panel_text(&panel),
            "choice:mc29@404/2:tir30@405/ok123"
        );
    }

    #[test]
    fn format_runtime_choice_panel_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeChoicePanelModel {
            menu_choose_count: 29,
            last_menu_choose_menu_id: Some(404),
            last_menu_choose_option: Some(2),
            text_input_result_count: 30,
            last_text_input_result_id: Some(405),
            last_text_input_result_text: Some("ok123".to_string()),
        };

        assert_eq!(
            format_runtime_choice_panel_text_if_nonempty(&panel),
            Some("choice:mc29@404/2:tir30@405/ok123".to_string())
        );
        assert_eq!(
            format_runtime_choice_panel_text_if_nonempty(&RuntimeChoicePanelModel {
                menu_choose_count: 0,
                last_menu_choose_menu_id: None,
                last_menu_choose_option: None,
                text_input_result_count: 0,
                last_text_input_result_id: None,
                last_text_input_result_text: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_choice_detail_text_preserves_field_order() {
        let panel = RuntimeChoicePanelModel {
            menu_choose_count: 29,
            last_menu_choose_menu_id: Some(404),
            last_menu_choose_option: Some(2),
            text_input_result_count: 30,
            last_text_input_result_id: Some(405),
            last_text_input_result_text: Some("ok123".to_string()),
        };

        assert_eq!(
            format_runtime_choice_detail_text(&panel),
            "choiced:mid404:opt2:rid405:rlen5"
        );
    }

    #[test]
    fn format_runtime_choice_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeChoicePanelModel {
            menu_choose_count: 29,
            last_menu_choose_menu_id: Some(404),
            last_menu_choose_option: Some(2),
            text_input_result_count: 30,
            last_text_input_result_id: Some(405),
            last_text_input_result_text: Some("ok123".to_string()),
        };

        assert_eq!(
            format_runtime_choice_detail_text_if_nonempty(&panel),
            Some("choiced:mid404:opt2:rid405:rlen5".to_string())
        );
        assert_eq!(
            format_runtime_choice_detail_text_if_nonempty(&RuntimeChoicePanelModel {
                menu_choose_count: 0,
                last_menu_choose_menu_id: None,
                last_menu_choose_option: None,
                text_input_result_count: 0,
                last_text_input_result_id: None,
                last_text_input_result_text: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_menu_detail_text_preserves_field_order() {
        let panel = RuntimeMenuPanelModel {
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
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_menu_detail_text(&panel),
            "menud:a1:fo0:m40:4:4:2:3:fm41:6:4:1:2:hid41:tin53:id404:tDigits:d5:n1:e1"
        );
    }

    #[test]
    fn format_runtime_menu_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeMenuPanelModel {
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
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_menu_detail_text_if_nonempty(&panel),
            Some(
                "menud:a1:fo0:m40:4:4:2:3:fm41:6:4:1:2:hid41:tin53:id404:tDigits:d5:n1:e1"
                    .to_string()
            )
        );
        assert_eq!(
            format_runtime_menu_detail_text_if_nonempty(&RuntimeMenuPanelModel {
                menu_open_count: 0,
                follow_up_menu_open_count: 0,
                hide_follow_up_menu_count: 0,
                last_menu_open_id: None,
                last_menu_open_title: None,
                last_menu_open_message: None,
                last_menu_open_option_rows: 0,
                last_menu_open_first_row_len: 0,
                last_follow_up_menu_open_id: None,
                last_follow_up_menu_open_title: None,
                last_follow_up_menu_open_message: None,
                last_follow_up_menu_open_option_rows: 0,
                last_follow_up_menu_open_first_row_len: 0,
                last_hide_follow_up_menu_id: None,
                text_input_open_count: 0,
                text_input_last_id: None,
                text_input_last_title: None,
                text_input_last_default_text: None,
                text_input_last_length: None,
                text_input_last_numeric: None,
                text_input_last_allow_empty: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_menu_panel_text_preserves_field_order() {
        let panel = RuntimeMenuPanelModel {
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
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            text_input_last_title: Some("Digits".to_string()),
            text_input_last_default_text: Some("12345".to_string()),
            text_input_last_length: Some(16),
            text_input_last_numeric: Some(true),
            text_input_last_allow_empty: Some(true),
        };

        assert_eq!(
            format_runtime_menu_panel_text(&panel),
            "menu:m16@40:main/pick#2:3:fm17@41:follow/next#1:2:h18@41:tin53@404:Digits/12345#16:n1:e1"
        );
    }

    #[test]
    fn format_runtime_rules_panel_text_preserves_field_order() {
        let panel = RuntimeRulesPanelModel {
            mutation_count: 64,
            parse_fail_count: 65,
            set_rules_count: 67,
            set_objectives_count: 69,
            set_rule_count: 71,
            clear_objectives_count: 73,
            complete_objective_count: 74,
            waves: Some(true),
            pvp: Some(false),
            objective_count: 11,
            qualified_objective_count: 7,
            objective_parent_edge_count: 13,
            objective_flag_count: 17,
            complete_out_of_range_count: 19,
            last_completed_index: Some(23),
        };

        assert_eq!(
            format_runtime_rules_panel_text(&panel),
            "rules:mut64:fail65:wv1:pvp0:obj11:q7:par13:fg17:oor19:last23"
        );
    }

    #[test]
    fn format_runtime_rules_detail_text_preserves_field_order() {
        let panel = RuntimeRulesPanelModel {
            mutation_count: 64,
            parse_fail_count: 65,
            set_rules_count: 67,
            set_objectives_count: 69,
            set_rule_count: 71,
            clear_objectives_count: 73,
            complete_objective_count: 74,
            waves: Some(true),
            pvp: Some(false),
            objective_count: 11,
            qualified_objective_count: 7,
            objective_parent_edge_count: 13,
            objective_flag_count: 17,
            complete_out_of_range_count: 19,
            last_completed_index: Some(23),
        };

        assert_eq!(
            format_runtime_rules_detail_text(&panel),
            "rulesd:set67:obj69:rule71:clr73:done74"
        );
    }

    #[test]
    fn format_runtime_rules_detail_text_if_nonempty_handles_empty_and_nonempty() {
        let panel = RuntimeRulesPanelModel {
            mutation_count: 64,
            parse_fail_count: 65,
            set_rules_count: 67,
            set_objectives_count: 69,
            set_rule_count: 71,
            clear_objectives_count: 73,
            complete_objective_count: 74,
            waves: Some(true),
            pvp: Some(false),
            objective_count: 11,
            qualified_objective_count: 7,
            objective_parent_edge_count: 13,
            objective_flag_count: 17,
            complete_out_of_range_count: 19,
            last_completed_index: Some(23),
        };

        assert_eq!(
            format_runtime_rules_detail_text_if_nonempty(&panel),
            Some("rulesd:set67:obj69:rule71:clr73:done74".to_string())
        );
        assert_eq!(
            format_runtime_rules_detail_text_if_nonempty(&RuntimeRulesPanelModel {
                mutation_count: 0,
                parse_fail_count: 0,
                set_rules_count: 0,
                set_objectives_count: 0,
                set_rule_count: 0,
                clear_objectives_count: 0,
                complete_objective_count: 0,
                waves: None,
                pvp: None,
                objective_count: 0,
                qualified_objective_count: 0,
                objective_parent_edge_count: 0,
                objective_flag_count: 0,
                complete_out_of_range_count: 0,
                last_completed_index: None,
            }),
            None
        );
    }

    #[test]
    fn format_runtime_admin_panel_text_preserves_field_order() {
        let panel = RuntimeAdminPanelModel {
            trace_info_count: 11,
            trace_info_parse_fail_count: 3,
            last_trace_info_player_id: Some(404),
            debug_status_client_count: 7,
            debug_status_client_parse_fail_count: 2,
            debug_status_client_unreliable_count: 5,
            debug_status_client_unreliable_parse_fail_count: 1,
            last_debug_status_value: Some(9),
            parse_fail_count: 4,
        };

        assert_eq!(
            format_runtime_admin_panel_text(&panel),
            "admin:t11@404:f3:dbg7/5@9:f4"
        );
    }

    #[test]
    fn format_runtime_admin_detail_text_preserves_field_order() {
        let panel = RuntimeAdminPanelModel {
            trace_info_count: 11,
            trace_info_parse_fail_count: 3,
            last_trace_info_player_id: Some(404),
            debug_status_client_count: 7,
            debug_status_client_parse_fail_count: 2,
            debug_status_client_unreliable_count: 5,
            debug_status_client_unreliable_parse_fail_count: 1,
            last_debug_status_value: Some(9),
            parse_fail_count: 4,
        };

        assert_eq!(
            format_runtime_admin_detail_text(&panel),
            "admind:tr11/3@404:dbg7/2:udbg5/1:last9"
        );
    }

    #[test]
    fn format_runtime_admin_detail_text_if_nonempty_handles_empty_and_nonempty() {
        assert_eq!(
            format_runtime_admin_detail_text_if_nonempty(&RuntimeAdminPanelModel {
                trace_info_count: 0,
                trace_info_parse_fail_count: 0,
                last_trace_info_player_id: None,
                debug_status_client_count: 0,
                debug_status_client_parse_fail_count: 0,
                debug_status_client_unreliable_count: 0,
                debug_status_client_unreliable_parse_fail_count: 0,
                last_debug_status_value: None,
                parse_fail_count: 0,
            }),
            None
        );

        let panel = RuntimeAdminPanelModel {
            trace_info_count: 11,
            trace_info_parse_fail_count: 3,
            last_trace_info_player_id: Some(404),
            debug_status_client_count: 7,
            debug_status_client_parse_fail_count: 2,
            debug_status_client_unreliable_count: 5,
            debug_status_client_unreliable_parse_fail_count: 1,
            last_debug_status_value: Some(9),
            parse_fail_count: 4,
        };
        assert_eq!(
            format_runtime_admin_detail_text_if_nonempty(&panel),
            Some("admind:tr11/3@404:dbg7/2:udbg5/1:last9".to_string())
        );
    }

    #[test]
    fn format_runtime_stack_panel_text_preserves_field_order() {
        let panel = RuntimeUiStackPanelModel {
            foreground_kind: Some(RuntimeUiStackForegroundKind::TextInput),
            menu_active: true,
            outstanding_follow_up_count: 0,
            text_input_active: true,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            notice_kind: Some(RuntimeDialogNoticeKind::ToastWarning),
            hud_notice_active: true,
            reliable_hud_notice_active: true,
            toast_info_active: true,
            toast_warning_active: true,
            chat_active: true,
            server_message_count: 7,
            chat_message_count: 8,
            last_chat_sender_entity_id: Some(404),
        };

        assert_eq!(
            format_runtime_stack_panel_text(&panel),
            "stack:f=input:p2@input>menu:n=warn@hud>reliable>info>warn:c1:g3:t7:tin404:s404"
        );
    }

    #[test]
    fn format_runtime_stack_detail_text_preserves_field_order() {
        let panel = RuntimeDialogStackPanelModel {
            foreground_kind: Some(RuntimeUiStackForegroundKind::TextInput),
            prompt: RuntimePromptPanelModel {
                kind: Some(RuntimeDialogPromptKind::TextInput),
                menu_active: true,
                text_input_active: true,
                menu_open_count: 1,
                follow_up_menu_open_count: 0,
                hide_follow_up_menu_count: 0,
                text_input_open_count: 53,
                text_input_last_id: Some(404),
                text_input_last_title: Some("Digits".to_string()),
                text_input_last_message: Some("Only numbers".to_string()),
                text_input_last_default_text: Some("12345".to_string()),
                text_input_last_length: Some(16),
                text_input_last_numeric: Some(true),
                text_input_last_allow_empty: Some(true),
            },
            notice: RuntimeNoticeStatePanelModel {
                kind: Some(RuntimeDialogNoticeKind::ToastWarning),
                text: Some("warn".to_string()),
                count: 48,
                hud_active: true,
                reliable_hud_active: true,
                toast_info_active: true,
                toast_warning_active: true,
            },
            chat: RuntimeChatPanelModel {
                server_message_count: 7,
                last_server_message: Some("server text".to_string()),
                chat_message_count: 8,
                last_chat_message: Some("[cyan]hello".to_string()),
                last_chat_unformatted: Some("hello".to_string()),
                last_chat_sender_entity_id: Some(404),
            },
        };

        assert_eq!(
            format_runtime_stack_detail_text(&panel),
            "stackd:f=input:g3:t7:p=input:m1:fo0:i53:n=warn:h1:r1:i1:w1:c1:7/8:sid404"
        );
    }

    #[test]
    fn format_runtime_stack_depth_text_preserves_field_order() {
        let summary = RuntimeUiStackDepthSummary {
            prompt_depth: 2,
            notice_depth: 4,
            chat_depth: 1,
            active_group_count: 3,
            total_depth: 7,
        };

        assert_eq!(
            format_runtime_stack_depth_text(&summary),
            "sdepth:p2:n4:c1:m2:h4:d7:g3:t7"
        );
    }

    #[test]
    fn format_runtime_dialog_stack_summary_text_preserves_field_order() {
        let summary = RuntimeUiStackSummary {
            foreground_kind: Some(RuntimeUiStackForegroundSummaryKind::TextInput),
            prompt_kind: Some(RuntimeUiPromptLayerKind::TextInput),
            prompt_layers: vec![RuntimeUiPromptLayerKind::TextInput, RuntimeUiPromptLayerKind::Menu],
            notice_kind: Some(RuntimeUiNoticeLayerKind::ToastWarning),
            notice_layers: vec![
                RuntimeUiNoticeLayerKind::Hud,
                RuntimeUiNoticeLayerKind::HudReliable,
                RuntimeUiNoticeLayerKind::ToastInfo,
                RuntimeUiNoticeLayerKind::ToastWarning,
            ],
            chat_active: true,
            menu_open_count: 16,
            outstanding_follow_up_count: 0,
            text_input_open_count: 53,
            text_input_last_id: Some(404),
            server_message_count: 7,
            chat_message_count: 8,
            last_chat_sender_entity_id: Some(404),
        };

        assert_eq!(
            format_runtime_dialog_stack_summary_text(&summary),
            "stackx:f=input:p=input@input>menu:m16:fo0:i53:n=warn@hud>reliable>info>warn:md2:hd4:c1:7/8:tin404:s404:dd7:t7"
        );
    }

    #[test]
    fn format_runtime_command_leaf_text_helpers_format_expected_payloads() {
        assert_eq!(format_runtime_command_i32_list_text(&[]), "none");
        assert_eq!(format_runtime_command_i32_list_text(&[11, 22, 33]), "11,22,33");

        assert_eq!(format_runtime_command_rect_text(None), "none");
        assert_eq!(
            format_runtime_command_rect_text(Some(RuntimeCommandRectObservability {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            })),
            "-3:4:12:18"
        );

        assert_eq!(
            format_runtime_command_control_groups_text(&[
                RuntimeCommandControlGroupPanelModel {
                    index: 2,
                    unit_count: 3,
                    first_unit_id: Some(11),
                },
                RuntimeCommandControlGroupPanelModel {
                    index: 4,
                    unit_count: 1,
                    first_unit_id: Some(99),
                },
            ]),
            "2#3@11,4#1@99"
        );
        assert_eq!(
            format_runtime_command_control_group_operation_text(Some(
                RuntimeCommandRecentControlGroupOperationObservability::Recall
            )),
            "group-recall"
        );
        assert_eq!(
            format_runtime_command_control_group_operation_text(None),
            "none"
        );
    }

    #[test]
    fn format_runtime_command_target_text_formats_all_optional_fields() {
        assert_eq!(format_runtime_command_target_text(None), "none");
        assert_eq!(
            format_runtime_command_target_text(Some(RuntimeCommandTargetObservability {
                build_target: Some(589834),
                unit_target: Some(RuntimeCommandUnitRefObservability { kind: 2, value: 808 }),
                position_target: Some(RuntimeWorldPositionObservability {
                    x_bits: 0x4240_0000,
                    y_bits: 0x42c0_0000,
                }),
                rect_target: Some(RuntimeCommandRectObservability {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
            })),
            "b589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4"
        );
    }

    #[test]
    fn format_runtime_command_unit_and_stance_text_handle_missing_values() {
        assert_eq!(format_runtime_command_unit_ref_text(None), "none");
        assert_eq!(
            format_runtime_command_unit_ref_text(Some(RuntimeCommandUnitRefObservability {
                kind: 2,
                value: 808,
            })),
            "2:808"
        );
        assert_eq!(format_runtime_command_stance_text(None), "none");
        assert_eq!(
            format_runtime_command_stance_text(Some(RuntimeCommandStanceObservability {
                stance_id: Some(7),
                enabled: false,
            })),
            "7/0"
        );
    }

    #[test]
    fn format_runtime_command_mode_panel_text_preserves_field_order() {
        let panel = RuntimeCommandModePanelModel {
            active: true,
            selected_unit_count: 4,
            selected_unit_sample: vec![11, 22, 33],
            command_building_count: 2,
            first_command_building: Some(327686),
            command_rect: Some(RuntimeCommandRectObservability {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            }),
            control_groups: vec![
                RuntimeCommandControlGroupPanelModel {
                    index: 2,
                    unit_count: 3,
                    first_unit_id: Some(11),
                },
                RuntimeCommandControlGroupPanelModel {
                    index: 4,
                    unit_count: 1,
                    first_unit_id: Some(99),
                },
            ],
            last_control_group_operation: Some(
                RuntimeCommandRecentControlGroupOperationObservability::Recall,
            ),
            last_target: Some(RuntimeCommandTargetObservability {
                build_target: Some(589834),
                unit_target: Some(RuntimeCommandUnitRefObservability { kind: 2, value: 808 }),
                position_target: Some(RuntimeWorldPositionObservability {
                    x_bits: 0x4240_0000,
                    y_bits: 0x42c0_0000,
                }),
                rect_target: Some(RuntimeCommandRectObservability {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
            }),
            last_command_selection: Some(RuntimeCommandSelectionObservability {
                command_id: Some(5),
            }),
            last_stance_selection: Some(RuntimeCommandStanceObservability {
                stance_id: Some(7),
                enabled: false,
            }),
        };

        assert_eq!(
            format_runtime_command_mode_panel_text(&panel),
            "cmd:act1:sel4@11,22,33:bld2@327686:rect-3:4:12:18:grp2#3@11,4#1@99:opgroup-recall:tb589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4:c5:s7/0"
        );
    }

    #[test]
    fn format_runtime_command_mode_detail_text_preserves_field_order() {
        let panel = RuntimeCommandModePanelModel {
            active: false,
            selected_unit_count: 0,
            selected_unit_sample: vec![11, 22, 33],
            command_building_count: 0,
            first_command_building: Some(327686),
            command_rect: Some(RuntimeCommandRectObservability {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            }),
            control_groups: vec![
                RuntimeCommandControlGroupPanelModel {
                    index: 2,
                    unit_count: 3,
                    first_unit_id: Some(11),
                },
                RuntimeCommandControlGroupPanelModel {
                    index: 4,
                    unit_count: 1,
                    first_unit_id: Some(99),
                },
            ],
            last_control_group_operation: Some(
                RuntimeCommandRecentControlGroupOperationObservability::Recall,
            ),
            last_target: Some(RuntimeCommandTargetObservability {
                build_target: Some(589834),
                unit_target: Some(RuntimeCommandUnitRefObservability { kind: 2, value: 808 }),
                position_target: Some(RuntimeWorldPositionObservability {
                    x_bits: 0x4240_0000,
                    y_bits: 0x42c0_0000,
                }),
                rect_target: Some(RuntimeCommandRectObservability {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
            }),
            last_command_selection: Some(RuntimeCommandSelectionObservability {
                command_id: Some(5),
            }),
            last_stance_selection: Some(RuntimeCommandStanceObservability {
                stance_id: Some(7),
                enabled: false,
            }),
        };

        assert_eq!(
            format_runtime_command_mode_detail_text(&panel),
            "cmdd:sample11,22,33:grp2#3@11,4#1@99:opgroup-recall:bld327686:rect-3:4:12:18:tb589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4:c5:s7/0"
        );
    }

    #[test]
    fn format_runtime_command_group_lines_preserves_order_and_group_count() {
        let panel = RuntimeCommandModePanelModel {
            active: false,
            selected_unit_count: 0,
            selected_unit_sample: Vec::new(),
            command_building_count: 0,
            first_command_building: None,
            command_rect: None,
            control_groups: vec![
                RuntimeCommandControlGroupPanelModel {
                    index: 2,
                    unit_count: 3,
                    first_unit_id: Some(11),
                },
                RuntimeCommandControlGroupPanelModel {
                    index: 4,
                    unit_count: 1,
                    first_unit_id: Some(99),
                },
            ],
            last_control_group_operation: None,
            last_target: None,
            last_command_selection: None,
            last_stance_selection: None,
        };

        assert_eq!(
            format_runtime_command_group_lines(&panel),
            vec![
                "cmdg:1/2:g2#3@11".to_string(),
                "cmdg:2/2:g4#1@99".to_string(),
            ]
        );
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
    fn format_build_config_alignment_text_handles_all_variants() {
        assert_eq!(format_build_config_alignment_text(Some(true)), "match");
        assert_eq!(format_build_config_alignment_text(Some(false)), "split");
        assert_eq!(format_build_config_alignment_text(None), "none");
    }

    #[test]
    fn format_optional_i16_text_handles_some_and_none() {
        assert_eq!(format_optional_i16_text(Some(-2)), "-2");
        assert_eq!(format_optional_i16_text(Some(3)), "3");
        assert_eq!(format_optional_i16_text(None), "none");
    }

    #[test]
    fn format_optional_u8_text_handles_some_and_none() {
        assert_eq!(format_optional_u8_text(Some(0)), "0");
        assert_eq!(format_optional_u8_text(Some(7)), "7");
        assert_eq!(format_optional_u8_text(None), "none");
    }

    #[test]
    fn format_optional_bool_flag_handles_all_variants() {
        assert_eq!(format_optional_bool_flag(Some(true)), '1');
        assert_eq!(format_optional_bool_flag(Some(false)), '0');
        assert_eq!(format_optional_bool_flag(None), 'n');
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
    fn format_minimap_kind_text_preserves_field_order() {
        let mut panel = sample_minimap_panel();
        panel.tracked_object_count = 7;
        panel.player_count = 1;
        panel.marker_count = 2;
        panel.plan_count = 3;
        panel.block_count = 4;
        panel.runtime_count = 5;
        panel.terrain_count = 6;
        panel.unknown_count = 8;

        assert_eq!(
            format_minimap_kind_text(&panel),
            "minikind:obj7@pl1:mk2:pn3:bk4:rt5:tr6:uk8"
        );
    }

    #[test]
    fn format_minimap_density_visibility_text_preserves_field_order() {
        let mut panel = sample_minimap_panel();
        panel.overlay_visible = true;
        panel.fog_enabled = false;
        panel.window_coverage_percent = 24;
        panel.tracked_object_count = 20;
        panel.map_tile_count = 200;
        panel.window_tracked_object_count = 12;
        panel.window_tile_count = 48;
        panel.outside_window_count = 5;

        assert_eq!(
            format_minimap_density_visibility_text(&panel),
            "minidv:ov1:fg0:cov24:mapd10:wind25:out25"
        );
    }

    #[test]
    fn format_minimap_detail_lines_preserves_order_and_appends_density_line() {
        let mut panel = sample_minimap_panel();
        panel.overlay_visible = true;
        panel.fog_enabled = false;
        panel.window_coverage_percent = 24;
        panel.tracked_object_count = 20;
        panel.map_tile_count = 200;
        panel.window_tracked_object_count = 12;
        panel.window_tile_count = 48;
        panel.outside_window_count = 5;
        panel.detail_counts = vec![
            RenderSemanticDetailCount {
                label: "player",
                count: 1,
            },
            RenderSemanticDetailCount {
                label: "runtime",
                count: 2,
            },
        ];

        assert_eq!(
            format_minimap_detail_lines(&panel),
            vec![
                "minid:1/2:player=1".to_string(),
                "minid:2/2:runtime=2".to_string(),
                "minidv:ov1:fg0:cov24:mapd10:wind25:out25".to_string(),
            ]
        );
    }

    #[test]
    fn format_minimap_edge_detail_text_preserves_field_order() {
        let mut panel = sample_minimap_panel();
        panel.window = PresenterViewWindow {
            origin_x: 1,
            origin_y: 2,
            width: 8,
            height: 6,
        };
        panel.window_last_x = 8;
        panel.window_last_y = 7;
        panel.window_coverage_percent = 24;
        panel.focus_tile = Some((4, 5));
        panel.focus_in_window = Some(true);
        panel.focus_offset_x = Some(-2);
        panel.focus_offset_y = Some(3);
        panel.window_clamped_left = true;
        panel.window_clamped_bottom = true;
        panel.outside_window_count = 5;
        panel.tracked_object_count = 20;
        panel.window_tracked_object_count = 12;

        assert_eq!(
            format_minimap_edge_detail_text(&panel),
            "origin=1:2 last=8:7 size=8x6 cover=24% focus=4:5 in-window=1 drift=-2:3 clamp=left+bottom outside=5/20 window=12/20"
        );
    }

    #[test]
    fn format_semantic_detail_text_handles_empty_and_preserves_order() {
        assert_eq!(format_semantic_detail_text(&[]), None);

        let detail_counts = vec![
            RenderSemanticDetailCount {
                label: "player",
                count: 1,
            },
            RenderSemanticDetailCount {
                label: "runtime",
                count: 2,
            },
        ];

        assert_eq!(
            format_semantic_detail_text(&detail_counts),
            Some("player:1,runtime:2".to_string())
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
    fn format_render_primitive_payload_fields_with_keeps_variant_first() {
        let payload = RenderPrimitivePayload {
            label: "icon".to_string(),
            fields: BTreeMap::from([
                ("content_id", RenderPrimitivePayloadValue::I32(7)),
                (
                    "variant",
                    RenderPrimitivePayloadValue::Text("content".to_string()),
                ),
                ("x_bits", RenderPrimitivePayloadValue::U32(0x41000000)),
            ]),
        };

        assert_eq!(
            format_render_primitive_payload_fields_with(&payload, |name, value| match value {
                RenderPrimitivePayloadValue::Text(value) => value.clone(),
                RenderPrimitivePayloadValue::I32(value) => value.to_string(),
                RenderPrimitivePayloadValue::U32(value) => {
                    if name.ends_with("_bits") {
                        format!("0x{value:08x}")
                    } else {
                        value.to_string()
                    }
                }
                _ => unreachable!("test payload only uses text, i32 and u32"),
            }),
            "variant=content,content_id=7,x_bits=0x41000000"
        );
    }

    #[test]
    fn format_render_primitive_payload_value_with_uses_bool_and_u32_strategies() {
        assert_eq!(
            format_render_primitive_payload_value_with(
                "active",
                &RenderPrimitivePayloadValue::Bool(true),
                |value| if value { "1".to_string() } else { "0".to_string() },
                |_field_name, value| value.to_string(),
            ),
            "1"
        );
        assert_eq!(
            format_render_primitive_payload_value_with(
                "x_bits",
                &RenderPrimitivePayloadValue::U32(0x41000000),
                |value| value.to_string(),
                |field_name, value| {
                    if field_name.ends_with("_bits") {
                        format!("0x{value:08x}")
                    } else {
                        value.to_string()
                    }
                },
            ),
            "0x41000000"
        );
        assert_eq!(
            format_render_primitive_payload_value_with(
                "tags",
                &RenderPrimitivePayloadValue::TextList(vec!["a".to_string(), "b".to_string()]),
                |value| value.to_string(),
                |_field_name, value| value.to_string(),
            ),
            "[a,b]"
        );
    }

    #[test]
    fn format_render_line_signature_preserves_coordinate_template() {
        assert_eq!(
            format_render_line_signature("trace", 2, 1, 3, 5, 8),
            "trace@2:1:3->5:8"
        );
    }

    #[test]
    fn format_render_rect_signature_preserves_coordinate_template() {
        assert_eq!(
            format_render_rect_signature("command", 4, 1, 2, 3, 4),
            "command@4:1:2:3:4"
        );
    }

    #[test]
    fn format_render_icon_signature_preserves_coordinate_template() {
        assert_eq!(
            format_render_icon_signature("runtime-break", "break", 7, 3, 5),
            "runtime-break/break@7:3:5"
        );
    }

    #[test]
    fn format_world_position_status_text_handles_missing_finite_and_nonfinite_values() {
        assert_eq!(format_world_position_status_text(None), "none");
        assert_eq!(
            format_world_position_status_text(Some(&RuntimeWorldPositionObservability {
                x_bits: 12.5f32.to_bits(),
                y_bits: 7.0f32.to_bits(),
            })),
            "12.5:7.0"
        );
        assert_eq!(
            format_world_position_status_text(Some(&RuntimeWorldPositionObservability {
                x_bits: f32::NAN.to_bits(),
                y_bits: f32::NEG_INFINITY.to_bits(),
            })),
            format!(
                "0x{:08x}:0x{:08x}",
                f32::NAN.to_bits(),
                f32::NEG_INFINITY.to_bits()
            )
        );
    }

    #[test]
    fn format_live_effect_position_source_text_maps_all_variants() {
        assert_eq!(format_live_effect_position_source_text(None), "none");
        assert_eq!(
            format_live_effect_position_source_text(Some(
                RuntimeLiveEffectPositionSource::ActiveOverlay
            )),
            "active"
        );
        assert_eq!(
            format_live_effect_position_source_text(Some(
                RuntimeLiveEffectPositionSource::BusinessProjection
            )),
            "biz"
        );
        assert_eq!(
            format_live_effect_position_source_text(Some(
                RuntimeLiveEffectPositionSource::EffectPacket
            )),
            "pkt"
        );
        assert_eq!(
            format_live_effect_position_source_text(Some(
                RuntimeLiveEffectPositionSource::SpawnEffectPacket
            )),
            "spawn"
        );
    }

    #[test]
    fn format_live_effect_ttl_text_handles_missing_and_present_values() {
        assert_eq!(format_live_effect_ttl_text(None), "none");
        assert_eq!(format_live_effect_ttl_text(Some((3, 5))), "3/5");
    }

    #[test]
    fn format_live_effect_data_shape_text_handles_partial_values() {
        assert_eq!(format_live_effect_data_shape_text(None, None), "none");
        assert_eq!(format_live_effect_data_shape_text(Some(9), Some(4)), "9/4");
        assert_eq!(format_live_effect_data_shape_text(Some(9), None), "9/none");
        assert_eq!(
            format_live_effect_data_shape_text(None, Some(4)),
            "none/4"
        );
    }

    #[test]
    fn format_live_effect_reliable_flag_text_handles_all_variants() {
        assert_eq!(format_live_effect_reliable_flag_text(Some(true)), "1");
        assert_eq!(format_live_effect_reliable_flag_text(Some(false)), "0");
        assert_eq!(format_live_effect_reliable_flag_text(None), "?");
    }

    #[test]
    fn format_render_text_signature_preserves_coordinate_template() {
        assert_eq!(
            format_render_text_signature("label", 3, 4, 5),
            "label@3:4:5"
        );
    }

    #[test]
    fn format_counted_preview_text_preserves_count_and_more_suffix() {
        assert_eq!(
            format_counted_preview_text(4, vec!["a".to_string(), "b".to_string()]),
            "count=4 a b more=2"
        );
        assert_eq!(
            format_counted_preview_text(2, vec!["a".to_string(), "b".to_string()]),
            "count=2 a b"
        );
    }

    #[test]
    fn format_counted_detail_text_preserves_separator() {
        assert_eq!(
            format_counted_detail_text(2, " | ", vec!["a".to_string(), "b".to_string()]),
            "count=2 | a | b"
        );
        assert_eq!(
            format_counted_detail_text(2, " ", vec!["a".to_string(), "b".to_string()]),
            "count=2 a b"
        );
    }

    #[test]
    fn format_render_rect_detail_fields_preserves_field_order() {
        assert_eq!(
            format_render_rect_detail_fields(1, 2, 3, 4, 4, Some("duo"), Some(1), Some(2)),
            "left_tile=1,top_tile=2,right_tile=3,bottom_tile=4,width_tiles=2,height_tiles=2,line_count=4,block_name=duo,tile_x=1,tile_y=2"
        );
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
