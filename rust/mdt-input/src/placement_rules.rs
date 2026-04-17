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

/// First rejection cause reported by the local-plan placement gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRejectReason {
    RequestSizeNonPositive { size: i32 },
    PlanSizeNonPositive { plan_index: usize, size: i32 },
    PlanOverlapsRequest { plan_index: usize },
    ExactOverlapRequiresReplacement { plan_index: usize },
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
    valid_place_against_local_plans_with_reason(request, local_plans, ignore_plan_index).is_ok()
}

/// Mirrors the local-plan overlap part of Java `InputHandler.validPlace(...)`.
/// Returns the first rejection cause encountered while evaluating local plans.
pub fn valid_place_against_local_plans_with_reason(
    request: PlacementRequest,
    local_plans: &[LocalPlanPlacement],
    ignore_plan_index: Option<usize>,
) -> Result<(), PlacementRejectReason> {
    if request.size <= 0 {
        return Err(PlacementRejectReason::RequestSizeNonPositive {
            size: request.size,
        });
    }
    let request_bounds = placement_bounds(request.x, request.y, request.size);

    for (index, plan) in local_plans.iter().enumerate() {
        if Some(index) == ignore_plan_index || plan.breaking {
            continue;
        }
        if plan.size <= 0 {
            return Err(PlacementRejectReason::PlanSizeNonPositive {
                plan_index: index,
                size: plan.size,
            });
        }

        let plan_bounds = placement_bounds(plan.x, plan.y, plan.size);
        if plan_bounds.overlaps(request_bounds) {
            if plan.candidate_can_replace_plan && plan_bounds == request_bounds {
                continue;
            }
            return if plan_bounds == request_bounds {
                Err(PlacementRejectReason::ExactOverlapRequiresReplacement {
                    plan_index: index,
                })
            } else {
                Err(PlacementRejectReason::PlanOverlapsRequest {
                    plan_index: index,
                })
            };
        }
    }

    Ok(())
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
        placement_bounds, repair_derelict_candidate, valid_place_against_local_plans,
        valid_place_against_local_plans_with_reason, LocalPlanPlacement, PlacementRejectReason,
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
        assert_eq!(
            valid_place_against_local_plans_with_reason(
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
            ),
            Err(PlacementRejectReason::PlanOverlapsRequest { plan_index: 0 })
        );
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
        assert_eq!(
            valid_place_against_local_plans_with_reason(
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
            ),
            Err(PlacementRejectReason::ExactOverlapRequiresReplacement {
                plan_index: 0
            })
        );
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
    fn valid_place_against_local_plans_rejects_partial_overlap_even_when_replace_is_allowed() {
        assert_eq!(
            valid_place_against_local_plans_with_reason(
                PlacementRequest {
                    x: 5,
                    y: 5,
                    size: 2,
                },
                &[LocalPlanPlacement {
                    x: 6,
                    y: 5,
                    size: 2,
                    breaking: false,
                    candidate_can_replace_plan: true,
                }],
                None,
            ),
            Err(PlacementRejectReason::PlanOverlapsRequest { plan_index: 0 })
        );
        assert!(!valid_place_against_local_plans(
            PlacementRequest {
                x: 5,
                y: 5,
                size: 2,
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
    fn valid_place_against_local_plans_skips_ignored_conflicting_plan_index() {
        let request = PlacementRequest {
            x: 7,
            y: 9,
            size: 2,
        };
        let plans = [
            LocalPlanPlacement {
                x: 7,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: false,
            },
            LocalPlanPlacement {
                x: 20,
                y: 20,
                size: 1,
                breaking: false,
                candidate_can_replace_plan: false,
            },
        ];

        assert_eq!(
            valid_place_against_local_plans_with_reason(request, &plans, Some(0)),
            Ok(())
        );
        assert!(valid_place_against_local_plans(request, &plans, Some(0)));
        assert_eq!(
            valid_place_against_local_plans_with_reason(request, &plans, None),
            Err(PlacementRejectReason::ExactOverlapRequiresReplacement {
                plan_index: 0
            })
        );
    }

    #[test]
    fn valid_place_against_local_plans_skips_ignored_invalid_plan_size() {
        let request = PlacementRequest {
            x: 3,
            y: 4,
            size: 1,
        };
        let plans = [
            LocalPlanPlacement {
                x: 3,
                y: 4,
                size: 0,
                breaking: false,
                candidate_can_replace_plan: false,
            },
            LocalPlanPlacement {
                x: 20,
                y: 20,
                size: 1,
                breaking: false,
                candidate_can_replace_plan: false,
            },
        ];

        assert_eq!(
            valid_place_against_local_plans_with_reason(request, &plans, Some(0)),
            Ok(())
        );
        assert!(valid_place_against_local_plans(request, &plans, Some(0)));
        assert_eq!(
            valid_place_against_local_plans_with_reason(request, &plans, None),
            Err(PlacementRejectReason::PlanSizeNonPositive {
                plan_index: 0,
                size: 0
            })
        );
    }

    #[test]
    fn valid_place_against_local_plans_skips_exact_replacement_plan_before_later_overlap() {
        let request = PlacementRequest {
            x: 7,
            y: 9,
            size: 2,
        };
        let plans = [
            LocalPlanPlacement {
                x: 7,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: true,
            },
            LocalPlanPlacement {
                x: 8,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: false,
            },
        ];

        assert_eq!(
            valid_place_against_local_plans_with_reason(request, &plans, None),
            Err(PlacementRejectReason::PlanOverlapsRequest { plan_index: 1 })
        );
        assert!(!valid_place_against_local_plans(request, &plans, None));
    }

    #[test]
    fn valid_place_against_local_plans_skips_exact_replacement_plan_before_later_exact_overlap() {
        let request = PlacementRequest {
            x: 7,
            y: 9,
            size: 2,
        };
        let plans = [
            LocalPlanPlacement {
                x: 7,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: true,
            },
            LocalPlanPlacement {
                x: 7,
                y: 9,
                size: 2,
                breaking: false,
                candidate_can_replace_plan: false,
            },
        ];

        assert_eq!(
            valid_place_against_local_plans_with_reason(request, &plans, None),
            Err(PlacementRejectReason::ExactOverlapRequiresReplacement {
                plan_index: 1
            })
        );
        assert!(!valid_place_against_local_plans(request, &plans, None));
    }

    #[test]
    fn valid_place_against_local_plans_rejects_non_positive_sizes() {
        assert_eq!(
            valid_place_against_local_plans_with_reason(
                PlacementRequest {
                    x: 3,
                    y: 4,
                    size: 0,
                },
                &[LocalPlanPlacement {
                    x: 3,
                    y: 4,
                    size: 1,
                    breaking: false,
                    candidate_can_replace_plan: false,
                }],
                None,
            ),
            Err(PlacementRejectReason::RequestSizeNonPositive { size: 0 })
        );
        assert!(!valid_place_against_local_plans(
            PlacementRequest {
                x: 3,
                y: 4,
                size: 0,
            },
            &[LocalPlanPlacement {
                x: 3,
                y: 4,
                size: 1,
                breaking: false,
                candidate_can_replace_plan: false,
            }],
            None,
        ));
        assert_eq!(
            valid_place_against_local_plans_with_reason(
                PlacementRequest {
                    x: 3,
                    y: 4,
                    size: 1,
                },
                &[LocalPlanPlacement {
                    x: 3,
                    y: 4,
                    size: 0,
                    breaking: false,
                    candidate_can_replace_plan: false,
                }],
                None,
            ),
            Err(PlacementRejectReason::PlanSizeNonPositive {
                plan_index: 0,
                size: 0
            })
        );
        assert!(!valid_place_against_local_plans(
            PlacementRequest {
                x: 3,
                y: 4,
                size: 1,
            },
            &[LocalPlanPlacement {
                x: 3,
                y: 4,
                size: 0,
                breaking: false,
                candidate_can_replace_plan: false,
            }],
            None,
        ));
    }

    #[test]
    fn placement_bounds_and_overlaps_handle_center_overlap_and_touching_edges() {
        let center = placement_bounds(10, 10, 1);
        let overlapping = placement_bounds(10, 10, 1);
        let touching = placement_bounds(11, 10, 1);

        assert!(center.overlaps(overlapping));
        assert!(!center.overlaps(touching));
    }

    #[test]
    fn placement_bounds_calculates_center_and_extent_for_even_and_odd_sizes() {
        assert_eq!(
            placement_bounds(10, 10, 1),
            super::PlacementBounds {
                left: 76.0,
                right: 84.0,
                bottom: 76.0,
                top: 84.0,
            }
        );
        assert_eq!(
            placement_bounds(10, 10, 2),
            super::PlacementBounds {
                left: 76.0,
                right: 92.0,
                bottom: 76.0,
                top: 92.0,
            }
        );
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
