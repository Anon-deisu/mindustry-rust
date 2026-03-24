use crate::{SavePostLoadRuntimeSeedPlan, SavePostLoadWorldIssue, SavePostLoadWorldObservation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePostLoadConsumerStageKind {
    WorldShell,
    EntityRemaps,
    TeamPlans,
    Markers,
    StaticFog,
    CustomChunks,
    Buildings,
    LoadableEntities,
    SkippedEntities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerStage {
    pub kind: SavePostLoadConsumerStageKind,
    pub step_count: usize,
    pub deferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePostLoadConsumerBlocker {
    ContractIssue(SavePostLoadWorldIssue),
    DuplicateEntityId(i32),
    InvalidBuildingReference {
        center_index: usize,
        tile_index: usize,
        block_id: u16,
    },
    SkippedEntity {
        entity_index: usize,
        entity_id: i32,
        source_name: String,
        effective_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerApplyPlan {
    pub can_seed_runtime_apply: bool,
    pub stages: Vec<SavePostLoadConsumerStage>,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
}

impl SavePostLoadConsumerApplyPlan {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn total_step_count(&self) -> usize {
        self.stages.iter().map(|stage| stage.step_count).sum()
    }

    pub fn required_step_count(&self) -> usize {
        self.stages
            .iter()
            .filter(|stage| !stage.deferred)
            .map(|stage| stage.step_count)
            .sum()
    }

    pub fn deferred_step_count(&self) -> usize {
        self.stages
            .iter()
            .filter(|stage| stage.deferred)
            .map(|stage| stage.step_count)
            .sum()
    }
}

impl SavePostLoadWorldObservation {
    pub fn consumer_apply_plan(&self) -> SavePostLoadConsumerApplyPlan {
        self.runtime_seed_plan().consumer_apply_plan()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn consumer_apply_plan(&self) -> SavePostLoadConsumerApplyPlan {
        SavePostLoadConsumerApplyPlan {
            can_seed_runtime_apply: self.can_seed_runtime_apply(),
            stages: consumer_stages(self),
            blockers: consumer_blockers(self),
        }
    }
}

fn consumer_stages(plan: &SavePostLoadRuntimeSeedPlan) -> Vec<SavePostLoadConsumerStage> {
    vec![
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::WorldShell,
            step_count: 1,
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::EntityRemaps,
            step_count: plan.entity_remap_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::TeamPlans,
            step_count: plan.team_plan_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::Markers,
            step_count: plan.marker_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::StaticFog,
            step_count: usize::from(plan.static_fog_seed.is_some()),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::CustomChunks,
            step_count: plan.custom_chunk_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::Buildings,
            step_count: plan.building_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::LoadableEntities,
            step_count: plan.loadable_entity_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::SkippedEntities,
            step_count: plan.skipped_entity_seeds.len(),
            deferred: true,
        },
    ]
}

fn consumer_blockers(plan: &SavePostLoadRuntimeSeedPlan) -> Vec<SavePostLoadConsumerBlocker> {
    let mut blockers = Vec::new();

    blockers.extend(
        plan.contract
            .issues
            .iter()
            .copied()
            .map(SavePostLoadConsumerBlocker::ContractIssue),
    );
    blockers.extend(
        plan.activation
            .duplicate_entity_ids
            .iter()
            .copied()
            .map(SavePostLoadConsumerBlocker::DuplicateEntityId),
    );
    blockers.extend(
        plan.building_seeds
            .iter()
            .filter(|seed| !seed.activation.center_reference_valid)
            .map(
                |seed| SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: seed.activation.center_index,
                    tile_index: seed.activation.tile_index,
                    block_id: seed.activation.block_id,
                },
            ),
    );
    blockers.extend(plan.skipped_entity_seeds.iter().map(|seed| {
        SavePostLoadConsumerBlocker::SkippedEntity {
            entity_index: seed.entity_index,
            entity_id: seed.activation.entity_id,
            source_name: seed.activation.source_name.clone(),
            effective_name: seed.activation.effective_name.clone(),
        }
    }));

    blockers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, ContentHeaderEntry,
        CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail, ParsedCustomChunk,
        PointMarkerModel, SaveEntityChunkObservation, SaveEntityClassKind, SaveEntityClassSummary,
        SaveEntityPostLoadClassSummary, SaveEntityPostLoadKind, SaveEntityPostLoadSummary,
        SaveEntityRemapEntry, SaveEntityRemapSummary, SaveMapRegionObservation,
        SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam, TeamPlan, TeamPlanGroup,
        TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn consumer_apply_plan_tracks_required_and_deferred_steps() {
        let mut observation = test_observation();
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

        let plan = observation.consumer_apply_plan();

        assert!(plan.can_seed_runtime_apply);
        assert!(!plan.has_blockers());
        assert_eq!(
            plan.stages,
            vec![
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::WorldShell,
                    step_count: 1,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::EntityRemaps,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::TeamPlans,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::Markers,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::StaticFog,
                    step_count: 1,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::CustomChunks,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::Buildings,
                    step_count: 1,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::LoadableEntities,
                    step_count: 3,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::SkippedEntities,
                    step_count: 0,
                    deferred: true,
                },
            ]
        );
        assert_eq!(plan.total_step_count(), 14);
        assert_eq!(plan.required_step_count(), 14);
        assert_eq!(plan.deferred_step_count(), 0);
    }

    #[test]
    fn consumer_apply_plan_surfaces_contract_and_activation_blockers() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let plan = observation.consumer_apply_plan();

        assert!(!plan.can_seed_runtime_apply);
        assert!(plan.has_blockers());
        assert_eq!(plan.total_step_count(), 14);
        assert_eq!(plan.required_step_count(), 13);
        assert_eq!(plan.deferred_step_count(), 1);
        assert_eq!(
            plan.blockers,
            vec![
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                ),
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                ),
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::EntitySummaryMismatch,
                ),
                SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: 0,
                    tile_index: 0,
                    block_id: 0x0153,
                },
                SavePostLoadConsumerBlocker::SkippedEntity {
                    entity_index: 1,
                    entity_id: 43,
                    source_name: "mod-unit".to_string(),
                    effective_name: None,
                },
            ]
        );
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
                    parsed_tail: ParsedBuildingTail::Core(crate::CoreTailSnapshot {
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
