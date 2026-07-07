# rust-mindustry 逐文件还原账本

更新日期：2026-07-07

## 约束

- 工作目录：`D:/MDT/rust-mindustry`
- 参考目录：`D:/MDT/mindustry-upstream-v157.4`
- 不读取工作目录既有文档；本文件作为新的迁移/验证账本维护。
- 文件读取按 UTF-8 处理。
- 命令通过 Git Bash 执行，当前显式使用 `C:/Program Files/Git/bin/bash.exe`。
- 不做防御性编程；按参考项目运行语义逐步还原。

## 当前验证基线

- `cargo metadata --no-deps --format-version 1`：通过。
- `cargo fmt --all`：通过。
- `cargo check --workspace --all-targets`：通过。
- `git diff --check`：通过。
- `cargo test -p mindustry-core about_dialog -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-core campaign_rules_dialog -- --nocapture`：通过，`7 passed; 0 failed`。
- `cargo test -p mindustry-core color_picker -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-core content_info_dialog -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-core database_dialog -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-core discord_dialog -- --nocapture`：通过，`2 passed; 0 failed`。
- `cargo test -p mindustry-core editor_maps_dialog -- --nocapture`：通过，`8 passed; 0 failed`。
- `cargo test -p mindustry-core file_chooser -- --nocapture`：通过，`8 passed; 0 failed`。
- `cargo test -p mindustry-core loadout_dialog -- --nocapture`：通过，`19 passed; 0 failed`。
- `cargo test -p mindustry-core map_list_dialog -- --nocapture`：通过，`9 passed; 0 failed`。
- `cargo test -p mindustry-core palette_dialog -- --nocapture`：通过，`2 passed; 0 failed`。
- `cargo test -p mindustry-core research_dialog -- --nocapture`：通过，`9 passed; 0 failed`。
- `cargo test -p mindustry-core schematics_dialog -- --nocapture`：通过，`7 passed; 0 failed`。
- `cargo test -p mindustry-core settings_menu_dialog -- --nocapture`：通过，`8 passed; 0 failed`。
- `cargo test -p mindustry-core mods_dialog -- --nocapture`：通过，`8 passed; 0 failed`。
- `cargo test -p mindustry-core custom_rules_dialog -- --nocapture`：通过，`10 passed; 0 failed`。
- `cargo test -p mindustry-core cubemap_mesh -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-core sector_launch_loadout_event_keeps_java_sector_from_loadout_fields -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-core planet_renderer -- --nocapture`：通过，`3 passed; 0 failed`。
- `cargo test -p mindustry-core planet_dialog -- --nocapture`：通过，`10 passed; 0 failed`。
- `cargo test -p mindustry-core g3d -- --nocapture`：通过，`32 passed; 0 failed`。
- `cargo test -p mindustry-core debug_collision -- --nocapture`：通过，`3 passed; 0 failed`。
- `cargo test -p mindustry-core intel_gpu -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-core nv_gpu -- --nocapture`：通过，`3 passed; 0 failed`。
- `cargo test -p mindustry-core ui -- --nocapture`：通过，`873 passed; 0 failed; 2 ignored`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_builds_planet_renderer_params_from_dialog_state -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_cursor_move_updates_hovered_sector_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_planet_surface_hover_uses_ray_picking_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_planet_surface_hover_label_projects_to_surface_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_planet_surface_hover_ -- --nocapture`：通过，`2 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_hovered_projection_uses_java_special_icon_branch -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_projection_debug_show_numbers_renders_preset_sector_ids_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_projection_layer_renders_all_visible_sectors_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_overprojection_draws_launcher_arc_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_overprojection_draws_import_export_arcs_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_overprojection_draws_shield_arcs_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_overprojection_draws_attack_arcs_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_route_ -- --nocapture`：通过，`25 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_launch_loadout_confirm_ -- --nocapture`：通过，`4 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_launch_loadout_ -- --nocapture`：通过，`12 passed; 0 failed`。
- `cargo test -p mindustry-core render_engine -- --nocapture`：通过，`13 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_graphics_opengl_backend_draw_texture_region_uses_explicit_uv_like_java_clouds -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_core_launch_cutscene_emits_space_pass_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_core_launch_cutscene_ -- --nocapture`：通过，`3 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_core_launch_cutscene_queues_core_land_dust_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_campaign_core_launch_cutscene_fades_foreground_like_java -- --nocapture`：通过，`1 passed; 0 failed`。
- `cargo test -p mindustry-desktop desktop_launcher_ticks_accelerator_launch_time_from_land_time_state -- --nocapture`：通过，`1 passed; 0 failed`。
- Workspace crate：
  - `mindustry-core`
  - `mindustry-server`
  - `mindustry-desktop`
  - `mindustry-android`
  - `mindustry-ios`
  - `mindustry-annotations`
  - `mindustry-tools`
  - `mindustry-tests`

## 参考项目 UI/游玩流程优先链

1. `Vars` / `Platform` / `ClientLauncher`
2. `UI` / `Styles` / `Fonts`
3. `MenuFragment` / `LoadingFragment` / `HudFragment`
4. `Renderer` / `MenuRenderer` / `LoadRenderer` / `MinimapRenderer`
5. 其余 dialogs / fragments / editor / graphics / g3d

## 当前代码映射观察

- `core/src/mindustry/ui` 已有共享 UI 基础：
  - `bar.rs`
  - `border_image.rs`
  - `displayable.rs`
  - `fonts.rs`
  - `items_display.rs`
  - `links.rs`
  - `menus.rs`
  - `minimap.rs`
  - `mobile_button.rs`
  - `styles.rs`
  - `warning_bar.rs`
  - `dialogs/base_dialog.rs`
  - `dialogs/full_text_dialog.rs`
  - `dialogs/keybind_dialog.rs`
  - `dialogs/language_dialog.rs`
  - `dialogs/map_locales_dialog.rs`
- 大量桌面端菜单、对话框、渲染流程集中在 `desktop/src/lib.rs`。
- `core/src/mindustry/graphics` 已覆盖多数基础渲染模块，`g3d/PlanetRenderer` 已提供后端无关 `PlanetScenePlan`。
- `desktop/src/lib.rs::push_campaign_route_page` 已将 campaign planet card 的视觉核接入 `PlanetRendererParams -> PlanetScenePlan`，并以 `planet-renderer-scene-plan` / `planet-renderer-scene-step` 自定义渲染命令保留 trace；`push_campaign_planet_scene_preview` 已按 `PlanetSceneStep` 生成可见 `DrawPixel` / `DrawCircle` / `DrawPolygon` / `DrawLine` / `DrawSprite` 投影 primitive；`CursorMoved` 已同步当前桌面可命中的 sector selector / sector list hover 到 `CampaignPlanetDialogState.hovered_sector_id`。
- `desktop/src/lib.rs` 已补 PlanetDialog 地表 hover 首段：`campaign_route_planet_scene_plan` 的 `cam_pos` 跟随 `selected_sector_id` 对应的 `PlanetGrid` tile，新增 `campaign_planet_surface_sector_id_at_surface_point` 用预览相机基向量把 surface point 还原为球面 ray pick，并接入 `campaign_hovered_sector_id_at_surface_point`；新增 `desktop_launcher_campaign_route_planet_surface_hover_uses_ray_picking_like_java` 覆盖中心 ray 命中 selected sector 且 `CursorMoved` 只更新 hover。
- `desktop/src/lib.rs` 已补 PlanetDialog hover label 投影：`campaign_hovered_sector_projected_label_like_java` 复用 `campaign_planet_surface_sector_preview_point` 将 `hoverLabel` 放到 hovered sector 的球面投影点，替代旧的 sector card 固定位置；`campaign_sector_hover_label_like_java` 的 selectable 名称优先走 runtime `Sector::name(...)`，覆盖 numbered sector 与 preset localized name；新增 `desktop_launcher_campaign_route_planet_surface_hover_label_projects_to_surface_like_java` 锁定投影位置。
- `desktop/src/lib.rs` 已补 PlanetDialog projection 全量 sector 层：`campaign_planet_projection_sector_ids_like_java` 优先按 `PlanetGrid::create(sector_size).tiles` 保持 `planet.sectors` 顺序，并 union preset/runtime sector id；`DrawSectors` 与 `RenderProjections` 不再依赖最多 5 个 `campaign_route_sector_selector_ids`，而是遍历当前 planet 可见半球 sectors；projection 分支复用 `campaign_sector_large_icon_source_like_java` 处理 warning/terrain/lock/preficon，并以 `circle-shadow` 作为无图标 numbered sector 的 preview primitive；新增 `desktop_launcher_campaign_route_projection_layer_renders_all_visible_sectors_like_java` 覆盖非 selector numbered sector 也会被渲染。
- `desktop/src/lib.rs` 已补 PlanetDialog hovered projection 特例：`RenderProjections` 普通 sector loop 跳过 hovered sector，新增 `campaign_hovered_projection_icon_source_like_java` 按上游 locked/warning/icon 顺序单独绘制 hovered icon；launchable numbered hovered sector 无 icon 时不再走旧的 `circle-shadow` projection sprite，新增 `desktop_launcher_campaign_route_hovered_projection_uses_java_special_icon_branch` 锁定该行为。
- `desktop/src/lib.rs` 已补 PlanetDialog debugShowNumbers 调试编号层：新增默认关闭的 `campaign_debug_show_numbers`，`RenderProjections` 在开启后按 `sec.hasEnemyBase() || sec.preset != null` 于 sector 投影点绘制 `Fonts.outline` 编号；新增 `desktop_launcher_campaign_route_projection_debug_show_numbers_renders_preset_sector_ids_like_java` 覆盖默认关闭与开启后 preset sector 白色编号。
- `desktop/src/lib.rs` 已补 PlanetDialog over-projection 首段 sector-to-sector 连线：`campaign_find_launcher_sector_id_like_java` 按当前 runtime/game sector 查找有基地 launcher，`campaign_push_planet_projected_sector_line_like_java` 用两个 sector 的球面投影点画线；`RenderOverProjections` 不再使用旧的 preview center -> marker 简化线，新增 `desktop_launcher_campaign_route_overprojection_draws_launcher_arc_like_java` 覆盖 launcher sector 到目标 numbered sector 的投影连线。
- `desktop/src/lib.rs` 已补 PlanetDialog over-projection import/export 连线：`campaign_sector_destination_matches_like_java` 按 runtime sector 与 preset sector id/original position 对齐 `SectorInfo.destination`，`RenderOverProjections` 在 selected 有基地时遍历当前 planet base entries，分别按 `sec.info.destination == selected && sec.info.anyExports()` 与 `selected.info.destination == sec && selected.info.anyExports()` 画 import/export sector-to-sector 投影线；新增 `desktop_launcher_campaign_route_overprojection_draws_import_export_arcs_like_java` 覆盖双向弧线。
- `desktop/src/lib.rs` 已补 PlanetDialog over-projection shield/attack 连线：`campaign_planet_shield_arc_sector_pairs_like_java` 由 `SectorPreset.shield_sector_ids` 反查上游 `shieldTarget` source→target；`campaign_planet_attack_arc_sector_pairs_like_java` 由 `CampaignRules.sector_invasion`、`PlanetGrid` 邻接和 `Sector::has_enemy_base(...)` 还原 enemy→base vulnerable arcs；新增 `desktop_launcher_campaign_route_overprojection_draws_shield_arcs_like_java` 与 `desktop_launcher_campaign_route_overprojection_draws_attack_arcs_like_java` 覆盖可见投影连线。
- `desktop/src/lib.rs` 已补 PlanetDialog loadout 确认后的普通 core launch cutscene 入口：新增 `DesktopLaunchAnimationSource` 区分 `CampaignCore`/`Accelerator`，`ConfirmCampaignLaunchLoadout` 成功后按上游 `CoreBlock.landDuration=160f` 与 `Interp.pow3(landZoomFrom=0.02, landZoomTo=4f)` 驱动共享 `LaunchAnimationState`，并尊重 `skipcoreanimation`；新增 `desktop_launcher_campaign_launch_loadout_confirm_triggers_cutscene_like_java` 与 `desktop_launcher_campaign_launch_loadout_confirm_respects_skip_core_animation_like_java`。
- `desktop/src/lib.rs` 已继续补 PlanetDialog loadout 确认后的 pending launch 语义：有 source/core 且未跳过动画时，先用 source sector 启动 playable smoke world 并把 destination sector 存入 `campaign_launch_pending_destination_sector`，`LaunchAnimationState` 结束时再消费 pending destination 切到目标 sector；`skipcoreanimation`/无 core 路径保持立即提交目标；新增 `desktop_launcher_campaign_launch_loadout_confirm_defers_destination_like_java` 覆盖 source=`groundZero`、destination=`saltFlats` 的延迟 commit。
- `core/src/mindustry/game/event_type.rs` 与 `desktop/src/lib.rs` 已补 PlanetDialog loadout 确认后的资源扣除/事件副作用：`SectorLaunchLoadoutEvent` 保留上游 `sector/from/loadout` 字段；`ConfirmCampaignLaunchLoadout` 在切入 `skipcoreanimation`/核心起飞动画分支前，按 `campaign_launch_loadout_stacks()` 从 source sector 扣除 schematic requirements + launch resources，并记录发射事件；动画分支传入已扣资源 source 快照，新增 `desktop_launcher_campaign_launch_loadout_confirm_deducts_source_items_and_fires_event_like_java`。
- `core/src/mindustry/graphics/render_engine.rs` 与 `desktop/src/lib.rs` 已补普通 `CoreBuild` launch 过场的 `Layer.space` 数据化渲染闭环：新增 `RenderPassKind::Space` / `RendererDrawStage::Space`，排序在 `Fog(90)` 与 `BlockOverdraw(100)` 之间；`CampaignCore` cutscene active 时按上游 `Renderer.draw(Layer.space)` / `CoreBlock.drawLaunch()` 生成 space pass，包含 100 条 `Pal.lightTrail` `DrawLine` 粒子、4 组队伍色/白色 `DrawCircle` 推进火焰、16 个 `core-*-thruster1/2` thruster region `DrawSprite`、`circle-shadow`、core `fullIcon` 旋转+white/accent mix，并按 `CoreBlock.lightRadius = 30 + 20 * size` 与 render-time `absin` 注入 lighting primitive；测试已断言不再依赖 `campaign-core-launch-*` custom marker。
- `core/src/mindustry/graphics/render_engine.rs`、`core/src/mindustry/graphics/menu_renderer.rs` 与 `desktop/src/lib.rs` 已补 `DrawTextureRegion` typed primitive：payload 包含 symbol/rect/uv/origin/tint/mix/rotation/layer，按 sprite 类命令进入 screen-visible、menu alpha/viewport filter、atlas resolver、OpenGL backend action、texture binding、sprite quad、execution trace 与 `DrawTextureRegion` trace kind；`CampaignCore` launch pass 已按上游 `cloudScaling=1700`、`cfinScl=-2`、`cfinOffset=0.3`、`calphaFinOffset=0.25`、`cloudAlpha=0.81`、`cloudAlphas=[0,0.5,1,0.1,0,0]` 接入 `sprites/clouds.png` UV/scroll clouds，并在 begin core launch 时初始化稳定 `cloudSeed`。
- `desktop/src/lib.rs` 已补 `CoreBlock.beginLaunch(launching=true)` 的末段 foreground fade：新增 `campaign_core_launch_fade_render_pass`，按上游 `margin=30f` 与 `Interp.pow2In` 在 `land_time <= 30` 时生成全屏黑色 `Ui` pass `FillRect`，并接入 `graphics_frame_for_render`；新增 `desktop_launcher_campaign_core_launch_cutscene_fades_foreground_like_java` 覆盖早期无 overlay、末段 alpha=0.25 与 frame pipeline 可见。
- `desktop/src/lib.rs` 已补 `CoreBlock.drawLanding` 末尾 `teamRegions[team.id]` final core sprite：新增 `block_team_region_symbol_like_java`，按上游 `Block.load()` 的 `name + "-team-" + team.name` 专属团队贴图规则查询 atlas，缺图或无 palette 时回退 `name + "-team"` 并套队伍色；`campaign_core_launching_render_pass` 在第二轮 landing thrusters 后、clouds 前追加最终 team sprite，默认 Sharded 命中 `core-shard-team-sharded` 白色贴图；`desktop_launcher_campaign_core_launch_cutscene_emits_space_pass_like_java` 已覆盖贴图、tint、rect/origin、rotation、layer 与命令顺序。
- `desktop/src/lib.rs` 已补 `CoreBlock.updateLaunch()` 的 `Fx.coreLandDust` 本地事件：新增 `DESKTOP_CAMPAIGN_CORE_THRUSTER_SIZES`、`desktop_mathf_sample_like_java` 与 `campaign_core_land_particle_timer`，在 `CampaignCore` launch tick 的 `should_update_launch` 分支按上游 `(landTimeIn * duration + 35) / duration` 采样 timer，满 1 后遍历核心 footprint linked tiles，以 40% 概率生成 `EffectCallPacket2(coreLandDust, TypeValue::Null)`；事件位置来自 tile `world_x/world_y`，rotation 为核心中心 `angleTo(tile) + range(30)`，颜色为 floor `map_color_rgba * (1.5 ± 0.15)`；新增 `desktop_launcher_campaign_core_launch_cutscene_queues_core_land_dust_like_java` 覆盖 effect id、linked tile 坐标、角度范围与颜色倍率。
- 后续继续补真实 OpenGL 3D backend 执行、完整 numbered sector 选择/面板、sector 展开/选区实绘，以及 `CoreBlock.drawLaunch/updateLaunch()` 的 landing fade-out 等剩余细化。

## UI/图形缺口清单

### UI 已逐文件补齐到 Rust 源码的类

- 顶层 `core/src/mindustry/ui`：
  - `Bar`
  - `BorderImage`（已补 `core/src/mindustry/ui/border_image.rs`）
  - `CoreItemsDisplay`
  - `Displayable`
  - `Fonts`
  - `GridImage`
  - `IntFormat`
  - `ItemsDisplay`（已补 `core/src/mindustry/ui/items_display.rs`）
  - `Links`（已补 `core/src/mindustry/ui/links.rs`）
  - `Menus`（已补 `core/src/mindustry/ui/menus.rs`）
  - `Minimap`（已补 `core/src/mindustry/ui/minimap.rs`）
  - `MobileButton`
  - `MultiReqImage`
  - `ReqImage`
  - `Styles`
  - `WarningBar`
- `fragments` 已补：
  - `BlockConfigFragment`
  - `BlockInventoryFragment`
  - `ChatFragment`（已补 `core/src/mindustry/ui/fragments/chat_fragment.rs`）
  - `ConsoleFragment`（已补 `core/src/mindustry/ui/fragments/console_fragment.rs`）
  - `FadeInFragment`
  - `HintsFragment`（已补 `core/src/mindustry/ui/fragments/hints_fragment.rs`）
  - `HudFragment`（已补 `core/src/mindustry/ui/fragments/hud_fragment.rs`）
  - `LoadingFragment`（已补 `core/src/mindustry/ui/fragments/loading_fragment.rs`）
  - `MenuFragment`（已补 `core/src/mindustry/ui/fragments/menu_fragment.rs`）
  - `MinimapFragment`
  - `PlacementFragment`（已补 `core/src/mindustry/ui/fragments/placement_fragment.rs`）
  - `PlanConfigFragment`
  - `PlayerListFragment`
  - `layout/BranchTreeLayout`
  - `layout/RadialTreeLayout`
  - `layout/RowTreeLayout`
  - `layout/TreeLayout`
- `dialogs` 已补：
  - `AdminsDialog`
  - `AboutDialog`（已补 `core/src/mindustry/ui/dialogs/about_dialog.rs`）
  - `BansDialog`
  - `CampaignCompleteDialog`
  - `CampaignRulesDialog`（已补 `core/src/mindustry/ui/dialogs/campaign_rules_dialog.rs`）
  - `CanvasEditDialog`（已补 `core/src/mindustry/ui/dialogs/canvas_edit_dialog.rs`）
  - `ColorPicker`（已补 `core/src/mindustry/ui/dialogs/color_picker.rs`）
  - `ContentInfoDialog`（已补 `core/src/mindustry/ui/dialogs/content_info_dialog.rs`）
  - `CustomRulesDialog`（已补 `core/src/mindustry/ui/dialogs/custom_rules_dialog.rs`）
  - `CustomGameDialog`（已补 `core/src/mindustry/ui/dialogs/custom_game_dialog.rs`）
  - `DatabaseDialog`（已补 `core/src/mindustry/ui/dialogs/database_dialog.rs`）
  - `DiscordDialog`（已补 `core/src/mindustry/ui/dialogs/discord_dialog.rs`）
  - `EditorMapsDialog`（已补 `core/src/mindustry/ui/dialogs/editor_maps_dialog.rs`）
  - `EffectsDialog`（已补 `core/src/mindustry/ui/dialogs/effects_dialog.rs`）
  - `FileChooser`（已补 `core/src/mindustry/ui/dialogs/file_chooser.rs`）
  - `GameOverDialog`（已补 `core/src/mindustry/ui/dialogs/game_over_dialog.rs`）
  - `HostDialog`（已补 `core/src/mindustry/ui/dialogs/host_dialog.rs`）
  - `IconSelectDialog`
  - `JoinDialog`（已补 `core/src/mindustry/ui/dialogs/join_dialog.rs`）
  - `LaunchLoadoutDialog`（已补 `core/src/mindustry/ui/dialogs/launch_loadout_dialog.rs`）
  - `LoadDialog`（已补 `core/src/mindustry/ui/dialogs/load_dialog.rs`）
  - `LoadoutDialog`（已补 `core/src/mindustry/ui/dialogs/loadout_dialog.rs`）
  - `MapListDialog`（已补 `core/src/mindustry/ui/dialogs/map_list_dialog.rs`）
  - `MapPlayDialog`（已补 `core/src/mindustry/ui/dialogs/map_play_dialog.rs`）
  - `ModsDialog`（已补 `core/src/mindustry/ui/dialogs/mods_dialog.rs`）
  - `PaletteDialog`（已补 `core/src/mindustry/ui/dialogs/palette_dialog.rs`）
  - `PausedDialog`（已补 `core/src/mindustry/ui/dialogs/paused_dialog.rs`）
  - `PlanetDialog`（已补首批核心纯模型 `core/src/mindustry/ui/dialogs/planet_dialog.rs`）
  - `ResearchDialog`（已补 `core/src/mindustry/ui/dialogs/research_dialog.rs`）
  - `SaveDialog`（已补 `core/src/mindustry/ui/dialogs/save_dialog.rs`）
  - `SchematicsDialog`（已补 `core/src/mindustry/ui/dialogs/schematics_dialog.rs`）
  - `SectorSelectDialog`（已补 `core/src/mindustry/ui/dialogs/sector_select_dialog.rs`）
  - `SettingsMenuDialog`（已补 `core/src/mindustry/ui/dialogs/settings_menu_dialog.rs`）
  - `TraceDialog`

### UI 类名仍未在 Rust 源码中找到明确实现痕迹

- 暂无明确 dialogs 类名缺口；后续转入 graphics/g3d 与更细粒度运行时/UI 行为对齐。

### dialogs 还原优先级

- P0 主流程类名已补完；后续继续深化 `PlanetDialog` 3D 渲染/载荷发射细节，并进入 P1 高频支撑对话框。
- P1 高频支撑已补完。
- P2 工具/信息型已补完：`CustomRulesDialog`。

### Graphics 类名未在 Rust 源码中找到明确实现痕迹

- 暂无当前清单内明确 graphics/g3d 类名缺口；后续转入更细粒度行为/渲染后端接入复核。

### Graphics/g3d 本轮已补齐到 Rust 源码的类

- `CubemapMesh`：已补 `core/src/mindustry/graphics/cubemap_mesh.rs`，覆盖上游固定 skybox cube 顶点、linear filter、`u_cubemap` slot 0、`u_proj`/triangles render plan 与 dispose 语义。
- `DebugCollisionRenderer`：已补 `core/src/mindustry/graphics/debug_collision_renderer.rs`，覆盖 hitbox square、solid tile boundary line、avoidance square、ground unit tile rect、unit physics circle 与 reset 分支。
- `IntelGpuCheck`：已补 `core/src/mindustry/graphics/intel_gpu_check.rs`，覆盖 Windows Intel vendor marker 写入/删除、非 Windows no-op 与 last launch cache 语义。
- `NvGpuInfo`：已补 `core/src/mindustry/graphics/nv_gpu_info.rs`，覆盖 `GL_NVX_gpu_memory_info` extension 一次性检测、total/current memory pname 查询与 unsupported 返回 0。
- `g3d/HexMesher`：已补 `core/src/mindustry/graphics/g3d/hex_mesher.rs`，覆盖默认 height/color/emissive/skip 语义、固定颜色 mesher、Vec3/Color 与噪声入口。
- `g3d/PlanetGrid`：已补 `core/src/mindustry/graphics/g3d/planet_grid.rs`，覆盖上游 tile/corner/edge 数量公式、初始 12 五边形与 subdivision 连接结构。
- `g3d/MeshBuilder`：已补 `core/src/mindustry/graphics/g3d/mesh_builder.rs`，覆盖 icosphere 计数、planet grid line mesh、hex indexed/non-indexed fan、height cache、color/emissive、skip 与 normal pack 位布局。
- `g3d/HexMesh`：已补 `core/src/mindustry/graphics/g3d/hex_mesh.rs`，覆盖默认 planet shader 构造、custom mesher/shader 构造与 planet shader preRender plan。
- `g3d/HexSkyMesh`：已补 `core/src/mindustry/graphics/g3d/hex_sky_mesh.rs`，覆盖 cloud mesher height/color/skip、`relRot=globalTime*speed/40`、`uiAlpha≈1` skip、cloud alpha 与 transform/preRender plan。
- `g3d/MatMesh`：已补 `core/src/mindustry/graphics/g3d/mat_mesh.rs`，覆盖 `transform * local mat` 包裹渲染与 dispose 转发。
- `g3d/MultiMesh`：已补 `core/src/mindustry/graphics/g3d/multi_mesh.rs`，覆盖子 mesh 顺序 render/dispose fan-out。
- `g3d/NoiseMesh`：已补 `core/src/mindustry/graphics/g3d/noise_mesh.rs`，覆盖单色/双色噪声 mesh、`7+seed` height、`8+seed` color、`5f` 坐标偏移与 `intensity=0.2`。
- `g3d/PlanetRenderer`：已补最小场景壳 `core/src/mindustry/graphics/g3d/planet_renderer.rs`，覆盖上游 `fov=60`、`far=150`、`projector scaling=1/150`、skybox/bloom/depth/cull/planet/clouds/sectors/atmosphere/orbit/interface projection 的数据化阶段顺序；`desktop/src/lib.rs::push_campaign_route_page` 已开始消费该 plan，并用 `PlanetSceneStep` 驱动可见 preview primitives；`CursorMoved` 已能通过 `PlanetGrid` surface ray picking 更新 PlanetDialog hover，hover label 已随 hovered sector 投到 planet preview 表面，`RenderProjections` 已扩展到全 planet visible sector layer、hovered special icon branch 与 debugShowNumbers 编号层，`RenderOverProjections` 已开始使用 sector-to-sector launcher/import/export/shield/attack 投影线；loadout 确认后的普通 core launch cutscene 已接入共享 `LaunchAnimationState`，并补 pending destination 延迟提交、资源扣除、`SectorLaunchLoadoutEvent` 与 `Layer.space` 数据化 launch pass，且粒子线、推进火焰、thruster region sprite、clouds UV/scroll、launch foreground fade、final team sprite、coreLandDust 已落成可见 primitive/事件；后续仍需接入完整扇区选择、landing fade-out 分支与 OpenGL 实绘。
- `g3d/SunMesh`：已补 `core/src/mindustry/graphics/g3d/sun_mesh.rs`，覆盖 zero height、simplex/pow/mag 离散 palette clamp 与 `Shaders.unlit`。

### 本轮继续细化

- `LaunchLoadoutDialog` / `LoadoutDialog` 资源编辑器：`desktop/src/lib.rs` 已按上游 `loadout.show(..., i -> i.unlocked() && i.isOnPlanet(sector.planet), ...)` 收敛资源候选项，使用出发 sector 的 planet 过滤可携带 item，避免全局 unlocked 的 Erekir 物品泄漏到 Serpulo 发射资源子弹窗；新增 `desktop_launcher_campaign_launch_resources_filters_items_to_source_planet_like_java` 覆盖手动解锁 `beryllium` 后仍被 Serpulo source planet 拒绝的路径。
- `PlanetDialog` newPresets：`desktop/src/lib.rs` 已补上游 `presetShow/showed/showing()` 的时间推进语义，20 tick 后用 `Iconc.lockOpen + [accent]sectorName` 触发 transient announce，同一 preset 只 announce 一次，超过 `sectorShowDuration=60*2.4` 后 pop 当前新 preset 并重置计时；进入/切换 PlanetDialog 时重置计时，保留既有 planetLaunch 下 sector list 隐藏逻辑；新增 `desktop_launcher_campaign_route_new_presets_announce_and_rotate_like_java` 覆盖 announce 与轮播。
- `PlanetDialog.updateSelected()` 选中 sector 面板：`core/src/mindustry/ui/dialogs/planet_dialog.rs` 已扩展 selected panel 纯模型，补 `underattack/vulnerable/enemybase` 的 Java else-if 顺序、locked objective/resource 字段与 `sectorInvasion` 上下文；`desktop/src/lib.rs` 已将 campaign sector 卡片从旧的 `showStats()` 汇总行切换到 `updateSelected()` 行，未受攻击有 base 的 sector 不再在卡片显示 threat/wave，受攻击 base 显示 `@sectors.underattack`，邻接敌方基地且开启 sector invasion 时显示 `@sectors.vulnerable`，无 base 敌方扇区显示 threat + `@sectors.enemybase`，资源与 stats 按钮仍按 Java 条件独立渲染；新增/更新 `selected_panel_detail_flags_follow_java_update_selected_else_if_order`、`desktop_launcher_campaign_route_selected_panel_status_rows_follow_update_selected_like_java`、`desktop_launcher_campaign_route_renders_planetdialog_like_sector_stats` 与 attack-arc 回归断言。
- `HudFragment` / `PlayerListFragment` 联机玩家列表入口：`core/src/mindustry/ui/fragments/hud_fragment.rs` 已补移动端按钮模型，按上游 `HudFragment.java` 在 `net.active()` 时将 pause 按钮切成 `Icon.players` 与 `TogglePlayerList`，离线才使用 pause/play；`desktop/src/lib.rs` 已新增 `PlayerListFragment` 状态与 `DesktopInputAction::TogglePlayerList`、`HudMobileButtonAction::TogglePlayerList` 的统一消费链，联网点击 players/键位会调用 `PlayerListFragment::toggle()`，关闭时清 search，`update()` 会按 `net.active() && state.isGame()` 自动隐藏；新增 `desktop_launcher_consumes_toggle_player_list_action_like_java`、`desktop_launcher_hud_mobile_players_action_toggles_list_without_pausing_like_java` 覆盖入口闭环。
- `PlayerListFragment.rebuild()` 服务端连接边界：按上游 `connection == null && net.server() && !user.isLocal()` 的提前 `return` 语义修正 Rust 模型，服务端遇到无连接远端玩家时停止构建但不显示 `players.notfound`；新增 `server_missing_connection_returns_without_not_found_like_java`。
- `PlayerListFragment` 底部菜单与玩家行动作模型：`core/src/mindustry/ui/fragments/player_list_fragment.rs` 已补 footer 三按钮模型，按上游顺序输出 `@server.bans`、`@server.admins`、`@close`，并在 client 模式禁用 bans/admins；玩家行补 `Spectate/OpenMenu/StartVoteKick` action，服务端/admin 菜单模型按 Java 顺序输出 `@player.ban`/`@player.kick`/`@player.trace`/`@player.team`/`@player.admin` 与 `@back`，保留 220x55 菜单按钮和 50px 队伍按钮常量；`desktop/src/lib.rs` 新增 `desktop_launcher_player_list_model_exposes_footer_and_row_menu_actions_like_java`，覆盖服务端远端连接玩家的 footer、spectate、menu、votekick 与 admin toggle uuid 链路。
- `ConsoleFragment` 移动端工具栏与 HUD terminal 入口：`core/src/mindustry/ui/fragments/console_fragment.rs` 已补 `ConsoleMobileToolbarModel`、`ConsoleMobileButtonModel`、`ConsoleMobileButtonAction`，按上游 mobile toolbar 固定输出 `chat/upOpen/downOpen/fileText/cancel`、`Styles.cleari`、58px 按钮、up/down disabled 与 `toggleMobile()` 的 `shown`/`open=false` 语义；`desktop/src/lib.rs` 已挂载 `ConsoleFragment` 状态，`HudMobileButtonAction::ToggleConsoleMobile` 与菜单 terminal chrome 共享 `toggle_mobile()` 状态，mobile terminal overlay 渲染改为消费 core toolbar model，并补五按钮 hit-test/action 链：chat 触发 `OpenMobileTextInput`、up/down 更新 `scrollPos`、fileText 记录 JS file chooser request、cancel 关闭 shown；新增 `mobile_toolbar_model_matches_java_icons_actions_and_disabled_state`、`mobile_toolbar_actions_scroll_file_and_cancel_like_java`、`mobile_text_input_accept_sends_and_hides_keyboard_like_java`、`desktop_launcher_hud_mobile_terminal_toggle_consumes_toggle_console_mobile_like_java`、`desktop_launcher_mobile_terminal_toolbar_actions_consume_like_java` 覆盖核心模型与 desktop 消费闭环。
- `ChatFragment` 桌面消费闭环：`core/src/mindustry/ui/mod.rs` 已补 `ChatFragment/ChatAction/ChatMode/ChatDrawMessage/CHAT_MESSAGES_SHOWN` 顶层导出；`desktop/src/lib.rs` 已挂载 `ChatFragment` 状态，新增 `toggle_chat_like_java()`、`finish_scheduled_chat_send_like_java()` 与 `accept_mobile_chat_text_like_java()`，按上游 `toggle()` 打开请求 keyboard、关闭安排 2 frame delayed send、移动端请求 `MAX_TEXT_LENGTH` 文本输入，发送后记录 `ClientChatEvent`、`SendChatMessage` 与 `x,y text` ping action；`HudMobileButtonAction::ToggleChat` 已接入移动聊天输入入口；新增 `desktop_launcher_chat_toggle_send_and_ping_actions_follow_java`、`desktop_launcher_hud_mobile_chat_action_requests_text_input_like_java` 覆盖 desktop 消费链。
- `ChatFragment.update()` 每帧输入驱动：`core/src/mindustry/ui/fragments/chat_fragment.rs` 已新增 `ChatUpdateFrame` 与 `update_like_java()`，补上游 `Binding.chat`、`chatHistoryPrev`、`chatHistoryNext`、`chatMode`、`chatScroll` 的组合调度，并把 `hasOtherFocus`、`lastFrameHadFocus` 与 `ui.consolefrag.shown()` 阻断条件真正接入 toggle gate；`desktop/src/lib.rs` 新增 `update_chat_like_java()`，自动用当前 `ConsoleFragment.shown()` 和本地 admin 状态补齐 frame 后消费 action；新增 `update_opens_only_when_net_active_no_focus_and_console_closed_like_java`、`update_navigates_history_mode_and_scroll_when_shown_like_java`、`desktop_launcher_update_chat_consumes_frame_and_console_gate_like_java` 覆盖 core 和 desktop 闭环。
- `ChatFragment.visible()/draw()` 断线清理与淡出：`core/src/mindustry/ui/fragments/chat_fragment.rs` 已补 `ChatVisibilityUpdate`、`update_visibility_like_java()` 与 `tick_fade_like_java()`，按上游 `visible()` 在 `!net.active && messages.size > 0` 时清空消息并在输入打开时 `hide()`，可见性返回 `net.active && hudfrag.shown`；按上游 `draw()` 在隐藏状态下执行 `fadetime -= delta / 180f`，避免旧聊天消息永久遮挡视野；`desktop/src/lib.rs::update()` 已改为用 `hud_fragment.shown` 驱动 `update_chat_visibility_like_java(...)`，并记录 `last_chat_visibility` 供桌面消费链验证，避免 HUD 翻转隐藏后联网聊天层仍被视为可见；新增 `visibility_update_clears_messages_and_hides_on_disconnect_like_java`、`fade_time_decays_only_when_hidden_like_java_draw`、`desktop_launcher_update_applies_chat_visibility_and_fade_like_java`、`desktop_launcher_update_chat_visibility_tracks_hud_shown_like_java` 覆盖 core/desktop 闭环。
- `MinimapFragment` 桌面消费链：`desktop/src/lib.rs` 已挂载 `MinimapFragment` 状态与 `last_minimap_actions`，补 `toggle_minimap_fragment_like_java()`、`update_minimap_fragment_like_java()`、`scroll_minimap_fragment_like_java()`、`key_down_minimap_ping_like_java()` 与 dispatch 记录，接通上游小地图 toggle 居中、每帧 keyboard/scroll 请求、菜单键隐藏、滚轮 zoom clamp、Ctrl ping 文本输入请求（`MAX_PING_TEXT_LENGTH`）等核心交互；新增 `desktop_launcher_consumes_minimap_fragment_actions_like_java`，并复跑 `cargo test -p mindustry-core minimap_fragment -- --nocapture`、`cargo test -p mindustry-desktop desktop_launcher_consumes_minimap_fragment_actions_like_java -- --nocapture`、`cargo check --workspace --all-targets` 与 `git diff --check` 通过。
- `HudFragment` 移动端按钮 desktop 消费链：`desktop/src/lib.rs` 已正式挂载 `HudFragment` 状态，补齐此前仍为 no-op 的 `HudMobileButtonAction::OpenPauseDialog/ToggleMenus/OpenSchematics/TogglePause/OpenResearch/OpenDatabase` 桌面消费逻辑，按上游 `HudFragment.java` 让 mobile menu 打开 paused overlay、flip 走 `toggle_menus()`、schematics/database/research 打开对应 route、离线 pause 切换 `State.paused/playing`；新增 `desktop_launcher_hud_mobile_menu_and_dialog_actions_consume_like_java`，并复跑 `cargo test -p mindustry-core hud_fragment -- --nocapture`、`cargo test -p mindustry-desktop desktop_launcher_hud_mobile -- --nocapture`、`cargo test -p mindustry-desktop desktop_launcher_paused_world_overlay_renders_mobile_branch_like_java -- --nocapture`、`cargo check --workspace --all-targets` 与 `git diff --check` 通过。
- `PausedDialog.shown()` 战役统计保存副作用：`core/src/mindustry/ui/dialogs/paused_dialog.rs` 新增 `shown_effects()` 与 `SaveCampaignStats` action，对齐上游 `shown(() -> { rebuild(); if(state.isCampaign()) state.getPlanet().saveStats(); })` 的 campaign gate；`core/src/mindustry/game/campaign_stats.rs` 新增 `to_settings_json_like_java()`，按 Java `CampaignStats` 字段名输出稳定 settings JSON；`desktop/src/lib.rs` 在移动端 `OpenPauseDialog` 与从 gameplay 首次进入暂停子 route 时消费该 action，将当前 `GameRuntime.campaign_stats` 写入 `"{planet}-campaign-stats"` settings mirror，非 campaign 不写入；新增 `campaign_stats_settings_json_uses_java_field_names_and_stable_maps`、`shown_effects_save_campaign_stats_only_in_campaign_like_java`、`desktop_launcher_paused_dialog_shown_saves_campaign_stats_like_java`、`desktop_launcher_paused_dialog_shown_skips_campaign_stats_outside_campaign_like_java`，并复跑相关 core/desktop 测试、`cargo check --workspace --all-targets` 与 `git diff --check` 通过。
- `CampaignCompleteDialog` 桌面消费链：`desktop/src/lib.rs` 已挂载 `CampaignCompleteDialogModel` 状态，补 `sector_capture_like_java()` 对齐上游 `Logic.sectorCapture()` 的 `waves=false`、首次占领标记、`attackMode=false`、`disableWorldProcessors=true` 与 `Control` 中 `!net.client() && preset.isLastSector && initialCapture` 的 `60f*2f` 延迟显示条件；暂停层新增 campaign complete modal 渲染/命中/dispatch，`@continue` 只关闭并恢复游戏，`@menu` 关闭后复用暂停退出保存链回菜单；总游玩时间按当前 planet 的 sector save slots 汇总进 core model。新增 `desktop_launcher_campaign_complete_dialog_schedules_after_last_sector_capture_like_java`、`desktop_launcher_campaign_complete_dialog_honors_java_capture_filters`、`desktop_launcher_campaign_complete_menu_button_runs_exit_save_like_java`，并复跑 `cargo test -p mindustry-desktop campaign_complete --lib -- --nocapture`、game over/paused 邻近回归、`cargo check --workspace --all-targets` 与 `git diff --check` 通过。
- `TraceDialog` / `PlayerListFragment` 玩家追踪桌面消费链：`desktop/src/lib.rs` 已把玩家菜单 `@player.trace` 接到 `AdminRequestCallPacket(AdminAction::Trace)` 客户端请求链，并在服务端/本地路径用当前 `PlayerComp/NetConnection` 直接构建 `TraceDialogModel`；`NetClientState.last_trace_info` 的 `TraceInfoCallPacket` 现在由 desktop 游标消费并打开 `TraceDialog`，packet 结果覆盖本地模型；复制行调用 `Platform::set_clipboard_text`，记录 `last_player_trace_clipboard_text` 并复用全局 `last_menu_info_message="@copied"`，不新增独立 toast 体系。新增 `desktop_launcher_player_list_menu_trace_opens_trace_dialog_like_java`、`desktop_launcher_player_list_trace_action_requests_admin_trace_like_java`、`desktop_launcher_trace_packet_opens_trace_dialog_like_java`、`desktop_launcher_trace_dialog_copy_sets_clipboard_and_info_message_like_java`，并复跑 `cargo test -p mindustry-core trace_dialog --lib -- --nocapture` 与相关 desktop 定向测试通过。
- `PlayerListFragment` footer close 与 ban/kick 管理确认链：`desktop/src/lib.rs` 新增 `dispatch_player_list_footer_action_like_java()`，`@close` 复用 `toggle_player_list_like_java()`，保持关闭时清 search 与 `last_player_list_model=None` 的 Java 行为；玩家菜单 `@player.ban`/`@player.kick` 现在先生成 `DesktopPlayerListAdminConfirm { title: "@confirm", message: confirmban/confirmkick }`，确认后再走统一 `AdminRequestCallPacket(AdminAction::Ban/Kick, TypeValue::Null)` 请求记录，取消不会覆盖上一次请求，符合上游 `ui.showConfirm(... Call.adminRequest ...)` 的确认后执行语义。新增 `desktop_launcher_player_list_footer_close_hides_fragment_like_java`、`desktop_launcher_player_list_ban_kick_confirm_sends_admin_request_like_java`，并复跑 `cargo test -p mindustry-core player_list_fragment --lib -- --nocapture`、相关 desktop player-list/trace 定向测试、`cargo check --workspace --all-targets` 与 `git diff --check` 通过。
- `PlayerListFragment` `@player.admin` 管理员切换确认链：`desktop/src/lib.rs` 将 ban/kick 确认状态泛化为 `DesktopPlayerListAdminConfirmAction`，继续让 `@player.ban`/`@player.kick` 在确认后发送 `AdminRequestCallPacket(AdminAction::Ban/Kick, TypeValue::Null)`，并把 `@player.admin` 接入上游 `confirmadmin/confirmunadmin` 文案与 `@confirm` 弹窗；确认后按 Java 的 `user.admin = true/false` 更新远端 `PlayerComp.admin`，同时记录 `DesktopPlayerAdminToggle { player_id, uuid, usid, admin }`，取消仍只清空确认态不触发副作用。新增 `desktop_launcher_player_list_admin_toggle_confirm_updates_remote_admin_like_java`，并复跑 `cargo test -p mindustry-core player_list_fragment --lib -- --nocapture`、`cargo test -p mindustry-desktop player_list_admin_toggle --lib -- --nocapture`、ban/kick/footer/trace/menu trace 定向测试、`cargo check --workspace --all-targets` 与 `git diff --check` 通过。
- `PlayerListFragment` footer `@server.bans` / `@server.admins` 桌面弹窗链：`desktop/src/lib.rs` 新增 `net_server_admins: Administration` 镜像与 `bans_dialog/admins_dialog` 当前弹窗模型，footer `ShowBans/ShowAdmins` 不再是 no-op，而是分别调用 `BansDialog::setup()` / `AdminsDialog::setup()`，每次打开都从当前 banned/admin 列表重建，保持上游 `shown(this::setup)` 语义；client 模式仍按 footer disabled 语义拒绝打开；`@player.admin` 确认后同步 `Administration::admin_player/unadmin_player`，保证管理员列表弹窗能看到最新授权状态。新增 `desktop_launcher_player_list_footer_show_bans_and_admins_open_dialogs_like_java`，并复跑 footer/admin toggle 定向测试通过。
- `PlayerListFragment` `@player.team` 二级队伍选择弹窗链：`desktop/src/lib.rs` 新增 `DesktopPlayerListTeamSelect` 与 6 个 `Team.baseTeams` 按钮模型，按上游 `BaseDialog(Core.bundle.get("player.team") + ": " + user.name)`、50px 按钮、三列换行、当前队伍 checked 和 team color 构建；`OpenTeamSelect` 不再返回 false，选择队伍后走统一 `AdminRequestCallPacket(AdminAction::SwitchTeam, TypeValue::Team(team_id))` 并关闭队伍选择状态，取消不发包；`last_player_admin_request_params` 记录请求参数用于验证。新增 `desktop_launcher_player_list_team_select_sends_switch_team_like_java`，并复跑 team-select/footer/admin-toggle 定向测试通过。

## 下一步

1. 继续补齐当前 UI 明确缺口：
   - dialogs 剩余 0 个明确类名文件；`desktop/src/lib.rs::push_campaign_route_page` 已接入 `g3d/PlanetRenderer` 场景壳；不要在 desktop 重写 `PlanetDialog` 状态机。
   - 已补 planet surface hover 的首段 ray picking、hover label 投影、projection 全量可见 sector 层、hovered special icon branch、debugShowNumbers 编号层、over-projection launcher/import/export/shield/attack 连线、newPresets transient announce/轮播、loadout 确认后的普通 core launch cutscene 入口、pending destination 延迟提交、资源扣除、`SectorLaunchLoadoutEvent` 与 `Layer.space` 数据化 launch pass、`CoreBlock.drawLaunch/updateLaunch()` clouds UV/scroll、final team sprite、coreLandDust 与 launch foreground fade；后续继续补完整 numbered sector 选择/面板、sector 展开/选区实绘与 landing fade-out 分支。
   - 高频 UI 行为复核顺序：`ConsoleFragment` → `PlayerListFragment` 视觉/菜单动作 → 其它 HUD 细节。
2. 继续复核 graphics/g3d 行为深度：
   - `simplex_noise3d` 当前是本地确定性入口，仍需后续与 Arc `Simplex.noise3d` 做数值级对照。
    - `PlanetRenderer` 场景壳已完成，桌面 campaign route 已消费 `PlanetScenePlan` 并生成可见 preview primitives，且已补 surface hover ray picking、hover label 投影、projection 全量可见 sector 层、hovered special icon branch、debugShowNumbers 编号层、over-projection launcher/import/export/shield/attack 连线、newPresets transient announce/轮播、loadout 确认后的普通 core launch cutscene 入口、pending destination 延迟提交、资源扣除、`SectorLaunchLoadoutEvent`、`Layer.space` 数据化 launch pass、clouds UV/scroll、final team sprite、coreLandDust 与 launch foreground fade；仍需 OpenGL backend 将 scene step / space pass 的 landing fade-out 等剩余细节真实落成 draw。
3. 对 `desktop/src/lib.rs` 中已有的菜单/HUD/对话框集中实现继续拆分映射，避免重复实现但保留逐文件 Rust 对应。
4. 跑桌面端最小启动/渲染路径验证，并继续保持 `cargo check --workspace --all-targets` 通过。
