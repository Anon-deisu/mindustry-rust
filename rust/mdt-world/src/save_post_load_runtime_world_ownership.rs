use crate::{
    bool_word_label,
    save_post_load_runtime_source_region::{
        find_or_push_source_region, find_source_region, source_region_name_for_stage_kind,
    },
    SavePostLoadConsumerBlocker, SavePostLoadConsumerStageKind, SavePostLoadRuntimeApplyStep,
    SavePostLoadRuntimeSeedPlan, SavePostLoadRuntimeWorldSemanticsExecution,
    SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SavePostLoadRuntimeWorldSurfaceKind {
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

impl SavePostLoadRuntimeWorldSurfaceKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::WorldShell => "world-shell",
            Self::EntityRemaps => "entity-remaps",
            Self::TeamPlans => "team-plans",
            Self::Markers => "markers",
            Self::StaticFog => "static-fog",
            Self::CustomChunks => "custom-chunks",
            Self::Buildings => "buildings",
            Self::LoadableEntities => "loadable-entities",
            Self::SkippedEntities => "skipped-entities",
        }
    }

    pub fn source_region_name(&self) -> &'static str {
        source_region_name_for_stage_kind(self.stage_kind())
    }

    pub const fn ordered() -> [Self; 9] {
        [
            Self::WorldShell,
            Self::EntityRemaps,
            Self::TeamPlans,
            Self::Markers,
            Self::StaticFog,
            Self::CustomChunks,
            Self::Buildings,
            Self::LoadableEntities,
            Self::SkippedEntities,
        ]
    }

    pub(crate) fn from_stage_kind(kind: SavePostLoadConsumerStageKind) -> Option<Self> {
        match kind {
            SavePostLoadConsumerStageKind::WorldShell => Some(Self::WorldShell),
            SavePostLoadConsumerStageKind::EntityRemaps => Some(Self::EntityRemaps),
            SavePostLoadConsumerStageKind::TeamPlans => Some(Self::TeamPlans),
            SavePostLoadConsumerStageKind::Markers => Some(Self::Markers),
            SavePostLoadConsumerStageKind::StaticFog => Some(Self::StaticFog),
            SavePostLoadConsumerStageKind::CustomChunks => Some(Self::CustomChunks),
            SavePostLoadConsumerStageKind::Buildings => Some(Self::Buildings),
            SavePostLoadConsumerStageKind::LoadableEntities => Some(Self::LoadableEntities),
            SavePostLoadConsumerStageKind::SkippedEntities => Some(Self::SkippedEntities),
        }
    }

    pub(crate) fn from_step(step: &SavePostLoadRuntimeApplyStep) -> Option<Self> {
        match step {
            SavePostLoadRuntimeApplyStep::WorldShell => Some(Self::WorldShell),
            SavePostLoadRuntimeApplyStep::EntityRemap { .. } => Some(Self::EntityRemaps),
            SavePostLoadRuntimeApplyStep::TeamPlan { .. } => Some(Self::TeamPlans),
            SavePostLoadRuntimeApplyStep::Marker { .. } => Some(Self::Markers),
            SavePostLoadRuntimeApplyStep::StaticFog => Some(Self::StaticFog),
            SavePostLoadRuntimeApplyStep::CustomChunk { .. } => Some(Self::CustomChunks),
            SavePostLoadRuntimeApplyStep::Building { .. } => Some(Self::Buildings),
            SavePostLoadRuntimeApplyStep::LoadableEntity { .. } => Some(Self::LoadableEntities),
            SavePostLoadRuntimeApplyStep::SkippedEntity { .. } => Some(Self::SkippedEntities),
        }
    }

    const fn stage_kind(&self) -> SavePostLoadConsumerStageKind {
        match self {
            SavePostLoadRuntimeWorldSurfaceKind::WorldShell => {
                SavePostLoadConsumerStageKind::WorldShell
            }
            SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps => {
                SavePostLoadConsumerStageKind::EntityRemaps
            }
            SavePostLoadRuntimeWorldSurfaceKind::TeamPlans => {
                SavePostLoadConsumerStageKind::TeamPlans
            }
            SavePostLoadRuntimeWorldSurfaceKind::Markers => SavePostLoadConsumerStageKind::Markers,
            SavePostLoadRuntimeWorldSurfaceKind::StaticFog => {
                SavePostLoadConsumerStageKind::StaticFog
            }
            SavePostLoadRuntimeWorldSurfaceKind::CustomChunks => {
                SavePostLoadConsumerStageKind::CustomChunks
            }
            SavePostLoadRuntimeWorldSurfaceKind::Buildings => {
                SavePostLoadConsumerStageKind::Buildings
            }
            SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities => {
                SavePostLoadConsumerStageKind::LoadableEntities
            }
            SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities => {
                SavePostLoadConsumerStageKind::SkippedEntities
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePostLoadRuntimeWorldOwnershipStatus {
    Absent,
    Owned,
    Failed,
    AwaitingWorldShell,
    Blocked,
    Deferred,
}

impl SavePostLoadRuntimeWorldOwnershipStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Owned => "owned",
            Self::Failed => "failed",
            Self::AwaitingWorldShell => "awaiting-world-shell",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeWorldOwnershipSurface {
    pub kind: SavePostLoadRuntimeWorldSurfaceKind,
    pub source_region_name: &'static str,
    pub required_step_count: usize,
    pub claimed_step_count: usize,
    pub status: SavePostLoadRuntimeWorldOwnershipStatus,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
    pub failed_steps: Vec<SavePostLoadRuntimeApplyStep>,
}

impl SavePostLoadRuntimeWorldOwnershipSurface {
    pub fn is_owned(&self) -> bool {
        self.status == SavePostLoadRuntimeWorldOwnershipStatus::Owned
    }

    pub fn summary_label(&self) -> String {
        format!(
            "{}:{}:{}/{} blockers={} failed={}",
            self.kind.label(),
            self.status.label(),
            self.claimed_step_count,
            self.required_step_count,
            self.blockers.len(),
            self.failed_steps.len(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "kind={} region={} status={} claim={}/{} blockers={} failed={}",
            self.kind.label(),
            self.source_region_name,
            self.status.label(),
            self.claimed_step_count,
            self.required_step_count,
            self.blockers.len(),
            self.failed_steps.len(),
        )
    }

    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn has_failures(&self) -> bool {
        !self.failed_steps.is_empty()
    }
}

fn sum_required_steps(surfaces: &[SavePostLoadRuntimeWorldOwnershipSurface]) -> usize {
    surfaces
        .iter()
        .map(|surface| surface.required_step_count)
        .sum()
}

fn sum_claimed_steps(surfaces: &[SavePostLoadRuntimeWorldOwnershipSurface]) -> usize {
    surfaces
        .iter()
        .map(|surface| surface.claimed_step_count)
        .sum()
}

fn count_owned_surfaces(surfaces: &[SavePostLoadRuntimeWorldOwnershipSurface]) -> usize {
    surfaces.iter().filter(|surface| surface.is_owned()).count()
}

fn count_surfaces_with_status(
    surfaces: &[SavePostLoadRuntimeWorldOwnershipSurface],
    status: SavePostLoadRuntimeWorldOwnershipStatus,
) -> usize {
    surfaces
        .iter()
        .filter(|surface| surface.status == status)
        .count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeWorldOwnershipSourceRegion {
    pub source_region_name: &'static str,
    pub surfaces: Vec<SavePostLoadRuntimeWorldOwnershipSurface>,
}

impl SavePostLoadRuntimeWorldOwnershipSourceRegion {
    pub fn surface(
        &self,
        kind: SavePostLoadRuntimeWorldSurfaceKind,
    ) -> Option<&SavePostLoadRuntimeWorldOwnershipSurface> {
        self.surfaces.iter().find(|surface| surface.kind == kind)
    }

    pub fn required_step_count(&self) -> usize {
        sum_required_steps(&self.surfaces)
    }

    pub fn claimed_step_count(&self) -> usize {
        sum_claimed_steps(&self.surfaces)
    }

    pub fn owned_surface_count(&self) -> usize {
        count_owned_surfaces(&self.surfaces)
    }

    pub fn awaiting_world_shell_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::AwaitingWorldShell,
        )
    }

    pub fn blocked_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked,
        )
    }

    pub fn failed_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Failed,
        )
    }

    pub fn deferred_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Deferred,
        )
    }

    pub fn absent_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Absent,
        )
    }

    pub fn summary_label(&self) -> String {
        format!(
            "region={} own={}/{} claim={}/{} wait={} block={} fail={} defer={} absent={}",
            self.source_region_name,
            self.owned_surface_count(),
            self.surfaces.len(),
            self.claimed_step_count(),
            self.required_step_count(),
            self.awaiting_world_shell_surface_count(),
            self.blocked_surface_count(),
            self.failed_surface_count(),
            self.deferred_surface_count(),
            self.absent_surface_count(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "region={} own={}/{} claim={}/{} wait={} block={} fail={} defer={} absent={} surfaces=[{}]",
            self.source_region_name,
            self.owned_surface_count(),
            self.surfaces.len(),
            self.claimed_step_count(),
            self.required_step_count(),
            self.awaiting_world_shell_surface_count(),
            self.blocked_surface_count(),
            self.failed_surface_count(),
            self.deferred_surface_count(),
            self.absent_surface_count(),
            self.surfaces
                .iter()
                .map(SavePostLoadRuntimeWorldOwnershipSurface::summary_label)
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadRuntimeWorldOwnership {
    pub world_shell_ready: bool,
    pub surfaces: Vec<SavePostLoadRuntimeWorldOwnershipSurface>,
}

impl SavePostLoadRuntimeWorldOwnership {
    pub fn surface(
        &self,
        kind: SavePostLoadRuntimeWorldSurfaceKind,
    ) -> Option<&SavePostLoadRuntimeWorldOwnershipSurface> {
        self.surfaces.iter().find(|surface| surface.kind == kind)
    }

    pub fn required_step_count(&self) -> usize {
        sum_required_steps(&self.surfaces)
    }

    pub fn claimed_step_count(&self) -> usize {
        sum_claimed_steps(&self.surfaces)
    }

    pub fn owned_surface_count(&self) -> usize {
        count_owned_surfaces(&self.surfaces)
    }

    pub fn awaiting_world_shell_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::AwaitingWorldShell,
        )
    }

    pub fn blocked_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked,
        )
    }

    pub fn failed_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Failed,
        )
    }

    pub fn deferred_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Deferred,
        )
    }

    pub fn absent_surface_count(&self) -> usize {
        count_surfaces_with_status(
            &self.surfaces,
            SavePostLoadRuntimeWorldOwnershipStatus::Absent,
        )
    }

    pub fn summary_label(&self) -> String {
        format!(
            "shell={} semantics={} own={}/{} claim={}/{} wait={} block={} fail={} defer={} absent={} regions={}",
            bool_word_label(self.world_shell_ready),
            bool_word_label(self.can_apply_world_semantics()),
            self.owned_surface_count(),
            self.surfaces.len(),
            self.claimed_step_count(),
            self.required_step_count(),
            self.awaiting_world_shell_surface_count(),
            self.blocked_surface_count(),
            self.failed_surface_count(),
            self.deferred_surface_count(),
            self.absent_surface_count(),
            self.source_regions().len(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "shell={} semantics={} own={}/{} claim={}/{} wait={} block={} fail={} defer={} absent={} regions=[{}]",
            bool_word_label(self.world_shell_ready),
            bool_word_label(self.can_apply_world_semantics()),
            self.owned_surface_count(),
            self.surfaces.len(),
            self.claimed_step_count(),
            self.required_step_count(),
            self.awaiting_world_shell_surface_count(),
            self.blocked_surface_count(),
            self.failed_surface_count(),
            self.deferred_surface_count(),
            self.absent_surface_count(),
            self.source_regions()
                .iter()
                .map(SavePostLoadRuntimeWorldOwnershipSourceRegion::summary_label)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadRuntimeWorldOwnershipSourceRegion> {
        find_source_region(self.source_regions(), source_region_name, |region| {
            region.source_region_name
        })
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadRuntimeWorldOwnershipSourceRegion> {
        let mut source_regions = Vec::new();

        for surface in &self.surfaces {
            let source_region = find_or_push_source_region(
                &mut source_regions,
                surface.source_region_name,
                |candidate: &SavePostLoadRuntimeWorldOwnershipSourceRegion| {
                    candidate.source_region_name
                },
                || SavePostLoadRuntimeWorldOwnershipSourceRegion {
                    source_region_name: surface.source_region_name,
                    surfaces: Vec::new(),
                },
            );

            source_region.surfaces.push(surface.clone());
        }

        source_regions
    }

    pub fn can_apply_world_semantics(&self) -> bool {
        self.world_shell_ready
            && self
                .surface(SavePostLoadRuntimeWorldSurfaceKind::WorldShell)
                .is_some_and(SavePostLoadRuntimeWorldOwnershipSurface::is_owned)
            && self.surfaces.iter().all(|surface| {
                matches!(
                    surface.status,
                    SavePostLoadRuntimeWorldOwnershipStatus::Absent
                        | SavePostLoadRuntimeWorldOwnershipStatus::Owned
                        | SavePostLoadRuntimeWorldOwnershipStatus::Deferred
                )
            })
    }

    pub fn can_activate_live_runtime(&self) -> bool {
        self.can_apply_world_semantics()
    }
}

impl SavePostLoadWorldObservation {
    pub fn runtime_world_ownership(&self) -> SavePostLoadRuntimeWorldOwnership {
        self.runtime_seed_plan().runtime_world_ownership()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn runtime_world_ownership(&self) -> SavePostLoadRuntimeWorldOwnership {
        self.execute_runtime_world_semantics().ownership
    }
}

pub(crate) fn build_runtime_world_ownership(
    plan: &SavePostLoadRuntimeSeedPlan,
    execution: &SavePostLoadRuntimeWorldSemanticsExecution,
) -> SavePostLoadRuntimeWorldOwnership {
    let helper = plan.consumer_runtime_helper();
    let apply_now_steps = plan.runtime_apply_script().apply_now_steps;
    let shell = execution.world_shell.as_ref();
    let surfaces = helper
        .stages
        .iter()
        .filter_map(|stage| {
            let kind = SavePostLoadRuntimeWorldSurfaceKind::from_stage_kind(stage.kind)?;
            let failed_steps = execution
                .failed_steps
                .iter()
                .filter(|step| SavePostLoadRuntimeWorldSurfaceKind::from_step(step) == Some(kind))
                .cloned()
                .collect::<Vec<_>>();
            let claimed_step_count = match kind {
                SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps
                | SavePostLoadRuntimeWorldSurfaceKind::CustomChunks => apply_now_steps
                    .iter()
                    .filter(|step| {
                        SavePostLoadRuntimeWorldSurfaceKind::from_step(step) == Some(kind)
                    })
                    .count(),
                _ => shell
                    .map(|shell| shell.owned_step_count(kind))
                    .unwrap_or_default(),
            };

            let status = if !failed_steps.is_empty() {
                SavePostLoadRuntimeWorldOwnershipStatus::Failed
            } else if stage.step_count == 0 {
                if stage.blockers.is_empty() {
                    SavePostLoadRuntimeWorldOwnershipStatus::Absent
                } else {
                    SavePostLoadRuntimeWorldOwnershipStatus::Blocked
                }
            } else if claimed_step_count == stage.step_count {
                SavePostLoadRuntimeWorldOwnershipStatus::Owned
            } else {
                match stage.disposition {
                    crate::SavePostLoadConsumerRuntimeDisposition::ApplyNow => {
                        SavePostLoadRuntimeWorldOwnershipStatus::Failed
                    }
                    crate::SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell => {
                        SavePostLoadRuntimeWorldOwnershipStatus::AwaitingWorldShell
                    }
                    crate::SavePostLoadConsumerRuntimeDisposition::Blocked => {
                        SavePostLoadRuntimeWorldOwnershipStatus::Blocked
                    }
                    crate::SavePostLoadConsumerRuntimeDisposition::Deferred => {
                        SavePostLoadRuntimeWorldOwnershipStatus::Deferred
                    }
                }
            };

            Some(SavePostLoadRuntimeWorldOwnershipSurface {
                kind,
                source_region_name: kind.source_region_name(),
                required_step_count: stage.step_count,
                claimed_step_count,
                status,
                blockers: stage.blockers.clone(),
                failed_steps,
            })
        })
        .collect();

    SavePostLoadRuntimeWorldOwnership {
        world_shell_ready: execution.world_shell_ready,
        surfaces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_post_load_runtime_execution::test_support;
    use crate::{ParsedCustomChunk, SavePostLoadWorldObservation, StaticFogTeam};

    fn expected_owned_surface(
        kind: SavePostLoadRuntimeWorldSurfaceKind,
        source_region_name: &'static str,
        step_count: usize,
    ) -> SavePostLoadRuntimeWorldOwnershipSurface {
        SavePostLoadRuntimeWorldOwnershipSurface {
            kind,
            source_region_name,
            required_step_count: step_count,
            claimed_step_count: step_count,
            status: SavePostLoadRuntimeWorldOwnershipStatus::Owned,
            blockers: Vec::new(),
            failed_steps: Vec::new(),
        }
    }

    fn assert_ownership_gate(
        ownership: &SavePostLoadRuntimeWorldOwnership,
        world_shell_ready: bool,
        can_apply_world_semantics: bool,
        can_activate_live_runtime: bool,
    ) {
        assert_eq!(ownership.world_shell_ready, world_shell_ready);
        assert_eq!(
            ownership.can_apply_world_semantics(),
            can_apply_world_semantics
        );
        assert_eq!(
            ownership.can_activate_live_runtime(),
            can_activate_live_runtime
        );
    }

    fn assert_surface_status(
        ownership: &SavePostLoadRuntimeWorldOwnership,
        kind: SavePostLoadRuntimeWorldSurfaceKind,
        expected_status: SavePostLoadRuntimeWorldOwnershipStatus,
    ) {
        assert_eq!(surface(ownership, kind).status, expected_status);
    }

    fn surface<'a>(
        ownership: &'a SavePostLoadRuntimeWorldOwnership,
        kind: SavePostLoadRuntimeWorldSurfaceKind,
    ) -> &'a SavePostLoadRuntimeWorldOwnershipSurface {
        ownership.surface(kind).unwrap()
    }

    fn source_region(
        ownership: &SavePostLoadRuntimeWorldOwnership,
        source_region_name: &str,
    ) -> SavePostLoadRuntimeWorldOwnershipSourceRegion {
        ownership.source_region(source_region_name).unwrap()
    }

    fn apply_blocked_pending_world_shell_fixture(observation: &mut SavePostLoadWorldObservation) {
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;
    }

    #[test]
    fn runtime_world_surface_kind_includes_entity_remaps_and_custom_chunks() {
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_stage_kind(
                SavePostLoadConsumerStageKind::EntityRemaps,
            ),
            Some(SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps)
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_stage_kind(
                SavePostLoadConsumerStageKind::CustomChunks,
            ),
            Some(SavePostLoadRuntimeWorldSurfaceKind::CustomChunks)
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_step(
                &SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 }
            ),
            Some(SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps)
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_step(
                &SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 }
            ),
            Some(SavePostLoadRuntimeWorldSurfaceKind::CustomChunks)
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::ordered(),
            [
                SavePostLoadRuntimeWorldSurfaceKind::WorldShell,
                SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps,
                SavePostLoadRuntimeWorldSurfaceKind::TeamPlans,
                SavePostLoadRuntimeWorldSurfaceKind::Markers,
                SavePostLoadRuntimeWorldSurfaceKind::StaticFog,
                SavePostLoadRuntimeWorldSurfaceKind::CustomChunks,
                SavePostLoadRuntimeWorldSurfaceKind::Buildings,
                SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
                SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities,
            ]
        );
    }

    #[test]
    fn runtime_world_ownership_marks_clean_world_surfaces_owned() {
        let observation = seedable_test_observation();

        let ownership = observation.runtime_world_ownership();

        assert_ownership_gate(&ownership, true, true, true);
        assert_eq!(ownership.required_step_count(), 14);
        assert_eq!(ownership.claimed_step_count(), 14);
        assert_eq!(ownership.owned_surface_count(), 8);
        assert_eq!(
            ownership.summary_label(),
            "shell=yes semantics=yes own=8/9 claim=14/14 wait=0 block=0 fail=0 defer=0 absent=1 regions=4"
        );
        assert!(ownership
            .detail_label()
            .contains("region=entities own=3/4 claim=7/7 wait=0 block=0 fail=0 defer=0 absent=1"));
        assert_eq!(
            surface(
                &ownership,
                SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities
            ),
            &expected_owned_surface(
                SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
                "entities",
                3,
            )
        );
        assert_eq!(
            surface(
                &ownership,
                SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps
            ),
            &expected_owned_surface(
                SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps,
                "entities",
                2,
            )
        );
        assert_eq!(
            surface(
                &ownership,
                SavePostLoadRuntimeWorldSurfaceKind::CustomChunks
            ),
            &expected_owned_surface(
                SavePostLoadRuntimeWorldSurfaceKind::CustomChunks,
                "custom",
                2,
            )
        );
    }

    #[test]
    fn runtime_world_ownership_groups_surfaces_by_source_region() {
        let observation = seedable_test_observation();

        let ownership = observation.runtime_world_ownership();
        let source_regions = ownership.source_regions();
        let entities = source_region(&ownership, "entities");

        assert_eq!(
            source_regions
                .iter()
                .map(|region| region.source_region_name)
                .collect::<Vec<_>>(),
            vec!["map", "entities", "markers", "custom"]
        );
        assert_eq!(source_regions[0].surfaces.len(), 2);
        assert_eq!(source_regions[1].surfaces.len(), 4);
        assert_eq!(source_regions[2].surfaces.len(), 1);
        assert_eq!(source_regions[3].surfaces.len(), 2);
        assert_eq!(
            source_regions[1]
                .surfaces
                .iter()
                .map(|surface| surface.kind)
                .collect::<Vec<_>>(),
            vec![
                SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps,
                SavePostLoadRuntimeWorldSurfaceKind::TeamPlans,
                SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
                SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities,
            ]
        );
        assert_eq!(entities.source_region_name, "entities");
        assert_eq!(entities.required_step_count(), 7);
        assert_eq!(entities.claimed_step_count(), 7);
        assert_eq!(entities.owned_surface_count(), 3);
        assert_eq!(
            entities.summary_label(),
            "region=entities own=3/4 claim=7/7 wait=0 block=0 fail=0 defer=0 absent=1"
        );
        assert!(entities
            .detail_label()
            .contains("team-plans:owned:2/2 blockers=0 failed=0"));
        assert_eq!(
            entities
                .surface(SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps)
                .unwrap()
                .required_step_count,
            2
        );
    }

    #[test]
    fn runtime_world_ownership_keeps_failed_marker_surface_unowned() {
        let mut observation = seedable_test_observation();
        observation.markers[1].id = observation.markers[0].id;

        let ownership = observation.runtime_world_ownership();
        let markers = surface(&ownership, SavePostLoadRuntimeWorldSurfaceKind::Markers);

        assert_ownership_gate(&ownership, true, false, false);
        assert_eq!(ownership.required_step_count(), 14);
        assert_eq!(ownership.claimed_step_count(), 13);
        assert_eq!(ownership.owned_surface_count(), 7);
        assert_eq!(markers.required_step_count, 2);
        assert_eq!(markers.claimed_step_count, 1);
        assert_eq!(
            markers.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Failed
        );
        assert!(markers.has_failures());
        assert_eq!(
            markers.failed_steps,
            vec![SavePostLoadRuntimeApplyStep::Marker { marker_index: 1 }]
        );
    }

    #[test]
    fn runtime_world_ownership_marks_zero_step_blocked_surface_blocked_not_absent() {
        let mut observation = test_observation();
        observation.team_plan_groups.clear();

        let ownership = observation.runtime_world_ownership();
        let team_plans = surface(&ownership, SavePostLoadRuntimeWorldSurfaceKind::TeamPlans);

        assert_eq!(team_plans.required_step_count, 0);
        assert_eq!(team_plans.claimed_step_count, 0);
        assert_eq!(
            team_plans.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
        assert!(team_plans.has_blockers());
        assert_ne!(
            team_plans.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Absent
        );
    }

    #[test]
    fn runtime_world_ownership_surfaces_blocked_and_awaiting_regions() {
        let mut observation = test_observation();
        apply_blocked_pending_world_shell_fixture(&mut observation);

        let ownership = observation.runtime_world_ownership();
        let entity_remaps = surface(
            &ownership,
            SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps,
        );
        let entities = source_region(&ownership, "entities");
        let custom_chunks = surface(
            &ownership,
            SavePostLoadRuntimeWorldSurfaceKind::CustomChunks,
        );

        assert_ownership_gate(&ownership, false, false, false);
        assert_eq!(ownership.required_step_count(), 14);
        assert_eq!(ownership.claimed_step_count(), 4);
        assert_eq!(ownership.owned_surface_count(), 2);
        assert_eq!(
            ownership.summary_label(),
            "shell=no semantics=no own=2/9 claim=4/14 wait=3 block=3 fail=0 defer=1 absent=0 regions=4"
        );
        assert!(entities
            .detail_label()
            .contains("entity-remaps:owned:2/2 blockers=0 failed=0"));
        assert_surface_status(
            &ownership,
            SavePostLoadRuntimeWorldSurfaceKind::WorldShell,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked,
        );
        assert_surface_status(
            &ownership,
            SavePostLoadRuntimeWorldSurfaceKind::TeamPlans,
            SavePostLoadRuntimeWorldOwnershipStatus::AwaitingWorldShell,
        );
        assert_surface_status(
            &ownership,
            SavePostLoadRuntimeWorldSurfaceKind::Buildings,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked,
        );
        assert_surface_status(
            &ownership,
            SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked,
        );
        assert_eq!(
            entity_remaps.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Owned
        );
        assert_eq!(entity_remaps.required_step_count, 2);
        assert_eq!(entity_remaps.claimed_step_count, 2);
        assert_eq!(
            entity_remaps.detail_label(),
            "kind=entity-remaps region=entities status=owned claim=2/2 blockers=0 failed=0"
        );
        assert_eq!(
            custom_chunks.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Owned
        );
        assert_eq!(custom_chunks.required_step_count, 2);
        assert_eq!(custom_chunks.claimed_step_count, 2);
    }

    #[test]
    fn runtime_world_ownership_preserves_deferred_skipped_entities_surface() {
        let observation = seedable_test_observation();
        let mut plan = observation.runtime_seed_plan();
        let mut skipped = plan.loadable_entity_seeds[1].clone();
        skipped.entity_index = 99;
        plan.skipped_entity_seeds.push(skipped);

        let execution = plan.execute_runtime_world_semantics();
        let skipped_surface = surface(
            &execution.ownership,
            SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities,
        );

        assert!(execution.can_apply_world_semantics());
        assert_eq!(
            skipped_surface.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Deferred
        );
        assert!(!skipped_surface.is_owned());
        assert_eq!(skipped_surface.required_step_count, 1);
        assert_eq!(skipped_surface.claimed_step_count, 0);
    }

    #[test]
    fn runtime_world_ownership_requires_ready_flag_for_world_semantics() {
        let observation = seedable_test_observation();

        let mut ownership = observation.runtime_world_ownership();
        ownership.world_shell_ready = false;

        assert_ownership_gate(&ownership, false, false, false);
    }

    #[test]
    fn runtime_world_ownership_blocks_duplicate_static_fog_team_ids() {
        let mut observation = seedable_test_observation();
        if let ParsedCustomChunk::StaticFog(chunk) = &mut observation.custom_chunks[0].parsed {
            chunk.used_teams = 2;
            chunk.teams.push(StaticFogTeam {
                team_id: chunk.teams[0].team_id,
                run_count: chunk.teams[0].run_count,
                rle_bytes: chunk.teams[0].rle_bytes.clone(),
                discovered: chunk.teams[0].discovered.clone(),
            });
        }

        let ownership = observation.runtime_world_ownership();
        let static_fog = surface(&ownership, SavePostLoadRuntimeWorldSurfaceKind::StaticFog);

        assert_ownership_gate(&ownership, false, false, false);
        assert_eq!(
            static_fog.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
        assert!(static_fog
            .blockers
            .contains(&SavePostLoadConsumerBlocker::ContractIssue(
                crate::SavePostLoadWorldIssue::DuplicateStaticFogTeamIds,
            )));
        assert_eq!(
            SavePostLoadRuntimeWorldOwnershipStatus::AwaitingWorldShell.label(),
            "awaiting-world-shell"
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::StaticFog.label(),
            "static-fog"
        );
    }

    fn seedable_test_observation() -> SavePostLoadWorldObservation {
        test_support::seedable_test_observation()
    }

    fn test_observation() -> SavePostLoadWorldObservation {
        test_support::test_observation()
    }
}
