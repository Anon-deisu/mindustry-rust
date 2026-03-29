use crate::SavePostLoadConsumerStageKind;

pub(crate) const fn source_region_name_for_stage_kind(
    kind: SavePostLoadConsumerStageKind,
) -> &'static str {
    match kind {
        SavePostLoadConsumerStageKind::WorldShell | SavePostLoadConsumerStageKind::Buildings => {
            "map"
        }
        SavePostLoadConsumerStageKind::EntityRemaps
        | SavePostLoadConsumerStageKind::TeamPlans
        | SavePostLoadConsumerStageKind::LoadableEntities
        | SavePostLoadConsumerStageKind::SkippedEntities => "entities",
        SavePostLoadConsumerStageKind::Markers => "markers",
        SavePostLoadConsumerStageKind::StaticFog | SavePostLoadConsumerStageKind::CustomChunks => {
            "custom"
        }
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
    fn source_region_name_covers_every_stage_kind_bucket() {
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
    fn runtime_region_and_surface_names_match_stage_buckets_exhaustively() {
        let region_cases = [
            (
                SavePostLoadRuntimeRegionKind::WorldShell,
                SavePostLoadRuntimeWorldSurfaceKind::WorldShell,
                "map",
            ),
            (
                SavePostLoadRuntimeRegionKind::EntityRemaps,
                SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps,
                "entities",
            ),
            (
                SavePostLoadRuntimeRegionKind::TeamPlans,
                SavePostLoadRuntimeWorldSurfaceKind::TeamPlans,
                "entities",
            ),
            (
                SavePostLoadRuntimeRegionKind::Markers,
                SavePostLoadRuntimeWorldSurfaceKind::Markers,
                "markers",
            ),
            (
                SavePostLoadRuntimeRegionKind::StaticFog,
                SavePostLoadRuntimeWorldSurfaceKind::StaticFog,
                "custom",
            ),
            (
                SavePostLoadRuntimeRegionKind::CustomChunks,
                SavePostLoadRuntimeWorldSurfaceKind::CustomChunks,
                "custom",
            ),
            (
                SavePostLoadRuntimeRegionKind::Buildings,
                SavePostLoadRuntimeWorldSurfaceKind::Buildings,
                "map",
            ),
            (
                SavePostLoadRuntimeRegionKind::LoadableEntities,
                SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
                "entities",
            ),
            (
                SavePostLoadRuntimeRegionKind::SkippedEntities,
                SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities,
                "entities",
            ),
        ];

        for (region_kind, surface_kind, expected) in region_cases {
            assert_eq!(region_kind.source_region_name(), expected);
            assert_eq!(surface_kind.source_region_name(), expected);
            assert_eq!(region_kind.source_region_name(), surface_kind.source_region_name());
        }
    }
}
