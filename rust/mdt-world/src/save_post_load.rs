use crate::{
    marker_region_is_empty, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
    ParsedCustomChunk, SavePostLoadWorldObservation, StaticFogChunk, TeamPlan, TeamPlanGroup,
    WorldGraph, WorldLoadUnknownCoverageSummary,
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

impl SavePostLoadWorldObservation {
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

#[cfg(test)]
mod tests {
    use crate::{
        CustomChunkEntry, MarkerEntry, MarkerModel, ParsedCustomChunk, SaveEntityChunkObservation,
        SaveEntityPostLoadSummary, SaveEntityRemapSummary, SaveMapRegionObservation,
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
