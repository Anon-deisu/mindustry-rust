use crate::{
    save_post_load_consumer_plan::extend_unique_consumer_blockers,
    save_post_load_runtime_source_region::{
        find_or_push_source_region, find_source_region, source_region_name_for_stage_kind,
    },
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
        find_source_region(self.source_regions(), source_region_name, |region| {
            region.source_region_name
        })
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeSourceRegionReadiness> {
        let mut source_regions = Vec::new();

        for region in &self.regions {
            let source_region = find_or_push_source_region(
                &mut source_regions,
                region.source_region_name,
                |candidate: &SavePostLoadRuntimeSourceRegionReadiness| candidate.source_region_name,
                || SavePostLoadRuntimeSourceRegionReadiness {
                    source_region_name: region.source_region_name,
                    apply_now_step_count: 0,
                    awaiting_world_shell_step_count: 0,
                    blocked_step_count: 0,
                    deferred_step_count: 0,
                    blockers: Vec::new(),
                },
            );

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
            extend_unique_consumer_blockers(&mut source_region.blockers, &region.blockers);
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

#[cfg(test)]
mod test_support {
    use super::*;
    use crate::save_post_load_runtime_execution::test_support::{
        runtime_plan_seedable_test_observation as execution_seedable_test_observation,
        runtime_plan_test_observation as execution_test_observation,
    };
    use crate::SavePostLoadWorldIssue;

    pub(super) fn seedable_test_observation() -> SavePostLoadWorldObservation {
        execution_seedable_test_observation()
    }

    pub(super) fn blocked_pending_world_shell_test_observation() -> SavePostLoadWorldObservation {
        let mut observation = execution_test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;
        observation
    }

    pub(super) fn region(
        kind: SavePostLoadRuntimeRegionKind,
        step_count: usize,
        disposition: SavePostLoadConsumerRuntimeDisposition,
    ) -> SavePostLoadRuntimeRegionReadiness {
        region_with_blockers(kind, step_count, disposition, Vec::new())
    }

    pub(super) fn region_with_blockers(
        kind: SavePostLoadRuntimeRegionKind,
        step_count: usize,
        disposition: SavePostLoadConsumerRuntimeDisposition,
        blockers: Vec<SavePostLoadConsumerBlocker>,
    ) -> SavePostLoadRuntimeRegionReadiness {
        SavePostLoadRuntimeRegionReadiness {
            kind,
            source_region_name: kind.source_region_name(),
            step_count,
            disposition,
            blockers,
        }
    }

    pub(super) fn source_region(
        source_region_name: &'static str,
        apply_now_step_count: usize,
        awaiting_world_shell_step_count: usize,
        blocked_step_count: usize,
        deferred_step_count: usize,
    ) -> SavePostLoadRuntimeSourceRegionReadiness {
        source_region_with_blockers(
            source_region_name,
            apply_now_step_count,
            awaiting_world_shell_step_count,
            blocked_step_count,
            deferred_step_count,
            Vec::new(),
        )
    }

    pub(super) fn source_region_with_blockers(
        source_region_name: &'static str,
        apply_now_step_count: usize,
        awaiting_world_shell_step_count: usize,
        blocked_step_count: usize,
        deferred_step_count: usize,
        blockers: Vec<SavePostLoadConsumerBlocker>,
    ) -> SavePostLoadRuntimeSourceRegionReadiness {
        SavePostLoadRuntimeSourceRegionReadiness {
            source_region_name,
            apply_now_step_count,
            awaiting_world_shell_step_count,
            blocked_step_count,
            deferred_step_count,
            blockers,
        }
    }

    pub(super) fn contract_issue(issue: SavePostLoadWorldIssue) -> SavePostLoadConsumerBlocker {
        SavePostLoadConsumerBlocker::ContractIssue(issue)
    }

    pub(super) fn duplicate_entity_id(entity_id: i32) -> SavePostLoadConsumerBlocker {
        SavePostLoadConsumerBlocker::DuplicateEntityId(entity_id)
    }

    pub(super) fn invalid_building_reference(
        center_index: usize,
        tile_index: usize,
        block_id: u16,
    ) -> SavePostLoadConsumerBlocker {
        SavePostLoadConsumerBlocker::InvalidBuildingReference {
            center_index,
            tile_index,
            block_id,
        }
    }

    pub(super) fn skipped_entity(
        entity_index: usize,
        entity_id: i32,
        source_name: &str,
    ) -> SavePostLoadConsumerBlocker {
        SavePostLoadConsumerBlocker::SkippedEntity {
            entity_index,
            entity_id,
            source_name: source_name.to_string(),
            effective_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{
        blocked_pending_world_shell_test_observation, contract_issue, duplicate_entity_id,
        invalid_building_reference, region, region_with_blockers, seedable_test_observation,
        skipped_entity, source_region, source_region_with_blockers,
    };
    use super::*;
    use crate::SavePostLoadWorldIssue;

    #[test]
    fn runtime_readiness_maps_clean_regions_to_apply_now() {
        let observation = seedable_test_observation();

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
            Some(&region(
                SavePostLoadRuntimeRegionKind::WorldShell,
                1,
                SavePostLoadConsumerRuntimeDisposition::ApplyNow,
            ))
        );
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::Buildings),
            Some(&region(
                SavePostLoadRuntimeRegionKind::Buildings,
                1,
                SavePostLoadConsumerRuntimeDisposition::ApplyNow,
            ))
        );
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::LoadableEntities),
            Some(&region(
                SavePostLoadRuntimeRegionKind::LoadableEntities,
                3,
                SavePostLoadConsumerRuntimeDisposition::ApplyNow,
            ))
        );
        assert_eq!(
            readiness.region(SavePostLoadRuntimeRegionKind::SkippedEntities),
            Some(&region(
                SavePostLoadRuntimeRegionKind::SkippedEntities,
                0,
                SavePostLoadConsumerRuntimeDisposition::Deferred,
            ))
        );
        assert!(readiness
            .regions
            .iter()
            .filter(|region| region.kind != SavePostLoadRuntimeRegionKind::SkippedEntities)
            .all(SavePostLoadRuntimeRegionReadiness::can_apply_now));
        assert_eq!(
            source_regions,
            vec![
                source_region("map", 2, 0, 0, 0),
                source_region("entities", 7, 0, 0, 0),
                source_region("markers", 2, 0, 0, 0),
                source_region("custom", 3, 0, 0, 0),
            ]
        );
        assert_eq!(
            readiness.source_region("entities"),
            Some(source_region("entities", 7, 0, 0, 0))
        );
        assert_eq!(source_regions[1].total_step_count(), 7);
        assert!(!source_regions[1].has_blockers());
        assert!(!source_regions[1].has_pending_world_shell());
        assert!(!source_regions[1].has_deferred());
    }

    #[test]
    fn runtime_readiness_tracks_blocked_awaiting_and_deferred_regions_by_source_region() {
        let observation = blocked_pending_world_shell_test_observation();

        let readiness = observation.runtime_readiness();
        let source_regions = readiness.source_regions();

        assert!(!readiness.can_seed_runtime_apply);
        assert!(!readiness.world_shell_ready);
        assert_eq!(readiness.apply_now_step_count(), 4);
        assert_eq!(readiness.awaiting_world_shell_step_count(), 5);
        assert_eq!(readiness.blocked_step_count(), 4);
        assert_eq!(readiness.deferred_step_count(), 1);
        assert_eq!(
            readiness.regions,
            vec![
                region_with_blockers(
                    SavePostLoadRuntimeRegionKind::WorldShell,
                    1,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                        contract_issue(SavePostLoadWorldIssue::DuplicateWorldEntityIds),
                        contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                    ],
                ),
                region(
                    SavePostLoadRuntimeRegionKind::EntityRemaps,
                    2,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                ),
                region(
                    SavePostLoadRuntimeRegionKind::TeamPlans,
                    2,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                ),
                region(
                    SavePostLoadRuntimeRegionKind::Markers,
                    2,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                ),
                region(
                    SavePostLoadRuntimeRegionKind::StaticFog,
                    1,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                ),
                region(
                    SavePostLoadRuntimeRegionKind::CustomChunks,
                    2,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                ),
                region_with_blockers(
                    SavePostLoadRuntimeRegionKind::Buildings,
                    1,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                        invalid_building_reference(0, 0, 0x0153),
                    ],
                ),
                region_with_blockers(
                    SavePostLoadRuntimeRegionKind::LoadableEntities,
                    2,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::DuplicateWorldEntityIds),
                        contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                        duplicate_entity_id(42),
                    ],
                ),
                region_with_blockers(
                    SavePostLoadRuntimeRegionKind::SkippedEntities,
                    1,
                    SavePostLoadConsumerRuntimeDisposition::Deferred,
                    vec![skipped_entity(1, 43, "mod-unit")],
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
                source_region_with_blockers(
                    "map",
                    0,
                    0,
                    2,
                    0,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                        contract_issue(SavePostLoadWorldIssue::DuplicateWorldEntityIds),
                        contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                        invalid_building_reference(0, 0, 0x0153),
                    ],
                ),
                source_region_with_blockers(
                    "entities",
                    2,
                    2,
                    2,
                    1,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::DuplicateWorldEntityIds),
                        contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                        duplicate_entity_id(42),
                        skipped_entity(1, 43, "mod-unit"),
                    ],
                ),
                source_region("markers", 0, 2, 0, 0),
                source_region("custom", 2, 1, 0, 0),
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
        let region = region(
            SavePostLoadRuntimeRegionKind::CustomChunks,
            0,
            SavePostLoadConsumerRuntimeDisposition::ApplyNow,
        );

        assert!(region.can_apply_now());
        assert!(!region.has_blockers());
    }

    #[test]
    fn source_regions_preserve_first_seen_order_and_dedup_blockers() {
        let readiness = SavePostLoadRuntimeReadiness {
            can_seed_runtime_apply: false,
            world_shell_ready: false,
            regions: vec![
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "beta",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::WorldShell,
                        1,
                        SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        vec![
                            contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                            duplicate_entity_id(7),
                        ],
                    )
                },
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "alpha",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::EntityRemaps,
                        2,
                        SavePostLoadConsumerRuntimeDisposition::Blocked,
                        vec![
                            contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                            duplicate_entity_id(7),
                        ],
                    )
                },
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "beta",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::TeamPlans,
                        3,
                        SavePostLoadConsumerRuntimeDisposition::Deferred,
                        vec![
                            duplicate_entity_id(7),
                            contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                            duplicate_entity_id(8),
                        ],
                    )
                },
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "alpha",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::Markers,
                        4,
                        SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                        vec![
                            contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                            skipped_entity(1, 99, "mod-unit"),
                        ],
                    )
                },
            ],
        };

        assert_eq!(
            readiness.source_regions(),
            vec![
                source_region_with_blockers(
                    "beta",
                    1,
                    0,
                    0,
                    3,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                        duplicate_entity_id(7),
                        duplicate_entity_id(8),
                    ],
                ),
                source_region_with_blockers(
                    "alpha",
                    0,
                    4,
                    2,
                    0,
                    vec![
                        contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                        duplicate_entity_id(7),
                        skipped_entity(1, 99, "mod-unit"),
                    ],
                ),
            ]
        );
        assert_eq!(
            readiness.source_region("alpha"),
            Some(source_region_with_blockers(
                "alpha",
                0,
                4,
                2,
                0,
                vec![
                    contract_issue(SavePostLoadWorldIssue::BuildingCenterReferenceMismatch),
                    duplicate_entity_id(7),
                    skipped_entity(1, 99, "mod-unit"),
                ],
            ))
        );
        assert_eq!(
            readiness.source_region("beta"),
            Some(source_region_with_blockers(
                "beta",
                1,
                0,
                0,
                3,
                vec![
                    contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                    duplicate_entity_id(7),
                    duplicate_entity_id(8),
                ],
            ))
        );
    }

    #[test]
    fn source_regions_merge_repeated_source_names_without_dropping_counts_or_blockers() {
        let readiness = SavePostLoadRuntimeReadiness {
            can_seed_runtime_apply: false,
            world_shell_ready: false,
            regions: vec![
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "shared",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::WorldShell,
                        1,
                        SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        vec![duplicate_entity_id(1)],
                    )
                },
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "shared",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::TeamPlans,
                        2,
                        SavePostLoadConsumerRuntimeDisposition::Blocked,
                        vec![
                            duplicate_entity_id(1),
                            contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                        ],
                    )
                },
                SavePostLoadRuntimeRegionReadiness {
                    source_region_name: "shared",
                    ..region_with_blockers(
                        SavePostLoadRuntimeRegionKind::Markers,
                        3,
                        SavePostLoadConsumerRuntimeDisposition::Deferred,
                        vec![duplicate_entity_id(2)],
                    )
                },
            ],
        };

        let source_regions = readiness.source_regions();

        assert_eq!(source_regions.len(), 1);
        assert_eq!(
            source_regions[0],
            source_region_with_blockers(
                "shared",
                1,
                0,
                2,
                3,
                vec![
                    duplicate_entity_id(1),
                    contract_issue(SavePostLoadWorldIssue::EntitySummaryMismatch),
                    duplicate_entity_id(2),
                ],
            )
        );
        assert_eq!(source_regions[0].total_step_count(), 6);
        assert!(source_regions[0].has_blockers());
        assert!(!source_regions[0].has_pending_world_shell());
        assert!(source_regions[0].has_deferred());
        assert_eq!(
            readiness.source_region("shared"),
            Some(source_regions[0].clone())
        );
    }
}
