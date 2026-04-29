use crate::{
    SavePostLoadConsumerRuntimeDisposition, SavePostLoadConsumerRuntimeHelper,
    SavePostLoadConsumerStageKind, SavePostLoadRuntimeSeedPlan, SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

impl SavePostLoadRuntimeApplyStep {
    pub fn targets_world_semantics(&self) -> bool {
        matches!(
            self,
            SavePostLoadRuntimeApplyStep::WorldShell
                | SavePostLoadRuntimeApplyStep::TeamPlan { .. }
                | SavePostLoadRuntimeApplyStep::Marker { .. }
                | SavePostLoadRuntimeApplyStep::StaticFog
                | SavePostLoadRuntimeApplyStep::Building { .. }
                | SavePostLoadRuntimeApplyStep::LoadableEntity { .. }
        )
    }
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
    use crate::save_post_load_runtime_execution::test_support::{
        seedable_test_observation, test_observation,
    };

    fn assert_script_counts_align_with_runtime_helper(
        observation: &SavePostLoadWorldObservation,
        script: &SavePostLoadRuntimeApplyScript,
    ) {
        let helper = observation.consumer_runtime_helper();

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

    fn apply_blocked_pending_world_shell_fixture(observation: &mut SavePostLoadWorldObservation) {
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;
    }

    fn blocked_pending_world_shell_observation() -> SavePostLoadWorldObservation {
        let mut observation = test_observation();
        apply_blocked_pending_world_shell_fixture(&mut observation);
        observation
    }

    #[test]
    fn runtime_apply_script_counts_align_with_runtime_helper() {
        let observation = seedable_test_observation();

        let script = observation.runtime_apply_script();

        assert!(script.can_seed_runtime_apply);
        assert!(script.world_shell_ready);
        assert_script_counts_align_with_runtime_helper(&observation, &script);
    }

    #[test]
    fn runtime_apply_script_preserves_step_order_for_clean_seedable_plan() {
        let observation = seedable_test_observation();

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
        let observation = blocked_pending_world_shell_observation();

        let script = observation.runtime_apply_script();

        assert!(!script.can_seed_runtime_apply);
        assert!(!script.world_shell_ready);
        assert_script_counts_align_with_runtime_helper(&observation, &script);
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

    #[test]
    fn runtime_apply_script_keeps_skipped_entities_and_empty_stages_out_of_world_semantics() {
        let observation = test_observation();
        let script = observation.runtime_apply_script();

        assert!(script
            .deferred_steps
            .iter()
            .all(|step| !step.targets_world_semantics()));
        assert!(
            !SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }
                .targets_world_semantics()
        );

        let mut plan = observation.runtime_seed_plan();
        plan.static_fog_seed = None;
        let mut stage_steps = Vec::new();

        expand_stage_steps(
            &plan,
            crate::SavePostLoadConsumerStageKind::StaticFog,
            &mut stage_steps,
        );

        assert!(stage_steps.is_empty());
    }
}
