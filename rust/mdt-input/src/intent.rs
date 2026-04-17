#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Fire,
    Boost,
    Chat,
    Interact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildPulse {
    pub tile: (i32, i32),
    pub breaking: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerIntent {
    SetMoveAxis { x: f32, y: f32 },
    SetAimAxis { x: f32, y: f32 },
    SetMiningTile { tile: Option<(i32, i32)> },
    SetBuilding { building: bool },
    ConfigTap { tile: (i32, i32) },
    BuildPulse(BuildPulse),
    ActionPressed(BinaryAction),
    ActionHeld(BinaryAction),
    ActionReleased(BinaryAction),
}

impl BinaryAction {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn label(self) -> &'static str {
        match self {
            Self::MoveUp => "move-up",
            Self::MoveDown => "move-down",
            Self::MoveLeft => "move-left",
            Self::MoveRight => "move-right",
            Self::Fire => "fire",
            Self::Boost => "boost",
            Self::Chat => "chat",
            Self::Interact => "interact",
        }
    }
}

impl BuildPulse {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn summary_label(&self) -> String {
        format!(
            "pulse={},{},{}",
            self.tile.0,
            self.tile.1,
            if self.breaking { "break" } else { "place" }
        )
    }
}

impl PlayerIntent {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn summary_label(&self) -> String {
        match self {
            Self::SetMoveAxis { x, y } => format!("move={x},{y}"),
            Self::SetAimAxis { x, y } => format!("aim={x},{y}"),
            Self::SetMiningTile { tile } => format!(
                "mining={}",
                tile.map_or_else(|| "none".to_string(), |(x, y)| format!("{x},{y}"))
            ),
            Self::SetBuilding { building } => {
                format!("building={}", if *building { "on" } else { "off" })
            }
            Self::ConfigTap { tile } => format!("tap={},{}", tile.0, tile.1),
            Self::BuildPulse(pulse) => pulse.summary_label(),
            Self::ActionPressed(action) => format!("press={}", action.label()),
            Self::ActionHeld(action) => format!("hold={}", action.label()),
            Self::ActionReleased(action) => format!("release={}", action.label()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BinaryAction, BuildPulse, PlayerIntent};

    #[test]
    fn binary_action_label_covers_all_variants() {
        assert_eq!(BinaryAction::MoveUp.label(), "move-up");
        assert_eq!(BinaryAction::MoveDown.label(), "move-down");
        assert_eq!(BinaryAction::MoveLeft.label(), "move-left");
        assert_eq!(BinaryAction::MoveRight.label(), "move-right");
        assert_eq!(BinaryAction::Fire.label(), "fire");
        assert_eq!(BinaryAction::Boost.label(), "boost");
        assert_eq!(BinaryAction::Chat.label(), "chat");
        assert_eq!(BinaryAction::Interact.label(), "interact");
    }

    #[test]
    fn build_pulse_summary_label_formats_break_and_place() {
        assert_eq!(
            BuildPulse {
                tile: (9, 10),
                breaking: true,
            }
            .summary_label(),
            "pulse=9,10,break"
        );
        assert_eq!(
            BuildPulse {
                tile: (9, 10),
                breaking: false,
            }
            .summary_label(),
            "pulse=9,10,place"
        );
    }

    #[test]
    fn player_intent_summary_label_compacts_axes_build_and_actions() {
        assert_eq!(
            PlayerIntent::SetMoveAxis { x: 1.0, y: -1.0 }.summary_label(),
            "move=1,-1"
        );
        assert_eq!(
            PlayerIntent::SetAimAxis { x: 8.0, y: 12.0 }.summary_label(),
            "aim=8,12"
        );
        assert_eq!(
            PlayerIntent::SetMiningTile { tile: None }.summary_label(),
            "mining=none"
        );
        assert_eq!(
            PlayerIntent::SetMiningTile {
                tile: Some((7, 8))
            }
            .summary_label(),
            "mining=7,8"
        );
        assert_eq!(
            PlayerIntent::SetBuilding { building: true }.summary_label(),
            "building=on"
        );
        assert_eq!(
            PlayerIntent::ConfigTap { tile: (3, 4) }.summary_label(),
            "tap=3,4"
        );
        assert_eq!(
            PlayerIntent::BuildPulse(BuildPulse {
                tile: (9, 10),
                breaking: true,
            })
            .summary_label(),
            "pulse=9,10,break"
        );
        assert_eq!(
            PlayerIntent::ActionPressed(BinaryAction::Fire).summary_label(),
            "press=fire"
        );
        assert_eq!(
            PlayerIntent::ActionHeld(BinaryAction::Boost).summary_label(),
            "hold=boost"
        );
        assert_eq!(
            PlayerIntent::ActionReleased(BinaryAction::Chat).summary_label(),
            "release=chat"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_config_tap_coordinates_stable() {
        assert_eq!(
            PlayerIntent::ConfigTap { tile: (-3, 4) }.summary_label(),
            "tap=-3,4"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_building_on_and_off() {
        assert_eq!(
            PlayerIntent::SetBuilding { building: true }.summary_label(),
            "building=on"
        );
        assert_eq!(
            PlayerIntent::SetBuilding { building: false }.summary_label(),
            "building=off"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_release_interact_stably() {
        assert_eq!(
            PlayerIntent::ActionReleased(BinaryAction::Interact).summary_label(),
            "release=interact"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_fractional_axes_without_regression() {
        assert_eq!(
            PlayerIntent::SetMoveAxis { x: 1.5, y: -0.25 }.summary_label(),
            "move=1.5,-0.25"
        );
        assert_eq!(
            PlayerIntent::SetAimAxis { x: -2.5, y: 3.125 }.summary_label(),
            "aim=-2.5,3.125"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_hold_interact_stably() {
        assert_eq!(
            PlayerIntent::ActionHeld(BinaryAction::Interact).summary_label(),
            "hold=interact"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_press_interact_stably() {
        assert_eq!(
            PlayerIntent::ActionPressed(BinaryAction::Interact).summary_label(),
            "press=interact"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_press_move_up_stably() {
        assert_eq!(
            PlayerIntent::ActionPressed(BinaryAction::MoveUp).summary_label(),
            "press=move-up"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_release_move_down_stably() {
        assert_eq!(
            PlayerIntent::ActionReleased(BinaryAction::MoveDown).summary_label(),
            "release=move-down"
        );
    }

    #[test]
    fn player_intent_summary_label_formats_press_chat_stably() {
        assert_eq!(
            PlayerIntent::ActionPressed(BinaryAction::Chat).summary_label(),
            "press=chat"
        );
    }
}
