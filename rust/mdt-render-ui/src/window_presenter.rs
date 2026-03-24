use crate::{
    hud_model::HudSummary, render_model::RenderObjectSemanticKind, BuildQueueHeadObservability,
    BuildQueueHeadStage, BuildUiObservability, HudModel, RenderModel, RenderObject,
    RuntimeUiObservability, ScenePresenter,
};
use minifb::{Scale, Window, WindowOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const TILE_SIZE: f32 = 8.0;
const COLOR_EMPTY: u32 = 0x10131A;
const COLOR_TERRAIN: u32 = 0x263238;
const COLOR_BLOCK: u32 = 0x6D6D6D;
const COLOR_PLAN: u32 = 0x00BCD4;
const COLOR_MARKER: u32 = 0xFFC107;
const COLOR_PLAYER: u32 = 0x66BB6A;
const COLOR_RUNTIME: u32 = 0xFF7043;
const COLOR_UNKNOWN: u32 = 0xEF5350;
const WINDOW_TARGET_FPS: usize = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct WindowFrame {
    pub frame_id: u64,
    pub title: String,
    pub wave_text: Option<String>,
    pub status_text: String,
    pub overlay_summary_text: Option<String>,
    pub fps: Option<f32>,
    pub zoom: f32,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl WindowFrame {
    pub fn pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.pixels[y * self.width + x])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendSignal {
    Continue,
    Close,
}

pub trait WindowBackend {
    fn present(&mut self, frame: &WindowFrame) -> Result<BackendSignal, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRunStats {
    pub frames_rendered: u64,
    pub elapsed_ms: u128,
    pub terminated_by_backend: bool,
}

pub struct WindowPresenter<B> {
    backend: B,
    frame_id: u64,
    frame_interval: Duration,
    max_view_tiles: Option<(usize, usize)>,
    last_error: Option<String>,
}

impl<B: WindowBackend> WindowPresenter<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            frame_id: 0,
            frame_interval: Duration::from_millis(33),
            max_view_tiles: None,
            last_error: None,
        }
    }

    pub fn with_target_fps(mut self, fps: u32) -> Self {
        self.frame_interval = frame_interval_for_fps(fps);
        self
    }

    pub fn with_max_view_tiles(mut self, width: usize, height: usize) -> Self {
        self.max_view_tiles = Some((width, height));
        self
    }

    pub fn frame_id(&self) -> u64 {
        self.frame_id
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }

    pub fn present_once(
        &mut self,
        scene: &RenderModel,
        hud: &HudModel,
    ) -> Result<BackendSignal, String> {
        let frame = compose_frame(scene, hud, self.frame_id, self.max_view_tiles);
        let signal = self.backend.present(&frame)?;
        self.frame_id += 1;
        self.last_error = None;
        Ok(signal)
    }

    pub fn run_offline<F>(
        &mut self,
        max_frames: u64,
        mut frame_source: F,
    ) -> Result<WindowRunStats, String>
    where
        F: FnMut(u64) -> (RenderModel, HudModel),
    {
        let started = Instant::now();
        let mut frames_rendered = 0u64;
        let mut terminated_by_backend = false;

        while frames_rendered < max_frames {
            let frame_started = Instant::now();
            let (scene, hud) = frame_source(self.frame_id);
            let signal = self.present_once(&scene, &hud)?;
            frames_rendered += 1;

            if signal == BackendSignal::Close {
                terminated_by_backend = true;
                break;
            }

            sleep_to_target(frame_started, self.frame_interval);
        }

        Ok(WindowRunStats {
            frames_rendered,
            elapsed_ms: started.elapsed().as_millis(),
            terminated_by_backend,
        })
    }
}

impl<B: WindowBackend> ScenePresenter for WindowPresenter<B> {
    fn present(&mut self, scene: &RenderModel, hud: &HudModel) {
        if let Err(err) = self.present_once(scene, hud) {
            self.last_error = Some(err);
        }
    }
}

pub struct PpmSequenceBackend {
    out_dir: PathBuf,
}

impl PpmSequenceBackend {
    pub fn new(out_dir: impl AsRef<Path>) -> Result<Self, String> {
        let out_dir = out_dir.as_ref().to_path_buf();
        fs::create_dir_all(&out_dir).map_err(|err| err.to_string())?;
        Ok(Self { out_dir })
    }
}

impl WindowBackend for PpmSequenceBackend {
    fn present(&mut self, frame: &WindowFrame) -> Result<BackendSignal, String> {
        let file = self
            .out_dir
            .join(format!("frame-{:05}.ppm", frame.frame_id));
        fs::write(file, encode_ppm(frame)).map_err(|err| err.to_string())?;
        Ok(BackendSignal::Continue)
    }
}

pub struct MinifbWindowBackend {
    tile_pixels: usize,
    title_prefix: String,
    window: Option<Window>,
    surface_size: Option<(usize, usize)>,
}

impl MinifbWindowBackend {
    pub fn new(tile_pixels: usize, title_prefix: impl Into<String>) -> Self {
        Self {
            tile_pixels: tile_pixels.max(1),
            title_prefix: title_prefix.into(),
            window: None,
            surface_size: None,
        }
    }

    fn ensure_window(&mut self, frame: &WindowFrame) -> Result<(), String> {
        let surface_size = (
            frame.width.max(1) * self.tile_pixels,
            frame.height.max(1) * self.tile_pixels,
        );
        let needs_recreate = self
            .surface_size
            .map(|current| current != surface_size)
            .unwrap_or(true);
        if !needs_recreate && self.window.is_some() {
            return Ok(());
        }

        let mut window = Window::new(
            &compose_window_title(frame, &self.title_prefix),
            surface_size.0,
            surface_size.1,
            WindowOptions {
                resize: false,
                scale: Scale::X1,
                ..WindowOptions::default()
            },
        )
        .map_err(|err| err.to_string())?;
        window.set_target_fps(WINDOW_TARGET_FPS);
        self.window = Some(window);
        self.surface_size = Some(surface_size);
        Ok(())
    }
}

impl WindowBackend for MinifbWindowBackend {
    fn present(&mut self, frame: &WindowFrame) -> Result<BackendSignal, String> {
        self.ensure_window(frame)?;
        let window = self
            .window
            .as_mut()
            .ok_or_else(|| "window backend was not initialized".to_string())?;
        if !window.is_open() {
            return Ok(BackendSignal::Close);
        }

        let surface_size = self
            .surface_size
            .ok_or_else(|| "window backend surface size missing".to_string())?;
        window.set_title(&compose_window_title(frame, &self.title_prefix));
        let pixels = scale_frame_pixels(frame, self.tile_pixels);
        window
            .update_with_buffer(&pixels, surface_size.0, surface_size.1)
            .map_err(|err| err.to_string())?;
        if window.is_open() {
            Ok(BackendSignal::Continue)
        } else {
            Ok(BackendSignal::Close)
        }
    }
}

fn frame_interval_for_fps(fps: u32) -> Duration {
    let safe_fps = fps.max(1);
    Duration::from_nanos(1_000_000_000u64 / u64::from(safe_fps))
}

fn sleep_to_target(frame_started: Instant, frame_interval: Duration) {
    let elapsed = frame_started.elapsed();
    if elapsed < frame_interval {
        thread::sleep(frame_interval - elapsed);
    }
}

fn compose_frame(
    scene: &RenderModel,
    hud: &HudModel,
    frame_id: u64,
    max_view_tiles: Option<(usize, usize)>,
) -> WindowFrame {
    let width = ((scene.viewport.width / TILE_SIZE).round().max(0.0) as usize).max(1);
    let height = ((scene.viewport.height / TILE_SIZE).round().max(0.0) as usize).max(1);
    let (window_x, window_y, window_width, window_height) =
        crop_window(scene, width, height, max_view_tiles);
    let mut tiles = vec![COLOR_EMPTY; window_width.saturating_mul(window_height)];

    let mut objects = scene
        .objects
        .iter()
        .filter_map(|object| {
            visible_window_tile(object, window_x, window_y, window_width, window_height)
        })
        .collect::<Vec<_>>();
    objects.sort_by_key(|(object, _, _)| object.layer);
    for (object, local_x, local_y) in objects {
        tiles[local_y * window_width + local_x] = color_for_object(object);
    }

    let mut pixels = Vec::with_capacity(window_width.saturating_mul(window_height));

    for y in (0..window_height).rev() {
        for x in 0..window_width {
            pixels.push(tiles[y * window_width + x]);
        }
    }

    WindowFrame {
        frame_id,
        title: hud.title.clone(),
        wave_text: hud.wave_text.clone(),
        status_text: compose_frame_status_text(scene, hud),
        overlay_summary_text: hud.overlay_summary_text.clone(),
        fps: hud.fps,
        zoom: scene.viewport.zoom,
        width: window_width,
        height: window_height,
        pixels,
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
    let zoom = normalize_zoom(scene.viewport.zoom);
    let window_width = zoomed_view_tile_span(max_width, zoom, width);
    let window_height = zoomed_view_tile_span(max_height, zoom, height);
    if width <= window_width && height <= window_height {
        return (0, 0, width, height);
    };

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

    let window_x = crop_origin(focus.0, width, window_width);
    let window_y = crop_origin(focus.1, height, window_height);
    (window_x, window_y, window_width, window_height)
}

fn crop_origin(focus: usize, bound: usize, window: usize) -> usize {
    let half = window / 2;
    focus.saturating_sub(half).min(bound.saturating_sub(window))
}

fn visible_window_tile(
    object: &RenderObject,
    window_x: usize,
    window_y: usize,
    window_width: usize,
    window_height: usize,
) -> Option<(&RenderObject, usize, usize)> {
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

fn normalize_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    }
}

fn zoomed_view_tile_span(max_tiles: usize, zoom: f32, bound: usize) -> usize {
    let max_tiles = max_tiles.max(1);
    let desired = ((max_tiles as f32) / zoom).floor().max(1.0) as usize;
    desired.min(bound.max(1))
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

fn color_for_object(object: &RenderObject) -> u32 {
    color_for_semantic_kind(object.semantic_kind())
}

fn color_for_semantic_kind(kind: RenderObjectSemanticKind) -> u32 {
    match kind {
        RenderObjectSemanticKind::Player => COLOR_PLAYER,
        RenderObjectSemanticKind::Runtime => COLOR_RUNTIME,
        RenderObjectSemanticKind::Marker => COLOR_MARKER,
        RenderObjectSemanticKind::Plan => COLOR_PLAN,
        RenderObjectSemanticKind::Block => COLOR_BLOCK,
        RenderObjectSemanticKind::Terrain => COLOR_TERRAIN,
        RenderObjectSemanticKind::Unknown => COLOR_UNKNOWN,
    }
}

fn compose_window_title(frame: &WindowFrame, title_prefix: &str) -> String {
    let mut parts = vec![title_prefix.to_string(), frame.title.clone()];
    if let Some(wave_text) = frame.wave_text.as_deref().filter(|text| !text.is_empty()) {
        parts.push(wave_text.to_string());
    }
    if !frame.status_text.is_empty() {
        parts.push(frame.status_text.clone());
    }
    if let Some(summary_text) = frame
        .overlay_summary_text
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        parts.push(summary_text.to_string());
    }
    parts.join(" | ")
}

fn compose_frame_status_text(scene: &RenderModel, hud: &HudModel) -> String {
    let mut parts = Vec::new();
    if !hud.status_text.is_empty() {
        parts.push(hud.status_text.clone());
    }
    if let Some(summary) = hud.summary.as_ref() {
        parts.push(compose_hud_summary_status_text(summary));
    }
    if let Some(build_ui) = hud.build_ui.as_ref() {
        parts.push(compose_build_ui_status_text(build_ui));
    }
    if let Some(runtime_ui) = hud.runtime_ui.as_ref() {
        let runtime_ui_text = compose_runtime_ui_status_text(runtime_ui);
        if !runtime_ui_text.is_empty() {
            parts.push(runtime_ui_text);
        }
    }
    if let Some(overlay_semantics_text) = compose_overlay_semantics_status_text(scene) {
        parts.push(overlay_semantics_text);
    }
    parts.join(" ")
}

fn compose_hud_summary_status_text(summary: &HudSummary) -> String {
    format!(
        "hud:team={} sel={} plans={} mk={} map={}x{} ov{} fg{} vis{} hid{}",
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
    )
}

fn compose_runtime_ui_status_text(runtime_ui: &RuntimeUiObservability) -> String {
    let hud_text = &runtime_ui.hud_text;
    let toast = &runtime_ui.toast;
    let text_input = &runtime_ui.text_input;
    let live = &runtime_ui.live;
    format!(
        "ui:hud={}/{}/{}@{}/{}:toast={}/{}@{}/{}:tin={}@{}:{}/{}/{}#{}:n{}:e{}:live=ent={}:fx={}",
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
        compose_live_entity_status_text(&live.entity),
        compose_live_effect_status_text(&live.effect),
    )
}

fn compose_build_ui_status_text(build_ui: &BuildUiObservability) -> String {
    format!(
        "build:sel={}:r{}:b{}:q{}/i{}/f{}/r{}/o{}:h={}",
        optional_i16_label(build_ui.selected_block_id),
        build_ui.selected_rotation,
        if build_ui.building { 1 } else { 0 },
        build_ui.queued_count,
        build_ui.inflight_count,
        build_ui.finished_count,
        build_ui.removed_count,
        build_ui.orphan_authoritative_count,
        build_queue_head_status_text(build_ui.head.as_ref()),
    )
}

fn compose_live_entity_status_text(
    entity: &crate::RuntimeLiveEntitySummaryObservability,
) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_status_text(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
    )
}

fn compose_live_effect_status_text(
    effect: &crate::RuntimeLiveEffectSummaryObservability,
) -> String {
    format!(
        "{}/{}@{}:u{}:k{}:c{}/{}:p{}@{}",
        effect.effect_count,
        effect.spawn_effect_count,
        optional_i16_label(effect.last_effect_id),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.last_contract_name.as_deref()),
        compact_runtime_ui_text(effect.last_reliable_contract_name.as_deref()),
        live_effect_position_source_status_text(effect.last_position_source),
        world_position_status_text(effect.last_position_hint.as_ref()),
    )
}

fn compose_overlay_semantics_status_text(scene: &RenderModel) -> Option<String> {
    let counts = overlay_semantic_counts(scene);
    let total = counts.iter().map(|(_, count)| count).sum::<usize>();
    if total == 0 {
        return None;
    }

    Some(format!(
        "overlay:players={} markers={} plans={} blocks={} runtime={}",
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Player),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Marker),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Plan),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Block),
        overlay_semantic_count(&counts, RenderObjectSemanticKind::Runtime),
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

fn build_queue_head_status_text(head: Option<&BuildQueueHeadObservability>) -> String {
    let Some(head) = head else {
        return "none".to_string();
    };

    let stage = match head.stage {
        BuildQueueHeadStage::Queued => "queued",
        BuildQueueHeadStage::InFlight => "flight",
        BuildQueueHeadStage::Finished => "finish",
        BuildQueueHeadStage::Removed => "remove",
    };
    let mode = if head.breaking { "break" } else { "place" };
    format!(
        "{stage}@{}:{}:{mode}:b{}:r{}",
        head.x,
        head.y,
        optional_i16_label(head.block_id),
        optional_u8_label(head.rotation),
    )
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

fn optional_i16_label(value: Option<i16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_u8_label(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_u32_label(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn optional_u64_label(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn world_position_status_text(value: Option<&crate::RuntimeWorldPositionObservability>) -> String {
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

fn live_effect_position_source_status_text(
    source: Option<crate::RuntimeLiveEffectPositionSource>,
) -> &'static str {
    match source {
        Some(crate::RuntimeLiveEffectPositionSource::BusinessProjection) => "biz",
        Some(crate::RuntimeLiveEffectPositionSource::EffectPacket) => "pkt",
        Some(crate::RuntimeLiveEffectPositionSource::SpawnEffectPacket) => "spawn",
        None => "none",
    }
}

fn optional_bool_label(value: Option<bool>) -> char {
    match value {
        Some(true) => '1',
        Some(false) => '0',
        None => 'n',
    }
}

fn scale_frame_pixels(frame: &WindowFrame, tile_pixels: usize) -> Vec<u32> {
    let tile_pixels = tile_pixels.max(1);
    let width = frame.width.max(1);
    let height = frame.height.max(1);
    let mut pixels = vec![COLOR_EMPTY; width * height * tile_pixels * tile_pixels];
    let surface_width = width * tile_pixels;

    for y in 0..height {
        for x in 0..width {
            let color = frame.pixel(x, y).unwrap_or(COLOR_EMPTY);
            let start_x = x * tile_pixels;
            let start_y = y * tile_pixels;
            for sub_y in 0..tile_pixels {
                let row = (start_y + sub_y) * surface_width;
                for sub_x in 0..tile_pixels {
                    pixels[row + start_x + sub_x] = color;
                }
            }
        }
    }

    pixels
}

fn encode_ppm(frame: &WindowFrame) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(format!("P6\n{} {}\n255\n", frame.width, frame.height).as_bytes());
    for &pixel in &frame.pixels {
        out.push(((pixel >> 16) & 0xFF) as u8);
        out.push(((pixel >> 8) & 0xFF) as u8);
        out.push((pixel & 0xFF) as u8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        color_for_object, scale_frame_pixels, BackendSignal, WindowBackend, WindowFrame,
        WindowPresenter, COLOR_BLOCK, COLOR_EMPTY, COLOR_MARKER, COLOR_PLAN, COLOR_PLAYER,
        COLOR_RUNTIME, COLOR_TERRAIN, COLOR_UNKNOWN,
    };
    use crate::{
        hud_model::HudSummary, BuildQueueHeadObservability, BuildQueueHeadStage,
        BuildUiObservability, HudModel, RenderModel, RenderObject, RuntimeHudTextObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability, Viewport,
    };

    #[derive(Default)]
    struct RecordingBackend {
        frames: Vec<WindowFrame>,
        close_at: Option<u64>,
    }

    impl WindowBackend for RecordingBackend {
        fn present(&mut self, frame: &WindowFrame) -> Result<BackendSignal, String> {
            self.frames.push(frame.clone());
            if self
                .close_at
                .is_some_and(|close_at| frame.frame_id >= close_at)
            {
                Ok(BackendSignal::Close)
            } else {
                Ok(BackendSignal::Continue)
            }
        }
    }

    #[test]
    fn present_once_renders_layered_tile_surface() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            objects: vec![
                RenderObject {
                    id: "terrain:0".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "block:0:1".to_string(),
                    layer: 10,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "player:1".to_string(),
                    layer: 40,
                    x: 8.0,
                    y: 8.0,
                },
            ],
        };
        let hud = HudModel {
            title: "demo".to_string(),
            wave_text: None,
            status_text: "ok".to_string(),
            overlay_summary_text: None,
            fps: Some(60.0),
            summary: None,
            runtime_ui: None,
            build_ui: None,
        };

        presenter.present_once(&scene, &hud).unwrap();
        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 1), Some(COLOR_BLOCK));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_PLAYER));
        assert_eq!(frame.pixel(0, 0), Some(COLOR_TERRAIN));
    }

    #[test]
    fn run_offline_refreshes_until_backend_requests_close() {
        let backend = RecordingBackend {
            frames: Vec::new(),
            close_at: Some(2),
        };
        let mut presenter = WindowPresenter::new(backend).with_target_fps(1000);

        let stats = presenter
            .run_offline(20, |_| {
                (
                    RenderModel {
                        viewport: Viewport {
                            width: 8.0,
                            height: 8.0,
                            zoom: 1.0,
                        },
                        objects: vec![RenderObject {
                            id: "terrain:0".to_string(),
                            layer: 0,
                            x: 0.0,
                            y: 0.0,
                        }],
                    },
                    HudModel {
                        title: "loop".to_string(),
                        wave_text: None,
                        status_text: "loop".to_string(),
                        overlay_summary_text: None,
                        fps: None,
                        summary: None,
                        runtime_ui: None,
                        build_ui: None,
                    },
                )
            })
            .unwrap();

        assert_eq!(stats.frames_rendered, 3);
        assert!(stats.terminated_by_backend);
        assert_eq!(presenter.frame_id(), 3);
    }

    #[test]
    fn scale_frame_pixels_expands_each_tile_to_screen_surface() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            status_text: String::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 2,
            height: 1,
            pixels: vec![0x112233, 0x445566],
        };

        let pixels = scale_frame_pixels(&frame, 2);

        assert_eq!(
            pixels,
            vec![0x112233, 0x112233, 0x445566, 0x445566, 0x112233, 0x112233, 0x445566, 0x445566,]
        );
    }

    #[test]
    fn present_once_crops_view_around_player_with_stable_orientation() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(4, 4);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            objects: vec![
                RenderObject {
                    id: "terrain:origin".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "block:crop-origin".to_string(),
                    layer: 10,
                    x: 32.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "player:focus".to_string(),
                    layer: 40,
                    x: 56.0,
                    y: 56.0,
                },
            ],
        };
        let hud = HudModel::default();

        presenter.present_once(&scene, &hud).unwrap();
        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();

        assert_eq!((frame.width, frame.height), (4, 4));
        assert_eq!(frame.pixel(3, 0), Some(COLOR_PLAYER));
        assert_eq!(frame.pixel(0, 3), Some(COLOR_BLOCK));
        assert_eq!(frame.pixel(0, 0), Some(COLOR_EMPTY));
    }

    #[test]
    fn color_for_object_uses_semantic_prefix_mapping() {
        assert_eq!(color_for_object(&render_object("player:7")), COLOR_PLAYER);
        assert_eq!(color_for_object(&render_object("unit:7")), COLOR_PLAYER);
        assert_eq!(
            color_for_object(&render_object("marker:runtime-health:1:2")),
            COLOR_RUNTIME
        );
        assert_eq!(
            color_for_object(&render_object("block:runtime-building:1:2:3")),
            COLOR_RUNTIME
        );
        assert_eq!(color_for_object(&render_object("marker:1")), COLOR_MARKER);
        assert_eq!(color_for_object(&render_object("hint:1")), COLOR_MARKER);
        assert_eq!(color_for_object(&render_object("plan:99")), COLOR_PLAN);
        assert_eq!(
            color_for_object(&render_object("build-plan:99")),
            COLOR_PLAN
        );
        assert_eq!(color_for_object(&render_object("block:9:2")), COLOR_BLOCK);
        assert_eq!(
            color_for_object(&render_object("building:9:2")),
            COLOR_BLOCK
        );
        assert_eq!(color_for_object(&render_object("terrain:3")), COLOR_TERRAIN);
        assert_eq!(color_for_object(&render_object("tile:3")), COLOR_TERRAIN);
        assert_eq!(color_for_object(&render_object("unknown")), COLOR_UNKNOWN);
    }

    #[test]
    fn present_once_keeps_crop_stable_around_half_tile_player_motion() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(4, 4);
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
                    id: "player:focus".to_string(),
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
            .find(|object| object.id.starts_with("player:"))
            .unwrap()
            .x = 28.1;
        let hud = HudModel::default();

        presenter.present_once(&base_scene, &hud).unwrap();
        presenter.present_once(&moved_scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let first = backend.frames.first().unwrap();
        let second = backend.frames.get(1).unwrap();
        assert_eq!((first.width, first.height), (4, 4));
        assert_eq!((second.width, second.height), (4, 4));
        assert_eq!(first.pixels, second.pixels);
    }

    #[test]
    fn present_once_applies_zoom_to_view_window_size() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(4, 4);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 2.0,
            },
            objects: vec![RenderObject {
                id: "player:focus".to_string(),
                layer: 40,
                x: 32.0,
                y: 32.0,
            }],
        };
        let hud = HudModel::default();

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
    }

    #[test]
    fn present_once_zoom_out_expands_view_window_up_to_map_bounds() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(4, 4);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 0.5,
            },
            objects: vec![RenderObject {
                id: "player:focus".to_string(),
                layer: 40,
                x: 32.0,
                y: 32.0,
            }],
        };
        let hud = HudModel::default();

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!((frame.width, frame.height), (8, 8));
    }

    #[test]
    fn present_once_appends_structured_hud_slices_to_frame_status_text() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            objects: vec![
                render_object("player:1"),
                render_object("marker:7"),
                render_object("plan:1:2:3"),
                render_object("block:9:4"),
            ],
        };
        let hud = HudModel {
            title: "demo".to_string(),
            wave_text: Some("Wave 7".to_string()),
            status_text: "base".to_string(),
            overlay_summary_text: Some("Plans 1".to_string()),
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
                live: crate::RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability {
                        entity_count: 1,
                        hidden_count: 0,
                        local_entity_id: Some(404),
                        local_unit_kind: Some(2),
                        local_unit_value: Some(999),
                        local_hidden: Some(false),
                        local_last_seen_entity_snapshot_count: Some(3),
                        local_position: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 20.0f32.to_bits(),
                            y_bits: 33.0f32.to_bits(),
                        }),
                    },
                    effect: crate::RuntimeLiveEffectSummaryObservability {
                        effect_count: 11,
                        spawn_effect_count: 73,
                        last_effect_id: Some(8),
                        last_spawn_effect_unit_type_id: Some(19),
                        last_kind: Some("Point2".to_string()),
                        last_contract_name: Some("position_target".to_string()),
                        last_reliable_contract_name: Some("unit_parent".to_string()),
                        last_position_hint: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 24.0f32.to_bits(),
                            y_bits: 32.0f32.to_bits(),
                        }),
                        last_position_source: Some(
                            crate::RuntimeLiveEffectPositionSource::BusinessProjection,
                        ),
                    },
                },
            }),
            build_ui: Some(BuildUiObservability {
                selected_block_id: Some(257),
                selected_rotation: 2,
                building: true,
                queued_count: 1,
                inflight_count: 2,
                finished_count: 3,
                removed_count: 4,
                orphan_authoritative_count: 1,
                head: Some(BuildQueueHeadObservability {
                    x: 100,
                    y: 99,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: BuildQueueHeadStage::InFlight,
                }),
            }),
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!(frame.wave_text.as_deref(), Some("Wave 7"));
        assert_eq!(frame.overlay_summary_text.as_deref(), Some("Plans 1"));
        assert!(frame.status_text.starts_with("base "));
        assert!(frame
            .status_text
            .contains("hud:team=2 sel=payload-rout~ plans=3 mk=4 map=80x60 ov1 fg1 vis120 hid24"));
        assert!(frame
            .status_text
            .contains("build:sel=257:r2:b1:q1/i2/f3/r4/o1:h=flight@100:99:place:b301:r1"));
        assert!(frame
            .status_text
            .contains("ui:hud=9/10/11@hud_text/hud_rel"));
        assert!(frame
            .status_text
            .contains("live=ent=1/0@404:u2/999:p20.0:33.0:h0:s3"));
        assert!(frame
            .status_text
            .contains("fx=11/73@8:u19:kPoint2:cposition_tar~/unit_parent:pbiz@24.0:32.0"));
        assert!(frame
            .status_text
            .contains("overlay:players=1 markers=1 plans=1 blocks=1 runtime=0"));
        assert!(frame.status_text.contains("toast=14/15@toast/warn"));
        assert!(frame
            .status_text
            .contains("tin=53@404:Digits/Only_numbers/12345#16:n1:e1"));
        let window_title = super::compose_window_title(frame, "demo-client");
        assert!(window_title.contains("demo-client | demo | Wave 7 |"));
        assert!(window_title.contains("| Plans 1"));
    }

    fn render_object(id: &str) -> RenderObject {
        RenderObject {
            id: id.to_string(),
            layer: 0,
            x: 0.0,
            y: 0.0,
        }
    }
}
