use crate::save_post_load_consumer_plan::extend_unique_consumer_blockers;
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
                    extend_unique_consumer_blockers(&mut batch.blockers, &stage.blockers);
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
                    extend_unique_consumer_blockers(&mut batch.blockers, &stage.blockers);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_post_load_runtime_seed_plan::save_post_load_runtime_test_support::{
        seedable_test_observation, test_observation,
    };
    use crate::{SavePostLoadConsumerStageKind, SavePostLoadWorldIssue};

    fn clean_apply_steps() -> Vec<SavePostLoadRuntimeApplyStep> {
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
    }

    fn blocked_apply_now_next_batch_steps() -> Vec<SavePostLoadRuntimeApplyStep> {
        vec![
            SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
            SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 1 },
        ]
    }

    fn apply_blocked_pending_world_shell_fixture(
        observation: &mut crate::SavePostLoadWorldObservation,
    ) {
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;
    }

    fn blocked_pending_world_shell_observation() -> crate::SavePostLoadWorldObservation {
        let mut observation = test_observation();
        apply_blocked_pending_world_shell_fixture(&mut observation);
        observation
    }

    fn batch_view_summary(
        batch_view: &SavePostLoadRuntimeApplyBatchView,
    ) -> Vec<(
        usize,
        SavePostLoadConsumerRuntimeDisposition,
        usize,
        Vec<SavePostLoadConsumerStageKind>,
        Vec<SavePostLoadConsumerBlocker>,
    )> {
        batch_view
            .batches
            .iter()
            .map(|batch| {
                (
                    batch.batch_index,
                    batch.disposition,
                    batch.step_count,
                    batch.stages.iter().map(|stage| stage.kind).collect(),
                    batch.blockers.clone(),
                )
            })
            .collect()
    }

    fn batch_plan_summary(
        batch_plan_view: &SavePostLoadRuntimeApplyBatchPlanView,
    ) -> Vec<(
        usize,
        SavePostLoadConsumerRuntimeDisposition,
        Vec<SavePostLoadRuntimeApplyStep>,
    )> {
        batch_plan_view
            .batches
            .iter()
            .map(|batch| (batch.batch_index, batch.disposition, batch.steps.clone()))
            .collect()
    }

    #[test]
    fn runtime_apply_batch_view_collapses_clean_runtime_stages_into_single_apply_batch() {
        let observation = seedable_test_observation();

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
        assert_eq!(
            batch_view.next_apply_now_batch(),
            Some(&batch_view.batches[0])
        );
    }

    #[test]
    fn runtime_apply_batch_view_preserves_deterministic_batch_order_across_dispositions() {
        let observation = blocked_pending_world_shell_observation();

        let batch_view = observation.runtime_apply_batch_view();

        assert!(!batch_view.can_seed_runtime_apply);
        assert!(!batch_view.world_shell_ready);
        assert_eq!(batch_view.stage_count, 9);
        assert_eq!(batch_view.batch_count(), 6);
        assert_eq!(
            batch_view_summary(&batch_view),
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
        assert_eq!(
            batch_view.next_apply_now_batch(),
            Some(&batch_view.batches[1])
        );
    }

    #[test]
    fn runtime_apply_batch_plan_view_expands_clean_batches_into_exact_steps() {
        let observation = seedable_test_observation();

        let batch_plan_view = observation.runtime_apply_batch_plan_view();

        assert!(batch_plan_view.can_seed_runtime_apply);
        assert!(batch_plan_view.world_shell_ready);
        assert_eq!(batch_plan_view.stage_count, 8);
        assert_eq!(batch_plan_view.batch_count(), 1);
        assert_eq!(
            batch_plan_view
                .next_apply_now_batch()
                .map(|batch| &batch.steps),
            Some(&clean_apply_steps())
        );
    }

    #[test]
    fn runtime_apply_batch_plan_view_preserves_exact_steps_for_applyable_and_pending_batches() {
        let observation = blocked_pending_world_shell_observation();

        let batch_plan_view = observation.runtime_apply_batch_plan_view();

        assert!(!batch_plan_view.can_seed_runtime_apply);
        assert!(!batch_plan_view.world_shell_ready);
        assert_eq!(batch_plan_view.stage_count, 9);
        assert_eq!(batch_plan_view.batch_count(), 6);
        assert_eq!(
            batch_plan_view
                .next_apply_now_batch()
                .map(|batch| (batch.batch_index, batch.steps.clone())),
            Some((1, blocked_apply_now_next_batch_steps()))
        );
        assert_eq!(
            batch_plan_summary(&batch_plan_view),
            vec![
                (
                    0,
                    SavePostLoadConsumerRuntimeDisposition::Blocked,
                    vec![SavePostLoadRuntimeApplyStep::WorldShell],
                ),
                (
                    1,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    blocked_apply_now_next_batch_steps(),
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
}
