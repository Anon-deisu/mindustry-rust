use crate::{
    save_post_load_runtime_source_region::{
        find_or_push_source_region, find_source_region, source_region_name_for_step,
    },
    SavePostLoadRuntimeApplyExecution, SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeSeedPlan,
    SavePostLoadRuntimeWorldSemanticsExecution, SavePostLoadWorldObservation,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SavePostLoadRuntimeExecutionStepStatus {
    Executed,
    Failed,
    AwaitingWorldShell,
    Blocked,
    Deferred,
}

impl SavePostLoadRuntimeExecutionStepStatus {
    pub const fn ordered() -> [Self; 5] {
        [
            Self::Executed,
            Self::Failed,
            Self::AwaitingWorldShell,
            Self::Blocked,
            Self::Deferred,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeExecutionStatusBucket {
    pub status: SavePostLoadRuntimeExecutionStepStatus,
    pub steps: Vec<SavePostLoadRuntimeApplyStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeExecutionSourceRegion {
    pub source_region_name: &'static str,
    pub executed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub failed_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub awaiting_world_shell_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub blocked_steps: Vec<SavePostLoadRuntimeApplyStep>,
    pub deferred_steps: Vec<SavePostLoadRuntimeApplyStep>,
}

impl SavePostLoadRuntimeExecutionSourceRegion {
    pub fn step_count(&self, status: SavePostLoadRuntimeExecutionStepStatus) -> usize {
        self.steps_with_status(status).len()
    }

    pub fn total_step_count(&self) -> usize {
        SavePostLoadRuntimeExecutionStepStatus::ordered()
            .into_iter()
            .map(|status| self.step_count(status))
            .sum()
    }

    pub fn steps_with_status(
        &self,
        status: SavePostLoadRuntimeExecutionStepStatus,
    ) -> &[SavePostLoadRuntimeApplyStep] {
        match status {
            SavePostLoadRuntimeExecutionStepStatus::Executed => &self.executed_steps,
            SavePostLoadRuntimeExecutionStepStatus::Failed => &self.failed_steps,
            SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell => {
                &self.awaiting_world_shell_steps
            }
            SavePostLoadRuntimeExecutionStepStatus::Blocked => &self.blocked_steps,
            SavePostLoadRuntimeExecutionStepStatus::Deferred => &self.deferred_steps,
        }
    }

    fn steps_with_status_mut(
        &mut self,
        status: SavePostLoadRuntimeExecutionStepStatus,
    ) -> &mut Vec<SavePostLoadRuntimeApplyStep> {
        match status {
            SavePostLoadRuntimeExecutionStepStatus::Executed => &mut self.executed_steps,
            SavePostLoadRuntimeExecutionStepStatus::Failed => &mut self.failed_steps,
            SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell => {
                &mut self.awaiting_world_shell_steps
            }
            SavePostLoadRuntimeExecutionStepStatus::Blocked => &mut self.blocked_steps,
            SavePostLoadRuntimeExecutionStepStatus::Deferred => &mut self.deferred_steps,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeApplyExecutionView {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub step_status_lookup:
        BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
}

impl SavePostLoadRuntimeApplyExecutionView {
    pub fn step_status(
        &self,
        step: &SavePostLoadRuntimeApplyStep,
    ) -> Option<SavePostLoadRuntimeExecutionStepStatus> {
        self.step_status_lookup.get(step).copied()
    }

    pub fn step_count(&self, status: SavePostLoadRuntimeExecutionStepStatus) -> usize {
        self.step_status_lookup
            .values()
            .filter(|candidate| **candidate == status)
            .count()
    }

    pub fn total_step_count(&self) -> usize {
        self.step_status_lookup.len()
    }

    pub fn has_step(&self, step: &SavePostLoadRuntimeApplyStep) -> bool {
        self.step_status_lookup.contains_key(step)
    }

    pub fn steps_with_status(
        &self,
        status: SavePostLoadRuntimeExecutionStepStatus,
    ) -> Vec<&SavePostLoadRuntimeApplyStep> {
        steps_with_status(&self.step_status_lookup, status)
    }

    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadRuntimeExecutionSourceRegion> {
        find_source_region(self.source_regions(), source_region_name, |region| {
            region.source_region_name
        })
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeExecutionSourceRegion> {
        source_regions(&self.step_status_lookup)
    }

    pub fn status_counts(&self) -> BTreeMap<SavePostLoadRuntimeExecutionStepStatus, usize> {
        status_counts(&self.step_status_lookup)
    }

    pub fn status_buckets(&self) -> Vec<SavePostLoadRuntimeExecutionStatusBucket> {
        status_buckets(&self.step_status_lookup)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeWorldSemanticsExecutionView {
    pub can_apply_world_semantics: bool,
    pub world_shell_ready: bool,
    pub step_status_lookup:
        BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
}

impl SavePostLoadRuntimeWorldSemanticsExecutionView {
    pub fn step_status(
        &self,
        step: &SavePostLoadRuntimeApplyStep,
    ) -> Option<SavePostLoadRuntimeExecutionStepStatus> {
        self.step_status_lookup.get(step).copied()
    }

    pub fn step_count(&self, status: SavePostLoadRuntimeExecutionStepStatus) -> usize {
        self.step_status_lookup
            .values()
            .filter(|candidate| **candidate == status)
            .count()
    }

    pub fn total_step_count(&self) -> usize {
        self.step_status_lookup.len()
    }

    pub fn has_step(&self, step: &SavePostLoadRuntimeApplyStep) -> bool {
        self.step_status_lookup.contains_key(step)
    }

    pub fn steps_with_status(
        &self,
        status: SavePostLoadRuntimeExecutionStepStatus,
    ) -> Vec<&SavePostLoadRuntimeApplyStep> {
        steps_with_status(&self.step_status_lookup, status)
    }

    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadRuntimeExecutionSourceRegion> {
        find_source_region(self.source_regions(), source_region_name, |region| {
            region.source_region_name
        })
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeExecutionSourceRegion> {
        source_regions(&self.step_status_lookup)
    }

    pub fn status_counts(&self) -> BTreeMap<SavePostLoadRuntimeExecutionStepStatus, usize> {
        status_counts(&self.step_status_lookup)
    }

    pub fn status_buckets(&self) -> Vec<SavePostLoadRuntimeExecutionStatusBucket> {
        status_buckets(&self.step_status_lookup)
    }
}

impl SavePostLoadWorldObservation {
    pub fn runtime_apply_execution_view(&self) -> SavePostLoadRuntimeApplyExecutionView {
        self.runtime_seed_plan().runtime_apply_execution_view()
    }

    pub fn runtime_world_semantics_execution_view(
        &self,
    ) -> SavePostLoadRuntimeWorldSemanticsExecutionView {
        self.runtime_seed_plan()
            .runtime_world_semantics_execution_view()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn runtime_apply_execution_view(&self) -> SavePostLoadRuntimeApplyExecutionView {
        self.execute_runtime_apply().view()
    }

    pub fn runtime_world_semantics_execution_view(
        &self,
    ) -> SavePostLoadRuntimeWorldSemanticsExecutionView {
        self.execute_runtime_world_semantics().view()
    }
}

impl SavePostLoadRuntimeApplyExecution {
    pub fn view(&self) -> SavePostLoadRuntimeApplyExecutionView {
        SavePostLoadRuntimeApplyExecutionView {
            can_seed_runtime_apply: self.can_seed_runtime_apply,
            world_shell_ready: self.world_shell_ready,
            step_status_lookup: build_step_status_lookup(
                &self.executed_steps,
                &self.failed_steps,
                &self.awaiting_world_shell_steps,
                &self.blocked_steps,
                &self.deferred_steps,
            ),
        }
    }
}

impl SavePostLoadRuntimeWorldSemanticsExecution {
    pub fn view(&self) -> SavePostLoadRuntimeWorldSemanticsExecutionView {
        SavePostLoadRuntimeWorldSemanticsExecutionView {
            can_apply_world_semantics: self.can_apply_world_semantics(),
            world_shell_ready: self.world_shell_ready,
            step_status_lookup: build_step_status_lookup(
                &self.executed_steps,
                &self.failed_steps,
                &self.awaiting_world_shell_steps,
                &self.blocked_steps,
                &[],
            ),
        }
    }
}

fn build_step_status_lookup(
    executed_steps: &[SavePostLoadRuntimeApplyStep],
    failed_steps: &[SavePostLoadRuntimeApplyStep],
    awaiting_world_shell_steps: &[SavePostLoadRuntimeApplyStep],
    blocked_steps: &[SavePostLoadRuntimeApplyStep],
    deferred_steps: &[SavePostLoadRuntimeApplyStep],
) -> BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus> {
    let mut lookup = BTreeMap::new();
    // Preserve the first status assigned to a step so later buckets never overwrite it.
    insert_statuses(
        &mut lookup,
        executed_steps,
        SavePostLoadRuntimeExecutionStepStatus::Executed,
    );
    insert_statuses(
        &mut lookup,
        failed_steps,
        SavePostLoadRuntimeExecutionStepStatus::Failed,
    );
    insert_statuses(
        &mut lookup,
        awaiting_world_shell_steps,
        SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell,
    );
    insert_statuses(
        &mut lookup,
        blocked_steps,
        SavePostLoadRuntimeExecutionStepStatus::Blocked,
    );
    insert_statuses(
        &mut lookup,
        deferred_steps,
        SavePostLoadRuntimeExecutionStepStatus::Deferred,
    );
    lookup
}

fn insert_statuses(
    lookup: &mut BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
    steps: &[SavePostLoadRuntimeApplyStep],
    status: SavePostLoadRuntimeExecutionStepStatus,
) {
    for step in steps {
        lookup.entry(step.clone()).or_insert(status);
    }
}

fn steps_with_status(
    lookup: &BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
    status: SavePostLoadRuntimeExecutionStepStatus,
) -> Vec<&SavePostLoadRuntimeApplyStep> {
    lookup
        .iter()
        .filter_map(|(step, candidate_status)| (*candidate_status == status).then_some(step))
        .collect()
}

fn source_regions(
    lookup: &BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
) -> Vec<SavePostLoadRuntimeExecutionSourceRegion> {
    let mut source_regions = Vec::new();

    for (step, status) in lookup {
        let source_region_name = source_region_name_for_step(step);
        let source_region = find_or_push_source_region(
            &mut source_regions,
            source_region_name,
            |candidate: &SavePostLoadRuntimeExecutionSourceRegion| candidate.source_region_name,
            || SavePostLoadRuntimeExecutionSourceRegion {
                source_region_name,
                executed_steps: Vec::new(),
                failed_steps: Vec::new(),
                awaiting_world_shell_steps: Vec::new(),
                blocked_steps: Vec::new(),
                deferred_steps: Vec::new(),
            },
        );
        source_region
            .steps_with_status_mut(*status)
            .push(step.clone());
    }

    source_regions
}

fn status_counts(
    lookup: &BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
) -> BTreeMap<SavePostLoadRuntimeExecutionStepStatus, usize> {
    let mut counts = BTreeMap::new();
    for status in SavePostLoadRuntimeExecutionStepStatus::ordered() {
        counts.insert(status, 0);
    }
    for status in lookup.values() {
        *counts.entry(*status).or_default() += 1;
    }
    counts
}

fn status_buckets(
    lookup: &BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
) -> Vec<SavePostLoadRuntimeExecutionStatusBucket> {
    SavePostLoadRuntimeExecutionStepStatus::ordered()
        .into_iter()
        .filter_map(|status| {
            let steps = steps_with_status(lookup, status)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            (!steps.is_empty())
                .then_some(SavePostLoadRuntimeExecutionStatusBucket { status, steps })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_post_load_runtime_execution::test_support::{
        seedable_test_observation, test_observation,
    };

    fn apply_duplicate_marker_id_fixture(observation: &mut SavePostLoadWorldObservation) {
        observation.markers[1].id = observation.markers[0].id;
    }

    fn duplicate_marker_id_seedable_observation() -> SavePostLoadWorldObservation {
        let mut observation = seedable_test_observation();
        apply_duplicate_marker_id_fixture(&mut observation);
        observation
    }

    fn blocked_pending_world_shell_observation() -> SavePostLoadWorldObservation {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;
        observation
    }

    #[test]
    fn runtime_apply_execution_view_indexes_clean_execution_statuses() {
        let observation = seedable_test_observation();
        let view = observation.runtime_apply_execution_view();
        let executed_steps =
            view.steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Executed);
        let status_counts = view.status_counts();
        let status_buckets = view.status_buckets();

        assert!(view.can_seed_runtime_apply);
        assert!(view.world_shell_ready);
        assert!(view.has_step(&SavePostLoadRuntimeApplyStep::WorldShell));
        assert!(!view.has_step(&SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 99 }));
        assert_eq!(
            view.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            14
        );
        assert_eq!(
            view.step_count(SavePostLoadRuntimeExecutionStepStatus::Failed),
            0
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::WorldShell),
            Some(SavePostLoadRuntimeExecutionStepStatus::Executed)
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::Building { center_index: 0 }),
            Some(SavePostLoadRuntimeExecutionStepStatus::Executed)
        );
        assert_eq!(executed_steps.len(), 14);
        assert!(executed_steps.contains(&&SavePostLoadRuntimeApplyStep::WorldShell));
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Executed),
            Some(&14)
        );
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Failed),
            Some(&0)
        );
        assert_eq!(status_buckets.len(), 1);
        assert_eq!(
            status_buckets[0].status,
            SavePostLoadRuntimeExecutionStepStatus::Executed
        );
        assert_eq!(status_buckets[0].steps.len(), 14);
    }

    #[test]
    fn runtime_apply_execution_view_groups_steps_by_source_region() {
        let observation = seedable_test_observation();
        let view = observation.runtime_apply_execution_view();
        let source_regions = view.source_regions();
        let entities = view.source_region("entities").unwrap();

        assert_eq!(
            source_regions
                .iter()
                .map(|region| region.source_region_name)
                .collect::<Vec<_>>(),
            vec!["map", "entities", "markers", "custom"]
        );
        assert_eq!(source_regions[0].total_step_count(), 2);
        assert_eq!(source_regions[1].total_step_count(), 7);
        assert_eq!(source_regions[2].total_step_count(), 2);
        assert_eq!(source_regions[3].total_step_count(), 3);
        assert_eq!(
            entities.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            7
        );
        assert_eq!(
            entities.step_count(SavePostLoadRuntimeExecutionStepStatus::Blocked),
            0
        );
        assert!(entities
            .steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Executed)
            .contains(&SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 }));
        assert!(entities
            .steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Executed)
            .contains(&SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 }));
    }

    #[test]
    fn runtime_apply_execution_view_preserves_pending_and_deferred_lookup() {
        let observation = blocked_pending_world_shell_observation();
        let view = observation.runtime_apply_execution_view();
        let awaiting_steps =
            view.steps_with_status(SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell);
        let deferred_steps =
            view.steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Deferred);
        let status_counts = view.status_counts();
        let status_buckets = view.status_buckets();

        assert!(!view.can_seed_runtime_apply);
        assert!(!view.world_shell_ready);
        assert!(view.has_step(&SavePostLoadRuntimeApplyStep::WorldShell));
        assert_eq!(
            view.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            4
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::WorldShell),
            Some(SavePostLoadRuntimeExecutionStepStatus::Blocked)
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::StaticFog),
            Some(SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell)
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }),
            Some(SavePostLoadRuntimeExecutionStepStatus::Deferred)
        );
        assert!(awaiting_steps.contains(&&SavePostLoadRuntimeApplyStep::StaticFog));
        assert!(deferred_steps
            .contains(&&SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 }));
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Blocked),
            Some(&4)
        );
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::AwaitingWorldShell),
            Some(&5)
        );
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Deferred),
            Some(&1)
        );
        assert!(status_buckets.iter().any(|bucket| {
            bucket.status == SavePostLoadRuntimeExecutionStepStatus::Blocked
                && bucket
                    .steps
                    .contains(&SavePostLoadRuntimeApplyStep::WorldShell)
        }));
        assert!(status_buckets.iter().any(|bucket| {
            bucket.status == SavePostLoadRuntimeExecutionStepStatus::Deferred
                && bucket
                    .steps
                    .contains(&SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 1 })
        }));
    }

    #[test]
    fn runtime_world_semantics_execution_view_tracks_failed_world_steps() {
        let observation = duplicate_marker_id_seedable_observation();
        let view = observation.runtime_world_semantics_execution_view();
        let failed_steps = view.steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Failed);
        let status_counts = view.status_counts();
        let status_buckets = view.status_buckets();

        assert!(view.world_shell_ready);
        assert!(!view.can_apply_world_semantics);
        assert!(view.has_step(&SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }));
        assert!(!view.has_step(&SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 }));
        assert_eq!(
            view.step_count(SavePostLoadRuntimeExecutionStepStatus::Failed),
            1
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }),
            Some(SavePostLoadRuntimeExecutionStepStatus::Failed)
        );
        assert_eq!(
            view.step_status(&SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 }),
            None
        );
        assert_eq!(failed_steps.len(), 1);
        assert!(failed_steps.contains(&&SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }));
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Failed),
            Some(&1)
        );
        assert!(status_buckets.iter().any(|bucket| {
            bucket.status == SavePostLoadRuntimeExecutionStepStatus::Failed
                && bucket
                    .steps
                    .contains(&SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 })
        }));
    }

    #[test]
    fn runtime_world_semantics_execution_view_groups_source_regions() {
        let observation = duplicate_marker_id_seedable_observation();
        let view = observation.runtime_world_semantics_execution_view();
        let source_regions = view.source_regions();
        let markers = view.source_region("markers").unwrap();

        assert_eq!(
            source_regions
                .iter()
                .map(|region| region.source_region_name)
                .collect::<Vec<_>>(),
            vec!["map", "entities", "markers", "custom"]
        );
        assert_eq!(source_regions[0].total_step_count(), 2);
        assert_eq!(source_regions[1].total_step_count(), 5);
        assert_eq!(source_regions[2].total_step_count(), 2);
        assert_eq!(source_regions[3].total_step_count(), 1);
        assert_eq!(
            markers.step_count(SavePostLoadRuntimeExecutionStepStatus::Failed),
            1
        );
        assert_eq!(
            markers.step_count(SavePostLoadRuntimeExecutionStepStatus::Executed),
            1
        );
        assert!(markers
            .steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Failed)
            .contains(&SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }));
    }

    #[test]
    fn build_step_status_lookup_keeps_first_status_for_duplicate_steps() {
        let step = SavePostLoadRuntimeApplyStep::WorldShell;

        let lookup = build_step_status_lookup(
            std::slice::from_ref(&step),
            std::slice::from_ref(&step),
            std::slice::from_ref(&step),
            std::slice::from_ref(&step),
            std::slice::from_ref(&step),
        );
        let status_counts = status_counts(&lookup);
        let status_buckets = status_buckets(&lookup);

        assert_eq!(
            lookup.get(&step),
            Some(&SavePostLoadRuntimeExecutionStepStatus::Executed)
        );
        assert_eq!(lookup.len(), 1);
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Executed),
            Some(&1)
        );
        assert_eq!(
            status_counts.get(&SavePostLoadRuntimeExecutionStepStatus::Failed),
            Some(&0)
        );
        assert_eq!(status_buckets.len(), 1);
        assert_eq!(
            status_buckets[0].status,
            SavePostLoadRuntimeExecutionStepStatus::Executed
        );
        assert_eq!(status_buckets[0].steps, vec![step]);
    }
}
