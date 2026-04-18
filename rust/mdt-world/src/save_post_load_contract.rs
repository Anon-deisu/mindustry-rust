use std::collections::BTreeSet;

use crate::{
    marker_region_is_empty, MarkerModel, SaveEntityPostLoadSummary, SaveEntityRegionObservation,
    SavePostLoadWorldObservation, WorldLoadUnknownCoverageSummary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePostLoadWorldContract {
    pub has_world_graph: bool,
    pub tile_surface_consistent: bool,
    pub overlay_surface_consistent: bool,
    pub marker_surface_consistent: bool,
    pub static_fog_surface_consistent: bool,
    pub entity_surface_consistent: bool,
    pub unknown_coverage: WorldLoadUnknownCoverageSummary,
    pub issues: Vec<SavePostLoadWorldIssue>,
}

impl SavePostLoadWorldContract {
    pub fn can_project_world_shell(&self) -> bool {
        self.has_world_graph
            && self.tile_surface_consistent
            && self.overlay_surface_consistent
            && self.marker_surface_consistent
            && self.static_fog_surface_consistent
            && self.entity_surface_consistent
    }

    pub fn summary_label(&self) -> String {
        format!(
            "project={} graph={} tile={} overlay={} marker={} fog={} entity={} issues={}",
            bool_label(self.can_project_world_shell()),
            bool_label(self.has_world_graph),
            bool_label(self.tile_surface_consistent),
            bool_label(self.overlay_surface_consistent),
            bool_label(self.marker_surface_consistent),
            bool_label(self.static_fog_surface_consistent),
            bool_label(self.entity_surface_consistent),
            self.issues.len(),
        )
    }

    pub fn detail_label(&self) -> String {
        let issues = if self.issues.is_empty() {
            "none".to_string()
        } else {
            self.issues
                .iter()
                .copied()
                .map(SavePostLoadWorldIssue::label)
                .collect::<Vec<_>>()
                .join(",")
        };

        format!(
            "project={} graph={} tile={} overlay={} marker={} fog={} entity={} issues={}",
            bool_label(self.can_project_world_shell()),
            bool_label(self.has_world_graph),
            bool_label(self.tile_surface_consistent),
            bool_label(self.overlay_surface_consistent),
            bool_label(self.marker_surface_consistent),
            bool_label(self.static_fog_surface_consistent),
            bool_label(self.entity_surface_consistent),
            issues,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePostLoadWorldIssue {
    EmptyWorldGraph,
    TileSurfaceCountMismatch,
    TileSurfaceIndexMismatch,
    BuildingCenterReferenceMismatch,
    TeamPlanOverlayMismatch,
    TeamPlanOutOfBounds,
    DuplicateTeamPlanGroupIds,
    MarkerRegionMismatch,
    MarkerOutOfBounds,
    DuplicateMarkerIds,
    StaticFogDimensionMismatch,
    StaticFogCoverageMismatch,
    DuplicateStaticFogTeamIds,
    DuplicateCustomChunkNames,
    WorldEntityCountMismatch,
    DuplicateWorldEntityIds,
    EntitySummaryMismatch,
}

impl SavePostLoadWorldIssue {
    pub fn label(self) -> &'static str {
        match self {
            Self::EmptyWorldGraph => "empty-world-graph",
            Self::TileSurfaceCountMismatch => "tile-surface-count",
            Self::TileSurfaceIndexMismatch => "tile-surface-index",
            Self::BuildingCenterReferenceMismatch => "building-center-ref",
            Self::TeamPlanOverlayMismatch => "team-plan-overlay",
            Self::TeamPlanOutOfBounds => "team-plan-oob",
            Self::DuplicateTeamPlanGroupIds => "duplicate-team-plan-group-ids",
            Self::MarkerRegionMismatch => "marker-region",
            Self::MarkerOutOfBounds => "marker-oob",
            Self::DuplicateMarkerIds => "duplicate-marker-ids",
            Self::StaticFogDimensionMismatch => "static-fog-dimension",
            Self::StaticFogCoverageMismatch => "static-fog-coverage",
            Self::DuplicateStaticFogTeamIds => "duplicate-static-fog-team-ids",
            Self::DuplicateCustomChunkNames => "duplicate-custom-chunk-names",
            Self::WorldEntityCountMismatch => "world-entity-count",
            Self::DuplicateWorldEntityIds => "duplicate-world-entity-ids",
            Self::EntitySummaryMismatch => "entity-summary",
        }
    }
}

impl SavePostLoadWorldObservation {
    pub fn projection_contract(&self) -> SavePostLoadWorldContract {
        let mut issues = Vec::new();
        let has_world_graph = self.map.world.width > 0
            && self.map.world.height > 0
            && checked_tile_count(self.map.world.width, self.map.world.height)
                .map_or(false, |tile_count| tile_count > 0);
        if !has_world_graph {
            push_issue(&mut issues, SavePostLoadWorldIssue::EmptyWorldGraph);
        }

        let tile_surface_consistent = tile_surface_consistent(self, &mut issues);
        let overlay_surface_consistent = overlay_surface_consistent(self, &mut issues);
        let marker_surface_consistent = marker_surface_consistent(self, &mut issues);
        let static_fog_surface_consistent = static_fog_surface_consistent(self, &mut issues);
        let entity_surface_consistent = entity_surface_consistent(self, &mut issues);

        SavePostLoadWorldContract {
            has_world_graph,
            tile_surface_consistent,
            overlay_surface_consistent,
            marker_surface_consistent,
            static_fog_surface_consistent,
            entity_surface_consistent,
            unknown_coverage: self.unknown_coverage_summary(),
            issues,
        }
    }
}

fn tile_surface_consistent(
    observation: &SavePostLoadWorldObservation,
    issues: &mut Vec<SavePostLoadWorldIssue>,
) -> bool {
    let world = &observation.map.world;
    let tile_count = checked_tile_count(world.width, world.height);
    let mut consistent = true;

    if tile_count.map_or(true, |tile_count| {
        world.tiles.len() != tile_count
            || world.floors.len() != tile_count
            || world.overlays.len() != tile_count
            || world.blocks.len() != tile_count
    }) {
        push_issue(issues, SavePostLoadWorldIssue::TileSurfaceCountMismatch);
        consistent = false;
    }

    for (index, tile) in world.tiles.iter().enumerate() {
        let expected_index = checked_surface_index(tile.x, tile.y, world.width);
        let same_surface = world.floors.get(index) == Some(&tile.floor_id)
            && world.overlays.get(index) == Some(&tile.overlay_id)
            && world.blocks.get(index) == Some(&tile.block_id);
        if tile.tile_index != index
            || tile.x >= world.width
            || tile.y >= world.height
            || expected_index != Some(tile.tile_index)
            || !same_surface
        {
            push_issue(issues, SavePostLoadWorldIssue::TileSurfaceIndexMismatch);
            consistent = false;
            break;
        }
    }

    for (center_index, center) in world.building_centers.iter().enumerate() {
        let tile = world.tiles.get(center.tile_index);
        let expected_index = checked_surface_index(center.x, center.y, world.width);
        let center_ok = tile
            .map(|tile| tile.building_center_index == Some(center_index))
            .unwrap_or(false);
        if center.x >= world.width
            || center.y >= world.height
            || expected_index != Some(center.tile_index)
            || !center_ok
        {
            push_issue(
                issues,
                SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
            );
            consistent = false;
            break;
        }
    }

    for tile in &world.tiles {
        let center_ok = tile
            .building_center_index
            .and_then(|center_index| world.building_centers.get(center_index))
            .map(|center| center.tile_index == tile.tile_index)
            .unwrap_or_else(|| tile.building_center_index.is_none());
        if !center_ok {
            push_issue(
                issues,
                SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
            );
            consistent = false;
            break;
        }
    }

    consistent
}

fn overlay_surface_consistent(
    observation: &SavePostLoadWorldObservation,
    issues: &mut Vec<SavePostLoadWorldIssue>,
) -> bool {
    let world = &observation.map.world;
    let mut consistent = true;
    let group_ids: Vec<u32> = observation
        .team_plan_groups
        .iter()
        .map(|group| group.team_id)
        .collect();
    let group_counts: Vec<u32> = observation
        .team_plan_groups
        .iter()
        .map(|group| group.plan_count)
        .collect();
    let total_plans: usize = observation
        .team_plan_groups
        .iter()
        .map(|group| group.plans.len())
        .sum();

    if world.team_count != observation.team_plan_groups.len()
        || world.team_ids != group_ids
        || world.team_plan_counts != group_counts
        || world.total_plans != total_plans
        || observation
            .team_plan_groups
            .iter()
            .any(|group| group.plan_count != group.plans.len() as u32)
    {
        push_issue(issues, SavePostLoadWorldIssue::TeamPlanOverlayMismatch);
        consistent = false;
    }

    if observation.team_plan_groups.iter().any(|group| {
        group.plans.iter().any(|plan| {
            plan.x < 0
                || plan.y < 0
                || plan.x as usize >= world.width
                || plan.y as usize >= world.height
        })
    }) {
        push_issue(issues, SavePostLoadWorldIssue::TeamPlanOutOfBounds);
        consistent = false;
    }

    if has_duplicate_values(
        observation
            .team_plan_groups
            .iter()
            .map(|group| group.team_id),
    ) {
        push_issue(issues, SavePostLoadWorldIssue::DuplicateTeamPlanGroupIds);
        consistent = false;
    }

    consistent
}

fn marker_surface_consistent(
    observation: &SavePostLoadWorldObservation,
    issues: &mut Vec<SavePostLoadWorldIssue>,
) -> bool {
    let mut consistent = true;
    let width = observation.map.world.width;
    let height = observation.map.world.height;
    let empty_marker_region = marker_region_is_empty(&observation.marker_region_bytes);

    if observation.markers.is_empty() != empty_marker_region {
        push_issue(issues, SavePostLoadWorldIssue::MarkerRegionMismatch);
        consistent = false;
    }

    if observation
        .markers
        .iter()
        .any(|entry| !marker_in_bounds(&entry.marker, width, height))
    {
        push_issue(issues, SavePostLoadWorldIssue::MarkerOutOfBounds);
        consistent = false;
    }

    if has_duplicate_values(observation.markers.iter().map(|entry| entry.id)) {
        push_issue(issues, SavePostLoadWorldIssue::DuplicateMarkerIds);
    }

    consistent
}

fn marker_in_bounds(marker: &MarkerModel, width: usize, height: usize) -> bool {
    marker_tile_coords_in_bounds(marker.tile_coords(), width, height)
        && match marker {
            MarkerModel::Line(line) => {
                marker_tile_coords_in_bounds(line.end_tile_coords(), width, height)
            }
            _ => true,
        }
}

fn marker_tile_coords_in_bounds(coords: Option<(i16, i16)>, width: usize, height: usize) -> bool {
    match coords {
        Some((x, y)) => x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height,
        None => true,
    }
}

fn static_fog_surface_consistent(
    observation: &SavePostLoadWorldObservation,
    issues: &mut Vec<SavePostLoadWorldIssue>,
) -> bool {
    let mut consistent = true;
    if has_duplicate_values(
        observation
            .custom_chunks
            .iter()
            .map(|chunk| chunk.name.as_str()),
    ) {
        push_issue(issues, SavePostLoadWorldIssue::DuplicateCustomChunkNames);
    }
    let static_fog_chunks = observation
        .custom_chunks
        .iter()
        .filter(|chunk| chunk.name == "static-fog-data")
        .collect::<Vec<_>>();
    if !static_fog_chunks.is_empty()
        && static_fog_chunks
            .iter()
            .any(|chunk| chunk.static_fog().is_none())
    {
        push_issue(issues, SavePostLoadWorldIssue::StaticFogCoverageMismatch);
        consistent = false;
    }
    if let Some(chunk) = static_fog_chunks
        .iter()
        .find_map(|chunk| chunk.static_fog())
    {
        if chunk.width != observation.map.world.width
            || chunk.height != observation.map.world.height
        {
            push_issue(issues, SavePostLoadWorldIssue::StaticFogDimensionMismatch);
            consistent = false;
        }

        if chunk.used_teams != chunk.teams.len()
            || chunk
                .teams
                .iter()
                .any(|team| team.discovered.len() != observation.map.world.tile_count())
        {
            push_issue(issues, SavePostLoadWorldIssue::StaticFogCoverageMismatch);
            consistent = false;
        }

        if has_duplicate_values(chunk.teams.iter().map(|team| team.team_id)) {
            push_issue(issues, SavePostLoadWorldIssue::DuplicateStaticFogTeamIds);
            consistent = false;
        }
    }

    consistent
}

fn entity_surface_consistent(
    observation: &SavePostLoadWorldObservation,
    issues: &mut Vec<SavePostLoadWorldIssue>,
) -> bool {
    let chunks = &observation.world_entity_chunks;
    let actual_summary = recomputed_entity_summary(observation);
    let mut consistent = true;

    if observation.world_entity_count != chunks.len() {
        push_issue(issues, SavePostLoadWorldIssue::WorldEntityCountMismatch);
        consistent = false;
    }

    if !actual_summary.duplicate_entity_ids.is_empty() {
        push_issue(issues, SavePostLoadWorldIssue::DuplicateWorldEntityIds);
        consistent = false;
    }

    if observation.entity_summary != actual_summary {
        push_issue(issues, SavePostLoadWorldIssue::EntitySummaryMismatch);
        consistent = false;
    }

    consistent
}

fn recomputed_entity_summary(
    observation: &SavePostLoadWorldObservation,
) -> SaveEntityPostLoadSummary {
    SaveEntityRegionObservation {
        remap_count: observation.entity_remap_entries.len(),
        remap_entries: observation.entity_remap_entries.clone(),
        remap_bytes: observation.entity_remap_bytes.clone(),
        team_count: observation.team_plan_groups.len(),
        total_plans: observation
            .team_plan_groups
            .iter()
            .map(|group| group.plans.len())
            .sum(),
        team_plan_groups: observation.team_plan_groups.clone(),
        team_region_bytes: observation.team_region_bytes.clone(),
        world_entity_count: observation.world_entity_count,
        world_entity_bytes: observation.world_entity_bytes.clone(),
        entity_chunks: observation.world_entity_chunks.clone(),
    }
    .post_load_summary()
}

fn push_issue(issues: &mut Vec<SavePostLoadWorldIssue>, issue: SavePostLoadWorldIssue) {
    if !issues.contains(&issue) {
        issues.push(issue);
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn has_duplicate_values<T>(values: impl IntoIterator<Item = T>) -> bool
where
    T: Ord,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return true;
        }
    }
    false
}

fn checked_tile_count(width: usize, height: usize) -> Option<usize> {
    width.checked_mul(height)
}

fn checked_surface_index(x: usize, y: usize, width: usize) -> Option<usize> {
    y.checked_mul(width)?.checked_add(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingBaseSnapshot, BuildingCenter, BuildingSnapshot, CustomChunkEntry, LineMarkerModel,
        MarkerEntry, MarkerModel, ParsedBuildingTail, ParsedCustomChunk, PointMarkerModel,
        SaveEntityChunkObservation, SaveEntityClassSummary, SaveEntityPostLoadClassSummary,
        SaveEntityPostLoadKind, SaveEntityPostLoadSummary, SaveEntityRemapSummary,
        SaveMapRegionObservation, SavePostLoadWorldObservation, StaticFogChunk, StaticFogTeam,
        TeamPlan, TeamPlanGroup, TileModel, TypeIoValue, WorldLoadUnknownCoverageSummary,
        WorldModel,
    };

    #[test]
    fn projection_contract_accepts_consistent_post_load_world() {
        let observation = test_observation();
        let contract = observation.projection_contract();

        assert!(contract.can_project_world_shell());
        assert!(contract.has_world_graph);
        assert!(contract.tile_surface_consistent);
        assert!(contract.overlay_surface_consistent);
        assert!(contract.marker_surface_consistent);
        assert!(contract.static_fog_surface_consistent);
        assert!(contract.entity_surface_consistent);
        assert!(contract.issues.is_empty());
        assert_eq!(
            contract.unknown_coverage,
            WorldLoadUnknownCoverageSummary {
                building_tail_unknown_count: 0,
                marker_unknown_count: 0,
                custom_chunk_unknown_count: 0,
            }
        );
        assert_eq!(
            contract.summary_label(),
            "project=1 graph=1 tile=1 overlay=1 marker=1 fog=1 entity=1 issues=0"
        );
        assert_eq!(
            contract.detail_label(),
            "project=1 graph=1 tile=1 overlay=1 marker=1 fog=1 entity=1 issues=none"
        );
    }

    #[test]
    fn projection_contract_flags_overlay_and_fog_mismatches() {
        let mut observation = test_observation();
        observation.map.world.team_count = 2;
        let fog = observation
            .custom_chunks
            .iter_mut()
            .find(|chunk| chunk.name == "static-fog-data")
            .and_then(|chunk| match &mut chunk.parsed {
                ParsedCustomChunk::StaticFog(chunk) => Some(chunk),
                ParsedCustomChunk::Unknown => None,
            })
            .unwrap();
        fog.width = 3;
        fog.teams[0].discovered.pop();

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.overlay_surface_consistent);
        assert!(!contract.static_fog_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::TeamPlanOverlayMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::StaticFogDimensionMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::StaticFogCoverageMismatch));
        assert_eq!(
            contract.summary_label(),
            "project=0 graph=1 tile=1 overlay=0 marker=1 fog=0 entity=1 issues=3"
        );
        assert!(contract
            .detail_label()
            .contains("team-plan-overlay,static-fog-dimension,static-fog-coverage"));
    }

    #[test]
    fn projection_contract_flags_marker_and_entity_mismatches() {
        let mut observation = test_observation();
        observation.marker_region_bytes.clear();
        observation
            .world_entity_chunks
            .push(observation.world_entity_chunks[0].clone());

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.marker_surface_consistent);
        assert!(!contract.entity_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::MarkerRegionMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::WorldEntityCountMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::DuplicateWorldEntityIds));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::EntitySummaryMismatch));
    }

    #[test]
    fn projection_contract_flags_duplicate_team_plan_group_ids() {
        let mut observation = test_observation();
        observation
            .team_plan_groups
            .push(observation.team_plan_groups[0].clone());
        observation.map.world.team_count = 2;
        observation.map.world.total_plans = 2;
        observation.map.world.team_ids = vec![1, 1];
        observation.map.world.team_plan_counts = vec![1, 1];

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.overlay_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::DuplicateTeamPlanGroupIds));
    }

    #[test]
    fn projection_contract_flags_duplicate_marker_ids() {
        let mut observation = test_observation();
        observation.markers.push(observation.markers[0].clone());

        let contract = observation.projection_contract();

        assert!(contract.can_project_world_shell());
        assert!(contract.marker_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::DuplicateMarkerIds));
    }

    #[test]
    fn projection_contract_flags_line_marker_end_tile_out_of_bounds() {
        let mut observation = test_observation();
        observation.markers[0].marker = MarkerModel::Line(LineMarkerModel {
            class_tag: "lineMarker".to_string(),
            world: true,
            minimap: false,
            autoscale: false,
            draw_layer_bits: 120.0f32.to_bits(),
            x_bits: 8.0f32.to_bits(),
            y_bits: 8.0f32.to_bits(),
            end_x_bits: 40.0f32.to_bits(),
            end_y_bits: 56.0f32.to_bits(),
            stroke_bits: 1.0f32.to_bits(),
            outline: true,
            color1: Some("ffd37f".to_string()),
            color2: Some("ffd37f".to_string()),
        });

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.marker_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::MarkerOutOfBounds));
    }

    #[test]
    fn projection_contract_flags_duplicate_custom_chunk_names() {
        let mut observation = test_observation();
        observation
            .custom_chunks
            .push(observation.custom_chunks[0].clone());

        let contract = observation.projection_contract();

        assert!(contract.can_project_world_shell());
        assert!(contract.static_fog_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::DuplicateCustomChunkNames));
    }

    #[test]
    fn projection_contract_rejects_duplicate_static_fog_team_ids() {
        let mut observation = test_observation();
        let mut duplicate = observation.custom_chunks[0].clone();
        if let ParsedCustomChunk::StaticFog(chunk) = &mut duplicate.parsed {
            chunk.used_teams = 2;
            chunk.teams.push(StaticFogTeam {
                team_id: chunk.teams[0].team_id,
                run_count: chunk.teams[0].run_count,
                rle_bytes: chunk.teams[0].rle_bytes.clone(),
                discovered: chunk.teams[0].discovered.clone(),
            });
        }
        observation.custom_chunks[0] = duplicate;

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.static_fog_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::DuplicateStaticFogTeamIds));
    }

    #[test]
    fn projection_contract_flags_damaged_static_fog_chunks() {
        let mut observation = test_observation();
        observation.custom_chunks[0].parsed = ParsedCustomChunk::Unknown;

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.static_fog_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::StaticFogCoverageMismatch));
    }

    #[test]
    fn projection_contract_flags_mixed_duplicate_static_fog_chunks_with_damaged_tail() {
        let mut observation = test_observation();
        let mut duplicate = observation.custom_chunks[0].clone();
        duplicate.chunk_sha256 = "fog-damaged".to_string();
        duplicate.parsed = ParsedCustomChunk::Unknown;
        observation.custom_chunks.push(duplicate);

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.static_fog_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::DuplicateCustomChunkNames));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::StaticFogCoverageMismatch));
    }

    #[test]
    fn projection_contract_treats_brace_object_marker_region_as_empty() {
        let mut observation = test_observation();
        observation.markers.clear();
        observation.marker_region_bytes = b"{}".to_vec();

        let contract = observation.projection_contract();

        assert!(contract.can_project_world_shell());
        assert!(contract.marker_surface_consistent);
        assert!(!contract
            .issues
            .contains(&SavePostLoadWorldIssue::MarkerRegionMismatch));
    }

    #[test]
    fn projection_contract_flags_brace_object_marker_region_when_markers_exist() {
        let mut observation = test_observation();
        observation.marker_region_bytes = b"{}".to_vec();

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.marker_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::MarkerRegionMismatch));
    }

    #[test]
    fn projection_contract_flags_nonempty_marker_region_as_mismatch() {
        let mut observation = test_observation();
        observation.markers.clear();
        observation.marker_region_bytes = b"{markers}".to_vec();

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.marker_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::MarkerRegionMismatch));
    }

    #[test]
    fn marker_tile_coords_in_bounds_handles_none_and_edge_boundaries() {
        assert!(marker_tile_coords_in_bounds(None, 4, 3));
        assert!(marker_tile_coords_in_bounds(Some((0, 0)), 4, 3));
        assert!(marker_tile_coords_in_bounds(Some((3, 2)), 4, 3));
        assert!(!marker_tile_coords_in_bounds(Some((-1, 0)), 4, 3));
        assert!(!marker_tile_coords_in_bounds(Some((4, 0)), 4, 3));
        assert!(!marker_tile_coords_in_bounds(Some((0, 3)), 4, 3));
    }

    #[test]
    fn projection_contract_flags_post_load_entity_summary_drift() {
        let mut observation = test_observation();
        observation.entity_summary.loadable_entities = 1;
        observation.entity_summary.skipped_entities = 0;
        observation.entity_summary.post_load_class_summaries.clear();

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.entity_surface_consistent);
        assert_eq!(
            contract.issues,
            vec![SavePostLoadWorldIssue::EntitySummaryMismatch]
        );
    }

    #[test]
    fn bool_label_formats_true_and_false() {
        assert_eq!(bool_label(true), "1");
        assert_eq!(bool_label(false), "0");
    }

    #[test]
    fn checked_tile_count_and_surface_index_handle_overflow_boundaries() {
        assert_eq!(checked_tile_count(3, 4), Some(12));
        assert_eq!(checked_tile_count(usize::MAX, 2), None);

        assert_eq!(checked_surface_index(2, 3, 4), Some(14));
        assert_eq!(checked_surface_index(0, 2, usize::MAX), None);
        assert_eq!(checked_surface_index(1, 1, usize::MAX), None);
    }

    #[test]
    fn save_post_load_world_contract_can_project_world_shell_requires_all_surface_flags() {
        let contract = SavePostLoadWorldContract {
            has_world_graph: true,
            tile_surface_consistent: true,
            overlay_surface_consistent: true,
            marker_surface_consistent: true,
            static_fog_surface_consistent: true,
            entity_surface_consistent: true,
            unknown_coverage: WorldLoadUnknownCoverageSummary {
                building_tail_unknown_count: 0,
                marker_unknown_count: 0,
                custom_chunk_unknown_count: 0,
            },
            issues: Vec::new(),
        };

        assert!(contract.can_project_world_shell());

        let mut broken_contract = contract.clone();
        broken_contract.marker_surface_consistent = false;

        assert!(!broken_contract.can_project_world_shell());
    }

    #[test]
    fn save_post_load_world_issue_labels_cover_all_variants() {
        let cases = [
            (
                SavePostLoadWorldIssue::EmptyWorldGraph,
                "empty-world-graph",
            ),
            (
                SavePostLoadWorldIssue::TileSurfaceCountMismatch,
                "tile-surface-count",
            ),
            (
                SavePostLoadWorldIssue::TileSurfaceIndexMismatch,
                "tile-surface-index",
            ),
            (
                SavePostLoadWorldIssue::BuildingCenterReferenceMismatch,
                "building-center-ref",
            ),
            (
                SavePostLoadWorldIssue::TeamPlanOverlayMismatch,
                "team-plan-overlay",
            ),
            (SavePostLoadWorldIssue::TeamPlanOutOfBounds, "team-plan-oob"),
            (
                SavePostLoadWorldIssue::DuplicateTeamPlanGroupIds,
                "duplicate-team-plan-group-ids",
            ),
            (SavePostLoadWorldIssue::MarkerRegionMismatch, "marker-region"),
            (SavePostLoadWorldIssue::MarkerOutOfBounds, "marker-oob"),
            (
                SavePostLoadWorldIssue::DuplicateMarkerIds,
                "duplicate-marker-ids",
            ),
            (
                SavePostLoadWorldIssue::StaticFogDimensionMismatch,
                "static-fog-dimension",
            ),
            (
                SavePostLoadWorldIssue::StaticFogCoverageMismatch,
                "static-fog-coverage",
            ),
            (
                SavePostLoadWorldIssue::DuplicateStaticFogTeamIds,
                "duplicate-static-fog-team-ids",
            ),
            (
                SavePostLoadWorldIssue::DuplicateCustomChunkNames,
                "duplicate-custom-chunk-names",
            ),
            (
                SavePostLoadWorldIssue::WorldEntityCountMismatch,
                "world-entity-count",
            ),
            (
                SavePostLoadWorldIssue::DuplicateWorldEntityIds,
                "duplicate-world-entity-ids",
            ),
            (
                SavePostLoadWorldIssue::EntitySummaryMismatch,
                "entity-summary",
            ),
        ];

        for (issue, expected_label) in cases {
            assert_eq!(issue.label(), expected_label);
        }
    }

    #[test]
    fn save_post_load_world_contract_summary_and_detail_labels_are_stable() {
        let contract = SavePostLoadWorldContract {
            has_world_graph: true,
            tile_surface_consistent: false,
            overlay_surface_consistent: true,
            marker_surface_consistent: false,
            static_fog_surface_consistent: true,
            entity_surface_consistent: false,
            unknown_coverage: crate::WorldLoadUnknownCoverageSummary {
                building_tail_unknown_count: 1,
                marker_unknown_count: 2,
                custom_chunk_unknown_count: 3,
            },
            issues: vec![
                SavePostLoadWorldIssue::TileSurfaceIndexMismatch,
                SavePostLoadWorldIssue::EntitySummaryMismatch,
            ],
        };

        assert_eq!(
            contract.summary_label(),
            "project=0 graph=1 tile=0 overlay=1 marker=0 fog=1 entity=0 issues=2"
        );
        assert_eq!(
            contract.detail_label(),
            "project=0 graph=1 tile=0 overlay=1 marker=0 fog=1 entity=0 issues=tile-surface-index,entity-summary"
        );
    }

    #[test]
    fn projection_contract_flags_tile_surface_breakage() {
        let mut observation = test_observation();
        observation.map.world.tiles[0].building_center_index = None;
        observation.map.world.blocks.pop();

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.tile_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::TileSurfaceCountMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::BuildingCenterReferenceMismatch));
    }

    #[test]
    fn projection_contract_does_not_panic_on_overflowing_tile_coordinates() {
        let mut observation = test_observation();
        observation.map.world.width = usize::MAX;
        observation.map.world.height = 2;
        observation.map.world.tiles[0].x = usize::MAX - 1;
        observation.map.world.tiles[0].y = 1;
        observation.map.world.building_centers[0].x = usize::MAX - 1;
        observation.map.world.building_centers[0].y = 1;
        observation.custom_chunks.clear();

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::TileSurfaceCountMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::TileSurfaceIndexMismatch));
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::BuildingCenterReferenceMismatch));
    }

    #[test]
    fn projection_contract_flags_stale_building_center_index_on_unreferenced_tile() {
        let mut observation = test_observation();
        observation.map.world.tiles[1].building_center_index = Some(0);

        let contract = observation.projection_contract();

        assert!(!contract.can_project_world_shell());
        assert!(!contract.tile_surface_consistent);
        assert!(contract
            .issues
            .contains(&SavePostLoadWorldIssue::BuildingCenterReferenceMismatch));
    }

    fn test_observation() -> SavePostLoadWorldObservation {
        SavePostLoadWorldObservation {
            save_version: 11,
            content_header: Vec::new(),
            patches: Vec::new(),
            map: SaveMapRegionObservation {
                floor_runs: 1,
                floor_region_bytes: vec![1],
                block_runs: 1,
                block_region_bytes: vec![2],
                world: test_world(),
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
            team_plan_groups: vec![TeamPlanGroup {
                team_id: 1,
                plan_count: 1,
                plans: vec![TeamPlan {
                    x: 1,
                    y: 1,
                    rotation: 0,
                    block_id: 0x0101,
                    config: TypeIoValue::Null,
                    config_bytes: Vec::new(),
                    config_sha256: "plan".to_string(),
                }],
            }],
            team_region_bytes: vec![3],
            world_entity_count: 1,
            world_entity_bytes: vec![4],
            world_entity_chunks: vec![SaveEntityChunkObservation {
                chunk_len: 3,
                chunk_bytes: vec![4, 5, 6],
                chunk_sha256: "chunk".to_string(),
                class_id: 255,
                custom_name: Some("test-entity".to_string()),
                entity_id: 42,
                body_len: 2,
                body_bytes: vec![5, 6],
                body_sha256: "entity".to_string(),
            }],
            markers: vec![MarkerEntry {
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
            }],
            marker_region_bytes: b"{markers}".to_vec(),
            custom_chunks: vec![CustomChunkEntry {
                name: "static-fog-data".to_string(),
                chunk_len: 1,
                chunk_bytes: vec![7],
                chunk_sha256: "fog".to_string(),
                parsed: ParsedCustomChunk::StaticFog(StaticFogChunk {
                    used_teams: 1,
                    width: 2,
                    height: 2,
                    teams: vec![StaticFogTeam {
                        team_id: 1,
                        run_count: 1,
                        rle_bytes: vec![8],
                        discovered: vec![true, false, true, true],
                    }],
                }),
            }],
            custom_region_bytes: vec![9],
            entity_summary: SaveEntityPostLoadSummary {
                total_entities: 1,
                unique_entity_ids: 1,
                duplicate_entity_ids: Vec::new(),
                builtin_entities: 0,
                custom_entities: 1,
                unknown_entities: 0,
                class_summaries: vec![SaveEntityClassSummary {
                    class_id: 255,
                    kind: crate::SaveEntityClassKind::Custom,
                    resolved_name: "test-entity".to_string(),
                    count: 1,
                }],
                loadable_entities: 0,
                skipped_entities: 1,
                post_load_class_summaries: vec![SaveEntityPostLoadClassSummary {
                    source_class_ids: vec![255],
                    effective_class_id: None,
                    kind: SaveEntityPostLoadKind::UnresolvedCustom,
                    resolved_name: "unresolved:test-entity".to_string(),
                    count: 1,
                }],
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
                chunk_len: 0,
                chunk_bytes: Vec::new(),
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
            team_count: 1,
            total_plans: 1,
            team_ids: vec![1],
            team_plan_counts: vec![1],
        }
    }
}
