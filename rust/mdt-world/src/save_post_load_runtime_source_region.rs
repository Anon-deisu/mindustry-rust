use crate::{
    SavePostLoadConsumerStageKind, SavePostLoadRuntimeApplyStep,
    SavePostLoadRuntimeWorldSurfaceKind,
};

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

pub(crate) fn source_region_name_for_step(step: &SavePostLoadRuntimeApplyStep) -> &'static str {
    SavePostLoadRuntimeWorldSurfaceKind::from_step(step)
        .map(|kind| kind.source_region_name())
        .expect("every runtime apply step must map to a world surface kind")
}

pub(crate) fn source_region_sort_key(source_region_name: &str) -> u8 {
    match source_region_name {
        "map" => 0,
        "entities" => 1,
        "markers" => 2,
        "custom" => 3,
        _ => 4,
    }
}

pub(crate) fn find_source_region<T>(
    source_regions: impl IntoIterator<Item = T>,
    source_region_name: &str,
    source_region_name_of: impl Fn(&T) -> &str,
) -> Option<T> {
    source_regions
        .into_iter()
        .find(|region| source_region_name_of(region) == source_region_name)
}

pub(crate) fn find_or_push_source_region<'a, T>(
    source_regions: &'a mut Vec<T>,
    source_region_name: &str,
    source_region_name_of: impl Fn(&T) -> &str,
    create_source_region: impl FnOnce() -> T,
) -> &'a mut T {
    if let Some(index) = source_regions
        .iter()
        .position(|region| source_region_name_of(region) == source_region_name)
    {
        return &mut source_regions[index];
    }

    source_regions.push(create_source_region());
    source_regions
        .last_mut()
        .expect("source region was just pushed")
}

#[cfg(test)]
mod tests {
    use super::{
        find_or_push_source_region, find_source_region, source_region_name_for_stage_kind,
        source_region_name_for_step, source_region_sort_key,
    };
    use crate::{
        SavePostLoadConsumerStageKind, SavePostLoadRuntimeApplyStep, SavePostLoadRuntimeRegionKind,
        SavePostLoadRuntimeWorldSurfaceKind,
    };

    const STAGE_SOURCE_REGION_CASES: &[(SavePostLoadConsumerStageKind, &str)] = &[
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

    const SOURCE_REGION_SORT_ORDER: &[&str] = &["map", "entities", "markers", "custom", "unknown"];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestRuntimeRegionSurfaceCase {
        region_kind: SavePostLoadRuntimeRegionKind,
        surface_kind: SavePostLoadRuntimeWorldSurfaceKind,
        expected_region_name: &'static str,
    }

    const RUNTIME_REGION_SURFACE_CASES: &[TestRuntimeRegionSurfaceCase] = &[
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::WorldShell,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::WorldShell,
            expected_region_name: "map",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::EntityRemaps,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::EntityRemaps,
            expected_region_name: "entities",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::TeamPlans,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::TeamPlans,
            expected_region_name: "entities",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::Markers,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::Markers,
            expected_region_name: "markers",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::StaticFog,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::StaticFog,
            expected_region_name: "custom",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::CustomChunks,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::CustomChunks,
            expected_region_name: "custom",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::Buildings,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::Buildings,
            expected_region_name: "map",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::LoadableEntities,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::LoadableEntities,
            expected_region_name: "entities",
        },
        TestRuntimeRegionSurfaceCase {
            region_kind: SavePostLoadRuntimeRegionKind::SkippedEntities,
            surface_kind: SavePostLoadRuntimeWorldSurfaceKind::SkippedEntities,
            expected_region_name: "entities",
        },
    ];

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestStepSourceRegionCase {
        step: SavePostLoadRuntimeApplyStep,
        expected_region_name: &'static str,
    }

    fn step_source_region_cases() -> Vec<TestStepSourceRegionCase> {
        vec![
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::WorldShell,
                expected_region_name: "map",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::EntityRemap { remap_index: 0 },
                expected_region_name: "entities",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::TeamPlan {
                    group_index: 0,
                    plan_index: 0,
                },
                expected_region_name: "entities",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::Marker { marker_index: 0 },
                expected_region_name: "markers",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::StaticFog,
                expected_region_name: "custom",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::CustomChunk { chunk_index: 0 },
                expected_region_name: "custom",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::Building { center_index: 0 },
                expected_region_name: "map",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::LoadableEntity { entity_index: 0 },
                expected_region_name: "entities",
            },
            TestStepSourceRegionCase {
                step: SavePostLoadRuntimeApplyStep::SkippedEntity { entity_index: 0 },
                expected_region_name: "entities",
            },
        ]
    }

    #[test]
    fn source_region_name_covers_every_stage_kind_bucket() {
        for &(kind, expected) in STAGE_SOURCE_REGION_CASES {
            assert_eq!(source_region_name_for_stage_kind(kind), expected);
        }
    }

    #[test]
    fn runtime_region_and_surface_names_match_stage_buckets_exhaustively() {
        for case in RUNTIME_REGION_SURFACE_CASES {
            assert_eq!(case.region_kind.source_region_name(), case.expected_region_name);
            assert_eq!(case.surface_kind.source_region_name(), case.expected_region_name);
            assert_eq!(
                case.region_kind.source_region_name(),
                case.surface_kind.source_region_name()
            );
        }
    }

    #[test]
    fn source_region_name_for_step_matches_world_surface_mapping_exhaustively() {
        for case in step_source_region_cases() {
            assert_eq!(source_region_name_for_step(&case.step), case.expected_region_name);
        }
    }

    #[test]
    fn source_region_sort_key_matches_expected_bucket_order() {
        assert_source_region_sort_order(SOURCE_REGION_SORT_ORDER);
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestSourceRegion {
        source_region_name: &'static str,
        value: usize,
    }

    fn test_source_region(source_region_name: &'static str, value: usize) -> TestSourceRegion {
        TestSourceRegion {
            source_region_name,
            value,
        }
    }

    fn test_source_regions() -> Vec<TestSourceRegion> {
        vec![
            test_source_region("map", 1),
            test_source_region("entities", 2),
        ]
    }

    fn assert_source_region_sort_order(ordered: &[&str]) {
        let expected = ordered
            .iter()
            .map(|region| source_region_sort_key(region))
            .collect::<Vec<_>>();
        let mut sorted = expected.clone();
        sorted.sort_unstable();

        assert_eq!(expected, sorted);
    }

    fn assert_borrowed_source_region_lookup(
        source_regions: &[TestSourceRegion],
        source_region_name: &str,
        expected: Option<&TestSourceRegion>,
    ) {
        assert_eq!(
            find_source_region(source_regions.iter(), source_region_name, |region| {
                region.source_region_name
            }),
            expected
        );
    }

    #[test]
    fn find_source_region_supports_borrowed_regions() {
        let source_regions = test_source_regions();

        assert_borrowed_source_region_lookup(&source_regions, "entities", Some(&source_regions[1]));
        assert_borrowed_source_region_lookup(&source_regions, "missing", None);
    }

    #[test]
    fn find_source_region_supports_owned_regions() {
        let source_regions = test_source_regions();

        assert_eq!(
            find_source_region(source_regions, "map", |region| region.source_region_name),
            Some(test_source_region("map", 1))
        );
    }

    #[test]
    fn find_or_push_source_region_reuses_existing_region() {
        let mut source_regions = vec![test_source_region("map", 1)];

        let source_region = find_or_push_source_region(
            &mut source_regions,
            "map",
            |region| region.source_region_name,
            || test_source_region("map", 99),
        );

        source_region.value += 1;

        assert_eq!(source_regions, vec![test_source_region("map", 2)]);
    }

    #[test]
    fn find_or_push_source_region_pushes_missing_region() {
        let mut source_regions = vec![test_source_region("map", 1)];

        find_or_push_source_region(
            &mut source_regions,
            "entities",
            |region| region.source_region_name,
            || test_source_region("entities", 2),
        );

        assert_eq!(
            source_regions,
            vec![test_source_region("map", 1), test_source_region("entities", 2)]
        );
    }
}
