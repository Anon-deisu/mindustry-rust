use crate::save_post_load_activation::{
    activation_surface_from_contract, building_activation_candidate, entity_activation_candidate,
};
use crate::{
    BuildingSnapshot, ContentHeaderEntry, CustomChunkEntry, MarkerModel, ParsedCustomChunk,
    SaveEntityChunkObservation, SaveEntityRemapEntry, SavePostLoadActivationSurface,
    SavePostLoadBuildingActivationCandidate, SavePostLoadEntityActivationCandidate,
    SavePostLoadWorldContract, SavePostLoadWorldObservation, StaticFogTeam, TeamPlan, WorldModel,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeSeedPlan {
    pub contract: SavePostLoadWorldContract,
    pub activation: SavePostLoadActivationSurface,
    pub world_seed: SavePostLoadRuntimeWorldSeed,
    pub entity_remap_seeds: Vec<SavePostLoadRuntimeEntityRemapSeed>,
    pub team_plan_seeds: Vec<SavePostLoadRuntimeTeamPlanSeed>,
    pub marker_seeds: Vec<SavePostLoadRuntimeMarkerSeed>,
    pub static_fog_seed: Option<SavePostLoadRuntimeStaticFogSeed>,
    pub custom_chunk_seeds: Vec<SavePostLoadRuntimeCustomChunkSeed>,
    pub building_seeds: Vec<SavePostLoadRuntimeBuildingSeed>,
    pub loadable_entity_seeds: Vec<SavePostLoadRuntimeEntitySeed>,
    pub skipped_entity_seeds: Vec<SavePostLoadRuntimeEntitySeed>,
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn can_seed_runtime_apply(&self) -> bool {
        self.activation.can_seed_runtime_apply()
    }

    pub fn seed_step_count(&self) -> usize {
        1 + self.entity_remap_seeds.len()
            + self.team_plan_seeds.len()
            + self.marker_seeds.len()
            + usize::from(self.static_fog_seed.is_some())
            + self.custom_chunk_seeds.len()
            + self.building_seeds.len()
            + self.loadable_entity_seeds.len()
            + self.skipped_entity_seeds.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeWorldSeed {
    pub save_version: i32,
    pub content_header: Vec<ContentHeaderEntry>,
    pub patches: Vec<Vec<u8>>,
    pub world: WorldModel,
}

impl SavePostLoadRuntimeWorldSeed {
    pub fn tile_count(&self) -> usize {
        self.world.tile_count()
    }

    pub fn building_center_count(&self) -> usize {
        self.world.building_centers.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeEntityRemapSeed {
    pub remap_index: usize,
    pub custom_id: u16,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeTeamPlanSeed {
    pub group_index: usize,
    pub plan_index: usize,
    pub team_id: u32,
    pub plan: TeamPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeMarkerSeed {
    pub marker_index: usize,
    pub id: i32,
    pub kind_name: &'static str,
    pub class_tag: Option<String>,
    pub tile_coords: Option<(i16, i16)>,
    pub marker: MarkerModel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadRuntimeStaticFogSeed {
    pub source_chunk_name: String,
    pub source_chunk_sha256: String,
    pub width: usize,
    pub height: usize,
    pub teams: Vec<SavePostLoadRuntimeStaticFogTeamSeed>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeStaticFogTeamSeed {
    pub team_index: usize,
    pub team_id: u8,
    pub run_count: usize,
    pub discovered_count: usize,
    pub discovered_indices: Vec<u32>,
    pub discovered: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeCustomChunkSeed {
    pub chunk_index: usize,
    pub name: String,
    pub chunk_len: usize,
    pub chunk_sha256: String,
    pub chunk_bytes: Vec<u8>,
    pub parsed: ParsedCustomChunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeBuildingSeed {
    pub activation: SavePostLoadBuildingActivationCandidate,
    pub chunk_len: usize,
    pub chunk_sha256: String,
    pub chunk_bytes: Vec<u8>,
    pub building: BuildingSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeEntitySeed {
    pub entity_index: usize,
    pub activation: SavePostLoadEntityActivationCandidate,
    pub chunk_len: usize,
    pub chunk_sha256: String,
    pub chunk_bytes: Vec<u8>,
    pub body_len: usize,
    pub body_sha256: String,
    pub body_bytes: Vec<u8>,
}

impl SavePostLoadWorldObservation {
    pub fn runtime_seed_plan(&self) -> SavePostLoadRuntimeSeedPlan {
        let contract = self.projection_contract();
        let activation = activation_surface_from_contract(self, &contract);
        let world_seed = SavePostLoadRuntimeWorldSeed {
            save_version: self.save_version,
            content_header: self.content_header.clone(),
            patches: self.patches.clone(),
            world: self.map.world.clone(),
        };
        let entity_remap_seeds = self
            .entity_remap_entries
            .iter()
            .enumerate()
            .map(runtime_entity_remap_seed)
            .collect();
        let team_plan_seeds = self
            .team_plan_groups
            .iter()
            .enumerate()
            .flat_map(|(group_index, group)| {
                group
                    .plans
                    .iter()
                    .enumerate()
                    .map(move |(plan_index, plan)| SavePostLoadRuntimeTeamPlanSeed {
                        group_index,
                        plan_index,
                        team_id: group.team_id,
                        plan: plan.clone(),
                    })
            })
            .collect();
        let marker_seeds = self
            .markers
            .iter()
            .enumerate()
            .map(|(marker_index, entry)| SavePostLoadRuntimeMarkerSeed {
                marker_index,
                id: entry.id,
                kind_name: entry.marker.kind_name(),
                class_tag: entry.marker.class_tag().map(str::to_string),
                tile_coords: entry.marker.tile_coords(),
                marker: entry.marker.clone(),
            })
            .collect();
        let static_fog_seed = runtime_static_fog_seed(&self.custom_chunks);
        let custom_chunk_seeds = self
            .custom_chunks
            .iter()
            .enumerate()
            .map(runtime_custom_chunk_seed)
            .collect();
        let building_seeds = self
            .map
            .world
            .building_centers
            .iter()
            .enumerate()
            .map(|(center_index, center)| SavePostLoadRuntimeBuildingSeed {
                activation: building_activation_candidate(&self.map.world, center_index, center),
                chunk_len: center.chunk_len,
                chunk_sha256: center.chunk_sha256.clone(),
                chunk_bytes: center.chunk_bytes.clone(),
                building: center.building.clone(),
            })
            .collect();

        let mut loadable_entity_seeds = Vec::new();
        let mut skipped_entity_seeds = Vec::new();
        for (entity_index, chunk) in self.world_entity_chunks.iter().enumerate() {
            let seed = runtime_entity_seed(entity_index, chunk);
            if chunk.would_post_load_skip() {
                skipped_entity_seeds.push(seed);
            } else {
                loadable_entity_seeds.push(seed);
            }
        }

        SavePostLoadRuntimeSeedPlan {
            contract,
            activation,
            world_seed,
            entity_remap_seeds,
            team_plan_seeds,
            marker_seeds,
            static_fog_seed,
            custom_chunk_seeds,
            building_seeds,
            loadable_entity_seeds,
            skipped_entity_seeds,
        }
    }
}

fn runtime_entity_remap_seed(
    (remap_index, entry): (usize, &SaveEntityRemapEntry),
) -> SavePostLoadRuntimeEntityRemapSeed {
    SavePostLoadRuntimeEntityRemapSeed {
        remap_index,
        custom_id: entry.custom_id,
        name: entry.name.clone(),
    }
}

fn runtime_static_fog_seed(
    custom_chunks: &[CustomChunkEntry],
) -> Option<SavePostLoadRuntimeStaticFogSeed> {
    let mut static_fog_chunks = custom_chunks
        .iter()
        .filter(|chunk| chunk.name == "static-fog-data");
    let chunk = static_fog_chunks.next()?;
    if static_fog_chunks.next().is_some() {
        return None;
    }

    runtime_static_fog_seed_from_chunk(chunk)
}

fn runtime_static_fog_seed_from_chunk(
    chunk: &CustomChunkEntry,
) -> Option<SavePostLoadRuntimeStaticFogSeed> {
    let fog = chunk.static_fog()?;
    Some(SavePostLoadRuntimeStaticFogSeed {
        source_chunk_name: chunk.name.clone(),
        source_chunk_sha256: chunk.chunk_sha256.clone(),
        width: fog.width,
        height: fog.height,
        teams: fog
            .teams
            .iter()
            .enumerate()
            .map(runtime_static_fog_team_seed)
            .collect(),
    })
}

fn runtime_static_fog_team_seed(
    (team_index, team): (usize, &StaticFogTeam),
) -> SavePostLoadRuntimeStaticFogTeamSeed {
    SavePostLoadRuntimeStaticFogTeamSeed {
        team_index,
        team_id: team.team_id,
        run_count: team.run_count,
        discovered_count: team.discovered_count(),
        discovered_indices: team.discovered_indices(),
        discovered: team.discovered.clone(),
    }
}

fn runtime_custom_chunk_seed(
    (chunk_index, chunk): (usize, &CustomChunkEntry),
) -> SavePostLoadRuntimeCustomChunkSeed {
    SavePostLoadRuntimeCustomChunkSeed {
        chunk_index,
        name: chunk.name.clone(),
        chunk_len: chunk.chunk_len,
        chunk_sha256: chunk.chunk_sha256.clone(),
        chunk_bytes: chunk.chunk_bytes.clone(),
        parsed: chunk.parsed.clone(),
    }
}

fn runtime_entity_seed(
    entity_index: usize,
    chunk: &SaveEntityChunkObservation,
) -> SavePostLoadRuntimeEntitySeed {
    SavePostLoadRuntimeEntitySeed {
        entity_index,
        activation: entity_activation_candidate(chunk),
        chunk_len: chunk.chunk_len,
        chunk_sha256: chunk.chunk_sha256.clone(),
        chunk_bytes: chunk.chunk_bytes.clone(),
        body_len: chunk.body_len,
        body_sha256: chunk.body_sha256.clone(),
        body_bytes: chunk.body_bytes.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, CustomChunkEntry, MarkerEntry,
        ParsedBuildingTail, PointMarkerModel, SaveEntityChunkObservation, SaveEntityClassKind,
        SaveEntityClassSummary, SaveEntityPostLoadClassSummary, SaveEntityPostLoadKind,
        SaveEntityPostLoadSummary, SaveEntityRemapSummary, SaveMapRegionObservation,
        SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam, TeamPlan, TeamPlanGroup,
        TileModel, TypeIoValue,
    };

    #[test]
    fn runtime_seed_plan_carries_deterministic_runtime_inputs() {
        let observation = test_observation();
        let plan = observation.runtime_seed_plan();

        assert_eq!(plan.contract, observation.projection_contract());
        assert_eq!(plan.activation, observation.activation_surface());
        assert!(!plan.can_seed_runtime_apply());
        assert_eq!(plan.seed_step_count(), 14);

        assert_eq!(plan.world_seed.save_version, 11);
        assert_eq!(plan.world_seed.tile_count(), 4);
        assert_eq!(plan.world_seed.building_center_count(), 1);
        assert_eq!(plan.entity_remap_seeds.len(), 2);
        assert_eq!(
            plan.entity_remap_seeds[0],
            SavePostLoadRuntimeEntityRemapSeed {
                remap_index: 0,
                custom_id: 255,
                name: "flare".to_string(),
            }
        );
        assert_eq!(
            plan.team_plan_seeds
                .iter()
                .map(|seed| (
                    seed.group_index,
                    seed.plan_index,
                    seed.team_id,
                    seed.plan.block_id
                ))
                .collect::<Vec<_>>(),
            vec![(0, 0, 1, 0x0101), (1, 0, 2, 0x0102)]
        );
        assert_eq!(
            plan.marker_seeds
                .iter()
                .map(|seed| (
                    seed.marker_index,
                    seed.id,
                    seed.kind_name,
                    seed.class_tag.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, 11, "Point", Some("Minimap".to_string())),
                (1, 12, "Point", Some("Objective".to_string())),
            ]
        );
        assert_eq!(
            plan.static_fog_seed,
            Some(SavePostLoadRuntimeStaticFogSeed {
                source_chunk_name: "static-fog-data".to_string(),
                source_chunk_sha256: "fog".to_string(),
                width: 2,
                height: 2,
                teams: vec![
                    SavePostLoadRuntimeStaticFogTeamSeed {
                        team_index: 0,
                        team_id: 1,
                        run_count: 1,
                        discovered_count: 3,
                        discovered_indices: vec![0, 2, 3],
                        discovered: vec![true, false, true, true],
                    },
                    SavePostLoadRuntimeStaticFogTeamSeed {
                        team_index: 1,
                        team_id: 2,
                        run_count: 1,
                        discovered_count: 2,
                        discovered_indices: vec![1, 3],
                        discovered: vec![false, true, false, true],
                    },
                ],
            })
        );
        assert_eq!(plan.custom_chunk_seeds.len(), 2);
        assert_eq!(plan.custom_chunk_seeds[1].name, "mystery".to_string());
        assert_eq!(
            plan.building_seeds[0].activation,
            SavePostLoadBuildingActivationCandidate {
                center_index: 0,
                tile_index: 0,
                x: 0,
                y: 0,
                block_id: 0x0153,
                revision: 0,
                tail_kind: "core",
                center_reference_valid: true,
            }
        );
        assert_eq!(
            plan.loadable_entity_seeds
                .iter()
                .map(|seed| (
                    seed.entity_index,
                    seed.activation.entity_id,
                    seed.chunk_sha256.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, 42, "chunk-remap".to_string()),
                (2, 44, "chunk-builtin".to_string()),
            ]
        );
        assert_eq!(
            plan.skipped_entity_seeds[0],
            SavePostLoadRuntimeEntitySeed {
                entity_index: 1,
                activation: SavePostLoadEntityActivationCandidate {
                    entity_id: 43,
                    source_class_id: 254,
                    effective_class_id: None,
                    source_name: "mod-unit".to_string(),
                    effective_name: None,
                    chunk_len: 3,
                    body_len: 2,
                },
                chunk_len: 3,
                chunk_sha256: "chunk-skip".to_string(),
                chunk_bytes: vec![6, 7, 8],
                body_len: 2,
                body_sha256: "entity-skip".to_string(),
                body_bytes: vec![7, 8],
            }
        );
    }

    #[test]
    fn runtime_seed_plan_is_seedable_when_activation_surface_is_clean() {
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

        let plan = observation.runtime_seed_plan();

        assert!(plan.contract.can_project_world_shell());
        assert!(plan.can_seed_runtime_apply());
        assert!(plan.activation.can_seed_runtime_apply());
        assert!(plan.skipped_entity_seeds.is_empty());
        assert_eq!(
            plan.loadable_entity_seeds
                .iter()
                .map(|seed| seed.activation.entity_id)
                .collect::<Vec<_>>(),
            vec![42, 43, 44]
        );
    }

    #[test]
    fn runtime_seed_plan_blocks_duplicate_static_fog_data_chunks() {
        let mut observation = test_observation();
        observation.custom_chunks.push(CustomChunkEntry {
            name: "static-fog-data".to_string(),
            chunk_len: 1,
            chunk_bytes: vec![10],
            chunk_sha256: "fog-duplicate".to_string(),
            parsed: ParsedCustomChunk::Unknown,
        });

        let plan = observation.runtime_seed_plan();
        let script = observation.runtime_apply_script();

        assert!(plan.static_fog_seed.is_none());
        assert_eq!(plan.seed_step_count(), 14);
        assert_eq!(plan.custom_chunk_seeds.len(), 3);
        assert_eq!(
            plan.custom_chunk_seeds
                .iter()
                .filter(|seed| seed.name == "static-fog-data")
                .count(),
            2
        );
        assert_eq!(script.total_step_count(), 14);
        assert_eq!(script.total_step_count(), plan.seed_step_count());
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
