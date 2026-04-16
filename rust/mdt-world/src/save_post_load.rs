use crate::{
    marker_region_is_empty, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
    ParsedCustomChunk, SavePostLoadRuntimeApplyExecution, SavePostLoadRuntimeReadiness,
    SavePostLoadRuntimeSeedSurface, SavePostLoadRuntimeSourceRegionReadiness,
    SavePostLoadRuntimeWorldOwnership, save_post_load_runtime_world_ownership::SavePostLoadRuntimeWorldOwnershipSourceRegion,
    SavePostLoadRuntimeWorldSemanticsExecution, SavePostLoadWorldObservation, StaticFogChunk,
    TeamPlan, TeamPlanGroup, WorldGraph, WorldLoadUnknownCoverageSummary,
};

fn unique_match<'a, T, F>(
    mut iter: impl Iterator<Item = &'a T>,
    mut predicate: F,
) -> Option<&'a T>
where
    F: FnMut(&T) -> bool,
{
    let first = iter.find(|item| predicate(item))?;
    if iter.any(|item| predicate(item)) {
        None
    } else {
        Some(first)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavePostLoadWorldApplyBundle<'a> {
    observation: &'a SavePostLoadWorldObservation,
    pub runtime_readiness: SavePostLoadRuntimeReadiness,
    pub runtime_seed_surface: SavePostLoadRuntimeSeedSurface,
    pub runtime_apply: SavePostLoadRuntimeApplyExecution,
    pub runtime_world_semantics: SavePostLoadRuntimeWorldSemanticsExecution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadWorldApplyBundleReadiness<'a> {
    pub runtime_readiness: &'a SavePostLoadRuntimeReadiness,
    pub runtime_seed_surface: &'a SavePostLoadRuntimeSeedSurface,
    pub source_regions: Vec<SavePostLoadRuntimeSourceRegionReadiness>,
}

impl<'a> SavePostLoadWorldApplyBundleReadiness<'a> {
    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<&SavePostLoadRuntimeSourceRegionReadiness> {
        self.source_regions
            .iter()
            .find(|region| region.source_region_name == source_region_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadWorldApplyBundleOwnership<'a> {
    pub runtime_world_ownership: &'a SavePostLoadRuntimeWorldOwnership,
    pub source_regions: Vec<SavePostLoadRuntimeWorldOwnershipSourceRegion>,
}

impl<'a> SavePostLoadWorldApplyBundleOwnership<'a> {
    pub fn source_region(
        &self,
        source_region_name: &str,
    ) -> Option<&SavePostLoadRuntimeWorldOwnershipSourceRegion> {
        self.source_regions
            .iter()
            .find(|region| region.source_region_name == source_region_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadWorldApplyBundleDecision<'a> {
    pub runtime_readiness: &'a SavePostLoadRuntimeReadiness,
    pub runtime_world_ownership: &'a SavePostLoadRuntimeWorldOwnership,
}

impl<'a> SavePostLoadWorldApplyBundleDecision<'a> {
    pub fn can_seed_runtime_apply(&self) -> bool {
        self.runtime_readiness.can_seed_runtime_apply
    }

    pub fn can_apply_world_semantics(&self) -> bool {
        self.runtime_world_ownership.can_apply_world_semantics()
    }

    pub fn readiness_world_shell_ready(&self) -> bool {
        self.runtime_readiness.world_shell_ready
    }

    pub fn ownership_world_shell_ready(&self) -> bool {
        self.runtime_world_ownership.world_shell_ready
    }

    pub fn apply_now_step_count(&self) -> usize {
        self.runtime_readiness.apply_now_step_count()
    }

    pub fn awaiting_world_shell_step_count(&self) -> usize {
        self.runtime_readiness.awaiting_world_shell_step_count()
    }

    pub fn blocked_step_count(&self) -> usize {
        self.runtime_readiness.blocked_step_count()
    }

    pub fn deferred_step_count(&self) -> usize {
        self.runtime_readiness.deferred_step_count()
    }

    pub fn required_surface_count(&self) -> usize {
        self.runtime_world_ownership.surfaces.len()
    }

    pub fn owned_surface_count(&self) -> usize {
        self.runtime_world_ownership.owned_surface_count()
    }

    pub fn required_step_count(&self) -> usize {
        self.runtime_world_ownership.required_step_count()
    }

    pub fn claimed_step_count(&self) -> usize {
        self.runtime_world_ownership.claimed_step_count()
    }

    pub fn summary_label(&self) -> String {
        format!(
            "seed={} semantics={} shell={}/{} apply={} wait={} block={} defer={} own={}/{} claim={}/{}",
            bool_label(self.can_seed_runtime_apply()),
            bool_label(self.can_apply_world_semantics()),
            bool_label(self.readiness_world_shell_ready()),
            bool_label(self.ownership_world_shell_ready()),
            self.apply_now_step_count(),
            self.awaiting_world_shell_step_count(),
            self.blocked_step_count(),
            self.deferred_step_count(),
            self.owned_surface_count(),
            self.required_surface_count(),
            self.claimed_step_count(),
            self.required_step_count(),
        )
    }

    pub fn detail_label(&self) -> String {
        format!(
            "seed={} semantics={} shell={}/{} apply={} wait={} block={} defer={} own={}/{} claim={}/{} readiness_sources={} ownership_sources={}",
            bool_label(self.can_seed_runtime_apply()),
            bool_label(self.can_apply_world_semantics()),
            bool_label(self.readiness_world_shell_ready()),
            bool_label(self.ownership_world_shell_ready()),
            self.apply_now_step_count(),
            self.awaiting_world_shell_step_count(),
            self.blocked_step_count(),
            self.deferred_step_count(),
            self.owned_surface_count(),
            self.required_surface_count(),
            self.claimed_step_count(),
            self.required_step_count(),
            self.runtime_readiness.source_regions().len(),
            self.runtime_world_ownership.source_regions().len(),
        )
    }
}

impl<'a> SavePostLoadWorldApplyBundle<'a> {
    pub fn graph(&self) -> WorldGraph<'a> {
        self.observation.graph()
    }

    pub fn runtime_readiness_summary(&self) -> SavePostLoadWorldApplyBundleReadiness<'_> {
        SavePostLoadWorldApplyBundleReadiness {
            runtime_readiness: &self.runtime_readiness,
            runtime_seed_surface: &self.runtime_seed_surface,
            source_regions: self.runtime_readiness.source_regions(),
        }
    }

    pub fn runtime_world_ownership_summary(
        &self,
    ) -> SavePostLoadWorldApplyBundleOwnership<'_> {
        SavePostLoadWorldApplyBundleOwnership {
            runtime_world_ownership: self.runtime_world_ownership(),
            source_regions: self.runtime_world_ownership().source_regions(),
        }
    }

    pub fn runtime_decision_summary(&self) -> SavePostLoadWorldApplyBundleDecision<'_> {
        SavePostLoadWorldApplyBundleDecision {
            runtime_readiness: &self.runtime_readiness,
            runtime_world_ownership: self.runtime_world_ownership(),
        }
    }

    pub fn team_plan_group(&self, team_id: u32) -> Option<&'a TeamPlanGroup> {
        self.observation.team_plan_group(team_id)
    }

    pub fn custom_chunk(&self, name: &str) -> Option<&'a CustomChunkEntry> {
        self.observation.custom_chunk(name)
    }

    pub fn world_entity_chunk(&self, entity_id: i32) -> Option<&'a crate::SaveEntityChunkObservation> {
        self.observation.world_entity_chunk(entity_id)
    }

    pub fn marker(&self, id: i32) -> Option<&'a MarkerEntry> {
        self.observation.marker(id)
    }

    pub fn static_fog_chunk(&self) -> Option<&'a StaticFogChunk> {
        self.observation.static_fog_chunk()
    }

    pub fn unknown_coverage_summary(&self) -> WorldLoadUnknownCoverageSummary {
        self.observation.unknown_coverage_summary()
    }

    pub fn runtime_world_ownership(&self) -> &SavePostLoadRuntimeWorldOwnership {
        &self.runtime_world_semantics.ownership
    }
}

impl SavePostLoadWorldObservation {
    pub fn post_load_world_apply_bundle(&self) -> SavePostLoadWorldApplyBundle<'_> {
        SavePostLoadWorldApplyBundle {
            observation: self,
            runtime_readiness: self.runtime_readiness(),
            runtime_seed_surface: self.runtime_seed_surface(),
            runtime_apply: self.execute_runtime_apply(),
            runtime_world_semantics: self.execute_runtime_world_semantics(),
        }
    }

    pub fn graph(&self) -> WorldGraph<'_> {
        WorldGraph::from_parts(
            &self.map.world,
            &self.team_plan_groups,
            &self.markers,
            &self.marker_region_bytes,
            &self.custom_chunks,
            self.static_fog_chunk(),
        )
    }

    pub fn team_plan_group(&self, team_id: u32) -> Option<&TeamPlanGroup> {
        unique_match(self.team_plan_groups.iter(), |group| group.team_id == team_id)
    }

    pub fn all_team_plans(&self) -> impl Iterator<Item = &TeamPlan> {
        self.team_plan_groups
            .iter()
            .flat_map(|group| group.plans.iter())
    }

    pub fn custom_chunk(&self, name: &str) -> Option<&CustomChunkEntry> {
        unique_match(self.custom_chunks.iter(), |chunk| chunk.name == name)
    }

    pub fn world_entity_chunk(&self, entity_id: i32) -> Option<&crate::SaveEntityChunkObservation> {
        unique_match(self.world_entity_chunks.iter(), |chunk| chunk.entity_id == entity_id)
    }

    pub fn marker(&self, id: i32) -> Option<&MarkerEntry> {
        unique_match(self.markers.iter(), |marker| marker.id == id)
    }

    pub fn all_markers(&self) -> impl Iterator<Item = &MarkerEntry> {
        self.markers.iter()
    }

    pub fn unknown_building_tail_count(&self) -> usize {
        self.map
            .world
            .building_centers
            .iter()
            .filter(|center| matches!(center.building.parsed_tail, ParsedBuildingTail::Unknown))
            .count()
    }

    pub fn unknown_marker_model_count(&self) -> usize {
        self.markers
            .iter()
            .filter(|entry| matches!(entry.marker, MarkerModel::Unknown(_)))
            .count()
    }

    pub fn unknown_custom_chunk_count(&self) -> usize {
        self.custom_chunks
            .iter()
            .filter(|entry| matches!(entry.parsed, ParsedCustomChunk::Unknown))
            .count()
    }

    pub fn unknown_coverage_summary(&self) -> WorldLoadUnknownCoverageSummary {
        WorldLoadUnknownCoverageSummary {
            building_tail_unknown_count: self.unknown_building_tail_count(),
            marker_unknown_count: self.unknown_marker_model_count(),
            custom_chunk_unknown_count: self.unknown_custom_chunk_count(),
        }
    }

    pub fn static_fog_chunk(&self) -> Option<&StaticFogChunk> {
        self.custom_chunk("static-fog-data")
            .and_then(CustomChunkEntry::static_fog)
    }

    pub fn marker_region(&self) -> &[u8] {
        &self.marker_region_bytes
    }

    pub fn markers_are_empty(&self) -> bool {
        self.markers.is_empty() && marker_region_is_empty(&self.marker_region_bytes)
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

#[cfg(test)]
mod tests {
    use crate::{
        CustomChunkEntry, MarkerEntry, MarkerModel, ParsedCustomChunk, SaveEntityChunkObservation,
        SaveEntityPostLoadSummary, SaveEntityRemapSummary, SaveMapRegionObservation,
        SavePostLoadRuntimeWorldOwnership, SavePostLoadRuntimeWorldOwnershipStatus,
        SavePostLoadRuntimeWorldOwnershipSurface, SavePostLoadRuntimeWorldSurfaceKind,
        SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam, TeamPlanGroup,
        UnknownMarkerModel, WorldModel,
    };

    #[test]
    fn save_post_load_accessors_do_not_silently_shadow_duplicate_keys() {
        let observation = test_observation();

        assert!(observation.team_plan_group(7).is_none());
        assert!(observation.custom_chunk("static-fog-data").is_none());
        assert!(observation.world_entity_chunk(11).is_none());
        assert!(observation.marker(11).is_none());
        assert!(observation.static_fog_chunk().is_none());
    }

    #[test]
    fn world_entity_chunk_returns_none_for_duplicate_entity_ids() {
        let observation = test_observation();

        assert!(observation.world_entity_chunk(11).is_none());
    }

    #[test]
    fn world_entity_chunk_resolves_unique_entity_ids() {
        let observation = SavePostLoadWorldObservation {
            world_entity_chunks: vec![SaveEntityChunkObservation {
                chunk_len: 4,
                chunk_bytes: vec![1, 2, 3, 4],
                chunk_sha256: "unique-chunk".to_string(),
                class_id: 7,
                custom_name: None,
                entity_id: 42,
                body_len: 2,
                body_bytes: vec![5, 6],
                body_sha256: "unique-body".to_string(),
            }],
            ..test_observation()
        };

        let chunk = observation.world_entity_chunk(42).expect("unique entity chunk");
        assert_eq!(chunk.entity_id, 42);
        assert_eq!(chunk.class_id, 7);
        assert_eq!(chunk.chunk_bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn post_load_world_apply_bundle_aggregates_runtime_and_query_surfaces() {
        let observation = SavePostLoadWorldObservation {
            world_entity_chunks: vec![SaveEntityChunkObservation {
                chunk_len: 4,
                chunk_bytes: vec![1, 2, 3, 4],
                chunk_sha256: "unique-chunk".to_string(),
                class_id: 7,
                custom_name: None,
                entity_id: 42,
                body_len: 2,
                body_bytes: vec![5, 6],
                body_sha256: "unique-body".to_string(),
            }],
            markers: vec![MarkerEntry {
                id: 42,
                marker: MarkerModel::Unknown(UnknownMarkerModel {
                    class_tag: None,
                    world: true,
                    minimap: false,
                    autoscale: false,
                    draw_layer_bits: None,
                    x_bits: None,
                    y_bits: None,
                }),
            }],
            team_plan_groups: vec![TeamPlanGroup {
                team_id: 9,
                plan_count: 1,
                plans: Vec::new(),
            }],
            custom_chunks: vec![CustomChunkEntry {
                name: "static-fog-data".to_string(),
                chunk_len: 1,
                chunk_bytes: vec![1],
                chunk_sha256: "fog".to_string(),
                parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                    used_teams: 1,
                    width: 1,
                    height: 1,
                    teams: vec![StaticFogTeam {
                        team_id: 1,
                        run_count: 1,
                        rle_bytes: vec![1],
                        discovered: vec![true],
                    }],
                }),
            }],
            ..test_observation()
        };

        let bundle = observation.post_load_world_apply_bundle();

        assert_eq!(bundle.runtime_readiness, observation.runtime_readiness());
        assert_eq!(bundle.runtime_seed_surface, observation.runtime_seed_surface());
        assert_eq!(bundle.runtime_apply, observation.execute_runtime_apply());
        assert_eq!(
            bundle.runtime_world_semantics,
            observation.execute_runtime_world_semantics()
        );
        assert_eq!(
            bundle.runtime_world_ownership(),
            &observation.runtime_world_ownership()
        );
        assert!(bundle.graph().marker(42).is_some());
        assert!(bundle.team_plan_group(9).is_some());
        assert!(bundle.custom_chunk("static-fog-data").is_some());
        assert!(bundle.world_entity_chunk(42).is_some());
        assert!(bundle.marker(42).is_some());
        assert!(bundle.static_fog_chunk().is_some());
        assert_eq!(
            bundle.unknown_coverage_summary(),
            observation.unknown_coverage_summary()
        );
    }

    #[test]
    fn post_load_world_apply_bundle_reports_source_region_readiness() {
        let observation = test_observation();
        let bundle = observation.post_load_world_apply_bundle();
        let readiness_summary = bundle.runtime_readiness_summary();
        let runtime_readiness = observation.runtime_readiness();
        let runtime_seed_surface = observation.runtime_seed_surface();

        assert_eq!(readiness_summary.runtime_readiness, &runtime_readiness);
        assert_eq!(readiness_summary.runtime_seed_surface, &runtime_seed_surface);
        assert_eq!(
            readiness_summary.source_regions,
            runtime_readiness.source_regions()
        );
        assert_eq!(
            readiness_summary.source_region("entities").cloned(),
            runtime_readiness.source_region("entities")
        );
    }

    #[test]
    fn post_load_world_apply_bundle_reports_source_region_ownership() {
        let observation = test_observation();
        let bundle = observation.post_load_world_apply_bundle();
        let ownership_summary = bundle.runtime_world_ownership_summary();
        let runtime_world_ownership = observation.runtime_world_ownership();

        assert_eq!(
            ownership_summary.runtime_world_ownership,
            &runtime_world_ownership
        );
        assert_eq!(
            ownership_summary.source_regions,
            runtime_world_ownership.source_regions()
        );
        assert_eq!(
            ownership_summary.source_region("entities").cloned(),
            runtime_world_ownership.source_region("entities")
        );
    }

    #[test]
    fn post_load_world_apply_bundle_reports_combined_runtime_decision_summary() {
        let observation = test_observation();
        let bundle = observation.post_load_world_apply_bundle();
        let runtime_readiness = observation.runtime_readiness();
        let runtime_world_ownership = observation.runtime_world_ownership();
        let decision = bundle.runtime_decision_summary();

        assert_eq!(decision.runtime_readiness, &runtime_readiness);
        assert_eq!(decision.runtime_world_ownership, &runtime_world_ownership);
        assert_eq!(
            decision.can_seed_runtime_apply(),
            runtime_readiness.can_seed_runtime_apply
        );
        assert_eq!(
            decision.can_apply_world_semantics(),
            runtime_world_ownership.can_apply_world_semantics()
        );
        assert_eq!(
            decision.readiness_world_shell_ready(),
            runtime_readiness.world_shell_ready
        );
        assert_eq!(
            decision.ownership_world_shell_ready(),
            runtime_world_ownership.world_shell_ready
        );
        assert_eq!(
            decision.apply_now_step_count(),
            runtime_readiness.apply_now_step_count()
        );
        assert_eq!(
            decision.awaiting_world_shell_step_count(),
            runtime_readiness.awaiting_world_shell_step_count()
        );
        assert_eq!(decision.blocked_step_count(), runtime_readiness.blocked_step_count());
        assert_eq!(decision.deferred_step_count(), runtime_readiness.deferred_step_count());
        assert_eq!(
            decision.required_surface_count(),
            runtime_world_ownership.surfaces.len()
        );
        assert_eq!(
            decision.owned_surface_count(),
            runtime_world_ownership.owned_surface_count()
        );
        assert_eq!(
            decision.required_step_count(),
            runtime_world_ownership.required_step_count()
        );
        assert_eq!(
            decision.claimed_step_count(),
            runtime_world_ownership.claimed_step_count()
        );
        assert_eq!(
            decision.summary_label(),
            "seed=0 semantics=0 shell=0/0 apply=0 wait=0 block=5 defer=0 own=0/9 claim=0/5"
        );
        assert_eq!(
            decision.detail_label(),
            "seed=0 semantics=0 shell=0/0 apply=0 wait=0 block=5 defer=0 own=0/9 claim=0/5 readiness_sources=4 ownership_sources=4"
        );
    }

    #[test]
    fn save_post_load_runtime_world_ownership_summary_and_detail_labels_use_bool_labels() {
        let ownership = SavePostLoadRuntimeWorldOwnership {
            world_shell_ready: true,
            surfaces: vec![
                SavePostLoadRuntimeWorldOwnershipSurface {
                    kind: SavePostLoadRuntimeWorldSurfaceKind::WorldShell,
                    source_region_name: "test",
                    required_step_count: 3,
                    claimed_step_count: 3,
                    status: SavePostLoadRuntimeWorldOwnershipStatus::Owned,
                    blockers: Vec::new(),
                    failed_steps: Vec::new(),
                },
                SavePostLoadRuntimeWorldOwnershipSurface {
                    kind: SavePostLoadRuntimeWorldSurfaceKind::Markers,
                    source_region_name: "test",
                    required_step_count: 2,
                    claimed_step_count: 0,
                    status: SavePostLoadRuntimeWorldOwnershipStatus::Blocked,
                    blockers: Vec::new(),
                    failed_steps: Vec::new(),
                },
            ],
        };

        assert_eq!(
            ownership.summary_label(),
            "shell=yes semantics=no own=1/2 claim=3/5 wait=0 block=1 fail=0 defer=0 absent=0 regions=1"
        );
        assert_eq!(
            ownership.detail_label(),
            "shell=yes semantics=no own=1/2 claim=3/5 wait=0 block=1 fail=0 defer=0 absent=0 regions=[region=test own=1/2 claim=3/5 wait=0 block=1 fail=0 defer=0 absent=0]"
        );
    }

    fn test_observation() -> SavePostLoadWorldObservation {
        SavePostLoadWorldObservation {
            save_version: 0,
            content_header: Vec::new(),
            patches: Vec::new(),
            map: SaveMapRegionObservation {
                floor_runs: 0,
                floor_region_bytes: Vec::new(),
                block_runs: 0,
                block_region_bytes: Vec::new(),
                world: WorldModel {
                    width: 0,
                    height: 0,
                    floors: Vec::new(),
                    overlays: Vec::new(),
                    blocks: Vec::new(),
                    tiles: Vec::new(),
                    building_centers: Vec::new(),
                    data_tiles: 0,
                    team_count: 0,
                    total_plans: 0,
                    team_ids: Vec::new(),
                    team_plan_counts: Vec::new(),
                },
            },
            entity_remap_entries: Vec::new(),
            entity_remap_bytes: Vec::new(),
            entity_remap_summary: SaveEntityRemapSummary {
                remap_count: 0,
                unique_custom_ids: 0,
                duplicate_custom_ids: Vec::new(),
                unique_names: 0,
                duplicate_names: Vec::new(),
                effective_custom_ids: 0,
                resolved_builtin_custom_ids: Vec::new(),
                unresolved_effective_names: Vec::new(),
            },
            team_plan_groups: vec![
                TeamPlanGroup {
                    team_id: 7,
                    plan_count: 1,
                    plans: Vec::new(),
                },
                TeamPlanGroup {
                    team_id: 7,
                    plan_count: 1,
                    plans: Vec::new(),
                },
            ],
            team_region_bytes: Vec::new(),
            world_entity_count: 0,
            world_entity_bytes: Vec::new(),
            world_entity_chunks: vec![
                SaveEntityChunkObservation {
                    chunk_len: 4,
                    chunk_bytes: vec![1, 2, 3, 4],
                    chunk_sha256: "duplicate-a".to_string(),
                    class_id: 7,
                    custom_name: None,
                    entity_id: 11,
                    body_len: 2,
                    body_bytes: vec![5, 6],
                    body_sha256: "duplicate-body-a".to_string(),
                },
                SaveEntityChunkObservation {
                    chunk_len: 4,
                    chunk_bytes: vec![7, 8, 9, 10],
                    chunk_sha256: "duplicate-b".to_string(),
                    class_id: 8,
                    custom_name: None,
                    entity_id: 11,
                    body_len: 2,
                    body_bytes: vec![11, 12],
                    body_sha256: "duplicate-body-b".to_string(),
                },
            ],
            markers: vec![
                MarkerEntry {
                    id: 11,
                    marker: MarkerModel::Unknown(UnknownMarkerModel {
                        class_tag: None,
                        world: true,
                        minimap: false,
                        autoscale: false,
                        draw_layer_bits: None,
                        x_bits: None,
                        y_bits: None,
                    }),
                },
                MarkerEntry {
                    id: 11,
                    marker: MarkerModel::Unknown(UnknownMarkerModel {
                        class_tag: None,
                        world: true,
                        minimap: false,
                        autoscale: false,
                        draw_layer_bits: None,
                        x_bits: None,
                        y_bits: None,
                    }),
                },
            ],
            marker_region_bytes: Vec::new(),
            custom_chunks: vec![
                CustomChunkEntry {
                    name: "static-fog-data".to_string(),
                    chunk_len: 1,
                    chunk_bytes: vec![1],
                    chunk_sha256: "a".to_string(),
                    parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                        used_teams: 1,
                        width: 1,
                        height: 1,
                        teams: vec![StaticFogTeam {
                            team_id: 1,
                            run_count: 1,
                            rle_bytes: vec![1],
                            discovered: vec![true],
                        }],
                    }),
                },
                CustomChunkEntry {
                    name: "static-fog-data".to_string(),
                    chunk_len: 1,
                    chunk_bytes: vec![2],
                    chunk_sha256: "b".to_string(),
                    parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                        used_teams: 1,
                        width: 1,
                        height: 1,
                        teams: vec![StaticFogTeam {
                            team_id: 2,
                            run_count: 1,
                            rle_bytes: vec![2],
                            discovered: vec![false],
                        }],
                    }),
                },
            ],
            custom_region_bytes: Vec::new(),
            entity_summary: SaveEntityPostLoadSummary {
                total_entities: 0,
                unique_entity_ids: 0,
                duplicate_entity_ids: Vec::new(),
                builtin_entities: 0,
                custom_entities: 0,
                unknown_entities: 0,
                class_summaries: Vec::new(),
                loadable_entities: 0,
                skipped_entities: 0,
                post_load_class_summaries: Vec::new(),
            },
        }
    }
}
