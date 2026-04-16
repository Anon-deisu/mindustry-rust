use crate::{
    BuildingCenter, ParsedBuildingTail, SaveEntityChunkObservation, SavePostLoadWorldContract,
    SavePostLoadWorldObservation, WorldModel,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadEntityActivationCandidate {
    pub entity_id: i32,
    pub source_class_id: u8,
    pub effective_class_id: Option<u8>,
    pub source_name: String,
    pub effective_name: Option<String>,
    pub chunk_len: usize,
    pub body_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadBuildingActivationCandidate {
    pub center_index: usize,
    pub tile_index: usize,
    pub x: usize,
    pub y: usize,
    pub block_id: u16,
    pub revision: u8,
    pub tail_kind: &'static str,
    pub center_reference_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadActivationSurface {
    pub world_shell_ready: bool,
    pub entity_ids_unique: bool,
    pub duplicate_entity_ids: Vec<i32>,
    pub duplicate_custom_ids: Vec<u16>,
    pub duplicate_names: Vec<String>,
    pub unresolved_effective_names: Vec<String>,
    pub building_candidates: Vec<SavePostLoadBuildingActivationCandidate>,
    pub loadable_entities: Vec<SavePostLoadEntityActivationCandidate>,
    pub skipped_entities: Vec<SavePostLoadEntityActivationCandidate>,
}

impl SavePostLoadActivationSurface {
    pub fn valid_building_reference_count(&self) -> usize {
        self.building_candidates
            .iter()
            .filter(|candidate| candidate.center_reference_valid)
            .count()
    }

    pub fn can_seed_runtime_apply(&self) -> bool {
        self.world_shell_ready
            && self.entity_ids_unique
            && self.duplicate_custom_ids.is_empty()
            && self.duplicate_names.is_empty()
            && self.unresolved_effective_names.is_empty()
            && self.skipped_entities.is_empty()
            && self.valid_building_reference_count() == self.building_candidates.len()
    }
}

impl SavePostLoadWorldObservation {
    pub fn activation_surface(&self) -> SavePostLoadActivationSurface {
        let contract = self.projection_contract();
        activation_surface_from_contract(self, &contract)
    }
}

pub(crate) fn activation_surface_from_contract(
    observation: &SavePostLoadWorldObservation,
    contract: &SavePostLoadWorldContract,
) -> SavePostLoadActivationSurface {
    let world = &observation.map.world;
    let building_candidates = world
        .building_centers
        .iter()
        .enumerate()
        .map(|(center_index, center)| building_activation_candidate(world, center_index, center))
        .collect();

    let mut loadable_entities = Vec::new();
    let mut skipped_entities = Vec::new();
    for chunk in &observation.world_entity_chunks {
        let candidate = entity_activation_candidate(chunk);
        if chunk.would_post_load_skip() {
            skipped_entities.push(candidate);
        } else {
            loadable_entities.push(candidate);
        }
    }

    SavePostLoadActivationSurface {
        world_shell_ready: contract.can_project_world_shell(),
        entity_ids_unique: observation.entity_summary.duplicate_entity_ids.is_empty(),
        duplicate_entity_ids: observation.entity_summary.duplicate_entity_ids.clone(),
        duplicate_custom_ids: observation
            .entity_remap_summary
            .duplicate_custom_ids
            .clone(),
        duplicate_names: observation.entity_remap_summary.duplicate_names.clone(),
        unresolved_effective_names: observation
            .entity_remap_summary
            .unresolved_effective_names
            .clone(),
        building_candidates,
        loadable_entities,
        skipped_entities,
    }
}

pub(crate) fn building_activation_candidate(
    world: &WorldModel,
    center_index: usize,
    center: &BuildingCenter,
) -> SavePostLoadBuildingActivationCandidate {
    let center_reference_valid = world
        .tiles
        .get(center.tile_index)
        .map(|tile| {
            tile.tile_index == center.tile_index
                && tile.x == center.x
                && tile.y == center.y
                && tile.block_id == center.block_id
                && tile.building_center_index == Some(center_index)
        })
        .unwrap_or(false);

    SavePostLoadBuildingActivationCandidate {
        center_index,
        tile_index: center.tile_index,
        x: center.x,
        y: center.y,
        block_id: center.block_id,
        revision: center.building.revision,
        tail_kind: building_tail_kind(&center.building.parsed_tail),
        center_reference_valid,
    }
}

pub(crate) fn entity_activation_candidate(
    chunk: &SaveEntityChunkObservation,
) -> SavePostLoadEntityActivationCandidate {
    SavePostLoadEntityActivationCandidate {
        entity_id: chunk.entity_id,
        source_class_id: chunk.class_id,
        effective_class_id: chunk.post_load_class_id(),
        source_name: chunk.resolved_name().into_owned(),
        effective_name: chunk
            .post_load_resolved_name()
            .map(|name| name.into_owned()),
        chunk_len: chunk.chunk_len,
        body_len: chunk.body_len,
    }
}

pub(crate) fn building_tail_kind(parsed_tail: &ParsedBuildingTail) -> &'static str {
    match parsed_tail {
        ParsedBuildingTail::Empty => "empty",
        ParsedBuildingTail::Conveyor(_) => "conveyor",
        ParsedBuildingTail::StackConveyor(_) => "stackConveyor",
        ParsedBuildingTail::Core(_) => "core",
        ParsedBuildingTail::UnitFactory(_) => "unitFactory",
        ParsedBuildingTail::Reconstructor(_) => "reconstructor",
        ParsedBuildingTail::NullableItemRef(_) => "nullableItemRef",
        ParsedBuildingTail::ItemBridge(_) => "itemBridge",
        ParsedBuildingTail::BufferedItemBridge(_) => "bufferedItemBridge",
        ParsedBuildingTail::MassDriver(_) => "massDriver",
        ParsedBuildingTail::Junction(_) => "junction",
        ParsedBuildingTail::SorterLegacy(_) => "sorterLegacy",
        ParsedBuildingTail::Turret(_) => "turret",
        ParsedBuildingTail::ItemTurret(_) => "itemTurret",
        ParsedBuildingTail::ContinuousTurret(_) => "continuousTurret",
        ParsedBuildingTail::BuildTurret(_) => "buildTurret",
        ParsedBuildingTail::PayloadLoader(_) => "payloadLoader",
        ParsedBuildingTail::PayloadSource(_) => "payloadSource",
        ParsedBuildingTail::PayloadRouter(_) => "payloadRouter",
        ParsedBuildingTail::PayloadMassDriver(_) => "payloadMassDriver",
        ParsedBuildingTail::BlockProducer(_) => "blockProducer",
        ParsedBuildingTail::Constructor(_) => "constructor",
        ParsedBuildingTail::UnitAssembler(_) => "unitAssembler",
        ParsedBuildingTail::OneF32(_) => "oneF32",
        ParsedBuildingTail::OneI8(_) => "oneI8",
        ParsedBuildingTail::OneI32(_) => "oneI32",
        ParsedBuildingTail::OneBool(_) => "oneBool",
        ParsedBuildingTail::OneF32Bool(_) => "oneF32Bool",
        ParsedBuildingTail::TwoF32(_) => "twoF32",
        ParsedBuildingTail::TwoF32I32(_) => "twoF32I32",
        ParsedBuildingTail::ThreeF32(_) => "threeF32",
        ParsedBuildingTail::FiveF32(_) => "fiveF32",
        ParsedBuildingTail::LandingPad(_) => "landingPad",
        ParsedBuildingTail::Message(_) => "message",
        ParsedBuildingTail::DuctUnloader(_) => "ductUnloader",
        ParsedBuildingTail::Memory(_) => "memory",
        ParsedBuildingTail::Canvas(_) => "canvas",
        ParsedBuildingTail::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, CustomChunkEntry, MarkerEntry,
        MarkerModel, ParsedCustomChunk, PointMarkerModel, SaveEntityChunkObservation,
        SaveEntityClassKind, SaveEntityClassSummary, SaveEntityPostLoadClassSummary,
        SaveEntityPostLoadKind, SaveEntityPostLoadSummary, SaveEntityRemapSummary,
        SaveMapRegionObservation, SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam,
        TeamPlan, TeamPlanGroup, TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn activation_surface_partitions_loadable_and_skipped_entities() {
        let observation = test_observation();
        let surface = observation.activation_surface();

        assert!(surface.world_shell_ready);
        assert!(surface.entity_ids_unique);
        assert_eq!(surface.duplicate_entity_ids, Vec::<i32>::new());
        assert_eq!(
            surface.unresolved_effective_names,
            vec!["mod-unit".to_string()]
        );
        assert_eq!(surface.valid_building_reference_count(), 1);
        assert!(!surface.can_seed_runtime_apply());

        assert_eq!(surface.loadable_entities.len(), 2);
        assert_eq!(surface.skipped_entities.len(), 1);
        assert_eq!(
            surface.loadable_entities[0],
            SavePostLoadEntityActivationCandidate {
                entity_id: 42,
                source_class_id: 255,
                effective_class_id: Some(3),
                source_name: "flare".to_string(),
                effective_name: Some("flare".to_string()),
                chunk_len: 3,
                body_len: 2,
            }
        );
        assert_eq!(
            surface.loadable_entities[1],
            SavePostLoadEntityActivationCandidate {
                entity_id: 44,
                source_class_id: 4,
                effective_class_id: Some(4),
                source_name: "mace".to_string(),
                effective_name: Some("mace".to_string()),
                chunk_len: 3,
                body_len: 2,
            }
        );
        assert_eq!(
            surface.skipped_entities[0],
            SavePostLoadEntityActivationCandidate {
                entity_id: 43,
                source_class_id: 254,
                effective_class_id: None,
                source_name: "mod-unit".to_string(),
                effective_name: None,
                chunk_len: 3,
                body_len: 2,
            }
        );
        assert_eq!(
            surface.building_candidates[0],
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
    }

    #[test]
    fn activation_candidate_extractors_copy_building_and_entity_fields_stably() {
        let observation = test_observation();
        let world = &observation.map.world;
        let center = &world.building_centers[0];
        let entity = &observation.world_entity_chunks[0];

        assert_eq!(
            building_activation_candidate(world, 0, center),
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
            entity_activation_candidate(entity),
            SavePostLoadEntityActivationCandidate {
                entity_id: 42,
                source_class_id: 255,
                effective_class_id: Some(3),
                source_name: "flare".to_string(),
                effective_name: Some("flare".to_string()),
                chunk_len: 3,
                body_len: 2,
            }
        );
    }

    #[test]
    fn activation_surface_reports_duplicate_entity_ids_and_invalid_building_reference() {
        let mut observation = test_observation();
        observation.world_entity_chunks[1].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.map.world.tiles[0].building_center_index = None;

        let surface = observation.activation_surface();

        assert!(!surface.entity_ids_unique);
        assert_eq!(surface.duplicate_entity_ids, vec![42]);
        assert_eq!(surface.valid_building_reference_count(), 0);
        assert!(!surface.building_candidates[0].center_reference_valid);
        assert!(!surface.can_seed_runtime_apply());
    }

    #[test]
    fn activation_surface_rejects_duplicate_entity_remap_keys() {
        let mut custom_id_observation = test_observation();
        custom_id_observation.entity_remap_summary.duplicate_custom_ids = vec![99];

        let custom_id_surface = custom_id_observation.activation_surface();
        assert_eq!(custom_id_surface.duplicate_custom_ids, vec![99]);
        assert!(custom_id_surface.duplicate_names.is_empty());
        assert!(!custom_id_surface.can_seed_runtime_apply());

        let mut name_observation = test_observation();
        name_observation.entity_remap_summary.duplicate_names = vec![
            "mod-duplicate".to_string(),
        ];

        let name_surface = name_observation.activation_surface();
        assert!(name_surface.duplicate_custom_ids.is_empty());
        assert_eq!(
            name_surface.duplicate_names,
            vec!["mod-duplicate".to_string()]
        );
        assert!(!name_surface.can_seed_runtime_apply());
    }

    #[test]
    fn activation_surface_rejects_unresolved_effective_names() {
        let mut observation = test_observation();
        observation.entity_remap_summary.unresolved_effective_names = vec!["mod-unit".to_string()];
        observation.world_entity_chunks[1].class_id = 4;
        observation.world_entity_chunks[1].custom_name = None;

        let surface = observation.activation_surface();

        assert_eq!(
            surface.unresolved_effective_names,
            vec!["mod-unit".to_string()]
        );
        assert!(surface.skipped_entities.is_empty());
        assert!(!surface.can_seed_runtime_apply());
    }

    #[test]
    fn activation_surface_rejects_world_shell_contract_failures() {
        let mut observation = test_observation();
        if let MarkerModel::Point(marker) = &mut observation.markers[0].marker {
            marker.x_bits = 99.0f32.to_bits();
            marker.y_bits = 99.0f32.to_bits();
        }
        observation.entity_remap_summary.unresolved_effective_names.clear();
        observation
            .world_entity_chunks
            .retain(|chunk| !chunk.would_post_load_skip());

        let surface = observation.activation_surface();

        assert!(!surface.world_shell_ready);
        assert!(surface.unresolved_effective_names.is_empty());
        assert!(surface.skipped_entities.is_empty());
        assert!(!surface.can_seed_runtime_apply());
    }

    fn test_observation() -> SavePostLoadWorldObservation {
        SavePostLoadWorldObservation {
            save_version: 11,
            content_header: Vec::new(),
            patches: Vec::new(),
            map: SaveMapRegionObservation {
                floor_runs: 1,
                floor_region_bytes: vec![1],
                block_runs: 1,
                block_region_bytes: vec![2],
                world: test_world(),
            },
            entity_remap_entries: Vec::new(),
            entity_remap_bytes: Vec::new(),
            entity_remap_summary: SaveEntityRemapSummary {
                remap_count: 1,
                unique_custom_ids: 1,
                duplicate_custom_ids: Vec::new(),
                unique_names: 1,
                duplicate_names: Vec::new(),
                effective_custom_ids: 1,
                resolved_builtin_custom_ids: vec![255],
                unresolved_effective_names: vec!["mod-unit".to_string()],
            },
            team_plan_groups: vec![TeamPlanGroup {
                team_id: 1,
                plan_count: 1,
                plans: vec![TeamPlan {
                    x: 1,
                    y: 1,
                    rotation: 0,
                    block_id: 0x0101,
                    config: TypeIoValue::Null,
                    config_bytes: Vec::new(),
                    config_sha256: "plan".to_string(),
                }],
            }],
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
            markers: vec![MarkerEntry {
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
            }],
            marker_region_bytes: b"{markers}".to_vec(),
            custom_chunks: vec![CustomChunkEntry {
                name: "static-fog-data".to_string(),
                chunk_len: 1,
                chunk_bytes: vec![7],
                chunk_sha256: "fog".to_string(),
                parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                    used_teams: 1,
                    width: 2,
                    height: 2,
                    teams: vec![StaticFogTeam {
                        team_id: 1,
                        run_count: 1,
                        rle_bytes: vec![8],
                        discovered: vec![true, false, true, true],
                    }],
                }),
            }],
            custom_region_bytes: vec![9],
            entity_summary: SaveEntityPostLoadSummary {
                total_entities: 3,
                unique_entity_ids: 3,
                duplicate_entity_ids: Vec::new(),
                builtin_entities: 1,
                custom_entities: 2,
                unknown_entities: 0,
                class_summaries: vec![
                    SaveEntityClassSummary {
                        class_id: 4,
                        kind: SaveEntityClassKind::Builtin,
                        resolved_name: "mace".to_string(),
                        count: 1,
                    },
                    SaveEntityClassSummary {
                        class_id: 254,
                        kind: SaveEntityClassKind::Custom,
                        resolved_name: "mod-unit".to_string(),
                        count: 1,
                    },
                    SaveEntityClassSummary {
                        class_id: 255,
                        kind: SaveEntityClassKind::Custom,
                        resolved_name: "flare".to_string(),
                        count: 1,
                    },
                ],
                loadable_entities: 2,
                skipped_entities: 1,
                post_load_class_summaries: vec![
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
                    SaveEntityPostLoadClassSummary {
                        source_class_ids: vec![254],
                        effective_class_id: None,
                        kind: SaveEntityPostLoadKind::UnresolvedCustom,
                        resolved_name: "unresolved:mod-unit".to_string(),
                        count: 1,
                    },
                ],
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
                chunk_len: 0,
                chunk_bytes: Vec::new(),
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
            team_count: 1,
            total_plans: 1,
            team_ids: vec![1],
            team_plan_counts: vec![1],
        }
    }

    #[test]
    fn building_tail_kind_maps_empty_and_unknown_stably() {
        assert_eq!(building_tail_kind(&ParsedBuildingTail::Empty), "empty");
        assert_eq!(building_tail_kind(&ParsedBuildingTail::Unknown), "unknown");
    }
}
