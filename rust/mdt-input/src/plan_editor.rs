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
            rotation.rem_euclid(4)
        }
    }

    pub fn flip_rotation(self, rotation: i32, flip_x: bool) -> i32 {
        let even_rotation = rotation.rem_euclid(2) == 0;
        if (flip_x == even_rotation) != self.invert_flip {
            self.plan_rotation(rotation + 2)
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

/// Computes Mindustry block offset from block size.
pub fn block_offset(size: i32) -> f32 {
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
        plan.set_rotation(block.plan_rotation(plan.rotation() + direction));
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

    #[test]
    fn rotate_plans_clockwise_matches_java_behavior_and_skips_breaking() {
        let mut plans = vec![
            PlanEditorPlan {
                x: 3,
                y: 2,
                rotation: 1,
                breaking: false,
                block: PlanBlockMeta::with_size(2),
                point_config: PlanPointConfig::Point(PlanPoint { x: 1, y: 0 }),
            },
            PlanEditorPlan {
                x: 9,
                y: 8,
                rotation: 3,
                breaking: true,
                block: PlanBlockMeta::with_size(2),
                point_config: PlanPointConfig::Point(PlanPoint { x: 7, y: -2 }),
            },
        ];

        rotate_plans(&mut plans, (1, 1), 1);

        assert_eq!(plans[0].x, -1);
        assert_eq!(plans[0].y, 3);
        assert_eq!(plans[0].rotation, 2);
        assert_eq!(
            plans[0].point_config,
            PlanPointConfig::Point(PlanPoint { x: 1, y: 1 })
        );
        assert_eq!(
            plans[1],
            PlanEditorPlan {
                x: 9,
                y: 8,
                rotation: 3,
                breaking: true,
                block: PlanBlockMeta::with_size(2),
                point_config: PlanPointConfig::Point(PlanPoint { x: 7, y: -2 }),
            }
        );
    }

    #[test]
    fn rotate_plans_uses_plan_rotation_locking_rule() {
        let mut block = PlanBlockMeta::with_size(1);
        block.rotate = false;
        block.lock_rotation = true;
        let mut plans = vec![PlanEditorPlan {
            x: 2,
            y: 0,
            rotation: 3,
            breaking: false,
            block,
            point_config: PlanPointConfig::None,
        }];

        rotate_plans(&mut plans, (0, 0), -1);

        assert_eq!(plans[0].x, 0);
        assert_eq!(plans[0].y, -2);
        assert_eq!(plans[0].rotation, 0);
    }

    #[test]
    fn rotate_plans_supports_multi_step_direction_consistently() {
        let mut plans = vec![PlanEditorPlan {
            x: 2,
            y: 1,
            rotation: 1,
            breaking: false,
            block: PlanBlockMeta::with_size(1),
            point_config: PlanPointConfig::Point(PlanPoint { x: 1, y: 0 }),
        }];

        rotate_plans(&mut plans, (0, 0), 2);

        assert_eq!(plans[0].x, -2);
        assert_eq!(plans[0].y, -1);
        assert_eq!(plans[0].rotation, 3);
        assert_eq!(
            plans[0].point_config,
            PlanPointConfig::Point(PlanPoint { x: -1, y: 0 })
        );
    }

    #[test]
    fn flip_plans_x_axis_matches_java_even_size_rules() {
        let mut plans = vec![PlanEditorPlan {
            x: 2,
            y: 5,
            rotation: 0,
            breaking: false,
            block: PlanBlockMeta::with_size(2),
            point_config: PlanPointConfig::Point(PlanPoint { x: 2, y: 1 }),
        }];

        flip_plans(&mut plans, (1, 3), true);

        assert_eq!(plans[0].x, -1);
        assert_eq!(plans[0].y, 5);
        assert_eq!(plans[0].rotation, 2);
        assert_eq!(
            plans[0].point_config,
            PlanPointConfig::Point(PlanPoint { x: -1, y: 1 })
        );
    }

    #[test]
    fn flip_plans_y_axis_respects_invert_flip_and_point_arrays() {
        let mut block = PlanBlockMeta::with_size(1);
        block.invert_flip = true;
        let mut plans = vec![PlanEditorPlan {
            x: 3,
            y: 4,
            rotation: 1,
            breaking: false,
            block,
            point_config: PlanPointConfig::Points(vec![
                PlanPoint { x: 1, y: 2 },
                PlanPoint { x: -3, y: 0 },
            ]),
        }];

        flip_plans(&mut plans, (0, 2), false);

        assert_eq!(plans[0].x, 3);
        assert_eq!(plans[0].y, 0);
        assert_eq!(plans[0].rotation, 1);
        assert_eq!(
            plans[0].point_config,
            PlanPointConfig::Points(vec![PlanPoint { x: 1, y: -2 }, PlanPoint { x: -3, y: 0 },])
        );
    }

    #[test]
    fn world_to_tile_matches_java_rounding_for_negative_half_tile() {
        assert_eq!(world_to_tile(4.0), 1);
        assert_eq!(world_to_tile(-4.0), 0);
    }
}
