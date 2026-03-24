use crate::{render_model::RenderObjectSemanticKind, BuildQueueHeadStage, HudModel, RenderModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenterViewWindow {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapPanelModel {
    pub map_width: usize,
    pub map_height: usize,
    pub window: PresenterViewWindow,
    pub window_last_x: usize,
    pub window_last_y: usize,
    pub window_tile_count: usize,
    pub map_tile_count: usize,
    pub known_tile_count: usize,
    pub focus_tile: Option<(usize, usize)>,
    pub overlay_visible: bool,
    pub fog_enabled: bool,
    pub visible_tile_count: usize,
    pub hidden_tile_count: usize,
    pub tracked_object_count: usize,
    pub player_count: usize,
    pub marker_count: usize,
    pub plan_count: usize,
    pub block_count: usize,
    pub runtime_count: usize,
    pub terrain_count: usize,
    pub unknown_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigPanelModel {
    pub selected_block_id: Option<i16>,
    pub selected_rotation: i32,
    pub building: bool,
    pub queued_count: usize,
    pub inflight_count: usize,
    pub pending_count: usize,
    pub finished_count: u64,
    pub removed_count: u64,
    pub orphan_authoritative_count: u64,
    pub tracked_family_count: usize,
    pub tracked_sample_count: usize,
    pub truncated_family_count: usize,
    pub selected_matches_head: Option<bool>,
    pub head: Option<BuildConfigHeadModel>,
    pub entries: Vec<BuildConfigPanelEntryModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigHeadModel {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub stage: BuildQueueHeadStage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfigPanelEntryModel {
    pub family: String,
    pub tracked_count: usize,
    pub sample: String,
}

pub fn build_minimap_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<MinimapPanelModel> {
    let summary = hud.summary.as_ref()?;
    let player_count = semantic_count(scene, RenderObjectSemanticKind::Player);
    let marker_count = semantic_count(scene, RenderObjectSemanticKind::Marker);
    let plan_count = semantic_count(scene, RenderObjectSemanticKind::Plan);
    let block_count = semantic_count(scene, RenderObjectSemanticKind::Block);
    let runtime_count = semantic_count(scene, RenderObjectSemanticKind::Runtime);
    let terrain_count = semantic_count(scene, RenderObjectSemanticKind::Terrain);
    let unknown_count = semantic_count(scene, RenderObjectSemanticKind::Unknown);
    let window_last_x = window
        .origin_x
        .saturating_add(window.width.saturating_sub(1));
    let window_last_y = window
        .origin_y
        .saturating_add(window.height.saturating_sub(1));
    let focus_tile = scene
        .objects
        .iter()
        .find(|object| object.semantic_kind() == RenderObjectSemanticKind::Player)
        .map(|object| {
            (
                world_to_tile_index_floor(object.x).max(0) as usize,
                world_to_tile_index_floor(object.y).max(0) as usize,
            )
        });

    Some(MinimapPanelModel {
        map_width: summary.map_width,
        map_height: summary.map_height,
        window,
        window_last_x,
        window_last_y,
        window_tile_count: window.width.saturating_mul(window.height),
        map_tile_count: summary.map_width.saturating_mul(summary.map_height),
        known_tile_count: summary
            .visible_tile_count
            .saturating_add(summary.hidden_tile_count),
        focus_tile,
        overlay_visible: summary.overlay_visible,
        fog_enabled: summary.fog_enabled,
        visible_tile_count: summary.visible_tile_count,
        hidden_tile_count: summary.hidden_tile_count,
        tracked_object_count: player_count
            .saturating_add(marker_count)
            .saturating_add(plan_count)
            .saturating_add(block_count)
            .saturating_add(runtime_count)
            .saturating_add(terrain_count)
            .saturating_add(unknown_count),
        player_count,
        marker_count,
        plan_count,
        block_count,
        runtime_count,
        terrain_count,
        unknown_count,
    })
}

pub fn build_build_config_panel(
    hud: &HudModel,
    max_entries: usize,
) -> Option<BuildConfigPanelModel> {
    let build_ui = hud.build_ui.as_ref()?;
    let mut entries = build_ui.inspector_entries.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .tracked_count
            .cmp(&left.tracked_count)
            .then_with(|| left.family.cmp(&right.family))
            .then_with(|| left.sample.cmp(&right.sample))
    });
    let tracked_family_count = entries.len();
    let tracked_sample_count = entries.iter().map(|entry| entry.tracked_count).sum();
    let capped_entries = entries
        .into_iter()
        .take(max_entries)
        .map(|entry| BuildConfigPanelEntryModel {
            family: entry.family.clone(),
            tracked_count: entry.tracked_count,
            sample: entry.sample.clone(),
        })
        .collect::<Vec<_>>();
    Some(BuildConfigPanelModel {
        selected_block_id: build_ui.selected_block_id,
        selected_rotation: build_ui.selected_rotation,
        building: build_ui.building,
        queued_count: build_ui.queued_count,
        inflight_count: build_ui.inflight_count,
        pending_count: build_ui
            .queued_count
            .saturating_add(build_ui.inflight_count),
        finished_count: build_ui.finished_count,
        removed_count: build_ui.removed_count,
        orphan_authoritative_count: build_ui.orphan_authoritative_count,
        tracked_family_count,
        tracked_sample_count,
        truncated_family_count: tracked_family_count.saturating_sub(capped_entries.len()),
        selected_matches_head: build_ui.head.as_ref().and_then(|head| {
            build_ui.selected_block_id.map(|selected_block_id| {
                head.block_id == Some(selected_block_id)
                    && head
                        .rotation
                        .map(|rotation| i32::from(rotation) == build_ui.selected_rotation)
                        .unwrap_or(true)
            })
        }),
        head: build_ui.head.as_ref().map(|head| BuildConfigHeadModel {
            x: head.x,
            y: head.y,
            breaking: head.breaking,
            block_id: head.block_id,
            rotation: head.rotation,
            stage: head.stage,
        }),
        entries: capped_entries,
    })
}

fn semantic_count(scene: &RenderModel, kind: RenderObjectSemanticKind) -> usize {
    scene
        .objects
        .iter()
        .filter(|object| object.semantic_kind() == kind)
        .count()
}

fn world_to_tile_index_floor(world_position: f32) -> i32 {
    if !world_position.is_finite() {
        return 0;
    }
    (world_position / 8.0).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::{build_build_config_panel, build_minimap_panel, PresenterViewWindow};
    use crate::{
        hud_model::HudSummary, BuildConfigInspectorEntryObservability, BuildQueueHeadObservability,
        BuildQueueHeadStage, BuildUiObservability, HudModel, RenderModel, RenderObject, Viewport,
    };

    #[test]
    fn builds_minimap_panel_from_summary_window_and_scene_semantics() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            objects: vec![
                RenderObject {
                    id: "player:focus".to_string(),
                    layer: 10,
                    x: 40.0,
                    y: 24.0,
                },
                RenderObject {
                    id: "marker:1".to_string(),
                    layer: 11,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "plan:2".to_string(),
                    layer: 12,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "block:3".to_string(),
                    layer: 13,
                    x: 16.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config:3:2:string".to_string(),
                    layer: 14,
                    x: 24.0,
                    y: 24.0,
                },
            ],
        };
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
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 2,
                origin_y: 1,
                width: 8,
                height: 7,
            },
        )
        .unwrap();

        assert_eq!(panel.map_width, 80);
        assert_eq!(panel.map_height, 60);
        assert_eq!(panel.window_last_x, 9);
        assert_eq!(panel.window_last_y, 7);
        assert_eq!(panel.window_tile_count, 56);
        assert_eq!(panel.map_tile_count, 4800);
        assert_eq!(panel.known_tile_count, 144);
        assert_eq!(panel.focus_tile, Some((5, 3)));
        assert_eq!(panel.tracked_object_count, 5);
        assert_eq!(panel.marker_count, 1);
        assert_eq!(panel.plan_count, 1);
        assert_eq!(panel.block_count, 1);
        assert_eq!(panel.runtime_count, 1);
        assert_eq!(panel.terrain_count, 0);
        assert_eq!(panel.unknown_count, 0);
    }

    #[test]
    fn builds_build_config_panel_with_capped_and_sorted_entries() {
        let hud = HudModel {
            build_ui: Some(BuildUiObservability {
                selected_block_id: Some(257),
                selected_rotation: 2,
                building: true,
                queued_count: 1,
                inflight_count: 2,
                finished_count: 3,
                removed_count: 4,
                orphan_authoritative_count: 5,
                head: Some(BuildQueueHeadObservability {
                    x: 10,
                    y: 11,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(1),
                    stage: BuildQueueHeadStage::InFlight,
                }),
                inspector_entries: vec![
                    BuildConfigInspectorEntryObservability {
                        family: "message".to_string(),
                        tracked_count: 1,
                        sample: "18:40:len=5:text=hello".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "power-node".to_string(),
                        tracked_count: 3,
                        sample: "23:45:links=24:46|25:47".to_string(),
                    },
                    BuildConfigInspectorEntryObservability {
                        family: "battery".to_string(),
                        tracked_count: 1,
                        sample: "20:41:cap=120".to_string(),
                    },
                ],
            }),
            ..HudModel::default()
        };

        let panel = build_build_config_panel(&hud, 2).unwrap();
        assert_eq!(panel.selected_block_id, Some(257));
        assert_eq!(panel.pending_count, 3);
        assert_eq!(panel.tracked_family_count, 3);
        assert_eq!(panel.tracked_sample_count, 5);
        assert_eq!(panel.truncated_family_count, 1);
        assert_eq!(panel.selected_matches_head, Some(false));
        assert_eq!(
            panel.head.as_ref().map(|head| head.stage),
            Some(BuildQueueHeadStage::InFlight)
        );
        assert_eq!(panel.entries.len(), 2);
        assert_eq!(panel.entries[0].family, "power-node");
        assert_eq!(panel.entries[1].family, "battery");
    }
}
