use crate::{
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
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
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
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
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

fn steps_with_status<'a>(
    lookup: &'a BTreeMap<SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeExecutionStepStatus>,
    status: SavePostLoadRuntimeExecutionStepStatus,
) -> Vec<&'a SavePostLoadRuntimeApplyStep> {
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
        let source_region = match source_regions.iter_mut().find(
            |candidate: &&mut SavePostLoadRuntimeExecutionSourceRegion| {
                candidate.source_region_name == source_region_name
            },
        ) {
            Some(source_region) => source_region,
            None => {
                source_regions.push(SavePostLoadRuntimeExecutionSourceRegion {
                    source_region_name,
                    executed_steps: Vec::new(),
                    failed_steps: Vec::new(),
                    awaiting_world_shell_steps: Vec::new(),
                    blocked_steps: Vec::new(),
                    deferred_steps: Vec::new(),
                });
                source_regions
                    .last_mut()
                    .expect("source region was just pushed")
            }
        };
        source_region.steps_with_status_mut(*status).push(step.clone());
    }

    source_regions
}

const fn source_region_name_for_step(step: &SavePostLoadRuntimeApplyStep) -> &'static str {
    match step {
        SavePostLoadRuntimeApplyStep::WorldShell | SavePostLoadRuntimeApplyStep::Building { .. } => {
            "map"
        }
        SavePostLoadRuntimeApplyStep::EntityRemap { .. }
        | SavePostLoadRuntimeApplyStep::TeamPlan { .. }
        | SavePostLoadRuntimeApplyStep::LoadableEntity { .. }
        | SavePostLoadRuntimeApplyStep::SkippedEntity { .. } => "entities",
        SavePostLoadRuntimeApplyStep::Marker { .. } => "markers",
        SavePostLoadRuntimeApplyStep::StaticFog
        | SavePostLoadRuntimeApplyStep::CustomChunk { .. } => "custom",
    }
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
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, ContentHeaderEntry,
        CoreTailSnapshot, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
        ParsedCustomChunk, PointMarkerModel, SaveEntityChunkObservation, SaveEntityClassKind,
        SaveEntityClassSummary, SaveEntityPostLoadClassSummary, SaveEntityPostLoadKind,
        SaveEntityPostLoadSummary, SaveEntityRemapEntry, SaveEntityRemapSummary,
        SaveMapRegionObservation, StaticFogChunk, StaticFogTeam, TeamPlan, TeamPlanGroup,
        TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn runtime_apply_execution_view_indexes_clean_execution_statuses() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

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
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

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
        assert!(
            entities
                .steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Executed)
                .contains(&SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 })
        );
        assert!(
            entities
                .steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Executed)
                .contains(&SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 2 })
        );
    }

    #[test]
    fn runtime_apply_execution_view_preserves_pending_and_deferred_lookup() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

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
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.markers[1].id = observation.markers[0].id;

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
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.markers[1].id = observation.markers[0].id;

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
        assert!(
            markers
                .steps_with_status(SavePostLoadRuntimeExecutionStepStatus::Failed)
                .contains(&SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 })
        );
    }

    #[test]
    fn build_step_status_lookup_keeps_first_status_for_duplicate_steps() {
        let step = SavePostLoadRuntimeApplyStep::WorldShell;

        let lookup = build_step_status_lookup(
            &[step.clone()],
            &[step.clone()],
            &[step.clone()],
            &[step.clone()],
            &[step.clone()],
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

    fn make_observation_seedable(observation: &mut crate::SavePostLoadWorldObservation) {
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

    fn test_observation() -> crate::SavePostLoadWorldObservation {
        crate::SavePostLoadWorldObservation {
            save_version: 11,
            content_header: vec![ContentHeaderEntry {
                content_type: 1,
                names: vec!["core-nucleus".to_string()],
            }],
            patches: vec![vec![1, 2, 3]],
            map: SaveMapRegionObservation {
                floor_runs: 1,
                floor_region_bytes: vec![0],
                block_runs: 1,
                block_region_bytes: vec![0],
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
            entity_remap_bytes: vec![1, 2],
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
                        x: 0,
                        y: 0,
                        rotation: 0,
                        block_id: 0x0153,
                        config: TypeIoValue::Integer(9),
                        config_bytes: vec![9],
                        config_sha256: "cfg-0".to_string(),
                    }],
                },
                TeamPlanGroup {
                    team_id: 2,
                    plan_count: 1,
                    plans: vec![TeamPlan {
                        x: 1,
                        y: 1,
                        rotation: 1,
                        block_id: 0x0001,
                        config: TypeIoValue::Null,
                        config_bytes: Vec::new(),
                        config_sha256: "cfg-1".to_string(),
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
                    parsed_tail: ParsedBuildingTail::Core(CoreTailSnapshot {
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
