use crate::panel_model::{
    build_build_minimap_assist_panel, BuildInteractionAuthorityState, BuildInteractionMode,
    BuildInteractionQueueState, BuildInteractionSelectionState, BuildMinimapAssistPanelModel,
    PresenterViewWindow,
};
use crate::{HudModel, RenderModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildUserFlowBlocker {
    Arm,
    Realign,
    Resolve,
    Refocus,
    Survey,
}

impl BuildUserFlowBlocker {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Arm => "arm",
            Self::Realign => "realign",
            Self::Resolve => "resolve",
            Self::Refocus => "refocus",
            Self::Survey => "survey",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildUserFlowPanelModel {
    pub next_action: &'static str,
    pub blockers: Vec<BuildUserFlowBlocker>,
    pub route: Vec<&'static str>,
}

impl BuildUserFlowPanelModel {
    pub(crate) fn blocker_labels(&self) -> Vec<&'static str> {
        self.blockers
            .iter()
            .copied()
            .map(BuildUserFlowBlocker::label)
            .collect()
    }

    pub(crate) fn blocker_count(&self) -> usize {
        self.blockers.len()
    }

    pub(crate) fn route_count(&self) -> usize {
        self.route.len()
    }
}

pub(crate) fn build_build_user_flow_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<BuildUserFlowPanelModel> {
    let assist = build_build_minimap_assist_panel(scene, hud, window)?;
    Some(build_user_flow_from_assist(&assist))
}

fn build_user_flow_from_assist(assist: &BuildMinimapAssistPanelModel) -> BuildUserFlowPanelModel {
    let blockers = build_blockers(assist);
    let mut route = blockers
        .iter()
        .copied()
        .map(BuildUserFlowBlocker::label)
        .collect::<Vec<_>>();

    match assist.mode {
        BuildInteractionMode::Idle => push_route_step(&mut route, "idle"),
        BuildInteractionMode::Break => push_route_step(&mut route, "break"),
        BuildInteractionMode::Place => {
            if !assist.place_ready {
                push_route_step(&mut route, "arm");
            } else {
                if matches!(assist.queue_state, BuildInteractionQueueState::Empty) {
                    push_route_step(&mut route, "seed");
                }
                push_route_step(&mut route, "commit");
            }
        }
    }

    BuildUserFlowPanelModel {
        next_action: assist.next_action_label(),
        blockers,
        route,
    }
}

fn build_blockers(assist: &BuildMinimapAssistPanelModel) -> Vec<BuildUserFlowBlocker> {
    match assist.mode {
        BuildInteractionMode::Idle => Vec::new(),
        BuildInteractionMode::Break => {
            if focus_needs_refocus(assist) {
                vec![BuildUserFlowBlocker::Refocus]
            } else {
                Vec::new()
            }
        }
        BuildInteractionMode::Place => {
            if !assist.place_ready {
                return vec![BuildUserFlowBlocker::Arm];
            }

            let mut blockers = Vec::new();
            if matches!(
                assist.selection_state,
                BuildInteractionSelectionState::HeadDiverged
            ) {
                blockers.push(BuildUserFlowBlocker::Realign);
            }
            if authority_needs_attention(assist.authority_state) {
                blockers.push(BuildUserFlowBlocker::Resolve);
            }
            if focus_needs_refocus(assist) {
                blockers.push(BuildUserFlowBlocker::Refocus);
            }
            if matches!(assist.map_visibility_label(), "unseen" | "hidden") {
                blockers.push(BuildUserFlowBlocker::Survey);
            }
            blockers
        }
    }
}

fn authority_needs_attention(state: BuildInteractionAuthorityState) -> bool {
    !matches!(
        state,
        BuildInteractionAuthorityState::None | BuildInteractionAuthorityState::Applied
    )
}

fn focus_needs_refocus(assist: &BuildMinimapAssistPanelModel) -> bool {
    assist.focus_tile.is_none() || matches!(assist.focus_in_window, Some(false))
}

fn push_route_step(route: &mut Vec<&'static str>, step: &'static str) {
    if route.last().copied() != Some(step) {
        route.push(step);
    }
}

#[cfg(test)]
mod tests {
    use super::{build_user_flow_from_assist, BuildUserFlowBlocker, BuildUserFlowPanelModel};
    use crate::panel_model::{
        BuildInteractionAuthorityState, BuildInteractionMode, BuildInteractionQueueState,
        BuildInteractionSelectionState, BuildMinimapAssistPanelModel,
    };

    #[test]
    fn build_user_flow_route_tracks_ordered_place_blockers_and_commit_path() {
        let panel = build_user_flow_from_assist(&BuildMinimapAssistPanelModel {
            mode: BuildInteractionMode::Place,
            selection_state: BuildInteractionSelectionState::HeadDiverged,
            queue_state: BuildInteractionQueueState::Queued,
            place_ready: true,
            config_family_count: 2,
            config_sample_count: 5,
            top_config_family: Some("power-node".to_string()),
            authority_state: BuildInteractionAuthorityState::Rollback,
            focus_tile: Some((12, 18)),
            focus_in_window: Some(false),
            visible_map_percent: 0,
            unknown_tile_percent: 100,
            window_coverage_percent: 25,
            tracked_object_count: 8,
            runtime_count: 2,
        });

        assert_eq!(panel.next_action, "realign");
        assert_eq!(
            panel.blockers,
            vec![
                BuildUserFlowBlocker::Realign,
                BuildUserFlowBlocker::Resolve,
                BuildUserFlowBlocker::Refocus,
                BuildUserFlowBlocker::Survey,
            ]
        );
        assert_eq!(
            panel.route,
            vec!["realign", "resolve", "refocus", "survey", "commit"]
        );
    }

    #[test]
    fn build_user_flow_route_adds_seed_before_commit_for_ready_empty_queue() {
        let panel = build_user_flow_from_assist(&BuildMinimapAssistPanelModel {
            mode: BuildInteractionMode::Place,
            selection_state: BuildInteractionSelectionState::HeadAligned,
            queue_state: BuildInteractionQueueState::Empty,
            place_ready: true,
            config_family_count: 1,
            config_sample_count: 1,
            top_config_family: Some("message".to_string()),
            authority_state: BuildInteractionAuthorityState::Applied,
            focus_tile: Some((4, 6)),
            focus_in_window: Some(true),
            visible_map_percent: 100,
            unknown_tile_percent: 0,
            window_coverage_percent: 40,
            tracked_object_count: 3,
            runtime_count: 0,
        });

        assert_eq!(panel.next_action, "seed");
        assert!(panel.blockers.is_empty());
        assert_eq!(panel.route, vec!["seed", "commit"]);
    }

    #[test]
    fn build_user_flow_route_stays_bounded_for_unarmed_break_and_idle_states() {
        let place_arm = build_user_flow_from_assist(&BuildMinimapAssistPanelModel {
            mode: BuildInteractionMode::Place,
            selection_state: BuildInteractionSelectionState::Unarmed,
            queue_state: BuildInteractionQueueState::Empty,
            place_ready: false,
            config_family_count: 0,
            config_sample_count: 0,
            top_config_family: None,
            authority_state: BuildInteractionAuthorityState::None,
            focus_tile: Some((1, 1)),
            focus_in_window: Some(true),
            visible_map_percent: 50,
            unknown_tile_percent: 50,
            window_coverage_percent: 10,
            tracked_object_count: 1,
            runtime_count: 0,
        });
        assert_eq!(place_arm.next_action, "arm");
        assert_eq!(place_arm.blocker_labels(), vec!["arm"]);
        assert_eq!(place_arm.route, vec!["arm"]);

        let break_refocus = build_user_flow_from_assist(&BuildMinimapAssistPanelModel {
            mode: BuildInteractionMode::Break,
            selection_state: BuildInteractionSelectionState::BreakingHead,
            queue_state: BuildInteractionQueueState::Queued,
            place_ready: false,
            config_family_count: 0,
            config_sample_count: 0,
            top_config_family: None,
            authority_state: BuildInteractionAuthorityState::Applied,
            focus_tile: None,
            focus_in_window: None,
            visible_map_percent: 100,
            unknown_tile_percent: 0,
            window_coverage_percent: 100,
            tracked_object_count: 1,
            runtime_count: 0,
        });
        assert_eq!(break_refocus.next_action, "refocus");
        assert_eq!(break_refocus.blocker_labels(), vec!["refocus"]);
        assert_eq!(break_refocus.route, vec!["refocus", "break"]);

        let idle = build_user_flow_from_assist(&BuildMinimapAssistPanelModel {
            mode: BuildInteractionMode::Idle,
            selection_state: BuildInteractionSelectionState::Unarmed,
            queue_state: BuildInteractionQueueState::Empty,
            place_ready: false,
            config_family_count: 0,
            config_sample_count: 0,
            top_config_family: None,
            authority_state: BuildInteractionAuthorityState::None,
            focus_tile: None,
            focus_in_window: None,
            visible_map_percent: 0,
            unknown_tile_percent: 100,
            window_coverage_percent: 0,
            tracked_object_count: 0,
            runtime_count: 0,
        });
        assert_eq!(
            idle,
            BuildUserFlowPanelModel {
                next_action: "idle",
                blockers: Vec::new(),
                route: vec!["idle"],
            }
        );
    }
}
