use crate::{
    SavePostLoadConsumerRuntimeDisposition, SavePostLoadConsumerRuntimeHelper,
    SavePostLoadConsumerStageKind, SavePostLoadRuntimeSeedPlan, SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePostLoadRuntimeApplyStep {
    WorldShell,
    EntityRemap {
        remap_index: usize,
    },
    TeamPlan {
        group_index: usize,
        plan_index: usize,
    },
    Marker {
        marker_index: usize,
    },
    StaticFog,
    CustomChunk {
        chunk_index: usize,
    },
    Building {
        center_index: usize,
    },
    LoadableEntity {
        entity_index: usize,
    },
    SkippedEntity {
        entity_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeApplyScript {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub apply_now_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub awaiting_world_shell_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub blocked_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub deferred_steps: Vec<SavePostLoadRuntimeApplyStep>,
}

impl SavePostLoadRuntimeApplyScript {
    pub fn apply_now_step_count(&self) -> usize {
        self.apply_now_steps.len()
    }

    pub fn awaiting_world_shell_step_count(&self) -> usize {
        self.awaiting_world_shell_steps.len()
    }

    pub fn blocked_step_count(&self) -> usize {
        self.blocked_steps.len()
    }

    pub fn deferred_step_count(&self) -> usize {
        self.deferred_steps.len()
    }

    pub fn total_step_count(&self) -> usize {
        self.apply_now_step_count()
            + self.awaiting_world_shell_step_count()
            + self.blocked_step_count()
            + self.deferred_step_count()
    }
}

impl SavePostLoadWorldObservation {
    pub fn runtime_apply_script(&self) -> SavePostLoadRuntimeApplyScript {
        self.runtime_seed_plan().runtime_apply_script()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn runtime_apply_script(&self) -> SavePostLoadRuntimeApplyScript {
        self.consumer_runtime_helper().runtime_apply_script(self)
    }
}

impl SavePostLoadConsumerRuntimeHelper {
    pub fn runtime_apply_script(
        &self,
        plan: &SavePostLoadRuntimeSeedPlan,
    ) -> SavePostLoadRuntimeApplyScript {
        let mut apply_now_steps = Vec::new();
        let mut awaiting_world_shell_steps = Vec::new();
        let mut blocked_steps = Vec::new();
        let mut deferred_steps = Vec::new();

        for stage in &self.stages {
            let target = match stage.disposition {
                SavePostLoadConsumerRuntimeDisposition::ApplyNow => &mut apply_now_steps,
                SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell => {
                    &mut awaiting_world_shell_steps
                }
                SavePostLoadConsumerRuntimeDisposition::Blocked => &mut blocked_steps,
                SavePostLoadConsumerRuntimeDisposition::Deferred => &mut deferred_steps,
            };
            expand_stage_steps(plan, stage.kind, target);
        }

        SavePostLoadRuntimeApplyScript {
            can_seed_runtime_apply: self.can_seed_runtime_apply,
            world_shell_ready: self.world_shell_ready,
            apply_now_steps,
            awaiting_world_shell_steps,
            blocked_steps,
            deferred_steps,
        }
    }
}

pub(crate) fn expand_stage_steps(
    plan: &SavePostLoadRuntimeSeedPlan,
    kind: SavePostLoadConsumerStageKind,
    out: &mut Vec<SavePostLoadRuntimeApplyStep>,
) {
    match kind {
        SavePostLoadConsumerStageKind::WorldShell => {
            out.push(SavePostLoadRuntimeApplyStep::WorldShell);
        }
        SavePostLoadConsumerStageKind::EntityRemaps => {
            out.extend(plan.entity_remap_seeds.iter().map(|seed| {
                SavePostLoadRuntimeApplyStep::EntityRemap {
                    remap_index: seed.remap_index,
                }
            }));
        }
        SavePostLoadConsumerStageKind::TeamPlans => {
            out.extend(plan.team_plan_seeds.iter().map(|seed| {
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: seed.group_index,
                    plan_index: seed.plan_index,
                }
            }));
        }
        SavePostLoadConsumerStageKind::Markers => {
            out.extend(
                plan.marker_seeds
                    .iter()
                    .map(|seed| SavePostLoadRuntimeApplyStep::Marker {
                        marker_index: seed.marker_index,
                    }),
            );
        }
        SavePostLoadConsumerStageKind::StaticFog => {
            if plan.static_fog_seed.is_some() {
                out.push(SavePostLoadRuntimeApplyStep::StaticFog);
            }
        }
        SavePostLoadConsumerStageKind::CustomChunks => {
            out.extend(plan.custom_chunk_seeds.iter().map(|seed| {
                SavePostLoadRuntimeApplyStep::CustomChunk {
                    chunk_index: seed.chunk_index,
                }
            }));
        }
        SavePostLoadConsumerStageKind::Buildings => {
            out.extend(plan.building_seeds.iter().map(|seed| {
                SavePostLoadRuntimeApplyStep::Building {
                    center_index: seed.activation.center_index,
                }
            }));
        }
        SavePostLoadConsumerStageKind::LoadableEntities => {
            out.extend(plan.loadable_entity_seeds.iter().map(|seed| {
                SavePostLoadRuntimeApplyStep::LoadableEntity {
                    entity_index: seed.entity_index,
                }
            }));
        }
        SavePostLoadConsumerStageKind::SkippedEntities => {
            out.extend(plan.skipped_entity_seeds.iter().map(|seed| {
                SavePostLoadRuntimeApplyStep::SkippedEntity {
                    entity_index: seed.entity_index,
                }
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, ContentHeaderEntry,
        CoreTailSnapshot, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
        ParsedCustomChunk, PointMarkerModel, SaveEntityChunkObservation, SaveEntityClassKind,
        SaveEntityClassSummary, SaveEntityPostLoadClassSummary, SaveEntityPostLoadKind,
        SaveEntityPostLoadSummary, SaveEntityRemapEntry, SaveEntityRemapSummary,
        SaveMapRegionObservation, StaticFogChunk, StaticFogTeam, TeamPlan, TeamPlanGroup,
        TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn runtime_apply_script_counts_align_with_runtime_helper() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let helper = observation.consumer_runtime_helper();
        let script = observation.runtime_apply_script();

        assert!(script.can_seed_runtime_apply);
        assert!(script.world_shell_ready);
        assert_eq!(script.apply_now_step_count(), helper.apply_now_step_count());
        assert_eq!(
            script.awaiting_world_shell_step_count(),
            helper.awaiting_world_shell_step_count()
        );
        assert_eq!(script.blocked_step_count(), helper.blocked_step_count());
        assert_eq!(script.deferred_step_count(), helper.deferred_step_count());
        assert_eq!(
            script.total_step_count(),
            observation.runtime_seed_plan().seed_step_count()
        );
    }

    #[test]
    fn runtime_apply_script_preserves_step_order_for_clean_seedable_plan() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let script = observation.runtime_apply_script();

        assert!(script.awaiting_world_shell_steps.is_empty());
        assert!(script.blocked_steps.is_empty());
        assert!(script.deferred_steps.is_empty());
        assert_eq!(
            script.apply_now_steps,
            vec![
                SavePostLoadRuntimeApplyStep::WorldShell,
                SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
                SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 1 },
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 0,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 1,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 0 },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 },
                SavePostLoadRuntimeApplyStep::StaticFog,
                SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 },
                SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 1 },
                SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 1 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 },
            ]
        );
    }

    #[test]
    fn runtime_apply_script_classifies_steps_by_runtime_disposition() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let helper = observation.consumer_runtime_helper();
        let script = observation.runtime_apply_script();

        assert!(!script.can_seed_runtime_apply);
        assert!(!script.world_shell_ready);
        assert_eq!(script.apply_now_step_count(), helper.apply_now_step_count());
        assert_eq!(
            script.awaiting_world_shell_step_count(),
            helper.awaiting_world_shell_step_count()
        );
        assert_eq!(script.blocked_step_count(), helper.blocked_step_count());
        assert_eq!(script.deferred_step_count(), helper.deferred_step_count());
        assert_eq!(
            script.blocked_steps,
            vec![
                SavePostLoadRuntimeApplyStep::WorldShell,
                SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 },
            ]
        );
        assert_eq!(
            script.awaiting_world_shell_steps,
            vec![
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 0,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 1,
                    plan_index: 0,
                },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 0 },
                SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 },
                SavePostLoadRuntimeApplyStep::StaticFog,
            ]
        );
        assert_eq!(
            script.apply_now_steps,
            vec![
                SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
                SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 1 },
                SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 },
                SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 1 },
            ]
        );
        assert_eq!(
            script.deferred_steps,
            vec![SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }]
        );
    }

    fn make_observation_seedable(observation: &mut SavePostLoadWorldObservation) {
        observation.world_entity_chunks[1].class_id = 3;
        observation.world_entity_chunks[1].custom_name = None;
        observation
            .entity_remap_summary
            .unresolved_effective_names
            .clear();
        observation.entity_summary.loadable_entities = 3;
        observation.entity_summary.skipped_entities = 0;
        observation.entity_summary.builtin_entities = 2;
        observation.entity_summary.custom_entities = 1;
        observation.entity_summary.class_summaries = vec![
            SaveEntityClassSummary {
                class_id: 3,
                kind: SaveEntityClassKind::Builtin,
                resolved_name: "flare".to_string(),
                count: 1,
            },
            SaveEntityClassSummary {
                class_id: 4,
                kind: SaveEntityClassKind::Builtin,
                resolved_name: "mace".to_string(),
                count: 1,
            },
            SaveEntityClassSummary {
                class_id: 255,
                kind: SaveEntityClassKind::Custom,
                resolved_name: "flare".to_string(),
                count: 1,
            },
        ];
        observation.entity_summary.post_load_class_summaries = vec![
            SaveEntityPostLoadClassSummary {
                source_class_ids: vec![3],
                effective_class_id: Some(3),
                kind: SaveEntityPostLoadKind::Builtin,
                resolved_name: "flare".to_string(),
                count: 1,
            },
            SaveEntityPostLoadClassSummary {
                source_class_ids: vec![4],
                effective_class_id: Some(4),
                kind: SaveEntityPostLoadKind::Builtin,
                resolved_name: "mace".to_string(),
                count: 1,
            },
            SaveEntityPostLoadClassSummary {
                source_class_ids: vec![255],
                effective_class_id: Some(3),
                kind: SaveEntityPostLoadKind::RemappedBuiltin,
                resolved_name: "flare".to_string(),
                count: 1,
            },
        ];
    }

    fn test_observation() -> SavePostLoadWorldObservation {
        SavePostLoadWorldObservation {
            save_version: 11,
            content_header: vec![ContentHeaderEntry {
                content_type: 1,
                names: vec!["core-nucleus".to_string(), "duo".to_string()],
            }],
            patches: vec![vec![0xaa, 0xbb]],
            map: SaveMapRegionObservation {
                floor_runs: 1,
                floor_region_bytes: vec![1],
                block_runs: 1,
                block_region_bytes: vec![2],
                world: test_world(),
            },
            entity_remap_entries: vec![
                SaveEntityRemapEntry {
                    custom_id: 255,
                    name: "flare".to_string(),
                },
                SaveEntityRemapEntry {
                    custom_id: 254,
                    name: "mod-unit".to_string(),
                },
            ],
            entity_remap_bytes: Vec::new(),
            entity_remap_summary: SaveEntityRemapSummary {
                remap_count: 2,
                unique_custom_ids: 2,
                duplicate_custom_ids: Vec::new(),
                unique_names: 2,
                duplicate_names: Vec::new(),
                effective_custom_ids: 1,
                resolved_builtin_custom_ids: vec![255],
                unresolved_effective_names: vec!["mod-unit".to_string()],
            },
            team_plan_groups: vec![
                TeamPlanGroup {
                    team_id: 1,
                    plan_count: 1,
                    plans: vec![TeamPlan {
                        x: 1,
                        y: 1,
                        rotation: 0,
                        block_id: 0x0101,
                        config: TypeIoValue::Null,
                        config_bytes: Vec::new(),
                        config_sha256: "plan-a".to_string(),
                    }],
                },
                TeamPlanGroup {
                    team_id: 2,
                    plan_count: 1,
                    plans: vec![TeamPlan {
                        x: 0,
                        y: 1,
                        rotation: 1,
                        block_id: 0x0102,
                        config: TypeIoValue::Integer(7),
                        config_bytes: vec![7],
                        config_sha256: "plan-b".to_string(),
                    }],
                },
            ],
            team_region_bytes: vec![3],
            world_entity_count: 3,
            world_entity_bytes: vec![4],
            world_entity_chunks: vec![
                SaveEntityChunkObservation {
                    chunk_len: 3,
                    chunk_bytes: vec![4, 5, 6],
                    chunk_sha256: "chunk-remap".to_string(),
                    class_id: 255,
                    custom_name: Some("flare".to_string()),
                    entity_id: 42,
                    body_len: 2,
                    body_bytes: vec![5, 6],
                    body_sha256: "entity-remap".to_string(),
                },
                SaveEntityChunkObservation {
                    chunk_len: 3,
                    chunk_bytes: vec![6, 7, 8],
                    chunk_sha256: "chunk-skip".to_string(),
                    class_id: 254,
                    custom_name: Some("mod-unit".to_string()),
                    entity_id: 43,
                    body_len: 2,
                    body_bytes: vec![7, 8],
                    body_sha256: "entity-skip".to_string(),
                },
                SaveEntityChunkObservation {
                    chunk_len: 3,
                    chunk_bytes: vec![8, 9, 10],
                    chunk_sha256: "chunk-builtin".to_string(),
                    class_id: 4,
                    custom_name: None,
                    entity_id: 44,
                    body_len: 2,
                    body_bytes: vec![9, 10],
                    body_sha256: "entity-builtin".to_string(),
                },
            ],
            markers: vec![
                MarkerEntry {
                    id: 11,
                    marker: MarkerModel::Point(PointMarkerModel {
                        class_tag: "Minimap".to_string(),
                        world: true,
                        minimap: true,
                        autoscale: false,
                        draw_layer_bits: 0.0f32.to_bits(),
                        x_bits: 8.0f32.to_bits(),
                        y_bits: 0.0f32.to_bits(),
                        radius_bits: 1.0f32.to_bits(),
                        stroke_bits: 1.0f32.to_bits(),
                        color: Some("ffffff".to_string()),
                    }),
                },
                MarkerEntry {
                    id: 12,
                    marker: MarkerModel::Point(PointMarkerModel {
                        class_tag: "Objective".to_string(),
                        world: true,
                        minimap: false,
                        autoscale: false,
                        draw_layer_bits: 0.0f32.to_bits(),
                        x_bits: 0.0f32.to_bits(),
                        y_bits: 8.0f32.to_bits(),
                        radius_bits: 1.5f32.to_bits(),
                        stroke_bits: 1.0f32.to_bits(),
                        color: Some("00ff00".to_string()),
                    }),
                },
            ],
            marker_region_bytes: b"{markers}".to_vec(),
            custom_chunks: vec![
                CustomChunkEntry {
                    name: "static-fog-data".to_string(),
                    chunk_len: 1,
                    chunk_bytes: vec![7],
                    chunk_sha256: "fog".to_string(),
                    parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                        used_teams: 2,
                        width: 2,
                        height: 2,
                        teams: vec![
                            StaticFogTeam {
                                team_id: 1,
                                run_count: 1,
                                rle_bytes: vec![8],
                                discovered: vec![true, false, true, true],
                            },
                            StaticFogTeam {
                                team_id: 2,
                                run_count: 1,
                                rle_bytes: vec![9],
                                discovered: vec![false, true, false, true],
                            },
                        ],
                    }),
                },
                CustomChunkEntry {
                    name: "mystery".to_string(),
                    chunk_len: 2,
                    chunk_bytes: vec![1, 2],
                    chunk_sha256: "mystery".to_string(),
                    parsed: ParsedCustomChunk::Unknown,
                },
            ],
            custom_region_bytes: vec![9],
            entity_summary: SaveEntityPostLoadSummary {
                total_entities: 3,
                unique_entity_ids: 3,
                duplicate_entity_ids: Vec::new(),
                builtin_entities: 1,
                custom_entities: 2,
                unknown_entities: 0,
                class_summaries: Vec::new(),
                loadable_entities: 2,
                skipped_entities: 1,
                post_load_class_summaries: Vec::new(),
            },
        }
    }

    fn test_world() -> WorldModel {
        let floors = vec![1, 1, 1, 1];
        let overlays = vec![0, 0, 0, 0];
        let blocks = vec![0x0153, 0, 0, 0];
        WorldModel {
            width: 2,
            height: 2,
            floors: floors.clone(),
            overlays: overlays.clone(),
            blocks: blocks.clone(),
            tiles: vec![
                TileModel {
                    tile_index: 0,
                    x: 0,
                    y: 0,
                    floor_id: floors[0],
                    overlay_id: overlays[0],
                    block_id: blocks[0],
                    building_center_index: Some(0),
                },
                TileModel {
                    tile_index: 1,
                    x: 1,
                    y: 0,
                    floor_id: floors[1],
                    overlay_id: overlays[1],
                    block_id: blocks[1],
                    building_center_index: None,
                },
                TileModel {
                    tile_index: 2,
                    x: 0,
                    y: 1,
                    floor_id: floors[2],
                    overlay_id: overlays[2],
                    block_id: blocks[2],
                    building_center_index: None,
                },
                TileModel {
                    tile_index: 3,
                    x: 1,
                    y: 1,
                    floor_id: floors[3],
                    overlay_id: overlays[3],
                    block_id: blocks[3],
                    building_center_index: None,
                },
            ],
            building_centers: vec![BuildingCenter {
                tile_index: 0,
                x: 0,
                y: 0,
                block_id: 0x0153,
                chunk_len: 3,
                chunk_bytes: vec![0, 1, 2],
                chunk_sha256: "center".to_string(),
                building: BuildingSnapshot {
                    revision: 0,
                    base_len: 0,
                    base: BuildingBaseSnapshot {
                        health_bits: 1.0f32.to_bits(),
                        rotation: 0,
                        team_id: 1,
                        legacy: false,
                        save_version: None,
                        enabled: None,
                        module_bitmask: None,
                        item_module: None,
                        power_module: None,
                        liquid_module: None,
                        time_scale_bits: None,
                        time_scale_duration_bits: None,
                        last_disabler_pos: None,
                        legacy_consume_connected: None,
                        efficiency: None,
                        optional_efficiency: None,
                        visible_flags: None,
                    },
                    tail_len: 0,
                    tail_bytes: Vec::new(),
                    tail_sha256: "tail".to_string(),
                    parsed_tail: ParsedBuildingTail::Core(CoreTailSnapshot {
                        command_pos_present: false,
                        command_pos_x_bits: 0,
                        command_pos_y_bits: 0,
                    }),
                },
            }],
            data_tiles: 1,
            team_count: 2,
            total_plans: 2,
            team_ids: vec![1, 2],
            team_plan_counts: vec![1, 1],
        }
    }
}
