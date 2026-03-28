use crate::{
    save_post_load_runtime_source_region::source_region_name_for_stage_kind,
    SavePostLoadConsumerBlocker, SavePostLoadConsumerRuntimeDisposition,
    SavePostLoadConsumerRuntimeHelper, SavePostLoadConsumerStageKind, SavePostLoadRuntimeSeedPlan,
    SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePostLoadRuntimeRegionKind {
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

impl SavePostLoadRuntimeRegionKind {
    pub fn source_region_name(&self) -> &'static str {
        source_region_name_for_stage_kind(self.stage_kind())
    }

    const fn stage_kind(&self) -> SavePostLoadConsumerStageKind {
        match self {
            SavePostLoadRuntimeRegionKind::WorldShell => SavePostLoadConsumerStageKind::WorldShell,
            SavePostLoadRuntimeRegionKind::EntityRemaps => {
                SavePostLoadConsumerStageKind::EntityRemaps
            }
            SavePostLoadRuntimeRegionKind::TeamPlans => SavePostLoadConsumerStageKind::TeamPlans,
            SavePostLoadRuntimeRegionKind::Markers => SavePostLoadConsumerStageKind::Markers,
            SavePostLoadRuntimeRegionKind::StaticFog => SavePostLoadConsumerStageKind::StaticFog,
            SavePostLoadRuntimeRegionKind::CustomChunks => {
                SavePostLoadConsumerStageKind::CustomChunks
            }
            SavePostLoadRuntimeRegionKind::Buildings => SavePostLoadConsumerStageKind::Buildings,
            SavePostLoadRuntimeRegionKind::LoadableEntities => {
                SavePostLoadConsumerStageKind::LoadableEntities
            }
            SavePostLoadRuntimeRegionKind::SkippedEntities => {
                SavePostLoadConsumerStageKind::SkippedEntities
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeRegionReadiness {
    pub kind: SavePostLoadRuntimeRegionKind,
    pub source_region_name: &'static str,
    pub step_count: usize,
    pub disposition: SavePostLoadConsumerRuntimeDisposition,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
}

impl SavePostLoadRuntimeRegionReadiness {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn can_apply_now(&self) -> bool {
        self.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeReadiness {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub regions: Vec<SavePostLoadRuntimeRegionReadiness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeSourceRegionReadiness {
    pub source_region_name: &'static str,
    pub apply_now_step_count: usize,
    pub awaiting_world_shell_step_count: usize,
    pub blocked_step_count: usize,
    pub deferred_step_count: usize,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
}

impl SavePostLoadRuntimeSourceRegionReadiness {
    pub fn total_step_count(&self) -> usize {
        self.apply_now_step_count
            + self.awaiting_world_shell_step_count
            + self.blocked_step_count
            + self.deferred_step_count
    }

    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn has_pending_world_shell(&self) -> bool {
        self.awaiting_world_shell_step_count > 0
    }

    pub fn has_deferred(&self) -> bool {
        self.deferred_step_count > 0
    }
}

impl SavePostLoadRuntimeReadiness {
    pub fn region(
        &self,
        kind: SavePostLoadRuntimeRegionKind,
    ) -> Option<&SavePostLoadRuntimeRegionReadiness> {
        self.regions.iter().find(|region| region.kind == kind)
    }

    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadRuntimeSourceRegionReadiness> {
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeSourceRegionReadiness> {
        let mut source_regions = Vec::new();

        for region in &self.regions {
            let source_region = match source_regions.iter_mut().find(
                |candidate: &&mut SavePostLoadRuntimeSourceRegionReadiness| {
                    candidate.source_region_name == region.source_region_name
                },
            ) {
                Some(source_region) => source_region,
                None => {
                    source_regions.push(SavePostLoadRuntimeSourceRegionReadiness {
                        source_region_name: region.source_region_name,
                        apply_now_step_count: 0,
                        awaiting_world_shell_step_count: 0,
                        blocked_step_count: 0,
                        deferred_step_count: 0,
                        blockers: Vec::new(),
                    });
                    source_regions
                        .last_mut()
                        .expect("source region was just pushed")
                }
            };

            match region.disposition {
                SavePostLoadConsumerRuntimeDisposition::ApplyNow => {
                    source_region.apply_now_step_count += region.step_count;
                }
                SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell => {
                    source_region.awaiting_world_shell_step_count += region.step_count;
                }
                SavePostLoadConsumerRuntimeDisposition::Blocked => {
                    source_region.blocked_step_count += region.step_count;
                }
                SavePostLoadConsumerRuntimeDisposition::Deferred => {
                    source_region.deferred_step_count += region.step_count;
                }
            }
            extend_unique_blockers(&mut source_region.blockers, &region.blockers);
        }

        source_regions
    }

    pub fn apply_now_step_count(&self) -> usize {
        readiness_step_count(self, SavePostLoadConsumerRuntimeDisposition::ApplyNow)
    }

    pub fn awaiting_world_shell_step_count(&self) -> usize {
        readiness_step_count(
            self,
            SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
        )
    }

    pub fn blocked_step_count(&self) -> usize {
        readiness_step_count(self, SavePostLoadConsumerRuntimeDisposition::Blocked)
    }

    pub fn deferred_step_count(&self) -> usize {
        readiness_step_count(self, SavePostLoadConsumerRuntimeDisposition::Deferred)
    }
}

impl SavePostLoadWorldObservation {
    pub fn runtime_readiness(&self) -> SavePostLoadRuntimeReadiness {
        self.runtime_seed_plan().runtime_readiness()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn runtime_readiness(&self) -> SavePostLoadRuntimeReadiness {
        self.consumer_runtime_helper().runtime_readiness()
    }
}

impl SavePostLoadConsumerRuntimeHelper {
    pub fn runtime_readiness(&self) -> SavePostLoadRuntimeReadiness {
        SavePostLoadRuntimeReadiness {
            can_seed_runtime_apply: self.can_seed_runtime_apply,
            world_shell_ready: self.world_shell_ready,
            regions: self
                .stages
                .iter()
                .map(|stage| {
                    let kind = region_kind(stage.kind);
                    SavePostLoadRuntimeRegionReadiness {
                        kind,
                        source_region_name: kind.source_region_name(),
                        step_count: stage.step_count,
                        disposition: stage.disposition,
                        blockers: stage.blockers.clone(),
                    }
                })
                .collect(),
        }
    }
}

fn region_kind(kind: SavePostLoadConsumerStageKind) -> SavePostLoadRuntimeRegionKind {
    match kind {
        SavePostLoadConsumerStageKind::WorldShell => SavePostLoadRuntimeRegionKind::WorldShell,
        SavePostLoadConsumerStageKind::EntityRemaps => SavePostLoadRuntimeRegionKind::EntityRemaps,
        SavePostLoadConsumerStageKind::TeamPlans => SavePostLoadRuntimeRegionKind::TeamPlans,
        SavePostLoadConsumerStageKind::Markers => SavePostLoadRuntimeRegionKind::Markers,
        SavePostLoadConsumerStageKind::StaticFog => SavePostLoadRuntimeRegionKind::StaticFog,
        SavePostLoadConsumerStageKind::CustomChunks => SavePostLoadRuntimeRegionKind::CustomChunks,
        SavePostLoadConsumerStageKind::Buildings => SavePostLoadRuntimeRegionKind::Buildings,
        SavePostLoadConsumerStageKind::LoadableEntities => {
            SavePostLoadRuntimeRegionKind::LoadableEntities
        }
        SavePostLoadConsumerStageKind::SkippedEntities => {
            SavePostLoadRuntimeRegionKind::SkippedEntities
        }
    }
}

fn readiness_step_count(
    readiness: &SavePostLoadRuntimeReadiness,
    disposition: SavePostLoadConsumerRuntimeDisposition,
) -> usize {
    readiness
        .regions
        .iter()
        .filter(|region| region.disposition == disposition)
        .map(|region| region.step_count)
        .sum()
}

fn extend_unique_blockers(
    blockers: &mut Vec<SavePostLoadConsumerBlocker>,
    additions: &[SavePostLoadConsumerBlocker],
) {
    for blocker in additions {
        if !blockers.contains(blocker) {
            blockers.push(blocker.clone());
        }
    }
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
        SavePostLoadWorldIssue, StaticFogChunk, StaticFogTeam, TeamPlan, TeamPlanGroup, TileModel,
        TypeIoValue, WorldModel,
    };

    #[test]
    fn runtime_readiness_maps_clean_regions_to_apply_now() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let readiness = observation.runtime_readiness();
        let source_regions = readiness.source_regions();

        assert!(readiness.can_seed_runtime_apply);
        assert!(readiness.world_shell_ready);
        assert_eq!(readiness.apply_now_step_count(), 14);
        assert_eq!(readiness.awaiting_world_shell_step_count(), 0);
        assert_eq!(readiness.blocked_step_count(), 0);
        assert_eq!(readiness.deferred_step_count(), 0);
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::WorldShell),
            Some(&SavePostLoadRuntimeRegionReadiness {
                kind: SavePostLoadRuntimeRegionKind::WorldShell,
                source_region_name: "map",
                step_count: 1,
                disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                blockers: Vec::new(),
            })
        );
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::Buildings),
            Some(&SavePostLoadRuntimeRegionReadiness {
                kind: SavePostLoadRuntimeRegionKind::Buildings,
                source_region_name: "map",
                step_count: 1,
                disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                blockers: Vec::new(),
            })
        );
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::LoadableEntities),
            Some(&SavePostLoadRuntimeRegionReadiness {
                kind: SavePostLoadRuntimeRegionKind::LoadableEntities,
                source_region_name: "entities",
                step_count: 3,
                disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                blockers: Vec::new(),
            })
        );
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::SkippedEntities),
            Some(&SavePostLoadRuntimeRegionReadiness {
                kind: SavePostLoadRuntimeRegionKind::SkippedEntities,
                source_region_name: "entities",
                step_count: 0,
                disposition: SavePostLoadConsumerRuntimeDisposition::Deferred,
                blockers: Vec::new(),
            })
        );
        assert!(readiness
            .regions
            .iter()
            .filter(|region| region.kind != SavePostLoadRuntimeRegionKind::SkippedEntities)
            .all(SavePostLoadRuntimeRegionReadiness::can_apply_now));
        assert_eq!(
            source_regions,
            vec![
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "map",
                    apply_now_step_count: 2,
                    awaiting_world_shell_step_count: 0,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "entities",
                    apply_now_step_count: 7,
                    awaiting_world_shell_step_count: 0,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "markers",
                    apply_now_step_count: 2,
                    awaiting_world_shell_step_count: 0,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "custom",
                    apply_now_step_count: 3,
                    awaiting_world_shell_step_count: 0,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
            ]
        );
        assert_eq!(
            readiness.source_region("entities"),
            Some(SavePostLoadRuntimeSourceRegionReadiness {
                source_region_name: "entities",
                apply_now_step_count: 7,
                awaiting_world_shell_step_count: 0,
                blocked_step_count: 0,
                deferred_step_count: 0,
                blockers: Vec::new(),
            })
        );
        assert_eq!(source_regions[1].total_step_count(), 7);
        assert!(!source_regions[1].has_blockers());
        assert!(!source_regions[1].has_pending_world_shell());
        assert!(!source_regions[1].has_deferred());
    }

    #[test]
    fn runtime_readiness_tracks_blocked_awaiting_and_deferred_regions_by_source_region() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let readiness = observation.runtime_readiness();
        let source_regions = readiness.source_regions();

        assert!(!readiness.can_seed_runtime_apply);
        assert!(!readiness.world_shell_ready);
        assert_eq!(readiness.apply_now_step_count(), 4);
        assert_eq!(readiness.awaiting_world_shell_step_count(), 5);
        assert_eq!(readiness.blocked_step_count(), 4);
        assert_eq!(readiness.deferred_step_count(), 1);
        assert_eq!(
            readiness
                .regions
                .iter()
                .map(|region| (
                    region.kind,
                    region.source_region_name,
                    region.step_count,
                    region.disposition,
                    region.blockers.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    SavePostLoadRuntimeRegionKind::WorldShell,
                    "map",
                    1,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
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
                    ],
                ),
                (
                    SavePostLoadRuntimeRegionKind::EntityRemaps,
                    "entities",
                    2,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    Vec::new(),
                ),
                (
                    SavePostLoadRuntimeRegionKind::TeamPlans,
                    "entities",
                    2,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                    Vec::new(),
                ),
                (
                    SavePostLoadRuntimeRegionKind::Markers,
                    "markers",
                    2,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                    Vec::new(),
                ),
                (
                    SavePostLoadRuntimeRegionKind::StaticFog,
                    "custom",
                    1,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                    Vec::new(),
                ),
                (
                    SavePostLoadRuntimeRegionKind::CustomChunks,
                    "custom",
                    2,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    Vec::new(),
                ),
                (
                    SavePostLoadRuntimeRegionKind::Buildings,
                    "map",
                    1,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                        ),
                        SavePostLoadConsumerBlocker::InvalidBuildingReference {
                            center_index: 0,
                            tile_index: 0,
                            block_id: 0x0153,
                        },
                    ],
                ),
                (
                    SavePostLoadRuntimeRegionKind::LoadableEntities,
                    "entities",
                    2,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                        ),
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::EntitySummaryMismatch,
                        ),
                        SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                    ],
                ),
                (
                    SavePostLoadRuntimeRegionKind::SkippedEntities,
                    "entities",
                    1,
                    SavePostLoadConsumerRuntimeDisposition::Deferred,
                    vec![SavePostLoadConsumerBlocker::SkippedEntity {
                        entity_index: 1,
                        entity_id: 43,
                        source_name: "mod-unit".to_string(),
                        effective_name: None,
                    }],
                ),
            ]
        );
        assert!(!readiness
            .region(SavePostLoadRuntimeRegionKind::Buildings)
            .unwrap()
            .can_apply_now());
        assert!(readiness
            .region(SavePostLoadRuntimeRegionKind::Buildings)
            .unwrap()
            .has_blockers());
        assert_eq!(
            source_regions,
            vec![
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "map",
                    apply_now_step_count: 0,
                    awaiting_world_shell_step_count: 0,
                    blocked_step_count: 2,
                    deferred_step_count: 0,
                    blockers: vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                        ),
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                        ),
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::EntitySummaryMismatch,
                        ),
                        SavePostLoadConsumerBlocker::InvalidBuildingReference {
                            center_index: 0,
                            tile_index: 0,
                            block_id: 0x0153,
                        },
                    ],
                },
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "entities",
                    apply_now_step_count: 2,
                    awaiting_world_shell_step_count: 2,
                    blocked_step_count: 2,
                    deferred_step_count: 1,
                    blockers: vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                        ),
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::EntitySummaryMismatch,
                        ),
                        SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                        SavePostLoadConsumerBlocker::SkippedEntity {
                            entity_index: 1,
                            entity_id: 43,
                            source_name: "mod-unit".to_string(),
                            effective_name: None,
                        },
                    ],
                },
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "markers",
                    apply_now_step_count: 0,
                    awaiting_world_shell_step_count: 2,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
                SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: "custom",
                    apply_now_step_count: 2,
                    awaiting_world_shell_step_count: 1,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
            ]
        );
        let entities = readiness
            .source_region("entities")
            .expect("entities source region should be present");
        assert_eq!(entities.total_step_count(), 7);
        assert!(entities.has_blockers());
        assert!(entities.has_pending_world_shell());
        assert!(entities.has_deferred());
    }

    #[test]
    fn runtime_readiness_can_apply_now_accepts_zero_step_apply_now_regions() {
        let region = SavePostLoadRuntimeRegionReadiness {
            kind: SavePostLoadRuntimeRegionKind::CustomChunks,
            source_region_name: "custom",
            step_count: 0,
            disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
            blockers: Vec::new(),
        };

        assert!(region.can_apply_now());
        assert!(!region.has_blockers());
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
