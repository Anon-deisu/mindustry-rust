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
    pub overlay_target_count: usize,
    pub visible_map_percent: usize,
    pub unknown_tile_percent: usize,
    pub window_coverage_percent: usize,
}

impl MinimapUserFlowPanelModel {
    pub(crate) fn visibility_label(&self) -> &'static str {
        if self.unknown_tile_percent == 100 {
            "unseen"
        } else if self.visible_map_percent == 0 {
            "hidden"
        } else if self.unknown_tile_percent == 0 {
            "mapped"
        } else {
            "mixed"
        }
    }

    pub(crate) fn coverage_label(&self) -> &'static str {
        if self.window_coverage_percent == 0 {
            "offscreen"
        } else if self.window_coverage_percent == 100 {
            "full"
        } else {
            "partial"
        }
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
    let visibility_label = if panel.unknown_tile_percent == 100 {
        "unseen"
    } else if panel.visible_map_percent() == 0 {
        "hidden"
    } else if panel.unknown_tile_percent == 0 {
        "mapped"
    } else {
        "mixed"
    };
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
            ) => "inspect",
        MinimapUserFocusState::Inside => "hold",
    };

    Some(MinimapUserFlowPanelModel {
        next_action,
        focus_state,
        pan_horizontal: pan_horizontal_direction(&panel),
        pan_vertical: pan_vertical_direction(&panel),
        target_kind,
        focus_tile: panel.focus_tile,
        overlay_target_count: panel.plan_count + panel.marker_count + panel.runtime_count,
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
        build_minimap_user_flow_panel, MinimapPanAxisDirection, MinimapUserFocusState,
        MinimapUserTargetKind,
    };
    use crate::hud_model::{HudMinimapSummary, HudSummary, HudViewWindowSummary};
    use crate::{HudModel, RenderModel, RenderObject, Viewport};
    use crate::panel_model::PresenterViewWindow;

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
    }
}
