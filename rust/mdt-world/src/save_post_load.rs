use crate::{
    marker_region_is_empty, CustomChunkEntry, MarkerEntry, MarkerModel, ParsedBuildingTail,
    ParsedCustomChunk, SavePostLoadWorldObservation, StaticFogChunk, TeamPlan, TeamPlanGroup,
    WorldGraph, WorldLoadUnknownCoverageSummary,
};

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
        self.team_plan_groups
            .iter()
            .find(|group| group.team_id == team_id)
    }

    pub fn all_team_plans(&self) -> impl Iterator<Item = &TeamPlan> {
        self.team_plan_groups
            .iter()
            .flat_map(|group| group.plans.iter())
    }

    pub fn custom_chunk(&self, name: &str) -> Option<&CustomChunkEntry> {
        self.custom_chunks.iter().find(|chunk| chunk.name == name)
    }

    pub fn marker(&self, id: i32) -> Option<&MarkerEntry> {
        self.markers.iter().find(|marker| marker.id == id)
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
