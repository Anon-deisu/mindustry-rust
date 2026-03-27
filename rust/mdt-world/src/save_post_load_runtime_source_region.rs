use crate::SavePostLoadConsumerStageKind;

pub(crate) const fn source_region_name_for_stage_kind(
    kind: SavePostLoadConsumerStageKind,
) -> &'static str {
    match kind {
        SavePostLoadConsumerStageKind::WorldShell
        | SavePostLoadConsumerStageKind::Buildings => "map",
        SavePostLoadConsumerStageKind::EntityRemaps
        | SavePostLoadConsumerStageKind::TeamPlans
        | SavePostLoadConsumerStageKind::LoadableEntities
        | SavePostLoadConsumerStageKind::SkippedEntities => "entities",
        SavePostLoadConsumerStageKind::Markers => "markers",
        SavePostLoadConsumerStageKind::StaticFog
        | SavePostLoadConsumerStageKind::CustomChunks => "custom",
    }
}

#[cfg(test)]
mod tests {
    use super::source_region_name_for_stage_kind;
    use crate::{
        SavePostLoadConsumerStageKind, SavePostLoadRuntimeRegionKind,
        SavePostLoadRuntimeWorldSurfaceKind,
    };

    #[test]
    fn source_region_name_groups_follow_expected_buckets() {
        let cases = [
            (SavePostLoadConsumerStageKind::WorldShell, "map"),
            (SavePostLoadConsumerStageKind::Buildings, "map"),
            (SavePostLoadConsumerStageKind::EntityRemaps, "entities"),
            (SavePostLoadConsumerStageKind::TeamPlans, "entities"),
            (SavePostLoadConsumerStageKind::LoadableEntities, "entities"),
            (SavePostLoadConsumerStageKind::SkippedEntities, "entities"),
            (SavePostLoadConsumerStageKind::Markers, "markers"),
            (SavePostLoadConsumerStageKind::StaticFog, "custom"),
            (SavePostLoadConsumerStageKind::CustomChunks, "custom"),
        ];

        for (kind, expected) in cases {
            assert_eq!(source_region_name_for_stage_kind(kind), expected);
        }
    }

    #[test]
    fn readiness_and_world_ownership_share_source_region_names() {
        let cases = [
            (
                SavePostLoadRuntimeRegionKind::WorldShell,
                SavePostLoadRuntimeWorldSurfaceKind::WorldShell,
            ),
            (
                SavePostLoadRuntimeRegionKind::TeamPlans,
                SavePostLoadRuntimeWorldSurfaceKind::TeamPlans,
            ),
            (
                SavePostLoadRuntimeRegionKind::Markers,
                SavePostLoadRuntimeWorldSurfaceKind::Markers,
            ),
            (
                SavePostLoadRuntimeRegionKind::StaticFog,
                SavePostLoadRuntimeWorldSurfaceKind::StaticFog,
            ),
            (
                SavePostLoadRuntimeRegionKind::Buildings,
                SavePostLoadRuntimeWorldSurfaceKind::Buildings,
            ),
            (
                SavePostLoadRuntimeRegionKind::LoadableEntities,
                SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
            ),
        ];

        for (region_kind, surface_kind) in cases {
            assert_eq!(
                region_kind.source_region_name(),
                surface_kind.source_region_name()
            );
        }
    }
}
