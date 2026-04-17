use crate::{
    save_post_load_runtime_source_region::source_region_name_for_stage_kind,
    SavePostLoadRuntimeSeedPlan, SavePostLoadWorldIssue, SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePostLoadConsumerStageKind {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerStage {
    pub kind: SavePostLoadConsumerStageKind,
    pub step_count: usize,
    pub deferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePostLoadConsumerBlocker {
    ContractIssue(SavePostLoadWorldIssue),
    DuplicateEntityId(i32),
    InvalidBuildingReference {
        center_index: usize,
        tile_index: usize,
        block_id: u16,
    },
    SkippedEntity {
        entity_index: usize,
        entity_id: i32,
        source_name: String,
        effective_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerApplyPlan {
    pub can_seed_runtime_apply: bool,
    pub stages: Vec<SavePostLoadConsumerStage>,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePostLoadConsumerRuntimeDisposition {
    ApplyNow,
    AwaitingWorldShell,
    Blocked,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerRuntimeStageHelper {
    pub kind: SavePostLoadConsumerStageKind,
    pub step_count: usize,
    pub disposition: SavePostLoadConsumerRuntimeDisposition,
    pub blockers: Vec<SavePostLoadConsumerBlocker>,
}

impl SavePostLoadConsumerRuntimeStageHelper {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn can_apply_now(&self) -> bool {
        self.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerRuntimeHelper {
    pub can_seed_runtime_apply: bool,
    pub world_shell_ready: bool,
    pub stages: Vec<SavePostLoadConsumerRuntimeStageHelper>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadConsumerSourceRegion {
    pub source_region_name: &'static str,
    pub stage_count: usize,
    pub step_count: usize,
    pub blocker_count: usize,
}

impl SavePostLoadConsumerSourceRegion {
    pub fn total_step_count(&self) -> usize {
        self.step_count
    }

    pub fn has_blockers(&self) -> bool {
        self.blocker_count > 0
    }
}

impl SavePostLoadConsumerApplyPlan {
    pub fn summary_label(&self) -> String {
        format!(
            "seed={} stages={} steps={} blockers={} regions={}",
            bool_label(self.can_seed_runtime_apply),
            self.stages.len(),
            self.total_step_count(),
            self.blockers.len(),
            self.source_regions().len(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "seed={} stages={} steps={} blockers={} regions=[{}]",
            bool_label(self.can_seed_runtime_apply),
            self.stages.len(),
            self.total_step_count(),
            self.blockers.len(),
            self.source_regions()
                .iter()
                .map(source_region_summary)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadConsumerSourceRegion> {
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadConsumerSourceRegion> {
        let mut source_regions = Vec::new();

        for stage in &self.stages {
            let source_region_name = source_region_name_for_stage_kind(stage.kind);
            let source_region = match source_regions.iter_mut().find(
                |candidate: &&mut SavePostLoadConsumerSourceRegion| {
                    candidate.source_region_name == source_region_name
                },
            ) {
                Some(source_region) => source_region,
                None => {
                    source_regions.push(SavePostLoadConsumerSourceRegion {
                        source_region_name,
                        stage_count: 0,
                        step_count: 0,
                        blocker_count: 0,
                    });
                    source_regions
                        .last_mut()
                        .expect("source region was just pushed")
                }
            };

            source_region.stage_count += 1;
            source_region.step_count += stage.step_count;
            source_region.blocker_count += stage_blockers(self, stage.kind).len();
        }

        source_regions
    }

    pub fn total_step_count(&self) -> usize {
        self.stages.iter().map(|stage| stage.step_count).sum()
    }

    pub fn required_step_count(&self) -> usize {
        self.stages
            .iter()
            .filter(|stage| !stage.deferred)
            .map(|stage| stage.step_count)
            .sum()
    }

    pub fn deferred_step_count(&self) -> usize {
        self.stages
            .iter()
            .filter(|stage| stage.deferred)
            .map(|stage| stage.step_count)
            .sum()
    }

    pub fn consumer_runtime_helper(&self) -> SavePostLoadConsumerRuntimeHelper {
        let world_shell_ready =
            stage_blockers(self, SavePostLoadConsumerStageKind::WorldShell).is_empty();
        let stages = self
            .stages
            .iter()
            .map(|stage| {
                let blockers = stage_blockers(self, stage.kind);
                let disposition = if stage.deferred {
                    SavePostLoadConsumerRuntimeDisposition::Deferred
                } else if !blockers.is_empty() {
                    SavePostLoadConsumerRuntimeDisposition::Blocked
                } else if stage_requires_world_shell(stage.kind) && !world_shell_ready {
                    SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell
                } else {
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow
                };

                SavePostLoadConsumerRuntimeStageHelper {
                    kind: stage.kind,
                    step_count: stage.step_count,
                    disposition,
                    blockers,
                }
            })
            .collect();

        SavePostLoadConsumerRuntimeHelper {
            can_seed_runtime_apply: self.can_seed_runtime_apply,
            world_shell_ready,
            stages,
        }
    }
}

impl SavePostLoadConsumerRuntimeHelper {
    pub fn summary_label(&self) -> String {
        format!(
            "seed={} shell={} stages={} apply={} waiting={} blocked={} deferred={} regions={}",
            bool_label(self.can_seed_runtime_apply),
            bool_label(self.world_shell_ready),
            self.stages.len(),
            self.apply_now_step_count(),
            self.awaiting_world_shell_step_count(),
            self.blocked_step_count(),
            self.deferred_step_count(),
            self.source_regions().len(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "seed={} shell={} stages={} apply={} waiting={} blocked={} deferred={} regions=[{}]",
            bool_label(self.can_seed_runtime_apply),
            bool_label(self.world_shell_ready),
            self.stages.len(),
            self.apply_now_step_count(),
            self.awaiting_world_shell_step_count(),
            self.blocked_step_count(),
            self.deferred_step_count(),
            self.source_regions()
                .iter()
                .map(source_region_summary)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    pub fn has_blocked_stages(&self) -> bool {
        self.stages
            .iter()
            .any(|stage| stage.disposition == SavePostLoadConsumerRuntimeDisposition::Blocked)
    }

    pub fn stage(
        &self,
        kind: SavePostLoadConsumerStageKind,
    ) -> Option<&SavePostLoadConsumerRuntimeStageHelper> {
        self.stages.iter().find(|stage| stage.kind == kind)
    }

    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<SavePostLoadConsumerSourceRegion> {
        self.source_regions()
            .into_iter()
            .find(|region| region.source_region_name == source_region_name)
    }

    pub fn source_regions(&self) -> Vec<SavePostLoadConsumerSourceRegion> {
        let mut source_regions = Vec::new();

        for stage in &self.stages {
            let source_region_name = source_region_name_for_stage_kind(stage.kind);
            let source_region = match source_regions.iter_mut().find(
                |candidate: &&mut SavePostLoadConsumerSourceRegion| {
                    candidate.source_region_name == source_region_name
                },
            ) {
                Some(source_region) => source_region,
                None => {
                    source_regions.push(SavePostLoadConsumerSourceRegion {
                        source_region_name,
                        stage_count: 0,
                        step_count: 0,
                        blocker_count: 0,
                    });
                    source_regions
                        .last_mut()
                        .expect("source region was just pushed")
                }
            };

            source_region.stage_count += 1;
            source_region.step_count += stage.step_count;
            source_region.blocker_count += stage.blockers.len();
        }

        source_regions
    }

    pub fn apply_now_step_count(&self) -> usize {
        runtime_step_count(self, SavePostLoadConsumerRuntimeDisposition::ApplyNow)
    }

    pub fn awaiting_world_shell_step_count(&self) -> usize {
        runtime_step_count(
            self,
            SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
        )
    }

    pub fn blocked_step_count(&self) -> usize {
        runtime_step_count(self, SavePostLoadConsumerRuntimeDisposition::Blocked)
    }

    pub fn deferred_step_count(&self) -> usize {
        runtime_step_count(self, SavePostLoadConsumerRuntimeDisposition::Deferred)
    }
}

fn source_region_summary(region: &SavePostLoadConsumerSourceRegion) -> String {
    format!(
        "{}:{}/{}/{}",
        region.source_region_name, region.stage_count, region.step_count, region.blocker_count
    )
}

fn bool_label(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

impl SavePostLoadWorldObservation {
    pub fn consumer_apply_plan(&self) -> SavePostLoadConsumerApplyPlan {
        self.runtime_seed_plan().consumer_apply_plan()
    }

    pub fn consumer_runtime_helper(&self) -> SavePostLoadConsumerRuntimeHelper {
        self.runtime_seed_plan().consumer_runtime_helper()
    }
}

impl SavePostLoadRuntimeSeedPlan {
    pub fn consumer_apply_plan(&self) -> SavePostLoadConsumerApplyPlan {
        SavePostLoadConsumerApplyPlan {
            can_seed_runtime_apply: self.can_seed_runtime_apply(),
            stages: consumer_stages(self),
            blockers: consumer_blockers(self),
        }
    }

    pub fn consumer_runtime_helper(&self) -> SavePostLoadConsumerRuntimeHelper {
        self.consumer_apply_plan().consumer_runtime_helper()
    }
}

fn consumer_stages(plan: &SavePostLoadRuntimeSeedPlan) -> Vec<SavePostLoadConsumerStage> {
    vec![
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::WorldShell,
            step_count: 1,
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::EntityRemaps,
            step_count: plan.entity_remap_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::TeamPlans,
            step_count: plan.team_plan_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::Markers,
            step_count: plan.marker_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::StaticFog,
            step_count: usize::from(plan.static_fog_seed.is_some()),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::CustomChunks,
            step_count: plan.custom_chunk_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::Buildings,
            step_count: plan.building_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::LoadableEntities,
            step_count: plan.loadable_entity_seeds.len(),
            deferred: false,
        },
        SavePostLoadConsumerStage {
            kind: SavePostLoadConsumerStageKind::SkippedEntities,
            step_count: plan.skipped_entity_seeds.len(),
            deferred: true,
        },
    ]
}

fn consumer_blockers(plan: &SavePostLoadRuntimeSeedPlan) -> Vec<SavePostLoadConsumerBlocker> {
    let mut blockers = Vec::new();

    blockers.extend(
        plan.contract
            .issues
            .iter()
            .copied()
            .map(SavePostLoadConsumerBlocker::ContractIssue),
    );
    blockers.extend(
        plan.activation
            .duplicate_entity_ids
            .iter()
            .copied()
            .map(SavePostLoadConsumerBlocker::DuplicateEntityId),
    );
    blockers.extend(
        plan.building_seeds
            .iter()
            .filter(|seed| !seed.activation.center_reference_valid)
            .map(
                |seed| SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: seed.activation.center_index,
                    tile_index: seed.activation.tile_index,
                    block_id: seed.activation.block_id,
                },
            ),
    );
    blockers.extend(plan.skipped_entity_seeds.iter().map(|seed| {
        SavePostLoadConsumerBlocker::SkippedEntity {
            entity_index: seed.entity_index,
            entity_id: seed.activation.entity_id,
            source_name: seed.activation.source_name.clone(),
            effective_name: seed.activation.effective_name.clone(),
        }
    }));

    blockers
}

fn runtime_step_count(
    helper: &SavePostLoadConsumerRuntimeHelper,
    disposition: SavePostLoadConsumerRuntimeDisposition,
) -> usize {
    helper
        .stages
        .iter()
        .filter(|stage| stage.disposition == disposition)
        .map(|stage| stage.step_count)
        .sum()
}

fn stage_requires_world_shell(kind: SavePostLoadConsumerStageKind) -> bool {
    matches!(
        kind,
        SavePostLoadConsumerStageKind::TeamPlans
            | SavePostLoadConsumerStageKind::Markers
            | SavePostLoadConsumerStageKind::StaticFog
            | SavePostLoadConsumerStageKind::Buildings
            | SavePostLoadConsumerStageKind::LoadableEntities
    )
}

fn stage_blockers(
    plan: &SavePostLoadConsumerApplyPlan,
    kind: SavePostLoadConsumerStageKind,
) -> Vec<SavePostLoadConsumerBlocker> {
    plan.blockers
        .iter()
        .filter(|blocker| blocker_blocks_stage(blocker, kind))
        .cloned()
        .collect()
}

fn blocker_blocks_stage(
    blocker: &SavePostLoadConsumerBlocker,
    kind: SavePostLoadConsumerStageKind,
) -> bool {
    match blocker {
        SavePostLoadConsumerBlocker::ContractIssue(issue) => {
            contract_issue_blocks_world_shell(*issue)
                && kind == SavePostLoadConsumerStageKind::WorldShell
                || contract_issue_blocks_stage(*issue, kind)
        }
        SavePostLoadConsumerBlocker::DuplicateEntityId(_) => {
            kind == SavePostLoadConsumerStageKind::LoadableEntities
        }
        SavePostLoadConsumerBlocker::InvalidBuildingReference { .. } => {
            kind == SavePostLoadConsumerStageKind::Buildings
        }
        SavePostLoadConsumerBlocker::SkippedEntity { .. } => {
            kind == SavePostLoadConsumerStageKind::SkippedEntities
        }
    }
}

fn contract_issue_blocks_world_shell(issue: SavePostLoadWorldIssue) -> bool {
    contract_issue_stage_kind(issue).is_some()
        || matches!(
            issue,
            SavePostLoadWorldIssue::EmptyWorldGraph
                | SavePostLoadWorldIssue::TileSurfaceCountMismatch
                | SavePostLoadWorldIssue::TileSurfaceIndexMismatch
        )
}

fn contract_issue_blocks_stage(
    issue: SavePostLoadWorldIssue,
    kind: SavePostLoadConsumerStageKind,
) -> bool {
    contract_issue_stage_kind(issue) == Some(kind)
}

fn contract_issue_stage_kind(
    issue: SavePostLoadWorldIssue,
) -> Option<SavePostLoadConsumerStageKind> {
    matches!(
        issue,
        SavePostLoadWorldIssue::BuildingCenterReferenceMismatch
    )
    .then_some(SavePostLoadConsumerStageKind::Buildings)
    .or_else(|| {
        matches!(
            issue,
            SavePostLoadWorldIssue::TeamPlanOverlayMismatch
                | SavePostLoadWorldIssue::TeamPlanOutOfBounds
                | SavePostLoadWorldIssue::DuplicateTeamPlanGroupIds
        )
        .then_some(SavePostLoadConsumerStageKind::TeamPlans)
    })
    .or_else(|| {
        matches!(
            issue,
            SavePostLoadWorldIssue::MarkerRegionMismatch
                | SavePostLoadWorldIssue::MarkerOutOfBounds
        )
        .then_some(SavePostLoadConsumerStageKind::Markers)
    })
    .or_else(|| {
        matches!(
            issue,
            SavePostLoadWorldIssue::StaticFogDimensionMismatch
                | SavePostLoadWorldIssue::StaticFogCoverageMismatch
                | SavePostLoadWorldIssue::DuplicateStaticFogTeamIds
        )
        .then_some(SavePostLoadConsumerStageKind::StaticFog)
    })
    .or_else(|| {
        matches!(
            issue,
            SavePostLoadWorldIssue::WorldEntityCountMismatch
                | SavePostLoadWorldIssue::DuplicateWorldEntityIds
                | SavePostLoadWorldIssue::EntitySummaryMismatch
        )
        .then_some(SavePostLoadConsumerStageKind::LoadableEntities)
    })
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
        SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam, TeamPlan, TeamPlanGroup,
        TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn consumer_apply_plan_tracks_required_and_deferred_steps() {
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

        let plan = observation.consumer_apply_plan();

        assert!(plan.can_seed_runtime_apply);
        assert!(!plan.has_blockers());
        assert_eq!(
            plan.stages,
            vec![
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::WorldShell,
                    step_count: 1,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::EntityRemaps,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::TeamPlans,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::Markers,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::StaticFog,
                    step_count: 1,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::CustomChunks,
                    step_count: 2,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::Buildings,
                    step_count: 1,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::LoadableEntities,
                    step_count: 3,
                    deferred: false,
                },
                SavePostLoadConsumerStage {
                    kind: SavePostLoadConsumerStageKind::SkippedEntities,
                    step_count: 0,
                    deferred: true,
                },
            ]
        );
        assert_eq!(plan.total_step_count(), 14);
        assert_eq!(plan.required_step_count(), 14);
        assert_eq!(plan.deferred_step_count(), 0);
    }

    #[test]
    fn consumer_apply_plan_surfaces_contract_and_activation_blockers() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let plan = observation.consumer_apply_plan();

        assert!(!plan.can_seed_runtime_apply);
        assert!(plan.has_blockers());
        assert_eq!(plan.total_step_count(), 14);
        assert_eq!(plan.required_step_count(), 13);
        assert_eq!(plan.deferred_step_count(), 1);
        assert_eq!(
            plan.blockers,
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
                SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                SavePostLoadConsumerBlocker::InvalidBuildingReference {
                    center_index: 0,
                    tile_index: 0,
                    block_id: 0x0153,
                },
                SavePostLoadConsumerBlocker::SkippedEntity {
                    entity_index: 1,
                    entity_id: 43,
                    source_name: "mod-unit".to_string(),
                    effective_name: None,
                },
            ]
        );
    }

    #[test]
    fn consumer_runtime_helper_marks_clean_stages_apply_now() {
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

        let helper = observation.consumer_runtime_helper();

        assert!(helper.can_seed_runtime_apply);
        assert!(helper.world_shell_ready);
        assert!(!helper.has_blocked_stages());
        assert_eq!(helper.apply_now_step_count(), 14);
        assert_eq!(helper.awaiting_world_shell_step_count(), 0);
        assert_eq!(helper.blocked_step_count(), 0);
        assert_eq!(helper.deferred_step_count(), 0);
        assert_eq!(
            helper
                .stages
                .iter()
                .map(|stage| (stage.kind, stage.disposition, stage.blockers.len()))
                .collect::<Vec<_>>(),
            vec![
                (
                    SavePostLoadConsumerStageKind::WorldShell,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::EntityRemaps,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::TeamPlans,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::Markers,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::StaticFog,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::CustomChunks,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::Buildings,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::LoadableEntities,
                    SavePostLoadConsumerRuntimeDisposition::ApplyNow,
                    0,
                ),
                (
                    SavePostLoadConsumerStageKind::SkippedEntities,
                    SavePostLoadConsumerRuntimeDisposition::Deferred,
                    0,
                ),
            ]
        );
        assert!(helper
            .stages
            .iter()
            .filter(|stage| stage.kind != SavePostLoadConsumerStageKind::SkippedEntities)
            .all(SavePostLoadConsumerRuntimeStageHelper::can_apply_now));
        assert_eq!(
            helper
                .stage(SavePostLoadConsumerStageKind::LoadableEntities)
                .map(|stage| stage.disposition),
            Some(SavePostLoadConsumerRuntimeDisposition::ApplyNow)
        );
    }

    #[test]
    fn stage_requires_world_shell_marks_only_world_shell_sensitive_stages() {
        let cases = [
            (SavePostLoadConsumerStageKind::WorldShell, false),
            (SavePostLoadConsumerStageKind::EntityRemaps, false),
            (SavePostLoadConsumerStageKind::TeamPlans, true),
            (SavePostLoadConsumerStageKind::Markers, true),
            (SavePostLoadConsumerStageKind::StaticFog, true),
            (SavePostLoadConsumerStageKind::CustomChunks, false),
            (SavePostLoadConsumerStageKind::Buildings, true),
            (SavePostLoadConsumerStageKind::LoadableEntities, true),
            (SavePostLoadConsumerStageKind::SkippedEntities, false),
        ];

        for (kind, expected) in cases {
            assert_eq!(stage_requires_world_shell(kind), expected, "{kind:?}");
        }
    }

    #[test]
    fn blocker_blocks_stage_routes_duplicate_entity_ids_only_to_loadable_entities() {
        let blocker = SavePostLoadConsumerBlocker::DuplicateEntityId(42);

        assert!(blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::LoadableEntities
        ));
        assert!(!blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::WorldShell
        ));
        assert!(!blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::Buildings
        ));
        assert!(!blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::SkippedEntities
        ));
    }

    #[test]
    fn blocker_blocks_stage_routes_invalid_building_reference_only_to_buildings() {
        let blocker = SavePostLoadConsumerBlocker::InvalidBuildingReference {
            center_index: 3,
            tile_index: 9,
            block_id: 0x0153,
        };

        assert!(blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::Buildings
        ));
        assert!(!blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::WorldShell
        ));
        assert!(!blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::LoadableEntities
        ));
        assert!(!blocker_blocks_stage(
            &blocker,
            SavePostLoadConsumerStageKind::SkippedEntities
        ));
    }

    #[test]
    fn contract_issue_blocks_world_shell_and_stage_route_all_variants_stably() {
        let no_stages: &[SavePostLoadConsumerStageKind] = &[];
        let building_stage = &[SavePostLoadConsumerStageKind::Buildings];
        let team_plan_stage = &[SavePostLoadConsumerStageKind::TeamPlans];
        let marker_stage = &[SavePostLoadConsumerStageKind::Markers];
        let static_fog_stage = &[SavePostLoadConsumerStageKind::StaticFog];
        let loadable_entity_stage = &[SavePostLoadConsumerStageKind::LoadableEntities];
        let cases = [
            (SavePostLoadWorldIssue::EmptyWorldGraph, true, no_stages),
            (
                SavePostLoadWorldIssue::TileSurfaceCountMismatch,
                true,
                no_stages,
            ),
            (
                SavePostLoadWorldIssue::TileSurfaceIndexMismatch,
                true,
                no_stages,
            ),
            (
                SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                true,
                building_stage,
            ),
            (
                SavePostLoadWorldIssue::TeamPlanOverlayMismatch,
                true,
                team_plan_stage,
            ),
            (
                SavePostLoadWorldIssue::TeamPlanOutOfBounds,
                true,
                team_plan_stage,
            ),
            (
                SavePostLoadWorldIssue::DuplicateTeamPlanGroupIds,
                true,
                team_plan_stage,
            ),
            (
                SavePostLoadWorldIssue::MarkerRegionMismatch,
                true,
                marker_stage,
            ),
            (
                SavePostLoadWorldIssue::MarkerOutOfBounds,
                true,
                marker_stage,
            ),
            (
                SavePostLoadWorldIssue::DuplicateMarkerIds,
                false,
                no_stages,
            ),
            (
                SavePostLoadWorldIssue::StaticFogDimensionMismatch,
                true,
                static_fog_stage,
            ),
            (
                SavePostLoadWorldIssue::StaticFogCoverageMismatch,
                true,
                static_fog_stage,
            ),
            (
                SavePostLoadWorldIssue::DuplicateStaticFogTeamIds,
                true,
                static_fog_stage,
            ),
            (
                SavePostLoadWorldIssue::DuplicateCustomChunkNames,
                false,
                no_stages,
            ),
            (
                SavePostLoadWorldIssue::WorldEntityCountMismatch,
                true,
                loadable_entity_stage,
            ),
            (
                SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                true,
                loadable_entity_stage,
            ),
            (
                SavePostLoadWorldIssue::EntitySummaryMismatch,
                true,
                loadable_entity_stage,
            ),
        ];
        let stage_kinds = [
            SavePostLoadConsumerStageKind::WorldShell,
            SavePostLoadConsumerStageKind::EntityRemaps,
            SavePostLoadConsumerStageKind::TeamPlans,
            SavePostLoadConsumerStageKind::Markers,
            SavePostLoadConsumerStageKind::StaticFog,
            SavePostLoadConsumerStageKind::CustomChunks,
            SavePostLoadConsumerStageKind::Buildings,
            SavePostLoadConsumerStageKind::LoadableEntities,
            SavePostLoadConsumerStageKind::SkippedEntities,
        ];

        for (issue, expected_world_shell, expected_stages) in cases {
            assert_eq!(
                contract_issue_blocks_world_shell(issue),
                expected_world_shell,
                "{issue:?}"
            );
            for kind in stage_kinds {
                assert_eq!(
                    contract_issue_blocks_stage(issue, kind),
                    expected_stages.contains(&kind),
                    "{issue:?} -> {kind:?}"
                );
            }
        }
    }

    #[test]
    fn consumer_runtime_helper_marks_zero_step_apply_now_stage_ready() {
        let mut observation = test_observation();
        observation.custom_chunks.clear();

        let helper = observation.consumer_runtime_helper();
        let stage = helper
            .stage(SavePostLoadConsumerStageKind::CustomChunks)
            .expect("custom chunks stage should be present");

        assert_eq!(stage.step_count, 0);
        assert_eq!(
            stage.disposition,
            SavePostLoadConsumerRuntimeDisposition::ApplyNow
        );
        assert!(stage.can_apply_now());
    }

    #[test]
    fn consumer_apply_plan_groups_stage_and_blocker_counts_by_source_region() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let plan = observation.consumer_apply_plan();
        let helper = observation.consumer_runtime_helper();
        let expected_regions = vec![
            SavePostLoadConsumerSourceRegion {
                source_region_name: "map",
                stage_count: 2,
                step_count: 2,
                blocker_count: 5,
            },
            SavePostLoadConsumerSourceRegion {
                source_region_name: "entities",
                stage_count: 4,
                step_count: 7,
                blocker_count: 4,
            },
            SavePostLoadConsumerSourceRegion {
                source_region_name: "markers",
                stage_count: 1,
                step_count: 2,
                blocker_count: 0,
            },
            SavePostLoadConsumerSourceRegion {
                source_region_name: "custom",
                stage_count: 2,
                step_count: 3,
                blocker_count: 0,
            },
        ];

        assert_eq!(plan.source_regions(), expected_regions);
        assert_eq!(helper.source_regions(), expected_regions);
        assert_eq!(plan.source_region("entities"), Some(expected_regions[1].clone()));
        assert_eq!(helper.source_region("map"), Some(expected_regions[0].clone()));
        assert!(plan.source_region("missing").is_none());
        assert!(helper.source_region("missing").is_none());
        assert_eq!(
            plan.summary_label(),
            "seed=0 stages=9 steps=14 blockers=6 regions=4"
        );
        assert_eq!(
            plan.detail_label(),
            "seed=0 stages=9 steps=14 blockers=6 regions=[map:2/2/5,entities:4/7/4,markers:1/2/0,custom:2/3/0]"
        );
        assert_eq!(
            helper.summary_label(),
            "seed=0 shell=0 stages=9 apply=4 waiting=5 blocked=4 deferred=1 regions=4"
        );
        assert_eq!(
            helper.detail_label(),
            "seed=0 shell=0 stages=9 apply=4 waiting=5 blocked=4 deferred=1 regions=[map:2/2/5,entities:4/7/4,markers:1/2/0,custom:2/3/0]"
        );
    }

    #[test]
    fn consumer_runtime_helper_splits_apply_now_blocked_and_awaiting_stages() {
        let mut observation = test_observation();
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let helper = observation.runtime_seed_plan().consumer_runtime_helper();

        assert!(!helper.can_seed_runtime_apply);
        assert!(!helper.world_shell_ready);
        assert!(helper.has_blocked_stages());
        assert_eq!(helper.apply_now_step_count(), 4);
        assert_eq!(helper.awaiting_world_shell_step_count(), 5);
        assert_eq!(helper.blocked_step_count(), 4);
        assert_eq!(helper.deferred_step_count(), 1);
        assert_eq!(
            helper.stages,
            vec![
                SavePostLoadConsumerRuntimeStageHelper {
                    kind: SavePostLoadConsumerStageKind::WorldShell,
                    step_count: 1,
                    disposition: SavePostLoadConsumerRuntimeDisposition::Blocked,
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
                    ],
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
                    disposition: SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                    blockers: Vec::new(),
                },
                SavePostLoadConsumerRuntimeStageHelper {
                    kind: SavePostLoadConsumerStageKind::Markers,
                    step_count: 2,
                    disposition: SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
                    blockers: Vec::new(),
                },
                SavePostLoadConsumerRuntimeStageHelper {
                    kind: SavePostLoadConsumerStageKind::StaticFog,
                    step_count: 1,
                    disposition: SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell,
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
                    disposition: SavePostLoadConsumerRuntimeDisposition::Blocked,
                    blockers: vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                        ),
                        SavePostLoadConsumerBlocker::InvalidBuildingReference {
                            center_index: 0,
                            tile_index: 0,
                            block_id: 0x0153,
                        },
                    ],
                },
                SavePostLoadConsumerRuntimeStageHelper {
                    kind: SavePostLoadConsumerStageKind::LoadableEntities,
                    step_count: 2,
                    disposition: SavePostLoadConsumerRuntimeDisposition::Blocked,
                    blockers: vec![
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                        ),
                        SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::EntitySummaryMismatch,
                        ),
                        SavePostLoadConsumerBlocker::DuplicateEntityId(42),
                    ],
                },
                SavePostLoadConsumerRuntimeStageHelper {
                    kind: SavePostLoadConsumerStageKind::SkippedEntities,
                    step_count: 1,
                    disposition: SavePostLoadConsumerRuntimeDisposition::Deferred,
                    blockers: vec![SavePostLoadConsumerBlocker::SkippedEntity {
                        entity_index: 1,
                        entity_id: 43,
                        source_name: "mod-unit".to_string(),
                        effective_name: None,
                    }],
                },
            ]
        );
    }

    #[test]
    fn consumer_runtime_helper_blocks_duplicate_team_plan_group_ids_marker_ids_and_custom_chunk_names(
    ) {
        let mut observation = test_observation();
        observation
            .team_plan_groups
            .push(observation.team_plan_groups[0].clone());
        observation.markers.push(observation.markers[0].clone());
        observation
            .custom_chunks
            .push(observation.custom_chunks[0].clone());

        let helper = observation.consumer_runtime_helper();

        assert!(!helper.can_seed_runtime_apply);
        assert!(!helper.world_shell_ready);
        assert!(helper.has_blocked_stages());
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::WorldShell)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::Blocked
            }));
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::TeamPlans)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::Blocked
                    && stage
                        .blockers
                        .contains(&SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateTeamPlanGroupIds,
                        ))
            }));
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::Markers)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::AwaitingWorldShell
            }));
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::CustomChunks)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
            }));
    }

    #[test]
    fn consumer_runtime_helper_keeps_world_shell_ready_for_auxiliary_marker_and_chunk_duplicates() {
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
        observation.markers.push(observation.markers[0].clone());
        observation
            .custom_chunks
            .push(observation.custom_chunks[0].clone());

        let helper = observation.consumer_runtime_helper();

        assert!(helper.can_seed_runtime_apply);
        assert!(helper.world_shell_ready);
        assert!(!helper.has_blocked_stages());
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::WorldShell)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
            }));
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::Markers)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
            }));
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::CustomChunks)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::ApplyNow
            }));
    }

    #[test]
    fn consumer_runtime_helper_blocks_duplicate_static_fog_team_ids() {
        let mut observation = test_observation();
        if let ParsedCustomChunk::StaticFog(chunk) = &mut observation.custom_chunks[0].parsed {
            chunk.used_teams = 2;
            chunk.teams.push(StaticFogTeam {
                team_id: chunk.teams[0].team_id,
                run_count: chunk.teams[0].run_count,
                rle_bytes: chunk.teams[0].rle_bytes.clone(),
                discovered: chunk.teams[0].discovered.clone(),
            });
        }

        let helper = observation.consumer_runtime_helper();

        assert!(!helper.can_seed_runtime_apply);
        assert!(!helper.world_shell_ready);
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::WorldShell)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::Blocked
                    && stage
                        .blockers
                        .contains(&SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateStaticFogTeamIds,
                        ))
            }));
        assert!(helper
            .stage(SavePostLoadConsumerStageKind::StaticFog)
            .is_some_and(|stage| {
                stage.disposition == SavePostLoadConsumerRuntimeDisposition::Blocked
                    && stage
                        .blockers
                        .contains(&SavePostLoadConsumerBlocker::ContractIssue(
                            SavePostLoadWorldIssue::DuplicateStaticFogTeamIds,
                        ))
            }));
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

    #[test]
    fn bool_label_formats_true_and_false() {
        assert_eq!(bool_label(true), "1");
        assert_eq!(bool_label(false), "0");
    }

    #[test]
    fn consumer_apply_plan_summary_and_detail_labels_are_stable_for_empty_state() {
        let plan = SavePostLoadConsumerApplyPlan {
            can_seed_runtime_apply: false,
            stages: Vec::new(),
            blockers: Vec::new(),
        };

        assert_eq!(
            plan.summary_label(),
            "seed=0 stages=0 steps=0 blockers=0 regions=0"
        );
        assert_eq!(
            plan.detail_label(),
            "seed=0 stages=0 steps=0 blockers=0 regions=[]"
        );
    }
}
