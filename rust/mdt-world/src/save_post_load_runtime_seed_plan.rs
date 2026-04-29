use crate::save_post_load_activation::{
    activation_surface_from_contract, building_activation_candidate, entity_activation_candidate,
};
use crate::{
    bool_word_label, decode_save_content_patches_utf8,
    save_post_load_runtime_source_region::find_source_region, BuildingSnapshot, ContentHeaderEntry,
    CustomChunkEntry, MarkerModel, ParsedCustomChunk,
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

    pub fn summary_label(&self) -> String {
        format!(
            "seed={} steps={} regions={}",
            bool_word_label(self.can_seed_runtime_apply()),
            self.seed_step_count(),
            self.source_regions().len(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "seed={} steps={} regions=[{}]",
            bool_word_label(self.can_seed_runtime_apply()),
            self.seed_step_count(),
            self.source_regions()
                .iter()
                .map(SavePostLoadRuntimeSeedRegion::summary_label)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    pub fn source_region(&self, source_region_name: &str) -> Option<SavePostLoadRuntimeSeedRegion> {
        find_source_region(self.source_regions(), source_region_name, |region| {
            region.source_region_name
        })
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeSeedRegion> {
        let mut source_regions = Vec::new();

        let map = SavePostLoadRuntimeSeedRegion {
            source_region_name: "map",
            world_seed: Some(self.world_seed.clone()),
            entity_remap_seeds: Vec::new(),
            team_plan_seeds: Vec::new(),
            marker_seeds: Vec::new(),
            static_fog_seed: None,
            custom_chunk_seeds: Vec::new(),
            building_seeds: self.building_seeds.clone(),
            loadable_entity_seeds: Vec::new(),
            skipped_entity_seeds: Vec::new(),
        };
        if map.seed_step_count() > 0 {
            source_regions.push(map);
        }

        let entities = SavePostLoadRuntimeSeedRegion {
            source_region_name: "entities",
            world_seed: None,
            entity_remap_seeds: self.entity_remap_seeds.clone(),
            team_plan_seeds: self.team_plan_seeds.clone(),
            marker_seeds: Vec::new(),
            static_fog_seed: None,
            custom_chunk_seeds: Vec::new(),
            building_seeds: Vec::new(),
            loadable_entity_seeds: self.loadable_entity_seeds.clone(),
            skipped_entity_seeds: self.skipped_entity_seeds.clone(),
        };
        if entities.seed_step_count() > 0 {
            source_regions.push(entities);
        }

        let markers = SavePostLoadRuntimeSeedRegion {
            source_region_name: "markers",
            world_seed: None,
            entity_remap_seeds: Vec::new(),
            team_plan_seeds: Vec::new(),
            marker_seeds: self.marker_seeds.clone(),
            static_fog_seed: None,
            custom_chunk_seeds: Vec::new(),
            building_seeds: Vec::new(),
            loadable_entity_seeds: Vec::new(),
            skipped_entity_seeds: Vec::new(),
        };
        if markers.seed_step_count() > 0 {
            source_regions.push(markers);
        }

        let custom = SavePostLoadRuntimeSeedRegion {
            source_region_name: "custom",
            world_seed: None,
            entity_remap_seeds: Vec::new(),
            team_plan_seeds: Vec::new(),
            marker_seeds: Vec::new(),
            static_fog_seed: self.static_fog_seed.clone(),
            custom_chunk_seeds: self.custom_chunk_seeds.clone(),
            building_seeds: Vec::new(),
            loadable_entity_seeds: Vec::new(),
            skipped_entity_seeds: Vec::new(),
        };
        if custom.seed_step_count() > 0 {
            source_regions.push(custom);
        }

        source_regions
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
    pub fn patch_texts(&self) -> Result<Vec<String>, String> {
        decode_save_content_patches_utf8(&self.patches)
    }

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
pub struct SavePostLoadRuntimeSeedRegion {
    pub source_region_name: &'static str,
    pub world_seed: Option<SavePostLoadRuntimeWorldSeed>,
    pub entity_remap_seeds: Vec<SavePostLoadRuntimeEntityRemapSeed>,
    pub team_plan_seeds: Vec<SavePostLoadRuntimeTeamPlanSeed>,
    pub marker_seeds: Vec<SavePostLoadRuntimeMarkerSeed>,
    pub static_fog_seed: Option<SavePostLoadRuntimeStaticFogSeed>,
    pub custom_chunk_seeds: Vec<SavePostLoadRuntimeCustomChunkSeed>,
    pub building_seeds: Vec<SavePostLoadRuntimeBuildingSeed>,
    pub loadable_entity_seeds: Vec<SavePostLoadRuntimeEntitySeed>,
    pub skipped_entity_seeds: Vec<SavePostLoadRuntimeEntitySeed>,
}

impl SavePostLoadRuntimeSeedRegion {
    pub fn seed_step_count(&self) -> usize {
        usize::from(self.world_seed.is_some())
            + self.entity_remap_seeds.len()
            + self.team_plan_seeds.len()
            + self.marker_seeds.len()
            + usize::from(self.static_fog_seed.is_some())
            + self.custom_chunk_seeds.len()
            + self.building_seeds.len()
            + self.loadable_entity_seeds.len()
            + self.skipped_entity_seeds.len()
    }

    pub fn summary_label(&self) -> String {
        format!(
            "region={} world={} remaps={} plans={} markers={} fog={} chunks={} buildings={} loadable={} skipped={} total={}",
            self.source_region_name,
            usize::from(self.world_seed.is_some()),
            self.entity_remap_seeds.len(),
            self.team_plan_seeds.len(),
            self.marker_seeds.len(),
            usize::from(self.static_fog_seed.is_some()),
            self.custom_chunk_seeds.len(),
            self.building_seeds.len(),
            self.loadable_entity_seeds.len(),
            self.skipped_entity_seeds.len(),
            self.seed_step_count(),
        )
    }

    pub fn detail_label(&self) -> String {
        self.summary_label()
    }
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
        let static_fog_chunk_count = self
            .custom_chunks
            .iter()
            .filter(|chunk| chunk.name == "static-fog-data")
            .count();
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
            // Duplicate static-fog chunks are intentionally dropped from the custom-chunk
            // seed list so the singleton accessor and runtime seeds stay aligned.
            .filter(|(_, chunk)| static_fog_chunk_count <= 1 || chunk.name != "static-fog-data")
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
pub(crate) mod save_post_load_runtime_test_support {
    pub(crate) use crate::save_post_load_runtime_execution::test_support::{
        make_runtime_plan_observation_seedable as make_observation_seedable,
        runtime_plan_seedable_test_observation as seedable_test_observation,
        runtime_plan_test_observation as test_observation,
    };
}

#[cfg(test)]
mod tests {
    use super::save_post_load_runtime_test_support::{make_observation_seedable, test_observation};
    use super::*;
    use crate::{
        CustomChunkEntry, ParsedCustomChunk, SavePostLoadRuntimeApplyScript,
        SavePostLoadWorldObservation,
    };

    #[derive(Clone, Copy)]
    struct RegionCountExpectation {
        source_region_name: &'static str,
        world: usize,
        remaps: usize,
        plans: usize,
        markers: usize,
        fog: usize,
        chunks: usize,
        buildings: usize,
        loadable: usize,
        skipped: usize,
    }

    impl RegionCountExpectation {
        fn total(self) -> usize {
            self.world
                + self.remaps
                + self.plans
                + self.markers
                + self.fog
                + self.chunks
                + self.buildings
                + self.loadable
                + self.skipped
        }

        fn summary_label(self) -> String {
            format!(
                "region={} world={} remaps={} plans={} markers={} fog={} chunks={} buildings={} loadable={} skipped={} total={}",
                self.source_region_name,
                self.world,
                self.remaps,
                self.plans,
                self.markers,
                self.fog,
                self.chunks,
                self.buildings,
                self.loadable,
                self.skipped,
                self.total(),
            )
        }
    }

    fn assert_plan_summary(
        plan: &SavePostLoadRuntimeSeedPlan,
        expected_seedable: bool,
        expected_steps: usize,
        expected_regions: usize,
    ) {
        assert_eq!(plan.seed_step_count(), expected_steps);
        assert_eq!(
            plan.summary_label(),
            format!(
                "seed={} steps={} regions={}",
                bool_word_label(expected_seedable),
                expected_steps,
                expected_regions,
            )
        );
        assert_eq!(plan.source_regions().len(), expected_regions);
    }

    fn assert_source_region_counts(
        region: &SavePostLoadRuntimeSeedRegion,
        expected: RegionCountExpectation,
    ) {
        assert_eq!(region.source_region_name, expected.source_region_name);
        assert_eq!(region.seed_step_count(), expected.total());
        assert_eq!(region.summary_label(), expected.summary_label());
        assert_eq!(region.detail_label(), expected.summary_label());
    }

    fn expected_plan_detail_label(
        expected_seedable: bool,
        expected_steps: usize,
        expected_regions: &[RegionCountExpectation],
    ) -> String {
        format!(
            "seed={} steps={} regions=[{}]",
            bool_word_label(expected_seedable),
            expected_steps,
            expected_regions
                .iter()
                .map(|expected| expected.summary_label())
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    fn assert_static_fog_seed_blocked(
        plan: &SavePostLoadRuntimeSeedPlan,
        script: &SavePostLoadRuntimeApplyScript,
        expected_chunk_name: &str,
    ) {
        let custom_region = plan
            .source_region("custom")
            .expect("custom region should stay present when static fog seeding is blocked");

        assert_plan_summary(plan, false, 12, 4);
        assert!(plan.static_fog_seed.is_none());
        assert_eq!(plan.custom_chunk_seeds.len(), 1);
        assert_eq!(plan.custom_chunk_seeds[0].name, expected_chunk_name);
        assert_source_region_counts(
            &custom_region,
            RegionCountExpectation {
                source_region_name: "custom",
                world: 0,
                remaps: 0,
                plans: 0,
                markers: 0,
                fog: 0,
                chunks: 1,
                buildings: 0,
                loadable: 0,
                skipped: 0,
            },
        );
        assert_eq!(script.total_step_count(), 12);
        assert_eq!(script.total_step_count(), plan.seed_step_count());
    }

    #[test]
    fn runtime_seed_plan_keeps_only_nonempty_map_region_and_rejects_missing_source_region() {
        let mut observation = test_observation();
        observation.entity_remap_entries.clear();
        observation.team_plan_groups.clear();
        observation.markers.clear();
        observation.custom_chunks.clear();
        observation.world_entity_chunks.clear();
        observation.map.world.building_centers.clear();

        let plan = observation.runtime_seed_plan();
        let regions = plan.source_regions();
        let map_region = RegionCountExpectation {
            source_region_name: "map",
            world: 1,
            remaps: 0,
            plans: 0,
            markers: 0,
            fog: 0,
            chunks: 0,
            buildings: 0,
            loadable: 0,
            skipped: 0,
        };

        assert_plan_summary(&plan, false, 1, 1);
        assert_eq!(
            plan.detail_label(),
            expected_plan_detail_label(false, 1, &[map_region])
        );
        assert_eq!(regions.len(), 1);
        assert_source_region_counts(&regions[0], map_region);
        assert_eq!(plan.source_region("map"), Some(regions[0].clone()));
        assert!(plan.source_region("entities").is_none());
        assert!(plan.source_region("markers").is_none());
        assert!(plan.source_region("custom").is_none());
    }

    #[test]
    fn runtime_seed_plan_carries_deterministic_runtime_inputs() {
        let observation = test_observation();
        let plan = observation.runtime_seed_plan();
        let source_regions = plan.source_regions();
        let entities = plan.source_region("entities").unwrap();
        let map_region = RegionCountExpectation {
            source_region_name: "map",
            world: 1,
            remaps: 0,
            plans: 0,
            markers: 0,
            fog: 0,
            chunks: 0,
            buildings: 1,
            loadable: 0,
            skipped: 0,
        };
        let entities_region = RegionCountExpectation {
            source_region_name: "entities",
            world: 0,
            remaps: 2,
            plans: 2,
            markers: 0,
            fog: 0,
            chunks: 0,
            buildings: 0,
            loadable: 2,
            skipped: 1,
        };
        let markers_region = RegionCountExpectation {
            source_region_name: "markers",
            world: 0,
            remaps: 0,
            plans: 0,
            markers: 2,
            fog: 0,
            chunks: 0,
            buildings: 0,
            loadable: 0,
            skipped: 0,
        };
        let custom_region = RegionCountExpectation {
            source_region_name: "custom",
            world: 0,
            remaps: 0,
            plans: 0,
            markers: 0,
            fog: 1,
            chunks: 2,
            buildings: 0,
            loadable: 0,
            skipped: 0,
        };

        assert_eq!(plan.contract, observation.projection_contract());
        assert_eq!(plan.activation, observation.activation_surface());
        assert!(!plan.can_seed_runtime_apply());
        assert_plan_summary(&plan, false, 14, 4);
        assert!(plan.detail_label().contains(&entities_region.summary_label()));
        assert_eq!(
            source_regions
                .iter()
                .map(|region| region.source_region_name)
                .collect::<Vec<_>>(),
            vec!["map", "entities", "markers", "custom"]
        );
        assert_source_region_counts(&source_regions[0], map_region);
        assert_source_region_counts(&source_regions[1], entities_region);
        assert_source_region_counts(&source_regions[2], markers_region);
        assert_source_region_counts(&source_regions[3], custom_region);
        assert_eq!(
            source_regions[0].world_seed.as_ref(),
            Some(&plan.world_seed)
        );
        assert_eq!(source_regions[0].building_seeds.len(), 1);
        assert_source_region_counts(&entities, entities_region);
        assert_eq!(entities.team_plan_seeds.len(), 2);

        assert_eq!(plan.world_seed.save_version, 11);
        assert_eq!(plan.world_seed.tile_count(), 4);
        assert_eq!(plan.world_seed.building_center_count(), 1);
        assert_eq!(plan.world_seed.patch_texts(), observation.patch_texts());
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
        assert_eq!(source_regions[2].marker_seeds.len(), 2);
        assert_eq!(
            source_regions[3]
                .static_fog_seed
                .as_ref()
                .expect("static fog seed should be present")
                .source_chunk_name,
            "static-fog-data"
        );
        assert_eq!(source_regions[3].custom_chunk_seeds.len(), 2);
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
        make_observation_seedable(&mut observation);

        let plan = observation.runtime_seed_plan();

        assert!(plan.contract.can_project_world_shell());
        assert!(plan.can_seed_runtime_apply());
        assert!(plan.activation.can_seed_runtime_apply());
        assert!(plan.skipped_entity_seeds.is_empty());
        assert_plan_summary(&plan, true, 14, 4);
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
        let observation = test_observation_with_duplicate_static_fog_chunk();
        let (plan, script) = runtime_seed_outputs(&observation);

        assert_static_fog_seed_blocked(&plan, &script, "mystery");
    }

    #[test]
    fn runtime_seed_plan_blocks_damaged_static_fog_data_chunk() {
        let observation = test_observation_with_damaged_static_fog_chunk();
        let (plan, script) = runtime_seed_outputs(&observation);

        assert_static_fog_seed_blocked(&plan, &script, "static-fog-data");
    }

    fn test_observation_with_duplicate_static_fog_chunk() -> SavePostLoadWorldObservation {
        let mut observation = test_observation();
        observation.custom_chunks.push(CustomChunkEntry {
            name: "static-fog-data".to_string(),
            chunk_len: 1,
            chunk_bytes: vec![10],
            chunk_sha256: "fog-duplicate".to_string(),
            parsed: ParsedCustomChunk::Unknown,
        });
        observation
    }

    fn test_observation_with_damaged_static_fog_chunk() -> SavePostLoadWorldObservation {
        let mut observation = test_observation();
        observation.custom_chunks.truncate(1);
        observation.custom_chunks[0].parsed = ParsedCustomChunk::Unknown;
        observation.custom_chunks[0].chunk_bytes = vec![10, 11, 12];
        observation.custom_chunks[0].chunk_sha256 = "fog-corrupt".to_string();
        observation
    }

    fn runtime_seed_outputs(
        observation: &SavePostLoadWorldObservation,
    ) -> (SavePostLoadRuntimeSeedPlan, SavePostLoadRuntimeApplyScript) {
        (
            observation.runtime_seed_plan(),
            observation.runtime_apply_script(),
        )
    }
}
