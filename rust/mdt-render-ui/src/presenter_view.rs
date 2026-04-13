use crate::{
    hud_model::{RuntimeUiStackDepthSummary, RuntimeUiStackSummary},
    panel_model::{
        MinimapPanelModel, PresenterViewWindow, RuntimeChatPanelModel,
        RuntimeCommandControlGroupPanelModel, RuntimeCommandModePanelModel,
        RuntimeDialogNoticeKind, RuntimeDialogPanelModel, RuntimeDialogPromptKind,
        RuntimeDialogStackPanelModel, RuntimeNoticeStatePanelModel, RuntimePromptPanelModel,
        RuntimeUiNoticePanelModel, RuntimeUiStackPanelModel,
    },
    render_model::{RenderPrimitivePayload, RenderPrimitivePayloadValue},
    BuildQueueHeadStage, RenderModel, RenderObject,
    RuntimeCommandRecentControlGroupOperationObservability, RuntimeCommandRectObservability,
    RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
    RuntimeCommandUnitRefObservability, RuntimeLiveEffectPositionSource,
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

pub(crate) fn compose_minimap_window_distribution_text(panel: &MinimapPanelModel) -> String {
    format_minimap_window_counts_text("miniwin:", ":", panel)
}

pub(crate) fn compose_minimap_window_kind_distribution_text(panel: &MinimapPanelModel) -> String {
    format_minimap_window_counts_text("miniwin-kinds: ", " ", panel)
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

fn format_optional_bool_flag(value: Option<bool>) -> char {
    match value {
        Some(true) => '1',
        Some(false) => '0',
        None => 'n',
    }
}

fn format_optional_u8_text(value: Option<u8>) -> String {
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
        format_counted_detail_text, format_counted_preview_text,
        format_runtime_command_control_group_operation_text,
        format_runtime_command_group_lines,
        format_runtime_command_control_groups_text, format_runtime_command_i32_list_text,
        format_runtime_command_mode_detail_text, format_runtime_command_mode_panel_text,
        format_runtime_notice_state_detail_text, format_runtime_notice_state_panel_text,
        format_runtime_command_rect_text, format_runtime_command_stance_text,
        format_runtime_command_target_text, format_runtime_command_unit_ref_text,
        format_runtime_dialog_stack_summary_text,
        format_runtime_dialog_detail_text, format_runtime_dialog_panel_text,
        format_runtime_dialog_notice_text, format_runtime_dialog_prompt_text,
        format_runtime_prompt_detail_text, format_runtime_prompt_panel_text,
        format_runtime_chat_detail_text, format_runtime_chat_panel_text,
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
            RuntimeUiNoticeLayerKind, RuntimeUiPromptLayerKind,
            RuntimeUiStackDepthSummary, RuntimeUiStackForegroundSummaryKind, RuntimeUiStackSummary,
        },
        panel_model::{
            MinimapPanelModel, PresenterViewWindow, RuntimeChatPanelModel,
            RuntimeCommandControlGroupPanelModel, RuntimeCommandModePanelModel,
            RuntimeDialogNoticeKind, RuntimeDialogPanelModel, RuntimeDialogPromptKind,
            RuntimeDialogStackPanelModel, RuntimeNoticeStatePanelModel, RuntimePromptPanelModel,
            RuntimeUiStackForegroundKind, RuntimeUiStackPanelModel,
        },
        render_model::{RenderPrimitivePayload, RenderPrimitivePayloadValue},
        BuildQueueHeadStage, RenderModel, RenderObject,
        RuntimeCommandRecentControlGroupOperationObservability, RuntimeCommandRectObservability,
        RuntimeCommandSelectionObservability,
        RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
        RuntimeCommandUnitRefObservability, RuntimeLiveEffectPositionSource,
        RuntimeWorldPositionObservability, Viewport,
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
