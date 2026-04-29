//! Offline build-plan edit primitives.
//! Mirrors the minimum behavior of Java `InputHandler.rotatePlans/flipPlans`.

/// Tile size used by world-space conversion in Mindustry.
pub const TILE_SIZE: f32 = 8.0;

/// Relative point used by plan config transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanPoint {
    pub x: i32,
    pub y: i32,
}

/// Minimum point-config variants needed by plan rotate/flip transforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanPointConfig {
    None,
    Point(PlanPoint),
    Points(Vec<PlanPoint>),
}

impl PlanPointConfig {
    pub fn map_points<F>(&mut self, mut mapper: F)
    where
        F: FnMut(&mut PlanPoint),
    {
        match self {
            Self::None => {}
            Self::Point(point) => mapper(point),
            Self::Points(points) => {
                for point in points {
                    mapper(point);
                }
            }
        }
    }
}

/// High-signal point-config family used by plan summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPointConfigFamily {
    None,
    Point,
    Points,
}

impl PlanPointConfigFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Point => "point",
            Self::Points => "points",
        }
    }
}

impl PlanPointConfig {
    pub fn family(&self) -> PlanPointConfigFamily {
        match self {
            Self::None => PlanPointConfigFamily::None,
            Self::Point(_) => PlanPointConfigFamily::Point,
            Self::Points(_) => PlanPointConfigFamily::Points,
        }
    }
}

/// Block metadata needed for plan transforms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanBlockMeta {
    /// Multiblock size in tiles.
    pub size: i32,
    /// World-space draw/placement offset.
    pub offset: f32,
    /// Equivalent to Java `Block.rotate`.
    pub rotate: bool,
    /// Equivalent to Java `Block.lockRotation`.
    pub lock_rotation: bool,
    /// Equivalent to Java `Block.invertFlip`.
    pub invert_flip: bool,
}

impl PlanBlockMeta {
    pub fn with_size(size: i32) -> Self {
        assert!(size > 0, "plan block size must be positive");
        Self {
            size,
            offset: block_offset(size),
            rotate: true,
            lock_rotation: true,
            invert_flip: false,
        }
    }

    pub fn plan_rotation(self, rotation: i32) -> i32 {
        if !self.rotate && self.lock_rotation {
            0
        } else {
            rotation
        }
    }

    pub fn flip_rotation(self, rotation: i32, flip_x: bool) -> i32 {
        let even_rotation = rotation.rem_euclid(2) == 0;
        if (flip_x == even_rotation) != self.invert_flip {
            self.plan_rotation((rotation + 2).rem_euclid(4))
        } else {
            rotation
        }
    }
}

impl Default for PlanBlockMeta {
    fn default() -> Self {
        Self::with_size(1)
    }
}

/// Tile-space bounds for a plan collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl PlanBounds {
    pub fn width(self) -> i32 {
        self.max_x - self.min_x + 1
    }

    pub fn height(self) -> i32 {
        self.max_y - self.min_y + 1
    }

    pub fn label(self) -> String {
        format!(
            "{}:{}..{}:{}",
            self.min_x, self.min_y, self.max_x, self.max_y
        )
    }
}

/// Read-only summary for a plan collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanCollectionSummary {
    pub plan_count: usize,
    pub breaking_count: usize,
    pub bounds: Option<PlanBounds>,
    pub rotation_counts: [usize; 4],
    pub point_config_family_counts: [usize; 3],
    pub point_config_point_count: usize,
}

impl PlanCollectionSummary {
    pub fn from_plans<P: PlanEditable>(plans: &[P]) -> Self {
        let mut breaking_count = 0usize;
        let mut rotation_counts = [0usize; 4];
        let mut point_config_family_counts = [0usize; 3];
        let mut point_config_point_count = 0usize;
        let mut bounds: Option<PlanBounds> = None;

        for plan in plans {
            if plan.is_breaking() {
                breaking_count = breaking_count.saturating_add(1);
            }

            let rotation = plan.rotation().rem_euclid(4) as usize;
            rotation_counts[rotation] = rotation_counts[rotation].saturating_add(1);

            let family = plan.point_config_family();
            point_config_family_counts[family as usize] =
                point_config_family_counts[family as usize].saturating_add(1);
            point_config_point_count =
                point_config_point_count.saturating_add(plan.point_config_point_count());

            let (x, y) = plan.tile();
            bounds = Some(match bounds {
                Some(bounds) => PlanBounds {
                    min_x: bounds.min_x.min(x),
                    min_y: bounds.min_y.min(y),
                    max_x: bounds.max_x.max(x),
                    max_y: bounds.max_y.max(y),
                },
                None => PlanBounds {
                    min_x: x,
                    min_y: y,
                    max_x: x,
                    max_y: y,
                },
            });
        }

        Self {
            plan_count: plans.len(),
            breaking_count,
            bounds,
            rotation_counts,
            point_config_family_counts,
            point_config_point_count,
        }
    }

    pub fn has_bounds(&self) -> bool {
        self.bounds.is_some()
    }

    pub fn bounds_label(&self) -> String {
        self.bounds
            .map_or_else(|| "none".to_string(), PlanBounds::label)
    }

    pub fn rotation_label(&self) -> String {
        format!(
            "r0={} r1={} r2={} r3={}",
            self.rotation_counts[0],
            self.rotation_counts[1],
            self.rotation_counts[2],
            self.rotation_counts[3]
        )
    }

    pub fn point_config_label(&self) -> String {
        format!(
            "none={} point={} points={} total={}",
            self.point_config_family_counts[PlanPointConfigFamily::None as usize],
            self.point_config_family_counts[PlanPointConfigFamily::Point as usize],
            self.point_config_family_counts[PlanPointConfigFamily::Points as usize],
            self.point_config_point_count
        )
    }

    pub fn summary_label(&self) -> String {
        format!(
            "plans={} breaking={} bounds={} rot={} cfg={}",
            self.plan_count,
            self.breaking_count,
            self.bounds_label(),
            self.rotation_label(),
            self.point_config_label()
        )
    }
}

/// Computes Mindustry block offset from block size.
pub fn block_offset(size: i32) -> f32 {
    assert!(size > 0, "plan block size must be positive");
    ((size + 1).rem_euclid(2) as f32) * TILE_SIZE / 2.0
}

/// Plain plan type that directly implements [`PlanEditable`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlanEditorPlan {
    pub x: i32,
    pub y: i32,
    pub rotation: i32,
    pub breaking: bool,
    pub block: PlanBlockMeta,
    pub point_config: PlanPointConfig,
}

pub trait PlanEditable {
    fn is_breaking(&self) -> bool;
    fn tile(&self) -> (i32, i32);
    fn set_tile(&mut self, x: i32, y: i32);
    fn rotation(&self) -> i32;
    fn set_rotation(&mut self, rotation: i32);
    fn block_meta(&self) -> PlanBlockMeta;
    fn point_config_family(&self) -> PlanPointConfigFamily;
    fn point_config_point_count(&self) -> usize;
    fn map_point_config<F>(&mut self, mapper: F)
    where
        F: FnMut(&mut PlanPoint);
}

impl PlanEditable for PlanEditorPlan {
    fn is_breaking(&self) -> bool {
        self.breaking
    }

    fn tile(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    fn set_tile(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    fn rotation(&self) -> i32 {
        self.rotation
    }

    fn set_rotation(&mut self, rotation: i32) {
        self.rotation = rotation;
    }

    fn block_meta(&self) -> PlanBlockMeta {
        self.block
    }

    fn point_config_family(&self) -> PlanPointConfigFamily {
        self.point_config.family()
    }

    fn point_config_point_count(&self) -> usize {
        match &self.point_config {
            PlanPointConfig::None => 0,
            PlanPointConfig::Point(_) => 1,
            PlanPointConfig::Points(points) => points.len(),
        }
    }

    fn map_point_config<F>(&mut self, mapper: F)
    where
        F: FnMut(&mut PlanPoint),
    {
        self.point_config.map_points(mapper);
    }
}

/// Rotates plans around tile-space origin.
/// Follows Java `InputHandler.rotatePlans` including point-config behavior.
pub fn rotate_plans<P: PlanEditable>(plans: &mut [P], origin: (i32, i32), direction: i32) {
    let (origin_x, origin_y) = origin;
    let quarter_turns = direction.rem_euclid(4);

    for plan in plans {
        if plan.is_breaking() {
            continue;
        }

        let block = plan.block_meta();
        let point_offset = if block.size.rem_euclid(2) == 0 {
            -0.5
        } else {
            0.0
        };
        plan.map_point_config(|point| {
            let mut cx = point.x as f32 + point_offset;
            let mut cy = point.y as f32 + point_offset;
            for _ in 0..quarter_turns {
                let lx = cx;
                cx = -cy;
                cy = lx;
            }

            point.x = (cx - point_offset).floor() as i32;
            point.y = (cy - point_offset).floor() as i32;
        });

        let (x, y) = plan.tile();
        let mut wx = (x - origin_x) as f32 * TILE_SIZE + block.offset;
        let mut wy = (y - origin_y) as f32 * TILE_SIZE + block.offset;
        for _ in 0..quarter_turns {
            let original_wx = wx;
            wx = -wy;
            wy = original_wx;
        }

        let next_x = world_to_tile(wx - block.offset) + origin_x;
        let next_y = world_to_tile(wy - block.offset) + origin_y;
        plan.set_tile(next_x, next_y);
        plan.set_rotation(block.plan_rotation((plan.rotation() + direction).rem_euclid(4)));
    }
}

/// Flips plans around one axis through tile-space origin.
/// Follows Java `InputHandler.flipPlans` including point-config and flip-rotation behavior.
pub fn flip_plans<P: PlanEditable>(plans: &mut [P], origin: (i32, i32), flip_x: bool) {
    let world_origin = if flip_x { origin.0 } else { origin.1 } as f32 * TILE_SIZE;

    for plan in plans {
        if plan.is_breaking() {
            continue;
        }

        let block = plan.block_meta();
        let (x, y) = plan.tile();
        let world_coord = if flip_x { x } else { y } as f32 * TILE_SIZE;
        let value = -(world_coord - world_origin + block.offset) + world_origin;
        let flipped_coord = ((value - block.offset) / TILE_SIZE) as i32;

        if flip_x {
            plan.set_tile(flipped_coord, y);
        } else {
            plan.set_tile(x, flipped_coord);
        }

        plan.map_point_config(|point| {
            if flip_x {
                if block.size.rem_euclid(2) == 0 {
                    point.x -= 1;
                }
                point.x = -point.x;
            } else {
                if block.size.rem_euclid(2) == 0 {
                    point.y -= 1;
                }
                point.y = -point.y;
            }
        });

        plan.set_rotation(block.flip_rotation(plan.rotation(), flip_x));
    }
}

fn world_to_tile(world: f32) -> i32 {
    ((world / TILE_SIZE) + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: i32, y: i32) -> PlanPoint {
        PlanPoint { x, y }
    }

    fn point_config_point(x: i32, y: i32) -> PlanPointConfig {
        PlanPointConfig::Point(point(x, y))
    }

    fn point_config_points(points: &[(i32, i32)]) -> PlanPointConfig {
        PlanPointConfig::Points(points.iter().map(|&(x, y)| point(x, y)).collect())
    }

    fn test_plan(x: i32, y: i32, rotation: i32) -> PlanEditorPlan {
        PlanEditorPlan {
            x,
            y,
            rotation,
            breaking: false,
            block: PlanBlockMeta::default(),
            point_config: PlanPointConfig::None,
        }
    }

    fn test_plan_with_block(
        x: i32,
        y: i32,
        rotation: i32,
        block: PlanBlockMeta,
        point_config: PlanPointConfig,
    ) -> PlanEditorPlan {
        PlanEditorPlan {
            block,
            point_config,
            ..test_plan(x, y, rotation)
        }
    }

    fn breaking_plan_with_block(
        x: i32,
        y: i32,
        rotation: i32,
        block: PlanBlockMeta,
        point_config: PlanPointConfig,
    ) -> PlanEditorPlan {
        PlanEditorPlan {
            breaking: true,
            block,
            point_config,
            ..test_plan(x, y, rotation)
        }
    }

    fn assert_plan_state(
        plan: &PlanEditorPlan,
        x: i32,
        y: i32,
        rotation: i32,
        point_config: PlanPointConfig,
    ) {
        assert_eq!(plan.x, x);
        assert_eq!(plan.y, y);
        assert_eq!(plan.rotation, rotation);
        assert_eq!(plan.point_config, point_config);
    }

    fn assert_summary_counts(
        summary: &PlanCollectionSummary,
        plan_count: usize,
        breaking_count: usize,
        bounds: Option<PlanBounds>,
        rotation_counts: [usize; 4],
        point_config_family_counts: [usize; 3],
        point_config_point_count: usize,
    ) {
        assert_eq!(summary.plan_count, plan_count);
        assert_eq!(summary.breaking_count, breaking_count);
        assert_eq!(summary.bounds, bounds);
        assert_eq!(summary.rotation_counts, rotation_counts);
        assert_eq!(
            summary.point_config_family_counts,
            point_config_family_counts
        );
        assert_eq!(summary.point_config_point_count, point_config_point_count);
    }

    fn assert_summary_labels(
        summary: &PlanCollectionSummary,
        bounds_label: &str,
        rotation_label: &str,
        point_config_label: &str,
        summary_label: &str,
    ) {
        assert_eq!(summary.bounds_label(), bounds_label);
        assert_eq!(summary.rotation_label(), rotation_label);
        assert_eq!(summary.point_config_label(), point_config_label);
        assert_eq!(summary.summary_label(), summary_label);
    }

    #[test]
    fn rotate_plans_clockwise_matches_java_behavior_and_skips_breaking() {
        let mut plans = vec![
            test_plan_with_block(
                3,
                2,
                1,
                PlanBlockMeta::with_size(2),
                point_config_point(1, 0),
            ),
            breaking_plan_with_block(9, 8, 3, PlanBlockMeta::with_size(2), point_config_point(7, -2)),
        ];

        rotate_plans(&mut plans, (1, 1), 1);

        assert_plan_state(&plans[0], -1, 3, 2, point_config_point(1, 1));
        assert_eq!(
            plans[1],
            breaking_plan_with_block(9, 8, 3, PlanBlockMeta::with_size(2), point_config_point(7, -2))
        );
    }

    #[test]
    fn rotate_plans_uses_plan_rotation_locking_rule() {
        let mut block = PlanBlockMeta::with_size(1);
        block.rotate = false;
        block.lock_rotation = true;
        let mut plans = vec![test_plan_with_block(2, 0, 3, block, PlanPointConfig::None)];

        rotate_plans(&mut plans, (0, 0), -1);

        assert_eq!(plans[0].x, 0);
        assert_eq!(plans[0].y, -2);
        assert_eq!(plans[0].rotation, 0);
    }

    #[test]
    fn rotate_plans_supports_multi_step_direction_consistently() {
        let mut plans = vec![test_plan_with_block(
            2,
            1,
            1,
            PlanBlockMeta::with_size(1),
            point_config_point(1, 0),
        )];

        rotate_plans(&mut plans, (0, 0), 2);

        assert_plan_state(&plans[0], -2, -1, 3, point_config_point(-1, 0));
    }

    #[test]
    fn flip_plans_x_axis_matches_java_even_size_rules() {
        let mut plans = vec![test_plan_with_block(
            2,
            5,
            0,
            PlanBlockMeta::with_size(2),
            point_config_point(2, 1),
        )];

        flip_plans(&mut plans, (1, 3), true);

        assert_plan_state(&plans[0], -1, 5, 2, point_config_point(-1, 1));
    }

    #[test]
    fn flip_plans_y_axis_respects_invert_flip_and_point_arrays() {
        let mut block = PlanBlockMeta::with_size(1);
        block.invert_flip = true;
        let mut plans = vec![test_plan_with_block(
            3,
            4,
            1,
            block,
            point_config_points(&[(1, 2), (-3, 0)]),
        )];

        flip_plans(&mut plans, (0, 2), false);

        assert_plan_state(&plans[0], 3, 0, 1, point_config_points(&[(1, -2), (-3, 0)]));
    }

    #[test]
    fn plan_collection_summary_tracks_bounds_rotations_and_config_families() {
        let plans = vec![
            test_plan_with_block(
                3,
                2,
                1,
                PlanBlockMeta::with_size(2),
                point_config_point(1, 0),
            ),
            PlanEditorPlan { breaking: true, ..test_plan(-1, 5, 3) },
            test_plan_with_block(
                7,
                -4,
                0,
                PlanBlockMeta::with_size(3),
                point_config_points(&[(2, 2), (-1, 4)]),
            ),
        ];

        let summary = PlanCollectionSummary::from_plans(&plans);

        let bounds = Some(PlanBounds {
            min_x: -1,
            min_y: -4,
            max_x: 7,
            max_y: 5,
        });

        assert_summary_counts(
            &summary,
            3,
            1,
            bounds,
            [1, 1, 0, 1],
            [1, 1, 1],
            3,
        );
        assert_summary_labels(
            &summary,
            "-1:-4..7:5",
            "r0=1 r1=1 r2=0 r3=1",
            "none=1 point=1 points=1 total=3",
            "plans=3 breaking=1 bounds=-1:-4..7:5 rot=r0=1 r1=1 r2=0 r3=1 cfg=none=1 point=1 points=1 total=3",
        );
        assert!(summary.has_bounds());
    }

    #[test]
    fn plan_collection_summary_from_empty_plans_returns_empty_defaults() {
        let plans: Vec<PlanEditorPlan> = vec![];

        let summary = PlanCollectionSummary::from_plans(&plans);

        assert_summary_counts(&summary, 0, 0, None, [0, 0, 0, 0], [0, 0, 0], 0);
        assert_summary_labels(
            &summary,
            "none",
            "r0=0 r1=0 r2=0 r3=0",
            "none=0 point=0 points=0 total=0",
            "plans=0 breaking=0 bounds=none rot=r0=0 r1=0 r2=0 r3=0 cfg=none=0 point=0 points=0 total=0",
        );
        assert!(!summary.has_bounds());
    }

    #[test]
    fn plan_point_config_map_points_covers_none_point_and_points_variants() {
        let mut none = PlanPointConfig::None;
        let mut none_calls = 0;
        none.map_points(|_| {
            none_calls += 1;
        });
        assert_eq!(none, PlanPointConfig::None);
        assert_eq!(none_calls, 0);

        let mut point = point_config_point(1, -2);
        point.map_points(|value| {
            value.x += 3;
            value.y -= 4;
        });
        assert_eq!(point, point_config_point(4, -6));

        let mut points = point_config_points(&[(0, 1), (-3, 5)]);
        points.map_points(|value| {
            value.x = -value.x;
            value.y += 2;
        });
        assert_eq!(points, point_config_points(&[(0, 3), (3, 7)]));
    }

    #[test]
    fn plan_bounds_helpers_report_width_height_and_label() {
        let bounds = PlanBounds {
            min_x: -2,
            min_y: 3,
            max_x: 4,
            max_y: 9,
        };

        assert_eq!(bounds.width(), 7);
        assert_eq!(bounds.height(), 7);
        assert_eq!(bounds.label(), "-2:3..4:9");
    }

    #[test]
    fn plan_point_config_family_and_labels_match_variants() {
        let cases = [
            (PlanPointConfig::None, PlanPointConfigFamily::None, "none"),
            (point_config_point(2, 1), PlanPointConfigFamily::Point, "point"),
            (
                point_config_points(&[(-1, 0)]),
                PlanPointConfigFamily::Points,
                "points",
            ),
        ];

        for (config, family, label) in cases {
            assert_eq!(config.family(), family);
            assert_eq!(config.family().label(), label);
            assert_eq!(family.label(), label);
        }
    }

    #[test]
    fn plan_collection_summary_reflects_rotate_and_flip_output_shape() {
        let mut plans = vec![
            test_plan_with_block(
                2,
                1,
                0,
                PlanBlockMeta::with_size(2),
                point_config_point(1, 0),
            ),
            test_plan_with_block(
                -2,
                4,
                3,
                PlanBlockMeta::with_size(1),
                point_config_points(&[(0, 1)]),
            ),
        ];

        rotate_plans(&mut plans, (0, 0), 1);
        flip_plans(&mut plans, (0, 0), true);

        let summary = PlanCollectionSummary::from_plans(&plans);

        assert_summary_counts(
            &summary,
            2,
            0,
            Some(PlanBounds {
                min_x: 1,
                min_y: -2,
                max_x: 4,
                max_y: 2,
            }),
            [0, 1, 1, 0],
            [0, 1, 1],
            2,
        );
    }

    #[test]
    fn flip_plans_skips_breaking_plans() {
        let mut plans = vec![
            test_plan_with_block(
                2,
                5,
                0,
                PlanBlockMeta::with_size(2),
                point_config_point(2, 1),
            ),
            breaking_plan_with_block(9, 8, 3, PlanBlockMeta::with_size(2), point_config_point(7, -2)),
        ];

        flip_plans(&mut plans, (1, 3), true);

        assert_plan_state(&plans[0], -1, 5, 2, point_config_point(-1, 1));
        assert_eq!(
            plans[1],
            breaking_plan_with_block(9, 8, 3, PlanBlockMeta::with_size(2), point_config_point(7, -2))
        );
    }

    #[test]
    fn plan_collection_summary_normalizes_negative_and_wrapped_rotations() {
        let plans = vec![
            test_plan(0, 0, -1),
            test_plan(1, 0, 4),
            test_plan_with_block(
                2,
                0,
                9,
                PlanBlockMeta::default(),
                point_config_points(&[(0, 0), (1, 1)]),
            ),
        ];

        let summary = PlanCollectionSummary::from_plans(&plans);

        assert_summary_counts(
            &summary,
            3,
            0,
            Some(PlanBounds {
                min_x: 0,
                min_y: 0,
                max_x: 2,
                max_y: 0,
            }),
            [1, 1, 0, 1],
            [2, 0, 1],
            2,
        );
    }

    #[test]
    fn world_to_tile_matches_java_rounding_for_negative_half_tile() {
        for (world, tile) in [(4.0, 1), (-4.0, 0)] {
            assert_eq!(world_to_tile(world), tile);
        }
    }

    #[test]
    fn plan_rotation_matches_java_passthrough_semantics() {
        let mut block = PlanBlockMeta::with_size(1);
        block.rotate = true;
        block.lock_rotation = false;

        assert_eq!(block.plan_rotation(5), 5);
        assert_eq!(block.flip_rotation(4, true), 2);
    }

    #[test]
    fn plan_block_meta_rejects_non_positive_sizes() {
        for size in [0, -1] {
            assert!(std::panic::catch_unwind(|| PlanBlockMeta::with_size(size)).is_err());
        }
        for size in [0, -2] {
            assert!(std::panic::catch_unwind(|| block_offset(size)).is_err());
        }
    }
}
