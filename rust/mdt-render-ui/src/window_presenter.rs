use crate::{
    build_user_flow::build_build_user_flow_panel,
    minimap_user_flow::build_minimap_user_flow_panel,
    panel_model::{
        build_build_config_entry_breakdown, build_build_config_panel,
        build_build_interaction_panel, build_build_minimap_assist_panel, build_hud_status_panel,
        build_hud_visibility_panel, build_minimap_panel, build_runtime_admin_panel,
        build_runtime_bootstrap_panel, build_runtime_chat_panel, build_runtime_choice_panel,
        build_runtime_command_mode_panel, build_runtime_core_binding_panel,
        build_runtime_dialog_panel, build_runtime_dialog_stack_panel, build_runtime_kick_panel,
        build_runtime_live_effect_panel, build_runtime_live_entity_panel,
        build_runtime_loading_panel, build_runtime_marker_panel, build_runtime_menu_panel,
        build_runtime_notice_state_panel, build_runtime_prompt_panel,
        build_runtime_reconnect_panel, build_runtime_rules_panel, build_runtime_session_panel,
        build_runtime_ui_notice_panel, build_runtime_ui_stack_panel,
        build_runtime_world_label_panel, MinimapPanelModel, PresenterViewWindow,
        RuntimeDialogNoticeKind, RuntimeDialogPromptKind, RuntimeUiNoticePanelModel,
    },
    presenter_view::{
        crop_window_to_focus, normalize_zoom, projected_window, visible_window_tile,
        zoomed_view_tile_span,
    },
    render_model::{
        RenderIconPrimitiveFamily, RenderObjectSemanticFamily, RenderObjectSemanticKind,
        RenderPrimitive, RenderPrimitivePayload, RenderPrimitivePayloadValue,
    },
    BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudModel, RenderModel,
    RenderObject, RuntimeUiObservability, ScenePresenter,
};
use minifb::{Scale, Window, WindowOptions};
use std::collections::{BTreeMap, BTreeSet};
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
const COLOR_ICON_RUNTIME_EFFECT: u32 = 0xFFE082;
const COLOR_ICON_RUNTIME_EFFECT_MARKER: u32 = 0xFFB74D;
const COLOR_ICON_BUILD_CONFIG: u32 = 0x4FC3F7;
const COLOR_ICON_RUNTIME_HEALTH: u32 = 0xEF5350;
const COLOR_ICON_RUNTIME_COMMAND: u32 = 0x4488FF;
const COLOR_ICON_RUNTIME_UNIT_ASSEMBLER: u32 = 0x81C784;
const COLOR_ICON_RUNTIME_BREAK: u32 = 0xFF8A65;
const COLOR_ICON_RUNTIME_BULLET: u32 = 0xFFD54F;
const COLOR_ICON_RUNTIME_LOGIC_EXPLOSION: u32 = 0xBA68C8;
const COLOR_ICON_RUNTIME_SOUND_AT: u32 = 0x26C6DA;
const COLOR_ICON_RUNTIME_TILE_ACTION: u32 = 0x9CCC65;
const COLOR_WINDOW_HUD_BAR: u32 = 0x091018;
const COLOR_WINDOW_HUD_TEXT: u32 = 0xE8EEF2;
const COLOR_MINIMAP_INSET_BORDER: u32 = 0x90A4AE;
const COLOR_MINIMAP_INSET_BACKGROUND: u32 = COLOR_TERRAIN;
const COLOR_MINIMAP_INSET_BACKGROUND_PARTIAL: u32 = 0x2F3D45;
const COLOR_MINIMAP_INSET_BACKGROUND_WARN: u32 = 0x3A2E2A;
const COLOR_MINIMAP_INSET_VIEWPORT: u32 = 0xECEFF1;
const COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL: u32 = 0xFFF59D;
const COLOR_MINIMAP_INSET_VIEWPORT_WARN: u32 = 0xFFAB91;
const WINDOW_TARGET_FPS: usize = 60;
const WINDOW_HUD_FONT_WIDTH: usize = 3;
const WINDOW_HUD_FONT_HEIGHT: usize = 5;
const WINDOW_HUD_FONT_SPACING: usize = 1;
const WINDOW_HUD_BAR_PADDING_X: usize = 2;
const WINDOW_HUD_BAR_PADDING_Y: usize = 2;
const WINDOW_MINIMAP_INSET_PADDING: usize = 4;
const WINDOW_MINIMAP_INSET_BORDER_WIDTH: usize = 1;
const WINDOW_BUILD_CONFIG_ENTRY_CAP: usize = 3;
const WINDOW_BUILD_CONFIG_ENTRY_SAMPLE_LIMIT: usize = 56;
const WINDOW_BUILD_INSPECTOR_SAMPLE_LIMIT: usize = 72;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMinimapInset {
    pub map_width: usize,
    pub map_height: usize,
    pub window: PresenterViewWindow,
    pub window_coverage_percent: usize,
    pub map_object_density_percent: usize,
    pub window_object_density_percent: usize,
    pub outside_object_percent: usize,
    pub focus_tile: Option<(usize, usize)>,
    pub focus_in_window: Option<bool>,
    pub player_tile: Option<(usize, usize)>,
    pub ping_tile: Option<(usize, usize)>,
    pub unit_assembler_tiles: Vec<(usize, usize)>,
    pub tile_action_tiles: Vec<(usize, usize)>,
    pub command_tiles: Vec<(usize, usize)>,
    pub command_rects: Vec<WindowMinimapCommandRect>,
    pub runtime_break_rects: Vec<WindowMinimapBreakRect>,
    pub unit_assembler_rects: Vec<WindowMinimapUnitAssemblerRect>,
    pub world_label_tiles: Vec<(usize, usize)>,
    pub runtime_overlay_tiles: Vec<WindowMinimapRuntimeOverlayTile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMinimapRuntimeOverlayTile {
    pub tile: (usize, usize),
    pub kind: WindowMinimapRuntimeOverlayKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMinimapRuntimeOverlayKind {
    Config,
    ConfigAlert,
    Break,
    Place,
    Building,
    Health,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMinimapCommandRect {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
    pub kind: WindowMinimapCommandRectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMinimapCommandRectKind {
    Selection,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMinimapBreakRect {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMinimapUnitAssemblerRect {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowFrame {
    pub frame_id: u64,
    pub title: String,
    pub wave_text: Option<String>,
    pub session_banner_text: Option<String>,
    pub status_text: String,
    pub build_strip_text: Option<String>,
    pub build_strip_detail_text: Option<String>,
    pub panel_lines: Vec<String>,
    pub overlay_lines: Vec<String>,
    pub overlay_summary_text: Option<String>,
    pub fps: Option<f32>,
    pub zoom: f32,
    pub width: usize,
    pub height: usize,
    pub minimap_inset: Option<WindowMinimapInset>,
    pub pixels: Vec<u32>,
}

impl WindowFrame {
    pub fn pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        self.pixels.get(index).copied()
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
        let signal = match self.backend.present(&frame) {
            Ok(signal) => signal,
            Err(err) => {
                self.last_error = Some(err.clone());
                return Err(err);
            }
        };
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
        let Some((surface_width, surface_height, _)) =
            scaled_surface_metrics(frame, self.tile_pixels)
        else {
            return Err("window surface size overflowed".to_string());
        };
        let surface_size = (surface_width, surface_height);
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
    let width = viewport_tile_span(scene.viewport.width);
    let height = viewport_tile_span(scene.viewport.height);
    let window = crop_window(scene, width, height, max_view_tiles);
    let mut tiles = vec![COLOR_EMPTY; window.width.saturating_mul(window.height)];
    let line_end_objects = scene
        .objects
        .iter()
        .filter_map(window_line_end_object_pair)
        .collect::<BTreeMap<_, _>>();
    let primitives = scene.primitives();
    let icon_primitive_ids = primitives
        .iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Icon { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let rect_line_ids = primitives
        .iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Rect { line_ids, .. } => Some(line_ids.iter().map(String::as_str)),
            _ => None,
        })
        .flatten()
        .collect::<BTreeSet<_>>();

    let mut commands = scene
        .objects
        .iter()
        .filter(|object| {
            !icon_primitive_ids.contains(object.id.as_str())
                && !rect_line_ids.contains(object.id.as_str())
        })
        .filter_map(|object| window_render_command(object, &line_end_objects, window))
        .collect::<Vec<_>>();
    commands.extend(
        primitives
            .iter()
            .filter_map(|primitive| window_primitive_render_command(primitive, window)),
    );
    commands.sort_by_key(WindowRenderCommand::layer);
    for command in commands {
        match command {
            WindowRenderCommand::Point {
                object,
                local_x,
                local_y,
            } => {
                tiles[local_y * window.width + local_x] = color_for_object(object);
            }
            WindowRenderCommand::Line {
                start_tile,
                end_tile,
                color,
                ..
            } => draw_window_line_segment(&mut tiles, window, start_tile, end_tile, color),
            WindowRenderCommand::Rect {
                left_tile,
                top_tile,
                right_tile,
                bottom_tile,
                color,
                ..
            } => draw_window_rect_outline(
                &mut tiles,
                window,
                left_tile,
                top_tile,
                right_tile,
                bottom_tile,
                color,
            ),
            WindowRenderCommand::Icon {
                local_x,
                local_y,
                color,
                ..
            } => {
                tiles[local_y * window.width + local_x] = color;
            }
        }
    }

    let mut pixels = Vec::with_capacity(window.width.saturating_mul(window.height));

    for y in (0..window.height).rev() {
        for x in 0..window.width {
            pixels.push(tiles[y * window.width + x]);
        }
    }

    let minimap_inset = compose_window_minimap_inset(scene, hud, window);

    WindowFrame {
        frame_id,
        title: hud.title.clone(),
        wave_text: hud.wave_text.clone(),
        session_banner_text: compose_frame_session_banner_text(hud),
        status_text: compose_frame_status_text(scene, hud, window),
        build_strip_text: compose_frame_build_strip_text(hud),
        build_strip_detail_text: compose_frame_build_strip_detail_text(hud),
        panel_lines: compose_frame_panel_lines(scene, hud, window),
        overlay_lines: compose_frame_overlay_lines(scene, hud),
        overlay_summary_text: hud.overlay_summary_text.clone(),
        fps: hud.fps,
        zoom: scene.viewport.zoom,
        width: window.width,
        height: window.height,
        minimap_inset,
        pixels,
    }
}

fn compose_window_minimap_inset(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<WindowMinimapInset> {
    let panel = build_minimap_panel(scene, hud, window)?;
    if panel.map_width == 0 || panel.map_height == 0 {
        return None;
    }

    Some(WindowMinimapInset {
        map_width: panel.map_width,
        map_height: panel.map_height,
        window: panel.window,
        window_coverage_percent: panel.window_coverage_percent,
        map_object_density_percent: panel.map_object_density_percent(),
        window_object_density_percent: panel.window_object_density_percent(),
        outside_object_percent: panel.outside_object_percent(),
        focus_tile: clamp_window_minimap_tile(panel.focus_tile, panel.map_width, panel.map_height),
        focus_in_window: panel.focus_in_window,
        player_tile: clamp_window_minimap_tile(
            scene.player_focus_tile(TILE_SIZE),
            panel.map_width,
            panel.map_height,
        ),
        ping_tile: runtime_ping_minimap_tile(scene, panel.map_width, panel.map_height),
        unit_assembler_tiles: runtime_unit_assembler_minimap_tiles(
            scene,
            panel.map_width,
            panel.map_height,
            8,
        ),
        tile_action_tiles: runtime_tile_action_minimap_tiles(
            scene,
            panel.map_width,
            panel.map_height,
            8,
        ),
        command_tiles: runtime_command_minimap_tiles(scene, panel.map_width, panel.map_height, 8),
        command_rects: runtime_command_minimap_rects(scene, panel.map_width, panel.map_height, 2),
        runtime_break_rects: runtime_break_minimap_rects(
            scene,
            panel.map_width,
            panel.map_height,
            2,
        ),
        unit_assembler_rects: runtime_unit_assembler_minimap_rects(
            scene,
            panel.map_width,
            panel.map_height,
            4,
        ),
        world_label_tiles: runtime_world_label_minimap_tiles(
            scene,
            panel.map_width,
            panel.map_height,
            8,
        ),
        runtime_overlay_tiles: runtime_minimap_overlay_tiles(
            scene,
            panel.map_width,
            panel.map_height,
            8,
        ),
    })
}

fn clamp_window_minimap_tile(
    tile: Option<(usize, usize)>,
    map_width: usize,
    map_height: usize,
) -> Option<(usize, usize)> {
    let max_x = map_width.checked_sub(1)?;
    let max_y = map_height.checked_sub(1)?;
    tile.map(|(tile_x, tile_y)| (tile_x.min(max_x), tile_y.min(max_y)))
}

fn runtime_ping_minimap_tile(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
) -> Option<(usize, usize)> {
    scene
        .objects
        .iter()
        .filter(|object| object.id.starts_with("marker:text:runtime-ping:"))
        .filter_map(|object| {
            let tile = runtime_minimap_object_tile(object, map_width, map_height)?;
            Some((
                runtime_ping_minimap_sequence(&object.id).unwrap_or(0),
                object.id.as_str(),
                tile,
            ))
        })
        .max_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.cmp(right.1))
                .then(left.2.cmp(&right.2))
        })
        .map(|(_, _, tile)| tile)
}

fn runtime_world_label_minimap_tiles(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_tiles: usize,
) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();

    for object in &scene.objects {
        if !object.id.starts_with("world-label:") {
            continue;
        }
        let Some(tile) = runtime_minimap_object_tile(object, map_width, map_height) else {
            continue;
        };
        candidates.push(StableMinimapTileCandidate {
            priority: 0,
            tile,
            id: object.id.clone(),
        });
    }

    collect_stable_minimap_tiles(candidates, max_tiles)
}

fn runtime_command_minimap_tiles(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_tiles: usize,
) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();

    for (priority, prefix) in [
        (0, "marker:runtime-command-build-target:"),
        (1, "marker:runtime-command-position-target:"),
        (2, "marker:runtime-command-unit-target:"),
        (3, "marker:runtime-command-selected-unit:"),
        (4, "marker:runtime-command-building:"),
    ] {
        for object in &scene.objects {
            if !object.id.starts_with(prefix) {
                continue;
            }
            let Some(tile) = runtime_minimap_object_tile(object, map_width, map_height) else {
                continue;
            };
            candidates.push(StableMinimapTileCandidate {
                priority,
                tile,
                id: object.id.clone(),
            });
        }
    }

    collect_stable_minimap_tiles(candidates, max_tiles)
}

fn runtime_unit_assembler_minimap_tiles(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_tiles: usize,
) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();

    for (priority, prefix) in [
        (0, "marker:runtime-unit-assembler-progress:"),
        (1, "marker:runtime-unit-assembler-command:"),
    ] {
        for object in &scene.objects {
            if !object.id.starts_with(prefix) {
                continue;
            }
            let Some(tile) = runtime_minimap_object_tile(object, map_width, map_height) else {
                continue;
            };
            candidates.push(StableMinimapTileCandidate {
                priority,
                tile,
                id: object.id.clone(),
            });
        }
    }

    collect_stable_minimap_tiles(candidates, max_tiles)
}

fn runtime_tile_action_minimap_tiles(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_tiles: usize,
) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();

    for (priority, prefix) in [
        (0, "marker:runtime-unit-block-spawn:"),
        (1, "marker:runtime-unit-tether-block-spawned:"),
        (2, "marker:runtime-landing-pad-landed:"),
        (3, "marker:runtime-assembler-drone-spawned:"),
        (4, "marker:runtime-assembler-unit-spawned:"),
    ] {
        for object in &scene.objects {
            if !object.id.starts_with(prefix) {
                continue;
            }
            let tile_x = crate::presenter_view::world_to_tile_index_floor(object.x, TILE_SIZE);
            let tile_y = crate::presenter_view::world_to_tile_index_floor(object.y, TILE_SIZE);
            if tile_x < 0 || tile_y < 0 {
                continue;
            }
            let Some(tile) = clamp_window_minimap_tile(
                Some((tile_x as usize, tile_y as usize)),
                map_width,
                map_height,
            ) else {
                continue;
            };
            candidates.push(StableMinimapTileCandidate {
                priority,
                tile,
                id: object.id.clone(),
            });
        }
    }

    collect_stable_minimap_tiles(candidates, max_tiles)
}

fn runtime_command_minimap_rects(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_rects: usize,
) -> Vec<WindowMinimapCommandRect> {
    let mut rects = Vec::new();

    for primitive in scene.primitives() {
        let RenderPrimitive::Rect {
            family,
            left,
            top,
            right,
            bottom,
            ..
        } = primitive
        else {
            continue;
        };
        let kind = match family.as_str() {
            "runtime-command-rect" => WindowMinimapCommandRectKind::Selection,
            "runtime-command-target-rect" => WindowMinimapCommandRectKind::Target,
            _ => continue,
        };
        if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
            continue;
        }
        let origin_x = runtime_world_to_minimap_tile(left, map_width);
        let origin_y = runtime_world_to_minimap_tile(top, map_height);
        let width =
            runtime_world_span_to_tile_span(right - left, map_width.saturating_sub(origin_x));
        let height =
            runtime_world_span_to_tile_span(bottom - top, map_height.saturating_sub(origin_y));
        rects.push(WindowMinimapCommandRect {
            origin_x,
            origin_y,
            width,
            height,
            kind,
        });
    }

    rects.sort_unstable_by(|left, right| {
        runtime_command_rect_kind_priority(left.kind)
            .cmp(&runtime_command_rect_kind_priority(right.kind))
            .then(left.origin_y.cmp(&right.origin_y))
            .then(left.origin_x.cmp(&right.origin_x))
            .then(left.width.cmp(&right.width))
            .then(left.height.cmp(&right.height))
    });
    rects.truncate(max_rects);
    rects
}

fn runtime_break_minimap_rects(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_rects: usize,
) -> Vec<WindowMinimapBreakRect> {
    let mut rects = Vec::new();

    for primitive in scene.primitives() {
        let RenderPrimitive::Rect {
            family,
            left,
            top,
            right,
            bottom,
            ..
        } = primitive
        else {
            continue;
        };
        if family != "runtime-break-rect" {
            continue;
        }
        if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
            continue;
        }
        let origin_x = runtime_world_to_minimap_tile(left, map_width);
        let origin_y = runtime_world_to_minimap_tile(top, map_height);
        let width =
            runtime_world_span_to_tile_span(right - left, map_width.saturating_sub(origin_x));
        let height =
            runtime_world_span_to_tile_span(bottom - top, map_height.saturating_sub(origin_y));
        rects.push(WindowMinimapBreakRect {
            origin_x,
            origin_y,
            width,
            height,
        });
    }

    rects.sort_unstable_by(|left, right| {
        left.origin_y
            .cmp(&right.origin_y)
            .then(left.origin_x.cmp(&right.origin_x))
            .then(left.width.cmp(&right.width))
            .then(left.height.cmp(&right.height))
    });
    rects.truncate(max_rects);
    rects
}

fn runtime_unit_assembler_minimap_rects(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_rects: usize,
) -> Vec<WindowMinimapUnitAssemblerRect> {
    let mut rects = Vec::new();

    for primitive in scene.primitives() {
        let RenderPrimitive::Rect {
            family,
            left,
            top,
            right,
            bottom,
            ..
        } = primitive
        else {
            continue;
        };
        if family != "runtime-unit-assembler-area" {
            continue;
        }
        if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
            continue;
        }
        let origin_x = runtime_world_to_minimap_tile(left, map_width);
        let origin_y = runtime_world_to_minimap_tile(top, map_height);
        let width =
            runtime_world_span_to_tile_span(right - left, map_width.saturating_sub(origin_x));
        let height =
            runtime_world_span_to_tile_span(bottom - top, map_height.saturating_sub(origin_y));
        rects.push(WindowMinimapUnitAssemblerRect {
            origin_x,
            origin_y,
            width,
            height,
        });
    }

    rects.sort_unstable_by(|left, right| {
        left.origin_y
            .cmp(&right.origin_y)
            .then(left.origin_x.cmp(&right.origin_x))
            .then(left.width.cmp(&right.width))
            .then(left.height.cmp(&right.height))
    });
    rects.truncate(max_rects);
    rects
}

fn runtime_minimap_overlay_tiles(
    scene: &RenderModel,
    map_width: usize,
    map_height: usize,
    max_tiles: usize,
) -> Vec<WindowMinimapRuntimeOverlayTile> {
    let mut candidates = Vec::new();

    for (priority, kind) in [
        (0, WindowMinimapRuntimeOverlayKind::ConfigAlert),
        (1, WindowMinimapRuntimeOverlayKind::Config),
        (2, WindowMinimapRuntimeOverlayKind::Break),
        (3, WindowMinimapRuntimeOverlayKind::Place),
        (4, WindowMinimapRuntimeOverlayKind::Building),
        (5, WindowMinimapRuntimeOverlayKind::Health),
    ] {
        for object in &scene.objects {
            if runtime_minimap_overlay_kind(object.semantic_kind()) != Some(kind) {
                continue;
            }
            let Some(tile) = runtime_minimap_object_tile(object, map_width, map_height) else {
                continue;
            };
            candidates.push(StableMinimapOverlayTileCandidate {
                priority,
                tile,
                kind,
                id: object.id.clone(),
            });
        }
    }

    collect_stable_minimap_overlay_tiles(candidates, max_tiles)
}

fn runtime_minimap_overlay_kind(
    kind: RenderObjectSemanticKind,
) -> Option<WindowMinimapRuntimeOverlayKind> {
    match kind {
        RenderObjectSemanticKind::RuntimeConfig => Some(WindowMinimapRuntimeOverlayKind::Config),
        RenderObjectSemanticKind::RuntimeConfigParseFail
        | RenderObjectSemanticKind::RuntimeConfigNoApply
        | RenderObjectSemanticKind::RuntimeConfigRollback => {
            Some(WindowMinimapRuntimeOverlayKind::ConfigAlert)
        }
        RenderObjectSemanticKind::RuntimeConfigPendingMismatch => {
            Some(WindowMinimapRuntimeOverlayKind::Config)
        }
        RenderObjectSemanticKind::RuntimeBreak | RenderObjectSemanticKind::RuntimeDeconstruct => {
            Some(WindowMinimapRuntimeOverlayKind::Break)
        }
        RenderObjectSemanticKind::RuntimePlace => Some(WindowMinimapRuntimeOverlayKind::Place),
        RenderObjectSemanticKind::RuntimeBuilding => {
            Some(WindowMinimapRuntimeOverlayKind::Building)
        }
        RenderObjectSemanticKind::RuntimeHealth => Some(WindowMinimapRuntimeOverlayKind::Health),
        _ => None,
    }
}

fn runtime_world_to_minimap_tile(world_position: f32, bound: usize) -> usize {
    if bound == 0 || !world_position.is_finite() {
        return 0;
    }
    let upper = bound.saturating_sub(1).min(i32::MAX as usize) as i32;
    crate::presenter_view::world_to_tile_index_floor(world_position, TILE_SIZE).clamp(0, upper)
        as usize
}

fn runtime_world_span_to_tile_span(world_span: f32, bound: usize) -> usize {
    if bound == 0 || !world_span.is_finite() || world_span <= 0.0 {
        return 0;
    }
    ((world_span / TILE_SIZE).round() as usize).clamp(1, bound)
}

fn runtime_minimap_object_tile(
    object: &RenderObject,
    map_width: usize,
    map_height: usize,
) -> Option<(usize, usize)> {
    if !object.x.is_finite() || !object.y.is_finite() {
        return None;
    }
    let tile_x = crate::presenter_view::world_to_tile_index_floor(object.x, TILE_SIZE);
    let tile_y = crate::presenter_view::world_to_tile_index_floor(object.y, TILE_SIZE);
    if tile_x < 0 || tile_y < 0 {
        return None;
    }
    clamp_window_minimap_tile(
        Some((tile_x as usize, tile_y as usize)),
        map_width,
        map_height,
    )
}

#[derive(Debug, Clone)]
struct StableMinimapTileCandidate {
    priority: usize,
    tile: (usize, usize),
    id: String,
}

#[derive(Debug, Clone)]
struct StableMinimapOverlayTileCandidate {
    priority: usize,
    tile: (usize, usize),
    kind: WindowMinimapRuntimeOverlayKind,
    id: String,
}

fn collect_stable_minimap_tiles(
    mut candidates: Vec<StableMinimapTileCandidate>,
    max_tiles: usize,
) -> Vec<(usize, usize)> {
    if max_tiles == 0 {
        return Vec::new();
    }

    candidates.sort_unstable_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then(left.tile.1.cmp(&right.tile.1))
            .then(left.tile.0.cmp(&right.tile.0))
            .then(left.id.cmp(&right.id))
    });

    let mut seen = BTreeSet::new();
    let mut tiles = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.tile) {
            tiles.push(candidate.tile);
            if tiles.len() >= max_tiles {
                break;
            }
        }
    }

    tiles
}

fn collect_stable_minimap_overlay_tiles(
    mut candidates: Vec<StableMinimapOverlayTileCandidate>,
    max_tiles: usize,
) -> Vec<WindowMinimapRuntimeOverlayTile> {
    if max_tiles == 0 {
        return Vec::new();
    }

    candidates.sort_unstable_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then(left.tile.1.cmp(&right.tile.1))
            .then(left.tile.0.cmp(&right.tile.0))
            .then(left.id.cmp(&right.id))
    });

    let mut seen = BTreeSet::new();
    let mut tiles = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.tile) {
            tiles.push(WindowMinimapRuntimeOverlayTile {
                tile: candidate.tile,
                kind: candidate.kind,
            });
            if tiles.len() >= max_tiles {
                break;
            }
        }
    }

    tiles
}

fn runtime_ping_minimap_sequence(id: &str) -> Option<usize> {
    id.strip_prefix("marker:text:runtime-ping:")
        .and_then(|rest| rest.split(':').next())
        .and_then(|sequence| sequence.parse().ok())
}

fn runtime_command_rect_kind_priority(kind: WindowMinimapCommandRectKind) -> usize {
    match kind {
        WindowMinimapCommandRectKind::Selection => 0,
        WindowMinimapCommandRectKind::Target => 1,
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

    crop_window_to_focus(scene, TILE_SIZE, base_window, window_width, window_height)
}

#[derive(Debug, Clone, Copy)]
enum WindowRenderCommand<'a> {
    Point {
        object: &'a RenderObject,
        local_x: usize,
        local_y: usize,
    },
    Line {
        layer: i32,
        start_tile: (i32, i32),
        end_tile: (i32, i32),
        color: u32,
    },
    Rect {
        layer: i32,
        left_tile: i32,
        top_tile: i32,
        right_tile: i32,
        bottom_tile: i32,
        color: u32,
    },
    Icon {
        layer: i32,
        local_x: usize,
        local_y: usize,
        color: u32,
    },
}

impl WindowRenderCommand<'_> {
    fn layer(&self) -> i32 {
        match self {
            Self::Point { object, .. } => object.layer,
            Self::Line { layer, .. } | Self::Rect { layer, .. } | Self::Icon { layer, .. } => {
                *layer
            }
        }
    }
}

fn window_render_command<'a>(
    object: &'a RenderObject,
    line_end_objects: &BTreeMap<String, &'a RenderObject>,
    window: PresenterViewWindow,
) -> Option<WindowRenderCommand<'a>> {
    match object.semantic_kind() {
        RenderObjectSemanticKind::MarkerLineEnd => None,
        RenderObjectSemanticKind::MarkerLine => {
            if let Some(line_end) = line_end_objects.get(&object.id) {
                let (Some(start_tile), Some(end_tile)) = (
                    window_world_object_tile(object),
                    window_world_object_tile(line_end),
                ) else {
                    return None;
                };
                return Some(WindowRenderCommand::Line {
                    layer: object.layer,
                    start_tile,
                    end_tile,
                    color: color_for_object(object),
                });
            }
            visible_window_tile(
                object,
                TILE_SIZE,
                window.origin_x,
                window.origin_y,
                window.width,
                window.height,
            )
            .map(|(object, local_x, local_y)| WindowRenderCommand::Point {
                object,
                local_x,
                local_y,
            })
        }
        _ => visible_window_tile(
            object,
            TILE_SIZE,
            window.origin_x,
            window.origin_y,
            window.width,
            window.height,
        )
        .map(|(object, local_x, local_y)| WindowRenderCommand::Point {
            object,
            local_x,
            local_y,
        }),
    }
}

fn window_line_end_object_pair(object: &RenderObject) -> Option<(String, &RenderObject)> {
    if object.semantic_kind() != RenderObjectSemanticKind::MarkerLineEnd {
        return None;
    }
    object
        .id
        .strip_suffix(":line-end")
        .map(|base_id| (base_id.to_string(), object))
}

fn window_primitive_render_command<'a>(
    primitive: &'a RenderPrimitive,
    window: PresenterViewWindow,
) -> Option<WindowRenderCommand<'a>> {
    match primitive {
        RenderPrimitive::Rect {
            layer,
            left,
            top,
            right,
            bottom,
            ..
        } => {
            let left_tile = crate::presenter_view::world_to_tile_index_floor(*left, TILE_SIZE);
            let top_tile = crate::presenter_view::world_to_tile_index_floor(*top, TILE_SIZE);
            let right_tile = crate::presenter_view::world_to_tile_index_floor(*right, TILE_SIZE);
            let bottom_tile = crate::presenter_view::world_to_tile_index_floor(*bottom, TILE_SIZE);
            if right_tile < window.origin_x as i32
                || bottom_tile < window.origin_y as i32
                || left_tile >= window.origin_x.saturating_add(window.width) as i32
                || top_tile >= window.origin_y.saturating_add(window.height) as i32
            {
                return None;
            }
            Some(WindowRenderCommand::Rect {
                layer: *layer,
                left_tile,
                top_tile,
                right_tile,
                bottom_tile,
                color: 0xff44_88ff,
            })
        }
        RenderPrimitive::Icon {
            family,
            layer,
            x,
            y,
            ..
        } => {
            let (tile_x, tile_y) = finite_tile_coords(*x, *y)?;
            if tile_x < 0 || tile_y < 0 {
                return None;
            }
            let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
            if tile_x < window.origin_x
                || tile_y < window.origin_y
                || tile_x >= window.origin_x.saturating_add(window.width)
                || tile_y >= window.origin_y.saturating_add(window.height)
            {
                return None;
            }
            Some(WindowRenderCommand::Icon {
                layer: *layer,
                local_x: tile_x - window.origin_x,
                local_y: tile_y - window.origin_y,
                color: color_for_icon(*family),
            })
        }
        _ => None,
    }
}

fn viewport_tile_span(world_span: f32) -> usize {
    if !world_span.is_finite() {
        return 1;
    }
    ((world_span / TILE_SIZE).round().max(0.0) as usize).max(1)
}

fn finite_tile_coords(world_x: f32, world_y: f32) -> Option<(i32, i32)> {
    if !world_x.is_finite() || !world_y.is_finite() {
        return None;
    }
    Some((
        crate::presenter_view::world_to_tile_index_floor(world_x, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(world_y, TILE_SIZE),
    ))
}

fn finite_rect_tile_coords(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Option<(i32, i32, i32, i32)> {
    if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
        return None;
    }
    Some((
        crate::presenter_view::world_to_tile_index_floor(left, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(top, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(right, TILE_SIZE),
        crate::presenter_view::world_to_tile_index_floor(bottom, TILE_SIZE),
    ))
}

fn window_world_object_tile(object: &RenderObject) -> Option<(i32, i32)> {
    finite_tile_coords(object.x, object.y)
}

fn draw_window_line_segment(
    tiles: &mut [u32],
    window: PresenterViewWindow,
    start_tile: (i32, i32),
    end_tile: (i32, i32),
    color: u32,
) {
    let (mut x0, mut y0) = start_tile;
    let (x1, y1) = end_tile;
    let dx = (x1 - x0).abs();
    let sx = if x0 <= x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 <= y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        draw_window_tile_if_visible(tiles, window, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled_error = err.saturating_mul(2);
        if doubled_error >= dy {
            err += dy;
            x0 += sx;
        }
        if doubled_error <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_window_rect_outline(
    tiles: &mut [u32],
    window: PresenterViewWindow,
    left_tile: i32,
    top_tile: i32,
    right_tile: i32,
    bottom_tile: i32,
    color: u32,
) {
    draw_window_line_segment(
        tiles,
        window,
        (left_tile, top_tile),
        (right_tile, top_tile),
        color,
    );
    draw_window_line_segment(
        tiles,
        window,
        (right_tile, top_tile),
        (right_tile, bottom_tile),
        color,
    );
    draw_window_line_segment(
        tiles,
        window,
        (right_tile, bottom_tile),
        (left_tile, bottom_tile),
        color,
    );
    draw_window_line_segment(
        tiles,
        window,
        (left_tile, bottom_tile),
        (left_tile, top_tile),
        color,
    );
}

fn draw_window_tile_if_visible(
    tiles: &mut [u32],
    window: PresenterViewWindow,
    tile_x: i32,
    tile_y: i32,
    color: u32,
) {
    let Ok(tile_x) = usize::try_from(tile_x) else {
        return;
    };
    let Ok(tile_y) = usize::try_from(tile_y) else {
        return;
    };
    if tile_x < window.origin_x
        || tile_y < window.origin_y
        || tile_x >= window.origin_x.saturating_add(window.width)
        || tile_y >= window.origin_y.saturating_add(window.height)
    {
        return;
    }
    let local_x = tile_x - window.origin_x;
    let local_y = tile_y - window.origin_y;
    tiles[local_y * window.width + local_x] = color;
}

fn color_for_object(object: &RenderObject) -> u32 {
    color_for_semantic_kind(object.semantic_kind())
}

fn color_for_icon(family: RenderIconPrimitiveFamily) -> u32 {
    match family {
        RenderIconPrimitiveFamily::RuntimeEffect => COLOR_ICON_RUNTIME_EFFECT,
        RenderIconPrimitiveFamily::RuntimeEffectMarker => COLOR_ICON_RUNTIME_EFFECT_MARKER,
        RenderIconPrimitiveFamily::RuntimeBuildConfig => COLOR_ICON_BUILD_CONFIG,
        RenderIconPrimitiveFamily::RuntimeConfig
        | RenderIconPrimitiveFamily::RuntimeConfigParseFail
        | RenderIconPrimitiveFamily::RuntimeConfigNoApply
        | RenderIconPrimitiveFamily::RuntimeConfigRollback
        | RenderIconPrimitiveFamily::RuntimeConfigPendingMismatch => COLOR_ICON_BUILD_CONFIG,
        RenderIconPrimitiveFamily::RuntimeHealth => COLOR_ICON_RUNTIME_HEALTH,
        RenderIconPrimitiveFamily::RuntimeCommand => COLOR_ICON_RUNTIME_COMMAND,
        RenderIconPrimitiveFamily::RuntimePlace => COLOR_PLAN,
        RenderIconPrimitiveFamily::RuntimeUnitAssemblerProgress
        | RenderIconPrimitiveFamily::RuntimeUnitAssemblerCommand => {
            COLOR_ICON_RUNTIME_UNIT_ASSEMBLER
        }
        RenderIconPrimitiveFamily::RuntimeBreak => COLOR_ICON_RUNTIME_BREAK,
        RenderIconPrimitiveFamily::RuntimeBullet => COLOR_ICON_RUNTIME_BULLET,
        RenderIconPrimitiveFamily::RuntimeLogicExplosion => COLOR_ICON_RUNTIME_LOGIC_EXPLOSION,
        RenderIconPrimitiveFamily::RuntimeSoundAt => COLOR_ICON_RUNTIME_SOUND_AT,
        RenderIconPrimitiveFamily::RuntimeTileAction => COLOR_ICON_RUNTIME_TILE_ACTION,
    }
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
    let mut parts = Vec::new();
    match (
        title_prefix.is_empty(),
        frame.title.is_empty(),
    ) {
        (false, false) => parts.push(format!("{title_prefix}/{}", frame.title)),
        (false, true) => parts.push(title_prefix.to_string()),
        (true, false) => parts.push(frame.title.clone()),
        (true, true) => {}
    }
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
    parts.join(" · ")
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
    if let Some(zoom_text) = compose_zoom_status_text(scene) {
        parts.push(zoom_text);
    }
    parts.join(" ")
}

fn compose_zoom_status_text(scene: &RenderModel) -> Option<String> {
    let zoom = scene.viewport.zoom;
    (zoom != 1.0).then(|| format!("zoom={zoom:.2}"))
}

fn compose_frame_session_banner_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if !panel.kick.is_empty() {
        return Some(format!(
            "KICK {}",
            compose_runtime_kick_panel_status_text(&panel.kick)
        ));
    }
    let mut segments = Vec::new();
    if let Some(world_reload) = panel.loading.last_world_reload.as_ref() {
        segments.push(format!(
            "RELOAD {}",
            runtime_world_reload_panel_status_text(Some(world_reload))
        ));
    }
    if !panel.reconnect.is_empty() {
        segments.push(format!(
            "RECONNECT {}",
            compose_runtime_reconnect_panel_status_text(&panel.reconnect)
        ));
    }
    if !panel.loading.is_empty() {
        segments.push(format!(
            "LOADING {}",
            compose_runtime_loading_panel_status_text(&panel.loading)
        ));
    }
    (!segments.is_empty()).then(|| segments.join(" | "))
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
    if let Some(visibility_detail_text) = compose_hud_visibility_detail_status_text(hud) {
        lines.push(format!("HUD-VIS-DETAIL: {visibility_detail_text}"));
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
    if let Some(visibility_minimap_text) =
        compose_visibility_minimap_status_text(scene, hud, window)
    {
        lines.push(format!("VIS-MINIMAP: {visibility_minimap_text}"));
    }
    if let Some(minimap_visibility_detail_text) =
        compose_minimap_visibility_detail_status_text(scene, hud, window)
    {
        lines.push(format!(
            "MINIMAP-VIS-DETAIL: {minimap_visibility_detail_text}"
        ));
    }
    if let Some(minimap_flow_text) = compose_minimap_flow_status_text(scene, hud, window) {
        lines.push(format!("MINIMAP-FLOW: {minimap_flow_text}"));
    }
    if let Some(minimap_flow_detail_text) =
        compose_minimap_flow_detail_status_text(scene, hud, window)
    {
        lines.push(format!("MINIMAP-FLOW-DETAIL: {minimap_flow_detail_text}"));
    }
    if let Some(minimap_kind_text) = compose_minimap_kind_status_text(scene, hud) {
        lines.push(format!("MINIMAP-KINDS: {minimap_kind_text}"));
    }
    if let Some(minimap_kind_detail_text) = compose_minimap_kind_detail_status_text(scene, hud) {
        lines.push(format!("MINIMAP-KINDS-DETAIL: {minimap_kind_detail_text}"));
    }
    if let Some(minimap_window_kinds_text) = compose_minimap_window_kind_status_text(scene, hud) {
        lines.push(format!("MINIMAP-WINDOW-KINDS: {minimap_window_kinds_text}"));
    }
    if let Some(minimap_window_text) =
        compose_minimap_window_distribution_line_status_text(scene, hud)
    {
        lines.push(format!("MINIMAP-WINDOW: {minimap_window_text}"));
    }
    if let Some(minimap_legend_text) = compose_minimap_legend_status_text(hud) {
        lines.push(format!("MINIMAP-LEGEND: {minimap_legend_text}"));
    }
    if let Some(minimap_edge_text) = compose_minimap_edge_status_text(scene, hud, window) {
        lines.push(format!("MINIMAP-EDGE: {minimap_edge_text}"));
    }
    if let Some(minimap_edge_detail_text) =
        compose_minimap_edge_detail_status_text(scene, hud, window)
    {
        lines.push(format!("MINIMAP-EDGE-DETAIL: {minimap_edge_detail_text}"));
    }
    for minimap_detail_text in compose_minimap_detail_status_lines(scene, hud, window) {
        lines.push(format!("MINIMAP-DETAIL: {minimap_detail_text}"));
    }
    if let Some(render_pipeline_text) = compose_render_pipeline_status_text(scene, window) {
        lines.push(format!("RENDER-PIPELINE: {render_pipeline_text}"));
    }
    if let Some(render_pipeline_detail_text) =
        compose_render_pipeline_detail_status_text(scene, window)
    {
        lines.push(format!(
            "RENDER-PIPELINE-DETAIL: {render_pipeline_detail_text}"
        ));
    }
    for render_layer_text in compose_render_layer_status_lines(scene, window) {
        lines.push(format!("RENDER-LAYER: {render_layer_text}"));
    }
    for render_layer_detail_text in compose_render_layer_detail_status_lines(scene, window) {
        lines.push(format!("RENDER-LAYER-DETAIL: {render_layer_detail_text}"));
    }
    if let Some(render_line_text) = compose_render_line_status_text(scene, window) {
        lines.push(format!("RENDER-LINE: {render_line_text}"));
    }
    if let Some(render_line_detail_text) = compose_render_line_detail_status_text(scene, window) {
        lines.push(format!("RENDER-LINE-DETAIL: {render_line_detail_text}"));
    }
    if let Some(render_text_text) = compose_render_text_status_text(scene, window) {
        lines.push(format!("RENDER-TEXT: {render_text_text}"));
    }
    if let Some(render_text_detail_text) = compose_render_text_detail_status_text(scene, window) {
        lines.push(format!("RENDER-TEXT-DETAIL: {render_text_detail_text}"));
    }
    if let Some(render_rect_text) = compose_render_rect_status_text(scene, window) {
        lines.push(format!("RENDER-RECT: {render_rect_text}"));
    }
    if let Some(render_rect_detail_text) = compose_render_rect_detail_status_text(scene, window) {
        lines.push(format!("RENDER-RECT-DETAIL: {render_rect_detail_text}"));
    }
    if let Some(render_icon_text) = compose_render_icon_status_text(scene, window) {
        lines.push(format!("RENDER-ICON: {render_icon_text}"));
    }
    if let Some(render_icon_detail_text) = compose_render_icon_detail_status_text(scene, window) {
        lines.push(format!("RENDER-ICON-DETAIL: {render_icon_detail_text}"));
    }
    if let Some(build_panel_text) = compose_build_config_panel_status_text(hud) {
        lines.push(format!("BUILD-CONFIG: {build_panel_text}"));
    }
    if let Some(build_config_detail_text) = compose_build_config_detail_status_text(hud) {
        lines.push(format!("BUILD-CONFIG-DETAIL: {build_config_detail_text}"));
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
    if let Some(build_rollback_detail_text) = compose_build_config_rollback_detail_status_text(hud)
    {
        lines.push(format!(
            "BUILD-ROLLBACK-DETAIL: {build_rollback_detail_text}"
        ));
    }
    if let Some(build_interaction_text) = compose_build_interaction_status_text(hud) {
        lines.push(format!("BUILD-INTERACTION: {build_interaction_text}"));
    }
    if let Some(build_interaction_detail_text) = compose_build_interaction_detail_status_text(hud) {
        lines.push(format!(
            "BUILD-INTERACTION-DETAIL: {build_interaction_detail_text}"
        ));
    }
    if let Some(build_queue_text) = compose_build_ui_queue_status_text(hud) {
        lines.push(format!("BUILD-QUEUE: {build_queue_text}"));
    }
    if let Some(build_queue_detail_text) = compose_build_ui_queue_detail_status_text(hud) {
        lines.push(format!("BUILD-QUEUE-DETAIL: {build_queue_detail_text}"));
    }
    if let Some(build_minimap_aux_text) = compose_build_minimap_aux_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-MINIMAP-AUX: {build_minimap_aux_text}"));
    }
    if let Some(build_minimap_diag_text) =
        compose_build_minimap_diag_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-MINIMAP-DIAG: {build_minimap_diag_text}"));
    }
    if let Some(build_minimap_flow_text) =
        compose_build_minimap_flow_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-MINIMAP-FLOW: {build_minimap_flow_text}"));
    }
    if let Some(build_minimap_detail_text) =
        compose_build_minimap_detail_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-MINIMAP-DETAIL: {build_minimap_detail_text}"));
    }
    if let Some(build_flow_text) = compose_build_flow_status_text(scene, hud, window) {
        lines.push(format!("BUILD-FLOW: {build_flow_text}"));
    }
    if let Some(build_flow_summary_text) =
        compose_build_flow_summary_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-FLOW-SUMMARY: {build_flow_summary_text}"));
    }
    if let Some(build_route_text) = compose_build_route_status_text(scene, hud, window) {
        lines.push(format!("BUILD-ROUTE: {build_route_text}"));
    }
    if let Some(build_route_detail_text) =
        compose_build_route_detail_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-ROUTE-DETAIL: {build_route_detail_text}"));
    }
    if let Some(build_flow_detail_text) = compose_build_flow_detail_status_text(scene, hud, window)
    {
        lines.push(format!("BUILD-FLOW-DETAIL: {build_flow_detail_text}"));
    }
    if let Some(build_ui) = hud.build_ui.as_ref() {
        let inspector_text = compose_build_ui_inspector_status_text(build_ui);
        if !inspector_text.is_empty() {
            lines.push(format!("BUILD-INSPECTOR: {inspector_text}"));
        }
    }
    if let Some(runtime_ui_notice_text) = compose_runtime_ui_notice_panel_status_text(hud) {
        lines.push(format!("RUNTIME-NOTICE: {runtime_ui_notice_text}"));
    }
    if let Some(runtime_notice_state_text) = compose_runtime_notice_state_panel_status_text(hud) {
        lines.push(format!("RUNTIME-NOTICE-STATE: {runtime_notice_state_text}"));
    }
    if let Some(runtime_notice_state_detail_text) =
        compose_runtime_notice_state_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-NOTICE-STATE-DETAIL: {runtime_notice_state_detail_text}"
        ));
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
    if let Some(runtime_choice_text) = compose_runtime_choice_panel_status_text(hud) {
        lines.push(format!("RUNTIME-CHOICE: {runtime_choice_text}"));
    }
    if let Some(runtime_choice_detail_text) = compose_runtime_choice_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-CHOICE-DETAIL: {runtime_choice_detail_text}"
        ));
    }
    if let Some(runtime_prompt_text) = compose_runtime_prompt_panel_status_text(hud) {
        lines.push(format!("RUNTIME-PROMPT: {runtime_prompt_text}"));
    }
    if let Some(runtime_prompt_detail_text) = compose_runtime_prompt_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-PROMPT-DETAIL: {runtime_prompt_detail_text}"
        ));
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
    if let Some(runtime_dialog_stack_text) = compose_runtime_dialog_stack_status_text(hud) {
        lines.push(format!("RUNTIME-DIALOG-STACK: {runtime_dialog_stack_text}"));
    }
    if let Some(runtime_command_text) = compose_runtime_command_mode_panel_status_text(hud) {
        lines.push(format!("RUNTIME-COMMAND: {runtime_command_text}"));
    }
    if let Some(runtime_command_detail_text) = compose_runtime_command_mode_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-COMMAND-DETAIL: {runtime_command_detail_text}"
        ));
    }
    for runtime_command_group_text in compose_runtime_command_group_status_lines(hud) {
        lines.push(format!(
            "RUNTIME-COMMAND-GROUP: {runtime_command_group_text}"
        ));
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
    if let Some(runtime_marker_text) = compose_runtime_marker_panel_status_text(hud) {
        lines.push(format!("RUNTIME-MARKER: {runtime_marker_text}"));
    }
    if let Some(runtime_marker_detail_text) = compose_runtime_marker_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-MARKER-DETAIL: {runtime_marker_detail_text}"
        ));
    }
    if let Some(runtime_session_text) = compose_runtime_session_status_text(hud) {
        lines.push(format!("RUNTIME-SESSION: {runtime_session_text}"));
    }
    if let Some(runtime_session_detail_text) = compose_runtime_session_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-SESSION-DETAIL: {runtime_session_detail_text}"
        ));
    }
    if let Some(runtime_bootstrap_text) = compose_runtime_bootstrap_status_text(hud) {
        lines.push(format!("RUNTIME-BOOTSTRAP: {runtime_bootstrap_text}"));
    }
    if let Some(runtime_bootstrap_detail_text) = compose_runtime_bootstrap_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-BOOTSTRAP-DETAIL: {runtime_bootstrap_detail_text}"
        ));
    }
    if let Some(runtime_resource_delta_text) = compose_runtime_resource_delta_status_text(hud) {
        lines.push(format!(
            "RUNTIME-RESOURCE-DELTA: {runtime_resource_delta_text}"
        ));
    }
    if let Some(runtime_resource_delta_detail_text) =
        compose_runtime_resource_delta_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-RESOURCE-DELTA-DETAIL: {runtime_resource_delta_detail_text}"
        ));
    }
    if let Some(runtime_kick_text) = compose_runtime_kick_status_text(hud) {
        lines.push(format!("RUNTIME-KICK: {runtime_kick_text}"));
    }
    if let Some(runtime_kick_detail_text) = compose_runtime_kick_detail_status_text(hud) {
        lines.push(format!("RUNTIME-KICK-DETAIL: {runtime_kick_detail_text}"));
    }
    if let Some(runtime_loading_text) = compose_runtime_loading_status_text(hud) {
        lines.push(format!("RUNTIME-LOADING: {runtime_loading_text}"));
    }
    if let Some(runtime_loading_detail_text) = compose_runtime_loading_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-LOADING-DETAIL: {runtime_loading_detail_text}"
        ));
    }
    if let Some(runtime_world_reload_text) = compose_runtime_world_reload_status_text(hud) {
        lines.push(format!("RUNTIME-WORLD-RELOAD: {runtime_world_reload_text}"));
    }
    if let Some(runtime_world_reload_detail_text) =
        compose_runtime_world_reload_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-WORLD-RELOAD-DETAIL: {runtime_world_reload_detail_text}"
        ));
    }
    if let Some(runtime_core_binding_text) = compose_runtime_core_binding_panel_status_text(hud) {
        lines.push(format!("RUNTIME-CORE-BINDING: {runtime_core_binding_text}"));
    }
    if let Some(runtime_core_binding_detail_text) =
        compose_runtime_core_binding_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-CORE-BINDING-DETAIL: {runtime_core_binding_detail_text}"
        ));
    }
    if let Some(runtime_reconnect_text) = compose_runtime_reconnect_status_text(hud) {
        lines.push(format!("RUNTIME-RECONNECT: {runtime_reconnect_text}"));
    }
    if let Some(runtime_reconnect_detail_text) = compose_runtime_reconnect_detail_status_text(hud) {
        lines.push(format!(
            "RUNTIME-RECONNECT-DETAIL: {runtime_reconnect_detail_text}"
        ));
    }
    if let Some(runtime_live_entity_text) = compose_runtime_live_entity_panel_status_text(hud) {
        lines.push(format!("RUNTIME-LIVE-ENTITY: {runtime_live_entity_text}"));
    }
    if let Some(runtime_live_entity_detail_text) =
        compose_runtime_live_entity_detail_status_text(hud)
    {
        lines.push(format!(
            "RUNTIME-LIVE-ENTITY-DETAIL: {runtime_live_entity_detail_text}"
        ));
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

fn compose_frame_build_strip_text(hud: &HudModel) -> Option<String> {
    let interaction_panel = build_build_interaction_panel(hud);
    let build_ui = hud.build_ui.as_ref();

    if interaction_panel.is_none() && build_ui.is_none() {
        return None;
    }

    let selected_block_id = interaction_panel
        .as_ref()
        .and_then(|panel| panel.selected_block_id)
        .or_else(|| build_ui.and_then(|panel| panel.selected_block_id));
    let rotation = interaction_panel
        .as_ref()
        .map(|panel| panel.selected_rotation)
        .or_else(|| build_ui.map(|panel| panel.selected_rotation))
        .unwrap_or_default();
    let queue_text = interaction_panel
        .as_ref()
        .map(compose_build_strip_queue_text)
        .or_else(|| build_ui.map(compose_build_strip_queue_fallback_text))
        .unwrap_or_else(|| "none".to_string());
    let authority_text = interaction_panel
        .as_ref()
        .map(|panel| build_interaction_authority_status_text(panel.authority_state).to_string())
        .unwrap_or_else(|| "none".to_string());

    Some(format!(
        "BUILD: sel={} r{} q={} auth={}",
        optional_i16_label(selected_block_id),
        rotation,
        queue_text,
        authority_text,
    ))
}

fn compose_frame_build_strip_detail_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_interaction_panel(hud)?;
    Some(format!("BUILD-STRIP-DETAIL: {}", panel.detail_label()))
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
    if let Some(overlay_detail_text) = compose_overlay_detail_status_text(scene) {
        lines.push(format!("OVERLAY-DETAIL: {overlay_detail_text}"));
    }
    lines
}

fn compose_render_line_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut line_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Line {
                layer,
                x0,
                y0,
                x1,
                y1,
                ..
            } => {
                let (start_tile_x, start_tile_y) = finite_tile_coords(*x0, *y0)?;
                let (end_tile_x, end_tile_y) = finite_tile_coords(*x1, *y1)?;
                if !render_line_is_visible(
                    window,
                    start_tile_x,
                    start_tile_y,
                    end_tile_x,
                    end_tile_y,
                ) {
                    return None;
                }
                let label = primitive
                    .payload()
                    .map(|payload| payload.label)
                    .unwrap_or_else(|| "line".to_string());
                Some((
                    *layer,
                    label,
                    start_tile_x,
                    start_tile_y,
                    end_tile_x,
                    end_tile_y,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if line_primitives.is_empty() {
        return None;
    }

    let total = line_primitives.len();
    line_primitives.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
    });

    let mut parts = vec![format!("count={total}")];
    for (layer, label, start_tile_x, start_tile_y, end_tile_x, end_tile_y) in
        line_primitives.into_iter().take(2)
    {
        parts.push(format!(
            "{label}@{layer}:{start_tile_x}:{start_tile_y}->{end_tile_x}:{end_tile_y}"
        ));
    }
    if total > 2 {
        parts.push(format!("more={}", total - 2));
    }
    Some(parts.join(" "))
}

fn compose_render_line_detail_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut line_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Line {
                layer,
                x0,
                y0,
                x1,
                y1,
                ..
            } => {
                let (start_tile_x, start_tile_y) = finite_tile_coords(*x0, *y0)?;
                let (end_tile_x, end_tile_y) = finite_tile_coords(*x1, *y1)?;
                if !render_line_is_visible(
                    window,
                    start_tile_x,
                    start_tile_y,
                    end_tile_x,
                    end_tile_y,
                ) {
                    return None;
                }
                let payload = primitive.payload()?;
                Some((
                    *layer,
                    payload.label.clone(),
                    start_tile_x,
                    start_tile_y,
                    end_tile_x,
                    end_tile_y,
                    payload,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if line_primitives.is_empty() {
        return None;
    }

    line_primitives.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
    });

    let mut parts = vec![format!("count={}", line_primitives.len())];
    for (layer, label, start_tile_x, start_tile_y, end_tile_x, end_tile_y, payload) in
        line_primitives
    {
        parts.push(format!(
            "{label}@{layer}:{start_tile_x}:{start_tile_y}->{end_tile_x}:{end_tile_y} {}",
            format_render_primitive_payload(&payload)
        ));
    }
    Some(parts.join(" "))
}

fn compose_render_text_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut text_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Text {
                kind,
                layer,
                x,
                y,
                text,
                ..
            } => Some((kind, layer, x, y, text)),
            _ => None,
        })
        .filter(|(_, _, x, y, _)| {
            let Some((tile_x, tile_y)) = finite_tile_coords(*x, *y) else {
                return false;
            };
            tile_x >= 0
                && tile_y >= 0
                && (tile_x as usize) >= window.origin_x
                && (tile_y as usize) >= window.origin_y
                && (tile_x as usize) < window.origin_x.saturating_add(window.width)
                && (tile_y as usize) < window.origin_y.saturating_add(window.height)
        })
        .collect::<Vec<_>>();

    if text_primitives.is_empty() {
        return None;
    }

    let total = text_primitives.len();
    text_primitives.sort_by_key(|(_, layer, _, _, _)| *layer);

    let mut parts = vec![format!("count={total}")];
    for (kind, layer, x, y, text) in text_primitives.into_iter().take(2) {
        let kind_text = kind.detail_label().unwrap_or("text");
        parts.push(format!(
            "{kind_text}@{layer}:{}:{}={}",
            x as i32,
            y as i32,
            compact_runtime_ui_text(Some(text.as_str()))
        ));
    }
    if total > 2 {
        parts.push(format!("more={}", total - 2));
    }

    Some(parts.join(" "))
}

fn compose_render_text_detail_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut text_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Text {
                kind, layer, x, y, ..
            } => {
                let Some((tile_x, tile_y)) = finite_tile_coords(*x, *y) else {
                    return None;
                };
                if tile_x < 0
                    || tile_y < 0
                    || (tile_x as usize) < window.origin_x
                    || (tile_y as usize) < window.origin_y
                    || (tile_x as usize) >= window.origin_x.saturating_add(window.width)
                    || (tile_y as usize) >= window.origin_y.saturating_add(window.height)
                {
                    return None;
                }
                let payload = primitive.payload()?;
                Some((
                    kind.detail_label().unwrap_or("text"),
                    *layer,
                    *x as i32,
                    *y as i32,
                    payload,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if text_primitives.is_empty() {
        return None;
    }

    text_primitives.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| left.0.cmp(right.0))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
    });

    let mut parts = vec![format!("count={}", text_primitives.len())];
    for (kind_label, layer, tile_x, tile_y, payload) in text_primitives {
        parts.push(format!(
            "{kind_label}@{layer}:{tile_x}:{tile_y} {}",
            format_render_primitive_payload(&payload)
        ));
    }
    Some(parts.join(" "))
}

fn compose_render_rect_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut rect_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Rect {
                family,
                layer,
                left,
                top,
                right,
                bottom,
                ..
            } => Some((family, layer, left, top, right, bottom)),
            _ => None,
        })
        .filter(|(_, _, left, top, right, bottom)| {
            let Some((left_tile, top_tile, right_tile, bottom_tile)) =
                finite_rect_tile_coords(*left, *top, *right, *bottom)
            else {
                return false;
            };
            render_rect_detail_is_visible(window, left_tile, top_tile, right_tile, bottom_tile)
        })
        .collect::<Vec<_>>();

    if rect_primitives.is_empty() {
        return None;
    }

    let total = rect_primitives.len();
    rect_primitives.sort_by_key(|(_, layer, _, _, _, _)| *layer);
    let mut parts = vec![format!("count={total}")];
    for (family, layer, left, top, right, bottom) in rect_primitives.into_iter().take(2) {
        parts.push(format!(
            "{family}@{layer}:{}:{}:{}:{}",
            left as i32, top as i32, right as i32, bottom as i32
        ));
    }
    if total > 2 {
        parts.push(format!("more={}", total - 2));
    }
    Some(parts.join(" "))
}

fn compose_render_rect_detail_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut rect_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Rect {
                family,
                layer,
                left,
                top,
                right,
                bottom,
                line_ids,
                ..
            } => {
                let (left_tile, top_tile, right_tile, bottom_tile) =
                    finite_rect_tile_coords(*left, *top, *right, *bottom)?;
                if !render_rect_detail_is_visible(
                    window,
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                ) {
                    return None;
                }
                let payload = primitive.payload();
                let (block_name, tile_x, tile_y) =
                    render_rect_detail_payload_fields(payload.as_ref());
                Some((
                    *layer,
                    family.clone(),
                    *left as i32,
                    *top as i32,
                    *right as i32,
                    *bottom as i32,
                    left_tile,
                    top_tile,
                    right_tile,
                    bottom_tile,
                    line_ids.len(),
                    block_name,
                    tile_x,
                    tile_y,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if rect_primitives.is_empty() {
        return None;
    }

    rect_primitives.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
    });

    let mut parts = vec![format!("count={}", rect_primitives.len())];
    for (
        layer,
        family,
        left,
        top,
        right,
        bottom,
        left_tile,
        top_tile,
        right_tile,
        bottom_tile,
        line_count,
        block_name,
        tile_x,
        tile_y,
    ) in rect_primitives
    {
        parts.push(format!(
            "{family}@{layer}:{left}:{top}:{right}:{bottom} {family}{{{}}}",
            format_render_rect_detail_fields(
                left_tile,
                top_tile,
                right_tile,
                bottom_tile,
                line_count,
                block_name.as_deref(),
                tile_x,
                tile_y
            )
        ));
    }
    Some(parts.join(" "))
}

fn compose_render_icon_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut icon_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match primitive {
            RenderPrimitive::Icon {
                family,
                variant,
                layer,
                x,
                y,
                ..
            } => Some((family, variant, layer, x, y)),
            _ => None,
        })
        .filter(|(_, _, _, x, y)| {
            let Some((tile_x, tile_y)) = finite_tile_coords(*x, *y) else {
                return false;
            };
            tile_x >= 0
                && tile_y >= 0
                && (tile_x as usize) >= window.origin_x
                && (tile_y as usize) >= window.origin_y
                && (tile_x as usize) < window.origin_x.saturating_add(window.width)
                && (tile_y as usize) < window.origin_y.saturating_add(window.height)
        })
        .collect::<Vec<_>>();

    if icon_primitives.is_empty() {
        return None;
    }

    let total = icon_primitives.len();
    icon_primitives.sort_by_key(|(_, _, layer, _, _)| *layer);
    let mut parts = vec![format!("count={total}")];
    for (family, variant, layer, x, y) in icon_primitives.into_iter().take(2) {
        let Some((tile_x, tile_y)) = finite_tile_coords(x, y) else {
            continue;
        };
        parts.push(format!(
            "{}/{}@{layer}:{tile_x}:{tile_y}",
            family.label(),
            variant
        ));
    }
    if total > 2 {
        parts.push(format!("more={}", total - 2));
    }
    Some(parts.join(" "))
}

fn compose_render_icon_detail_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let mut icon_primitives = scene
        .primitives()
        .into_iter()
        .filter_map(|primitive| match &primitive {
            RenderPrimitive::Icon {
                family,
                variant,
                layer,
                x,
                y,
                ..
            } => {
                let Some((tile_x, tile_y)) = finite_tile_coords(*x, *y) else {
                    return None;
                };
                if tile_x < 0
                    || tile_y < 0
                    || (tile_x as usize) < window.origin_x
                    || (tile_y as usize) < window.origin_y
                    || (tile_x as usize) >= window.origin_x.saturating_add(window.width)
                    || (tile_y as usize) >= window.origin_y.saturating_add(window.height)
                {
                    return None;
                }
                let payload = primitive.payload()?;
                Some((*family, variant.clone(), *layer, tile_x, tile_y, payload))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if icon_primitives.is_empty() {
        return None;
    }

    icon_primitives.sort_by_key(|(_, _, layer, _, _, _)| *layer);
    let mut parts = vec![format!("count={}", icon_primitives.len())];
    for (family, variant, layer, tile_x, tile_y, payload) in icon_primitives {
        parts.push(format!(
            "{}/{}@{layer}:{tile_x}:{tile_y} {}",
            family.label(),
            variant,
            format_render_primitive_payload(&payload)
        ));
    }
    Some(parts.join(" "))
}

fn format_render_primitive_payload(payload: &RenderPrimitivePayload) -> String {
    let mut parts = Vec::new();
    if let Some(variant) = payload.field("variant") {
        parts.push(format!(
            "variant={}",
            format_render_primitive_payload_value("variant", variant)
        ));
    }
    for (field_name, field_value) in &payload.fields {
        if *field_name == "variant" {
            continue;
        }
        parts.push(format!(
            "{field_name}={}",
            format_render_primitive_payload_value(field_name, field_value)
        ));
    }
    format!("{}{{{}}}", payload.label, parts.join(","))
}

fn render_line_is_visible(
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

fn render_rect_detail_is_visible(
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

fn render_rect_detail_payload_fields(
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

fn format_render_rect_detail_fields(
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

fn format_render_primitive_payload_value(
    field_name: &str,
    value: &RenderPrimitivePayloadValue,
) -> String {
    match value {
        RenderPrimitivePayloadValue::Bool(value) => value.to_string(),
        RenderPrimitivePayloadValue::I16(value) => value.to_string(),
        RenderPrimitivePayloadValue::I32(value) => value.to_string(),
        RenderPrimitivePayloadValue::I32List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        RenderPrimitivePayloadValue::U8(value) => value.to_string(),
        RenderPrimitivePayloadValue::U8List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        RenderPrimitivePayloadValue::U32(value) => {
            if field_name.ends_with("_bits") {
                format!("0x{value:08x}")
            } else {
                value.to_string()
            }
        }
        RenderPrimitivePayloadValue::Usize(value) => value.to_string(),
        RenderPrimitivePayloadValue::Text(value) => value.clone(),
        RenderPrimitivePayloadValue::TextList(values) => format!("[{}]", values.join(",")),
    }
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
        "hudvis:ov{}:fg{}:k{}p{}:v{}p{}:h{}p{}:u{}p{}:vm{}:hm{}",
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
        visibility.visible_map_percent(),
        visibility.hidden_map_percent(),
    ))
}

fn compose_hud_visibility_detail_status_text(hud: &HudModel) -> Option<String> {
    let summary = hud.summary.as_ref()?;
    let visibility = build_hud_visibility_panel(hud)?;
    Some(format!(
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
    ))
}

fn compose_hud_detail_status_text(hud: &HudModel) -> Option<String> {
    let hud_summary = hud.summary.as_ref()?;
    let summary = build_hud_status_panel(hud)?;
    let visibility = build_hud_visibility_panel(hud)?;
    Some(format!(
        "huddet:p={}#{}:sel={}#{}:t{}:vm{}:hm{}:ov{}:fg{}:mini=f{}:w{}+{}:a{}",
        compact_runtime_ui_text(Some(summary.player_name.as_str())),
        summary.player_name_len(),
        compact_runtime_ui_text(Some(summary.selected_block.as_str())),
        summary.selected_block_len(),
        summary.map_tile_count(),
        visibility.visible_map_percent(),
        visibility.hidden_map_percent(),
        bool_flag(hud_summary.overlay_visible),
        bool_flag(hud_summary.fog_enabled),
        optional_focus_tile_status_text(hud_summary.minimap.focus_tile),
        hud_summary.minimap.view_window.origin_label(),
        hud_summary.minimap.view_window.size_label(),
        hud_summary.minimap.view_window.tile_count(),
    ))
}

fn compose_runtime_ui_status_text(runtime_ui: &RuntimeUiObservability) -> String {
    let hud_text = &runtime_ui.hud_text;
    let toast = &runtime_ui.toast;
    let menu = &runtime_ui.menu;
    let text_input = &runtime_ui.text_input;
    let live = &runtime_ui.live;
    format!(
        "ui:hud={}/{}/{}@{}/{}:ann={}@{}:info={}@{}:toast={}/{}@{}/{}:popup={}/{}:clip{}:uri{}:choice={}/{}:tin={}@{}:{}/{}/{}#{}:n{}:e{}:live=ent={}:fx={}",
        hud_text.set_count,
        hud_text.set_reliable_count,
        hud_text.hide_count,
        compact_runtime_ui_text(hud_text.last_message.as_deref()),
        compact_runtime_ui_text(hud_text.last_reliable_message.as_deref()),
        hud_text.announce_count,
        compact_runtime_ui_text(hud_text.last_announce_message.as_deref()),
        hud_text.info_message_count,
        compact_runtime_ui_text(hud_text.last_info_message.as_deref()),
        toast.info_count,
        toast.warning_count,
        compact_runtime_ui_text(toast.last_info_message.as_deref()),
        compact_runtime_ui_text(toast.last_warning_text.as_deref()),
        toast.info_popup_count,
        toast.info_popup_reliable_count,
        toast.clipboard_count,
        toast.open_uri_count,
        menu.menu_choose_count,
        menu.text_input_result_count,
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
        optional_bool_label(panel.last_info_popup_reliable),
        compact_runtime_ui_text(panel.last_info_popup_id.as_deref()),
        compact_runtime_ui_text(panel.last_info_popup_message.as_deref()),
        panel.clipboard_count,
        compact_runtime_ui_text(panel.last_clipboard_text.as_deref()),
        panel.open_uri_count,
        compact_runtime_ui_text(panel.last_open_uri.as_deref()),
        runtime_ui_uri_scheme(panel.last_open_uri.as_deref()),
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
        optional_bool_label(panel.last_info_popup_reliable),
        runtime_ui_text_len(panel.last_info_popup_id.as_deref()),
        runtime_ui_text_len(panel.last_info_popup_message.as_deref()),
        optional_u32_label(panel.last_info_popup_duration_bits),
        optional_i32_label(panel.last_info_popup_align),
        optional_i32_label(panel.last_info_popup_top),
        optional_i32_label(panel.last_info_popup_left),
        optional_i32_label(panel.last_info_popup_bottom),
        optional_i32_label(panel.last_info_popup_right),
        panel.clipboard_count,
        runtime_ui_text_len(panel.last_clipboard_text.as_deref()),
        panel.open_uri_count,
        runtime_ui_text_len(panel.last_open_uri.as_deref()),
        runtime_ui_uri_scheme(panel.last_open_uri.as_deref()),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        runtime_ui_text_len(panel.text_input_last_message.as_deref()),
        runtime_ui_text_len(panel.text_input_last_default_text.as_deref()),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_notice_state_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_notice_state_panel(hud)?;
    let notice_text = format!(
        "{}@{}",
        runtime_dialog_notice_status_text(panel.kind),
        compact_runtime_ui_text(panel.text.as_deref())
    );
    let layers = panel.layer_labels();
    let source = layers.last().copied().unwrap_or("none");
    let active_layers = if layers.is_empty() {
        "none".to_string()
    } else {
        layers.join(">")
    };
    Some(format!(
        "notice-state:n={}:src={}:layers={}:c{}",
        notice_text,
        source,
        active_layers,
        panel.count,
    ))
}

fn compose_runtime_notice_state_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_notice_state_panel(hud)?;
    let notice_text = format!(
        "{}@{}",
        runtime_dialog_notice_status_text(panel.kind),
        compact_runtime_ui_text(panel.text.as_deref())
    );
    let layers = panel.layer_labels().join(">");
    let source = panel
        .layer_labels()
        .last()
        .copied()
        .unwrap_or("none");
    Some(format!(
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
        "menu:m{}@{}:{}/{}#{}:{}:fm{}@{}:{}/{}#{}:{}:h{}@{}:tin{}@{}:{}/{}#{}:n{}:e{}",
        panel.menu_open_count,
        optional_i32_label(panel.last_menu_open_id),
        compact_runtime_ui_text(panel.last_menu_open_title.as_deref()),
        compact_runtime_ui_text(panel.last_menu_open_message.as_deref()),
        panel.last_menu_open_option_rows,
        panel.last_menu_open_first_row_len,
        panel.follow_up_menu_open_count,
        optional_i32_label(panel.last_follow_up_menu_open_id),
        compact_runtime_ui_text(panel.last_follow_up_menu_open_title.as_deref()),
        compact_runtime_ui_text(panel.last_follow_up_menu_open_message.as_deref()),
        panel.last_follow_up_menu_open_option_rows,
        panel.last_follow_up_menu_open_first_row_len,
        panel.hide_follow_up_menu_count,
        optional_i32_label(panel.last_hide_follow_up_menu_id),
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
        optional_i32_label(panel.last_menu_open_id),
        panel.menu_title_len(),
        panel.menu_message_len(),
        panel.last_menu_open_option_rows,
        panel.last_menu_open_first_row_len,
        optional_i32_label(panel.last_follow_up_menu_open_id),
        panel.follow_up_title_len(),
        panel.follow_up_message_len(),
        panel.last_follow_up_menu_open_option_rows,
        panel.last_follow_up_menu_open_first_row_len,
        optional_i32_label(panel.last_hide_follow_up_menu_id),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        compact_runtime_ui_text(panel.text_input_last_title.as_deref()),
        panel.default_text_len(),
        optional_bool_label(panel.text_input_last_numeric),
        optional_bool_label(panel.text_input_last_allow_empty),
    ))
}

fn compose_runtime_choice_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_choice_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "choice:mc{}@{}/{}:tir{}@{}/{}",
        panel.menu_choose_count,
        optional_i32_label(panel.last_menu_choose_menu_id),
        optional_i32_label(panel.last_menu_choose_option),
        panel.text_input_result_count,
        optional_i32_label(panel.last_text_input_result_id),
        compact_runtime_ui_text(panel.last_text_input_result_text.as_deref()),
    ))
}

fn compose_runtime_choice_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_choice_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "choiced:mid{}:opt{}:rid{}:rlen{}",
        optional_i32_label(panel.last_menu_choose_menu_id),
        optional_i32_label(panel.last_menu_choose_option),
        optional_i32_label(panel.last_text_input_result_id),
        panel.text_input_result_len(),
    ))
}

fn compose_runtime_prompt_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_prompt_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let layers = panel.layer_labels().join(">");
    Some(format!(
        "prompt:k={}:a{}:d{}:l={}:m{}:fo{}:tin{}@{}:{}/{}/{}#{}:n{}:e{}",
        runtime_dialog_prompt_status_text(panel.kind),
        if panel.is_active() { 1 } else { 0 },
        panel.depth(),
        if layers.is_empty() {
            "none"
        } else {
            layers.as_str()
        },
        panel.menu_open_count,
        panel.outstanding_follow_up_count(),
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

fn compose_runtime_prompt_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_prompt_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "promptd:ma{}:fm{}:fh{}:fo{}:tin{}:id{}:t{}:m{}:d{}:n{}:e{}",
        if panel.menu_active() { 1 } else { 0 },
        panel.follow_up_menu_open_count,
        panel.hide_follow_up_menu_count,
        panel.outstanding_follow_up_count(),
        panel.text_input_open_count,
        optional_i32_label(panel.text_input_last_id),
        runtime_ui_text_len(panel.text_input_last_title.as_deref()),
        panel.prompt_message_len(),
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
        "stackdepth:p{}:n{}:c{}:m{}:h{}:d{}:g{}:t{}",
        summary.prompt_depth,
        summary.notice_depth,
        summary.chat_depth,
        summary.menu_depth(),
        summary.hud_depth(),
        summary.dialog_depth(),
        summary.active_group_count,
        summary.total_depth,
    ))
}

fn compose_runtime_dialog_stack_status_text(hud: &HudModel) -> Option<String> {
    let summary = hud.runtime_ui_stack_summary()?;
    if summary.is_empty() {
        return None;
    }
    let prompt_layers = summary.prompt_layer_labels().join(">");
    let notice_layers = summary.notice_layer_labels().join(">");
    Some(format!(
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
        if summary.chat_active { 1 } else { 0 },
        summary.server_message_count,
        summary.chat_message_count,
        optional_i32_label(summary.text_input_last_id),
        optional_i32_label(summary.last_chat_sender_entity_id),
        summary.dialog_depth(),
        summary.total_depth(),
    ))
}

fn compose_runtime_command_mode_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_command_mode_panel(hud)?;
    Some(format!(
        "cmd:act{}:sel{}@{}:bld{}@{}:rect{}:grp{}:op{}:t{}:c{}:s{}",
        if panel.active { 1 } else { 0 },
        panel.selected_unit_count,
        command_i32_status_text(&panel.selected_unit_sample),
        panel.command_building_count,
        optional_i32_label(panel.first_command_building),
        command_rect_status_text(panel.command_rect),
        command_control_groups_status_text(&panel.control_groups),
        command_control_group_operation_status_text(panel.last_control_group_operation),
        command_target_status_text(panel.last_target),
        optional_u8_label(
            panel
                .last_command_selection
                .and_then(|selection| selection.command_id)
        ),
        command_stance_status_text(panel.last_stance_selection),
    ))
}

fn compose_runtime_command_mode_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_command_mode_panel(hud)?;
    Some(format!(
        "cmdd:sample{}:grp{}:op{}:bld{}:rect{}:t{}:c{}:s{}",
        command_i32_status_text(&panel.selected_unit_sample),
        command_control_groups_status_text(&panel.control_groups),
        command_control_group_operation_status_text(panel.last_control_group_operation),
        optional_i32_label(panel.first_command_building),
        command_rect_status_text(panel.command_rect),
        command_target_status_text(panel.last_target),
        optional_u8_label(
            panel
                .last_command_selection
                .and_then(|selection| selection.command_id)
        ),
        command_stance_status_text(panel.last_stance_selection),
    ))
}

fn compose_runtime_command_group_status_lines(hud: &HudModel) -> Vec<String> {
    let Some(panel) = build_runtime_command_mode_panel(hud) else {
        return Vec::new();
    };
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
                optional_i32_label(group.first_unit_id)
            )
        })
        .collect()
}

fn command_control_group_operation_status_text(
    value: Option<crate::RuntimeCommandRecentControlGroupOperationObservability>,
) -> &'static str {
    value.map(|operation| operation.label()).unwrap_or("none")
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
        "wlabeld:set{}:rel{}:rm{}:tot{}:act{}:in{}:last{}:f{}:txt{}x{}:fs{}:z{}:p{}",
        panel.label_count,
        panel.reliable_label_count,
        panel.remove_label_count,
        panel.total_count,
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

fn compose_runtime_marker_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_marker_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "marker:cr{}:rm{}:up{}:txt{}:tex{}:f{}:last{}:ctl{}",
        panel.create_count,
        panel.remove_count,
        panel.update_count,
        panel.update_text_count,
        panel.update_texture_count,
        panel.decode_fail_count,
        optional_i32_label(panel.last_marker_id),
        compact_runtime_ui_text(panel.last_control_name.as_deref()),
    ))
}

fn compose_runtime_marker_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_marker_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "markerd:tot{}:mut{}:txt{}:tex{}:f{}:last{}:c{}",
        panel.total_count(),
        panel.mutate_count(),
        panel.update_text_count,
        panel.update_texture_count,
        panel.decode_fail_count,
        optional_i32_label(panel.last_marker_id),
        panel.control_name_len(),
    ))
}

fn compose_runtime_kick_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_kick_panel(hud)?;
    Some(format!(
        "kick:{}",
        compose_runtime_kick_panel_status_text(&panel)
    ))
}

fn compose_runtime_bootstrap_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_bootstrap_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(panel.summary_label())
}

fn compose_runtime_bootstrap_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_bootstrap_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(panel.detail_label())
}

fn compose_runtime_resource_delta_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.resource_delta.is_empty() {
        return None;
    }
    Some(compose_runtime_resource_delta_panel_status_text(
        &panel.resource_delta,
    ))
}

fn compose_runtime_resource_delta_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.resource_delta.is_empty() {
        return None;
    }
    Some(compose_runtime_resource_delta_detail_panel_status_text(
        &panel.resource_delta,
    ))
}

fn compose_runtime_session_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let bootstrap_text = compose_runtime_bootstrap_status_text(hud);
    let mut segments = Vec::new();
    if let Some(bootstrap_text) = bootstrap_text {
        segments.push(format!("bootstrap={bootstrap_text}"));
    }
    if let Some(core_binding_text) = compose_runtime_core_binding_panel_status_text(hud) {
        segments.push(format!("cb={core_binding_text}"));
    }
    segments.push(format!(
        "rd={}",
        compose_runtime_resource_delta_panel_status_text(&panel.resource_delta)
    ));
    segments.push(format!(
        "k={}",
        compose_runtime_kick_panel_status_text(&panel.kick)
    ));
    segments.push(format!(
        "l={}",
        compose_runtime_loading_panel_status_text(&panel.loading)
    ));
    segments.push(format!(
        "r={}",
        compose_runtime_reconnect_panel_status_text(&panel.reconnect)
    ));
    Some(format!("sess:{}", segments.join(";")))
}

fn compose_runtime_session_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_session_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    if let Some(bootstrap_text) = compose_runtime_bootstrap_detail_status_text(hud) {
        segments.push(format!("bootstrap({bootstrap_text})"));
    }
    if let Some(core_binding_text) = compose_runtime_core_binding_detail_status_text(hud) {
        segments.push(format!("cb({core_binding_text})"));
    }
    segments.push(format!(
        "rd({})",
        compose_runtime_resource_delta_detail_panel_status_text(&panel.resource_delta)
    ));
    segments.push(format!(
        "k({})",
        compose_runtime_kick_detail_panel_status_text(&panel.kick)
    ));
    segments.push(format!(
        "l({})",
        compose_runtime_loading_detail_panel_status_text(&panel.loading)
    ));
    segments.push(format!(
        "r({})",
        compose_runtime_reconnect_detail_panel_status_text(&panel.reconnect)
    ));
    Some(format!("sessd:{}", segments.join(":")))
}

fn compose_runtime_resource_delta_panel_status_text(
    resource_delta: &crate::panel_model::RuntimeResourceDeltaPanelModel,
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
        optional_i16_label(resource_delta.last_item_id),
        optional_i32_label(resource_delta.last_amount),
        optional_i32_label(resource_delta.last_build_pos),
        command_unit_ref_status_text(resource_delta.last_unit),
        optional_i32_label(resource_delta.last_to_entity_id),
        resource_delta.build_count,
        resource_delta.build_stack_count,
        resource_delta.entity_count,
        resource_delta.authoritative_build_update_count,
        resource_delta.delta_apply_count,
        resource_delta.delta_skip_count,
        resource_delta.delta_conflict_count,
        optional_i32_label(resource_delta.last_changed_build_pos),
        optional_i32_label(resource_delta.last_changed_entity_id),
        optional_i16_label(resource_delta.last_changed_item_id),
        optional_i32_label(resource_delta.last_changed_amount),
    )
}

fn compose_runtime_resource_delta_detail_panel_status_text(
    resource_delta: &crate::panel_model::RuntimeResourceDeltaPanelModel,
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
        optional_i16_label(resource_delta.last_item_id),
        optional_i32_label(resource_delta.last_amount),
        optional_i32_label(resource_delta.last_build_pos),
        command_unit_ref_status_text(resource_delta.last_unit),
        optional_i32_label(resource_delta.last_to_entity_id),
        resource_delta.build_count,
        resource_delta.build_stack_count,
        resource_delta.entity_count,
        resource_delta.authoritative_build_update_count,
        resource_delta.delta_apply_count,
        resource_delta.delta_skip_count,
        resource_delta.delta_conflict_count,
        optional_i32_label(resource_delta.last_changed_build_pos),
        optional_i32_label(resource_delta.last_changed_entity_id),
        optional_i16_label(resource_delta.last_changed_item_id),
        optional_i32_label(resource_delta.last_changed_amount),
    )
}

fn compose_runtime_loading_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    Some(format!(
        "loading:{}",
        compose_runtime_loading_panel_status_text(&panel)
    ))
}

fn compose_runtime_kick_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_kick_panel(hud)?;
    (!panel.is_empty()).then(|| compose_runtime_kick_detail_panel_status_text(&panel))
}

fn compose_runtime_loading_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    (!panel.is_empty()).then(|| compose_runtime_loading_detail_panel_status_text(&panel))
}

fn compose_runtime_world_reload_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_loading_panel(hud)?;
    (!panel.is_empty())
        .then(|| runtime_world_reload_panel_status_text(panel.last_world_reload.as_ref()))
}

fn compose_runtime_core_binding_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_core_binding_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "core:{}:a{}@{}:m{}@{}",
        panel.kind_label(),
        panel.ambiguous_team_count,
        team_u8_status_text(&panel.ambiguous_team_sample),
        panel.missing_team_count,
        team_u8_status_text(&panel.missing_team_sample),
    ))
}

fn compose_runtime_core_binding_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_core_binding_panel(hud)?;
    if panel.is_empty() {
        return None;
    }
    Some(format!(
        "cored:{}:a{}@{}:m{}@{}",
        panel.kind_label(),
        panel.ambiguous_team_count,
        team_u8_status_text(&panel.ambiguous_team_sample),
        panel.missing_team_count,
        team_u8_status_text(&panel.missing_team_sample),
    ))
}

fn compose_runtime_reconnect_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_reconnect_panel(hud)?;
    Some(format!(
        "reconnect:{}",
        compose_runtime_reconnect_panel_status_text(&panel)
    ))
}

fn compose_runtime_reconnect_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_reconnect_panel(hud)?;
    (!panel.is_empty()).then(|| compose_runtime_reconnect_detail_panel_status_text(&panel))
}

fn compose_runtime_live_entity_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(format!(
        "liveent:{}",
        compose_live_entity_panel_status_text(&panel)
    ))
}

fn compose_runtime_live_entity_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_runtime_live_entity_panel(hud)?;
    Some(format!("liveentd:{}", panel.detail_label()))
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
        "livefxd:hint{}:src{}:pos{}:ttl{}:data{}:arel{}:ctr{}:rel{}:bind{}",
        panel.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_text(panel.display_position_source()),
        world_position_status_text(panel.display_position()),
        live_effect_ttl_status_text(panel.display_overlay_ttl()),
        live_effect_data_shape_status_text(panel.last_data_len, panel.last_data_type_tag),
        live_effect_reliable_flag_status_text(panel.active_reliable),
        compact_runtime_ui_text(panel.display_contract_name()),
        compact_runtime_ui_text(panel.display_reliable_contract_name()),
        panel.binding_detail.as_deref().unwrap_or("none"),
    ))
}

fn compose_build_ui_status_text(build_ui: &BuildUiObservability) -> String {
    format!(
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
    )
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
        "minivis:ov{}:fg{}:k{}p{}:v{}p{}m{}:h{}p{}m{}:u{}p{}:d{}@{}p{}:w{}@{}p{}:o{}@{}p{}",
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
        panel.tracked_object_count,
        panel.map_tile_count,
        panel.map_object_density_percent(),
        panel.window_tracked_object_count,
        panel.window_tile_count,
        panel.window_object_density_percent(),
        panel.outside_window_count,
        panel.tracked_object_count,
        panel.outside_object_percent(),
    ))
}

fn compose_visibility_minimap_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let visibility = build_hud_visibility_panel(hud)?;
    let minimap = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "overlay={} fog={} known={}({}%) vis={}({}%/{}%) hid={}({}%/{}%) map={}x{} window={}:{}->{}:{} size={}x{} cover={}/{}({}%) focus={} in-window={}",
        bool_flag(visibility.overlay_visible),
        bool_flag(visibility.fog_enabled),
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
        optional_focus_tile_status_text(minimap.focus_tile),
        optional_bool_label(minimap.focus_in_window),
    ))
}

fn compose_minimap_flow_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_user_flow_panel(scene, hud, window)?;
    Some(format!(
        "miniflow:n={}:f={}:p={}:v={}:c={}:t={}:o{}",
        panel.next_action,
        panel.focus_state.label(),
        panel.pan_label(),
        panel.visibility_label(),
        panel.coverage_label(),
        panel.target_kind.label(),
        panel.overlay_target_count,
    ))
}

fn compose_minimap_flow_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_user_flow_panel(scene, hud, window)?;
    Some(panel.detail_label())
}

fn compose_minimap_visibility_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let minimap = build_minimap_panel(scene, hud, window)?;
    Some(format!(
        "minivisd:v={}:c={}:md{}:wd{}:od{}:vp={}",
        minimap.visibility_label(),
        minimap.coverage_label(),
        minimap.map_object_density_percent(),
        minimap.window_object_density_percent(),
        minimap.outside_object_percent(),
        minimap.viewport_band(),
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
    let text = format!(
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
    Some(text)
}

fn compose_minimap_kind_detail_status_text(scene: &RenderModel, hud: &HudModel) -> Option<String> {
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
    semantic_detail_text(&panel.detail_counts)
}

fn compose_minimap_window_kind_status_text(scene: &RenderModel, hud: &HudModel) -> Option<String> {
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
    Some(compose_minimap_window_kind_distribution_status_text(&panel))
}

fn compose_minimap_window_distribution_line_status_text(
    scene: &RenderModel,
    hud: &HudModel,
) -> Option<String> {
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
    Some(compose_minimap_window_distribution_status_text(&panel))
}

fn compose_minimap_legend_status_text(hud: &HudModel) -> Option<String> {
    let summary = hud.summary.as_ref()?;
    Some(format!(
        "legend:pl@/mkM/pnP/bk#/rtR/tr./uk?:vis={}:ov{}:fg{}",
        summary.visibility_label(),
        bool_flag(summary.overlay_visible),
        bool_flag(summary.fog_enabled),
    ))
}

fn compose_minimap_detail_status_lines(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Vec<String> {
    let Some(panel) = build_minimap_panel(scene, hud, window) else {
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
    lines.push(compose_minimap_density_visibility_status_text(&panel));
    lines
}

fn compose_minimap_density_visibility_status_text(panel: &MinimapPanelModel) -> String {
    format!(
        "minidv:ov{}:fg{}:cov{}:mapd{}:wind{}:out{}",
        bool_flag(panel.overlay_visible),
        bool_flag(panel.fog_enabled),
        panel.window_coverage_percent,
        panel.map_object_density_percent(),
        panel.window_object_density_percent(),
        panel.outside_object_percent(),
    )
}

fn compose_minimap_window_distribution_status_text(panel: &MinimapPanelModel) -> String {
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

fn compose_minimap_window_kind_distribution_status_text(panel: &MinimapPanelModel) -> String {
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

fn compose_minimap_edge_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_panel(scene, hud, window)?;
    Some(compose_minimap_edge_summary_status_text(&panel))
}

fn compose_minimap_edge_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_minimap_panel(scene, hud, window)?;
    Some(panel.edge_detail_label())
}

fn compose_minimap_edge_summary_status_text(panel: &MinimapPanelModel) -> String {
    format!(
        "miniedge:f={}@{}:dr={},{}:cl={}:out={}/{}:win={}/{}",
        optional_focus_tile_status_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        optional_signed_tile_status_text(panel.focus_offset_x),
        optional_signed_tile_status_text(panel.focus_offset_y),
        minimap_clamp_status_text(panel),
        panel.outside_window_count,
        panel.tracked_object_count,
        panel.window_tracked_object_count,
        panel.tracked_object_count,
    )
}

fn minimap_clamp_status_text(panel: &MinimapPanelModel) -> String {
    let mut clamp = String::new();
    if panel.window_clamped_left {
        clamp.push('l');
    }
    if panel.window_clamped_top {
        clamp.push('t');
    }
    if panel.window_clamped_right {
        clamp.push('r');
    }
    if panel.window_clamped_bottom {
        clamp.push('b');
    }
    if clamp.is_empty() {
        "-".to_string()
    } else {
        clamp
    }
}

fn compose_build_config_panel_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, WINDOW_BUILD_CONFIG_ENTRY_CAP)?;
    let (authority_text, pending_match_text, authority_source_text, authority_block_text) =
        build_build_interaction_panel(hud)
            .map(|panel| {
                (
                    build_interaction_authority_status_text(panel.authority_state),
                    build_config_pending_match_status_text(panel.authority_pending_match),
                    build_config_rollback_source_status_text(panel.authority_source),
                    compact_runtime_ui_text(panel.authority_block_name.as_deref()),
                )
            })
            .unwrap_or(("none", "none", "none", "none".to_string()));
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
        "cfgpanel:sel{}:r{}:m{}:p{}/{}:hist{}/{}:o{}:h={}:align={}:auth={}:pm={}:src={}:b={}:fam{}/{}:more{}:t{}@{}",
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
        authority_text,
        pending_match_text,
        authority_source_text,
        authority_block_text,
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

fn compose_build_config_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, WINDOW_BUILD_CONFIG_ENTRY_CAP)?;
    Some(panel.detail_label())
}

fn compose_build_config_entry_status_lines(hud: &HudModel) -> Vec<String> {
    let Some(entries) = build_build_config_entry_breakdown(hud) else {
        return Vec::new();
    };

    let entry_count = entries.len();

    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                "{}/{}:{}#{}@{}",
                index + 1,
                entry_count,
                compact_runtime_ui_text(Some(entry.family.as_str())),
                entry.tracked_count,
                compact_build_inspector_text(
                    entry.sample.as_str(),
                    WINDOW_BUILD_CONFIG_ENTRY_SAMPLE_LIMIT,
                ),
            )
        })
        .collect()
}

fn compose_build_config_more_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, WINDOW_BUILD_CONFIG_ENTRY_CAP)?;
    (panel.truncated_family_count > 0).then(|| format!("cfgmore:+{}", panel.truncated_family_count))
}

fn compose_build_config_rollback_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, WINDOW_BUILD_CONFIG_ENTRY_CAP)?;
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

fn compose_build_config_rollback_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_config_panel(hud, WINDOW_BUILD_CONFIG_ENTRY_CAP)?;
    Some(panel.rollback_strip.detail_label())
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

fn compose_build_interaction_detail_status_text(hud: &HudModel) -> Option<String> {
    let panel = build_build_interaction_panel(hud)?;
    Some(panel.detail_label())
}

fn compose_build_ui_queue_status_text(hud: &HudModel) -> Option<String> {
    let build_ui = hud.build_ui.as_ref()?;
    Some(format!(
        "bqueue:q{}:i{}:f{}:r{}:o{}:h={}",
        build_ui.queued_count,
        build_ui.inflight_count,
        build_ui.finished_count,
        build_ui.removed_count,
        build_ui.orphan_authoritative_count,
        build_queue_head_status_text(build_ui.head.as_ref()),
    ))
}

fn compose_build_ui_queue_detail_status_text(hud: &HudModel) -> Option<String> {
    let build_ui = hud.build_ui.as_ref()?;
    Some(format!(
        "q={} i={} f={} r={} o={} h={}",
        build_ui.queued_count,
        build_ui.inflight_count,
        build_ui.finished_count,
        build_ui.removed_count,
        build_ui.orphan_authoritative_count,
        build_queue_head_status_text(build_ui.head.as_ref()),
    ))
}

fn compose_build_minimap_aux_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    let window_tile_count = window.width.saturating_mul(window.height);
    Some(format!(
        "preb:m={}:s={}:q={}:r{}:c={}/{}@{}:a={}:p={}:h={}:t={}:x={}:b={}:f={}@{}:v{}:u{}:w{}:d{}:o{}:rt{}:rs{}",
        build_interaction_mode_status_text(panel.mode),
        build_interaction_selection_status_text(panel.selection_state),
        build_interaction_queue_status_text(panel.queue_state),
        if panel.place_ready { 1 } else { 0 },
        panel.config_family_count,
        panel.config_sample_count,
        compact_runtime_ui_text(panel.top_config_family.as_deref()),
        build_interaction_authority_status_text(panel.authority_state),
        build_config_pending_match_status_text(panel.authority_pending_match),
        optional_build_tile_status_text(panel.head_tile),
        build_config_tile_status_text(panel.authority_tile),
        build_config_rollback_source_status_text(panel.authority_source),
        compact_runtime_ui_text(panel.authority_block_name.as_deref()),
        optional_focus_tile_status_text(panel.focus_tile),
        optional_bool_label(panel.focus_in_window),
        panel.visible_map_percent,
        panel.unknown_tile_percent,
        panel.window_coverage_percent,
        panel.window_object_density_percent(window_tile_count),
        panel.tracked_object_count,
        panel.runtime_count,
        panel.runtime_share_percent(),
    ))
}

fn compose_build_minimap_diag_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(format!(
        "bmdiag:n={}:p={}:a={}:f={}:c={}:v={}",
        panel.next_action_label(),
        panel.head_authority_pair_label(),
        panel.focus_anchor_label(),
        panel.focus_state_label(),
        panel.window_coverage_label(),
        panel.map_visibility_label(),
    ))
}

fn compose_build_minimap_flow_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(format!(
        "bflow:n={}:s={}:q={}:r{}:f={}:c={}:rt{}",
        panel.next_action_label(),
        build_interaction_selection_status_text(panel.selection_state),
        build_interaction_queue_status_text(panel.queue_state),
        if panel.place_ready { 1 } else { 0 },
        panel.focus_state_label(),
        panel.window_coverage_label(),
        panel.runtime_share_percent(),
    ))
}

fn compose_build_minimap_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_minimap_assist_panel(scene, hud, window)?;
    let window_tile_count = window.width.saturating_mul(window.height);
    Some(format!(
        "bmdetail:n={}:pair={}:a={}:f={}:v={}:c={}:scope={}:auth={}:pm={}:src={}:h={}:b={}:rt{}:od{}",
        panel.next_action_label(),
        panel.head_authority_pair_label(),
        panel.focus_anchor_label(),
        panel.focus_state_label(),
        panel.map_visibility_label(),
        panel.window_coverage_label(),
        panel.config_scope_label(),
        build_interaction_authority_status_text(panel.authority_state),
        build_config_pending_match_status_text(panel.authority_pending_match),
        build_config_rollback_source_status_text(panel.authority_source),
        optional_build_tile_status_text(panel.head_tile),
        compact_runtime_ui_text(panel.authority_block_name.as_deref()),
        panel.runtime_share_percent(),
        panel.window_object_density_percent(window_tile_count),
    ))
}

fn compose_build_flow_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(format!(
        "cfgflow:n={}:m={}:f={}:p={}:t={}:scope={}:h={}:a={}:pm={}",
        panel.next_action,
        panel.minimap_next_action,
        panel.focus_state.label(),
        panel.pan_label(),
        panel.target_kind.label(),
        panel.config_scope,
        optional_build_tile_status_text(panel.head_tile),
        build_interaction_authority_status_text(panel.authority_state),
        build_config_pending_match_status_text(panel.authority_pending_match),
    ))
}

fn compose_build_flow_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(panel.detail_label())
}

fn compose_build_flow_summary_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(panel.summary_label())
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
        "cfgroute:n={}:m={}:b{}@{}:r{}@{}",
        panel.next_action,
        panel.minimap_next_action,
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

fn compose_build_route_detail_status_text(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let panel = build_build_user_flow_panel(scene, hud, window)?;
    Some(panel.route_detail_label())
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
                compact_build_inspector_text(
                    entry.sample.as_str(),
                    WINDOW_BUILD_INSPECTOR_SAMPLE_LIMIT,
                ),
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

fn optional_build_tile_status_text(value: Option<(i32, i32)>) -> String {
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
        "{}/{}:ov{}@{}:u{}:d{}:k{}:c{}/{}:bind{}:r{}:h{}:p{}@{}:ttl{}",
        effect.effect_count,
        effect.spawn_effect_count,
        effect.active_overlay_count,
        optional_i16_label(effect.display_effect_id()),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        live_effect_data_shape_status_text(effect.last_data_len, effect.last_data_type_tag),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.display_contract_name()),
        compact_runtime_ui_text(effect.display_reliable_contract_name()),
        effect.binding_label.as_deref().unwrap_or("none"),
        live_effect_reliable_flag_status_text(effect.active_reliable),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_text(effect.display_position_source()),
        world_position_status_text(effect.display_position()),
        live_effect_ttl_status_text(effect.display_overlay_ttl()),
    )
}

fn compose_live_effect_panel_status_text(
    effect: &crate::panel_model::RuntimeLiveEffectPanelModel,
) -> String {
    format!(
        "{}/{}:ov{}@{}:u{}:d{}:k{}:c{}/{}:bind{}:r{}:h{}:p{}@{}:ttl{}",
        effect.effect_count,
        effect.spawn_effect_count,
        effect.active_overlay_count,
        optional_i16_label(effect.display_effect_id()),
        optional_i16_label(effect.last_spawn_effect_unit_type_id),
        live_effect_data_shape_status_text(effect.last_data_len, effect.last_data_type_tag),
        compact_runtime_ui_text(effect.last_kind.as_deref()),
        compact_runtime_ui_text(effect.display_contract_name()),
        compact_runtime_ui_text(effect.display_reliable_contract_name()),
        effect.binding_label.as_deref().unwrap_or("none"),
        live_effect_reliable_flag_status_text(effect.active_reliable),
        effect.last_business_hint.as_deref().unwrap_or("none"),
        live_effect_position_source_status_text(effect.display_position_source()),
        world_position_status_text(effect.display_position()),
        live_effect_ttl_status_text(effect.display_overlay_ttl()),
    )
}

fn live_effect_ttl_status_text(ttl: Option<(u8, u8)>) -> String {
    match ttl {
        Some((remaining, total)) => format!("{remaining}/{total}"),
        None => "none".to_string(),
    }
}

fn live_effect_data_shape_status_text(
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

fn live_effect_reliable_flag_status_text(flag: Option<bool>) -> &'static str {
    match flag {
        Some(true) => "1",
        Some(false) => "0",
        None => "?",
    }
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

fn compose_runtime_kick_detail_panel_status_text(
    kick: &crate::panel_model::RuntimeKickPanelModel,
) -> String {
    format!(
        "kickd:r{}:o{}:c{}:h{}",
        runtime_ui_text_len(kick.reason_text.as_deref()),
        optional_i32_label(kick.reason_ordinal),
        runtime_ui_text_len(kick.hint_category.as_deref()),
        runtime_ui_text_len(kick.hint_text.as_deref()),
    )
}

fn compose_runtime_loading_detail_panel_status_text(
    loading: &crate::panel_model::RuntimeLoadingPanelModel,
) -> String {
    format!(
        "loadingd:rdy{}@{}:to{}/{}/{}:{}@{}:rs{}/{}/{}/{}:{}:{}",
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

fn compose_runtime_reconnect_detail_panel_status_text(
    reconnect: &crate::panel_model::RuntimeReconnectPanelModel,
) -> String {
    format!(
        "reconnectd:{}#{}:{}:r{}@{}:h{}:rd{}@{}:{}",
        runtime_reconnect_phase_status_text(reconnect.phase),
        reconnect.phase_transition_count,
        runtime_reconnect_reason_kind_status_text(reconnect.reason_kind),
        runtime_ui_text_len(reconnect.reason_text.as_deref()),
        optional_i32_label(reconnect.reason_ordinal),
        runtime_ui_text_len(reconnect.hint_text.as_deref()),
        reconnect.redirect_count,
        compact_runtime_ui_text(reconnect.last_redirect_ip.as_deref()),
        optional_i32_label(reconnect.last_redirect_port),
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

fn compose_runtime_world_reload_detail_status_text(hud: &HudModel) -> Option<String> {
    let loading = build_runtime_loading_panel(hud)?;
    let world_reload = loading.last_world_reload.as_ref()?;
    Some(runtime_world_reload_detail_status_text(world_reload))
}

fn runtime_world_reload_detail_status_text(
    world_reload: &crate::panel_model::RuntimeWorldReloadPanelModel,
) -> String {
    format!(
        "reloadd:lw{}:cl{}:rd{}:cc{}:p{}:d{}:r{}",
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
    )
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

    Some(format!("overlay:{}", summary.family_and_detail_text()))
}

fn compose_overlay_detail_status_text(scene: &RenderModel) -> Option<String> {
    let summary = scene.semantic_summary();
    summary.detail_text()
}

fn compose_render_pipeline_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let summary = render_pipeline_summary(scene, window)?;
    let window = summary.window?;
    let span_text = summary
        .layer_span
        .map(|(min, max)| format!("{min}..{max}"))
        .unwrap_or_else(|| "none".to_string());
    let focus_text = summary
        .focus_tile
        .map(|(x, y)| format!("{x},{y}"))
        .unwrap_or_else(|| "none".to_string());

    Some(format!(
        "pipe:tot{}:vis{}:clip{}:ly{}:span{}:f{}:w{},{}+{}x{}:{}",
        summary.total_object_count,
        summary.visible_object_count,
        summary.clipped_object_count,
        summary.layers.len(),
        span_text,
        focus_text,
        window.origin_x,
        window.origin_y,
        window.width,
        window.height,
        summary.visible_semantics.family_text(),
    ))
}

fn compose_render_pipeline_detail_status_text(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<String> {
    let summary = render_pipeline_summary(scene, window)?;
    summary.visible_semantics.detail_text()
}

fn compose_render_layer_status_lines(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Vec<String> {
    let Some(summary) = render_pipeline_summary(scene, window) else {
        return Vec::new();
    };

    let layer_count = summary.layers.len();
    summary
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            format!(
                "lay:{}/{}:l{}:o{}@pl{}:mk{}:pn{}:bk{}:rt{}:tr{}:uk{}",
                index + 1,
                layer_count,
                layer.layer,
                layer.object_count,
                layer.player_count,
                layer.marker_count,
                layer.plan_count,
                layer.block_count,
                layer.runtime_count,
                layer.terrain_count,
                layer.unknown_count,
            )
        })
        .collect()
}

fn compose_render_layer_detail_status_lines(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Vec<String> {
    let Some(summary) = render_pipeline_summary(scene, window) else {
        return Vec::new();
    };

    let layer_count = summary.layers.len();
    summary
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            let detail_text = layer.detail_text()?;
            Some(format!(
                "layd:{}/{}:l{}:detail={}",
                index + 1,
                layer_count,
                layer.layer,
                detail_text,
            ))
        })
        .collect()
}

fn render_pipeline_summary(
    scene: &RenderModel,
    window: PresenterViewWindow,
) -> Option<crate::render_model::RenderPipelineSummary> {
    if scene.objects.is_empty() {
        return None;
    }

    Some(scene.pipeline_summary_for_window(
        TILE_SIZE,
        crate::RenderViewWindow {
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            width: window.width,
            height: window.height,
        },
    ))
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

fn compose_build_strip_queue_text(
    panel: &crate::panel_model::BuildInteractionPanelModel,
) -> String {
    if let Some(head) = panel.head.as_ref() {
        build_queue_head_stage_status_text(head.stage, panel.pending_count)
    } else {
        format!(
            "{}/p{}",
            build_interaction_queue_status_text(panel.queue_state),
            panel.pending_count
        )
    }
}

fn compose_build_strip_queue_fallback_text(build_ui: &BuildUiObservability) -> String {
    if let Some(head) = build_ui.head.as_ref() {
        build_queue_head_stage_status_text(head.stage, build_ui.queued_count)
    } else {
        format!("queued@{}", build_ui.queued_count)
    }
}

fn build_queue_head_stage_status_text(stage: BuildQueueHeadStage, pending_count: usize) -> String {
    let stage_text = match stage {
        BuildQueueHeadStage::Queued => "queued",
        BuildQueueHeadStage::InFlight => "flight",
        BuildQueueHeadStage::Finished => "finish",
        BuildQueueHeadStage::Removed => "remove",
    };
    format!("{stage_text}@{pending_count}")
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

fn runtime_ui_uri_scheme(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .and_then(|uri| uri.split_once(':').map(|(scheme, _)| scheme.trim()))
        .filter(|scheme| !scheme.is_empty())
        .map(|scheme| compact_runtime_ui_text(Some(scheme)))
        .unwrap_or_else(|| "none".to_string())
}

fn runtime_ui_notice_panel_is_empty(panel: &RuntimeUiNoticePanelModel) -> bool {
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

fn team_u8_status_text(values: &[u8]) -> String {
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
        Some(crate::RuntimeLiveEffectPositionSource::ActiveOverlay) => "active",
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
    let unit_target = command_unit_ref_status_text(value.unit_target);
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

fn command_unit_ref_status_text(
    value: Option<crate::RuntimeCommandUnitRefObservability>,
) -> String {
    value
        .map(|unit| format!("{}:{}", unit.kind, unit.value))
        .unwrap_or_else(|| "none".to_string())
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

fn scaled_surface_metrics(
    frame: &WindowFrame,
    tile_pixels: usize,
) -> Option<(usize, usize, usize)> {
    let tile_pixels = tile_pixels.max(1);
    let width = frame.width.max(1);
    let height = frame.height.max(1);
    let surface_width = width.checked_mul(tile_pixels)?;
    let surface_height = height.checked_mul(tile_pixels)?;
    let pixel_count = surface_width.checked_mul(surface_height)?;
    Some((surface_width, surface_height, pixel_count))
}

fn scale_frame_pixels(frame: &WindowFrame, tile_pixels: usize) -> Vec<u32> {
    let Some((surface_width, surface_height, pixel_count)) =
        scaled_surface_metrics(frame, tile_pixels)
    else {
        return Vec::new();
    };
    let tile_pixels = tile_pixels.max(1);
    let width = frame.width.max(1);
    let height = frame.height.max(1);
    let mut pixels = vec![COLOR_EMPTY; pixel_count];

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

    overlay_window_hud(frame, &mut pixels, surface_width, surface_height);
    overlay_window_minimap_inset(frame, &mut pixels, surface_width, surface_height);
    pixels
}

fn overlay_window_minimap_inset(
    frame: &WindowFrame,
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
) {
    let Some(inset) = frame.minimap_inset.as_ref() else {
        return;
    };
    if inset.map_width == 0 || inset.map_height == 0 || surface_width == 0 || surface_height == 0 {
        return;
    }

    let top_reserved = if frame
        .wave_text
        .as_deref()
        .is_some_and(|text| !text.is_empty())
    {
        window_hud_bar_height(1).saturating_add(WINDOW_MINIMAP_INSET_PADDING)
    } else {
        WINDOW_MINIMAP_INSET_PADDING
    };
    let max_inner_width = surface_width.saturating_div(5).min(96);
    let max_inner_height = surface_height.saturating_div(4).min(72);
    let Some((map_pixel_width, map_pixel_height)) = fit_window_minimap_size(
        inset.map_width,
        inset.map_height,
        max_inner_width,
        max_inner_height,
    ) else {
        return;
    };

    let outer_width = map_pixel_width.saturating_add(WINDOW_MINIMAP_INSET_BORDER_WIDTH * 2);
    let outer_height = map_pixel_height.saturating_add(WINDOW_MINIMAP_INSET_BORDER_WIDTH * 2);
    if outer_width.saturating_add(WINDOW_MINIMAP_INSET_PADDING) > surface_width
        || top_reserved.saturating_add(outer_height) > surface_height
    {
        return;
    }

    let start_x = surface_width
        .saturating_sub(outer_width)
        .saturating_sub(WINDOW_MINIMAP_INSET_PADDING);
    let start_y = top_reserved;
    let map_start_x = start_x.saturating_add(WINDOW_MINIMAP_INSET_BORDER_WIDTH);
    let map_start_y = start_y.saturating_add(WINDOW_MINIMAP_INSET_BORDER_WIDTH);

    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        start_x,
        start_y,
        outer_width,
        outer_height,
        COLOR_MINIMAP_INSET_BORDER,
    );
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        map_start_x,
        map_start_y,
        map_pixel_width,
        map_pixel_height,
        window_minimap_background_color(
            inset.window_coverage_percent,
            inset.map_object_density_percent,
            inset.window_object_density_percent,
            inset.outside_object_percent,
        ),
    );

    draw_window_minimap_viewport(
        inset,
        pixels,
        surface_width,
        surface_height,
        map_start_x,
        map_start_y,
        map_pixel_width,
        map_pixel_height,
    );
    for rect in &inset.command_rects {
        draw_window_minimap_command_rect(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            *rect,
        );
    }
    for rect in &inset.runtime_break_rects {
        draw_window_minimap_break_rect(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            *rect,
        );
    }
    for rect in &inset.unit_assembler_rects {
        draw_window_minimap_unit_assembler_rect(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            *rect,
        );
    }
    for &unit_assembler_tile in &inset.unit_assembler_tiles {
        draw_window_minimap_unit_assembler(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            unit_assembler_tile,
        );
    }
    for &tile_action_tile in &inset.tile_action_tiles {
        draw_window_minimap_tile_action(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            tile_action_tile,
        );
    }
    for overlay_tile in &inset.runtime_overlay_tiles {
        draw_window_minimap_runtime_overlay(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            *overlay_tile,
        );
    }
    for &world_label_tile in &inset.world_label_tiles {
        draw_window_minimap_world_label(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            world_label_tile,
        );
    }
    for &command_tile in &inset.command_tiles {
        draw_window_minimap_command(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            command_tile,
        );
    }
    if let Some(player_tile) = inset.player_tile {
        draw_window_minimap_player(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            player_tile,
            window_minimap_tile_in_window(inset.window, player_tile),
        );
    }
    if let Some(focus_tile) = inset.focus_tile {
        draw_window_minimap_focus(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            focus_tile,
            inset.focus_in_window,
        );
    }
    if let Some(ping_tile) = inset.ping_tile {
        draw_window_minimap_ping(
            pixels,
            surface_width,
            surface_height,
            map_start_x,
            map_start_y,
            map_pixel_width,
            map_pixel_height,
            inset.map_width,
            inset.map_height,
            ping_tile,
        );
    }
}

fn fit_window_minimap_size(
    map_width: usize,
    map_height: usize,
    max_width: usize,
    max_height: usize,
) -> Option<(usize, usize)> {
    if map_width == 0 || map_height == 0 || max_width < 12 || max_height < 12 {
        return None;
    }

    let scale = ((max_width as f32) / (map_width as f32))
        .min((max_height as f32) / (map_height as f32))
        .min(4.0);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }

    Some((
        ((map_width as f32) * scale).floor().max(1.0) as usize,
        ((map_height as f32) * scale).floor().max(1.0) as usize,
    ))
}

fn draw_window_minimap_viewport(
    inset: &WindowMinimapInset,
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
) {
    let window_width = inset
        .window
        .width
        .min(inset.map_width.saturating_sub(inset.window.origin_x));
    let window_height = inset
        .window
        .height
        .min(inset.map_height.saturating_sub(inset.window.origin_y));
    if window_width == 0 || window_height == 0 {
        return;
    }

    let rect_x = map_start_x.saturating_add(project_window_minimap_x(
        inset.window.origin_x,
        inset.map_width,
        map_pixel_width,
    ));
    let rect_width = project_window_minimap_span(window_width, inset.map_width, map_pixel_width);
    let rect_top = inset
        .map_height
        .saturating_sub(inset.window.origin_y.saturating_add(window_height));
    let rect_y = map_start_y.saturating_add(project_window_minimap_y(
        rect_top,
        inset.map_height,
        map_pixel_height,
    ));
    let rect_height =
        project_window_minimap_span(window_height, inset.map_height, map_pixel_height);

    draw_window_minimap_outline(
        pixels,
        surface_width,
        surface_height,
        rect_x,
        rect_y,
        rect_width,
        rect_height,
        window_minimap_viewport_color(
            inset.window_coverage_percent,
            inset.map_object_density_percent,
            inset.window_object_density_percent,
            inset.outside_object_percent,
        ),
    );
}

fn window_minimap_viewport_band(
    window_coverage_percent: usize,
    map_object_density_percent: usize,
    window_object_density_percent: usize,
    outside_object_percent: usize,
) -> &'static str {
    crate::panel_model::minimap_viewport_band(
        window_coverage_percent,
        map_object_density_percent,
        window_object_density_percent,
        outside_object_percent,
    )
}

fn window_minimap_viewport_color(
    window_coverage_percent: usize,
    map_object_density_percent: usize,
    window_object_density_percent: usize,
    outside_object_percent: usize,
) -> u32 {
    match window_minimap_viewport_band(
        window_coverage_percent,
        map_object_density_percent,
        window_object_density_percent,
        outside_object_percent,
    ) {
        "warn" => COLOR_MINIMAP_INSET_VIEWPORT_WARN,
        "partial" => COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL,
        _ => COLOR_MINIMAP_INSET_VIEWPORT,
    }
}

fn window_minimap_background_color(
    window_coverage_percent: usize,
    map_object_density_percent: usize,
    window_object_density_percent: usize,
    outside_object_percent: usize,
) -> u32 {
    match window_minimap_viewport_band(
        window_coverage_percent,
        map_object_density_percent,
        window_object_density_percent,
        outside_object_percent,
    ) {
        "warn" => COLOR_MINIMAP_INSET_BACKGROUND_WARN,
        "partial" => COLOR_MINIMAP_INSET_BACKGROUND_PARTIAL,
        _ => COLOR_MINIMAP_INSET_BACKGROUND,
    }
}

fn draw_window_minimap_player(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
    player_in_window: bool,
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let radius = usize::from(map_pixel_width.min(map_pixel_height) >= 24);
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        map_start_x.saturating_add(pixel_x).saturating_sub(radius),
        map_start_y.saturating_add(pixel_y).saturating_sub(radius),
        radius * 2 + 1,
        radius * 2 + 1,
        window_minimap_player_color(player_in_window),
    );
}

fn draw_window_minimap_focus(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
    focus_in_window: Option<bool>,
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let arm = if map_pixel_width.min(map_pixel_height) >= 24 {
        2
    } else {
        1
    };
    let color = window_minimap_focus_color(focus_in_window);
    let center_x = map_start_x.saturating_add(pixel_x);
    let center_y = map_start_y.saturating_add(pixel_y);

    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x.saturating_sub(arm),
        center_y,
        arm * 2 + 1,
        1,
        color,
    );
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x,
        center_y.saturating_sub(arm),
        1,
        arm * 2 + 1,
        color,
    );
}

fn window_minimap_focus_color(focus_in_window: Option<bool>) -> u32 {
    match focus_in_window {
        Some(false) => COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL,
        _ => COLOR_MARKER,
    }
}

fn window_minimap_player_color(player_in_window: bool) -> u32 {
    if player_in_window {
        COLOR_PLAYER
    } else {
        COLOR_MINIMAP_INSET_VIEWPORT_WARN
    }
}

fn window_minimap_tile_in_window(window: PresenterViewWindow, tile: (usize, usize)) -> bool {
    let window_last_x = window
        .origin_x
        .saturating_add(window.width.saturating_sub(1));
    let window_last_y = window
        .origin_y
        .saturating_add(window.height.saturating_sub(1));
    tile.0 >= window.origin_x
        && tile.0 <= window_last_x
        && tile.1 >= window.origin_y
        && tile.1 <= window_last_y
}

fn draw_window_minimap_ping(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let radius = usize::from(map_pixel_width.min(map_pixel_height) >= 24);
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        map_start_x.saturating_add(pixel_x).saturating_sub(radius),
        map_start_y.saturating_add(pixel_y).saturating_sub(radius),
        radius * 2 + 1,
        radius * 2 + 1,
        COLOR_MARKER,
    );
}

fn draw_window_minimap_world_label(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let center_x = map_start_x.saturating_add(pixel_x);
    let center_y = map_start_y.saturating_add(pixel_y);
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x,
        center_y,
        1,
        1,
        COLOR_RUNTIME,
    );
}

fn draw_window_minimap_runtime_overlay(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    overlay: WindowMinimapRuntimeOverlayTile,
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        overlay.tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let center_x = map_start_x.saturating_add(pixel_x);
    let center_y = map_start_y.saturating_add(pixel_y);
    let color = match overlay.kind {
        WindowMinimapRuntimeOverlayKind::Config => COLOR_ICON_BUILD_CONFIG,
        WindowMinimapRuntimeOverlayKind::ConfigAlert => COLOR_UNKNOWN,
        WindowMinimapRuntimeOverlayKind::Break => COLOR_ICON_RUNTIME_BREAK,
        WindowMinimapRuntimeOverlayKind::Place => COLOR_PLAN,
        WindowMinimapRuntimeOverlayKind::Building => COLOR_RUNTIME,
        WindowMinimapRuntimeOverlayKind::Health => COLOR_ICON_RUNTIME_HEALTH,
    };
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x,
        center_y,
        1,
        1,
        color,
    );
}

fn draw_window_minimap_command(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let center_x = map_start_x.saturating_add(pixel_x);
    let center_y = map_start_y.saturating_add(pixel_y);
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x,
        center_y,
        1,
        1,
        COLOR_ICON_RUNTIME_COMMAND,
    );
}

fn draw_window_minimap_command_rect(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    rect: WindowMinimapCommandRect,
) {
    if rect.width == 0 || rect.height == 0 || map_width == 0 || map_height == 0 {
        return;
    }

    let rect_x = map_start_x.saturating_add(project_window_minimap_x(
        rect.origin_x,
        map_width,
        map_pixel_width,
    ));
    let rect_width = project_window_minimap_span(rect.width, map_width, map_pixel_width);
    let rect_top = map_height.saturating_sub(rect.origin_y.saturating_add(rect.height));
    let rect_y = map_start_y.saturating_add(project_window_minimap_y(
        rect_top,
        map_height,
        map_pixel_height,
    ));
    let rect_height = project_window_minimap_span(rect.height, map_height, map_pixel_height);
    let color = match rect.kind {
        WindowMinimapCommandRectKind::Selection | WindowMinimapCommandRectKind::Target => {
            COLOR_ICON_RUNTIME_COMMAND
        }
    };
    draw_window_minimap_outline(
        pixels,
        surface_width,
        surface_height,
        rect_x,
        rect_y,
        rect_width,
        rect_height,
        color,
    );
}

fn draw_window_minimap_break_rect(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    rect: WindowMinimapBreakRect,
) {
    if rect.width == 0 || rect.height == 0 || map_width == 0 || map_height == 0 {
        return;
    }

    let rect_x = map_start_x.saturating_add(project_window_minimap_x(
        rect.origin_x,
        map_width,
        map_pixel_width,
    ));
    let rect_width = project_window_minimap_span(rect.width, map_width, map_pixel_width);
    let rect_top = map_height.saturating_sub(rect.origin_y.saturating_add(rect.height));
    let rect_y = map_start_y.saturating_add(project_window_minimap_y(
        rect_top,
        map_height,
        map_pixel_height,
    ));
    let rect_height = project_window_minimap_span(rect.height, map_height, map_pixel_height);
    draw_window_minimap_outline(
        pixels,
        surface_width,
        surface_height,
        rect_x,
        rect_y,
        rect_width,
        rect_height,
        COLOR_ICON_RUNTIME_BREAK,
    );
}

fn draw_window_minimap_unit_assembler(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let center_x = map_start_x.saturating_add(pixel_x);
    let center_y = map_start_y.saturating_add(pixel_y);
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x,
        center_y,
        1,
        1,
        COLOR_ICON_RUNTIME_UNIT_ASSEMBLER,
    );
}

fn draw_window_minimap_unit_assembler_rect(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    rect: WindowMinimapUnitAssemblerRect,
) {
    if rect.width == 0 || rect.height == 0 || map_width == 0 || map_height == 0 {
        return;
    }

    let rect_x = map_start_x.saturating_add(project_window_minimap_x(
        rect.origin_x,
        map_width,
        map_pixel_width,
    ));
    let rect_width = project_window_minimap_span(rect.width, map_width, map_pixel_width);
    let rect_top = map_height.saturating_sub(rect.origin_y.saturating_add(rect.height));
    let rect_y = map_start_y.saturating_add(project_window_minimap_y(
        rect_top,
        map_height,
        map_pixel_height,
    ));
    let rect_height = project_window_minimap_span(rect.height, map_height, map_pixel_height);
    draw_window_minimap_outline(
        pixels,
        surface_width,
        surface_height,
        rect_x,
        rect_y,
        rect_width,
        rect_height,
        COLOR_ICON_RUNTIME_UNIT_ASSEMBLER,
    );
}

fn draw_window_minimap_tile_action(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    map_start_x: usize,
    map_start_y: usize,
    map_pixel_width: usize,
    map_pixel_height: usize,
    map_width: usize,
    map_height: usize,
    tile: (usize, usize),
) {
    let Some((pixel_x, pixel_y)) = project_window_minimap_point(
        tile,
        map_width,
        map_height,
        map_pixel_width,
        map_pixel_height,
    ) else {
        return;
    };
    let center_x = map_start_x.saturating_add(pixel_x);
    let center_y = map_start_y.saturating_add(pixel_y);
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        center_x,
        center_y,
        1,
        1,
        COLOR_ICON_RUNTIME_TILE_ACTION,
    );
}

fn project_window_minimap_point(
    tile: (usize, usize),
    map_width: usize,
    map_height: usize,
    pixel_width: usize,
    pixel_height: usize,
) -> Option<(usize, usize)> {
    if map_width == 0 || map_height == 0 || pixel_width == 0 || pixel_height == 0 {
        return None;
    }

    let center_x = ((tile.0.saturating_mul(2).saturating_add(1)).saturating_mul(pixel_width))
        / map_width.saturating_mul(2).max(1);
    let top_tile = map_height.saturating_sub(tile.1.saturating_add(1));
    let center_y = ((top_tile.saturating_mul(2).saturating_add(1)).saturating_mul(pixel_height))
        / map_height.saturating_mul(2).max(1);

    Some((
        center_x.min(pixel_width.saturating_sub(1)),
        center_y.min(pixel_height.saturating_sub(1)),
    ))
}

fn project_window_minimap_x(tile_x: usize, map_width: usize, pixel_width: usize) -> usize {
    tile_x.saturating_mul(pixel_width) / map_width.max(1)
}

fn project_window_minimap_y(tile_y: usize, map_height: usize, pixel_height: usize) -> usize {
    tile_y.saturating_mul(pixel_height) / map_height.max(1)
}

fn project_window_minimap_span(span: usize, map_span: usize, pixel_span: usize) -> usize {
    let projected = ((span.saturating_mul(pixel_span)).saturating_add(map_span.saturating_sub(1)))
        / map_span.max(1);
    projected.max(1)
}

fn draw_window_minimap_outline(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    start_x: usize,
    start_y: usize,
    width: usize,
    height: usize,
    color: u32,
) {
    if width == 0 || height == 0 {
        return;
    }

    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        start_x,
        start_y,
        width,
        1,
        color,
    );
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        start_x,
        start_y.saturating_add(height.saturating_sub(1)),
        width,
        1,
        color,
    );
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        start_x,
        start_y,
        1,
        height,
        color,
    );
    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        start_x.saturating_add(width.saturating_sub(1)),
        start_y,
        1,
        height,
        color,
    );
}

fn overlay_window_hud(
    frame: &WindowFrame,
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
) {
    let top_line = window_hud_top_line(frame);
    let panel_line = frame
        .panel_lines
        .iter()
        .find(|line| !line.is_empty())
        .map(String::as_str);
    let overlay_line = frame
        .overlay_lines
        .iter()
        .find(|line| !line.is_empty())
        .map(String::as_str);

    if let Some(text) = top_line {
        draw_window_hud_bar(&[text], 0, pixels, surface_width, surface_height);
    }

    let mut bottom_lines = Vec::new();
    if let Some(text) = panel_line {
        bottom_lines.push(text);
    }
    if let Some(text) = overlay_line {
        bottom_lines.push(text);
    }
    if let Some(text) = frame
        .build_strip_text
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        bottom_lines.push(text);
    }
    if let Some(text) = frame
        .build_strip_detail_text
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        bottom_lines.push(text);
    }
    if bottom_lines.is_empty() {
        return;
    }

    let bar_height = window_hud_bar_height(bottom_lines.len());
    let start_y = surface_height.saturating_sub(bar_height);
    draw_window_hud_bar(
        &bottom_lines,
        start_y,
        pixels,
        surface_width,
        surface_height,
    );
}

fn window_hud_top_line(frame: &WindowFrame) -> Option<&str> {
    frame
        .session_banner_text
        .as_deref()
        .filter(|text| !text.is_empty())
        .or_else(|| frame.wave_text.as_deref().filter(|text| !text.is_empty()))
}

fn draw_window_hud_bar(
    lines: &[&str],
    start_y: usize,
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
) {
    if lines.is_empty() || surface_width == 0 || surface_height == 0 {
        return;
    }

    fill_window_hud_rect(
        pixels,
        surface_width,
        surface_height,
        0,
        start_y,
        surface_width,
        window_hud_bar_height(lines.len()),
        COLOR_WINDOW_HUD_BAR,
    );

    let line_step = WINDOW_HUD_FONT_HEIGHT + 1;
    let text_origin_y = start_y + WINDOW_HUD_BAR_PADDING_Y;
    for (index, line) in lines.iter().enumerate() {
        draw_window_hud_text_line(
            line,
            WINDOW_HUD_BAR_PADDING_X,
            text_origin_y + index * line_step,
            pixels,
            surface_width,
            surface_height,
            COLOR_WINDOW_HUD_TEXT,
        );
    }
}

fn fill_window_hud_rect(
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    start_x: usize,
    start_y: usize,
    width: usize,
    height: usize,
    color: u32,
) {
    let end_x = start_x.saturating_add(width).min(surface_width);
    let end_y = start_y.saturating_add(height).min(surface_height);
    for y in start_y.min(surface_height)..end_y {
        let row = y * surface_width;
        for x in start_x.min(surface_width)..end_x {
            pixels[row + x] = color;
        }
    }
}

fn draw_window_hud_text_line(
    text: &str,
    start_x: usize,
    start_y: usize,
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    color: u32,
) {
    let mut cursor_x = start_x;
    let advance = WINDOW_HUD_FONT_WIDTH + WINDOW_HUD_FONT_SPACING;
    for ch in text.chars() {
        if cursor_x >= surface_width {
            break;
        }
        draw_window_hud_glyph(
            ch,
            cursor_x,
            start_y,
            pixels,
            surface_width,
            surface_height,
            color,
        );
        cursor_x = cursor_x.saturating_add(advance);
    }
}

fn draw_window_hud_glyph(
    ch: char,
    start_x: usize,
    start_y: usize,
    pixels: &mut [u32],
    surface_width: usize,
    surface_height: usize,
    color: u32,
) {
    let glyph = window_hud_glyph(ch);
    for (row, bits) in glyph.iter().enumerate() {
        let y = start_y + row;
        if y >= surface_height {
            break;
        }
        let row_offset = y * surface_width;
        for col in 0..WINDOW_HUD_FONT_WIDTH {
            let x = start_x + col;
            if x >= surface_width {
                break;
            }
            let shift = WINDOW_HUD_FONT_WIDTH - 1 - col;
            if ((bits >> shift) & 1) != 0 {
                pixels[row_offset + x] = color;
            }
        }
    }
}

fn window_hud_bar_height(line_count: usize) -> usize {
    if line_count == 0 {
        return 0;
    }

    let line_step = WINDOW_HUD_FONT_HEIGHT + 1;
    WINDOW_HUD_BAR_PADDING_Y * 2
        + WINDOW_HUD_FONT_HEIGHT
        + line_step.saturating_mul(line_count.saturating_sub(1))
}

fn window_hud_glyph(ch: char) -> [u8; WINDOW_HUD_FONT_HEIGHT] {
    let ch = if ch.is_ascii_lowercase() {
        ch.to_ascii_uppercase()
    } else {
        ch
    };

    match ch {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b011, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b011, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b100, 0b100],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b110],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        ';' => [0b000, 0b010, 0b000, 0b010, 0b100],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '\\' => [0b100, 0b100, 0b010, 0b001, 0b001],
        '|' => [0b010, 0b010, 0b010, 0b010, 0b010],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '@' => [0b010, 0b101, 0b111, 0b100, 0b011],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '[' => [0b110, 0b100, 0b100, 0b100, 0b110],
        ']' => [0b011, 0b001, 0b001, 0b001, 0b011],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '\'' => [0b010, 0b010, 0b000, 0b000, 0b000],
        '"' => [0b101, 0b101, 0b000, 0b000, 0b000],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '?' => [0b110, 0b001, 0b010, 0b000, 0b010],
        '~' => [0b000, 0b101, 0b010, 0b000, 0b000],
        '*' => [0b000, 0b101, 0b010, 0b101, 0b000],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '&' => [0b010, 0b101, 0b010, 0b101, 0b011],
        ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
        _ => [0b110, 0b001, 0b010, 0b000, 0b010],
    }
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
        collect_stable_minimap_overlay_tiles, color_for_object, compose_frame,
        fit_window_minimap_size, runtime_break_minimap_rects, runtime_command_minimap_rects,
        runtime_command_minimap_tiles, runtime_ping_minimap_tile,
        runtime_tile_action_minimap_tiles, runtime_unit_assembler_minimap_rects,
        runtime_unit_assembler_minimap_tiles, runtime_world_label_minimap_tiles,
        runtime_world_span_to_tile_span, runtime_world_to_minimap_tile, scale_frame_pixels,
        window_hud_bar_height, window_hud_top_line, BackendSignal,
        StableMinimapOverlayTileCandidate, WindowBackend, WindowFrame, WindowMinimapBreakRect,
        WindowMinimapCommandRect, WindowMinimapCommandRectKind, WindowMinimapInset,
        WindowMinimapRuntimeOverlayKind, WindowMinimapRuntimeOverlayTile,
        WindowMinimapUnitAssemblerRect, WindowPresenter, COLOR_BLOCK, COLOR_EMPTY,
        COLOR_ICON_BUILD_CONFIG, COLOR_ICON_RUNTIME_BREAK, COLOR_ICON_RUNTIME_BULLET,
        COLOR_ICON_RUNTIME_COMMAND, COLOR_ICON_RUNTIME_EFFECT, COLOR_ICON_RUNTIME_EFFECT_MARKER,
        COLOR_ICON_RUNTIME_HEALTH, COLOR_ICON_RUNTIME_LOGIC_EXPLOSION, COLOR_ICON_RUNTIME_SOUND_AT,
        COLOR_ICON_RUNTIME_TILE_ACTION, COLOR_ICON_RUNTIME_UNIT_ASSEMBLER,
        COLOR_MARKER, COLOR_MINIMAP_INSET_BACKGROUND, COLOR_MINIMAP_INSET_BACKGROUND_PARTIAL,
        COLOR_MINIMAP_INSET_BACKGROUND_WARN, COLOR_MINIMAP_INSET_VIEWPORT,
        COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL, COLOR_MINIMAP_INSET_VIEWPORT_WARN, COLOR_PLAN,
        COLOR_PLAYER, COLOR_RUNTIME, COLOR_TERRAIN, COLOR_UNKNOWN, COLOR_WINDOW_HUD_BAR,
        COLOR_WINDOW_HUD_TEXT, WINDOW_HUD_BAR_PADDING_X, WINDOW_HUD_BAR_PADDING_Y,
        WINDOW_HUD_FONT_HEIGHT,
    };
    use crate::{
        hud_model::{
            HudSummary, RuntimeBootstrapObservability, RuntimeReconnectObservability,
            RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind,
            RuntimeResourceDeltaObservability, RuntimeSessionObservability,
            RuntimeSessionResetKind, RuntimeSessionTimeoutKind, RuntimeWorldReloadObservability,
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

    fn runtime_command_rect_objects(
        family: &str,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> Vec<RenderObject> {
        let mut objects = Vec::new();
        for (edge, source, target) in [
            ("top", (left, top), (right, top)),
            ("right", (right, top), (right, bottom)),
            ("bottom", (right, bottom), (left, bottom)),
            ("left", (left, bottom), (left, top)),
        ] {
            let line_id = format!(
                "marker:line:{family}:{edge}:{}:{}:{}:{}",
                source.0.to_bits(),
                source.1.to_bits(),
                target.0.to_bits(),
                target.1.to_bits()
            );
            objects.push(RenderObject {
                id: line_id.clone(),
                layer: 29,
                x: source.0,
                y: source.1,
            });
            objects.push(RenderObject {
                id: format!("{line_id}:line-end"),
                layer: 29,
                x: target.0,
                y: target.1,
            });
        }
        objects
    }

    fn runtime_unit_assembler_area_objects(
        block_name: &str,
        tile_x: i32,
        tile_y: i32,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> Vec<RenderObject> {
        let mut objects = Vec::new();
        for (edge, source, target) in [
            ("top", (left, top), (right, top)),
            ("right", (right, top), (right, bottom)),
            ("bottom", (right, bottom), (left, bottom)),
            ("left", (left, bottom), (left, top)),
        ] {
            let line_id = format!(
                "marker:line:runtime-unit-assembler-area:{block_name}:{tile_x}:{tile_y}:{edge}"
            );
            objects.push(RenderObject {
                id: line_id.clone(),
                layer: 15,
                x: source.0,
                y: source.1,
            });
            objects.push(RenderObject {
                id: format!("{line_id}:line-end"),
                layer: 15,
                x: target.0,
                y: target.1,
            });
        }
        objects
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

    #[derive(Default)]
    struct FailingBackend {
        error: String,
    }

    impl WindowBackend for FailingBackend {
        fn present(&mut self, _frame: &WindowFrame) -> Result<BackendSignal, String> {
            if self.error.is_empty() {
                self.error = "backend failure".to_string();
            }
            Err(self.error.clone())
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
    fn present_once_records_last_error_on_backend_failure() {
        let backend = FailingBackend::default();
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
            title: "demo".to_string(),
            wave_text: None,
            status_text: "ok".to_string(),
            overlay_summary_text: None,
            fps: Some(60.0),
            summary: None,
            runtime_ui: None,
            build_ui: None,
        };

        let err = presenter.present_once(&scene, &hud).unwrap_err();
        assert_eq!(err, "backend failure");
        assert_eq!(presenter.last_error(), Some("backend failure"));
        assert_eq!(presenter.frame_id(), 0);
    }

    #[test]
    fn window_frame_pixel_returns_none_for_short_buffer() {
        let frame = WindowFrame {
            frame_id: 0,
            title: String::new(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 2,
            height: 2,
            minimap_inset: None,
            pixels: vec![1, 2, 3],
        };

        assert_eq!(frame.pixel(1, 1), None);
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
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 2,
            height: 1,
            minimap_inset: None,
            pixels: vec![0x112233, 0x445566],
        };

        let pixels = scale_frame_pixels(&frame, 2);

        assert_eq!(
            pixels,
            vec![0x112233, 0x112233, 0x445566, 0x445566, 0x112233, 0x112233, 0x445566, 0x445566,]
        );
    }

    #[test]
    fn scale_frame_pixels_handles_large_dimensions_without_overflow() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: usize::MAX,
            height: 2,
            minimap_inset: None,
            pixels: Vec::new(),
        };

        let pixels = scale_frame_pixels(&frame, 2);

        assert!(pixels.is_empty());
    }

    #[test]
    fn scale_frame_pixels_blits_window_hud_text_bars() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: Some("A".to_string()),
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: vec!["B".to_string()],
            overlay_lines: vec!["C".to_string()],
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 12,
            height: 8,
            minimap_inset: None,
            pixels: vec![COLOR_EMPTY; 12 * 8],
        };

        let pixels = scale_frame_pixels(&frame, 4);
        let surface_width = frame.width * 4;
        let surface_height = frame.height * 4;
        let bottom_bar_y = surface_height.saturating_sub(window_hud_bar_height(2));
        let second_bottom_line_y =
            bottom_bar_y + WINDOW_HUD_BAR_PADDING_Y + WINDOW_HUD_FONT_HEIGHT + 1;

        assert_eq!(pixels[0], COLOR_WINDOW_HUD_BAR);
        assert_eq!(
            pixels[WINDOW_HUD_BAR_PADDING_Y * surface_width + WINDOW_HUD_BAR_PADDING_X + 1],
            COLOR_WINDOW_HUD_TEXT
        );
        assert_eq!(
            pixels[(bottom_bar_y + WINDOW_HUD_BAR_PADDING_Y) * surface_width
                + WINDOW_HUD_BAR_PADDING_X],
            COLOR_WINDOW_HUD_TEXT
        );
        assert_eq!(
            pixels[second_bottom_line_y * surface_width + WINDOW_HUD_BAR_PADDING_X + 1],
            COLOR_WINDOW_HUD_TEXT
        );
        assert_eq!(pixels[12 * surface_width + 20], COLOR_EMPTY);
    }

    #[test]
    fn scale_frame_pixels_blits_build_strip_as_third_bottom_line() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: Some("D".to_string()),
            build_strip_detail_text: None,
            panel_lines: vec!["B".to_string()],
            overlay_lines: vec!["C".to_string()],
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 12,
            height: 8,
            minimap_inset: None,
            pixels: vec![COLOR_EMPTY; 12 * 8],
        };

        let pixels = scale_frame_pixels(&frame, 4);
        let surface_width = frame.width * 4;
        let surface_height = frame.height * 4;
        let bottom_bar_y = surface_height.saturating_sub(window_hud_bar_height(3));
        let third_bottom_line_y =
            bottom_bar_y + WINDOW_HUD_BAR_PADDING_Y + (WINDOW_HUD_FONT_HEIGHT + 1) * 2;

        assert_eq!(
            pixels[third_bottom_line_y * surface_width + WINDOW_HUD_BAR_PADDING_X + 1],
            COLOR_WINDOW_HUD_TEXT
        );
    }

    #[test]
    fn scale_frame_pixels_blits_build_strip_detail_as_fourth_bottom_line() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: Some("D".to_string()),
            build_strip_detail_text: Some("E".to_string()),
            panel_lines: vec!["B".to_string()],
            overlay_lines: vec!["C".to_string()],
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 12,
            height: 8,
            minimap_inset: None,
            pixels: vec![COLOR_EMPTY; 12 * 8],
        };

        let pixels = scale_frame_pixels(&frame, 4);
        let surface_width = frame.width * 4;
        let surface_height = frame.height * 4;
        let bottom_bar_y = surface_height.saturating_sub(window_hud_bar_height(4));
        let fourth_bottom_line_y =
            bottom_bar_y + WINDOW_HUD_BAR_PADDING_Y + (WINDOW_HUD_FONT_HEIGHT + 1) * 3;

        assert_eq!(
            pixels[fourth_bottom_line_y * surface_width + WINDOW_HUD_BAR_PADDING_X + 1],
            COLOR_WINDOW_HUD_TEXT
        );
    }

    #[test]
    fn window_hud_top_line_prefers_session_banner_and_falls_back_to_wave() {
        let mut frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: Some("Wave 7".to_string()),
            session_banner_text: Some("KICK idInUse@7:IdInUse:wait_for_old~".to_string()),
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 12,
            height: 8,
            minimap_inset: None,
            pixels: vec![COLOR_EMPTY; 12 * 8],
        };

        assert_eq!(
            window_hud_top_line(&frame),
            Some("KICK idInUse@7:IdInUse:wait_for_old~")
        );

        frame.session_banner_text = None;
        assert_eq!(window_hud_top_line(&frame), Some("Wave 7"));
    }

    #[test]
    fn window_hud_top_line_ignores_empty_strings() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: Some(String::new()),
            session_banner_text: Some(String::new()),
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 12,
            height: 8,
            minimap_inset: None,
            pixels: vec![COLOR_EMPTY; 12 * 8],
        };

        assert_eq!(window_hud_top_line(&frame), None);
    }

    #[test]
    fn compose_frame_rasterizes_marker_line_segments_across_window_bounds() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:line:demo".to_string(),
                    layer: 15,
                    x: -8.0,
                    y: 24.0,
                },
                RenderObject {
                    id: "marker:line:demo:line-end".to_string(),
                    layer: 15,
                    x: 16.0,
                    y: 24.0,
                },
            ],
        };

        let frame = compose_frame(&scene, &HudModel::default(), 0, None);

        assert_eq!(frame.pixel(0, 0), Some(COLOR_MARKER));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_MARKER));
        assert_eq!(frame.pixel(2, 0), Some(COLOR_MARKER));
        assert_eq!(frame.pixel(3, 0), Some(COLOR_EMPTY));
    }

    #[test]
    fn compose_frame_clamps_non_finite_viewport_span_to_one_tile() {
        let scene = RenderModel {
            viewport: Viewport {
                width: f32::NAN,
                height: f32::INFINITY,
                zoom: 1.0,
            },
            view_window: None,
            objects: Vec::new(),
        };

        let frame = compose_frame(&scene, &HudModel::default(), 0, None);

        assert_eq!((frame.width, frame.height), (1, 1));
        assert_eq!(frame.pixel(0, 0), Some(COLOR_EMPTY));
    }

    #[test]
    fn compose_frame_drops_marker_lines_with_non_finite_endpoints() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:line:demo".to_string(),
                    layer: 15,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:line:demo:line-end".to_string(),
                    layer: 15,
                    x: f32::NAN,
                    y: f32::INFINITY,
                },
            ],
        };

        let frame = compose_frame(&scene, &HudModel::default(), 0, None);

        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 0), Some(COLOR_EMPTY));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_EMPTY));
        assert_eq!(frame.pixel(0, 1), Some(COLOR_EMPTY));
        assert_eq!(frame.pixel(1, 1), Some(COLOR_EMPTY));
    }

    #[test]
    fn scale_frame_pixels_draws_minimap_inset_in_top_right_corner() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 24,
            height: 18,
            minimap_inset: Some(WindowMinimapInset {
                map_width: 80,
                map_height: 60,
                window: crate::panel_model::PresenterViewWindow {
                    origin_x: 16,
                    origin_y: 12,
                    width: 24,
                    height: 18,
                },
                window_coverage_percent: 9,
                map_object_density_percent: 2,
                window_object_density_percent: 11,
                outside_object_percent: 50,
                focus_tile: Some((60, 40)),
                focus_in_window: Some(false),
                player_tile: Some((20, 18)),
                ping_tile: Some((44, 22)),
                unit_assembler_tiles: vec![(34, 12), (36, 14)],
                tile_action_tiles: vec![(18, 14), (22, 30)],
                command_tiles: vec![(24, 20), (26, 18)],
                command_rects: vec![
                    WindowMinimapCommandRect {
                        origin_x: 8,
                        origin_y: 10,
                        width: 6,
                        height: 4,
                        kind: WindowMinimapCommandRectKind::Selection,
                    },
                    WindowMinimapCommandRect {
                        origin_x: 20,
                        origin_y: 14,
                        width: 5,
                        height: 3,
                        kind: WindowMinimapCommandRectKind::Target,
                    },
                ],
                runtime_break_rects: vec![WindowMinimapBreakRect {
                    origin_x: 12,
                    origin_y: 8,
                    width: 6,
                    height: 4,
                }],
                unit_assembler_rects: vec![WindowMinimapUnitAssemblerRect {
                    origin_x: 30,
                    origin_y: 10,
                    width: 6,
                    height: 6,
                }],
                world_label_tiles: vec![(30, 26)],
                runtime_overlay_tiles: vec![
                    WindowMinimapRuntimeOverlayTile {
                        tile: (12, 16),
                        kind: WindowMinimapRuntimeOverlayKind::Config,
                    },
                    WindowMinimapRuntimeOverlayTile {
                        tile: (28, 24),
                        kind: WindowMinimapRuntimeOverlayKind::ConfigAlert,
                    },
                    WindowMinimapRuntimeOverlayTile {
                        tile: (40, 28),
                        kind: WindowMinimapRuntimeOverlayKind::Place,
                    },
                    WindowMinimapRuntimeOverlayTile {
                        tile: (24, 20),
                        kind: WindowMinimapRuntimeOverlayKind::Building,
                    },
                ],
            }),
            pixels: vec![COLOR_EMPTY; 24 * 18],
        };

        let pixels = scale_frame_pixels(&frame, 4);
        let surface_width = frame.width * 4;
        let surface_height = frame.height * 4;
        let mut top_right_pixels = Vec::new();
        for y in 0..surface_height / 2 {
            for x in (surface_width * 3) / 5..surface_width {
                top_right_pixels.push(pixels[y * surface_width + x]);
            }
        }
        let mut lower_left_pixels = Vec::new();
        for y in (surface_height * 3) / 5..surface_height {
            for x in 0..surface_width / 2 {
                lower_left_pixels.push(pixels[y * surface_width + x]);
            }
        }

        assert!(top_right_pixels.contains(&COLOR_MINIMAP_INSET_BACKGROUND_WARN));
        assert!(top_right_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_WARN));
        assert!(top_right_pixels.contains(&COLOR_PLAYER));
        assert!(top_right_pixels.contains(&COLOR_MARKER));
        assert!(top_right_pixels.contains(&COLOR_RUNTIME));
        assert!(top_right_pixels.contains(&COLOR_ICON_RUNTIME_COMMAND));
        assert!(top_right_pixels.contains(&COLOR_ICON_RUNTIME_UNIT_ASSEMBLER));
        assert!(top_right_pixels.contains(&COLOR_ICON_RUNTIME_TILE_ACTION));
        assert!(top_right_pixels.contains(&COLOR_ICON_BUILD_CONFIG));
        assert!(top_right_pixels.contains(&COLOR_UNKNOWN));
        assert!(top_right_pixels.contains(&COLOR_ICON_RUNTIME_BREAK));
        assert!(top_right_pixels.contains(&COLOR_PLAN));
        assert!(top_right_pixels.contains(&COLOR_RUNTIME));
        assert!(lower_left_pixels.iter().all(|&pixel| pixel == COLOR_EMPTY));
    }

    #[test]
    fn scale_frame_pixels_draws_density_aware_minimap_viewport_colors() {
        let frame = WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 24,
            height: 18,
            minimap_inset: Some(WindowMinimapInset {
                map_width: 80,
                map_height: 60,
                window: crate::panel_model::PresenterViewWindow {
                    origin_x: 16,
                    origin_y: 12,
                    width: 24,
                    height: 18,
                },
                window_coverage_percent: 51,
                map_object_density_percent: 10,
                window_object_density_percent: 5,
                outside_object_percent: 10,
                focus_tile: None,
                focus_in_window: None,
                player_tile: None,
                ping_tile: None,
                unit_assembler_tiles: Vec::new(),
                tile_action_tiles: Vec::new(),
                command_tiles: Vec::new(),
                command_rects: Vec::new(),
                runtime_break_rects: Vec::new(),
                unit_assembler_rects: Vec::new(),
                world_label_tiles: Vec::new(),
                runtime_overlay_tiles: Vec::new(),
            }),
            pixels: vec![COLOR_EMPTY; 24 * 18],
        };

        let pixels = scale_frame_pixels(&frame, 4);
        let surface_width = frame.width * 4;
        let surface_height = frame.height * 4;
        let mut top_right_pixels = Vec::new();
        for y in 0..surface_height / 2 {
            for x in (surface_width * 3) / 5..surface_width {
                top_right_pixels.push(pixels[y * surface_width + x]);
            }
        }

        assert!(top_right_pixels.contains(&COLOR_MINIMAP_INSET_BACKGROUND_PARTIAL));
        assert!(top_right_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL));
        assert!(!top_right_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_WARN));
    }

    #[test]
    fn scale_frame_pixels_colors_focus_marker_by_focus_in_window() {
        let make_frame = |focus_in_window| WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 24,
            height: 18,
            minimap_inset: Some(WindowMinimapInset {
                map_width: 80,
                map_height: 60,
                window: crate::panel_model::PresenterViewWindow {
                    origin_x: 16,
                    origin_y: 12,
                    width: 24,
                    height: 18,
                },
                window_coverage_percent: 100,
                map_object_density_percent: 0,
                window_object_density_percent: 0,
                outside_object_percent: 0,
                focus_tile: Some((40, 30)),
                focus_in_window,
                player_tile: None,
                ping_tile: None,
                unit_assembler_tiles: Vec::new(),
                tile_action_tiles: Vec::new(),
                command_tiles: Vec::new(),
                command_rects: Vec::new(),
                runtime_break_rects: Vec::new(),
                unit_assembler_rects: Vec::new(),
                world_label_tiles: Vec::new(),
                runtime_overlay_tiles: Vec::new(),
            }),
            pixels: vec![COLOR_EMPTY; 24 * 18],
        };

        let inside_pixels = scale_frame_pixels(&make_frame(Some(true)), 4);
        let outside_pixels = scale_frame_pixels(&make_frame(Some(false)), 4);

        assert!(inside_pixels.contains(&COLOR_MARKER));
        assert!(!inside_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL));
        assert!(outside_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL));
        assert!(!outside_pixels.contains(&COLOR_MARKER));
    }

    #[test]
    fn scale_frame_pixels_colors_player_marker_by_window_membership() {
        let make_frame = |player_tile| WindowFrame {
            frame_id: 0,
            title: "demo".to_string(),
            wave_text: None,
            session_banner_text: None,
            status_text: String::new(),
            build_strip_text: None,
            build_strip_detail_text: None,
            panel_lines: Vec::new(),
            overlay_lines: Vec::new(),
            overlay_summary_text: None,
            fps: None,
            zoom: 1.0,
            width: 24,
            height: 18,
            minimap_inset: Some(WindowMinimapInset {
                map_width: 80,
                map_height: 60,
                window: crate::panel_model::PresenterViewWindow {
                    origin_x: 16,
                    origin_y: 12,
                    width: 24,
                    height: 18,
                },
                window_coverage_percent: 51,
                map_object_density_percent: 0,
                window_object_density_percent: 0,
                outside_object_percent: 0,
                focus_tile: None,
                focus_in_window: None,
                player_tile: Some(player_tile),
                ping_tile: None,
                unit_assembler_tiles: Vec::new(),
                tile_action_tiles: Vec::new(),
                command_tiles: Vec::new(),
                command_rects: Vec::new(),
                runtime_break_rects: Vec::new(),
                unit_assembler_rects: Vec::new(),
                world_label_tiles: Vec::new(),
                runtime_overlay_tiles: Vec::new(),
            }),
            pixels: vec![COLOR_EMPTY; 24 * 18],
        };

        let inside_pixels = scale_frame_pixels(&make_frame((20, 18)), 4);
        let outside_pixels = scale_frame_pixels(&make_frame((60, 50)), 4);

        assert!(inside_pixels.contains(&COLOR_PLAYER));
        assert!(!inside_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_WARN));
        assert!(outside_pixels.contains(&COLOR_MINIMAP_INSET_VIEWPORT_WARN));
        assert!(!outside_pixels.contains(&COLOR_PLAYER));
    }

    #[test]
    fn window_minimap_viewport_color_tracks_coverage_density_and_visibility_bands() {
        assert_eq!(super::window_minimap_viewport_band(0, 2, 11, 50), "warn");
        assert_eq!(super::window_minimap_viewport_band(11, 2, 2, 29), "partial");
        assert_eq!(super::window_minimap_viewport_band(51, 3, 3, 29), "full");
        assert_eq!(
            super::window_minimap_viewport_color(0, 2, 11, 50),
            COLOR_MINIMAP_INSET_VIEWPORT_WARN
        );
        assert_eq!(
            super::window_minimap_viewport_color(10, 2, 11, 50),
            COLOR_MINIMAP_INSET_VIEWPORT_WARN
        );
        assert_eq!(
            super::window_minimap_viewport_color(11, 2, 2, 29),
            COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL
        );
        assert_eq!(
            super::window_minimap_viewport_color(50, 2, 2, 29),
            COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL
        );
        assert_eq!(
            super::window_minimap_viewport_color(51, 3, 3, 29),
            COLOR_MINIMAP_INSET_VIEWPORT
        );
        assert_eq!(
            super::window_minimap_viewport_color(100, 6, 0, 10),
            COLOR_MINIMAP_INSET_VIEWPORT_WARN
        );
        assert_eq!(
            super::window_minimap_viewport_color(100, 10, 5, 10),
            COLOR_MINIMAP_INSET_VIEWPORT_PARTIAL
        );
        assert_eq!(
            super::window_minimap_viewport_color(100, 10, 10, 60),
            COLOR_MINIMAP_INSET_VIEWPORT_WARN
        );
        assert_eq!(
            super::window_minimap_viewport_color(100, 10, 10, 10),
            COLOR_MINIMAP_INSET_VIEWPORT
        );
        assert_eq!(
            super::window_minimap_background_color(0, 2, 11, 50),
            COLOR_MINIMAP_INSET_BACKGROUND_WARN
        );
        assert_eq!(
            super::window_minimap_background_color(11, 2, 2, 29),
            COLOR_MINIMAP_INSET_BACKGROUND_PARTIAL
        );
        assert_eq!(
            super::window_minimap_background_color(100, 10, 10, 10),
            COLOR_MINIMAP_INSET_BACKGROUND
        );
        assert_eq!(super::window_minimap_player_color(true), COLOR_PLAYER);
        assert_eq!(
            super::window_minimap_player_color(false),
            COLOR_MINIMAP_INSET_VIEWPORT_WARN
        );
        assert!(super::window_minimap_tile_in_window(
            crate::panel_model::PresenterViewWindow {
                origin_x: 16,
                origin_y: 12,
                width: 24,
                height: 18,
            },
            (20, 18)
        ));
        assert!(!super::window_minimap_tile_in_window(
            crate::panel_model::PresenterViewWindow {
                origin_x: 16,
                origin_y: 12,
                width: 24,
                height: 18,
            },
            (60, 50)
        ));
    }

    #[test]
    fn present_once_populates_runtime_building_overlay_tile_in_minimap_inset() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(RenderViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 4,
            }),
            objects: vec![RenderObject {
                id: "block:runtime-building:1:3:3".to_string(),
                layer: 35,
                x: 24.0,
                y: 24.0,
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
                visible_tile_count: 1,
                hidden_tile_count: 0,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((3, 3)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 4,
                        height: 4,
                    },
                },
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let inset = frame.minimap_inset.as_ref().expect("minimap inset");

        assert_eq!(
            inset.runtime_overlay_tiles,
            vec![WindowMinimapRuntimeOverlayTile {
                tile: (3, 3),
                kind: WindowMinimapRuntimeOverlayKind::Building,
            }]
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS: minikind:obj1@pl0:mk0:pn0:bk0:rt1:tr0:uk0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS-DETAIL: runtime-building:1",
        );
    }

    #[test]
    fn present_once_populates_runtime_health_overlay_tile_in_minimap_inset() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(RenderViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 4,
            }),
            objects: vec![RenderObject {
                id: "marker:runtime-health:1:3:3".to_string(),
                layer: 32,
                x: 24.0,
                y: 24.0,
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
                visible_tile_count: 1,
                hidden_tile_count: 0,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((3, 3)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 4,
                        height: 4,
                    },
                },
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let inset = frame.minimap_inset.as_ref().expect("minimap inset");

        assert_eq!(
            inset.runtime_overlay_tiles,
            vec![WindowMinimapRuntimeOverlayTile {
                tile: (3, 3),
                kind: WindowMinimapRuntimeOverlayKind::Health,
            }]
        );
    }

    #[test]
    fn present_once_populates_minimap_inset_metadata() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let mut objects = vec![
            RenderObject {
                id: "player:focus".to_string(),
                layer: 40,
                x: 32.0,
                y: 16.0,
            },
            RenderObject {
                id: "marker:text:runtime-ping:9:text:70696e67".to_string(),
                layer: 31,
                x: 40.0,
                y: 24.0,
            },
            RenderObject {
                id: "world-label:event:7:text:6c6162656c".to_string(),
                layer: 39,
                x: 48.0,
                y: 32.0,
            },
            RenderObject {
                id: "marker:runtime-config-rollback:1:1:string".to_string(),
                layer: 24,
                x: 8.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-config:2:2:string".to_string(),
                layer: 24,
                x: 16.0,
                y: 16.0,
            },
            RenderObject {
                id: "marker:runtime-break:0:3:3".to_string(),
                layer: 20,
                x: 24.0,
                y: 24.0,
            },
            RenderObject {
                id: "terrain:runtime-deconstruct:17:18".to_string(),
                layer: 16,
                x: 136.0,
                y: 144.0,
            },
            RenderObject {
                id: "plan:runtime-place:0:4:4".to_string(),
                layer: 20,
                x: 32.0,
                y: 32.0,
            },
            RenderObject {
                id: "marker:runtime-command-build-target:9:10".to_string(),
                layer: 29,
                x: 72.0,
                y: 80.0,
            },
            RenderObject {
                id: "marker:runtime-command-selected-unit:77".to_string(),
                layer: 29,
                x: 88.0,
                y: 96.0,
            },
            RenderObject {
                id: "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:0:0x40800000".to_string(),
                layer: 16,
                x: 168.0,
                y: 176.0,
            },
            RenderObject {
                id: "marker:runtime-unit-assembler-command:tank-assembler:30:40:0x42200000:0x42700000".to_string(),
                layer: 16,
                x: 184.0,
                y: 192.0,
            },
            RenderObject {
                id: "marker:runtime-unit-block-spawn:1:13:14".to_string(),
                layer: 28,
                x: 104.0,
                y: 112.0,
            },
            RenderObject {
                id: "marker:runtime-assembler-unit-spawned:2:15:16".to_string(),
                layer: 28,
                x: 120.0,
                y: 128.0,
            },
        ];
        objects.extend(runtime_command_rect_objects(
            "runtime-command-rect",
            40.0,
            48.0,
            64.0,
            72.0,
        ));
        objects.extend(runtime_command_rect_objects(
            "runtime-command-target-rect",
            8.0,
            40.0,
            32.0,
            56.0,
        ));
        objects.extend(runtime_command_rect_objects(
            "runtime-break-rect",
            16.0,
            64.0,
            40.0,
            80.0,
        ));
        objects.extend(runtime_unit_assembler_area_objects(
            "tank-assembler",
            30,
            40,
            216.0,
            280.0,
            256.0,
            320.0,
        ));
        objects.extend([
            RenderObject {
                id: "marker:runtime-config-rollback:1:2:string".to_string(),
                layer: 30,
                x: 8.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-config:2:3:string".to_string(),
                layer: 30,
                x: 16.0,
                y: 16.0,
            },
            RenderObject {
                id: "terrain:runtime-deconstruct:3:4".to_string(),
                layer: 30,
                x: 136.0,
                y: 144.0,
            },
            RenderObject {
                id: "plan:runtime-place:4:5".to_string(),
                layer: 30,
                x: 24.0,
                y: 24.0,
            },
        ]);
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
            objects,
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
                visible_tile_count: 10,
                hidden_tile_count: 20,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((7, 6)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let inset = frame.minimap_inset.as_ref().expect("minimap inset");
        let panel = super::build_minimap_panel(
            &scene,
            &hud,
            crate::panel_model::PresenterViewWindow {
                origin_x: 2,
                origin_y: 3,
                width: 4,
                height: 3,
            },
        )
        .expect("expected minimap panel");

        assert_eq!(inset.map_width, 80);
        assert_eq!(inset.map_height, 60);
        assert_eq!(
            inset.window,
            crate::panel_model::PresenterViewWindow {
                origin_x: 2,
                origin_y: 3,
                width: 4,
                height: 3,
            }
        );
        assert_eq!(inset.window_coverage_percent, panel.window_coverage_percent);
        assert_eq!(
            inset.map_object_density_percent,
            panel.map_object_density_percent()
        );
        assert_eq!(
            inset.window_object_density_percent,
            panel.window_object_density_percent()
        );
        assert_eq!(inset.outside_object_percent, panel.outside_object_percent());
        assert_eq!(inset.focus_in_window, panel.focus_in_window);
        assert_frame_line_contains(
            &frame.panel_lines,
            &format!(
                "MINIMAP-DETAIL: {}",
                super::compose_minimap_density_visibility_status_text(&panel)
            ),
        );
        assert_eq!(inset.focus_tile, Some((7, 6)));
        assert_eq!(inset.player_tile, Some((4, 2)));
        assert_eq!(inset.ping_tile, Some((5, 3)));
        assert_eq!(inset.unit_assembler_tiles, vec![(21, 22), (23, 24)]);
        assert_eq!(inset.tile_action_tiles, vec![(13, 14), (15, 16)]);
        assert_eq!(inset.command_tiles, vec![(9, 10), (11, 12)]);
        assert_eq!(
            inset.command_rects,
            vec![
                WindowMinimapCommandRect {
                    origin_x: 5,
                    origin_y: 6,
                    width: 3,
                    height: 3,
                    kind: WindowMinimapCommandRectKind::Selection,
                },
                WindowMinimapCommandRect {
                    origin_x: 1,
                    origin_y: 5,
                    width: 3,
                    height: 2,
                    kind: WindowMinimapCommandRectKind::Target,
                },
            ]
        );
        assert_eq!(
            inset.runtime_break_rects,
            vec![WindowMinimapBreakRect {
                origin_x: 2,
                origin_y: 8,
                width: 3,
                height: 2,
            }]
        );
        assert_eq!(
            inset.unit_assembler_rects,
            vec![WindowMinimapUnitAssemblerRect {
                origin_x: 27,
                origin_y: 35,
                width: 5,
                height: 5,
            }]
        );
        assert_eq!(inset.world_label_tiles, vec![(6, 4)]);
        assert_eq!(
            inset.runtime_overlay_tiles,
            vec![
                WindowMinimapRuntimeOverlayTile {
                    tile: (1, 1),
                    kind: WindowMinimapRuntimeOverlayKind::ConfigAlert,
                },
                WindowMinimapRuntimeOverlayTile {
                    tile: (2, 2),
                    kind: WindowMinimapRuntimeOverlayKind::Config,
                },
                WindowMinimapRuntimeOverlayTile {
                    tile: (3, 3),
                    kind: WindowMinimapRuntimeOverlayKind::Break,
                },
                WindowMinimapRuntimeOverlayTile {
                    tile: (17, 18),
                    kind: WindowMinimapRuntimeOverlayKind::Break,
                },
                WindowMinimapRuntimeOverlayTile {
                    tile: (4, 4),
                    kind: WindowMinimapRuntimeOverlayKind::Place,
                },
            ]
        );
    }

    #[test]
    fn present_once_surfaces_hud_visibility_map_percents() {
        let hud = HudModel {
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 1,
                        height: 1,
                    },
                },
            }),
            ..HudModel::default()
        };

        assert_eq!(
            super::compose_hud_visibility_status_text(&hud),
            Some("hudvis:ov1:fg1:k144p3:v120p83:h24p16:u4656p97:vm2:hm0".to_string())
        );
    }

    #[test]
    fn present_once_surfaces_hud_detail_visibility_and_minimap() {
        let hud = HudModel {
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 1,
                        height: 1,
                    },
                },
            }),
            ..HudModel::default()
        };

        assert_eq!(
            super::compose_hud_detail_status_text(&hud),
            Some("huddet:p=operator#8:sel=payload-rout~#14:t4800:vm2:hm0:ov1:fg1:mini=f0:0:w0:0+1x1:a1".to_string())
        );
        assert_eq!(
            super::compose_hud_visibility_detail_status_text(&hud),
            Some("hudvisd:s=mixed:ov=on:fg=on:k=144/4800:v=120/144:h=24/144:u=4656/4800".to_string())
        );
    }

    #[test]
    fn present_once_reports_minimap_inset_density_stats() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 0,
                x: 16.0,
                y: 24.0,
            }],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 10,
                map_height: 10,
                overlay_visible: true,
                fog_enabled: false,
                visible_tile_count: 25,
                hidden_tile_count: 15,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((2, 3)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 10,
                        height: 10,
                    },
                },
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let inset = frame.minimap_inset.as_ref().expect("minimap inset");
        let panel =
            super::build_minimap_panel(&scene, &hud, inset.window).expect("expected minimap panel");

        assert!(inset.window_coverage_percent > 0);
        assert!(inset.map_object_density_percent > 0);
        assert!(inset.window_object_density_percent > 0);
        assert_eq!(inset.window_coverage_percent, panel.window_coverage_percent);
        assert_eq!(
            inset.map_object_density_percent,
            panel.map_object_density_percent()
        );
        assert_eq!(
            inset.window_object_density_percent,
            panel.window_object_density_percent()
        );
        assert_eq!(inset.outside_object_percent, panel.outside_object_percent());
        assert_frame_line_contains(
            &frame.panel_lines,
            &format!(
                "MINIMAP-DETAIL: {}",
                super::compose_minimap_density_visibility_status_text(&panel)
            ),
        );
    }

    #[test]
    fn present_once_clamps_minimap_tiles_to_map_bounds() {
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
                x: 999.0,
                y: 999.0,
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
                visible_tile_count: 10,
                hidden_tile_count: 20,
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((999, 999)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let inset = frame.minimap_inset.as_ref().expect("minimap inset");

        assert_eq!(inset.map_width, 80);
        assert_eq!(inset.map_height, 60);
        assert_eq!(inset.focus_tile, Some((79, 59)));
        assert_eq!(inset.player_tile, Some((79, 59)));
    }

    #[test]
    fn minimap_helpers_ignore_non_finite_coordinates_and_spans() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:text:runtime-ping:9:text:70696e67".to_string(),
                    layer: 31,
                    x: f32::NAN,
                    y: 24.0,
                },
                RenderObject {
                    id: "world-label:event:7:text:6c6162656c".to_string(),
                    layer: 39,
                    x: 48.0,
                    y: f32::INFINITY,
                },
                RenderObject {
                    id: "marker:runtime-command-build-target:9:10".to_string(),
                    layer: 29,
                    x: f32::INFINITY,
                    y: 80.0,
                },
            ],
        };

        assert_eq!(runtime_ping_minimap_tile(&scene, 80, 60), None);
        assert!(runtime_world_label_minimap_tiles(&scene, 80, 60, 8).is_empty());
        assert!(runtime_command_minimap_tiles(&scene, 80, 60, 8).is_empty());
        assert_eq!(runtime_world_span_to_tile_span(f32::NAN, 12), 0);
        assert_eq!(runtime_world_span_to_tile_span(-4.0, 12), 0);
    }

    #[test]
    fn runtime_command_minimap_tiles_deduplicates_tiles_across_prefixes() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-command-build-target:1".to_string(),
                    layer: 29,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:runtime-command-position-target:2".to_string(),
                    layer: 29,
                    x: 24.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-command-unit-target:3".to_string(),
                    layer: 29,
                    x: 32.0,
                    y: 24.0,
                },
                RenderObject {
                    id: "marker:runtime-command-selected-unit:4".to_string(),
                    layer: 29,
                    x: 40.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "marker:runtime-command-building:5".to_string(),
                    layer: 29,
                    x: 8.0,
                    y: 8.0,
                },
            ],
        };

        assert_eq!(
            runtime_command_minimap_tiles(&scene, 80, 60, 8),
            vec![(1, 1), (3, 2), (4, 3), (5, 4)]
        );
    }

    #[test]
    fn runtime_ping_minimap_tile_prefers_latest_runtime_ping_marker() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:text:runtime-ping:1:text:6f6c64".to_string(),
                    layer: 31,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "terrain:sentinel".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:text:runtime-ping:9:text:6e6577".to_string(),
                    layer: 31,
                    x: 24.0,
                    y: 32.0,
                },
            ],
        };

        assert_eq!(runtime_ping_minimap_tile(&scene, 80, 60), Some((3, 4)));
    }

    #[test]
    fn minimap_collectors_remain_stable_when_input_order_changes() {
        let mut objects = vec![
            RenderObject {
                id: "world-label:10:text:6161".to_string(),
                layer: 39,
                x: 16.0,
                y: 0.0,
            },
            RenderObject {
                id: "world-label:11:text:6262".to_string(),
                layer: 39,
                x: 8.0,
                y: 16.0,
            },
            RenderObject {
                id: "world-label:12:text:6363".to_string(),
                layer: 39,
                x: 24.0,
                y: 8.0,
            },
            RenderObject {
                id: "world-label:13:text:6464".to_string(),
                layer: 39,
                x: 40.0,
                y: 16.0,
            },
            RenderObject {
                id: "marker:runtime-command-build-target:1".to_string(),
                layer: 29,
                x: 8.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-command-position-target:2".to_string(),
                layer: 29,
                x: 16.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-command-unit-target:3".to_string(),
                layer: 29,
                x: 24.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-command-selected-unit:4".to_string(),
                layer: 29,
                x: 8.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-command-building:5".to_string(),
                layer: 29,
                x: 32.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-unit-assembler-progress:1".to_string(),
                layer: 28,
                x: 8.0,
                y: 16.0,
            },
            RenderObject {
                id: "marker:runtime-unit-assembler-progress:2".to_string(),
                layer: 28,
                x: 24.0,
                y: 16.0,
            },
            RenderObject {
                id: "marker:runtime-unit-assembler-command:3".to_string(),
                layer: 28,
                x: 16.0,
                y: 16.0,
            },
            RenderObject {
                id: "marker:runtime-unit-block-spawn:1".to_string(),
                layer: 28,
                x: 8.0,
                y: 24.0,
            },
            RenderObject {
                id: "marker:runtime-landing-pad-landed:2".to_string(),
                layer: 28,
                x: 16.0,
                y: 24.0,
            },
            RenderObject {
                id: "marker:runtime-assembler-unit-spawned:3".to_string(),
                layer: 28,
                x: 24.0,
                y: 24.0,
            },
            RenderObject {
                id: "marker:text:runtime-ping:12:text:70696e67".to_string(),
                layer: 31,
                x: 40.0,
                y: 48.0,
            },
            RenderObject {
                id: "marker:text:runtime-ping:9:text:6f6c64".to_string(),
                layer: 31,
                x: 24.0,
                y: 16.0,
            },
        ];
        objects.extend(runtime_command_rect_objects(
            "runtime-command-rect",
            40.0,
            48.0,
            64.0,
            72.0,
        ));
        objects.extend(runtime_command_rect_objects(
            "runtime-command-target-rect",
            8.0,
            40.0,
            32.0,
            56.0,
        ));
        objects.extend(runtime_command_rect_objects(
            "runtime-break-rect",
            16.0,
            64.0,
            40.0,
            80.0,
        ));
        objects.extend(runtime_unit_assembler_area_objects(
            "tank-assembler",
            30,
            40,
            216.0,
            280.0,
            256.0,
            320.0,
        ));
        objects.extend([
            RenderObject {
                id: "marker:runtime-config-rollback:1:2:string".to_string(),
                layer: 24,
                x: 8.0,
                y: 8.0,
            },
            RenderObject {
                id: "marker:runtime-config:2:2:string".to_string(),
                layer: 24,
                x: 16.0,
                y: 16.0,
            },
            RenderObject {
                id: "terrain:runtime-deconstruct:17:18".to_string(),
                layer: 16,
                x: 136.0,
                y: 144.0,
            },
            RenderObject {
                id: "plan:runtime-place:0:4:4".to_string(),
                layer: 20,
                x: 32.0,
                y: 32.0,
            },
        ]);

        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: objects.clone(),
        };
        let mut reordered_scene = scene.clone();
        reordered_scene.objects.reverse();

        let world_labels = runtime_world_label_minimap_tiles(&scene, 80, 60, 3);
        let reordered_world_labels = runtime_world_label_minimap_tiles(&reordered_scene, 80, 60, 3);
        assert_eq!(world_labels, reordered_world_labels);
        assert_eq!(world_labels, vec![(2, 0), (3, 1), (1, 2)]);

        let command_tiles = runtime_command_minimap_tiles(&scene, 80, 60, 3);
        let reordered_command_tiles = runtime_command_minimap_tiles(&reordered_scene, 80, 60, 3);
        assert_eq!(command_tiles, reordered_command_tiles);
        assert_eq!(command_tiles, vec![(1, 1), (2, 1), (3, 1)]);

        let unit_assembler_tiles = runtime_unit_assembler_minimap_tiles(&scene, 80, 60, 2);
        let reordered_unit_assembler_tiles =
            runtime_unit_assembler_minimap_tiles(&reordered_scene, 80, 60, 2);
        assert_eq!(unit_assembler_tiles, reordered_unit_assembler_tiles);
        assert_eq!(unit_assembler_tiles, vec![(1, 2), (3, 2)]);

        let tile_action_tiles = runtime_tile_action_minimap_tiles(&scene, 80, 60, 2);
        let reordered_tile_action_tiles =
            runtime_tile_action_minimap_tiles(&reordered_scene, 80, 60, 2);
        assert_eq!(tile_action_tiles, reordered_tile_action_tiles);
        assert_eq!(tile_action_tiles, vec![(1, 3), (2, 3)]);

        let command_rects = runtime_command_minimap_rects(&scene, 80, 60, 2);
        let reordered_command_rects = runtime_command_minimap_rects(&reordered_scene, 80, 60, 2);
        assert_eq!(command_rects, reordered_command_rects);
        assert_eq!(
            command_rects,
            vec![
                WindowMinimapCommandRect {
                    origin_x: 5,
                    origin_y: 6,
                    width: 3,
                    height: 3,
                    kind: WindowMinimapCommandRectKind::Selection,
                },
                WindowMinimapCommandRect {
                    origin_x: 1,
                    origin_y: 5,
                    width: 3,
                    height: 2,
                    kind: WindowMinimapCommandRectKind::Target,
                },
            ]
        );

        let break_rects = runtime_break_minimap_rects(&scene, 80, 60, 1);
        let reordered_break_rects = runtime_break_minimap_rects(&reordered_scene, 80, 60, 1);
        assert_eq!(break_rects, reordered_break_rects);
        assert_eq!(
            break_rects,
            vec![WindowMinimapBreakRect {
                origin_x: 2,
                origin_y: 8,
                width: 3,
                height: 2,
            }]
        );

        let unit_assembler_rects = runtime_unit_assembler_minimap_rects(&scene, 80, 60, 1);
        let reordered_unit_assembler_rects =
            runtime_unit_assembler_minimap_rects(&reordered_scene, 80, 60, 1);
        assert_eq!(unit_assembler_rects, reordered_unit_assembler_rects);
        assert_eq!(
            unit_assembler_rects,
            vec![WindowMinimapUnitAssemblerRect {
                origin_x: 27,
                origin_y: 35,
                width: 5,
                height: 5,
            }]
        );

        assert_eq!(runtime_ping_minimap_tile(&scene, 80, 60), Some((5, 6)));
        assert_eq!(
            runtime_ping_minimap_tile(&reordered_scene, 80, 60),
            Some((5, 6))
        );
    }

    #[test]
    fn collect_stable_minimap_overlay_tiles_remains_deterministic() {
        let candidates = vec![
            StableMinimapOverlayTileCandidate {
                priority: 2,
                tile: (4, 4),
                kind: WindowMinimapRuntimeOverlayKind::Place,
                id: "plan:runtime-place:3".to_string(),
            },
            StableMinimapOverlayTileCandidate {
                priority: 0,
                tile: (1, 1),
                kind: WindowMinimapRuntimeOverlayKind::ConfigAlert,
                id: "marker:runtime-config-rollback:b".to_string(),
            },
            StableMinimapOverlayTileCandidate {
                priority: 0,
                tile: (1, 1),
                kind: WindowMinimapRuntimeOverlayKind::ConfigAlert,
                id: "marker:runtime-config-rollback:a".to_string(),
            },
            StableMinimapOverlayTileCandidate {
                priority: 1,
                tile: (2, 2),
                kind: WindowMinimapRuntimeOverlayKind::Config,
                id: "marker:runtime-config:2".to_string(),
            },
            StableMinimapOverlayTileCandidate {
                priority: 1,
                tile: (3, 2),
                kind: WindowMinimapRuntimeOverlayKind::Break,
                id: "terrain:runtime-deconstruct:3".to_string(),
            },
        ];
        let mut reversed = candidates.clone();
        reversed.reverse();

        let tiles = collect_stable_minimap_overlay_tiles(candidates, 3);
        let reversed_tiles = collect_stable_minimap_overlay_tiles(reversed, 3);

        assert_eq!(tiles, reversed_tiles);
        assert_eq!(
            tiles,
            vec![
                WindowMinimapRuntimeOverlayTile {
                    tile: (1, 1),
                    kind: WindowMinimapRuntimeOverlayKind::ConfigAlert,
                },
                WindowMinimapRuntimeOverlayTile {
                    tile: (2, 2),
                    kind: WindowMinimapRuntimeOverlayKind::Config,
                },
                WindowMinimapRuntimeOverlayTile {
                    tile: (3, 2),
                    kind: WindowMinimapRuntimeOverlayKind::Break,
                },
            ]
        );
    }

    #[test]
    fn fit_window_minimap_size_rejects_small_bounds_and_caps_scale() {
        assert_eq!(fit_window_minimap_size(0, 24, 128, 128), None);
        assert_eq!(fit_window_minimap_size(24, 0, 128, 128), None);
        assert_eq!(fit_window_minimap_size(24, 24, 0, 128), None);
        assert_eq!(fit_window_minimap_size(24, 24, 128, 0), None);
        assert_eq!(fit_window_minimap_size(24, 24, 11, 128), None);
        assert_eq!(fit_window_minimap_size(24, 24, 128, 11), None);
        assert_eq!(fit_window_minimap_size(1, 1, 12, 12), Some((4, 4)));
        assert_eq!(
            fit_window_minimap_size(100, 50, 1000, 1000),
            Some((400, 200))
        );
    }

    #[test]
    fn fit_window_minimap_size_preserves_exact_scale_boundary_rounding() {
        assert_eq!(fit_window_minimap_size(16, 16, 64, 64), Some((64, 64)));
        assert_eq!(fit_window_minimap_size(17, 10, 68, 39), Some((66, 39)));
    }

    #[test]
    fn runtime_world_to_minimap_tile_clamps_and_rejects_nonfinite_input() {
        assert_eq!(runtime_world_to_minimap_tile(f32::NAN, 8), 0);
        assert_eq!(runtime_world_to_minimap_tile(f32::INFINITY, 8), 0);
        assert_eq!(runtime_world_to_minimap_tile(f32::NEG_INFINITY, 8), 0);
        assert_eq!(runtime_world_to_minimap_tile(-1.0, 8), 0);
        assert_eq!(runtime_world_to_minimap_tile(16.0, 8), 2);
        assert_eq!(runtime_world_to_minimap_tile(64.0, 8), 7);
        assert_eq!(runtime_world_to_minimap_tile(16.0, 0), 0);
        assert_eq!(
            runtime_world_to_minimap_tile(16.0, (i32::MAX as usize) + 1),
            2
        );
    }

    #[test]
    fn runtime_world_to_minimap_tile_handles_unit_bounds_and_upper_edge() {
        assert_eq!(runtime_world_to_minimap_tile(0.0, 1), 0);
        assert_eq!(runtime_world_to_minimap_tile(999.0, 1), 0);
        assert_eq!(runtime_world_to_minimap_tile(0.0, 2), 0);
        assert_eq!(runtime_world_to_minimap_tile(7.999, 2), 0);
        assert_eq!(runtime_world_to_minimap_tile(8.0, 2), 1);
        assert_eq!(runtime_world_to_minimap_tile(999.0, 2), 1);
    }

    #[test]
    fn runtime_world_span_to_tile_span_clamps_upper_bound_and_handles_unit_bounds() {
        assert_eq!(runtime_world_span_to_tile_span(0.0, 1), 0);
        assert_eq!(runtime_world_span_to_tile_span(7.9, 1), 1);
        assert_eq!(runtime_world_span_to_tile_span(999.0, 1), 1);
        assert_eq!(runtime_world_span_to_tile_span(7.9, 2), 1);
        assert_eq!(runtime_world_span_to_tile_span(8.1, 2), 1);
        assert_eq!(runtime_world_span_to_tile_span(16.1, 2), 2);
        assert_eq!(runtime_world_span_to_tile_span(999.0, 2), 2);
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
        assert_eq!(frame.pixel(3, 0), Some(COLOR_RUNTIME));
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((4, 4)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
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
        assert_eq!(color_for_object(&render_object("unit:7")), COLOR_RUNTIME);
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
            "OVERLAY-KINDS: overlay:players=1 markers=2 plans=0 blocks=0 runtime=4 terrain=0 unknown=0",
        );
        assert_frame_line_contains(
            &frame.overlay_lines,
            "detail=marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1",
        );
        assert_frame_line_contains(
            &frame.overlay_lines,
            "OVERLAY-DETAIL: marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1",
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 2,
                        height: 2,
                    },
                },
            }),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let inset = frame.minimap_inset.as_ref().expect("minimap inset");
        let panel =
            super::build_minimap_panel(&scene, &hud, inset.window).expect("expected minimap panel");
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS: minikind:obj7@pl1:mk2:pn0:bk0:rt4:tr0:uk0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS-DETAIL: marker-line:1,marker-line-end:1,runtime-building:1,runtime-config:1,runtime-deconstruct:1,runtime-place:1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            &format!(
                "MINIMAP-EDGE: {}",
                super::compose_minimap_edge_summary_status_text(&panel)
            ),
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            &format!("MINIMAP-EDGE-DETAIL: {}", panel.edge_detail_label()),
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-DETAIL: minid:1/6:marker-line=1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            &format!(
                "MINIMAP-DETAIL: {}",
                super::compose_minimap_density_visibility_status_text(&panel)
            ),
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-WINDOW: miniwin:tracked=7:outside=0:player=1:marker=2:plan=0:block=0:runtime=4:terrain=0:unknown=0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-WINDOW-KINDS: window-kinds: tracked=7 outside=0 player=1 marker=2 plan=0 block=0 runtime=4 terrain=0 unknown=0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-LEGEND: legend:pl@/mkM/pnP/bk#/rtR/tr./uk?:vis=clear:ov1:fg0",
        );
    }

    #[test]
    fn present_once_surfaces_render_pipeline_layer_summary_for_visible_window() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend).with_max_view_tiles(2, 2);
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 32.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "terrain:0".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:line:7".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "player:focus".to_string(),
                    layer: 40,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "plan:build:1:3:3:257".to_string(),
                    layer: 20,
                    x: 24.0,
                    y: 24.0,
                },
                RenderObject {
                    id: "block:runtime-building:1:3:3".to_string(),
                    layer: 35,
                    x: 24.0,
                    y: 0.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-PIPELINE: pipe:tot5:vis3:clip2:ly3:span0..40:f1,1:w0,0+2x2:players=1 markers=1 plans=0 blocks=0 runtime=0 terrain=1 unknown=0",
        );
        assert_frame_line_contains(&frame.panel_lines, "RENDER-PIPELINE-DETAIL: marker-line:1");
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-LAYER: lay:1/3:l0:o1@pl0:mk0:pn0:bk0:rt0:tr1:uk0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-LAYER: lay:2/3:l30:o1@pl0:mk1:pn0:bk0:rt0:tr0:uk0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-LAYER-DETAIL: layd:2/3:l30:detail=marker-line:1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-LAYER: lay:3/3:l40:o1@pl1:mk0:pn0:bk0:rt0:tr0:uk0",
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
        assert!(frame.status_text.contains("zoom=2.00"));
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
        assert!(frame.status_text.contains("zoom=0.50"));
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
            }),
            runtime_ui: Some(RuntimeUiObservability {
                hud_text: RuntimeHudTextObservability {
                    set_count: 9,
                    set_reliable_count: 10,
                    hide_count: 11,
                    last_message: Some("hud text".to_string()),
                    last_reliable_message: Some("hud rel".to_string()),
                    announce_count: 12,
                    last_announce_message: Some("announce".to_string()),
                    info_message_count: 13,
                    last_info_message: Some("info".to_string()),
                    ..RuntimeHudTextObservability::default()
                },
                toast: RuntimeToastObservability {
                    info_count: 14,
                    warning_count: 15,
                    last_info_message: Some("toast".to_string()),
                    last_warning_text: Some("warn".to_string()),
                    info_popup_count: 16,
                    info_popup_reliable_count: 17,
                    last_info_popup_reliable: Some(true),
                    last_info_popup_id: Some("popup-a".to_string()),
                    last_info_popup_message: Some("popup text".to_string()),
                    last_info_popup_duration_bits: Some(2.5f32.to_bits()),
                    last_info_popup_align: Some(1),
                    last_info_popup_top: Some(2),
                    last_info_popup_left: Some(3),
                    last_info_popup_bottom: Some(4),
                    last_info_popup_right: Some(5),
                    clipboard_count: 18,
                    last_clipboard_text: Some("copied".to_string()),
                    open_uri_count: 19,
                    last_open_uri: Some("https://example.com".to_string()),
                    ..RuntimeToastObservability::default()
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
                    menu_choose_count: 29,
                    last_menu_choose_menu_id: Some(404),
                    last_menu_choose_option: Some(2),
                    text_input_result_count: 30,
                    last_text_input_result_id: Some(405),
                    last_text_input_result_text: Some("ok123".to_string()),
                    ..RuntimeMenuObservability::default()
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
                    last_control_group_operation: Some(
                        crate::RuntimeCommandRecentControlGroupOperationObservability::Recall,
                    ),
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
                    inactive_count: 1,
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
                markers: crate::hud_model::RuntimeMarkerObservability {
                    create_count: 54,
                    remove_count: 55,
                    update_count: 56,
                    update_text_count: 57,
                    update_texture_count: 58,
                    decode_fail_count: 2,
                    last_marker_id: Some(808),
                    last_control_name: Some("flushText".to_string()),
                },
                session: RuntimeSessionObservability {
                    bootstrap: RuntimeBootstrapObservability {
                        rules_label: "rules-hash-1".to_string(),
                        tags_label: "tags-hash-2".to_string(),
                        locales_label: "locales-hash-3".to_string(),
                        team_count: 2,
                        marker_count: 3,
                        custom_chunk_count: 4,
                        content_patch_count: 5,
                        player_team_plan_count: 6,
                        static_fog_team_count: 7,
                    },
                    core_binding: crate::RuntimeCoreBindingObservability {
                        kind: Some(
                            crate::RuntimeCoreBindingKindObservability::FirstCorePerTeamApproximation,
                        ),
                        ambiguous_team_count: 1,
                        ambiguous_team_sample: vec![1],
                        missing_team_count: 1,
                        missing_team_sample: vec![4],
                    },
                    resource_delta: RuntimeResourceDeltaObservability {
                        remove_tile_count: 80,
                        set_tile_count: 81,
                        set_floor_count: 82,
                        set_overlay_count: 83,
                        set_item_count: 22,
                        set_items_count: 23,
                        set_liquid_count: 24,
                        set_liquids_count: 25,
                        clear_items_count: 84,
                        clear_liquids_count: 85,
                        set_tile_items_count: 26,
                        set_tile_liquids_count: 27,
                        take_items_count: 1,
                        transfer_item_to_count: 2,
                        transfer_item_to_unit_count: 3,
                        last_kind: Some("to_unit".to_string()),
                        last_item_id: Some(6),
                        last_amount: None,
                        last_build_pos: None,
                        last_unit: Some(crate::RuntimeCommandUnitRefObservability {
                            kind: 2,
                            value: 808,
                        }),
                        last_to_entity_id: Some(404),
                        build_count: 2,
                        build_stack_count: 3,
                        entity_count: 1,
                        authoritative_build_update_count: 4,
                        delta_apply_count: 5,
                        delta_skip_count: 6,
                        delta_conflict_count: 7,
                        last_changed_build_pos: Some(999),
                        last_changed_entity_id: Some(900),
                        last_changed_item_id: Some(6),
                        last_changed_amount: Some(1),
                    },
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
                        local_owned_unit_entity_id: Some(202),
                        local_owned_unit_payload_count: Some(2),
                        local_owned_unit_payload_class_id: Some(5),
                        local_owned_unit_payload_revision: Some(7),
                        local_owned_unit_payload_body_len: Some(12),
                        local_owned_unit_payload_sha256: Some(
                            "0123456789abcdef0123456789abcdef".to_string(),
                        ),
                        local_owned_unit_payload_nested_descendant_count: Some(2),
                        local_owned_carried_item_id: Some(6),
                        local_owned_carried_item_amount: Some(4),
                        local_owned_controller_type: Some(4),
                        local_owned_controller_value: Some(101),
                    },
                    effect: crate::RuntimeLiveEffectSummaryObservability {
                        effect_count: 11,
                        spawn_effect_count: 73,
                        active_overlay_count: 1,
                        binding_label: Some(
                            "target:parent-follow/source:parent-follow".to_string(),
                        ),
                        binding_detail: Some(
                            "source=session session=target:parent-follow/source:parent-follow overlay=target:parent-follow/source:parent-follow active=1 target_counts=1/0/0 source_counts=1/0/0".to_string(),
                        ),
                        active_effect_id: Some(13),
                        active_contract_name: Some("lightning".to_string()),
                        active_reliable: Some(true),
                        active_position: Some(crate::RuntimeWorldPositionObservability {
                            x_bits: 28.0f32.to_bits(),
                            y_bits: 36.0f32.to_bits(),
                        }),
                        active_overlay_remaining_ticks: Some(3),
                        active_overlay_lifetime_ticks: Some(5),
                        last_effect_id: Some(8),
                        last_spawn_effect_unit_type_id: Some(19),
                        last_data_len: Some(9),
                        last_data_type_tag: Some(4),
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
        assert_eq!(
            frame.session_banner_text.as_deref(),
            Some("KICK idInUse@7:IdInUse:wait_for_old~")
        );
        assert_eq!(frame.overlay_summary_text.as_deref(), Some("Plans 1"));
        assert_eq!(
            frame.build_strip_text.as_deref(),
            Some("BUILD: sel=257 r2 q=flight@3 auth=rollback")
        );
        assert_eq!(
            frame.build_strip_detail_text.as_deref(),
            Some(
                "BUILD-STRIP-DETAIL: selected=257 rot=2 available=1 families=2 samples=2 top=message head=flight@100:99:place:b301:r1 authority=rollback pending=mismatch source=constructFinish tile=23:45 block=power-node orphan=1"
            )
        );
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
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INSPECTOR: message#1@18:40:len=5:text=hello;power-node#1@23:45:links=24:46|25:47",
        );
        assert!(frame
            .status_text
            .contains("ui:hud=9/10/11@hud_text/hud_rel"));
        assert!(frame
            .status_text
            .contains("live=ent=1/0@404:u2/999:p20.0:33.0:h0:s3"));
        assert!(frame.status_text.contains(
            "fx=11/73:ov1@13:u19:d9/4:kPoint2:clightning/lightning:bindtarget:parent-follow/source:parent-follow:r1:hpos:point2:3:4@1/0:pactive@28.0:36.0:ttl3/5"
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
            "HUD-VIS-DETAIL: hudvisd:s=mixed:ov=on:fg=on:k=144/4800:v=120/144:h=24/144:u=4656/4800",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "HUD-DETAIL: huddet:p=operator#8:sel=payload-rout~#14:t4800:vm2:hm0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-VIS: minivis:ov1:fg1:k144p3:v120p83m2:h24p16m0:u4656p97:d4@4800p0:w4@1p400:o0@4p0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "VIS-MINIMAP: overlay=1 fog=1 known=144(3%) vis=120(83%/2%) hid=24(16%/0%) map=80x60 window=0:0->0:0 size=1x1 cover=1/4800(0%) focus=0:0 in-window=1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-VIS-DETAIL: minivisd:v=mixed:c=offscreen:md0:wd400:od0:vp=warn",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-KINDS: minikind:obj4@pl1:mk1:pn1:bk1:rt0:tr0:uk0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-LEGEND: legend:pl@/mkM/pnP/bk#/rtR/tr./uk?:vis=mixed:ov1:fg1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG: cfgpanel:sel257:r2:m1:p1/2:hist3/4:o1:h=flight@100:99:place:b301:r1:align=split:auth=rollback:pm=mismatch:src=construct:b=power-node:fam2/2:more0:t2@message#1,power-node#1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-DETAIL: selected=257 rot=2 building=1 queued=1 inflight=2 pending=3 finished=3 removed=4 orphan=1 head=flight@100:99:place:b301:r1 align=split last=23:45 outcome=applied pm=mismatch source=constructFinish block=power-node families=2 samples=2 shown=2 more=0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: 1/2:message#1@18:40:len=5:text=hello",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: 2/2:power-node#1@23:45:links=24:46|25:47",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROLLBACK: cfgstrip:a3:rb1:last=23:45:src=construct:b1:cl1:lr1:pm=mismatch:out=applied:block=power-node",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROLLBACK-DETAIL: authoritative=3 rollback=1 last=23:45 source=constructFinish business=1 clear=1 last-rb=1 pending=mismatch outcome=applied block=power-node",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INTERACTION: cfgflow:m=place:s=head-diverged:q=mixed:p=3:pr=1:cfg=2/2:top=message:h=flight@100:99:place:b301:r1:auth=rollback:pm=mismatch:src=construct:t=23:45:b=power-node:o=1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INTERACTION-DETAIL: selected=257 rot=2 available=1 families=2 samples=2 top=message head=flight@100:99:place:b301:r1 authority=rollback pending=mismatch source=constructFinish tile=23:45 block=power-node orphan=1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-QUEUE: bqueue:q1:i2:f3:r4:o1:h=flight@100:99:place:b301:r1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-QUEUE-DETAIL: q=1 i=2 f=3 r=4 o=1 h=flight@100:99:place:b301:r1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-NOTICE: notice:hud=9/10/11@hud_text/hud_rel:ann=12@announce:info=13@info:toast=14/15@toast/warn:popup=16/17@1:popup-a/popup_text:clip=18@copied:uri=19@https_//exam~:https:tin=53@404:Digits/Only_numbers/12345#16:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-NOTICE-DETAIL: noticed:a1:h9/10/11:l8/7:ann12:a8:info13:i4:t14/15:l5/4:popup16/17:r1:pid7:pm10:pd1075838976:pb1:2:3:4:5:clip18:6:uri19:19:https:tin53:id404:t6:m12:d5:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-MENU: menu:m16@40:main/pick#2:3:fm17@41:follow/next#1:2:h18@41:tin53@404:Digits/12345#16:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-MENU-DETAIL: menud:a1:fo0:m40:4:4:2:3:fm41:6:4:1:2:hid41:tin53:id404:tDigits:d5:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-CHOICE: choice:mc29@404/2:tir30@405/ok123",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-CHOICE-DETAIL: choiced:mid404:opt2:rid405:rlen5",
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
            "RUNTIME-STACK-DEPTH: stackdepth:p2:n4:c1:m2:h4:d7:g3:t7",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK-DETAIL: stackd:f=input:g3:t7:p=input:m1:fo0:i53:n=warn:h1:r1:i1:w1:c1:7/8:sid404",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-DIALOG-STACK: stackx:f=input:p=input@input>menu:m16:fo0:i53:n=warn@hud>reliable>info>warn:md2:hd4:c1:7/8:tin404:s404:dd7:t7",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-COMMAND: cmd:act1:sel4@11,22,33:bld2@327686:rect-3:4:12:18:grp2#3@11,4#1@99:opgroup-recall:tb589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4:c5:s7/0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-COMMAND-DETAIL: cmdd:sample11,22,33:grp2#3@11,4#1@99:opgroup-recall:bld327686:rect-3:4:12:18:tb589834:u2:808:p0x42400000:0x42c00000:r1:2:3:4:c5:s7/0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-COMMAND-GROUP: cmdg:1/2:g2#3@11",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-COMMAND-GROUP: cmdg:2/2:g4#1@99",
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
            "RUNTIME-WORLD-LABEL: wlabel:set19:rel20:rm21:tot60:act2:inact1:last904:f3:fs1094713344@12.0:z1082130432@4.0:pos40.0:60.0:txtworld_label:l1:n11",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-WORLD-LABEL-DETAIL: wlabeld:set19:rel20:rm21:tot60:act2:in1:last904:f3:txt11x1:fs1094713344@12.0:z1082130432@4.0:p40.0:60.0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-MARKER: marker:cr54:rm55:up56:txt57:tex58:f2:last808:ctlflushText",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-MARKER-DETAIL: markerd:tot280:mut165:txt57:tex58:f2:last808:c9",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-SESSION: sess:bootstrap=rules=rules-hash-1:tags=tags-hash-2:locales=locales-hash-3:teams=2:markers=3:chunks=4:patches=5:plans=6:fog=7;cb=core:first-core-per-team:a1@1:m1@4;rd=resd:tile80/81/82/83:set22/23/24/25:clr84/85:tile26/27:flow1/2/3@to_unit:6:none:none:2:808:404:proj2/3/1:au4:d5/6/7:chg999/900/6/1;k=idInUse@7:IdInUse:wait_for_old~;l=defer5:replay6:drop7:qdrop8:sfail9:scfail10:efail11:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6;r=attempt3:redirect@1/127.0.0.1:6567:connectRedir~@none:server_reque~",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-SESSION-DETAIL: sessd:bootstrap(rules-label=rules-hash-1:tags-label=tags-hash-2:locales-label=locales-hash-3:team-count=2:marker-count=3:custom-chunk-count=4:content-patch-count=5:player-team-plan-count=6:static-fog-team-count=7):cb(cored:first-core-per-team:a1@1:m1@4):rd(resdd:rm80:st81:sf82:so83:set22/23/24/25:clr84/85:tile26/27:flow1/2/3:lastto_unit:6:none:none:2:808:404:proj2/3/1:au4:d5/6/7:chg999/900/6/1):k(kickd:r7:o7:c7:h20):l(loadingd:rdy12@1300:to2/1/1:ready@20000:rs3/1/1/1:reload:@lw1:cl0:rd1:cc0:p4:d5:r6):r(reconnectd:attempt#3:redirect:r15@none:h25:rd1@127.0.0.1:6567)",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-BOOTSTRAP: rules=rules-hash-1:tags=tags-hash-2:locales=locales-hash-3:teams=2:markers=3:chunks=4:patches=5:plans=6:fog=7",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-BOOTSTRAP-DETAIL: rules-label=rules-hash-1:tags-label=tags-hash-2:locales-label=locales-hash-3:team-count=2:marker-count=3:custom-chunk-count=4:content-patch-count=5:player-team-plan-count=6:static-fog-team-count=7",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RESOURCE-DELTA: resd:tile80/81/82/83:set22/23/24/25:clr84/85:tile26/27:flow1/2/3@to_unit:6:none:none:2:808:404:proj2/3/1:au4:d5/6/7:chg999/900/6/1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RESOURCE-DELTA-DETAIL: resdd:rm80:st81:sf82:so83:set22/23/24/25:clr84/85:tile26/27:flow1/2/3:lastto_unit:6:none:none:2:808:404:proj2/3/1:au4:d5/6/7:chg999/900/6/1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-KICK: kick:idInUse@7:IdInUse:wait_for_old~",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-KICK-DETAIL: kickd:r7:o7:c7:h20",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LOADING: loading:defer5:replay6:drop7:qdrop8:sfail9:scfail10:efail11:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LOADING-DETAIL: loadingd:rdy12@1300:to2/1/1:ready@20000:rs3/1/1/1:reload:@lw1:cl0:rd1:cc0:p4:d5:r6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-WORLD-RELOAD-DETAIL: reloadd:lw1:cl0:rd1:cc0:p4:d5:r6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-CORE-BINDING: core:first-core-per-team:a1@1:m1@4",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-CORE-BINDING-DETAIL: cored:first-core-per-team:a1@1:m1@4",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RECONNECT: reconnect:attempt3:redirect@1/127.0.0.1:6567:connectRedir~@none:server_reque~",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-RECONNECT-DETAIL: reconnectd:attempt#3:redirect:r15@none:h25:rd1@127.0.0.1:6567",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-ENTITY: liveent:1/0@404:u2/999:p20.0:33.0:h0:s3:tp1/0:last404/404/none",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-ENTITY-DETAIL: liveentd:local=404 unit=2/999 pos=20.0:33.0 hidden=0 seen=3 players=1 units=0 last=404/404/none owned=202 payload=count=2:unit=5/r7/l12:s0123456789ab nested=2 stack=6x4 controller=4/101",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-EFFECT: livefx:11/73:ov1@13:u19:d9/4:kPoint2:clightning/lightning:bindtarget:parent-follow/source:parent-follow:r1:hpos:point2:3:4@1/0:pactive@28.0:36.0:ttl3/5",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-LIVE-EFFECT-DETAIL: livefxd:hintpos:point2:3:4@1/0:srcactive:pos28.0:36.0:ttl3/5:data9/4:arel1:ctrlightning:rellightning:bindsource=session session=target:parent-follow/source:parent-follow overlay=target:parent-follow/source:parent-follow active=1 target_counts=1/0/0 source_counts=1/0/0",
        );
        assert_frame_line_contains(&frame.overlay_lines, "OVERLAY: Plans 1");
        assert_frame_line_contains(
            &frame.overlay_lines,
            "OVERLAY-KINDS: overlay:players=1 markers=1 plans=1 blocks=1 runtime=0",
        );
        let window_title = super::compose_window_title(frame, "demo-client");
        assert!(window_title.starts_with("demo-client/demo · Wave 7 · base hud:"));
        assert!(window_title.ends_with(" · Plans 1"));
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
                minimap: crate::hud_model::HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: crate::hud_model::HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 80,
                        height: 60,
                    },
                },
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
        assert_eq!(
            frame.build_strip_text.as_deref(),
            Some("BUILD: sel=301 r1 q=queued@3 auth=rej-miss-build")
        );
        assert_eq!(
            frame.build_strip_detail_text.as_deref(),
            Some(
                "BUILD-STRIP-DETAIL: selected=301 rot=1 available=1 families=3 samples=7 top=gamma head=queued@10:12:place:b301:r1 authority=rejected-missing-building pending=match source=tileConfig tile=10:12 block=gamma orphan=6"
            )
        );
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
            "MINIMAP-VIS-DETAIL: minivisd:v=unseen:c=offscreen:md0:wd75:od0:vp=warn",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG: cfgpanel:sel301:r1:m1:p2/1:hist4/5:o6:h=queued@10:12:place:b301:r1:align=match:auth=rej-miss-build:pm=match:src=tilecfg:b=gamma:fam3/3:more0:t7@gamma#4,beta#2,alpha#1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-DETAIL: selected=301 rot=1 building=1 queued=2 inflight=1 pending=3 finished=4 removed=5 orphan=6 head=queued@10:12:place:b301:r1 align=match last=10:12 outcome=rejected-missing-building pm=match source=tileConfig block=gamma families=3 samples=7 shown=3 more=0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: 1/3:gamma#4@four",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: 2/3:beta#2@two",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-CONFIG-ENTRY: 3/3:alpha#1@one",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROLLBACK: cfgstrip:a4:rb2:last=10:12:src=tilecfg:b1:cl0:lr0:pm=match:out=rej-miss-build:block=gamma",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROLLBACK-DETAIL: authoritative=4 rollback=2 last=10:12 source=tileConfig business=1 clear=0 last-rb=0 pending=match outcome=rejected-missing-building block=gamma",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INTERACTION: cfgflow:m=place:s=head-aligned:q=mixed:p=3:pr=1:cfg=3/7:top=gamma:h=queued@10:12:place:b301:r1:auth=rej-miss-build:pm=match:src=tilecfg:t=10:12:b=gamma:o=6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INTERACTION-DETAIL: selected=301 rot=1 available=1 families=3 samples=7 top=gamma head=queued@10:12:place:b301:r1 authority=rejected-missing-building pending=match source=tileConfig tile=10:12 block=gamma orphan=6",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-QUEUE: bqueue:q2:i1:f4:r5:o6:h=queued@10:12:place:b301:r1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-QUEUE-DETAIL: q=2 i=1 f=4 r=5 o=6 h=queued@10:12:place:b301:r1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-INSPECTOR: alpha#1@one;gamma#4@four;beta#2@two",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-MINIMAP-AUX: preb:m=place:s=head-aligned:q=mixed:r1:c=3/7@gamma:a=rej-miss-build:p=match:h=10:12:t=10:12:x=tilecfg:b=gamma:f=0:0@1:v0:u100:w0:d75:o3:rt0:rs0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-MINIMAP-DIAG: bmdiag:n=resolve:p=match:a=detached:f=inside:c=offscreen:v=unseen",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-MINIMAP-FLOW: bflow:n=resolve:s=head-aligned:q=mixed:r1:f=inside:c=offscreen:rt0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-MINIMAP-DETAIL: bmdetail:n=resolve:pair=match:a=detached:f=inside:v=unseen:c=offscreen:scope=multi:auth=rej-miss-build:pm=match:src=tilecfg:h=10:12:b=gamma:rt0:od75",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-FLOW: miniflow:n=survey:f=inside:p=hold:v=unseen:c=offscreen:t=player:o0",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "MINIMAP-FLOW-DETAIL: next=survey focus=inside vis=unseen cover=offscreen pan=hold target=player",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROUTE: cfgroute:n=resolve:m=survey:b2@resolve>survey:r3@resolve>survey>commit",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-ROUTE-DETAIL: next=resolve minimap=survey blockers=resolve+survey route=resolve+survey+commit",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-FLOW: cfgflow:n=resolve:m=survey:f=inside:p=hold:t=player:scope=multi:h=10:12:a=rej-miss-build:pm=match",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-FLOW-SUMMARY: next=resolve minimap=survey focus=inside pan=hold target=player scope=multi",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "BUILD-FLOW-DETAIL: next=resolve minimap=survey focus=inside pan=hold target=player scope=multi route=resolve+survey+commit authority=rejected-missing-building pending=match blockers=resolve+survey src=tileConfig block=gamma head=10,12",
        );
    }

    #[test]
    fn build_config_entry_status_lines_keep_extended_samples() {
        let sample = "abcdefghijklmnopqrstuvwxyz".repeat(3);
        let expected_sample = format!(
            "{}~",
            sample
                .chars()
                .take(super::WINDOW_BUILD_CONFIG_ENTRY_SAMPLE_LIMIT)
                .collect::<String>()
        );
        let hud = HudModel {
            build_ui: Some(BuildUiObservability {
                inspector_entries: vec![crate::BuildConfigInspectorEntryObservability {
                    family: "alpha".to_string(),
                    tracked_count: 1,
                    sample,
                }],
                ..BuildUiObservability::default()
            }),
            ..HudModel::default()
        };

        assert_eq!(
            super::compose_build_config_entry_status_lines(&hud),
            vec![format!("1/1:alpha#1@{expected_sample}")]
        );
    }

    #[test]
    fn build_config_entry_status_lines_emit_full_sorted_breakdown() {
        let hud = HudModel {
            build_ui: Some(BuildUiObservability {
                inspector_entries: vec![
                    crate::BuildConfigInspectorEntryObservability {
                        family: "gamma".to_string(),
                        tracked_count: 2,
                        sample: "g2".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "alpha".to_string(),
                        tracked_count: 4,
                        sample: "a4".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "beta".to_string(),
                        tracked_count: 4,
                        sample: "b4".to_string(),
                    },
                    crate::BuildConfigInspectorEntryObservability {
                        family: "delta".to_string(),
                        tracked_count: 1,
                        sample: "d1".to_string(),
                    },
                ],
                ..BuildUiObservability::default()
            }),
            ..HudModel::default()
        };

        assert_eq!(
            super::compose_build_config_entry_status_lines(&hud),
            vec![
                "1/4:alpha#4@a4".to_string(),
                "2/4:beta#4@b4".to_string(),
                "3/4:gamma#2@g2".to_string(),
                "4/4:delta#1@d1".to_string(),
            ]
        );
    }

    #[test]
    fn build_ui_inspector_status_text_keeps_extended_samples() {
        let sample = "abcdefghijklmnopqrstuvwxyz".repeat(3);
        let expected_sample = format!(
            "{}~",
            sample
                .chars()
                .take(super::WINDOW_BUILD_INSPECTOR_SAMPLE_LIMIT)
                .collect::<String>()
        );
        let build_ui = BuildUiObservability {
            inspector_entries: vec![crate::BuildConfigInspectorEntryObservability {
                family: "alpha".to_string(),
                tracked_count: 1,
                sample,
            }],
            ..BuildUiObservability::default()
        };

        assert_eq!(
            super::compose_build_ui_inspector_status_text(&build_ui),
            format!("alpha#1@{expected_sample}")
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
        assert_eq!(frame.session_banner_text, None);
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
                .all(|line| !line.starts_with("RUNTIME-SESSION-DETAIL:")),
            "unexpected runtime session detail line in {:?}",
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
                .all(|line| !line.starts_with("RUNTIME-COMMAND-DETAIL:")),
            "unexpected runtime command detail line in {:?}",
            frame.panel_lines
        );
        assert!(
            frame
                .panel_lines
                .iter()
                .all(|line| !line.starts_with("RUNTIME-COMMAND-GROUP:")),
            "unexpected runtime command group line in {:?}",
            frame.panel_lines
        );
        assert!(
            frame
                .panel_lines
                .iter()
                .all(|line| !line.starts_with("RUNTIME-DIALOG-STACK:")),
            "unexpected runtime dialog stack line in {:?}",
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
    fn present_once_reports_runtime_notice_state() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = runtime_stack_test_scene();
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.hud_text.set_count = 1;
        runtime_ui.hud_text.last_message = Some("hud".to_string());
        runtime_ui.hud_text.set_reliable_count = 1;
        runtime_ui.hud_text.last_reliable_message = Some("hud rel".to_string());
        runtime_ui.toast.info_count = 1;
        runtime_ui.toast.last_info_message = Some("info".to_string());
        runtime_ui.toast.warning_count = 1;
        runtime_ui.toast.last_warning_text = Some("warn".to_string());

        presenter
            .present_once(&scene, &runtime_stack_test_hud(runtime_ui))
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-NOTICE-STATE: notice-state:n=warn@warn:src=warn:layers=hud>reliable>info>warn:c4",
        );
    }

    #[test]
    fn present_once_reports_runtime_notice_state_detail() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = runtime_stack_test_scene();
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.hud_text.set_count = 1;
        runtime_ui.hud_text.last_message = Some("hud".to_string());
        runtime_ui.hud_text.set_reliable_count = 1;
        runtime_ui.hud_text.last_reliable_message = Some("hud rel".to_string());
        runtime_ui.toast.info_count = 1;
        runtime_ui.toast.last_info_message = Some("info".to_string());
        runtime_ui.toast.warning_count = 1;
        runtime_ui.toast.last_warning_text = Some("warn".to_string());

        presenter
            .present_once(&scene, &runtime_stack_test_hud(runtime_ui))
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-NOTICE-STATE-DETAIL: nstated:n=warn@warn:src=warn:c4:d4:l4:layers=hud>reliable>info>warn",
        );
    }

    #[test]
    fn present_once_keeps_reconnect_visible_when_loading_is_active() {
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
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.session.loading = crate::RuntimeLoadingObservability {
            deferred_inbound_packet_count: 5,
            replayed_inbound_packet_count: 6,
            dropped_loading_low_priority_packet_count: 7,
            dropped_loading_deferred_overflow_count: 8,
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
            ..crate::RuntimeLoadingObservability::default()
        };
        runtime_ui.session.reconnect = RuntimeReconnectObservability {
            phase: RuntimeReconnectPhaseObservability::Attempting,
            phase_transition_count: 3,
            reason_kind: Some(RuntimeReconnectReasonKind::ConnectRedirect),
            reason_text: Some("connectRedirect".to_string()),
            hint_text: Some("server requested redirect".to_string()),
            redirect_count: 1,
            last_redirect_ip: Some("127.0.0.1".to_string()),
            last_redirect_port: Some(6567),
            ..RuntimeReconnectObservability::default()
        };
        let hud = HudModel {
            title: "demo".to_string(),
            wave_text: Some("Wave 7".to_string()),
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!(
            frame.session_banner_text.as_deref(),
            Some(
                "RELOAD @lw1:cl0:rd1:cc0:p4:d5:r6 | RECONNECT attempt3:redirect@1/127.0.0.1:6567:connectRedir~@none:server_reque~ | LOADING defer5:replay6:drop7:qdrop8:sfail0:scfail0:efail0:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
            )
        );
        assert_eq!(
            window_hud_top_line(frame),
            frame.session_banner_text.as_deref()
        );
    }

    #[test]
    fn present_once_uses_loading_banner_when_reconnect_is_empty() {
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
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.session.loading = crate::RuntimeLoadingObservability {
            deferred_inbound_packet_count: 5,
            replayed_inbound_packet_count: 6,
            dropped_loading_low_priority_packet_count: 7,
            dropped_loading_deferred_overflow_count: 8,
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
            ..crate::RuntimeLoadingObservability::default()
        };
        let hud = HudModel {
            title: "demo".to_string(),
            wave_text: Some("Wave 7".to_string()),
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_eq!(
            frame.session_banner_text.as_deref(),
            Some(
                "RELOAD @lw1:cl0:rd1:cc0:p4:d5:r6 | LOADING defer5:replay6:drop7:qdrop8:sfail0:scfail0:efail0:rdy12@1300:to2:cto1:rto1:ltready@20000:rs3:rr1:wr1:kr1:lrreload:lwr@lw1:cl0:rd1:cc0:p4:d5:r6"
            )
        );
        assert_eq!(
            window_hud_top_line(frame),
            frame.session_banner_text.as_deref()
        );
    }

    #[test]
    fn present_once_reports_runtime_world_reload_summary() {
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
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.session.loading = crate::RuntimeLoadingObservability {
            deferred_inbound_packet_count: 5,
            replayed_inbound_packet_count: 6,
            dropped_loading_low_priority_packet_count: 7,
            dropped_loading_deferred_overflow_count: 8,
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
            ..crate::RuntimeLoadingObservability::default()
        };
        runtime_ui.session.reconnect = RuntimeReconnectObservability {
            phase: RuntimeReconnectPhaseObservability::Attempting,
            phase_transition_count: 3,
            reason_kind: Some(RuntimeReconnectReasonKind::ConnectRedirect),
            reason_text: Some("connectRedirect".to_string()),
            hint_text: Some("server requested redirect".to_string()),
            redirect_count: 1,
            last_redirect_ip: Some("127.0.0.1".to_string()),
            last_redirect_port: Some(6567),
            ..RuntimeReconnectObservability::default()
        };
        let hud = HudModel {
            title: "demo".to_string(),
            wave_text: Some("Wave 7".to_string()),
            runtime_ui: Some(runtime_ui),
            ..HudModel::default()
        };

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        let loading_detail_index = frame
            .panel_lines
            .iter()
            .position(|line| {
                line == "RUNTIME-LOADING-DETAIL: loadingd:rdy12@1300:to2/1/1:ready@20000:rs3/1/1/1:reload:@lw1:cl0:rd1:cc0:p4:d5:r6"
            })
            .unwrap();
        let world_reload_index = frame
            .panel_lines
            .iter()
            .position(|line| line == "RUNTIME-WORLD-RELOAD: @lw1:cl0:rd1:cc0:p4:d5:r6")
            .unwrap();
        let world_reload_detail_index = frame
            .panel_lines
            .iter()
            .position(|line| {
                line == "RUNTIME-WORLD-RELOAD-DETAIL: reloadd:lw1:cl0:rd1:cc0:p4:d5:r6"
            })
            .unwrap();
        assert!(loading_detail_index < world_reload_index);
        assert!(world_reload_index < world_reload_detail_index);
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
                "RUNTIME-STACK-DEPTH: stackdepth:p0:n0:c1:m0:h0:d1:g1:t1",
                "RUNTIME-STACK-DETAIL: stackd:f=chat:g1:t1:p=none:m0:fo0:i0:n=none:h0:r0:i0:w0:c1:1/2:sid42",
            ),
            (
                "menu-only",
                runtime_stack_test_hud(menu_only),
                "RUNTIME-STACK: stack:f=menu:p1@menu:n=none@none:c0:g1:t1:tinnone:snone",
                "RUNTIME-STACK-DEPTH: stackdepth:p1:n0:c0:m1:h0:d1:g1:t1",
                "RUNTIME-STACK-DETAIL: stackd:f=menu:g1:t1:p=menu:m1:fo0:i0:n=none:h0:r0:i0:w0:c0:0/0:sidnone",
            ),
            (
                "follow-up-without-text-input",
                runtime_stack_test_hud(follow_up_only),
                "RUNTIME-STACK: stack:f=follow-up:p1@follow-up:n=none@none:c0:g1:t1:tinnone:snone",
                "RUNTIME-STACK-DEPTH: stackdepth:p1:n0:c0:m1:h0:d1:g1:t1",
                "RUNTIME-STACK-DETAIL: stackd:f=follow-up:g1:t1:p=follow:m0:fo1:i0:n=none:h0:r0:i0:w0:c0:0/0:sidnone",
            ),
            (
                "text-input+notice+chat",
                runtime_stack_test_hud(input_notice_chat),
                "RUNTIME-STACK: stack:f=input:p1@input:n=warn@warn:c1:g3:t3:tin404:s404",
                "RUNTIME-STACK-DEPTH: stackdepth:p1:n1:c1:m1:h1:d3:g3:t3",
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

    #[test]
    fn present_once_drops_completed_prompt_history_from_stack_foreground() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.text_input.open_count = 1;
        runtime_ui.text_input.last_id = Some(404);
        runtime_ui.menu.menu_open_count = 1;
        runtime_ui.menu.last_menu_open_id = Some(11);
        runtime_ui.menu.menu_choose_count = 1;
        runtime_ui.menu.last_menu_choose_menu_id = Some(11);
        runtime_ui.menu.text_input_result_count = 1;
        runtime_ui.menu.last_text_input_result_id = Some(404);
        runtime_ui.chat.server_message_count = 1;
        runtime_ui.chat.chat_message_count = 2;
        runtime_ui.chat.last_chat_sender_entity_id = Some(42);

        presenter
            .present_once(
                &runtime_stack_test_scene(),
                &runtime_stack_test_hud(runtime_ui),
            )
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK: stack:f=chat:p0@none:n=none@none:c1:g1:t1:tin404:s42",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK-DEPTH: stackdepth:p0:n0:c1:m0:h0:d1:g1:t1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-STACK-DETAIL: stackd:f=chat:g1:t1:p=none:m0:fo0:i1:n=none:h0:r0:i0:w0:c1:1/2:sid42",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-DIALOG-STACK: stackx:f=chat:p=none@none:m1:fo0:i1:n=none@none:md0:hd0:c1:1/2:tin404:s42:dd1:t1",
        );
    }

    #[test]
    fn present_once_surfaces_runtime_prompt_rows() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let mut runtime_ui = RuntimeUiObservability::default();
        runtime_ui.text_input.open_count = 53;
        runtime_ui.text_input.last_id = Some(404);
        runtime_ui.text_input.last_title = Some("Digits".to_string());
        runtime_ui.text_input.last_message = Some("Only numbers".to_string());
        runtime_ui.text_input.last_default_text = Some("12345".to_string());
        runtime_ui.text_input.last_length = Some(16);
        runtime_ui.text_input.last_numeric = Some(true);
        runtime_ui.text_input.last_allow_empty = Some(true);
        runtime_ui.menu.menu_open_count = 16;
        runtime_ui.menu.follow_up_menu_open_count = 17;
        runtime_ui.menu.hide_follow_up_menu_count = 15;

        presenter
            .present_once(
                &runtime_stack_test_scene(),
                &runtime_stack_test_hud(runtime_ui),
            )
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-PROMPT: prompt:k=input:a1:d3:l=input>follow-up>menu:m16:fo2:tin53@404:Digits/Only_numbers/12345#16:n1:e1",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RUNTIME-PROMPT-DETAIL: promptd:ma1:fm17:fh15:fo2:tin53:id404:t6:m12:d5:n1:e1",
        );
    }

    #[test]
    fn runtime_ui_uri_scheme_rejects_empty_and_colonless_values() {
        for uri in ["", "noscheme", "://example.com"] {
            assert_eq!(super::runtime_ui_uri_scheme(Some(uri)), "none");
        }
        assert_eq!(
            super::runtime_ui_uri_scheme(Some("https://example.com")),
            "https"
        );
    }

    #[test]
    fn runtime_ui_uri_scheme_trims_whitespace_around_the_uri() {
        assert_eq!(
            super::runtime_ui_uri_scheme(Some("  https://example.com  ")),
            "https"
        );
    }

    #[test]
    fn present_once_surfaces_text_primitive_summary() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "world-label:7:text:48656c6c6f".to_string(),
                    layer: 39,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:text:8:text:4d61726b6572".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 0.0,
                },
            ],
        };
        let hud = HudModel::default();

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(&frame.panel_lines, "RENDER-TEXT: count=2");
        assert_frame_line_contains(&frame.panel_lines, "runtime-world-label@39:0:0=Hello");
        assert_frame_line_contains(&frame.panel_lines, "marker-text@30:8:0=Marker");
        assert_frame_line_contains(&frame.panel_lines, "RENDER-TEXT-DETAIL: count=2");
        assert_frame_line_contains(
            &frame.panel_lines,
            "marker-text@30:8:0 marker-text{text=Marker}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-world-label@39:0:0 runtime-world-label{text=Hello}",
        );
    }

    #[test]
    fn present_once_reports_render_text_overflow_count() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "world-label:7:text:48656c6c6f".to_string(),
                    layer: 39,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:text:8:text:4d61726b6572".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:shape-text:9:text:5368617065".to_string(),
                    layer: 31,
                    x: 16.0,
                    y: 0.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(&frame.panel_lines, "RENDER-TEXT: count=3");
        assert_frame_line_contains(&frame.panel_lines, "marker-text@30:8:0=Marker");
        assert_frame_line_contains(&frame.panel_lines, "marker-shape-text@31:16:0=Shape");
        assert_frame_line_contains(&frame.panel_lines, "more=1");
        assert_frame_line_contains(&frame.panel_lines, "RENDER-TEXT-DETAIL: count=3");
        assert_frame_line_contains(
            &frame.panel_lines,
            "marker-shape-text@31:16:0 marker-shape-text{text=Shape}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-world-label@39:0:0 runtime-world-label{text=Hello}",
        );
    }

    #[test]
    fn present_once_ignores_non_finite_text_primitives() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 16.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "world-label:9:text:48656c6c6f".to_string(),
                layer: 39,
                x: f32::NAN,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert!(!frame
            .panel_lines
            .iter()
            .any(|line| line.starts_with("RENDER-TEXT:")));
        assert!(!frame
            .panel_lines
            .iter()
            .any(|line| line.starts_with("RENDER-TEXT-DETAIL:")));
    }

    #[test]
    fn present_once_reports_render_line_overflow_count() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 32.0,
                height: 24.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:line:demo-a".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:line:demo-a:line-end".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:line:demo-b".to_string(),
                    layer: 31,
                    x: 0.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:line:demo-b:line-end".to_string(),
                    layer: 31,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:line:demo-c".to_string(),
                    layer: 32,
                    x: 0.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:line:demo-c:line-end".to_string(),
                    layer: 32,
                    x: 8.0,
                    y: 16.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(&frame.panel_lines, "RENDER-LINE: count=3");
        assert_frame_line_contains(&frame.panel_lines, "marker-line@30:0:0->1:0");
        assert_frame_line_contains(&frame.panel_lines, "marker-line@31:0:1->1:1");
        assert_frame_line_contains(&frame.panel_lines, "more=1");
        assert_frame_line_contains(&frame.panel_lines, "RENDER-LINE-DETAIL: count=3");
        assert_frame_line_contains(
            &frame.panel_lines,
            "marker-line@32:0:2->1:2 marker-line{marker_id=demo-c}",
        );
    }

    #[test]
    fn present_once_surfaces_rect_primitive_summary() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:top:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 32.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:top:{}:{}:{}:{}:line-end",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 48.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:right:{}:{}:{}:{}",
                        48.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        56.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 48.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:right:{}:{}:{}:{}:line-end",
                        48.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        56.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 48.0,
                    y: 56.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:bottom:{}:{}:{}:{}",
                        48.0f32.to_bits(),
                        56.0f32.to_bits(),
                        32.0f32.to_bits(),
                        56.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 48.0,
                    y: 56.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:bottom:{}:{}:{}:{}:line-end",
                        48.0f32.to_bits(),
                        56.0f32.to_bits(),
                        32.0f32.to_bits(),
                        56.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 32.0,
                    y: 56.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:left:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        56.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 32.0,
                    y: 56.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-target-rect:left:{}:{}:{}:{}:line-end",
                        32.0f32.to_bits(),
                        56.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 32.0,
                    y: 40.0,
                },
            ],
        };
        let hud = HudModel::default();

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-RECT: count=1 runtime-command-target-rect@29:32:40:48:56",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-RECT-DETAIL: count=1 runtime-command-target-rect@29:32:40:48:56 runtime-command-target-rect{left_tile=4,top_tile=5,right_tile=6,bottom_tile=7,width_tiles=2,height_tiles=2,line_count=4}",
        );
    }

    #[test]
    fn present_once_surfaces_runtime_break_rect_summary_and_pixels() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-break:0:2:3".to_string(),
                    layer: 31,
                    x: 16.0,
                    y: 24.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}",
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        24.0f32.to_bits(),
                        24.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 16.0,
                    y: 24.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}:line-end",
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        24.0f32.to_bits(),
                        24.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 24.0,
                    y: 24.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        24.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 24.0,
                    y: 24.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}:line-end",
                        24.0f32.to_bits(),
                        24.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        16.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}:line-end",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        16.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 16.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}",
                        16.0f32.to_bits(),
                        32.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 16.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}:line-end",
                        16.0f32.to_bits(),
                        32.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 16.0,
                    y: 24.0,
                },
            ],
        };
        let hud = HudModel::default();

        presenter.present_once(&scene, &hud).unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-RECT: count=1 runtime-break-rect@30:16:24:24:32",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-RECT-DETAIL: count=1 runtime-break-rect@30:16:24:24:32 runtime-break-rect{left_tile=2,top_tile=3,right_tile=3,bottom_tile=4,width_tiles=1,height_tiles=1,line_count=4}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=1 runtime-break/break@31:2:3",
        );
        assert_eq!(frame.pixel(2, 4), Some(COLOR_ICON_RUNTIME_BREAK));
        assert_eq!(frame.pixel(3, 4), Some(0xff44_88ff));
        assert_eq!(frame.pixel(3, 3), Some(0xff44_88ff));
        assert_eq!(frame.pixel(2, 3), Some(0xff44_88ff));
    }

    #[test]
    fn present_once_reports_render_rect_overflow_count() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let mut objects = Vec::new();
        objects.extend(runtime_command_rect_objects(
            "runtime-command-rect",
            8.0,
            16.0,
            24.0,
            32.0,
        ));
        objects.extend(
            runtime_command_rect_objects("runtime-break-rect", 32.0, 40.0, 40.0, 48.0)
                .into_iter()
                .map(|mut object| {
                    object.layer = 30;
                    object
                }),
        );
        objects.extend(
            runtime_command_rect_objects("runtime-command-target-rect", 48.0, 8.0, 56.0, 16.0)
                .into_iter()
                .map(|mut object| {
                    object.layer = 31;
                    object
                }),
        );
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects,
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(&frame.panel_lines, "RENDER-RECT: count=3");
        assert_frame_line_contains(&frame.panel_lines, "runtime-command-rect@29:8:16:24:32");
        assert_frame_line_contains(&frame.panel_lines, "runtime-break-rect@30:32:40:40:48");
        assert_frame_line_contains(&frame.panel_lines, "more=1");
        assert_frame_line_contains(&frame.panel_lines, "RENDER-RECT-DETAIL: count=3");
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-command-target-rect@31:48:8:56:16 runtime-command-target-rect{left_tile=6,top_tile=1,right_tile=7,bottom_tile=2,width_tiles=1,height_tiles=1,line_count=4}",
        );
    }

    #[test]
    fn present_once_surfaces_icon_primitive_summary_and_pixels() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-effect-icon:content-icon:normal:-1:6:9:0x00000000:0x00000000"
                        .to_string(),
                    layer: 31,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-build-config-icon:payload-source:1:0:1:7".to_string(),
                    layer: 32,
                    x: 8.0,
                    y: 0.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=2 runtime-effect-icon/content-icon@31:0:0 runtime-build-config-icon/payload-source@32:1:0",
        );
        assert_frame_line_contains(&frame.panel_lines, "RENDER-ICON-DETAIL: count=2");
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-effect-icon{variant=content-icon,content_id=9,content_type=6,delivery=normal,effect_id=-1,x_bits=0x00000000,y_bits=0x00000000}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-build-config-icon{variant=payload-source,content_id=7,content_type=1,tile_x=1,tile_y=0}",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_EFFECT));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_ICON_BUILD_CONFIG));
    }

    #[test]
    fn present_once_ignores_non_finite_icon_primitives() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-effect-icon:content-icon:normal:-1:6:9:0x00000000:0x00000000"
                    .to_string(),
                layer: 31,
                x: f32::INFINITY,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert!(!frame
            .panel_lines
            .iter()
            .any(|line| line.starts_with("RENDER-ICON:")));
        assert_ne!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_EFFECT));
    }

    #[test]
    fn present_once_surfaces_runtime_health_icon_primitive() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-health:0:0".to_string(),
                layer: 32,
                x: 0.0,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=1 runtime-health/health@32:0:0",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_HEALTH));
    }

    #[test]
    fn present_once_surfaces_runtime_command_icon_primitive() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-command-building:0:0".to_string(),
                layer: 29,
                x: 0.0,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=1 runtime-command/building@29:0:0",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_COMMAND));
    }

    #[test]
    fn present_once_surfaces_runtime_command_selected_unit_icon_primitive() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-command-selected-unit:22".to_string(),
                layer: 29,
                x: 0.0,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=1 runtime-command/selected-unit@29:0:0",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_COMMAND));
    }

    #[test]
    fn present_once_surfaces_runtime_place_icon_primitive() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "plan:runtime-place:0:8:9".to_string(),
                layer: 21,
                x: 0.0,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=1 runtime-place/place@21:0:0",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_PLAN));
    }

    #[test]
    fn present_once_surfaces_runtime_effect_marker_icon_primitive() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 8.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:runtime-effect:normal:13:0x41000000:0x41800000:1".to_string(),
                layer: 26,
                x: 0.0,
                y: 0.0,
            }],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=1 runtime-effect/normal@26:0:0",
        );
        assert_frame_line_contains(&frame.panel_lines, "RENDER-ICON-DETAIL: count=1");
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-effect{variant=normal,delivery=normal,effect_id=13,has_data=true,x_bits=0x41000000,y_bits=0x41800000}",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_EFFECT_MARKER));
    }

    #[test]
    fn present_once_surfaces_runtime_unit_assembler_icon_primitives() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 16.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:0:0x40800000".to_string(),
                    layer: 16,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-unit-assembler-command:tank-assembler:30:40:0x42200000:0x42700000".to_string(),
                    layer: 16,
                    x: 8.0,
                    y: 0.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=2 runtime-unit-assembler-progress/tank-assembler@16:0:0 runtime-unit-assembler-command/tank-assembler@16:1:0",
        );
        assert_frame_line_contains(&frame.panel_lines, "RENDER-ICON-DETAIL: count=2");
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-unit-assembler-progress{variant=tank-assembler,block_count=4,pay_rotation_bits=0x40800000,payload_present=false,progress_bits=0x3f400000,sample_id=9,sample_kind=b,sample_present=true,tile_x=30,tile_y=40,unit_count=2}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-unit-assembler-command{variant=tank-assembler,tile_x=30,tile_y=40,x_bits=0x42200000,y_bits=0x42700000}",
        );
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_UNIT_ASSEMBLER));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_ICON_RUNTIME_UNIT_ASSEMBLER));
    }

    #[test]
    fn present_once_surfaces_runtime_world_event_icon_primitives() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 40.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-break:0:3:4".to_string(),
                    layer: 14,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-bullet:1:17:4".to_string(),
                    layer: 28,
                    x: 8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-logic-explosion:2:2:0x42800000:1:1:0:1".to_string(),
                    layer: 28,
                    x: 16.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-sound-at:3:11".to_string(),
                    layer: 28,
                    x: 24.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-auto-door-toggle:4:3:4:1".to_string(),
                    layer: 28,
                    x: 32.0,
                    y: 0.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(
            &frame.panel_lines,
            "RENDER-ICON: count=5 runtime-break/break@14:0:0 runtime-bullet/bullet@28:1:0",
        );
        assert_frame_line_contains(&frame.panel_lines, "more=3");
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_RUNTIME_BREAK));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_ICON_RUNTIME_BULLET));
        assert_eq!(frame.pixel(2, 0), Some(COLOR_ICON_RUNTIME_LOGIC_EXPLOSION));
        assert_eq!(frame.pixel(3, 0), Some(COLOR_ICON_RUNTIME_SOUND_AT));
        assert_eq!(frame.pixel(4, 0), Some(COLOR_ICON_RUNTIME_TILE_ACTION));
    }

    #[test]
    fn present_once_surfaces_runtime_config_icon_primitives() {
        let backend = RecordingBackend::default();
        let mut presenter = WindowPresenter::new(backend);
        let scene = RenderModel {
            viewport: Viewport {
                width: 40.0,
                height: 8.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-config:0:0:string".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-parse-fail:1:0:int".to_string(),
                    layer: 31,
                    x: 8.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-noapply:2:0:content".to_string(),
                    layer: 32,
                    x: 16.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-rollback:3:0:unit".to_string(),
                    layer: 33,
                    x: 24.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config-pending-mismatch:4:0:payload".to_string(),
                    layer: 34,
                    x: 32.0,
                    y: 0.0,
                },
            ],
        };

        presenter
            .present_once(&scene, &HudModel::default())
            .unwrap();

        let backend = presenter.into_backend();
        let frame = backend.frames.last().unwrap();
        assert_frame_line_contains(&frame.panel_lines, "RENDER-ICON: count=5");
        assert_frame_line_contains(&frame.panel_lines, "RENDER-ICON-DETAIL: count=5");
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-config{variant=string,tile_x=0,tile_y=0}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-config-parse-fail{variant=int,tile_x=1,tile_y=0}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-config-noapply{variant=content,tile_x=2,tile_y=0}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-config-rollback{variant=unit,tile_x=3,tile_y=0}",
        );
        assert_frame_line_contains(
            &frame.panel_lines,
            "runtime-config-pending-mismatch{variant=payload,tile_x=4,tile_y=0}",
        );
        assert_frame_line_contains(&frame.panel_lines, "runtime-config/string@30:0:0");
        assert_frame_line_contains(&frame.panel_lines, "runtime-config-parse-fail/int@31:1:0");
        assert_eq!(frame.pixel(0, 0), Some(COLOR_ICON_BUILD_CONFIG));
        assert_eq!(frame.pixel(1, 0), Some(COLOR_ICON_BUILD_CONFIG));
        assert_eq!(frame.pixel(2, 0), Some(COLOR_ICON_BUILD_CONFIG));
        assert_eq!(frame.pixel(3, 0), Some(COLOR_ICON_BUILD_CONFIG));
        assert_eq!(frame.pixel(4, 0), Some(COLOR_ICON_BUILD_CONFIG));
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
