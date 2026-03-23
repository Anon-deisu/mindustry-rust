use crate::intent::{BinaryAction, PlayerIntent};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveIntentState {
    pub move_axis: (f32, f32),
    pub aim_axis: (f32, f32),
    pub active_actions: Vec<BinaryAction>,
    pub pressed_actions: Vec<BinaryAction>,
    pub released_actions: Vec<BinaryAction>,
}

impl LiveIntentState {
    pub fn apply_intents(&mut self, intents: &[PlayerIntent]) {
        self.pressed_actions.clear();
        self.released_actions.clear();

        for intent in intents {
            match intent {
                PlayerIntent::SetMoveAxis { x, y } => {
                    self.move_axis = (*x, *y);
                }
                PlayerIntent::SetAimAxis { x, y } => {
                    self.aim_axis = (*x, *y);
                }
                PlayerIntent::ActionPressed(action) => {
                    push_unique(&mut self.active_actions, *action);
                    push_unique(&mut self.pressed_actions, *action);
                }
                PlayerIntent::ActionHeld(action) => {
                    push_unique(&mut self.active_actions, *action);
                }
                PlayerIntent::ActionReleased(action) => {
                    remove_action(&mut self.active_actions, *action);
                    push_unique(&mut self.released_actions, *action);
                }
            }
        }
    }

    pub fn is_action_active(&self, action: BinaryAction) -> bool {
        self.active_actions.contains(&action)
    }
}

fn push_unique(actions: &mut Vec<BinaryAction>, action: BinaryAction) {
    if !actions.contains(&action) {
        actions.push(action);
    }
}

fn remove_action(actions: &mut Vec<BinaryAction>, action: BinaryAction) {
    if let Some(index) = actions.iter().position(|existing| *existing == action) {
        actions.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::{InputSnapshot, IntentMapper, IntentSamplingMode, StatelessIntentMapper};

    #[test]
    fn apply_intents_tracks_axes_and_action_edges() {
        let mut mapper = StatelessIntentMapper::default();
        let mut state = LiveIntentState::default();

        let first = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, -1.0),
            aim_axis: (16.0, 24.0),
            active_actions: vec![BinaryAction::Fire, BinaryAction::Use],
        });
        state.apply_intents(&first);
        assert_eq!(state.move_axis, (1.0, -1.0));
        assert_eq!(state.aim_axis, (16.0, 24.0));
        assert_eq!(
            state.pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Use]
        );
        assert!(state.released_actions.is_empty());
        assert!(state.is_action_active(BinaryAction::Fire));
        assert!(state.is_action_active(BinaryAction::Use));

        let second = mapper.map_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (32.0, 48.0),
            active_actions: vec![BinaryAction::Use],
        });
        state.apply_intents(&second);
        assert_eq!(state.move_axis, (0.0, 0.0));
        assert_eq!(state.aim_axis, (32.0, 48.0));
        assert!(state.pressed_actions.is_empty());
        assert_eq!(state.released_actions, vec![BinaryAction::Fire]);
        assert!(!state.is_action_active(BinaryAction::Fire));
        assert!(state.is_action_active(BinaryAction::Use));
    }

    #[test]
    fn live_sampling_mode_keeps_action_active_without_held_intents() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let mut state = LiveIntentState::default();

        let first = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (8.0, 12.0),
            active_actions: vec![BinaryAction::Fire],
        });
        state.apply_intents(&first);
        assert_eq!(state.pressed_actions, vec![BinaryAction::Fire]);
        assert!(state.released_actions.is_empty());
        assert!(state.is_action_active(BinaryAction::Fire));

        let second = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (8.0, 12.0),
            active_actions: vec![BinaryAction::Fire],
        });
        state.apply_intents(&second);
        assert!(state.pressed_actions.is_empty());
        assert!(state.released_actions.is_empty());
        assert!(state.is_action_active(BinaryAction::Fire));

        let third = mapper.map_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (16.0, 20.0),
            active_actions: vec![],
        });
        state.apply_intents(&third);
        assert!(state.pressed_actions.is_empty());
        assert_eq!(state.released_actions, vec![BinaryAction::Fire]);
        assert!(!state.is_action_active(BinaryAction::Fire));
    }
}
