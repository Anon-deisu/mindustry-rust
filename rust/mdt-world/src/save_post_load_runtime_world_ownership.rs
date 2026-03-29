use crate::{
    save_post_load_runtime_source_region::source_region_name_for_stage_kind,
    SavePostLoadConsumerBlocker, SavePostLoadConsumerStageKind, SavePostLoadRuntimeApplyStep,
    SavePostLoadRuntimeSeedPlan, SavePostLoadRuntimeWorldSemanticsExecution,
    SavePostLoadWorldObservation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SavePostLoadRuntimeWorldSurfaceKind {
    WorldShell,
    TeamPlans,
    Markers,
    StaticFog,
    Buildings,
    LoadableEntities,
    SkippedEntities,
}

impl SavePostLoadRuntimeWorldSurfaceKind {
    pub fn source_region_name(&self) -> &'static str {
        source_region_name_for_stage_kind(self.stage_kind())
    }

    pub const fn ordered() -> [Self; 7] {
        [
            Self::WorldShell,
            Self::TeamPlans,
            Self::Markers,
            Self::StaticFog,
            Self::Buildings,
            Self::LoadableEntities,
            Self::SkippedEntities,
        ]
    }

    pub(crate) fn from_stage_kind(kind: SavePostLoadConsumerStageKind) -> Option<Self> {
        match kind {
            SavePostLoadConsumerStageKind::WorldShell => Some(Self::WorldShell),
            SavePostLoadConsumerStageKind::TeamPlans => Some(Self::TeamPlans),
            SavePostLoadConsumerStageKind::Markers => Some(Self::Markers),
            SavePostLoadConsumerStageKind::StaticFog => Some(Self::StaticFog),
            SavePostLoadConsumerStageKind::Buildings => Some(Self::Buildings),
            SavePostLoadConsumerStageKind::LoadableEntities => Some(Self::LoadableEntities),
            SavePostLoadConsumerStageKind::SkippedEntities => Some(Self::SkippedEntities),
            SavePostLoadConsumerStageKind::EntityRemaps
            | SavePostLoadConsumerStageKind::CustomChunks => None,
        }
    }

    pub(crate) fn from_step(step: &SavePostLoadRuntimeApplyStep) -> Option<Self> {
        match step {
            SavePostLoadRuntimeApplyStep::WorldShell => Some(Self::WorldShell),
            SavePostLoadRuntimeApplyStep::TeamPlan { .. } => Some(Self::TeamPlans),
            SavePostLoadRuntimeApplyStep::Marker { .. } => Some(Self::Markers),
            SavePostLoadRuntimeApplyStep::StaticFog => Some(Self::StaticFog),
            SavePostLoadRuntimeApplyStep::Building { .. } => Some(Self::Buildings),
            SavePostLoadRuntimeApplyStep::LoadableEntity { .. } => Some(Self::LoadableEntities),
            SavePostLoadRuntimeApplyStep::SkippedEntity { .. } => Some(Self::SkippedEntities),
            SavePostLoadRuntimeApplyStep::EntityRemap { .. }
            | SavePostLoadRuntimeApplyStep::CustomChunk { .. } => None,
        }
    }

    const fn stage_kind(&self) -> SavePostLoadConsumerStageKind {
        match self {
            SavePostLoadRuntimeWorldSurfaceKind::WorldShell => {
                SavePostLoadConsumerStageKind::WorldShell
            }
            SavePostLoadRuntimeWorldSurfaceKind::TeamPlans => {
                SavePostLoadConsumerStageKind::TeamPlans
            }
            SavePostLoadRuntimeWorldSurfaceKind::Markers => SavePostLoadConsumerStageKind::Markers,
            SavePostLoadRuntimeWorldSurfaceKind::StaticFog => {
                SavePostLoadConsumerStageKind::StaticFog
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

    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn has_failures(&self) -> bool {
        !self.failed_steps.is_empty()
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
        self.surfaces
            .iter()
            .map(|surface| surface.required_step_count)
            .sum()
    }

    pub fn claimed_step_count(&self) -> usize {
        self.surfaces
            .iter()
            .map(|surface| surface.claimed_step_count)
            .sum()
    }

    pub fn owned_surface_count(&self) -> usize {
        self.surfaces
            .iter()
            .filter(|surface| surface.is_owned())
            .count()
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
            let claimed_step_count = shell
                .map(|shell| shell.owned_step_count(kind))
                .unwrap_or_default();

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
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, ContentHeaderEntry,
        CoreTailSnapshot, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
        ParsedCustomChunk, PointMarkerModel, SaveEntityChunkObservation, SaveEntityClassKind,
        SaveEntityClassSummary, SaveEntityPostLoadClassSummary, SaveEntityPostLoadKind,
        SaveEntityPostLoadSummary, SaveEntityRemapEntry, SaveEntityRemapSummary,
        SaveMapRegionObservation, SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam,
        TeamPlan, TeamPlanGroup, TileModel, TypeIoValue, WorldModel,
    };

    #[test]
    fn runtime_world_surface_kind_ignores_entity_remaps_and_custom_chunks() {
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_stage_kind(
                SavePostLoadConsumerStageKind::EntityRemaps,
            ),
            None
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_stage_kind(
                SavePostLoadConsumerStageKind::CustomChunks,
            ),
            None
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_step(
                &SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 }
            ),
            None
        );
        assert_eq!(
            SavePostLoadRuntimeWorldSurfaceKind::from_step(
                &SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 }
            ),
            None
        );
    }

    #[test]
    fn runtime_world_ownership_marks_clean_world_surfaces_owned() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let ownership = observation.runtime_world_ownership();

        assert!(ownership.world_shell_ready);
        assert!(ownership.can_apply_world_semantics());
        assert!(ownership.can_activate_live_runtime());
        assert_eq!(ownership.required_step_count(), 10);
        assert_eq!(ownership.claimed_step_count(), 10);
        assert_eq!(ownership.owned_surface_count(), 6);
        assert_eq!(
            ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities)
                .unwrap(),
            &SavePostLoadRuntimeWorldOwnershipSurface {
                kind: SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
                source_region_name: "entities",
                required_step_count: 3,
                claimed_step_count: 3,
                status: SavePostLoadRuntimeWorldOwnershipStatus::Owned,
                blockers: Vec::new(),
                failed_steps: Vec::new(),
            }
        );
    }

    #[test]
    fn runtime_world_ownership_keeps_failed_marker_surface_unowned() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        observation.markers[1].id = observation.markers[0].id;

        let ownership = observation.runtime_world_ownership();
        let markers = ownership
            .surface(SavePostLoadRuntimeWorldSurfaceKind::Markers)
            .unwrap();

        assert!(ownership.world_shell_ready);
        assert!(!ownership.can_apply_world_semantics());
        assert!(!ownership.can_activate_live_runtime());
        assert_eq!(ownership.required_step_count(), 10);
        assert_eq!(ownership.claimed_step_count(), 9);
        assert_eq!(ownership.owned_surface_count(), 5);
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
        let team_plans = ownership
            .surface(SavePostLoadRuntimeWorldSurfaceKind::TeamPlans)
            .unwrap();

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
        observation.world_entity_chunks[2].entity_id = 42;
        observation.entity_summary.duplicate_entity_ids = vec![42];
        observation.entity_summary.unique_entity_ids = 2;
        observation.map.world.tiles[0].building_center_index = None;

        let ownership = observation.runtime_world_ownership();

        assert!(!ownership.world_shell_ready);
        assert!(!ownership.can_apply_world_semantics());
        assert!(!ownership.can_activate_live_runtime());
        assert_eq!(ownership.claimed_step_count(), 0);
        assert_eq!(
            ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::WorldShell)
                .unwrap()
                .status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
        assert_eq!(
            ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::TeamPlans)
                .unwrap()
                .status,
            SavePostLoadRuntimeWorldOwnershipStatus::AwaitingWorldShell
        );
        assert_eq!(
            ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::Buildings)
                .unwrap()
                .status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
        assert_eq!(
            ownership
                .surface(SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities)
                .unwrap()
                .status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
    }

    #[test]
    fn runtime_world_ownership_preserves_deferred_skipped_entities_surface() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
        let mut plan = observation.runtime_seed_plan();
        let mut skipped = plan.loadable_entity_seeds[1].clone();
        skipped.entity_index = 99;
        plan.skipped_entity_seeds.push(skipped);

        let execution = plan.execute_runtime_world_semantics();
        let skipped_surface = execution
            .ownership
            .surface(SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities)
            .unwrap();

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
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);

        let mut ownership = observation.runtime_world_ownership();
        ownership.world_shell_ready = false;

        assert!(!ownership.can_apply_world_semantics());
        assert!(!ownership.can_activate_live_runtime());
    }

    #[test]
    fn runtime_world_ownership_blocks_duplicate_static_fog_team_ids() {
        let mut observation = test_observation();
        make_observation_seedable(&mut observation);
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
        let static_fog = ownership
            .surface(SavePostLoadRuntimeWorldSurfaceKind::StaticFog)
            .unwrap();

        assert!(!ownership.world_shell_ready);
        assert!(!ownership.can_apply_world_semantics());
        assert!(!ownership.can_activate_live_runtime());
        assert_eq!(
            static_fog.status,
            SavePostLoadRuntimeWorldOwnershipStatus::Blocked
        );
        assert!(static_fog
            .blockers
            .contains(&SavePostLoadConsumerBlocker::ContractIssue(
                crate::SavePostLoadWorldIssue::DuplicateStaticFogTeamIds,
            )));
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
