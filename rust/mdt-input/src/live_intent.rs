use crate::intent::{BinaryAction, PlayerIntent};
use crate::mapper::{InputSnapshot, IntentMapper, IntentSamplingMode, StatelessIntentMapper};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveIntentState {
    pub move_axis: (f32, f32),
    pub aim_axis: (f32, f32),
    pub mining_tile: Option<(i32, i32)>,
    pub building: bool,
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
                PlayerIntent::SetMiningTile { tile } => {
                    self.mining_tile = *tile;
                }
                PlayerIntent::SetBuilding { building } => {
                    self.building = *building;
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

#[derive(Debug)]
pub struct RuntimeIntentTracker {
    mapper: StatelessIntentMapper,
    state: LiveIntentState,
    override_snapshot: Option<InputSnapshot>,
}

impl RuntimeIntentTracker {
    pub fn new(sampling_mode: IntentSamplingMode) -> Self {
        Self {
            mapper: StatelessIntentMapper::new(sampling_mode),
            state: LiveIntentState::default(),
            override_snapshot: None,
        }
    }

    pub fn state(&self) -> &LiveIntentState {
        &self.state
    }

    pub fn set_override_snapshot(&mut self, snapshot: Option<InputSnapshot>) {
        self.override_snapshot = snapshot;
    }

    pub fn sample_runtime_snapshot(&mut self, runtime_snapshot: &InputSnapshot) -> bool {
        let snapshot = self.override_snapshot.as_ref().unwrap_or(runtime_snapshot);
        let intents = self.mapper.map_snapshot(snapshot);
        let previous_key = runtime_snapshot_apply_key(&self.state);
        self.state.apply_intents(&intents);
        runtime_snapshot_apply_key(&self.state) != previous_key
    }
}

fn runtime_snapshot_apply_key(
    state: &LiveIntentState,
) -> (
    (f32, f32),
    (f32, f32),
    Option<(i32, i32)>,
    bool,
    Vec<BinaryAction>,
) {
    (
        state.move_axis,
        state.aim_axis,
        state.mining_tile,
        state.building,
        state.active_actions.clone(),
    )
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

    #[test]
    fn apply_intents_tracks_axes_and_action_edges() {
        let mut mapper = StatelessIntentMapper::default();
        let mut state = LiveIntentState::default();

        let first = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, -1.0),
            aim_axis: (16.0, 24.0),
            mining_tile: Some((7, 9)),
            building: true,
            active_actions: vec![BinaryAction::Fire, BinaryAction::Boost],
        });
        state.apply_intents(&first);
        assert_eq!(state.move_axis, (1.0, -1.0));
        assert_eq!(state.aim_axis, (16.0, 24.0));
        assert_eq!(state.mining_tile, Some((7, 9)));
        assert!(state.building);
        assert_eq!(
            state.pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert!(state.released_actions.is_empty());
        assert!(state.is_action_active(BinaryAction::Fire));
        assert!(state.is_action_active(BinaryAction::Boost));

        let second = mapper.map_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (32.0, 48.0),
            mining_tile: None,
            building: false,
            active_actions: vec![BinaryAction::Boost],
        });
        state.apply_intents(&second);
        assert_eq!(state.move_axis, (0.0, 0.0));
        assert_eq!(state.aim_axis, (32.0, 48.0));
        assert_eq!(state.mining_tile, None);
        assert!(!state.building);
        assert!(state.pressed_actions.is_empty());
        assert_eq!(state.released_actions, vec![BinaryAction::Fire]);
        assert!(!state.is_action_active(BinaryAction::Fire));
        assert!(state.is_action_active(BinaryAction::Boost));
    }

    #[test]
    fn live_sampling_mode_keeps_action_active_without_held_intents() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let mut state = LiveIntentState::default();

        let first = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (8.0, 12.0),
            mining_tile: Some((4, 5)),
            building: true,
            active_actions: vec![BinaryAction::Fire],
        });
        state.apply_intents(&first);
        assert_eq!(state.pressed_actions, vec![BinaryAction::Fire]);
        assert!(state.released_actions.is_empty());
        assert!(state.is_action_active(BinaryAction::Fire));
        assert_eq!(state.mining_tile, Some((4, 5)));
        assert!(state.building);

        let second = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (8.0, 12.0),
            mining_tile: Some((4, 5)),
            building: true,
            active_actions: vec![BinaryAction::Fire],
        });
        state.apply_intents(&second);
        assert!(state.pressed_actions.is_empty());
        assert!(state.released_actions.is_empty());
        assert!(state.is_action_active(BinaryAction::Fire));

        let third = mapper.map_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (16.0, 20.0),
            mining_tile: None,
            building: false,
            active_actions: vec![],
        });
        state.apply_intents(&third);
        assert!(state.pressed_actions.is_empty());
        assert_eq!(state.released_actions, vec![BinaryAction::Fire]);
        assert!(!state.is_action_active(BinaryAction::Fire));
        assert_eq!(state.mining_tile, None);
        assert!(!state.building);
    }

    #[test]
    fn runtime_intent_tracker_samples_runtime_snapshot_without_schedule() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (16.0, 24.0),
            mining_tile: Some((7, 9)),
            building: true,
            active_actions: vec![BinaryAction::Fire, BinaryAction::Boost],
        }));
        assert_eq!(tracker.state().move_axis, (1.0, 0.0));
        assert_eq!(tracker.state().aim_axis, (16.0, 24.0));
        assert_eq!(tracker.state().mining_tile, Some((7, 9)));
        assert!(tracker.state().building);
        assert_eq!(
            tracker.state().pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert!(tracker.state().released_actions.is_empty());

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (32.0, 48.0),
            mining_tile: None,
            building: false,
            active_actions: vec![],
        }));
        assert_eq!(tracker.state().move_axis, (0.0, 0.0));
        assert_eq!(tracker.state().aim_axis, (32.0, 48.0));
        assert_eq!(tracker.state().mining_tile, None);
        assert!(!tracker.state().building);
        assert!(tracker.state().pressed_actions.is_empty());
        assert_eq!(
            tracker.state().released_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
    }

    #[test]
    fn runtime_intent_tracker_keeps_override_snapshot_active_until_replaced() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        tracker.set_override_snapshot(Some(InputSnapshot {
            move_axis: (0.5, -0.5),
            aim_axis: (10.0, 20.0),
            mining_tile: Some((3, 4)),
            building: true,
            active_actions: vec![BinaryAction::Chat],
        }));

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (2.0, 3.0),
            aim_axis: (40.0, 50.0),
            mining_tile: None,
            building: false,
            active_actions: vec![BinaryAction::Fire],
        }));
        assert_eq!(tracker.state().move_axis, (0.5, -0.5));
        assert_eq!(tracker.state().aim_axis, (10.0, 20.0));
        assert_eq!(tracker.state().mining_tile, Some((3, 4)));
        assert!(tracker.state().building);
        assert!(tracker.state().is_action_active(BinaryAction::Chat));
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));

        assert!(!tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (5.0, 6.0),
            aim_axis: (60.0, 70.0),
            mining_tile: Some((8, 9)),
            building: false,
            active_actions: vec![BinaryAction::Boost],
        }));
        assert_eq!(tracker.state().move_axis, (0.5, -0.5));

        tracker.set_override_snapshot(None);
        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (5.0, 6.0),
            aim_axis: (60.0, 70.0),
            mining_tile: Some((8, 9)),
            building: false,
            active_actions: vec![BinaryAction::Boost],
        }));
        assert_eq!(tracker.state().move_axis, (5.0, 6.0));
        assert_eq!(tracker.state().aim_axis, (60.0, 70.0));
        assert_eq!(tracker.state().mining_tile, Some((8, 9)));
        assert!(!tracker.state().building);
        assert!(tracker.state().is_action_active(BinaryAction::Boost));
    }

    #[test]
    fn runtime_intent_tracker_detects_building_state_changes() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: true,
            active_actions: Vec::new(),
        }));
        assert!(tracker.state().building);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            active_actions: Vec::new(),
        }));
        assert!(!tracker.state().building);
    }
}
