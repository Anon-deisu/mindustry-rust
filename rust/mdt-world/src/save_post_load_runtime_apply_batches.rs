use crate::save_post_load_runtime_script::expand_stage_steps;
use crate::{
    SavePostLoadConsumerApplyPlan, SavePostLoadConsumerBlocker,
    SavePostLoadConsumerRuntimeDisposition, SavePostLoadConsumerRuntimeHelper,
    SavePostLoadConsumerRuntimeStageHelper, SavePostLoadRuntimeApplyStep,
    SavePostLoadRuntimeSeedPlan, SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeApplyBatch {
    pub batch_index: usize,
    pub disposition: SavePostLoadConsumerRuntimeDisposition,
    pub step_count: usize,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
    pub stages: Vec<SavePostLoadConsumerRuntimeStageHelper>,
}

impl SavePostLoadRuntimeApplyBatch {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn can_apply_now(&self) -> bool {
        self.step_count > 0 && self.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeApplyBatchView {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub stage_count: usize,
    pub batches: Vec<SavePostLoadRuntimeApplyBatch>,
}

impl SavePostLoadRuntimeApplyBatchView {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn next_apply_now_batch(&self) -> Option<&SavePostLoadRuntimeApplyBatch> {
        self.batches.iter().find(|batch| batch.can_apply_now())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeApplyBatchPlan {
    pub batch_index: usize,
    pub disposition: SavePostLoadConsumerRuntimeDisposition,
    pub step_count: usize,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
    pub stages: Vec<SavePostLoadConsumerRuntimeStageHelper>,
    pub steps: Vec<SavePostLoadRuntimeApplyStep>,
}

impl SavePostLoadRuntimeApplyBatchPlan {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn can_apply_now(&self) -> bool {
        self.step_count > 0 && self.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeApplyBatchPlanView {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub stage_count: usize,
    pub batches: Vec<SavePostLoadRuntimeApplyBatchPlan>,
}

impl SavePostLoadRuntimeApplyBatchPlanView {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn next_apply_now_batch(&self) -> Option<&SavePostLoadRuntimeApplyBatchPlan> {
        self.batches.iter().find(|batch| batch.can_apply_now())
    }
}

impl SavePostLoadWorldObservation {
    pub fn runtime_apply_batch_view(&self) -> SavePostLoadRuntimeApplyBatchView {
        self.runtime_seed_plan().runtime_apply_batch_view()
    }

    pub fn runtime_apply_batch_plan_view(&self) -> SavePostLoadRuntimeApplyBatchPlanView {
        self.runtime_seed_plan().runtime_apply_batch_plan_view()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn runtime_apply_batch_view(&self) -> SavePostLoadRuntimeApplyBatchView {
        self.consumer_runtime_helper().runtime_apply_batch_view()
    }

    pub fn runtime_apply_batch_plan_view(&self) -> SavePostLoadRuntimeApplyBatchPlanView {
        let helper = self.consumer_runtime_helper();
        let mut batches: Vec<SavePostLoadRuntimeApplyBatchPlan> = Vec::new();

        for stage in helper.stages.iter().filter(|stage| stage.step_count > 0) {
            let mut stage_steps = Vec::new();
            expand_stage_steps(self, stage.kind, &mut stage_steps);
            debug_assert_eq!(stage.step_count, stage_steps.len());

            push_or_merge_runtime_apply_batch(
                &mut batches,
                stage,
                stage_steps,
                |batch, stage| batch.disposition == stage.disposition,
                |batch_index, stage, steps| SavePostLoadRuntimeApplyBatchPlan {
                    batch_index,
                    disposition: stage.disposition,
                    step_count: stage.step_count,
                    blockers: stage.blockers.clone(),
                    stages: vec![stage.clone()],
                    steps,
                },
                |batch, stage, steps| {
                    batch.step_count += stage.step_count;
                    extend_unique_blockers(&mut batch.blockers, &stage.blockers);
                    batch.stages.push(stage.clone());
                    batch.steps.extend(steps);
                },
            );
        }

        let stage_count = batches.iter().map(|batch| batch.stages.len()).sum();

        SavePostLoadRuntimeApplyBatchPlanView {
            can_seed_runtime_apply: helper.can_seed_runtime_apply,
            world_shell_ready: helper.world_shell_ready,
            stage_count,
            batches,
        }
    }
}

impl SavePostLoadConsumerApplyPlan {
    pub fn runtime_apply_batch_view(&self) -> SavePostLoadRuntimeApplyBatchView {
        self.consumer_runtime_helper().runtime_apply_batch_view()
    }
}

fn push_or_merge_runtime_apply_batch<B, P, FCreate, FCanMerge, FMerge>(
    batches: &mut Vec<B>,
    stage: &SavePostLoadConsumerRuntimeStageHelper,
    payload: P,
    mut can_merge: FCanMerge,
    mut create_batch: FCreate,
    mut merge_batch: FMerge,
) where
    FCreate: FnMut(usize, &SavePostLoadConsumerRuntimeStageHelper, P) -> B,
    FCanMerge: FnMut(&B, &SavePostLoadConsumerRuntimeStageHelper) -> bool,
    FMerge: FnMut(&mut B, &SavePostLoadConsumerRuntimeStageHelper, P),
{
    let should_merge = batches.last().is_some_and(|batch| can_merge(batch, stage));
    if should_merge {
        if let Some(batch) = batches.last_mut() {
            merge_batch(batch, stage, payload);
        }
    } else {
        batches.push(create_batch(batches.len(), stage, payload));
    }
}

impl SavePostLoadConsumerRuntimeHelper {
    pub fn runtime_apply_batch_view(&self) -> SavePostLoadRuntimeApplyBatchView {
        let mut batches: Vec<SavePostLoadRuntimeApplyBatch> = Vec::new();

        for stage in self.stages.iter().filter(|stage| stage.step_count > 0) {
            push_or_merge_runtime_apply_batch(
                &mut batches,
                stage,
                (),
                |batch, stage| batch.disposition == stage.disposition,
                |batch_index, stage, ()| SavePostLoadRuntimeApplyBatch {
                    batch_index,
                    disposition: stage.disposition,
                    step_count: stage.step_count,
                    blockers: stage.blockers.clone(),
                    stages: vec![stage.clone()],
                },
                |batch, stage, ()| {
                    batch.step_count += stage.step_count;
                    extend_unique_blockers(&mut batch.blockers, &stage.blockers);
                    batch.stages.push(stage.clone());
                },
            );
        }

        let stage_count = batches.iter().map(|batch| batch.stages.len()).sum();

        SavePostLoadRuntimeApplyBatchView {
            can_seed_runtime_apply: self.can_seed_runtime_apply,
            world_shell_ready: self.world_shell_ready,
            stage_count,
            batches,
        }
    }
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
        SavePostLoadConsumerStageKind, SavePostLoadWorldIssue, StaticFogChunk, StaticFogTeam,
        TeamPlan, TeamPlanGroup, TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn runtime_apply_batch_view_collapses_clean_runtime_stages_into_single_apply_batch() {
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

        let batch_view = observation.runtime_apply_batch_view();

        assert!(batch_view.can_seed_runtime_apply);
        assert!(batch_view.world_shell_ready);
        assert_eq!(batch_view.stage_count, 8);
        assert_eq!(batch_view.batch_count(), 1);
        assert_eq!(
            batch_view.batches,
            vec![SavePostLoadRuntimeApplyBatch {
                batch_index: 0,
                disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                step_count: 14,
                blockers: Vec::new(),
                stages: vec![
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::WorldShell,
                        step_count: 1,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::EntityRemaps,
                        step_count: 2,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::TeamPlans,
                        step_count: 2,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::Markers,
                        step_count: 2,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::StaticFog,
                        step_count: 1,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::CustomChunks,
                        step_count: 2,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::Buildings,
                        step_count: 1,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                    SavePostLoadConsumerRuntimeStageHelper {
                        kind: SavePostLoadConsumerStageKind::LoadableEntities,
                        step_count: 3,
                        disposition: SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                        blockers: Vec::new(),
                    },
                ],
            }]
        );
        assert!(batch_view.batches[0].can_apply_now());
        assert!(!batch_view.batches[0].has_blockers());
        assert_eq!(batch_view.next_apply_now_batch(), Some(&batch_view.batches[0]));
    }

    #[test]
    fn runtime_apply_batch_view_preserves_deterministic_batch_order_across_dispositions() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let batch_view = observation.runtime_apply_batch_view();

        assert!(!batch_view.can_seed_runtime_apply);
        assert!(!batch_view.world_shell_ready);
        assert_eq!(batch_view.stage_count, 9);
        assert_eq!(batch_view.batch_count(), 6);
        assert_eq!(
            batch_view
                .batches
                .iter()
                .map(|batch| (
                    batch.batch_index,
                    batch.disposition,
                    batch.step_count,
                    batch
                        .stages
                        .iter()
                        .map(|stage| stage.kind)
                        .collect::<Vec<_>>(),
                    batch.blockers.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    0,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    1,
                    vec![SavePostLoadConsumerStageKind::WorldShell],
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
                    1,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    2,
                    vec![SavePostLoadConsumerStageKind::EntityRemaps],
                    Vec::new(),
                ),
                (
                    2,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                    5,
                    vec![
                        SavePostLoadConsumerStageKind::TeamPlans,
                        SavePostLoadConsumerStageKind::Markers,
                        SavePostLoadConsumerStageKind::StaticFog,
                    ],
                    Vec::new(),
                ),
                (
                    3,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    2,
                    vec![SavePostLoadConsumerStageKind::CustomChunks],
                    Vec::new(),
                ),
                (
                    4,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    3,
                    vec![
                        SavePostLoadConsumerStageKind::Buildings,
                        SavePostLoadConsumerStageKind::LoadableEntities,
                    ],
                    vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                        ),
                        SavePostLoadConsumerBlocker::InvalidBuildingReference {
                            center_index: 0,
                            tile_index: 0,
                            block_id: 0x0153,
                        },
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
                    5,
                    SavePostLoadConsumerRuntimeDisposition::Deferred,
                    1,
                    vec![SavePostLoadConsumerStageKind::SkippedEntities],
                    vec![SavePostLoadConsumerBlocker::SkippedEntity {
                        entity_index: 1,
                        entity_id: 43,
                        source_name: "mod-unit".to_string(),
                        effective_name: None,
                    }],
                ),
            ]
        );
        assert!(batch_view.batches[1].can_apply_now());
        assert!(!batch_view.batches[4].can_apply_now());
        assert!(batch_view.batches[4].has_blockers());
        assert_eq!(batch_view.next_apply_now_batch(), Some(&batch_view.batches[1]));
    }

    #[test]
    fn runtime_apply_batch_plan_view_expands_clean_batches_into_exact_steps() {
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

        let batch_plan_view = observation.runtime_apply_batch_plan_view();

        assert!(batch_plan_view.can_seed_runtime_apply);
        assert!(batch_plan_view.world_shell_ready);
        assert_eq!(batch_plan_view.stage_count, 8);
        assert_eq!(batch_plan_view.batch_count(), 1);
        assert_eq!(
            batch_plan_view
                .next_apply_now_batch()
                .map(|batch| &batch.steps),
            Some(&vec![
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
            ])
        );
    }

    #[test]
    fn runtime_apply_batch_plan_view_preserves_exact_steps_for_applyable_and_pending_batches() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let batch_plan_view = observation.runtime_apply_batch_plan_view();

        assert!(!batch_plan_view.can_seed_runtime_apply);
        assert!(!batch_plan_view.world_shell_ready);
        assert_eq!(batch_plan_view.stage_count, 9);
        assert_eq!(batch_plan_view.batch_count(), 6);
        assert_eq!(
            batch_plan_view
                .next_apply_now_batch()
                .map(|batch| (batch.batch_index, batch.steps.clone())),
            Some((
                1,
                vec![
                    SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
                    SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 1 },
                ],
            ))
        );
        assert_eq!(
            batch_plan_view
                .batches
                .iter()
                .map(|batch| (batch.batch_index, batch.disposition, batch.steps.clone(),))
                .collect::<Vec<_>>(),
            vec![
                (
                    0,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![SavePostLoadRuntimeApplyStep::WorldShell],
                ),
                (
                    1,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    vec![
                        SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
                        SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 1 },
                    ],
                ),
                (
                    2,
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
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
                    ],
                ),
                (
                    3,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    vec![
                        SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 },
                        SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 1 },
                    ],
                ),
                (
                    4,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![
                        SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                        SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                        SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 },
                    ],
                ),
                (
                    5,
                    SavePostLoadConsumerRuntimeDisposition::Deferred,
                    vec![SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }],
                ),
            ]
        );
    }

    #[test]
    fn runtime_apply_batch_view_and_plan_view_share_batch_merging_and_counts() {
        let observation = test_observation();

        let batch_view = observation.runtime_apply_batch_view();
        let batch_plan_view = observation.runtime_apply_batch_plan_view();

        assert_eq!(batch_view.stage_count, batch_plan_view.stage_count);
        assert_eq!(batch_view.batch_count(), batch_plan_view.batch_count());
        assert_eq!(
            batch_view
                .batches
                .iter()
                .map(|batch| (batch.batch_index, batch.disposition, batch.stages.len()))
                .collect::<Vec<_>>(),
            batch_plan_view
                .batches
                .iter()
                .map(|batch| (batch.batch_index, batch.disposition, batch.stages.len()))
                .collect::<Vec<_>>(),
        );
        assert!(batch_view.batch_count() < batch_view.stage_count);
        assert!(batch_view
            .batches
            .windows(2)
            .all(|window| window[0].disposition != window[1].disposition));
    }

    #[test]
    fn extend_unique_blockers_preserves_first_seen_order_and_skips_duplicates() {
        let mut blockers = vec![
            SavePostLoadConsumerBlocker::ContractIssue(
                SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
            ),
            SavePostLoadConsumerBlocker::DuplicateEntityId(42),
        ];

        extend_unique_blockers(
            &mut blockers,
            &[
                SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                ),
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                ),
                SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: 1,
                    tile_index: 2,
                    block_id: 0x1234,
                },
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                ),
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::EntitySummaryMismatch,
                ),
                SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: 1,
                    tile_index: 2,
                    block_id: 0x1234,
                },
            ],
        );

        assert_eq!(
            blockers,
            vec![
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                ),
                SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                ),
                SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: 1,
                    tile_index: 2,
                    block_id: 0x1234,
                },
                SavePostLoadConsumerBlocker::ContractIssue(
                    SavePostLoadWorldIssue::EntitySummaryMismatch,
                ),
            ]
        );
    }

    #[test]
    fn runtime_apply_batch_view_reports_no_batches_for_empty_runtime_helper() {
        let helper = SavePostLoadConsumerRuntimeHelper {
            can_seed_runtime_apply: false,
            world_shell_ready: false,
            stages: Vec::new(),
        };

        let batch_view = helper.runtime_apply_batch_view();

        assert!(!batch_view.can_seed_runtime_apply);
        assert!(!batch_view.world_shell_ready);
        assert_eq!(batch_view.stage_count, 0);
        assert_eq!(batch_view.batch_count(), 0);
        assert!(batch_view.batches.is_empty());
        assert_eq!(batch_view.next_apply_now_batch(), None);
    }

    #[test]
    fn runtime_apply_batch_plan_view_keeps_world_shell_when_auxiliary_seeds_are_empty() {
        let mut plan = test_observation().runtime_seed_plan();
        plan.entity_remap_seeds.clear();
        plan.team_plan_seeds.clear();
        plan.marker_seeds.clear();
        plan.static_fog_seed = None;
        plan.custom_chunk_seeds.clear();
        plan.building_seeds.clear();
        plan.loadable_entity_seeds.clear();
        plan.skipped_entity_seeds.clear();

        let batch_plan_view = plan.runtime_apply_batch_plan_view();

        assert!(!batch_plan_view.can_seed_runtime_apply);
        assert!(!batch_plan_view.world_shell_ready);
        assert_eq!(batch_plan_view.stage_count, 1);
        assert_eq!(batch_plan_view.batch_count(), 1);
        assert_eq!(
            batch_plan_view.batches[0].disposition,
            SavePostLoadConsumerRuntimeDisposition::Blocked
        );
        assert_eq!(
            batch_plan_view.batches[0].steps,
            vec![SavePostLoadRuntimeApplyStep::WorldShell]
        );
        assert_eq!(batch_plan_view.next_apply_now_batch(), None);
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
