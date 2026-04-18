use crate::minimap_user_flow::{
    build_minimap_user_flow_panel, MinimapPanAxisDirection, MinimapUserFocusState,
    MinimapUserTargetKind,
};
use crate::panel_model::{
    build_build_interaction_panel, build_build_minimap_assist_panel,
    BuildInteractionAuthorityState, BuildInteractionMode, BuildInteractionQueueState,
    BuildInteractionSelectionState, BuildMinimapAssistPanelModel, PresenterViewWindow,
};
use crate::{HudModel, RenderModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildUserFlowBlocker {
    Arm,
    Missing,
    Realign,
    Resolve,
    Refocus,
    Survey,
}

impl BuildUserFlowBlocker {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Arm => "arm",
            Self::Missing => "missing",
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
    pub minimap_next_action: &'static str,
    pub focus_state: MinimapUserFocusState,
    pub pan_horizontal: MinimapPanAxisDirection,
    pub pan_vertical: MinimapPanAxisDirection,
    pub target_kind: MinimapUserTargetKind,
    pub config_scope: &'static str,
    pub authority_state: BuildInteractionAuthorityState,
    pub authority_pending_match: Option<bool>,
    pub authority_source: Option<crate::BuildConfigAuthoritySourceObservability>,
    pub authority_block_name: Option<String>,
    pub head_tile: Option<(i32, i32)>,
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

    fn join_or_none(labels: &[&'static str]) -> String {
        if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join("+")
        }
    }

    fn blocker_and_route_labels(&self) -> (String, String) {
        let blocker_labels = self.blocker_labels();
        (
            Self::join_or_none(&blocker_labels),
            Self::join_or_none(&self.route),
        )
    }

    pub(crate) fn pan_label(&self) -> &'static str {
        match (self.pan_horizontal, self.pan_vertical) {
            (MinimapPanAxisDirection::None, MinimapPanAxisDirection::None) => "hold",
            (MinimapPanAxisDirection::None, vertical) => vertical.label(),
            (horizontal, MinimapPanAxisDirection::None) => horizontal.label(),
            (horizontal, vertical) => match (horizontal, vertical) {
                (MinimapPanAxisDirection::Left, MinimapPanAxisDirection::Up) => "left+up",
                (MinimapPanAxisDirection::Left, MinimapPanAxisDirection::Down) => "left+down",
                (MinimapPanAxisDirection::Right, MinimapPanAxisDirection::Up) => "right+up",
                (MinimapPanAxisDirection::Right, MinimapPanAxisDirection::Down) => "right+down",
                _ => "hold",
            },
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn next_minimap_label_prefix(&self) -> String {
        format!(
            "next={} minimap={}",
            self.next_action, self.minimap_next_action
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn summary_label(&self) -> String {
        format!(
            "{} focus={} pan={} target={} scope={}",
            self.next_minimap_label_prefix(),
            self.focus_state.label(),
            self.pan_label(),
            self.target_kind.label(),
            self.config_scope,
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn detail_label(&self) -> String {
        let (blockers, route) = self.blocker_and_route_labels();
        let head = self
            .head_tile
            .map_or_else(|| "none".to_string(), |(x, y)| format!("{x},{y}"));

        format!(
            "{} focus={} pan={} target={} scope={} route={} authority={} pending={} blockers={} src={} block={} head={}",
            self.next_minimap_label_prefix(),
            self.focus_state.label(),
            self.pan_label(),
            self.target_kind.label(),
            self.config_scope,
            route,
            self.authority_state_label(),
            self.authority_pending_match_label(),
            blockers,
            self.authority_source_label(),
            self.authority_block_name
                .as_deref()
                .unwrap_or("none"),
            head,
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn route_detail_label(&self) -> String {
        let (blockers, route) = self.blocker_and_route_labels();

        format!(
            "{} blockers={} route={}",
            self.next_minimap_label_prefix(),
            blockers,
            route
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn authority_state_label(&self) -> &'static str {
        match self.authority_state {
            BuildInteractionAuthorityState::None => "none",
            BuildInteractionAuthorityState::Applied => "applied",
            BuildInteractionAuthorityState::Cleared => "cleared",
            BuildInteractionAuthorityState::Rollback => "rollback",
            BuildInteractionAuthorityState::RejectedMissingBuilding => "rejected-missing-building",
            BuildInteractionAuthorityState::RejectedMissingBlockMetadata => {
                "rejected-missing-block-metadata"
            }
            BuildInteractionAuthorityState::RejectedUnsupportedBlock => {
                "rejected-unsupported-block"
            }
            BuildInteractionAuthorityState::RejectedUnsupportedConfigType => {
                "rejected-unsupported-config-type"
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn authority_pending_match_label(&self) -> &'static str {
        match self.authority_pending_match {
            Some(true) => "match",
            Some(false) => "mismatch",
            None => "none",
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn authority_source_label(&self) -> &'static str {
        match self.authority_source {
            Some(crate::BuildConfigAuthoritySourceObservability::TileConfig) => "tileConfig",
            Some(crate::BuildConfigAuthoritySourceObservability::ConstructFinish) => {
                "constructFinish"
            }
            None => "none",
        }
    }
}

pub(crate) fn build_build_user_flow_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<BuildUserFlowPanelModel> {
    let assist = build_build_minimap_assist_panel(scene, hud, window);
    let minimap = build_minimap_user_flow_panel(scene, hud, window);
    let interaction = build_build_interaction_panel(hud);
    if assist.is_none() && minimap.is_none() && interaction.is_none() {
        return None;
    }
    Some(build_user_flow_from_panel_options(
        assist.as_ref(),
        minimap.as_ref(),
        interaction.as_ref(),
    ))
}

fn build_user_flow_from_panel_options(
    assist: Option<&BuildMinimapAssistPanelModel>,
    minimap: Option<&crate::minimap_user_flow::MinimapUserFlowPanelModel>,
    interaction: Option<&crate::panel_model::BuildInteractionPanelModel>,
) -> BuildUserFlowPanelModel {
    let missing = assist.is_none() || minimap.is_none() || interaction.is_none();

    let (next_action, mut blockers, mut route, config_scope) = if let Some(assist) = assist {
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

        (
            assist.next_action_label(),
            blockers,
            route,
            assist.config_scope_label(),
        )
    } else {
        (
            "missing",
            vec![BuildUserFlowBlocker::Missing],
            vec!["missing"],
            "missing",
        )
    };

    let next_action = if missing { "missing" } else { next_action };

    if missing {
        blockers = vec![BuildUserFlowBlocker::Missing];
        route = vec!["missing"];
    }

    BuildUserFlowPanelModel {
        next_action,
        blockers,
        route,
        minimap_next_action: minimap.map_or("missing", |minimap| minimap.next_action),
        focus_state: minimap.map_or(MinimapUserFocusState::Missing, |minimap| {
            minimap.focus_state
        }),
        pan_horizontal: minimap.map_or(MinimapPanAxisDirection::None, |minimap| {
            minimap.pan_horizontal
        }),
        pan_vertical: minimap.map_or(MinimapPanAxisDirection::None, |minimap| {
            minimap.pan_vertical
        }),
        target_kind: minimap.map_or(MinimapUserTargetKind::None, |minimap| minimap.target_kind),
        config_scope,
        authority_state: interaction.map_or(BuildInteractionAuthorityState::None, |interaction| {
            interaction.authority_state
        }),
        authority_pending_match: interaction
            .and_then(|interaction| interaction.authority_pending_match),
        authority_source: interaction.and_then(|interaction| interaction.authority_source),
        authority_block_name: interaction
            .and_then(|interaction| interaction.authority_block_name.clone()),
        head_tile: interaction
            .as_ref()
            .and_then(|interaction| interaction.head.as_ref().map(|head| (head.x, head.y))),
    }
}

#[cfg(test)]
fn build_user_flow_from_panels(
    assist: &BuildMinimapAssistPanelModel,
    minimap: &crate::minimap_user_flow::MinimapUserFlowPanelModel,
    interaction: &crate::panel_model::BuildInteractionPanelModel,
) -> BuildUserFlowPanelModel {
    build_user_flow_from_panel_options(Some(assist), Some(minimap), Some(interaction))
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
            if authority_needs_attention(assist.authority_state)
                || authority_pending_match_needs_attention(assist.authority_pending_match)
            {
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

fn authority_pending_match_needs_attention(value: Option<bool>) -> bool {
    value == Some(false)
}

fn focus_needs_refocus(assist: &BuildMinimapAssistPanelModel) -> bool {
    assist.focus_tile.is_none() || assist.focus_in_window != Some(true)
}

fn push_route_step(route: &mut Vec<&'static str>, step: &'static str) {
    if route.last().copied() != Some(step) {
        route.push(step);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_build_user_flow_panel, build_user_flow_from_panels, BuildUserFlowBlocker,
        BuildUserFlowPanelModel,
    };
    use crate::minimap_user_flow::{
        MinimapPanAxisDirection, MinimapUserFlowPanelModel, MinimapUserFocusState,
        MinimapUserTargetKind,
    };
    use crate::panel_model::{
        BuildConfigHeadModel, BuildInteractionAuthorityState, BuildInteractionMode,
        BuildInteractionPanelModel, BuildInteractionQueueState, BuildInteractionSelectionState,
        BuildMinimapAssistPanelModel, PresenterViewWindow,
    };
    use crate::{HudModel, RenderModel};

    #[test]
    fn build_build_user_flow_panel_returns_none_for_empty_default_inputs() {
        let panel = build_build_user_flow_panel(
            &RenderModel::default(),
            &HudModel::default(),
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 0,
                height: 0,
            },
        );

        assert!(panel.is_none());
    }

    fn empty_flow_panel() -> BuildUserFlowPanelModel {
        BuildUserFlowPanelModel {
            next_action: "hold",
            blockers: Vec::new(),
            route: Vec::new(),
            minimap_next_action: "inspect",
            focus_state: MinimapUserFocusState::Inside,
            pan_horizontal: MinimapPanAxisDirection::None,
            pan_vertical: MinimapPanAxisDirection::None,
            target_kind: MinimapUserTargetKind::Plan,
            config_scope: "single",
            authority_state: BuildInteractionAuthorityState::None,
            authority_pending_match: None,
            authority_source: None,
            authority_block_name: None,
            head_tile: None,
        }
    }

    #[test]
    fn build_user_flow_route_detail_label_formats_empty_blockers_and_route_with_stable_prefix() {
        let panel = empty_flow_panel();
        assert_eq!(
            panel.route_detail_label(),
            "next=hold minimap=inspect blockers=none route=none"
        );
    }

    #[test]
    fn build_user_flow_route_tracks_ordered_place_blockers_and_commit_path() {
        let panel = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadDiverged,
                queue_state: BuildInteractionQueueState::Queued,
                place_ready: true,
                config_family_count: 2,
                config_sample_count: 5,
                top_config_family: Some("power-node".to_string()),
                authority_state: BuildInteractionAuthorityState::Rollback,
                authority_pending_match: None,
                head_tile: Some((12, 18)),
                authority_tile: Some((12, 18)),
                authority_source: None,
                authority_block_name: None,
                focus_tile: Some((12, 18)),
                focus_in_window: Some(false),
                visible_map_percent: 0,
                unknown_tile_percent: 100,
                window_coverage_percent: 25,
                tracked_object_count: 8,
                runtime_count: 2,
            },
            &MinimapUserFlowPanelModel {
                next_action: "pan",
                focus_state: MinimapUserFocusState::Outside,
                pan_horizontal: MinimapPanAxisDirection::Right,
                pan_vertical: MinimapPanAxisDirection::Down,
                target_kind: MinimapUserTargetKind::Plan,
                focus_tile: Some((12, 18)),
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: true,
                window_clamped_bottom: true,
                focus_offset_x: Some(4),
                focus_offset_y: Some(4),
                overlay_target_count: 3,
                visible_tile_count: 0,
                visible_map_percent: 0,
                unknown_tile_percent: 100,
                window_coverage_percent: 25,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadDiverged,
                queue_state: BuildInteractionQueueState::Queued,
                selected_block_id: Some(301),
                selected_rotation: 0,
                pending_count: 1,
                orphan_authoritative_count: 0,
                place_ready: true,
                config_available: true,
                config_family_count: 2,
                config_sample_count: 5,
                top_config_family: Some("power-node".to_string()),
                head: Some(BuildConfigHeadModel {
                    x: 12,
                    y: 18,
                    breaking: false,
                    block_id: Some(301),
                    rotation: Some(0),
                    stage: crate::BuildQueueHeadStage::Queued,
                }),
                authority_state: BuildInteractionAuthorityState::Rollback,
                authority_pending_match: Some(false),
                authority_source: None,
                authority_tile: Some((12, 18)),
                authority_block_name: Some("power-node".to_string()),
            },
        );

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
        assert_eq!(
            panel.summary_label(),
            "next=realign minimap=pan focus=outside pan=right+down target=plan scope=multi"
        );
        assert_eq!(
            panel.detail_label(),
            "next=realign minimap=pan focus=outside pan=right+down target=plan scope=multi route=realign+resolve+refocus+survey+commit authority=rollback pending=mismatch blockers=realign+resolve+refocus+survey src=none block=power-node head=12,18"
        );
        assert_eq!(
            panel.route_detail_label(),
            "next=realign minimap=pan blockers=realign+resolve+refocus+survey route=realign+resolve+refocus+survey+commit"
        );
        assert_eq!(panel.minimap_next_action, "pan");
        assert_eq!(panel.focus_state, MinimapUserFocusState::Outside);
        assert_eq!(panel.pan_label(), "right+down");
        assert_eq!(panel.target_kind, MinimapUserTargetKind::Plan);
        assert_eq!(panel.config_scope, "multi");
        assert_eq!(
            panel.authority_state,
            BuildInteractionAuthorityState::Rollback
        );
        assert_eq!(panel.head_tile, Some((12, 18)));
    }

    #[test]
    fn build_user_flow_route_adds_seed_before_commit_for_ready_empty_queue() {
        let panel = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Empty,
                place_ready: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: None,
                head_tile: None,
                authority_tile: None,
                authority_source: None,
                authority_block_name: None,
                focus_tile: Some((4, 6)),
                focus_in_window: Some(true),
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 40,
                tracked_object_count: 3,
                runtime_count: 0,
            },
            &MinimapUserFlowPanelModel {
                next_action: "inspect",
                focus_state: MinimapUserFocusState::Inside,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::Marker,
                focus_tile: Some((4, 6)),
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: Some(0),
                focus_offset_y: Some(0),
                overlay_target_count: 1,
                visible_tile_count: 40,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 40,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Empty,
                selected_block_id: Some(1),
                selected_rotation: 0,
                pending_count: 0,
                orphan_authoritative_count: 0,
                place_ready: true,
                config_available: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                head: None,
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: Some(true),
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            },
        );

        assert_eq!(panel.next_action, "seed");
        assert!(panel.blockers.is_empty());
        assert_eq!(panel.route, vec!["seed", "commit"]);
        assert_eq!(panel.authority_pending_match, Some(true));
        assert_eq!(panel.authority_source, None);
        assert_eq!(panel.authority_block_name, None);
        assert_eq!(panel.minimap_next_action, "inspect");
        assert_eq!(panel.focus_state, MinimapUserFocusState::Inside);
        assert_eq!(panel.pan_label(), "hold");
        assert_eq!(panel.target_kind, MinimapUserTargetKind::Marker);
        assert_eq!(panel.config_scope, "single");
    }

    #[test]
    fn build_user_flow_treats_pending_mismatch_as_resolve_blocker_when_authority_is_applied() {
        let panel = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Queued,
                place_ready: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: Some(false),
                head_tile: Some((4, 6)),
                authority_tile: Some((4, 6)),
                authority_source: None,
                authority_block_name: None,
                focus_tile: Some((4, 6)),
                focus_in_window: Some(true),
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 40,
                tracked_object_count: 3,
                runtime_count: 0,
            },
            &MinimapUserFlowPanelModel {
                next_action: "inspect",
                focus_state: MinimapUserFocusState::Inside,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::Marker,
                focus_tile: Some((4, 6)),
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: Some(0),
                focus_offset_y: Some(0),
                overlay_target_count: 1,
                visible_tile_count: 40,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 40,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Queued,
                selected_block_id: Some(1),
                selected_rotation: 0,
                pending_count: 1,
                orphan_authoritative_count: 0,
                place_ready: true,
                config_available: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                head: Some(BuildConfigHeadModel {
                    x: 4,
                    y: 6,
                    breaking: false,
                    block_id: Some(1),
                    rotation: Some(0),
                    stage: crate::BuildQueueHeadStage::Queued,
                }),
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: Some(false),
                authority_source: None,
                authority_tile: Some((4, 6)),
                authority_block_name: Some("message".to_string()),
            },
        );

        assert_eq!(panel.next_action, "resolve");
        assert_eq!(panel.blocker_labels(), vec!["resolve"]);
        assert_eq!(panel.route, vec!["resolve", "commit"]);
        assert_eq!(panel.authority_pending_match, Some(false));
        assert_eq!(panel.authority_block_name.as_deref(), Some("message"));
        assert_eq!(
            panel.detail_label(),
            "next=resolve minimap=inspect focus=inside pan=hold target=marker scope=single route=resolve+commit authority=applied pending=mismatch blockers=resolve src=none block=message head=4,6"
        );
        assert_eq!(
            panel.route_detail_label(),
            "next=resolve minimap=inspect blockers=resolve route=resolve+commit"
        );
    }

    fn build_user_flow_panel_with_pending_match(
        authority_pending_match: Option<bool>,
    ) -> BuildUserFlowPanelModel {
        BuildUserFlowPanelModel {
            next_action: "noop",
            blockers: Vec::new(),
            route: Vec::new(),
            minimap_next_action: "noop",
            focus_state: MinimapUserFocusState::Inside,
            pan_horizontal: MinimapPanAxisDirection::None,
            pan_vertical: MinimapPanAxisDirection::None,
            target_kind: MinimapUserTargetKind::None,
            config_scope: "single",
            authority_state: BuildInteractionAuthorityState::None,
            authority_pending_match,
            authority_source: None,
            authority_block_name: None,
            head_tile: None,
        }
    }

    #[test]
    fn authority_pending_match_label_maps_none_match_and_mismatch() {
        let none = build_user_flow_panel_with_pending_match(None);
        let match_panel = build_user_flow_panel_with_pending_match(Some(true));
        let mismatch = build_user_flow_panel_with_pending_match(Some(false));

        assert_eq!(none.authority_pending_match_label(), "none");
        assert_eq!(match_panel.authority_pending_match_label(), "match");
        assert_eq!(mismatch.authority_pending_match_label(), "mismatch");
    }

    #[test]
    fn build_user_flow_pan_label_maps_hold_and_single_axis_directions() {
        let cases = [
            (
                MinimapPanAxisDirection::None,
                MinimapPanAxisDirection::None,
                "hold",
            ),
            (
                MinimapPanAxisDirection::None,
                MinimapPanAxisDirection::Up,
                "up",
            ),
            (
                MinimapPanAxisDirection::Right,
                MinimapPanAxisDirection::None,
                "right",
            ),
        ];

        for (pan_horizontal, pan_vertical, expected) in cases {
            let mut panel = empty_flow_panel();
            panel.pan_horizontal = pan_horizontal;
            panel.pan_vertical = pan_vertical;

            assert_eq!(panel.pan_label(), expected);
        }
    }

    #[test]
    fn build_user_flow_authority_labels_map_state_and_pending_variants() {
        let cases = [
            (
                BuildInteractionAuthorityState::None,
                None,
                "none",
                "none",
            ),
            (
                BuildInteractionAuthorityState::Applied,
                Some(true),
                "applied",
                "match",
            ),
            (
                BuildInteractionAuthorityState::Cleared,
                Some(false),
                "cleared",
                "mismatch",
            ),
            (
                BuildInteractionAuthorityState::Rollback,
                Some(false),
                "rollback",
                "mismatch",
            ),
        ];

        for (authority_state, authority_pending_match, expected_state, expected_pending) in cases {
            let mut panel = empty_flow_panel();
            panel.authority_state = authority_state;
            panel.authority_pending_match = authority_pending_match;

            assert_eq!(panel.authority_state_label(), expected_state);
            assert_eq!(panel.authority_pending_match_label(), expected_pending);
        }
    }

    #[test]
    fn build_user_flow_unknown_focus_window_state_requires_refocus() {
        let panel = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Break,
                selection_state: BuildInteractionSelectionState::BreakingHead,
                queue_state: BuildInteractionQueueState::Queued,
                place_ready: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: None,
                head_tile: Some((9, 7)),
                authority_tile: None,
                authority_source: None,
                authority_block_name: None,
                focus_tile: Some((9, 7)),
                focus_in_window: None,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 50,
                tracked_object_count: 2,
                runtime_count: 0,
            },
            &MinimapUserFlowPanelModel {
                next_action: "locate",
                focus_state: MinimapUserFocusState::Missing,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::None,
                focus_tile: Some((9, 7)),
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: None,
                focus_offset_y: None,
                overlay_target_count: 0,
                visible_tile_count: 50,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 50,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Break,
                selection_state: BuildInteractionSelectionState::BreakingHead,
                queue_state: BuildInteractionQueueState::Queued,
                selected_block_id: None,
                selected_rotation: 0,
                pending_count: 1,
                orphan_authoritative_count: 0,
                place_ready: false,
                config_available: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                head: Some(BuildConfigHeadModel {
                    x: 9,
                    y: 7,
                    breaking: true,
                    block_id: None,
                    rotation: None,
                    stage: crate::BuildQueueHeadStage::Queued,
                }),
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: None,
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            },
        );

        assert_eq!(panel.blocker_labels(), vec!["refocus"]);
        assert_eq!(panel.route, vec!["refocus", "break"]);
    }

    #[test]
    fn build_user_flow_route_stays_bounded_for_unarmed_break_and_idle_states() {
        let place_arm = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::Unarmed,
                queue_state: BuildInteractionQueueState::Empty,
                place_ready: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                authority_state: BuildInteractionAuthorityState::None,
                authority_pending_match: None,
                head_tile: None,
                authority_tile: None,
                authority_source: None,
                authority_block_name: None,
                focus_tile: Some((1, 1)),
                focus_in_window: Some(true),
                visible_map_percent: 50,
                unknown_tile_percent: 50,
                window_coverage_percent: 10,
                tracked_object_count: 1,
                runtime_count: 0,
            },
            &MinimapUserFlowPanelModel {
                next_action: "hold",
                focus_state: MinimapUserFocusState::Inside,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::None,
                focus_tile: Some((1, 1)),
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: Some(0),
                focus_offset_y: Some(0),
                overlay_target_count: 0,
                visible_tile_count: 50,
                visible_map_percent: 50,
                unknown_tile_percent: 50,
                window_coverage_percent: 10,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::Unarmed,
                queue_state: BuildInteractionQueueState::Empty,
                selected_block_id: None,
                selected_rotation: 0,
                pending_count: 0,
                orphan_authoritative_count: 0,
                place_ready: false,
                config_available: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                head: None,
                authority_state: BuildInteractionAuthorityState::None,
                authority_pending_match: None,
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            },
        );
        assert_eq!(place_arm.next_action, "arm");
        assert_eq!(place_arm.blocker_labels(), vec!["arm"]);
        assert_eq!(place_arm.route, vec!["arm"]);

        let break_refocus = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Break,
                selection_state: BuildInteractionSelectionState::BreakingHead,
                queue_state: BuildInteractionQueueState::Queued,
                place_ready: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: None,
                head_tile: Some((4, 5)),
                authority_tile: None,
                authority_source: None,
                authority_block_name: None,
                focus_tile: None,
                focus_in_window: None,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 100,
                tracked_object_count: 1,
                runtime_count: 0,
            },
            &MinimapUserFlowPanelModel {
                next_action: "locate",
                focus_state: MinimapUserFocusState::Missing,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::None,
                focus_tile: None,
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: None,
                focus_offset_y: None,
                overlay_target_count: 0,
                visible_tile_count: 100,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 100,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Break,
                selection_state: BuildInteractionSelectionState::BreakingHead,
                queue_state: BuildInteractionQueueState::Queued,
                selected_block_id: None,
                selected_rotation: 0,
                pending_count: 1,
                orphan_authoritative_count: 0,
                place_ready: false,
                config_available: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                head: Some(BuildConfigHeadModel {
                    x: 4,
                    y: 5,
                    breaking: true,
                    block_id: None,
                    rotation: None,
                    stage: crate::BuildQueueHeadStage::Queued,
                }),
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: None,
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            },
        );
        assert_eq!(break_refocus.next_action, "refocus");
        assert_eq!(break_refocus.blocker_labels(), vec!["refocus"]);
        assert_eq!(break_refocus.route, vec!["refocus", "break"]);

        let idle = build_user_flow_from_panels(
            &BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Idle,
                selection_state: BuildInteractionSelectionState::Unarmed,
                queue_state: BuildInteractionQueueState::Empty,
                place_ready: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                authority_state: BuildInteractionAuthorityState::None,
                authority_pending_match: None,
                head_tile: None,
                authority_tile: None,
                authority_source: None,
                authority_block_name: None,
                focus_tile: None,
                focus_in_window: None,
                visible_map_percent: 0,
                unknown_tile_percent: 100,
                window_coverage_percent: 0,
                tracked_object_count: 0,
                runtime_count: 0,
            },
            &MinimapUserFlowPanelModel {
                next_action: "locate",
                focus_state: MinimapUserFocusState::Missing,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::None,
                focus_tile: None,
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: None,
                focus_offset_y: None,
                overlay_target_count: 0,
                visible_tile_count: 0,
                visible_map_percent: 0,
                unknown_tile_percent: 100,
                window_coverage_percent: 0,
            },
            &BuildInteractionPanelModel {
                mode: BuildInteractionMode::Idle,
                selection_state: BuildInteractionSelectionState::Unarmed,
                queue_state: BuildInteractionQueueState::Empty,
                selected_block_id: None,
                selected_rotation: 0,
                pending_count: 0,
                orphan_authoritative_count: 0,
                place_ready: false,
                config_available: false,
                config_family_count: 0,
                config_sample_count: 0,
                top_config_family: None,
                head: None,
                authority_state: BuildInteractionAuthorityState::None,
                authority_pending_match: None,
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            },
        );
        assert_eq!(
            idle,
            BuildUserFlowPanelModel {
                next_action: "idle",
                blockers: Vec::new(),
                route: vec!["idle"],
                minimap_next_action: "locate",
                focus_state: MinimapUserFocusState::Missing,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::None,
                config_scope: "none",
                authority_state: BuildInteractionAuthorityState::None,
                authority_pending_match: None,
                authority_source: None,
                authority_block_name: None,
                head_tile: None,
            }
        );
    }

    #[test]
    fn build_build_user_flow_panel_preserves_missing_state() {
        let panel = super::build_user_flow_from_panel_options(
            None,
            Some(&MinimapUserFlowPanelModel {
                next_action: "inspect",
                focus_state: MinimapUserFocusState::Inside,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::Marker,
                focus_tile: Some((4, 6)),
                window_clamped_left: false,
                window_clamped_top: false,
                window_clamped_right: false,
                window_clamped_bottom: false,
                focus_offset_x: Some(0),
                focus_offset_y: Some(0),
                overlay_target_count: 1,
                visible_tile_count: 40,
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 40,
            }),
            Some(&BuildInteractionPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Empty,
                selected_block_id: Some(1),
                selected_rotation: 0,
                pending_count: 0,
                orphan_authoritative_count: 0,
                place_ready: true,
                config_available: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                head: None,
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: Some(true),
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            }),
        );

        assert_eq!(panel.next_action, "missing");
        assert_eq!(panel.blocker_labels(), vec!["missing"]);
        assert_eq!(panel.route, vec!["missing"]);
        assert_eq!(panel.minimap_next_action, "inspect");
        assert_eq!(panel.focus_state, MinimapUserFocusState::Inside);
        assert_eq!(panel.target_kind, MinimapUserTargetKind::Marker);
        assert_eq!(panel.config_scope, "missing");
        assert_eq!(
            panel.authority_state,
            BuildInteractionAuthorityState::Applied
        );
    }

    #[test]
    fn build_build_user_flow_panel_marks_partial_missing_state_as_missing() {
        let panel = super::build_user_flow_from_panel_options(
            Some(&BuildMinimapAssistPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Empty,
                place_ready: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: None,
                head_tile: None,
                authority_tile: None,
                authority_source: None,
                authority_block_name: None,
                focus_tile: Some((4, 6)),
                focus_in_window: Some(true),
                visible_map_percent: 100,
                unknown_tile_percent: 0,
                window_coverage_percent: 40,
                tracked_object_count: 3,
                runtime_count: 0,
            }),
            None,
            Some(&BuildInteractionPanelModel {
                mode: BuildInteractionMode::Place,
                selection_state: BuildInteractionSelectionState::HeadAligned,
                queue_state: BuildInteractionQueueState::Empty,
                selected_block_id: Some(1),
                selected_rotation: 0,
                pending_count: 0,
                orphan_authoritative_count: 0,
                place_ready: true,
                config_available: true,
                config_family_count: 1,
                config_sample_count: 1,
                top_config_family: Some("message".to_string()),
                head: None,
                authority_state: BuildInteractionAuthorityState::Applied,
                authority_pending_match: Some(true),
                authority_source: None,
                authority_tile: None,
                authority_block_name: None,
            }),
        );

        assert_eq!(panel.next_action, "missing");
        assert_eq!(panel.blocker_labels(), vec!["missing"]);
        assert_eq!(panel.route, vec!["missing"]);
        assert_eq!(
            panel.summary_label(),
            "next=missing minimap=missing focus=missing pan=hold target=none scope=single"
        );
        assert_eq!(
            panel.detail_label(),
            "next=missing minimap=missing focus=missing pan=hold target=none scope=single route=missing authority=applied pending=match blockers=missing src=none block=none head=none"
        );
        assert_eq!(
            panel.route_detail_label(),
            "next=missing minimap=missing blockers=missing route=missing"
        );
        assert_eq!(panel.minimap_next_action, "missing");
    }

    #[test]
    fn authority_source_label_maps_none_and_high_signal_sources() {
        let cases = [
            (None, "none"),
            (
                Some(crate::BuildConfigAuthoritySourceObservability::TileConfig),
                "tileConfig",
            ),
            (
                Some(crate::BuildConfigAuthoritySourceObservability::ConstructFinish),
                "constructFinish",
            ),
        ];

        for (authority_source, expected) in cases {
            let panel = BuildUserFlowPanelModel {
                next_action: "",
                blockers: Vec::new(),
                route: Vec::new(),
                minimap_next_action: "",
                focus_state: MinimapUserFocusState::Missing,
                pan_horizontal: MinimapPanAxisDirection::None,
                pan_vertical: MinimapPanAxisDirection::None,
                target_kind: MinimapUserTargetKind::None,
                config_scope: "",
                authority_state: BuildInteractionAuthorityState::None,
                authority_pending_match: None,
                authority_source,
                authority_block_name: None,
                head_tile: None,
            };

            assert_eq!(panel.authority_source_label(), expected);
        }
    }

    #[test]
    fn build_user_flow_blocker_labels_map_all_variants_stably() {
        let cases = [
            (BuildUserFlowBlocker::Arm, "arm"),
            (BuildUserFlowBlocker::Missing, "missing"),
            (BuildUserFlowBlocker::Realign, "realign"),
            (BuildUserFlowBlocker::Resolve, "resolve"),
            (BuildUserFlowBlocker::Refocus, "refocus"),
            (BuildUserFlowBlocker::Survey, "survey"),
        ];

        for (blocker, expected) in cases {
            assert_eq!(blocker.label(), expected);
        }

        let panel = BuildUserFlowPanelModel {
            next_action: "",
            blockers: vec![
                BuildUserFlowBlocker::Arm,
                BuildUserFlowBlocker::Missing,
                BuildUserFlowBlocker::Realign,
                BuildUserFlowBlocker::Resolve,
                BuildUserFlowBlocker::Refocus,
                BuildUserFlowBlocker::Survey,
            ],
            route: Vec::new(),
            minimap_next_action: "",
            focus_state: MinimapUserFocusState::Missing,
            pan_horizontal: MinimapPanAxisDirection::None,
            pan_vertical: MinimapPanAxisDirection::None,
            target_kind: MinimapUserTargetKind::None,
            config_scope: "",
            authority_state: BuildInteractionAuthorityState::None,
            authority_pending_match: None,
            authority_source: None,
            authority_block_name: None,
            head_tile: None,
        };

        assert_eq!(
            panel.blocker_labels(),
            vec!["arm", "missing", "realign", "resolve", "refocus", "survey"]
        );
    }

    #[test]
    fn build_user_flow_summary_label_keeps_prefix_pan_and_authority_stable() {
        let panel = BuildUserFlowPanelModel {
            next_action: "survey",
            blockers: vec![BuildUserFlowBlocker::Resolve],
            route: vec!["resolve", "commit"],
            minimap_next_action: "seek",
            focus_state: MinimapUserFocusState::Outside,
            pan_horizontal: MinimapPanAxisDirection::Left,
            pan_vertical: MinimapPanAxisDirection::Up,
            target_kind: MinimapUserTargetKind::Plan,
            config_scope: "multi",
            authority_state: BuildInteractionAuthorityState::Cleared,
            authority_pending_match: Some(true),
            authority_source: None,
            authority_block_name: Some("router".to_string()),
            head_tile: Some((2, 3)),
        };

        assert_eq!(
            panel.pan_label(),
            "left+up"
        );
        assert_eq!(
            panel.summary_label(),
            "next=survey minimap=seek focus=outside pan=left+up target=plan scope=multi"
        );
        assert_eq!(
            panel.detail_label(),
            "next=survey minimap=seek focus=outside pan=left+up target=plan scope=multi route=resolve+commit authority=cleared pending=match blockers=resolve src=none block=router head=2,3"
        );
    }

    #[test]
    fn build_user_flow_attention_helpers_distinguish_passive_and_problem_states() {
        assert!(!super::authority_needs_attention(
            BuildInteractionAuthorityState::None
        ));
        assert!(!super::authority_needs_attention(
            BuildInteractionAuthorityState::Applied
        ));
        assert!(super::authority_needs_attention(
            BuildInteractionAuthorityState::RejectedMissingBuilding
        ));
        assert!(!super::authority_pending_match_needs_attention(None));
        assert!(!super::authority_pending_match_needs_attention(Some(true)));
        assert!(super::authority_pending_match_needs_attention(Some(false)));
    }

    #[test]
    fn join_or_none_formats_empty_and_ordered_labels_with_stable_prefix() {
        assert_eq!(BuildUserFlowPanelModel::join_or_none(&[]), "none");
        assert_eq!(
            BuildUserFlowPanelModel::join_or_none(&["alpha", "beta", "gamma"]),
            "alpha+beta+gamma"
        );

        let panel = BuildUserFlowPanelModel {
            next_action: "survey",
            blockers: Vec::new(),
            route: Vec::new(),
            minimap_next_action: "build",
            focus_state: MinimapUserFocusState::Inside,
            pan_horizontal: MinimapPanAxisDirection::None,
            pan_vertical: MinimapPanAxisDirection::None,
            target_kind: MinimapUserTargetKind::None,
            config_scope: "scope",
            authority_state: BuildInteractionAuthorityState::None,
            authority_pending_match: None,
            authority_source: None,
            authority_block_name: None,
            head_tile: None,
        };

        assert!(panel
            .summary_label()
            .starts_with("next=survey minimap=build "));
    }

    #[test]
    fn build_user_flow_count_helpers_stay_stable_for_empty_multi_and_route_cases() {
        let empty = empty_flow_panel();
        assert_eq!(empty.blocker_count(), 0);
        assert_eq!(empty.route_count(), 0);

        let multi_blocker = BuildUserFlowPanelModel {
            blockers: vec![
                BuildUserFlowBlocker::Arm,
                BuildUserFlowBlocker::Resolve,
                BuildUserFlowBlocker::Survey,
            ],
            route: Vec::new(),
            ..empty.clone()
        };
        assert_eq!(multi_blocker.blocker_count(), 3);
        assert_eq!(multi_blocker.route_count(), 0);

        let routed = BuildUserFlowPanelModel {
            blockers: vec![BuildUserFlowBlocker::Resolve],
            route: vec!["resolve", "commit"],
            ..empty
        };
        assert_eq!(routed.blocker_count(), 1);
        assert_eq!(routed.route_count(), 2);
    }
}
