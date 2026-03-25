//! Pure placement-side gates extracted from Java placement/input flow.
//! Keeps only the lowest-risk local-plan conflict and derelict-repair observation checks.

use crate::plan_editor::{block_offset, TILE_SIZE};

/// Candidate placement request for local-plan conflict checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementRequest {
    pub x: i32,
    pub y: i32,
    pub size: i32,
}

/// Local plan facts needed by the `InputHandler.validPlace(...)` overlap gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalPlanPlacement {
    pub x: i32,
    pub y: i32,
    pub size: i32,
    pub breaking: bool,
    /// Pre-resolved `candidate_type.canReplace(plan.block)` for this request.
    pub candidate_can_replace_plan: bool,
}

/// Selected derelict build facts needed before any world-level `Build.validPlace(...)` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairDerelictBuildObservation<B, C> {
    pub block_unlocked: bool,
    pub team_is_derelict: bool,
    pub tile_x: i32,
    pub tile_y: i32,
    pub rotation: i32,
    pub block: B,
    pub config: C,
}

/// Pure observation inputs for derelict-repair candidate extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairDerelictObservation<B, C> {
    pub player_dead: bool,
    pub rules_editor: bool,
    pub player_team_is_derelict: bool,
    pub selected_build: Option<RepairDerelictBuildObservation<B, C>>,
}

/// Build-plan candidate emitted once the pure derelict-repair observation gate passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairDerelictCandidate<B, C> {
    pub x: i32,
    pub y: i32,
    pub rotation: i32,
    pub block: B,
    pub config: C,
}

/// Mirrors the local-plan overlap part of Java `InputHandler.validPlace(...)`.
pub fn valid_place_against_local_plans(
    request: PlacementRequest,
    local_plans: &[LocalPlanPlacement],
    ignore_plan_index: Option<usize>,
) -> bool {
    let request_bounds = placement_bounds(request.x, request.y, request.size);

    local_plans.iter().enumerate().all(|(index, plan)| {
        if Some(index) == ignore_plan_index || plan.breaking {
            return true;
        }

        let plan_bounds = placement_bounds(plan.x, plan.y, plan.size);
        !plan_bounds.overlaps(request_bounds)
            || (plan.candidate_can_replace_plan && plan_bounds == request_bounds)
    })
}

/// Mirrors the pure observation portion of Java `tryRepairDerelict/canRepairDerelict`.
/// The caller still needs to run world-level placement validation afterwards.
pub fn repair_derelict_candidate<B: Clone, C: Clone>(
    observation: &RepairDerelictObservation<B, C>,
) -> Option<RepairDerelictCandidate<B, C>> {
    if observation.player_dead || observation.rules_editor || observation.player_team_is_derelict {
        return None;
    }

    let build = observation.selected_build.as_ref()?;
    if !build.block_unlocked || !build.team_is_derelict {
        return None;
    }

    Some(RepairDerelictCandidate {
        x: build.tile_x,
        y: build.tile_y,
        rotation: build.rotation,
        block: build.block.clone(),
        config: build.config.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlacementBounds {
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
}

impl PlacementBounds {
    fn overlaps(self, other: Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.bottom < other.top
            && self.top > other.bottom
    }
}

fn placement_bounds(x: i32, y: i32, size: i32) -> PlacementBounds {
    debug_assert!(size > 0);

    let offset = block_offset(size);
    let half_extent = size as f32 * TILE_SIZE / 2.0;
    let center_x = x as f32 * TILE_SIZE + offset;
    let center_y = y as f32 * TILE_SIZE + offset;

    PlacementBounds {
        left: center_x - half_extent,
        right: center_x + half_extent,
        bottom: center_y - half_extent,
        top: center_y + half_extent,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        repair_derelict_candidate, valid_place_against_local_plans, LocalPlanPlacement,
        PlacementRequest, RepairDerelictBuildObservation, RepairDerelictCandidate,
        RepairDerelictObservation,
    };

    #[test]
    fn valid_place_against_local_plans_ignores_breaking_plans() {
        assert!(valid_place_against_local_plans(
            PlacementRequest {
                x: 10,
                y: 10,
                size: 1,
            },
            &[LocalPlanPlacement {
                x: 10,
                y: 10,
                size: 1,
                breaking: true,
                candidate_can_replace_plan: false,
            }],
            None,
        ));
    }

    #[test]
    fn valid_place_against_local_plans_rejects_overlapping_non_breaking_plan() {
        assert!(!valid_place_against_local_plans(
            PlacementRequest {
                x: 5,
                y: 5,
                size: 2
            },
            &[LocalPlanPlacement {
                x: 6,
                y: 5,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: true,
            }],
            None,
        ));
    }

    #[test]
    fn valid_place_against_local_plans_allows_exact_replace_when_candidate_can_replace() {
        assert!(valid_place_against_local_plans(
            PlacementRequest {
                x: 7,
                y: 9,
                size: 2
            },
            &[LocalPlanPlacement {
                x: 7,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: true,
            }],
            None,
        ));
    }

    #[test]
    fn valid_place_against_local_plans_rejects_exact_overlap_when_replace_is_not_allowed() {
        assert!(!valid_place_against_local_plans(
            PlacementRequest {
                x: 7,
                y: 9,
                size: 2
            },
            &[LocalPlanPlacement {
                x: 7,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: false,
            }],
            None,
        ));
    }

    #[test]
    fn valid_place_against_local_plans_skips_ignored_plan_index() {
        assert!(valid_place_against_local_plans(
            PlacementRequest {
                x: 3,
                y: 4,
                size: 1
            },
            &[
                LocalPlanPlacement {
                    x: 3,
                    y: 4,
                    size: 1,
                    breaking: false,
                    candidate_can_replace_plan: false,
                },
                LocalPlanPlacement {
                    x: 9,
                    y: 9,
                    size: 1,
                    breaking: false,
                    candidate_can_replace_plan: false,
                },
            ],
            Some(0),
        ));
    }

    #[test]
    fn repair_derelict_candidate_returns_build_plan_candidate_when_gate_passes() {
        let observation = RepairDerelictObservation {
            player_dead: false,
            rules_editor: false,
            player_team_is_derelict: false,
            selected_build: Some(RepairDerelictBuildObservation {
                block_unlocked: true,
                team_is_derelict: true,
                tile_x: 11,
                tile_y: 12,
                rotation: 3,
                block: "scrap-wall-large",
                config: Some(42),
            }),
        };

        assert_eq!(
            repair_derelict_candidate(&observation),
            Some(RepairDerelictCandidate {
                x: 11,
                y: 12,
                rotation: 3,
                block: "scrap-wall-large",
                config: Some(42),
            })
        );
    }

    #[test]
    fn repair_derelict_candidate_rejects_non_derelict_or_locked_builds() {
        let locked = RepairDerelictObservation {
            player_dead: false,
            rules_editor: false,
            player_team_is_derelict: false,
            selected_build: Some(RepairDerelictBuildObservation {
                block_unlocked: false,
                team_is_derelict: true,
                tile_x: 1,
                tile_y: 2,
                rotation: 0,
                block: "router",
                config: None::<i32>,
            }),
        };
        let wrong_team = RepairDerelictObservation {
            player_dead: false,
            rules_editor: false,
            player_team_is_derelict: false,
            selected_build: Some(RepairDerelictBuildObservation {
                block_unlocked: true,
                team_is_derelict: false,
                tile_x: 1,
                tile_y: 2,
                rotation: 0,
                block: "router",
                config: None::<i32>,
            }),
        };

        assert_eq!(repair_derelict_candidate(&locked), None);
        assert_eq!(repair_derelict_candidate(&wrong_team), None);
    }

    #[test]
    fn repair_derelict_candidate_rejects_player_and_mode_gates() {
        let selected_build = Some(RepairDerelictBuildObservation {
            block_unlocked: true,
            team_is_derelict: true,
            tile_x: 4,
            tile_y: 6,
            rotation: 1,
            block: "duo",
            config: None::<i32>,
        });

        let player_dead = RepairDerelictObservation {
            player_dead: true,
            rules_editor: false,
            player_team_is_derelict: false,
            selected_build: selected_build.clone(),
        };
        let editor_mode = RepairDerelictObservation {
            player_dead: false,
            rules_editor: true,
            player_team_is_derelict: false,
            selected_build: selected_build.clone(),
        };
        let derelict_player = RepairDerelictObservation {
            player_dead: false,
            rules_editor: false,
            player_team_is_derelict: true,
            selected_build,
        };

        assert_eq!(repair_derelict_candidate(&player_dead), None);
        assert_eq!(repair_derelict_candidate(&editor_mode), None);
        assert_eq!(repair_derelict_candidate(&derelict_player), None);
    }

    #[test]
    fn repair_derelict_candidate_requires_selected_build() {
        let observation = RepairDerelictObservation::<&'static str, Option<i32>> {
            player_dead: false,
            rules_editor: false,
            player_team_is_derelict: false,
            selected_build: None,
        };

        assert_eq!(repair_derelict_candidate(&observation), None);
    }
}
