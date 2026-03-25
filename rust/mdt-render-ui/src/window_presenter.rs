use crate::{
    build_user_flow::build_build_user_flow_panel,
    panel_model::{
        build_build_config_panel, build_build_interaction_panel, build_build_minimap_assist_panel,
        build_hud_status_panel, build_hud_visibility_panel, build_minimap_panel,
        build_runtime_admin_panel, build_runtime_chat_panel, build_runtime_command_mode_panel,
        build_runtime_dialog_panel, build_runtime_dialog_stack_panel, build_runtime_kick_panel,
        build_runtime_live_effect_panel, build_runtime_live_entity_panel,
        build_runtime_loading_panel, build_runtime_menu_panel, build_runtime_notice_state_panel,
        build_runtime_prompt_panel, build_runtime_reconnect_panel, build_runtime_rules_panel,
        build_runtime_session_panel, build_runtime_ui_notice_panel, build_runtime_ui_stack_panel,
        build_runtime_world_label_panel, MinimapPanelModel, PresenterViewWindow,
        RuntimeDialogNoticeKind, RuntimeDialogPromptKind, RuntimeUiNoticePanelModel,
    },
    render_model::{RenderObjectSemanticFamily, RenderObjectSemanticKind},
    BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudModel, RenderModel,
    RenderObject, RuntimeUiObservability, ScenePresenter,
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
    pub panel_lines: Vec<String>,
    pub overlay_lines: Vec<String>,
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
    let window = crop_window(scene, width, height, max_view_tiles);
    let mut tiles = vec![COLOR_EMPTY; window.width.saturating_mul(window.height)];

    let mut objects = scene
        .objects
        .iter()
        .filter_map(|object| {
            visible_window_tile(
                object,
                window.origin_x,
                window.origin_y,
                window.width,
                window.height,
            )
        })
        .collect::<Vec<_>>();
    objects.sort_by_key(|(object, _, _)| object.layer);
    for (object, local_x, local_y) in objects {
        tiles[local_y * window.width + local_x] = color_for_object(object);
    }

    let mut pixels = Vec::with_capacity(window.width.saturating_mul(window.height));

    for y in (0..window.height).rev() {
        for x in 0..window.width {
            pixels.push(tiles[y * window.width + x]);
        }
    }

    WindowFrame {
        frame_id,
        title: hud.title.clone(),
        wave_text: hud.wave_text.clone(),
        status_text: compose_frame_status_text(scene, hud, window),
        panel_lines: compose_frame_panel_lines(scene, hud, window),
        overlay_lines: compose_frame_overlay_lines(scene, hud),
        overlay_summary_text: hud.overlay_summary_text.clone(),
        fps: hud.fps,
        zoom: scene.viewport.zoom,
        width: window.width,
        height: window.height,
        pixels,
    }
}

fn crop_window(
    scene: &RenderModel,
    width: usize,
    height: usize,
    max_view_tiles: Option<(usize, usize)>,
) -> PresenterViewWindow {
    let base_window = projected_window(scene, width, height);
    let Some((max_width, max_height)) = max_view_tiles else {
        return base_window;
    };
    let zoom = normalize_zoom(scene.viewport.zoom);
    let window_width = zoomed_view_tile_span(max_width, zoom, base_window.width);
    let window_height = zoomed_view_tile_span(max_height, zoom, base_window.height);
    if base_window.width <= window_width && base_window.height <= window_height {
        return base_window;
    }

    let focus = scene.player_focus_tile(TILE_SIZE).unwrap_or((
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

fn projected_window(scene: &RenderModel, width: usize, height: usize) -> PresenterViewWindow {
    scene
        .view_window
        .map(|window| PresenterViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width.min(width),
            height: window.height.min(height),
        })
        .unwrap_or(PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width,
            height,
        })
}

fn crop_origin(focus: usize, origin: usize, bound: usize, window: usize) -> usize {
    let half = window / 2;
    focus
        .saturating_sub(half)
        .clamp(origin, origin.saturating_add(bound.saturating_sub(window)))
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

fn color_for_object(object: &RenderObject) -> u32 {
    color_for_semantic_kind(object.semantic_kind())
}

fn color_for_semantic_kind(kind: RenderObjectSemanticKind) -> u32 {
    match kind.family() {
        RenderObjectSemanticFamily::Player => COLOR_PLAYER,
        RenderObjectSemanticFamily::Runtime => COLOR_RUNTIME,
        RenderObjectSemanticFamily::Marker => COLOR_MARKER,
        RenderObjectSemanticFamily::Plan => COLOR_PLAN,
        RenderObjectSemanticFamily::Block => COLOR_BLOCK,
        RenderObjectSemanticFamily::Terrain => COLOR_TERRAIN,
        RenderObjectSemanticFamily::Unknown => COLOR_UNKNOWN,
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

fn compose_frame_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> String {
    let mut parts = Vec::new();
    if !hud.status_text.is_empty() {
        parts.push(hud.status_text.clone());
    }
    if let Some(summary_text) = compose_hud_summary_status_text(hud) {
        parts.push(summary_text);
    }
    if let Some(visibility_text) = compose_hud_visibility_status_text(hud) {
        parts.push(visibility_text);
    }
    if let Some(minimap_window_text) = compose_minimap_window_status_text(scene, hud, window) {
        parts.push(minimap_window_text);
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
    parts.join(" ")
}

fn compose_frame_panel_lines(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(summary_text) = compose_hud_summary_status_text(hud) {
        lines.push(format!("HUD: {summary_text}"));
    }
    if let Some(visibility_text) = compose_hud_visibility_status_text(hud) {
        lines.push(format!("HUD-VIS: {visibility_text}"));
    }
    if let Some(detail_text) = compose_hud_detail_status_text(hud) {
        lines.push(format!("HUD-DETAIL: {detail_text}"));
    }
    if let Some(minimap_window_text) = compose_minimap_window_status_text(scene, hud, window) {
        lines.push(format!("MINIMAP: {minimap_window_text}"));
    }
    if let Some(minimap_visibility_text) =
        compose_minimap_visibility_status_text(scene, hud, window)
    {
        lines.push(format!("MINIMAP-VIS: {minimap_visibility_text}"));
    }
    if let Some(minimap_kind_text) = compose_minimap_kind_status_text(scene, hud) {
        lines.push(format!("MINIMAP-KINDS: {minimap_kind_text}"));
    }
    if let Some(minimap_legend_text) = compose_minimap_legend_status_text(hud) {
        lines.push(format!("MINIMAP-LEGEND: {minimap_legend_text}"));
    }
    for minimap_detail_text in compose_minimap_detail_status_lines(scene, hud) {
        lines.push(format!("MINIMAP-DETAIL: {minimap_detail_text}"));
    }
    if let Some(build_panel_text) = compose_build_config_panel_status_text(hud) {
        lines.push(format!("BUILD-CONFIG: {build_panel_text}"));
    }
    for build_entry_text in compose_build_config_entry_status_lines(hud) {
        lines.push(format!("BUILD-CONFIG-ENTRY: {build_entry_text}"));
    }
    if let Some(build_config_more_text) = compose_build_config_more_status_text(hud) {
        lines.push(format!("BUILD-CONFIG-MORE: {build_config_more_text}"));
    }
    if let Some(build_rollback_text) = compose_build_config_rollback_status_text(hud) {
        lines.push(format!("BUILD-ROLLBACK: {build_rollback_text}"));
    }
    if let Some(build_interaction_text) = compose_build_interaction_status_text(hud) {
        lines.push(format!("BUILD-INTERACTION: {build_interaction_text}"));
    }
    if let Some(build_minimap_aux_text) = compose_build_minimap_aux_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-MINIMAP-AUX: {build_minimap_aux_text}"));
    }
    if let Some(build_flow_text) = compose_build_flow_status_text(scene, hud, window) {
        lines.push(format!("BUILD-FLOW: {build_flow_text}"));
    }
    if let Some(build_route_text) = compose_build_route_status_text(scene, hud, window) {
        lines.push(format!("BUILD-ROUTE: {build_route_text}"));
    }
    if let Some(build_flow_detail_text) = compose_build_flow_detail_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-FLOW-DETAIL: {build_flow_detail_text}"));
    }
    if let Some(runtime_ui_notice_text) = compose_runtime_ui_notice_panel_status_text(hud) {
        lines.push(format!("RUNTIME-NOTICE: {runtime_ui_notice_text}"));
    }
    if let Some(runtime_ui_notice_detail_text) = compose_runtime_ui_notice_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-NOTICE-DETAIL: {runtime_ui_notice_detail_text}"
        ));
    }
    if let Some(runtime_menu_text) = compose_runtime_menu_panel_status_text(hud) {
        lines.push(format!("RUNTIME-MENU: {runtime_menu_text}"));
    }
    if let Some(runtime_menu_detail_text) = compose_runtime_menu_detail_status_text(hud) {
        lines.push(format!("RUNTIME-MENU-DETAIL: {runtime_menu_detail_text}"));
    }
    if let Some(runtime_dialog_text) = compose_runtime_dialog_panel_status_text(hud) {
        lines.push(format!("RUNTIME-DIALOG: {runtime_dialog_text}"));
    }
    if let Some(runtime_dialog_detail_text) = compose_runtime_dialog_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-DIALOG-DETAIL: {runtime_dialog_detail_text}"
        ));
    }
    if let Some(runtime_chat_text) = compose_runtime_chat_panel_status_text(hud) {
        lines.push(format!("RUNTIME-CHAT: {runtime_chat_text}"));
    }
    if let Some(runtime_chat_detail_text) = compose_runtime_chat_detail_status_text(hud) {
        lines.push(format!("RUNTIME-CHAT-DETAIL: {runtime_chat_detail_text}"));
    }
    if let Some(runtime_stack_text) = compose_runtime_stack_panel_status_text(hud) {
        lines.push(format!("RUNTIME-STACK: {runtime_stack_text}"));
    }
    if let Some(runtime_stack_depth_text) = compose_runtime_stack_depth_status_text(hud) {
        lines.push(format!("RUNTIME-STACK-DEPTH: {runtime_stack_depth_text}"));
    }
    if let Some(runtime_stack_detail_text) = compose_runtime_stack_detail_status_text(hud) {
        lines.push(format!("RUNTIME-STACK-DETAIL: {runtime_stack_detail_text}"));
    }
    if let Some(runtime_command_text) = compose_runtime_command_mode_panel_status_text(hud) {
        lines.push(format!("RUNTIME-COMMAND: {runtime_command_text}"));
    }
    if let Some(runtime_admin_text) = compose_runtime_admin_panel_status_text(hud) {
        lines.push(format!("RUNTIME-ADMIN: {runtime_admin_text}"));
    }
    if let Some(runtime_admin_detail_text) = compose_runtime_admin_detail_status_text(hud) {
        lines.push(format!("RUNTIME-ADMIN-DETAIL: {runtime_admin_detail_text}"));
    }
    if let Some(runtime_rules_text) = compose_runtime_rules_panel_status_text(hud) {
        lines.push(format!("RUNTIME-RULES: {runtime_rules_text}"));
    }
    if let Some(runtime_rules_detail_text) = compose_runtime_rules_detail_status_text(hud) {
        lines.push(format!("RUNTIME-RULES-DETAIL: {runtime_rules_detail_text}"));
    }
    if let Some(runtime_world_label_text) = compose_runtime_world_label_panel_status_text(hud) {
        lines.push(format!("RUNTIME-WORLD-LABEL: {runtime_world_label_text}"));
    }
    if let Some(runtime_world_label_detail_text) =
        compose_runtime_world_label_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-WORLD-LABEL-DETAIL: {runtime_world_label_detail_text}"
        ));
    }
    if let Some(runtime_session_text) = compose_runtime_session_status_text(hud) {
        lines.push(format!("RUNTIME-SESSION: {runtime_session_text}"));
    }
    if let Some(runtime_kick_text) = compose_runtime_kick_status_text(hud) {
        lines.push(format!("RUNTIME-KICK: {runtime_kick_text}"));
    }
    if let Some(runtime_loading_text) = compose_runtime_loading_status_text(hud) {
        lines.push(format!("RUNTIME-LOADING: {runtime_loading_text}"));
    }
    if let Some(runtime_reconnect_text) = compose_runtime_reconnect_status_text(hud) {
        lines.push(format!("RUNTIME-RECONNECT: {runtime_reconnect_text}"));
    }
    if let Some(runtime_live_entity_text) = compose_runtime_live_entity_panel_status_text(hud) {
        lines.push(format!("RUNTIME-LIVE-ENTITY: {runtime_live_entity_text}"));
    }
    if let Some(runtime_live_effect_text) = compose_runtime_live_effect_panel_status_text(hud) {
        lines.push(format!("RUNTIME-LIVE-EFFECT: {runtime_live_effect_text}"));
    }
    if let Some(runtime_live_effect_detail_text) =
        compose_runtime_live_effect_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-LIVE-EFFECT-DETAIL: {runtime_live_effect_detail_text}"
        ));
    }
    lines
}

fn compose_frame_overlay_lines(scene: &RenderModel, hud: &HudModel) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(summary_text) = hud
        .overlay_summary_text
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        lines.push(format!("OVERLAY: {summary_text}"));
    }
    if let Some(overlay_semantics_text) = compose_overlay_semantics_status_text(scene) {
        lines.push(format!("OVERLAY-KINDS: {overlay_semantics_text}"));
    }
    lines
}

fn compose_hud_summary_status_text(hud: &HudModel) -> Option<String> {
    let summary = build_hud_status_panel(hud)?;
    Some(format!(
        "hud:team={} sel={} plans={} mk={} map={}x{}",
        summary.team_id,
        compact_runtime_ui_text(Some(summary.selected_block.as_str())),
        summary.plan_count,
        summary.marker_count,
        summary.map_width,
        summary.map_height,
    ))
}

fn compose_hud_visibility_status_text(hud: &HudModel) -> Option<String> {
    let visibility = build_hud_visibility_panel(hud)?;
    Some(format!(
        "hudvis:ov{}:fg{}:k{}p{}:v{}p{}:h{}p{}:u{}p{}",
        if visibility.overlay_visible { 1 } else { 0 },
        if visibility.fog_enabled { 1 } else { 0 },
        visibility.known_tile_count,
        visibility.known_tile_percent,
        visibility.visible_tile_count,
        visibility.visible_known_percent,
        visibility.hidden_tile_count,
        visibility.hidden_known_percent,
        visibility.unknown_tile_count,
        visibility.unknown_tile_percent,
    ))
}

fn compose_hud_detail_status_text(hud: &HudModel) -> Option<String> {
    let summary = build_hud_status_panel(hud)?;
    let visibility = build_hud_visibility_panel(hud)?;
    Some(format!(
        "huddet:p={}#{}:sel={}#{}:t{}:vm{}:hm{}",
        compact_runtime_ui_text(Some(summary.player_name.as_str())),
        summary.player_name_len(),
        compact_runtime_ui_text(Some(summary.selected_block.as_str())),
        summary.selected_block_len(),
        summary.map_tile_count(),
        visibility.visible_map_percent(),
        visibility.hidden_map_percent(),
    ))
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

fn compose_runtime_ui_notice_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_notice_panel(hud)?;
    Some(format!(
        "notice:hud={}/{}/{}@{}/{}:toast={}/{}@{}/{}:tin={}@{}:{}/{}/{}#{}:n{}:e{}",
        panel.hud_set_count,
        panel.hud_set_reliable_count,
        panel.hud_hide_count,
        compact_runtime_ui_text(panel.hud_last_message.as_deref()),
        compact_runtime_ui_text(panel.hud_last_reliable_message.as_deref()),
        panel.toast_info_count,
        panel.toast_warning_count,
        compact_runtime_ui_text(panel.toast_last_info_message.as_deref()),
        compact_runtime_ui_text(panel.toast_last_warning_text.as_deref()),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_ui_notice_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_notice_panel(hud)?;
    if runtime_ui_notice_panel_is_empty(&panel) {
        return None;
    }
    Some(format!(
        "noticed:a1:h{}/{}/{}:l{}/{}:t{}/{}:l{}/{}:tin{}:id{}:t{}:m{}:d{}:n{}:e{}",
        panel.hud_set_count,
        panel.hud_set_reliable_count,
        panel.hud_hide_count,
        runtime_ui_text_len(panel.hud_last_message.as_deref()),
        runtime_ui_text_len(panel.hud_last_reliable_message.as_deref()),
        panel.toast_info_count,
        panel.toast_warning_count,
        runtime_ui_text_len(panel.toast_last_info_message.as_deref()),
        runtime_ui_text_len(panel.toast_last_warning_text.as_deref()),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        runtime_ui_text_len(panel.text_input_last_message.as_deref()),
        runtime_ui_text_len(panel.text_input_last_default_text.as_deref()),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_rules_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_rules_panel(hud)?;
    Some(format!(
        "rules:mut{}:fail{}:wv{}:pvp{}:obj{}:q{}:par{}:fg{}:oor{}:last{}",
        panel.mutation_count,
        panel.parse_fail_count,
        optional_bool_label(panel.waves),
        optional_bool_label(panel.pvp),
        panel.objective_count,
        panel.qualified_objective_count,
        panel.objective_parent_edge_count,
        panel.objective_flag_count,
        panel.complete_out_of_range_count,
        optional_i32_label(panel.last_completed_index),
    ))
}

fn compose_runtime_rules_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_rules_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "rulesd:set{}:obj{}:rule{}:clr{}:done{}",
        panel.set_rules_count,
        panel.set_objectives_count,
        panel.set_rule_count,
        panel.clear_objectives_count,
        panel.complete_objective_count,
    ))
}

fn compose_runtime_menu_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_menu_panel(hud)?;
    Some(format!(
        "menu:m{}:fm{}:h{}:tin{}@{}:{}/{}#{}:n{}:e{}",
        panel.menu_open_count,
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_menu_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_menu_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "menud:a{}:fo{}:tin{}:id{}:t{}:d{}:n{}:e{}",
        if panel.text_input_open_count > 0
            || panel.menu_open_count > 0
            || panel.outstanding_follow_up_count() > 0
        {
            1
        } else {
            0
        },
        panel.outstanding_follow_up_count(),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        panel.default_text_len(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_dialog_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_dialog_panel(hud)?;
    Some(format!(
        "dialog:p={}:a{}:m{}/f{}/h{}:tin{}@{}:{}/{}/{}#{}:n{}:e{}:n={}@{}:c{}",
        runtime_dialog_prompt_status_text(panel.prompt_kind),
        if panel.prompt_active { 1 } else { 0 },
        panel.menu_open_count,
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_message.as_deref()),
        compact_runtime_ui_text(panel.text_input_last_default_text.as_deref()),
        panel.text_input_last_length.unwrap_or_default(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
        runtime_dialog_notice_status_text(panel.notice_kind),
        compact_runtime_ui_text(panel.notice_text.as_deref()),
        panel.notice_count,
    ))
}

fn compose_runtime_dialog_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_dialog_panel(hud)?;
    let prompt = build_runtime_prompt_panel(hud)?;
    let notice = build_runtime_notice_state_panel(hud)?;
    if panel.is_empty() && !notice.is_active() && notice.count == 0 && notice.text.is_none() {
        return None;
    }
    Some(format!(
        "dialogd:p={}:a{}:m{}:fo{}:tin{}:msg{}:def{}:n={}:h{}:r{}:i{}:w{}:l{}",
        runtime_dialog_prompt_status_text(prompt.kind),
        if prompt.is_active() { 1 } else { 0 },
        if prompt.menu_active() { 1 } else { 0 },
        panel.outstanding_follow_up_count(),
        prompt.text_input_open_count,
        panel.prompt_message_len(),
        panel.default_text_len(),
        runtime_dialog_notice_status_text(notice.kind),
        if notice.hud_active { 1 } else { 0 },
        if notice.reliable_hud_active { 1 } else { 0 },
        if notice.toast_info_active { 1 } else { 0 },
        if notice.toast_warning_active { 1 } else { 0 },
        panel.notice_text_len(),
    ))
}

fn compose_runtime_chat_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_chat_panel(hud)?;
    Some(format!(
        "chat:srv{}@{}:msg{}@{}:raw{}:s{}",
        panel.server_message_count,
        compact_runtime_ui_text(panel.last_server_message.as_deref()),
        panel.chat_message_count,
        compact_runtime_ui_text(panel.last_chat_message.as_deref()),
        compact_runtime_ui_text(panel.last_chat_unformatted.as_deref()),
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_chat_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_chat_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "chatd:s{}:c{}:r{}:eq{}:sid{}",
        panel.last_server_message_len(),
        panel.last_chat_message_len(),
        panel.last_chat_unformatted_len(),
        optional_bool_label(panel.formatted_matches_unformatted()),
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_stack_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_ui_stack_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let prompt_layers = panel.prompt_layer_labels().join(">");
    let notice_layers = panel.notice_layer_labels().join(">");
    Some(format!(
        "stack:f={}:p{}@{}:n={}@{}:c{}:g{}:t{}:tin{}:s{}",
        panel.foreground_label(),
        panel.prompt_depth(),
        if prompt_layers.is_empty() {
            "none"
        } else {
            prompt_layers.as_str()
        },
        runtime_dialog_notice_status_text(panel.notice_kind),
        if notice_layers.is_empty() {
            "none"
        } else {
            notice_layers.as_str()
        },
        panel.chat_depth(),
        panel.active_group_count(),
        panel.total_depth(),
        optional_i32_label(panel.text_input_last_id),
        optional_i32_label(panel.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_stack_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_dialog_stack_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "stackd:f={}:g{}:t{}:p={}:m{}:fo{}:i{}:n={}:h{}:r{}:i{}:w{}:c{}:{}/{}:sid{}",
        panel.foreground_label(),
        panel.active_group_count(),
        panel.total_depth(),
        runtime_dialog_prompt_status_text(panel.prompt.kind),
        if panel.prompt.menu_active() { 1 } else { 0 },
        panel.prompt.outstanding_follow_up_count(),
        panel.prompt.text_input_open_count,
        runtime_dialog_notice_status_text(panel.notice.kind),
        if panel.notice.hud_active { 1 } else { 0 },
        if panel.notice.reliable_hud_active {
            1
        } else {
            0
        },
        if panel.notice.toast_info_active { 1 } else { 0 },
        if panel.notice.toast_warning_active {
            1
        } else {
            0
        },
        if !panel.chat.is_empty() { 1 } else { 0 },
        panel.chat.server_message_count,
        panel.chat.chat_message_count,
        optional_i32_label(panel.chat.last_chat_sender_entity_id),
    ))
}

fn compose_runtime_stack_depth_status_text(hud: &HudModel) -> Option<String> {
    let summary = hud.runtime_ui_stack_depth_summary()?;
    if summary.is_empty() {
        return None;
    }
    Some(format!(
        "stackdepth:p{}:n{}:c{}:g{}:t{}",
        summary.prompt_depth,
        summary.notice_depth,
        summary.chat_depth,
        summary.active_group_count,
        summary.total_depth,
    ))
}

fn compose_runtime_command_mode_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_command_mode_panel(hud)?;
    Some(format!(
        "cmd:act{}:sel{}@{}:bld{}@{}:rect{}:grp{}:t{}:c{}:s{}",
        if panel.active { 1 } else { 0 },
        panel.selected_unit_count,
        command_i32_status_text(&panel.selected_unit_sample),
        panel.command_building_count,
        optional_i32_label(panel.first_command_building),
        command_rect_status_text(panel.command_rect),
        command_control_groups_status_text(&panel.control_groups),
        command_target_status_text(panel.last_target),
        optional_u8_label(
            panel
                .last_command_selection
                .and_then(|selection| selection.command_id)
        ),
        command_stance_status_text(panel.last_stance_selection),
    ))
}

fn compose_runtime_admin_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_admin_panel(hud)?;
    Some(format!(
        "admin:t{}@{}:f{}:dbg{}/{}@{}:f{}",
        panel.trace_info_count,
        optional_i32_label(panel.last_trace_info_player_id),
        panel.trace_info_parse_fail_count,
        panel.debug_status_client_count,
        panel.debug_status_client_unreliable_count,
        optional_i32_label(panel.last_debug_status_value),
        panel.parse_fail_count,
    ))
}

fn compose_runtime_admin_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_admin_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "admind:tr{}/{}@{}:dbg{}/{}:udbg{}/{}:last{}",
        panel.trace_info_count,
        panel.trace_info_parse_fail_count,
        optional_i32_label(panel.last_trace_info_player_id),
        panel.debug_status_client_count,
        panel.debug_status_client_parse_fail_count,
        panel.debug_status_client_unreliable_count,
        panel.debug_status_client_unreliable_parse_fail_count,
        optional_i32_label(panel.last_debug_status_value),
    ))
}

fn compose_runtime_world_label_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_world_label_panel(hud)?;
    Some(format!(
        "wlabel:set{}:rel{}:rm{}:tot{}:act{}:inact{}:last{}:f{}:fs{}:z{}:pos{}:txt{}:l{}:n{}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.total_count,
        panel.active_count,
        panel.inactive_count(),
        optional_i32_label(panel.last_entity_id),
        optional_u8_label(panel.last_flags),
        runtime_world_label_scalar_status_text(panel.last_font_size_bits, panel.last_font_size()),
        runtime_world_label_scalar_status_text(panel.last_z_bits, panel.last_z()),
        world_position_status_text(panel.last_position.as_ref()),
        runtime_world_label_status_sample(panel.last_text.as_deref()),
        panel.last_text_line_count(),
        panel.last_text_len(),
    ))
}

fn compose_runtime_world_label_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_world_label_panel(hud)?;
    if panel.label_count == 0
        && panel.reliable_label_count == 0
        && panel.remove_label_count == 0
        && panel.active_count == 0
        && panel.last_entity_id.is_none()
        && panel.last_text.is_none()
        && panel.last_flags.is_none()
        && panel.last_font_size_bits.is_none()
        && panel.last_z_bits.is_none()
        && panel.last_position.is_none()
    {
        return None;
    }

    Some(format!(
        "wlabeld:set{}:rel{}:rm{}:act{}:in{}:last{}:f{}:txt{}x{}:fs{}:z{}:p{}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.active_count,
        panel.inactive_count(),
        optional_i32_label(panel.last_entity_id),
        optional_u8_label(panel.last_flags),
        panel.last_text_len(),
        panel.last_text_line_count(),
        runtime_world_label_scalar_status_text(panel.last_font_size_bits, panel.last_font_size()),
        runtime_world_label_scalar_status_text(panel.last_z_bits, panel.last_z()),
        world_position_status_text(panel.last_position.as_ref()),
    ))
}

fn runtime_world_label_status_sample(value: Option<&str>) -> String {
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

fn compose_runtime_kick_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_kick_panel(hud)?;
    Some(format!(
        "kick:{}",
        compose_runtime_kick_panel_status_text(&panel)
    ))
}

fn compose_runtime_session_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "sess:k={};l={};r={}",
        compose_runtime_kick_panel_status_text(&panel.kick),
        compose_runtime_loading_panel_status_text(&panel.loading),
        compose_runtime_reconnect_panel_status_text(&panel.reconnect),
    ))
}

fn compose_runtime_loading_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    Some(format!(
        "loading:{}",
        compose_runtime_loading_panel_status_text(&panel)
    ))
}

fn compose_runtime_reconnect_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_reconnect_panel(hud)?;
    Some(format!(
        "reconnect:{}",
        compose_runtime_reconnect_panel_status_text(&panel)
    ))
}

fn compose_runtime_live_entity_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(format!(
        "liveent:{}",
        compose_live_entity_panel_status_text(&panel)
    ))
}

fn compose_runtime_live_effect_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_effect_panel(hud)?;
    Some(format!(
        "livefx:{}",
        compose_live_effect_panel_status_text(&panel)
    ))
}

fn compose_runtime_live_effect_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_effect_panel(hud)?;
    Some(format!(
        "livefxd:hint{}:src{}:pos{}:ctr{}:rel{}",
        panel.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_text(panel.last_position_source),
        world_position_status_text(panel.last_position_hint.as_ref()),
        compact_runtime_ui_text(panel.last_contract_name.as_deref()),
        compact_runtime_ui_text(panel.last_reliable_contract_name.as_deref()),
    ))
}

fn compose_build_ui_status_text(build_ui: &BuildUiObservability) -> String {
    let mut text = format!(
        "build:sel={}:r{}:b{}:q{}/i{}/f{}/r{}/o{}:h={}:cfg{}",
        optional_i16_label(build_ui.selected_block_id),
        build_ui.selected_rotation,
        if build_ui.building { 1 } else { 0 },
        build_ui.queued_count,
        build_ui.inflight_count,
        build_ui.finished_count,
        build_ui.removed_count,
        build_ui.orphan_authoritative_count,
        build_queue_head_status_text(build_ui.head.as_ref()),
        build_ui.inspector_entries.len(),
    );
    let inspector_text = compose_build_ui_inspector_status_text(build_ui);
    if !inspector_text.is_empty() {
        text.push_str(":cfg=");
        text.push_str(&inspector_text);
    }
    text
}

fn compose_minimap_window_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "mini:win{},{}-{},{}@s{}x{}:c{}:f{}:i{}:d{},{}:e{}{}{}{}",
        panel.window.origin_x,
        panel.window.origin_y,
        panel.window_last_x,
        panel.window_last_y,
        panel.window.width,
        panel.window.height,
        panel.window_coverage_percent,
        optional_focus_tile_status_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        optional_signed_tile_status_text(panel.focus_offset_x),
        optional_signed_tile_status_text(panel.focus_offset_y),
        bool_flag(panel.window_clamped_left),
        bool_flag(panel.window_clamped_top),
        bool_flag(panel.window_clamped_right),
        bool_flag(panel.window_clamped_bottom),
    ))
}

fn compose_minimap_visibility_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "minivis:ov{}:fg{}:k{}p{}:v{}p{}:vm{}:h{}p{}:hm{}:u{}p{}",
        if panel.overlay_visible { 1 } else { 0 },
        if panel.fog_enabled { 1 } else { 0 },
        panel.known_tile_count,
        panel.known_tile_percent,
        panel.visible_tile_count,
        panel.visible_known_percent,
        panel.visible_map_percent(),
        panel.hidden_tile_count,
        panel.hidden_known_percent,
        panel.hidden_map_percent(),
        panel.unknown_tile_count,
        panel.unknown_tile_percent,
    ))
}

fn compose_minimap_kind_status_text(scene: &RenderModel, hud: &HudModel) -> Option<String> {
    let panel = build_minimap_panel(
        scene,
        hud,
        PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: 0,
            height: 0,
        },
    )?;
    let mut text = format!(
        "minikind:obj{}@pl{}:mk{}:pn{}:bk{}:rt{}:tr{}:uk{}",
        panel.tracked_object_count,
        panel.player_count,
        panel.marker_count,
        panel.plan_count,
        panel.block_count,
        panel.runtime_count,
        panel.terrain_count,
        panel.unknown_count,
    );
    if let Some(detail_text) = semantic_detail_text(&panel.detail_counts) {
        text.push_str(" detail=");
        text.push_str(&detail_text);
    }
    Some(text)
}

fn compose_minimap_legend_status_text(hud: &HudModel) -> Option<String> {
    hud.summary.as_ref()?;
    Some("legend:pl@/mkM/pnP/bk#/rtR/tr./uk?".to_string())
}

fn compose_minimap_detail_status_lines(scene: &RenderModel, hud: &HudModel) -> Vec<String> {
    let Some(panel) = build_minimap_panel(
        scene,
        hud,
        PresenterViewWindow {
            origin_x: 0,
            origin_y: 0,
            width: 0,
            height: 0,
        },
    ) else {
        return Vec::new();
    };

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
    lines.push(compose_minimap_window_distribution_status_text(&panel));
    lines
}

fn compose_minimap_window_distribution_status_text(panel: &MinimapPanelModel) -> String {
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

fn compose_build_config_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, 2)?;
    let entries = panel
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{}#{}",
                compact_runtime_ui_text(Some(entry.family.as_str())),
                entry.tracked_count
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "cfgpanel:sel{}:r{}:m{}:p{}/{}:hist{}/{}:o{}:h={}:align={}:fam{}/{}:more{}:t{}@{}",
        optional_i16_label(panel.selected_block_id),
        panel.selected_rotation,
        if panel.building { 1 } else { 0 },
        panel.queued_count,
        panel.inflight_count,
        panel.finished_count,
        panel.removed_count,
        panel.orphan_authoritative_count,
        build_config_panel_head_status_text(panel.head.as_ref()),
        build_config_alignment_status_text(panel.selected_matches_head),
        panel.entries.len(),
        panel.tracked_family_count,
        panel.truncated_family_count,
        panel.tracked_sample_count,
        if entries.is_empty() {
            "-".to_string()
        } else {
            entries
        },
    ))
}

fn compose_build_config_entry_status_lines(hud: &HudModel) -> Vec<String> {
    let Some(panel) = build_build_config_panel(hud, 2) else {
        return Vec::new();
    };

    panel
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                "cfgentry:{}/{}:{}#{}@{}",
                index + 1,
                panel.tracked_family_count,
                compact_runtime_ui_text(Some(entry.family.as_str())),
                entry.tracked_count,
                compact_build_inspector_text(entry.sample.as_str(), 28),
            )
        })
        .collect()
}

fn compose_build_config_more_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, 2)?;
    (panel.truncated_family_count > 0).then(|| format!("cfgmore:+{}", panel.truncated_family_count))
}

fn compose_build_config_rollback_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, 2)?;
    let strip = &panel.rollback_strip;
    Some(format!(
        "cfgstrip:a{}:rb{}:last={}:src={}:b{}:cl{}:lr{}:pm={}:out={}:block={}",
        strip.applied_authoritative_count,
        strip.rollback_count,
        build_config_tile_status_text(strip.last_build_tile),
        build_config_rollback_source_status_text(strip.last_source),
        if strip.last_business_applied { 1 } else { 0 },
        if strip.last_cleared_pending_local {
            1
        } else {
            0
        },
        if strip.last_was_rollback { 1 } else { 0 },
        build_config_pending_match_status_text(strip.last_pending_local_match),
        build_config_outcome_status_text(strip.last_configured_outcome),
        compact_runtime_ui_text(strip.last_configured_block_name.as_deref()),
    ))
}

fn compose_build_interaction_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_interaction_panel(hud)?;
    Some(format!(
        "cfgflow:m={}:s={}:q={}:p={}:pr={}:cfg={}/{}:top={}:h={}:auth={}:pm={}:src={}:t={}:b={}:o={}",
        build_interaction_mode_status_text(panel.mode),
        build_interaction_selection_status_text(panel.selection_state),
        build_interaction_queue_status_text(panel.queue_state),
        panel.pending_count,
        if panel.place_ready { 1 } else { 0 },
        panel.config_family_count,
        panel.config_sample_count,
        compact_runtime_ui_text(panel.top_config_family.as_deref()),
        build_config_panel_head_status_text(panel.head.as_ref()),
        build_interaction_authority_status_text(panel.authority_state),
        build_config_pending_match_status_text(panel.authority_pending_match),
        build_config_rollback_source_status_text(panel.authority_source),
        build_config_tile_status_text(panel.authority_tile),
        compact_runtime_ui_text(panel.authority_block_name.as_deref()),
        panel.orphan_authoritative_count,
    ))
}

fn compose_build_minimap_aux_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(format!(
        "preb:m={}:s={}:q={}:r{}:cfg={}/{}@{}:auth={}:f={}@{}:v{}:u{}:w{}:obj{}:rt{}",
        build_interaction_mode_status_text(panel.mode),
        build_interaction_selection_status_text(panel.selection_state),
        build_interaction_queue_status_text(panel.queue_state),
        if panel.place_ready { 1 } else { 0 },
        panel.config_family_count,
        panel.config_sample_count,
        compact_runtime_ui_text(panel.top_config_family.as_deref()),
        build_interaction_authority_status_text(panel.authority_state),
        optional_focus_tile_status_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        panel.visible_map_percent,
        panel.unknown_tile_percent,
        panel.window_coverage_percent,
        panel.tracked_object_count,
        panel.runtime_count,
    ))
}

fn compose_build_flow_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(format!(
        "cfgnext:{}:m={}:s={}:q={}:r{}:f={}:v={}:w={}:scope={}:auth={}:rt{}",
        panel.next_action_label(),
        build_interaction_mode_status_text(panel.mode),
        build_interaction_selection_status_text(panel.selection_state),
        build_interaction_queue_status_text(panel.queue_state),
        if panel.place_ready { 1 } else { 0 },
        panel.focus_state_label(),
        panel.map_visibility_label(),
        panel.window_coverage_label(),
        panel.config_scope_label(),
        build_interaction_authority_status_text(panel.authority_state),
        panel.runtime_share_percent(),
    ))
}

fn compose_build_flow_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(format!(
        "cfgflowd:f={}@{}:v{}:u{}:w{}:obj{}:rt{}:cfg{}/{}@{}",
        optional_focus_tile_status_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        panel.visible_map_percent,
        panel.unknown_tile_percent,
        panel.window_coverage_percent,
        panel.tracked_object_count,
        panel.runtime_count,
        panel.config_family_count,
        panel.config_sample_count,
        compact_runtime_ui_text(panel.top_config_family.as_deref()),
    ))
}

fn compose_build_route_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    let blockers = panel.blocker_labels().join(">");
    let route = panel.route.join(">");
    Some(format!(
        "cfgroute:n={}:b{}@{}:r{}@{}",
        panel.next_action,
        panel.blocker_count(),
        if blockers.is_empty() {
            "none"
        } else {
            blockers.as_str()
        },
        panel.route_count(),
        route.as_str(),
    ))
}

fn compose_build_ui_inspector_status_text(build_ui: &BuildUiObservability) -> String {
    build_ui
        .inspector_entries
        .iter()
        .map(|entry| {
            format!(
                "{}#{}@{}",
                compact_runtime_ui_text(Some(entry.family.as_str())),
                entry.tracked_count,
                compact_build_inspector_text(entry.sample.as_str(), 28),
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn build_config_panel_head_status_text(
    head: Option<&crate::panel_model::BuildConfigHeadModel>,
) -> String {
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

fn build_config_tile_status_text(value: Option<(i32, i32)>) -> String {
    match value {
        Some((x, y)) => format!("{x}:{y}"),
        None => "none".to_string(),
    }
}

fn build_config_rollback_source_status_text(
    value: Option<crate::BuildConfigAuthoritySourceObservability>,
) -> &'static str {
    match value {
        Some(crate::BuildConfigAuthoritySourceObservability::TileConfig) => "tilecfg",
        Some(crate::BuildConfigAuthoritySourceObservability::ConstructFinish) => "construct",
        None => "none",
    }
}

fn build_config_pending_match_status_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "match",
        Some(false) => "mismatch",
        None => "none",
    }
}

fn build_interaction_mode_status_text(
    value: crate::panel_model::BuildInteractionMode,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionMode::Idle => "idle",
        crate::panel_model::BuildInteractionMode::Place => "place",
        crate::panel_model::BuildInteractionMode::Break => "break",
    }
}

fn build_interaction_selection_status_text(
    value: crate::panel_model::BuildInteractionSelectionState,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionSelectionState::Unarmed => "unarmed",
        crate::panel_model::BuildInteractionSelectionState::Armed => "armed",
        crate::panel_model::BuildInteractionSelectionState::HeadAligned => "head-aligned",
        crate::panel_model::BuildInteractionSelectionState::HeadDiverged => "head-diverged",
        crate::panel_model::BuildInteractionSelectionState::BreakingHead => "break-head",
    }
}

fn build_interaction_queue_status_text(
    value: crate::panel_model::BuildInteractionQueueState,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionQueueState::Empty => "empty",
        crate::panel_model::BuildInteractionQueueState::Queued => "queued",
        crate::panel_model::BuildInteractionQueueState::InFlight => "inflight",
        crate::panel_model::BuildInteractionQueueState::Mixed => "mixed",
    }
}

fn build_interaction_authority_status_text(
    value: crate::panel_model::BuildInteractionAuthorityState,
) -> &'static str {
    match value {
        crate::panel_model::BuildInteractionAuthorityState::None => "none",
        crate::panel_model::BuildInteractionAuthorityState::Applied => "applied",
        crate::panel_model::BuildInteractionAuthorityState::Cleared => "cleared",
        crate::panel_model::BuildInteractionAuthorityState::Rollback => "rollback",
        crate::panel_model::BuildInteractionAuthorityState::RejectedMissingBuilding => {
            "rej-miss-build"
        }
        crate::panel_model::BuildInteractionAuthorityState::RejectedMissingBlockMetadata => {
            "rej-miss-meta"
        }
        crate::panel_model::BuildInteractionAuthorityState::RejectedUnsupportedBlock => {
            "rej-unsupported-block"
        }
        crate::panel_model::BuildInteractionAuthorityState::RejectedUnsupportedConfigType => {
            "rej-unsupported-cfg"
        }
    }
}

fn build_config_outcome_status_text(
    value: Option<crate::BuildConfigOutcomeObservability>,
) -> &'static str {
    match value {
        Some(crate::BuildConfigOutcomeObservability::Applied) => "applied",
        Some(crate::BuildConfigOutcomeObservability::RejectedMissingBuilding) => "rej-miss-build",
        Some(crate::BuildConfigOutcomeObservability::RejectedMissingBlockMetadata) => {
            "rej-miss-meta"
        }
        Some(crate::BuildConfigOutcomeObservability::RejectedUnsupportedBlock) => {
            "rej-unsupported-block"
        }
        Some(crate::BuildConfigOutcomeObservability::RejectedUnsupportedConfigType) => {
            "rej-unsupported-cfg"
        }
        None => "none",
    }
}

fn optional_focus_tile_status_text(value: Option<(usize, usize)>) -> String {
    match value {
        Some((x, y)) => format!("{x}:{y}"),
        None => "-".to_string(),
    }
}

fn optional_signed_tile_status_text(value: Option<isize>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

fn build_config_alignment_status_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "match",
        Some(false) => "split",
        None => "none",
    }
}

fn compact_build_inspector_text(value: &str, limit: usize) -> String {
    let mut compact = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index == limit {
            compact.push('~');
            break;
        }
        compact.push(match ch {
            ' ' | '\t' | '\r' | '\n' => '_',
            _ => ch,
        });
    }
    if compact.is_empty() {
        "-".to_string()
    } else {
        compact
    }
}

fn compose_live_entity_status_text(
    entity: &crate::RuntimeLiveEntitySummaryObservability,
) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}:tp{}/{}:last{}/{}/{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_status_text(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
        entity.player_count,
        entity.unit_count,
        optional_i32_label(entity.last_entity_id),
        optional_i32_label(entity.last_player_entity_id),
        optional_i32_label(entity.last_unit_entity_id),
    )
}

fn compose_live_entity_panel_status_text(
    entity: &crate::panel_model::RuntimeLiveEntityPanelModel,
) -> String {
    format!(
        "{}/{}@{}:u{}/{}:p{}:h{}:s{}:tp{}/{}:last{}/{}/{}",
        entity.entity_count,
        entity.hidden_count,
        optional_i32_label(entity.local_entity_id),
        optional_u8_label(entity.local_unit_kind),
        optional_u32_label(entity.local_unit_value),
        world_position_status_text(entity.local_position.as_ref()),
        optional_bool_label(entity.local_hidden),
        optional_u64_label(entity.local_last_seen_entity_snapshot_count),
        entity.player_count,
        entity.unit_count,
        optional_i32_label(entity.last_entity_id),
        optional_i32_label(entity.last_player_entity_id),
        optional_i32_label(entity.last_unit_entity_id),
    )
}

fn compose_live_effect_status_text(
    effect: &crate::RuntimeLiveEffectSummaryObservability,
) -> String {
    format!(
        "{}/{}@{}:u{}:k{}:c{}/{}:h{}:p{}@{}",
        effect.effect_count,
        effect.spawn_effect_count,
        optional_i16_label(effect.last_effect_id),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.last_contract_name.as_deref()),
        compact_runtime_ui_text(effect.last_reliable_contract_name.as_deref()),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_text(effect.last_position_source),
        world_position_status_text(effect.last_position_hint.as_ref()),
    )
}

fn compose_live_effect_panel_status_text(
    effect: &crate::panel_model::RuntimeLiveEffectPanelModel,
) -> String {
    format!(
        "{}/{}@{}:u{}:k{}:c{}/{}:h{}:p{}@{}",
        effect.effect_count,
        effect.spawn_effect_count,
        optional_i16_label(effect.last_effect_id),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.last_contract_name.as_deref()),
        compact_runtime_ui_text(effect.last_reliable_contract_name.as_deref()),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_text(effect.last_position_source),
        world_position_status_text(effect.last_position_hint.as_ref()),
    )
}

fn compose_runtime_kick_panel_status_text(
    kick: &crate::panel_model::RuntimeKickPanelModel,
) -> String {
    format!(
        "{}@{}:{}:{}",
        compact_runtime_ui_text(kick.reason_text.as_deref()),
        optional_i32_label(kick.reason_ordinal),
        compact_runtime_ui_text(kick.hint_category.as_deref()),
        compact_runtime_ui_text(kick.hint_text.as_deref()),
    )
}

fn compose_runtime_loading_panel_status_text(
    loading: &crate::panel_model::RuntimeLoadingPanelModel,
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
        optional_u64_label(loading.last_ready_inbound_liveness_anchor_at_ms),
        loading.timeout_count,
        loading.connect_or_loading_timeout_count,
        loading.ready_snapshot_timeout_count,
        runtime_session_timeout_kind_status_text(loading.last_timeout_kind),
        optional_u64_label(loading.last_timeout_idle_ms),
        loading.reset_count,
        loading.reconnect_reset_count,
        loading.world_reload_count,
        loading.kick_reset_count,
        runtime_session_reset_kind_status_text(loading.last_reset_kind),
        runtime_world_reload_panel_status_text(loading.last_world_reload.as_ref()),
    )
}

fn compose_runtime_reconnect_panel_status_text(
    reconnect: &crate::panel_model::RuntimeReconnectPanelModel,
) -> String {
    format!(
        "{}{}:{}@{}/{}:{}:{}@{}:{}",
        runtime_reconnect_phase_status_text(reconnect.phase),
        reconnect.phase_transition_count,
        runtime_reconnect_reason_kind_status_text(reconnect.reason_kind),
        reconnect.redirect_count,
        compact_runtime_ui_text(reconnect.last_redirect_ip.as_deref()),
        optional_i32_label(reconnect.last_redirect_port),
        compact_runtime_ui_text(reconnect.reason_text.as_deref()),
        optional_i32_label(reconnect.reason_ordinal),
        compact_runtime_ui_text(reconnect.hint_text.as_deref()),
    )
}

fn runtime_session_timeout_kind_status_text(
    kind: Option<crate::hud_model::RuntimeSessionTimeoutKind>,
) -> &'static str {
    match kind {
        Some(crate::hud_model::RuntimeSessionTimeoutKind::ConnectOrLoading) => "cload",
        Some(crate::hud_model::RuntimeSessionTimeoutKind::ReadySnapshotStall) => "ready",
        None => "none",
    }
}

fn runtime_session_reset_kind_status_text(
    kind: Option<crate::hud_model::RuntimeSessionResetKind>,
) -> &'static str {
    match kind {
        Some(crate::hud_model::RuntimeSessionResetKind::Reconnect) => "reconnect",
        Some(crate::hud_model::RuntimeSessionResetKind::WorldReload) => "reload",
        Some(crate::hud_model::RuntimeSessionResetKind::Kick) => "kick",
        None => "none",
    }
}

fn runtime_world_reload_panel_status_text(
    world_reload: Option<&crate::panel_model::RuntimeWorldReloadPanelModel>,
) -> String {
    match world_reload {
        Some(world_reload) => format!(
            "@lw{}:cl{}:rd{}:cc{}:p{}:d{}:r{}",
            if world_reload.had_loaded_world { 1 } else { 0 },
            if world_reload.had_client_loaded { 1 } else { 0 },
            if world_reload.was_ready_to_enter_world {
                1
            } else {
                0
            },
            if world_reload.had_connect_confirm_sent {
                1
            } else {
                0
            },
            world_reload.cleared_pending_packets,
            world_reload.cleared_deferred_inbound_packets,
            world_reload.cleared_replayed_loading_events,
        ),
        None => "none".to_string(),
    }
}

fn runtime_reconnect_phase_status_text(
    phase: crate::hud_model::RuntimeReconnectPhaseObservability,
) -> &'static str {
    match phase {
        crate::hud_model::RuntimeReconnectPhaseObservability::Idle => "idle",
        crate::hud_model::RuntimeReconnectPhaseObservability::Scheduled => "sched",
        crate::hud_model::RuntimeReconnectPhaseObservability::Attempting => "attempt",
        crate::hud_model::RuntimeReconnectPhaseObservability::Succeeded => "ok",
        crate::hud_model::RuntimeReconnectPhaseObservability::Aborted => "abort",
    }
}

fn runtime_reconnect_reason_kind_status_text(
    kind: Option<crate::hud_model::RuntimeReconnectReasonKind>,
) -> &'static str {
    match kind {
        Some(crate::hud_model::RuntimeReconnectReasonKind::ConnectRedirect) => "redirect",
        Some(crate::hud_model::RuntimeReconnectReasonKind::Kick) => "kick",
        Some(crate::hud_model::RuntimeReconnectReasonKind::Timeout) => "timeout",
        Some(crate::hud_model::RuntimeReconnectReasonKind::ManualConnect) => "manual",
        None => "none",
    }
}

fn compose_overlay_semantics_status_text(scene: &RenderModel) -> Option<String> {
    let summary = scene.semantic_summary();
    if summary.total_count == 0 {
        return None;
    }

    let mut text = format!(
        "overlay:players={} markers={} plans={} blocks={} runtime={}",
        summary.player_count,
        summary.marker_count,
        summary.plan_count,
        summary.block_count,
        summary.runtime_count,
    );
    if let Some(detail_text) = summary.detail_text() {
        text.push_str(" detail=");
        text.push_str(&detail_text);
    }
    Some(text)
}

fn semantic_detail_text(
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

fn runtime_ui_text_len(value: Option<&str>) -> usize {
    value
        .map(str::chars)
        .map(Iterator::count)
        .unwrap_or_default()
}

fn runtime_ui_notice_panel_is_empty(panel: &RuntimeUiNoticePanelModel) -> bool {
    panel.hud_set_count == 0
        && panel.hud_set_reliable_count == 0
        && panel.hud_hide_count == 0
        && panel.hud_last_message.is_none()
        && panel.hud_last_reliable_message.is_none()
        && panel.toast_info_count == 0
        && panel.toast_warning_count == 0
        && panel.toast_last_info_message.is_none()
        && panel.toast_last_warning_text.is_none()
        && panel.text_input_open_count == 0
        && panel.text_input_last_id.is_none()
        && panel.text_input_last_title.is_none()
        && panel.text_input_last_message.is_none()
        && panel.text_input_last_default_text.is_none()
        && panel.text_input_last_length.is_none()
        && panel.text_input_last_numeric.is_none()
        && panel.text_input_last_allow_empty.is_none()
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

fn runtime_world_label_scalar_status_text(bits: Option<u32>, value: Option<f32>) -> String {
    match (bits, value) {
        (Some(bits), Some(value)) => format!("{bits}@{value:.1}"),
        (Some(bits), None) => bits.to_string(),
        (None, _) => "none".to_string(),
    }
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

fn bool_flag(value: bool) -> char {
    if value {
        '1'
    } else {
        '0'
    }
}

fn runtime_dialog_prompt_status_text(kind: Option<RuntimeDialogPromptKind>) -> &'static str {
    match kind {
        Some(RuntimeDialogPromptKind::Menu) => "menu",
        Some(RuntimeDialogPromptKind::FollowUpMenu) => "follow",
        Some(RuntimeDialogPromptKind::TextInput) => "input",
        None => "none",
    }
}

fn runtime_dialog_notice_status_text(kind: Option<RuntimeDialogNoticeKind>) -> &'static str {
    match kind {
        Some(RuntimeDialogNoticeKind::Hud) => "hud",
        Some(RuntimeDialogNoticeKind::HudReliable) => "hud-rel",
        Some(RuntimeDialogNoticeKind::ToastInfo) => "toast",
        Some(RuntimeDialogNoticeKind::ToastWarning) => "warn",
        None => "none",
    }
}

fn command_i32_status_text(values: &[i32]) -> String {
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

fn command_rect_status_text(value: Option<crate::RuntimeCommandRectObservability>) -> String {
    value
        .map(|rect| format!("{}:{}:{}:{}", rect.x0, rect.y0, rect.x1, rect.y1))
        .unwrap_or_else(|| "none".to_string())
}

fn command_control_groups_status_text(
    groups: &[crate::panel_model::RuntimeCommandControlGroupPanelModel],
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
                optional_i32_label(group.first_unit_id)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn command_target_status_text(value: Option<crate::RuntimeCommandTargetObservability>) -> String {
    let Some(value) = value else {
        return "none".to_string();
    };
    let unit_target = value
        .unit_target
        .map(|unit| format!("{}:{}", unit.kind, unit.value))
        .unwrap_or_else(|| "none".to_string());
    let position_target = value
        .position_target
        .map(|position| format!("0x{:08x}:0x{:08x}", position.x_bits, position.y_bits))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "b{}:u{}:p{}:r{}",
        optional_i32_label(value.build_target),
        unit_target,
        position_target,
        command_rect_status_text(value.rect_target)
    )
}

fn command_stance_status_text(value: Option<crate::RuntimeCommandStanceObservability>) -> String {
    value
        .map(|stance| {
            format!(
                "{}/{}",
                optional_u8_label(stance.stance_id),
                if stance.enabled { 1 } else { 0 }
            )
        })
        .unwrap_or_else(|| "none".to_string())
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
        hud_model::{
            HudSummary, RuntimeReconnectObservability, RuntimeReconnectPhaseObservability,
            RuntimeReconnectReasonKind, RuntimeSessionObservability, RuntimeSessionResetKind,
            RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
        },
        BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudModel,
        RenderModel, RenderObject, RenderViewWindow, RuntimeAdminObservability,
        RuntimeHudTextObservability, RuntimeMenuObservability, RuntimeRulesObservability,
        RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
        RuntimeWorldLabelObservability, Viewport,
    };

    fn runtime_stack_test_scene() -> RenderModel {
        RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: Vec::new(),
        }
    }

    fn runtime_stack_test_hud(runtime_ui: RuntimeUiObservability) -> HudModel {
        HudModel {
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        }
    }

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
            view_window: None,
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
                        view_window: None,
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
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
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
            view_window: None,
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
    fn present_once_crops_view_around_unit_focus_alias() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(4, 4);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
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
                    id: "unit:focus".to_string(),
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
    fn present_once_honors_projected_view_window_without_local_crop() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(RenderViewWindow {
                origin_x: 2,
                origin_y: 3,
                width: 4,
                height: 3,
            }),
            objects: vec![RenderObject {
                id: "player:focus".to_string(),
                layer: 40,
                x: 32.0,
                y: 32.0,
            }],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 80,
                map_height: 60,
                overlay_visible: true,
                fog_enabled: false,
                visible_tile_count: 0,
                hidden_tile_count: 0,
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!((frame.width, frame.height), (4, 3));
        assert!(frame
            .panel_lines
            .iter()
            .any(|line| line.contains("MINIMAP: mini:win2,3-5,5@s4x3")));
    }

    #[test]
    fn color_for_object_uses_semantic_prefix_mapping() {
        assert_eq!(color_for_object(&render_object("player:7")), COLOR_PLAYER);
        assert_eq!(color_for_object(&render_object("unit:7")), COLOR_PLAYER);
        assert_eq!(
            color_for_object(&render_object("marker:line:7")),
            COLOR_MARKER
        );
        assert_eq!(
            color_for_object(&render_object("marker:line:7:line-end")),
            COLOR_MARKER
        );
        assert_eq!(
            color_for_object(&render_object("marker:runtime-health:1:2")),
            COLOR_RUNTIME
        );
        assert_eq!(
            color_for_object(&render_object("marker:runtime-config-rollback:1:2:string")),
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
    fn present_once_surfaces_overlay_detail_semantics() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                render_object("player:1"),
                render_object("marker:line:7"),
                render_object("marker:line:7:line-end"),
                render_object("marker:runtime-config:3:2:string"),
                render_object("block:runtime-building:1:2:3"),
                render_object("plan:runtime-place:0:4:5"),
                render_object("terrain:runtime-deconstruct:9:4"),
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.overlay_lines,
            "OVERLAY-KINDS: overlay:players=1 markers=2 plans=0 blocks=0 runtime=4",
        );
        assert_frame_line_contains(
            &frame.overlay_lines,
            "detail=marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1",
        );
    }

    #[test]
    fn present_once_surfaces_minimap_detail_semantics() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                render_object("player:1"),
                render_object("marker:line:7"),
                render_object("marker:line:7:line-end"),
                render_object("marker:runtime-config:3:2:string"),
                render_object("block:runtime-building:1:2:3"),
                render_object("plan:runtime-place:0:4:5"),
                render_object("terrain:runtime-deconstruct:9:4"),
            ],
        };
        let hud = HudModel {
            summary: Some(crate::hud_model::HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 2,
                map_height: 2,
                overlay_visible: true,
                fog_enabled: false,
                visible_tile_count: 4,
                hidden_tile_count: 0,
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS: minikind:obj7@pl1:mk2:pn0:bk0:rt4:tr0:uk0 detail=marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-DETAIL: minid:1/6:marker-line=1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-DETAIL: miniwin:win7:off0@pl1:mk2:pn0:bk0:rt4:tr0:uk0",
        );
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
            view_window: None,
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
    fn present_once_keeps_crop_stable_around_half_tile_unit_motion() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(4, 4);
        let base_scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
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
            .find(|object| object.id.starts_with("unit:"))
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
            view_window: None,
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
            view_window: None,
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
            view_window: None,
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
                chat: crate::RuntimeChatObservability {
                    server_message_count: 7,
                    last_server_message: Some("server text".to_string()),
                    chat_message_count: 8,
                    last_chat_message: Some("[cyan]hello".to_string()),
                    last_chat_unformatted: Some("hello".to_string()),
                    last_chat_sender_entity_id: Some(404),
                },
                admin: RuntimeAdminObservability {
                    trace_info_count: 56,
                    trace_info_parse_fail_count: 76,
                    last_trace_info_player_id: Some(123456),
                    debug_status_client_count: 57,
                    debug_status_client_parse_fail_count: 77,
                    debug_status_client_unreliable_count: 58,
                    debug_status_client_unreliable_parse_fail_count: 78,
                    last_debug_status_value: Some(12),
                },
                menu: RuntimeMenuObservability {
                    menu_open_count: 16,
                    follow_up_menu_open_count: 17,
                    hide_follow_up_menu_count: 18,
                },
                command_mode: crate::RuntimeCommandModeObservability {
                    active: true,
                    selected_units: vec![11, 22, 33, 44],
                    command_buildings: vec![
                        ((5 & 0xffff) << 16) | (6 & 0xffff),
                        ((-7 & 0xffff) << 16) | (8 & 0xffff),
                    ],
                    command_rect: Some(crate::RuntimeCommandRectObservability {
                        x0: -3,
                        y0: 4,
                        x1: 12,
                        y1: 18,
                    }),
                    control_groups: vec![
                        crate::RuntimeCommandControlGroupObservability {
                            index: 2,
                            unit_ids: vec![11, 22, 33],
                        },
                        crate::RuntimeCommandControlGroupObservability {
                            index: 4,
                            unit_ids: vec![99],
                        },
                    ],
                    last_target: Some(crate::RuntimeCommandTargetObservability {
                        build_target: Some(((9 & 0xffff) << 16) | (10 & 0xffff)),
                        unit_target: Some(crate::RuntimeCommandUnitRefObservability {
                            kind: 2,
                            value: 808,
                        }),
                        position_target: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 48.0f32.to_bits(),
                            y_bits: 96.0f32.to_bits(),
                        }),
                        rect_target: Some(crate::RuntimeCommandRectObservability {
                            x0: 1,
                            y0: 2,
                            x1: 3,
                            y1: 4,
                        }),
                    }),
                    last_command_selection: Some(crate::RuntimeCommandSelectionObservability {
                        command_id: Some(5),
                    }),
                    last_stance_selection: Some(crate::RuntimeCommandStanceObservability {
                        stance_id: Some(7),
                        enabled: false,
                    }),
                },
                rules: RuntimeRulesObservability {
                    set_rules_count: 67,
                    set_rules_parse_fail_count: 68,
                    set_objectives_count: 69,
                    set_objectives_parse_fail_count: 70,
                    set_rule_count: 71,
                    set_rule_parse_fail_count: 72,
                    clear_objectives_count: 73,
                    complete_objective_count: 74,
                    waves: Some(true),
                    pvp: Some(false),
                    objective_count: 2,
                    qualified_objective_count: 1,
                    objective_parent_edge_count: 1,
                    objective_flag_count: 2,
                    complete_out_of_range_count: 75,
                    last_completed_index: Some(9),
                },
                world_labels: RuntimeWorldLabelObservability {
                    label_count: 19,
                    reliable_label_count: 20,
                    remove_label_count: 21,
                    active_count: 2,
                    last_entity_id: Some(904),
                    last_text: Some("world label".to_string()),
                    last_flags: Some(3),
                    last_font_size_bits: Some(12.0f32.to_bits()),
                    last_z_bits: Some(4.0f32.to_bits()),
                    last_position: Some(crate::RuntimeWorldPositionObservability {
                        x_bits: 40.0f32.to_bits(),
                        y_bits: 60.0f32.to_bits(),
                    }),
                },
                session: RuntimeSessionObservability {
                    kick: crate::hud_model::RuntimeKickObservability {
                        reason_text: Some("idInUse".to_string()),
                        reason_ordinal: Some(7),
                        hint_category: Some("IdInUse".to_string()),
                        hint_text: Some("wait for old session".to_string()),
                    },
                    loading: crate::hud_model::RuntimeLoadingObservability {
                        deferred_inbound_packet_count: 5,
                        replayed_inbound_packet_count: 6,
                        dropped_loading_low_priority_packet_count: 7,
                        dropped_loading_deferred_overflow_count: 8,
                        failed_state_snapshot_parse_count: 9,
                        failed_state_snapshot_core_data_parse_count: 10,
                        failed_entity_snapshot_parse_count: 11,
                        ready_inbound_liveness_anchor_count: 12,
                        last_ready_inbound_liveness_anchor_at_ms: Some(1300),
                        timeout_count: 2,
                        connect_or_loading_timeout_count: 1,
                        ready_snapshot_timeout_count: 1,
                        last_timeout_kind: Some(RuntimeSessionTimeoutKind::ReadySnapshotStall),
                        last_timeout_idle_ms: Some(20000),
                        reset_count: 3,
                        reconnect_reset_count: 1,
                        world_reload_count: 1,
                        kick_reset_count: 1,
                        last_reset_kind: Some(RuntimeSessionResetKind::WorldReload),
                        last_world_reload: Some(RuntimeWorldReloadObservability {
                            had_loaded_world: true,
                            had_client_loaded: false,
                            was_ready_to_enter_world: true,
                            had_connect_confirm_sent: false,
                            cleared_pending_packets: 4,
                            cleared_deferred_inbound_packets: 5,
                            cleared_replayed_loading_events: 6,
                        }),
                    },
                    reconnect: RuntimeReconnectObservability {
                        phase: RuntimeReconnectPhaseObservability::Attempting,
                        phase_transition_count: 3,
                        reason_kind: Some(RuntimeReconnectReasonKind::ConnectRedirect),
                        reason_text: Some("connectRedirect".to_string()),
                        reason_ordinal: None,
                        hint_text: Some("server requested redirect".to_string()),
                        redirect_count: 1,
                        last_redirect_ip: Some("127.0.0.1".to_string()),
                        last_redirect_port: Some(6567),
                    },
                },
                live: crate::RuntimeLiveSummaryObservability {
                    entity: crate::RuntimeLiveEntitySummaryObservability {
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
                        last_business_hint: Some("pos:point2:3:4@1/0".to_string()),
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
                rollback_strip: crate::BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 3,
                    rollback_count: 1,
                    last_build_tile: Some((23, 45)),
                    last_business_applied: true,
                    last_cleared_pending_local: true,
                    last_was_rollback: true,
                    last_pending_local_match: Some(false),
                    last_source: Some(
                        crate::BuildConfigAuthoritySourceObservability::ConstructFinish,
                    ),
                    last_configured_outcome: Some(crate::BuildConfigOutcomeObservability::Applied),
                    last_configured_block_name: Some("power-node".to_string()),
                },
                inspector_entries: vec![
                    crate::BuildConfigInspectorEntryObservability {
                        family: "message".to_string(),
                        tracked_count: 1,
                        sample: "18:40:len=5:text=hello".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "power-node".to_string(),
                        tracked_count: 1,
                        sample: "23:45:links=24:46|25:47".to_string(),
                    },
                ],
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
            .contains("hud:team=2 sel=payload-rout~ plans=3 mk=4 map=80x60"));
        assert!(frame
            .status_text
            .contains("hudvis:ov1:fg1:k144p3:v120p83:h24p16:u4656p97"));
        assert!(frame
            .status_text
            .contains("mini:win0,0-0,0@s1x1:c0:f0:0:i1"));
        assert!(frame
            .status_text
            .contains("build:sel=257:r2:b1:q1/i2/f3/r4/o1:h=flight@100:99:place:b301:r1:cfg2"));
        assert!(frame
            .status_text
            .contains("ui:hud=9/10/11@hud_text/hud_rel"));
        assert!(frame
            .status_text
            .contains("live=ent=1/0@404:u2/999:p20.0:33.0:h0:s3"));
        assert!(frame.status_text.contains(
            "fx=11/73@8:u19:kPoint2:cposition_tar~/unit_parent:hpos:point2:3:4@1/0:pbiz@24.0:32.0"
        ));
        assert!(frame
            .status_text
            .contains("tin=53@404:Digits/Only_numbers/12345#16:n1:e1"));
        assert_frame_line_contains(
            &frame.panel_lines,
            "HUD: hud:team=2 sel=payload-rout~ plans=3 mk=4 map=80x60",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "HUD-VIS: hudvis:ov1:fg1:k144p3:v120p83:h24p16:u4656p97",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "HUD-DETAIL: huddet:p=operator#8:sel=payload-rout~#14:t4800:vm2:hm0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-VIS: minivis:ov1:fg1:k144p3:v120p83:vm2:h24p16:hm0:u4656p97",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS: minikind:obj4@pl1:mk1:pn1:bk1:rt0:tr0:uk0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-LEGEND: legend:pl@/mkM/pnP/bk#/rtR/tr./uk?",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG: cfgpanel:sel257:r2:m1:p1/2:hist3/4:o1:h=flight@100:99:place:b301:r1:align=split:fam2/2:more0:t2@message#1,power-node#1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: cfgentry:1/2:message#1@18:40:len=5:text=hello",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: cfgentry:2/2:power-node#1@23:45:links=24:46|25:47",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROLLBACK: cfgstrip:a3:rb1:last=23:45:src=construct:b1:cl1:lr1:pm=mismatch:out=applied:block=power-node",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INTERACTION: cfgflow:m=place:s=head-diverged:q=mixed:p=3:pr=1:cfg=2/2:top=message:h=flight@100:99:place:b301:r1:auth=rollback:pm=mismatch:src=construct:t=23:45:b=power-node:o=1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-NOTICE: notice:hud=9/10/11@hud_text/hud_rel:toast=14/15@toast/warn:tin=53@404:Digits/Only_numbers/12345#16:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-NOTICE-DETAIL: noticed:a1:h9/10/11:l8/7:t14/15:l5/4:tin53:id404:t6:m12:d5:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-MENU: menu:m16:fm17:h18:tin53@404:Digits/12345#16:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-MENU-DETAIL: menud:a1:fo0:tin53:id404:tDigits:d5:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-DIALOG: dialog:p=input:a1:m16/f17/h18:tin53@404:Digits/Only_numbers/12345#16:n1:e1:n=warn@warn:c48",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-DIALOG-DETAIL: dialogd:p=input:a1:m1:fo0:tin53:msg12:def5:n=warn:h1:r1:i1:w1:l4",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-CHAT: chat:srv7@server_text:msg8@[cyan]hello:rawhello:s404",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-CHAT-DETAIL: chatd:s11:c11:r5:eq0:sid404",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK: stack:f=input:p2@input>menu:n=warn@hud>reliable>info>warn:c1:g3:t7:tin404:s404",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK-DEPTH: stackdepth:p2:n4:c1:g3:t7",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK-DETAIL: stackd:f=input:g3:t7:p=input:m1:fo0:i53:n=warn:h1:r1:i1:w1:c1:7/8:sid404",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-COMMAND: cmd:act1:sel4@11,22,33:bld2@327686:rect-3:4:12:18:grp2#3@11,4#1@99:tb589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4:c5:s7/0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-ADMIN: admin:t56@123456:f76:dbg57/58@12:f231",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-ADMIN-DETAIL: admind:tr56/76@123456:dbg57/77:udbg58/78:last12",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RULES: rules:mut354:fail210:wv1:pvp0:obj2:q1:par1:fg2:oor75:last9",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RULES-DETAIL: rulesd:set67:obj69:rule71:clr73:done74",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-WORLD-LABEL: wlabel:set19:rel20:rm21:tot60:act2:inact58:last904:f3:fs1094713344@12.0:z1082130432@4.0:pos40.0:60.0:txtworld_label:l1:n11",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-WORLD-LABEL-DETAIL: wlabeld:set19:rel20:rm21:act2:in58:last904:f3:txt11x1:fs1094713344@12.0:z1082130432@4.0:p40.0:60.0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-SESSION: sess:k=idInUse@7:IdInUse:wait_for_old~;l=defer5:replay6:drop7:qdrop8:sfail9:scfail10:efail11:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6;r=attempt3:redirect@1/127.0.0.1:6567:connectRedir~@none:server_reque~",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-KICK: kick:idInUse@7:IdInUse:wait_for_old~",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LOADING: loading:defer5:replay6:drop7:qdrop8:sfail9:scfail10:efail11:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RECONNECT: reconnect:attempt3:redirect@1/127.0.0.1:6567:connectRedir~@none:server_reque~",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-ENTITY: liveent:1/0@404:u2/999:p20.0:33.0:h0:s3:tp1/0:last404/404/none",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-EFFECT: livefx:11/73@8:u19:kPoint2:cposition_tar~/unit_parent:hpos:point2:3:4@1/0:pbiz@24.0:32.0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-EFFECT-DETAIL: livefxd:hintpos:point2:3:4@1/0:srcbiz:pos24.0:32.0:ctrposition_tar~:relunit_parent",
        );
        assert_frame_line_contains(&frame.overlay_lines, "OVERLAY: Plans 1");
        assert_frame_line_contains(
            &frame.overlay_lines,
            "OVERLAY-KINDS: overlay:players=1 markers=1 plans=1 blocks=1 runtime=0",
        );
        let window_title = super::compose_window_title(frame, "demo-client");
        assert!(window_title.contains("demo-client | demo | Wave 7 |"));
        assert!(window_title.contains("| Plans 1"));
    }

    #[test]
    fn present_once_surfaces_build_config_overflow_and_extended_minimap_counts() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                render_object("player:1"),
                render_object("terrain:2"),
                render_object("unknown"),
            ],
        };
        let hud = HudModel {
            status_text: "base".to_string(),
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 3,
                marker_count: 4,
                map_width: 80,
                map_height: 60,
                overlay_visible: false,
                fog_enabled: false,
                visible_tile_count: 0,
                hidden_tile_count: 0,
            }),
            build_ui: Some(BuildUiObservability {
                selected_block_id: Some(301),
                selected_rotation: 1,
                building: true,
                queued_count: 2,
                inflight_count: 1,
                finished_count: 4,
                removed_count: 5,
                orphan_authoritative_count: 6,
                head: Some(BuildQueueHeadObservability {
                    x: 10,
                    y: 12,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: BuildQueueHeadStage::Queued,
                }),
                rollback_strip: crate::BuildConfigRollbackStripObservability {
                    applied_authoritative_count: 4,
                    rollback_count: 2,
                    last_build_tile: Some((10, 12)),
                    last_business_applied: true,
                    last_cleared_pending_local: false,
                    last_was_rollback: false,
                    last_pending_local_match: Some(true),
                    last_source: Some(crate::BuildConfigAuthoritySourceObservability::TileConfig),
                    last_configured_outcome: Some(
                        crate::BuildConfigOutcomeObservability::RejectedMissingBuilding,
                    ),
                    last_configured_block_name: Some("gamma".to_string()),
                },
                inspector_entries: vec![
                    crate::BuildConfigInspectorEntryObservability {
                        family: "alpha".to_string(),
                        tracked_count: 1,
                        sample: "one".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "gamma".to_string(),
                        tracked_count: 4,
                        sample: "four".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "beta".to_string(),
                        tracked_count: 2,
                        sample: "two".to_string(),
                    },
                ],
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP: mini:win0,0-1,1@s2x2:c0:f0:0:i1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS: minikind:obj3@pl1:mk0:pn0:bk0:rt0:tr1:uk1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG: cfgpanel:sel301:r1:m1:p2/1:hist4/5:o6:h=queued@10:12:place:b301:r1:align=match:fam2/3:more1:t7@gamma#4,beta#2",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: cfgentry:1/3:gamma#4@four",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: cfgentry:2/3:beta#2@two",
        );
        assert_frame_line_contains(&frame.panel_lines, "BUILD-CONFIG-MORE: cfgmore:+1");
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROLLBACK: cfgstrip:a4:rb2:last=10:12:src=tilecfg:b1:cl0:lr0:pm=match:out=rej-miss-build:block=gamma",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INTERACTION: cfgflow:m=place:s=head-aligned:q=mixed:p=3:pr=1:cfg=3/7:top=gamma:h=queued@10:12:place:b301:r1:auth=rej-miss-build:pm=match:src=tilecfg:t=10:12:b=gamma:o=6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-MINIMAP-AUX: preb:m=place:s=head-aligned:q=mixed:r1:cfg=3/7@gamma:auth=rej-miss-build:f=0:0@1:v0:u100:w0:obj3:rt0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-FLOW: cfgnext:resolve:m=place:s=head-aligned:q=mixed:r1:f=inside:v=unseen:w=offscreen:scope=multi:auth=rej-miss-build:rt0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROUTE: cfgroute:n=resolve:b2@resolve>survey:r3@resolve>survey>commit",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-FLOW-DETAIL: cfgflowd:f=0:0@1:v0:u100:w0:obj3:rt0:cfg3/7@gamma",
        );
    }

    #[test]
    fn present_once_omits_runtime_session_line_for_empty_default_state() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: Vec::new(),
        };
        let hud = HudModel {
            runtime_ui: Some(RuntimeUiObservability::default()),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert!(
            frame
                .panel_lines
                .iter()
                .all(|line| !line.starts_with("RUNTIME-SESSION:")),
            "unexpected runtime session line in {:?}",
            frame.panel_lines
        );
        assert!(
            frame
                .panel_lines
                .iter()
                .all(|line| !line.starts_with("RUNTIME-NOTICE-DETAIL:")),
            "unexpected runtime notice detail line in {:?}",
            frame.panel_lines
        );
        assert!(
            frame
                .panel_lines
                .iter()
                .all(|line| !line.starts_with("BUILD-FLOW-DETAIL:")),
            "unexpected build flow detail line in {:?}",
            frame.panel_lines
        );
        assert!(
            frame
                .panel_lines
                .iter()
                .all(|line| !line.starts_with("RUNTIME-WORLD-LABEL-DETAIL:")),
            "unexpected runtime world-label detail line in {:?}",
            frame.panel_lines
        );
    }

    #[test]
    fn present_once_surfaces_runtime_stack_minimal_regression_cases() {
        let mut chat_only = RuntimeUiObservability::default();
        chat_only.chat.server_message_count = 1;
        chat_only.chat.chat_message_count = 2;
        chat_only.chat.last_chat_sender_entity_id = Some(42);

        let mut menu_only = RuntimeUiObservability::default();
        menu_only.menu.menu_open_count = 1;

        let mut follow_up_only = RuntimeUiObservability::default();
        follow_up_only.menu.follow_up_menu_open_count = 1;

        let mut input_notice_chat = RuntimeUiObservability::default();
        input_notice_chat.text_input.open_count = 1;
        input_notice_chat.text_input.last_id = Some(404);
        input_notice_chat.toast.warning_count = 1;
        input_notice_chat.toast.last_warning_text = Some("warn".to_string());
        input_notice_chat.chat.server_message_count = 1;
        input_notice_chat.chat.chat_message_count = 1;
        input_notice_chat.chat.last_chat_sender_entity_id = Some(404);

        let cases = vec![
            (
                "chat-only",
                runtime_stack_test_hud(chat_only),
                "RUNTIME-STACK: stack:f=chat:p0@none:n=none@none:c1:g1:t1:tinnone:s42",
                "RUNTIME-STACK-DEPTH: stackdepth:p0:n0:c1:g1:t1",
                "RUNTIME-STACK-DETAIL: stackd:f=chat:g1:t1:p=none:m0:fo0:i0:n=none:h0:r0:i0:w0:c1:1/2:sid42",
            ),
            (
                "menu-only",
                runtime_stack_test_hud(menu_only),
                "RUNTIME-STACK: stack:f=menu:p1@menu:n=none@none:c0:g1:t1:tinnone:snone",
                "RUNTIME-STACK-DEPTH: stackdepth:p1:n0:c0:g1:t1",
                "RUNTIME-STACK-DETAIL: stackd:f=menu:g1:t1:p=menu:m1:fo0:i0:n=none:h0:r0:i0:w0:c0:0/0:sidnone",
            ),
            (
                "follow-up-without-text-input",
                runtime_stack_test_hud(follow_up_only),
                "RUNTIME-STACK: stack:f=follow-up:p1@follow-up:n=none@none:c0:g1:t1:tinnone:snone",
                "RUNTIME-STACK-DEPTH: stackdepth:p1:n0:c0:g1:t1",
                "RUNTIME-STACK-DETAIL: stackd:f=follow-up:g1:t1:p=follow:m0:fo1:i0:n=none:h0:r0:i0:w0:c0:0/0:sidnone",
            ),
            (
                "text-input+notice+chat",
                runtime_stack_test_hud(input_notice_chat),
                "RUNTIME-STACK: stack:f=input:p1@input:n=warn@warn:c1:g3:t3:tin404:s404",
                "RUNTIME-STACK-DEPTH: stackdepth:p1:n1:c1:g3:t3",
                "RUNTIME-STACK-DETAIL: stackd:f=input:g3:t3:p=input:m0:fo0:i1:n=warn:h0:r0:i0:w1:c1:1/1:sid404",
            ),
        ];

        for (name, hud, stack_line, depth_line, detail_line) in cases {
            let backend = RecordingBackend::default();
            let mut presenter = WindowPresenter::new(backend);
            presenter
                .present_once(&runtime_stack_test_scene(), &hud)
                .unwrap();

            let backend = presenter.into_backend();
            let frame = backend.frames.last().unwrap();
            assert_frame_line_contains(&frame.panel_lines, stack_line);
            assert_frame_line_contains(&frame.panel_lines, depth_line);
            assert_frame_line_contains(&frame.panel_lines, detail_line);
            assert!(
                frame
                    .panel_lines
                    .iter()
                    .any(|line| line.contains(stack_line) && line.contains("RUNTIME-STACK:")),
                "missing runtime stack line for {name} in {:?}",
                frame.panel_lines
            );
        }
    }

    fn assert_frame_line_contains(lines: &[String], needle: &str) {
        assert!(
            lines.iter().any(|line| line.contains(needle)),
            "missing line containing `{needle}` in {:?}",
            lines
        );
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
