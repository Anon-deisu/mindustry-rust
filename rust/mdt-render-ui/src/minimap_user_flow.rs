use crate::panel_model::{build_minimap_panel, PresenterViewWindow};
use crate::{HudModel, RenderModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MinimapUserFocusState {
    Inside,
    Outside,
    Missing,
}

impl MinimapUserFocusState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Inside => "inside",
            Self::Outside => "outside",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MinimapPanAxisDirection {
    None,
    Left,
    Right,
    Up,
    Down,
}

impl MinimapPanAxisDirection {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::None => "hold",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MinimapUserTargetKind {
    None,
    Plan,
    Marker,
    Runtime,
    Player,
}

impl MinimapUserTargetKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Plan => "plan",
            Self::Marker => "marker",
            Self::Runtime => "runtime",
            Self::Player => "player",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MinimapUserFlowPanelModel {
    pub next_action: &'static str,
    pub focus_state: MinimapUserFocusState,
    pub pan_horizontal: MinimapPanAxisDirection,
    pub pan_vertical: MinimapPanAxisDirection,
    pub target_kind: MinimapUserTargetKind,
    pub focus_tile: Option<(usize, usize)>,
    pub window_clamped_left: bool,
    pub window_clamped_top: bool,
    pub window_clamped_right: bool,
    pub window_clamped_bottom: bool,
    pub focus_offset_x: Option<isize>,
    pub focus_offset_y: Option<isize>,
    pub overlay_target_count: usize,
    pub visible_tile_count: usize,
    pub visible_map_percent: usize,
    pub unknown_tile_percent: usize,
    pub window_coverage_percent: usize,
}

impl MinimapUserFlowPanelModel {
    pub(crate) fn visibility_label(&self) -> &'static str {
        crate::panel_model::minimap_visibility_label(
            self.visible_tile_count,
            self.unknown_tile_percent,
        )
    }

    pub(crate) fn coverage_label(&self) -> &'static str {
        crate::panel_model::minimap_coverage_label(self.window_coverage_percent)
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

    fn focus_tile_label(&self) -> String {
        pair_label(self.focus_tile)
    }

    fn focus_offset_label(&self) -> String {
        pair_label(self.focus_offset_x.zip(self.focus_offset_y))
    }

    fn clamp_label(&self) -> String {
        let mut parts = String::new();
        push_clamp_flag(&mut parts, self.window_clamped_left, 'L');
        push_clamp_flag(&mut parts, self.window_clamped_top, 'T');
        push_clamp_flag(&mut parts, self.window_clamped_right, 'R');
        push_clamp_flag(&mut parts, self.window_clamped_bottom, 'B');
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts
        }
    }

    fn shared_prefix_label(&self) -> String {
        format!(
            "next={} focus={} vis={} cover={} pan={} target={}",
            self.next_action,
            self.focus_state.label(),
            self.visibility_label(),
            self.coverage_label(),
            self.pan_label(),
            self.target_kind.label(),
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn summary_label(&self) -> String {
        self.shared_prefix_label()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn detail_label(&self) -> String {
        format!(
            "{} tile={} offset={} clamp={} overlay-targets={} visible={} visible-map={} unknown={} window={}",
            self.shared_prefix_label(),
            self.focus_tile_label(),
            self.focus_offset_label(),
            self.clamp_label(),
            self.overlay_target_count,
            self.visible_tile_count,
            self.visible_map_percent,
            self.unknown_tile_percent,
            self.window_coverage_percent,
        )
    }
}

fn pair_label<T, U>(value: Option<(T, U)>) -> String
where
    T: std::fmt::Display,
    U: std::fmt::Display,
{
    value
        .map(|(x, y)| format!("{x}:{y}"))
        .unwrap_or_else(|| "none".to_string())
}

fn push_clamp_flag(parts: &mut String, enabled: bool, flag: char) {
    if enabled {
        parts.push(flag);
    }
}

pub(crate) fn build_minimap_user_flow_panel(
    scene: &RenderModel,
    hud: &HudModel,
    window: PresenterViewWindow,
) -> Option<MinimapUserFlowPanelModel> {
    let panel = build_minimap_panel(scene, hud, window)?;
    let focus_state = match panel.focus_in_window {
        Some(true) => MinimapUserFocusState::Inside,
        Some(false) => MinimapUserFocusState::Outside,
        None => MinimapUserFocusState::Missing,
    };
    let target_kind = if panel.plan_count > 0 {
        MinimapUserTargetKind::Plan
    } else if panel.marker_count > 0 {
        MinimapUserTargetKind::Marker
    } else if panel.runtime_count > 0 {
        MinimapUserTargetKind::Runtime
    } else if panel.player_count > 0 {
        MinimapUserTargetKind::Player
    } else {
        MinimapUserTargetKind::None
    };
    let visibility_label = panel.visibility_label();
    let next_action = match focus_state {
        MinimapUserFocusState::Missing => "locate",
        MinimapUserFocusState::Outside => "pan",
        MinimapUserFocusState::Inside if matches!(visibility_label, "unseen" | "hidden") => {
            "survey"
        }
        MinimapUserFocusState::Inside
            if matches!(
                target_kind,
                MinimapUserTargetKind::Plan
                    | MinimapUserTargetKind::Marker
                    | MinimapUserTargetKind::Runtime
            ) =>
        {
            "inspect"
        }
        MinimapUserFocusState::Inside => "hold",
    };

    Some(MinimapUserFlowPanelModel {
        next_action,
        focus_state,
        pan_horizontal: pan_horizontal_direction(&panel),
        pan_vertical: pan_vertical_direction(&panel),
        target_kind,
        focus_tile: panel.focus_tile,
        window_clamped_left: panel.window_clamped_left,
        window_clamped_top: panel.window_clamped_top,
        window_clamped_right: panel.window_clamped_right,
        window_clamped_bottom: panel.window_clamped_bottom,
        focus_offset_x: panel.focus_offset_x,
        focus_offset_y: panel.focus_offset_y,
        overlay_target_count: panel.plan_count + panel.marker_count + panel.runtime_count,
        visible_tile_count: panel.visible_tile_count,
        visible_map_percent: panel.visible_map_percent(),
        unknown_tile_percent: panel.unknown_tile_percent,
        window_coverage_percent: panel.window_coverage_percent,
    })
}

fn pan_horizontal_direction(
    panel: &crate::panel_model::MinimapPanelModel,
) -> MinimapPanAxisDirection {
    if !matches!(panel.focus_in_window, Some(false)) {
        return MinimapPanAxisDirection::None;
    }

    match panel.focus_offset_x.unwrap_or_default().cmp(&0) {
        std::cmp::Ordering::Less => MinimapPanAxisDirection::Left,
        std::cmp::Ordering::Greater => MinimapPanAxisDirection::Right,
        std::cmp::Ordering::Equal => MinimapPanAxisDirection::None,
    }
}

fn pan_vertical_direction(
    panel: &crate::panel_model::MinimapPanelModel,
) -> MinimapPanAxisDirection {
    if !matches!(panel.focus_in_window, Some(false)) {
        return MinimapPanAxisDirection::None;
    }

    match panel.focus_offset_y.unwrap_or_default().cmp(&0) {
        std::cmp::Ordering::Less => MinimapPanAxisDirection::Up,
        std::cmp::Ordering::Greater => MinimapPanAxisDirection::Down,
        std::cmp::Ordering::Equal => MinimapPanAxisDirection::None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_minimap_user_flow_panel, pan_horizontal_direction, pan_vertical_direction,
        MinimapPanAxisDirection, MinimapUserFlowPanelModel, MinimapUserFocusState,
        MinimapUserTargetKind,
    };
    use crate::hud_model::{HudMinimapSummary, HudSummary, HudViewWindowSummary};
    use crate::panel_model::{MinimapPanelModel, PresenterViewWindow};
    use crate::{HudModel, RenderModel, RenderObject, Viewport};

    fn flow_model(
        left: bool,
        top: bool,
        right: bool,
        bottom: bool,
    ) -> MinimapUserFlowPanelModel {
        MinimapUserFlowPanelModel {
            next_action: "hold",
            focus_state: MinimapUserFocusState::Missing,
            pan_horizontal: MinimapPanAxisDirection::None,
            pan_vertical: MinimapPanAxisDirection::None,
            target_kind: MinimapUserTargetKind::None,
            focus_tile: None,
            window_clamped_left: left,
            window_clamped_top: top,
            window_clamped_right: right,
            window_clamped_bottom: bottom,
            focus_offset_x: None,
            focus_offset_y: None,
            overlay_target_count: 0,
            visible_tile_count: 0,
            visible_map_percent: 0,
            unknown_tile_percent: 0,
            window_coverage_percent: 0,
        }
    }

    fn minimap_panel(
        focus_in_window: Option<bool>,
        focus_offset_x: Option<isize>,
        focus_offset_y: Option<isize>,
    ) -> MinimapPanelModel {
        MinimapPanelModel {
            map_width: 10,
            map_height: 10,
            window: PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 4,
            },
            window_last_x: 0,
            window_last_y: 0,
            window_clamped_left: false,
            window_clamped_top: false,
            window_clamped_right: false,
            window_clamped_bottom: false,
            window_tile_count: 0,
            window_coverage_percent: 0,
            map_tile_count: 0,
            known_tile_count: 0,
            known_tile_percent: 0,
            unknown_tile_count: 0,
            unknown_tile_percent: 0,
            focus_tile: None,
            focus_in_window,
            focus_offset_x,
            focus_offset_y,
            overlay_visible: false,
            fog_enabled: false,
            visible_tile_count: 0,
            visible_known_percent: 0,
            hidden_tile_count: 0,
            hidden_known_percent: 0,
            tracked_object_count: 0,
            window_tracked_object_count: 0,
            outside_window_count: 0,
            player_count: 0,
            window_player_count: 0,
            marker_count: 0,
            window_marker_count: 0,
            plan_count: 0,
            window_plan_count: 0,
            block_count: 0,
            window_block_count: 0,
            runtime_count: 0,
            window_runtime_count: 0,
            terrain_count: 0,
            window_terrain_count: 0,
            unknown_count: 0,
            window_unknown_count: 0,
            detail_counts: Vec::new(),
        }
    }

    #[test]
    fn minimap_user_flow_clamp_label_orders_flags_and_handles_none() {
        assert_eq!(flow_model(false, false, false, false).clamp_label(), "none");
        assert_eq!(flow_model(true, false, false, false).clamp_label(), "L");
        assert_eq!(flow_model(false, true, false, false).clamp_label(), "T");
        assert_eq!(flow_model(false, false, true, false).clamp_label(), "R");
        assert_eq!(flow_model(false, false, false, true).clamp_label(), "B");
        assert_eq!(flow_model(true, true, true, true).clamp_label(), "LTRB");
        assert_eq!(flow_model(true, false, true, true).clamp_label(), "LRB");
    }

    #[test]
    fn minimap_user_flow_pan_directions_respect_offscreen_focus_signs() {
        let left_up = minimap_panel(Some(false), Some(-3), Some(-2));
        assert_eq!(
            pan_horizontal_direction(&left_up),
            MinimapPanAxisDirection::Left
        );
        assert_eq!(pan_vertical_direction(&left_up), MinimapPanAxisDirection::Up);

        let right_down = minimap_panel(Some(false), Some(5), Some(4));
        assert_eq!(
            pan_horizontal_direction(&right_down),
            MinimapPanAxisDirection::Right
        );
        assert_eq!(
            pan_vertical_direction(&right_down),
            MinimapPanAxisDirection::Down
        );

        let aligned = minimap_panel(Some(false), Some(0), Some(0));
        assert_eq!(
            pan_horizontal_direction(&aligned),
            MinimapPanAxisDirection::None
        );
        assert_eq!(
            pan_vertical_direction(&aligned),
            MinimapPanAxisDirection::None
        );

        let inside = minimap_panel(Some(true), Some(-8), Some(9));
        assert_eq!(
            pan_horizontal_direction(&inside),
            MinimapPanAxisDirection::None
        );
        assert_eq!(pan_vertical_direction(&inside), MinimapPanAxisDirection::None);
    }

    #[test]
    fn minimap_user_flow_prefers_pan_when_focus_is_offscreen() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 80.0,
                height: 80.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:1".to_string(),
                    layer: 2,
                    x: 56.0,
                    y: 56.0,
                },
                RenderObject {
                    id: "plan:build:1:7:7:301".to_string(),
                    layer: 1,
                    x: 56.0,
                    y: 56.0,
                },
            ],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 1,
                marker_count: 0,
                map_width: 10,
                map_height: 10,
                overlay_visible: true,
                fog_enabled: true,
                visible_tile_count: 10,
                hidden_tile_count: 10,
                minimap: HudMinimapSummary {
                    focus_tile: Some((7, 7)),
                    view_window: HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 4,
                        height: 4,
                    },
                },
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_user_flow_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 4,
            },
        )
        .expect("minimap user flow");

        assert_eq!(panel.next_action, "pan");
        assert_eq!(panel.focus_state, MinimapUserFocusState::Outside);
        assert_eq!(panel.pan_horizontal, MinimapPanAxisDirection::Right);
        assert_eq!(panel.pan_vertical, MinimapPanAxisDirection::Down);
        assert_eq!(panel.pan_label(), "right+down");
        assert_eq!(panel.target_kind, MinimapUserTargetKind::Plan);
        assert_eq!(panel.coverage_label(), "partial");
        assert_eq!(panel.visibility_label(), "mixed");
        assert!(panel.window_clamped_left);
        assert!(panel.window_clamped_top);
        assert!(!panel.window_clamped_right);
        assert!(!panel.window_clamped_bottom);
        assert_eq!(panel.focus_offset_x, Some(6));
        assert_eq!(panel.focus_offset_y, Some(6));
        assert_eq!(panel.overlay_target_count, 1);
    }

    #[test]
    fn minimap_user_flow_switches_between_locate_survey_inspect_and_hold() {
        let base_scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 1,
                x: 8.0,
                y: 8.0,
            }],
        };
        let summary = HudSummary {
            player_name: "operator".to_string(),
            team_id: 2,
            selected_block: "payload-router".to_string(),
            plan_count: 0,
            marker_count: 0,
            map_width: 8,
            map_height: 8,
            overlay_visible: true,
            fog_enabled: false,
            visible_tile_count: 64,
            hidden_tile_count: 0,
            minimap: HudMinimapSummary {
                focus_tile: None,
                view_window: HudViewWindowSummary {
                    origin_x: 0,
                    origin_y: 0,
                    width: 8,
                    height: 8,
                },
            },
        };

        let locate = build_minimap_user_flow_panel(
            &base_scene,
            &HudModel {
                summary: Some(summary.clone()),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("locate panel");
        assert_eq!(locate.next_action, "locate");
        assert_eq!(locate.focus_state, MinimapUserFocusState::Missing);
        assert!(locate.window_clamped_left);
        assert!(locate.window_clamped_top);
        assert!(locate.window_clamped_right);
        assert!(locate.window_clamped_bottom);
        assert_eq!(locate.focus_offset_x, None);
        assert_eq!(locate.focus_offset_y, None);

        let survey = build_minimap_user_flow_panel(
            &base_scene,
            &HudModel {
                summary: Some(HudSummary {
                    visible_tile_count: 0,
                    hidden_tile_count: 0,
                    minimap: HudMinimapSummary {
                        focus_tile: Some((1, 1)),
                        ..summary.minimap
                    },
                    ..summary.clone()
                }),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("survey panel");
        assert_eq!(survey.next_action, "survey");
        assert_eq!(survey.visibility_label(), "unseen");
        assert_eq!(
            survey.summary_label(),
            format!(
                "next={} focus={} vis={} cover={} pan={} target={}",
                survey.next_action,
                survey.focus_state.label(),
                survey.visibility_label(),
                survey.coverage_label(),
                survey.pan_label(),
                survey.target_kind.label(),
            )
        );
        assert_eq!(
            survey.detail_label(),
            format!(
                "next={} focus={} vis={} cover={} pan={} target={} tile={} offset={} clamp={} overlay-targets={} visible={} visible-map={} unknown={} window={}",
                survey.next_action,
                survey.focus_state.label(),
                survey.visibility_label(),
                survey.coverage_label(),
                survey.pan_label(),
                survey.target_kind.label(),
                survey.focus_tile_label(),
                survey.focus_offset_label(),
                survey.clamp_label(),
                survey.overlay_target_count,
                survey.visible_tile_count,
                survey.visible_map_percent,
                survey.unknown_tile_percent,
                survey.window_coverage_percent,
            )
        );
        assert!(survey.window_clamped_left);
        assert!(survey.window_clamped_top);
        assert!(survey.window_clamped_right);
        assert!(survey.window_clamped_bottom);

        let survey_plan_priority = build_minimap_user_flow_panel(
            &RenderModel {
                objects: vec![
                    RenderObject {
                        id: "player:1".to_string(),
                        layer: 1,
                        x: 8.0,
                        y: 8.0,
                    },
                    RenderObject {
                        id: "plan:build:1:1:1:301".to_string(),
                        layer: 2,
                        x: 8.0,
                        y: 8.0,
                    },
                ],
                ..base_scene.clone()
            },
            &HudModel {
                summary: Some(HudSummary {
                    visible_tile_count: 0,
                    hidden_tile_count: 16,
                    plan_count: 1,
                    minimap: HudMinimapSummary {
                        focus_tile: Some((1, 1)),
                        ..summary.minimap
                    },
                    ..summary.clone()
                }),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("survey plan priority panel");
        assert_eq!(survey_plan_priority.target_kind, MinimapUserTargetKind::Plan);
        assert_eq!(survey_plan_priority.visibility_label(), "hidden");
        assert_eq!(survey_plan_priority.next_action, "survey");

        let survey_marker_priority = build_minimap_user_flow_panel(
            &RenderModel {
                objects: vec![
                    RenderObject {
                        id: "player:1".to_string(),
                        layer: 1,
                        x: 8.0,
                        y: 8.0,
                    },
                    RenderObject {
                        id: "marker:point:2".to_string(),
                        layer: 2,
                        x: 8.0,
                        y: 8.0,
                    },
                ],
                ..base_scene.clone()
            },
            &HudModel {
                summary: Some(HudSummary {
                    visible_tile_count: 0,
                    hidden_tile_count: 0,
                    marker_count: 1,
                    minimap: HudMinimapSummary {
                        focus_tile: Some((1, 1)),
                        ..summary.minimap
                    },
                    ..summary.clone()
                }),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("survey marker priority panel");
        assert_eq!(
            survey_marker_priority.target_kind,
            MinimapUserTargetKind::Marker
        );
        assert_eq!(survey_marker_priority.visibility_label(), "unseen");
        assert_eq!(survey_marker_priority.next_action, "survey");

        let hidden = build_minimap_user_flow_panel(
            &base_scene,
            &HudModel {
                summary: Some(HudSummary {
                    visible_tile_count: 0,
                    hidden_tile_count: 24,
                    minimap: HudMinimapSummary {
                        focus_tile: Some((1, 1)),
                        ..summary.minimap
                    },
                    ..summary.clone()
                }),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("hidden panel");
        assert_eq!(hidden.visibility_label(), "hidden");
        assert_eq!(
            hidden.summary_label(),
            format!(
                "next={} focus={} vis={} cover={} pan={} target={}",
                hidden.next_action,
                hidden.focus_state.label(),
                hidden.visibility_label(),
                hidden.coverage_label(),
                hidden.pan_label(),
                hidden.target_kind.label(),
            )
        );
        assert!(
            hidden.detail_label().contains("vis=hidden"),
            "detail label should surface hidden visibility"
        );

        let inspect = build_minimap_user_flow_panel(
            &RenderModel {
                objects: vec![
                    RenderObject {
                        id: "player:1".to_string(),
                        layer: 1,
                        x: 8.0,
                        y: 8.0,
                    },
                    RenderObject {
                        id: "marker:point:2".to_string(),
                        layer: 2,
                        x: 16.0,
                        y: 16.0,
                    },
                ],
                ..base_scene.clone()
            },
            &HudModel {
                summary: Some(HudSummary {
                    minimap: HudMinimapSummary {
                        focus_tile: Some((1, 1)),
                        ..summary.minimap
                    },
                    ..summary.clone()
                }),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("inspect panel");
        assert_eq!(inspect.next_action, "inspect");
        assert_eq!(inspect.target_kind, MinimapUserTargetKind::Marker);
        assert!(inspect.window_clamped_left);
        assert!(inspect.window_clamped_top);
        assert!(inspect.window_clamped_right);
        assert!(inspect.window_clamped_bottom);

        let hold = build_minimap_user_flow_panel(
            &base_scene,
            &HudModel {
                summary: Some(HudSummary {
                    minimap: HudMinimapSummary {
                        focus_tile: Some((1, 1)),
                        ..summary.minimap
                    },
                    ..summary
                }),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("hold panel");
        assert_eq!(hold.next_action, "hold");
        assert_eq!(hold.target_kind, MinimapUserTargetKind::Player);
        assert_eq!(hold.coverage_label(), "full");
        assert_eq!(
            hold.summary_label(),
            format!(
                "next={} focus={} vis={} cover={} pan={} target={}",
                hold.next_action,
                hold.focus_state.label(),
                hold.visibility_label(),
                hold.coverage_label(),
                hold.pan_label(),
                hold.target_kind.label(),
            )
        );
        assert_eq!(
            hold.detail_label(),
            format!(
                "next={} focus={} vis={} cover={} pan={} target={} tile={} offset={} clamp={} overlay-targets={} visible={} visible-map={} unknown={} window={}",
                hold.next_action,
                hold.focus_state.label(),
                hold.visibility_label(),
                hold.coverage_label(),
                hold.pan_label(),
                hold.target_kind.label(),
                hold.focus_tile_label(),
                hold.focus_offset_label(),
                hold.clamp_label(),
                hold.overlay_target_count,
                hold.visible_tile_count,
                hold.visible_map_percent,
                hold.unknown_tile_percent,
                hold.window_coverage_percent,
            )
        );
        assert!(hold.window_clamped_left);
        assert!(hold.window_clamped_top);
        assert!(hold.window_clamped_right);
        assert!(hold.window_clamped_bottom);
    }

    #[test]
    fn minimap_user_flow_preserves_window_boundary_signals() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 96.0,
                height: 96.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 1,
                x: 8.0,
                y: 8.0,
            }],
        };

        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 16,
                map_height: 12,
                overlay_visible: true,
                fog_enabled: false,
                visible_tile_count: 16,
                hidden_tile_count: 0,
                minimap: HudMinimapSummary {
                    focus_tile: Some((0, 0)),
                    view_window: HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 4,
                        height: 4,
                    },
                },
            }),
            ..HudModel::default()
        };

        let top_left = build_minimap_user_flow_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 4,
            },
        )
        .expect("top-left panel");
        assert!(top_left.window_clamped_left);
        assert!(top_left.window_clamped_top);
        assert!(!top_left.window_clamped_right);
        assert!(!top_left.window_clamped_bottom);
        assert_eq!(top_left.focus_offset_x, Some(-1));
        assert_eq!(top_left.focus_offset_y, Some(-1));

        let mut bottom_right_summary = build_top_left_summary();
        bottom_right_summary.minimap.focus_tile = Some((15, 11));
        let bottom_right = build_minimap_user_flow_panel(
            &scene,
            &HudModel {
                summary: Some(bottom_right_summary),
                ..HudModel::default()
            },
            PresenterViewWindow {
                origin_x: 12,
                origin_y: 8,
                width: 4,
                height: 4,
            },
        )
        .expect("bottom-right panel");
        assert!(!bottom_right.window_clamped_left);
        assert!(!bottom_right.window_clamped_top);
        assert!(bottom_right.window_clamped_right);
        assert!(bottom_right.window_clamped_bottom);
        assert_eq!(bottom_right.focus_offset_x, Some(2));
        assert_eq!(bottom_right.focus_offset_y, Some(2));
        assert_eq!(bottom_right.pan_label(), "hold");
    }

    #[test]
    fn minimap_user_flow_does_not_treat_rounded_zero_visibility_as_hidden() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![RenderObject {
                id: "player:1".to_string(),
                layer: 1,
                x: 8.0,
                y: 8.0,
            }],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 100,
                map_height: 100,
                overlay_visible: true,
                fog_enabled: true,
                visible_tile_count: 1,
                hidden_tile_count: 9000,
                minimap: HudMinimapSummary {
                    focus_tile: Some((1, 1)),
                    view_window: HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 8,
                        height: 8,
                    },
                },
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_user_flow_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("rounded-zero visibility panel");

        assert_eq!(panel.visible_tile_count, 1);
        assert_eq!(panel.visible_map_percent, 0);
        assert_eq!(panel.visibility_label(), "mixed");
        assert_eq!(panel.next_action, "hold");
    }

    #[test]
    fn pair_label_formats_none_and_pair_values() {
        assert_eq!(super::pair_label::<usize, usize>(None), "none");
        assert_eq!(super::pair_label(Some((3, 7))), "3:7");
        assert_eq!(super::pair_label(Some(("left", "right"))), "left:right");
    }

    #[test]
    fn minimap_user_flow_focus_offset_label_returns_none_when_only_one_offset_is_present() {
        let panel = MinimapUserFlowPanelModel {
            next_action: "hold",
            focus_state: MinimapUserFocusState::Inside,
            pan_horizontal: MinimapPanAxisDirection::None,
            pan_vertical: MinimapPanAxisDirection::None,
            target_kind: MinimapUserTargetKind::None,
            focus_tile: Some((3, 4)),
            window_clamped_left: false,
            window_clamped_top: false,
            window_clamped_right: false,
            window_clamped_bottom: false,
            focus_offset_x: Some(-2),
            focus_offset_y: None,
            overlay_target_count: 0,
            visible_tile_count: 1,
            visible_map_percent: 1,
            unknown_tile_percent: 0,
            window_coverage_percent: 100,
        };

        assert_eq!(panel.focus_offset_label(), "none");
        assert_eq!(
            MinimapUserFlowPanelModel {
                focus_offset_x: None,
                focus_offset_y: Some(5),
                ..panel.clone()
            }
            .focus_offset_label(),
            "none"
        );
        assert_eq!(
            MinimapUserFlowPanelModel {
                focus_offset_x: Some(-2),
                focus_offset_y: Some(5),
                ..panel
            }
            .focus_offset_label(),
            "-2:5"
        );
    }

    #[test]
    fn minimap_user_flow_label_helpers_are_stable() {
        assert_eq!(MinimapUserFocusState::Inside.label(), "inside");
        assert_eq!(MinimapUserFocusState::Outside.label(), "outside");
        assert_eq!(MinimapUserFocusState::Missing.label(), "missing");

        assert_eq!(MinimapPanAxisDirection::None.label(), "hold");
        assert_eq!(MinimapPanAxisDirection::Left.label(), "left");
        assert_eq!(MinimapPanAxisDirection::Right.label(), "right");
        assert_eq!(MinimapPanAxisDirection::Up.label(), "up");
        assert_eq!(MinimapPanAxisDirection::Down.label(), "down");

        assert_eq!(MinimapUserTargetKind::None.label(), "none");
        assert_eq!(MinimapUserTargetKind::Plan.label(), "plan");
        assert_eq!(MinimapUserTargetKind::Marker.label(), "marker");
        assert_eq!(MinimapUserTargetKind::Player.label(), "player");
    }

    #[test]
    fn shared_prefix_label_keeps_summary_prefix_stable_for_none_and_focus_states() {
        let missing = flow_model(false, false, false, false);
        assert_eq!(
            missing.shared_prefix_label(),
            "next=hold focus=missing vis=hidden cover=offscreen pan=hold target=none"
        );
        assert_eq!(missing.summary_label(), missing.shared_prefix_label());

        let focused = MinimapUserFlowPanelModel {
            next_action: "inspect",
            focus_state: MinimapUserFocusState::Inside,
            pan_horizontal: MinimapPanAxisDirection::Right,
            pan_vertical: MinimapPanAxisDirection::Down,
            target_kind: MinimapUserTargetKind::Marker,
            focus_tile: Some((3, 4)),
            window_clamped_left: true,
            window_clamped_top: false,
            window_clamped_right: false,
            window_clamped_bottom: true,
            focus_offset_x: Some(-2),
            focus_offset_y: Some(5),
            overlay_target_count: 1,
            visible_tile_count: 5,
            visible_map_percent: 25,
            unknown_tile_percent: 50,
            window_coverage_percent: 75,
        };
        assert_eq!(
            focused.shared_prefix_label(),
            "next=inspect focus=inside vis=mixed cover=partial pan=right+down target=marker"
        );
        assert_eq!(focused.summary_label(), focused.shared_prefix_label());
    }

    #[test]
    fn minimap_user_flow_pan_label_covers_remaining_diagonals() {
        let base = flow_model(false, false, false, false);

        assert_eq!(
            MinimapUserFlowPanelModel {
                pan_horizontal: MinimapPanAxisDirection::Left,
                pan_vertical: MinimapPanAxisDirection::Up,
                ..base.clone()
            }
            .pan_label(),
            "left+up"
        );
        assert_eq!(
            MinimapUserFlowPanelModel {
                pan_horizontal: MinimapPanAxisDirection::Left,
                pan_vertical: MinimapPanAxisDirection::Down,
                ..base.clone()
            }
            .pan_label(),
            "left+down"
        );
        assert_eq!(
            MinimapUserFlowPanelModel {
                pan_horizontal: MinimapPanAxisDirection::Right,
                pan_vertical: MinimapPanAxisDirection::Up,
                ..base.clone()
            }
            .pan_label(),
            "right+up"
        );
        assert_eq!(
            MinimapUserFlowPanelModel {
                pan_horizontal: MinimapPanAxisDirection::Right,
                pan_vertical: MinimapPanAxisDirection::Down,
                ..base
            }
            .pan_label(),
            "right+down"
        );
    }

    #[test]
    fn minimap_user_flow_prefers_runtime_target_kind_over_player_when_present() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:1".to_string(),
                    layer: 1,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:runtime-health:1:2".to_string(),
                    layer: 2,
                    x: 16.0,
                    y: 16.0,
                },
            ],
        };
        let hud = HudModel {
            summary: Some(HudSummary {
                player_name: "operator".to_string(),
                team_id: 2,
                selected_block: "payload-router".to_string(),
                plan_count: 0,
                marker_count: 0,
                map_width: 8,
                map_height: 8,
                overlay_visible: true,
                fog_enabled: false,
                visible_tile_count: 64,
                hidden_tile_count: 0,
                minimap: HudMinimapSummary {
                    focus_tile: Some((1, 1)),
                    view_window: HudViewWindowSummary {
                        origin_x: 0,
                        origin_y: 0,
                        width: 8,
                        height: 8,
                    },
                },
            }),
            ..HudModel::default()
        };

        let panel = build_minimap_user_flow_panel(
            &scene,
            &hud,
            PresenterViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 8,
                height: 8,
            },
        )
        .expect("runtime priority panel");

        assert_eq!(panel.target_kind, MinimapUserTargetKind::Runtime);
        assert_eq!(panel.next_action, "inspect");
        assert_eq!(panel.overlay_target_count, 1);
    }

    fn build_top_left_summary() -> HudSummary {
        HudSummary {
            player_name: "operator".to_string(),
            team_id: 2,
            selected_block: "payload-router".to_string(),
            plan_count: 0,
            marker_count: 0,
            map_width: 16,
            map_height: 12,
            overlay_visible: true,
            fog_enabled: false,
            visible_tile_count: 16,
            hidden_tile_count: 0,
            minimap: HudMinimapSummary {
                focus_tile: Some((0, 0)),
                view_window: HudViewWindowSummary {
                    origin_x: 0,
                    origin_y: 0,
                    width: 4,
                    height: 4,
                },
            },
        }
    }
}
