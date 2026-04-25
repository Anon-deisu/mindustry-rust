use crate::intent::{BinaryAction, BuildPulse, PlayerIntent};
use crate::mapper::{InputSnapshot, IntentMapper, IntentSamplingMode, StatelessIntentMapper};
use crate::probe::{sample_runtime_input_snapshot, RuntimeInputSample};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveIntentState {
    pub move_axis: (f32, f32),
    pub aim_axis: (f32, f32),
    pub mining_tile: Option<(i32, i32)>,
    pub building: bool,
    pub last_config_tap_tile: Option<(i32, i32)>,
    pub config_tap_count: u32,
    pub last_build_pulse: Option<BuildPulse>,
    pub build_pulse_count: u32,
    pub active_actions: Vec<BinaryAction>,
    pub pressed_actions: Vec<BinaryAction>,
    pub released_actions: Vec<BinaryAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveIntentBindingProfile {
    pub move_axis: (f32, f32),
    pub aim_axis: (f32, f32),
    pub mining_tile: Option<(i32, i32)>,
    pub building: bool,
    pub last_config_tap_tile: Option<(i32, i32)>,
    pub last_build_pulse: Option<BuildPulse>,
    pub active_actions: Vec<BinaryAction>,
    pub pressed_actions: Vec<BinaryAction>,
    pub released_actions: Vec<BinaryAction>,
}

impl LiveIntentBindingProfile {
    pub fn has_motion(&self) -> bool {
        normalize_axis(self.move_axis) != (0.0, 0.0)
    }

    pub fn has_aim(&self) -> bool {
        normalize_axis(self.aim_axis) != (0.0, 0.0)
    }

    pub fn has_transient_signals(&self) -> bool {
        self.last_config_tap_tile.is_some()
            || self.last_build_pulse.is_some()
            || !self.pressed_actions.is_empty()
            || !self.released_actions.is_empty()
    }

    pub fn is_idle(&self) -> bool {
        !self.has_motion()
            && !self.has_aim()
            && self.mining_tile.is_none()
            && !self.building
            && self.last_config_tap_tile.is_none()
            && self.last_build_pulse.is_none()
            && self.active_actions.is_empty()
            && self.pressed_actions.is_empty()
            && self.released_actions.is_empty()
    }

    pub fn summary_label(&self) -> String {
        format!(
            "move={} aim={} mining={} building={} active={} transient={}",
            axis_label(self.move_axis),
            axis_label(self.aim_axis),
            tile_label(self.mining_tile),
            bool_label(self.building),
            self.active_actions.len(),
            transient_label(self),
        )
    }
}

impl LiveIntentState {
    pub fn apply_intents(&mut self, intents: &[PlayerIntent]) {
        self.clear_transient_edges();

        for intent in intents {
            match intent {
                PlayerIntent::SetMoveAxis { x, y } => {
                    self.move_axis = normalize_axis((*x, *y));
                }
                PlayerIntent::SetAimAxis { x, y } => {
                    self.aim_axis = normalize_axis((*x, *y));
                }
                PlayerIntent::SetMiningTile { tile } => {
                    self.mining_tile = *tile;
                }
                PlayerIntent::SetBuilding { building } => {
                    self.building = *building;
                }
                PlayerIntent::ConfigTap { tile } => {
                    self.last_config_tap_tile = Some(*tile);
                    self.config_tap_count = self.config_tap_count.saturating_add(1);
                }
                PlayerIntent::BuildPulse(pulse) => {
                    self.last_build_pulse = Some(*pulse);
                    self.build_pulse_count = self.build_pulse_count.saturating_add(1);
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

    pub fn clear_transient_edges(&mut self) {
        self.last_config_tap_tile = None;
        self.last_build_pulse = None;
        self.pressed_actions.clear();
        self.released_actions.clear();
    }

    pub fn binding_profile(&self) -> LiveIntentBindingProfile {
        LiveIntentBindingProfile {
            move_axis: normalize_axis(self.move_axis),
            aim_axis: normalize_axis(self.aim_axis),
            mining_tile: self.mining_tile,
            building: self.building,
            last_config_tap_tile: self.last_config_tap_tile,
            last_build_pulse: self.last_build_pulse,
            active_actions: self.active_actions.clone(),
            pressed_actions: self.pressed_actions.clone(),
            released_actions: self.released_actions.clone(),
        }
    }
}

#[derive(Debug)]
pub struct RuntimeIntentTracker {
    mapper: StatelessIntentMapper,
    state: LiveIntentState,
    override_snapshot: Option<InputSnapshot>,
    empty_batch_needs_reconcile: bool,
}

impl RuntimeIntentTracker {
    pub fn new(sampling_mode: IntentSamplingMode) -> Self {
        Self {
            mapper: StatelessIntentMapper::new(sampling_mode),
            state: LiveIntentState::default(),
            override_snapshot: None,
            empty_batch_needs_reconcile: false,
        }
    }

    pub fn state(&self) -> &LiveIntentState {
        &self.state
    }

    pub fn binding_profile(&self) -> LiveIntentBindingProfile {
        self.state.binding_profile()
    }

    pub fn set_override_snapshot(&mut self, snapshot: Option<InputSnapshot>) {
        self.override_snapshot = snapshot;
    }

    pub fn sample_runtime_snapshot(&mut self, runtime_snapshot: &InputSnapshot) -> bool {
        self.sample_snapshot(runtime_snapshot, None)
    }

    pub fn sample_probe_input(&mut self, probe_sample: RuntimeInputSample) -> bool {
        let runtime_snapshot = sampled_probe_snapshot(probe_sample);
        self.sample_runtime_snapshot(&runtime_snapshot)
    }

    pub fn sample_runtime_snapshot_with_transient_batch(
        &mut self,
        transient_snapshots: &[InputSnapshot],
        runtime_snapshot: &InputSnapshot,
    ) -> bool {
        let final_snapshot = self.override_snapshot.as_ref().unwrap_or(runtime_snapshot);
        let current_active_actions = final_snapshot.active_actions.clone();
        let intents = self
            .mapper
            .map_snapshot_batch_with_final_snapshot(transient_snapshots, final_snapshot);
        self.apply_mapped_intents(intents, true, &current_active_actions)
    }

    pub fn sample_probe_input_with_transient_batch(
        &mut self,
        transient_probe_samples: &[RuntimeInputSample],
        runtime_probe_sample: RuntimeInputSample,
    ) -> bool {
        let transient_snapshots = sampled_probe_snapshot_batch(transient_probe_samples);
        let runtime_snapshot = sampled_probe_snapshot(runtime_probe_sample);
        self.sample_runtime_snapshot_with_transient_batch(&transient_snapshots, &runtime_snapshot)
    }

    pub fn sample_runtime_snapshot_batch(&mut self, runtime_snapshots: &[InputSnapshot]) -> bool {
        if runtime_snapshots.is_empty() && self.override_snapshot.is_none() {
            let _ = self.mapper.map_snapshot(&InputSnapshot::default());
            self.empty_batch_needs_reconcile = true;
            self.state.clear_transient_edges();
            return false;
        }
        let override_snapshot = self.override_snapshot.clone();
        self.apply_runtime_batch(runtime_snapshots, override_snapshot.as_ref())
    }

    pub fn sample_probe_input_batch(&mut self, runtime_probe_samples: &[RuntimeInputSample]) -> bool {
        let runtime_snapshots = sampled_probe_snapshot_batch(runtime_probe_samples);
        self.sample_runtime_snapshot_batch(&runtime_snapshots)
    }

    pub fn sample_runtime_snapshot_batch_with_override(
        &mut self,
        runtime_snapshots: &[InputSnapshot],
        override_snapshot: &InputSnapshot,
    ) -> bool {
        let intents = self
            .mapper
            .map_snapshot_batch_with_final_snapshot(runtime_snapshots, override_snapshot);
        self.apply_mapped_intents(intents, true, &override_snapshot.active_actions)
    }

    fn apply_runtime_batch(
        &mut self,
        runtime_snapshots: &[InputSnapshot],
        override_snapshot: Option<&InputSnapshot>,
    ) -> bool {
        let current_active_actions: &[BinaryAction] = if let Some(snapshot) = override_snapshot {
            &snapshot.active_actions
        } else if let Some(snapshot) = runtime_snapshots.last() {
            &snapshot.active_actions
        } else {
            &[]
        };
        let intents = self
            .mapper
            .map_snapshot_batch_or_override(runtime_snapshots, override_snapshot);
        self.apply_mapped_intents(intents, true, current_active_actions)
    }

    pub fn sample_runtime_snapshot_with_override(
        &mut self,
        runtime_snapshot: &InputSnapshot,
        override_snapshot: &InputSnapshot,
    ) -> bool {
        self.sample_snapshot(runtime_snapshot, Some(override_snapshot))
    }

    fn sample_snapshot(
        &mut self,
        runtime_snapshot: &InputSnapshot,
        override_snapshot: Option<&InputSnapshot>,
    ) -> bool {
        let snapshot = override_snapshot
            .or(self.override_snapshot.as_ref())
            .unwrap_or(runtime_snapshot);
        let current_active_actions = snapshot.active_actions.clone();
        let intents = self.mapper.map_snapshot(snapshot);
        self.apply_mapped_intents(intents, false, &current_active_actions)
    }

    fn apply_mapped_intents(
        &mut self,
        mut intents: Vec<PlayerIntent>,
        include_transient_edges: bool,
        current_active_actions: &[BinaryAction],
    ) -> bool {
        if self.empty_batch_needs_reconcile {
            for action in self.state.active_actions.iter().copied() {
                if !current_active_actions.contains(&action) {
                    intents.push(PlayerIntent::ActionReleased(action));
                }
            }
            self.empty_batch_needs_reconcile = false;
        }
        let previous_key = runtime_snapshot_apply_key(&self.state);
        self.state.apply_intents(&intents);
        runtime_snapshot_apply_key(&self.state) != previous_key
            || (include_transient_edges
                && (!self.state.pressed_actions.is_empty()
                    || !self.state.released_actions.is_empty()))
    }
}

fn runtime_snapshot_apply_key(
    state: &LiveIntentState,
) -> (
    (f32, f32),
    (f32, f32),
    Option<(i32, i32)>,
    bool,
    Option<(i32, i32)>,
    u32,
    Option<BuildPulse>,
    u32,
    Vec<BinaryAction>,
) {
    (
        normalize_axis(state.move_axis),
        normalize_axis(state.aim_axis),
        state.mining_tile,
        state.building,
        state.last_config_tap_tile,
        state.config_tap_count,
        state.last_build_pulse,
        state.build_pulse_count,
        state.active_actions.clone(),
    )
}

fn sampled_probe_snapshot(probe_sample: RuntimeInputSample) -> InputSnapshot {
    sample_runtime_input_snapshot(probe_sample)
}

fn sampled_probe_snapshot_batch(probe_samples: &[RuntimeInputSample]) -> Vec<InputSnapshot> {
    probe_samples
        .iter()
        .copied()
        .map(sampled_probe_snapshot)
        .collect()
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

fn normalize_axis(axis: (f32, f32)) -> (f32, f32) {
    if axis.0.is_finite() && axis.1.is_finite() {
        axis
    } else {
        (0.0, 0.0)
    }
}

fn axis_label(axis: (f32, f32)) -> String {
    format!("{},{}", axis_value_label(axis.0), axis_value_label(axis.1))
}

fn axis_value_label(value: f32) -> String {
    if value == 0.0 {
        "0".to_string()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn tile_label(tile: Option<(i32, i32)>) -> String {
    match tile {
        Some((x, y)) => format!("{x},{y}"),
        None => "none".to_string(),
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn transient_label(profile: &LiveIntentBindingProfile) -> String {
    let mut parts = Vec::new();

    if let Some((x, y)) = profile.last_config_tap_tile {
        parts.push(format!("tap={x},{y}"));
    }
    if let Some(pulse) = profile.last_build_pulse {
        parts.push(format!(
            "pulse={},{},{}",
            pulse.tile.0,
            pulse.tile.1,
            if pulse.breaking { "break" } else { "place" }
        ));
    }
    if !profile.pressed_actions.is_empty() {
        parts.push(format!("pressed={}", profile.pressed_actions.len()));
    }
    if !profile.released_actions.is_empty() {
        parts.push(format!("released={}", profile.released_actions.len()));
    }

    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::classify_runtime_input_sample;

    #[test]
    fn apply_intents_tracks_axes_and_action_edges() {
        let mut mapper = StatelessIntentMapper::default();
        let mut state = LiveIntentState::default();

        let first = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, -1.0),
            aim_axis: (16.0, 24.0),
            mining_tile: Some((7, 9)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
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
    fn apply_intents_normalizes_non_finite_axes_before_state_update() {
        let mut state = LiveIntentState::default();

        state.apply_intents(&[
            PlayerIntent::SetMoveAxis {
                x: f32::NAN,
                y: 3.0,
            },
            PlayerIntent::SetAimAxis {
                x: 12.0,
                y: f32::INFINITY,
            },
        ]);

        assert_eq!(state.move_axis, (0.0, 0.0));
        assert_eq!(state.aim_axis, (0.0, 0.0));
        assert!(!state.binding_profile().has_motion());
        assert!(!state.binding_profile().has_aim());
        assert!(state.binding_profile().is_idle());
    }

    #[test]
    fn normalize_axis_rejects_non_finite_and_keeps_finite_axes() {
        assert_eq!(normalize_axis((1.5, -2.25)), (1.5, -2.25));
        assert_eq!(normalize_axis((f32::NAN, 3.0)), (0.0, 0.0));
        assert_eq!(normalize_axis((4.0, f32::INFINITY)), (0.0, 0.0));
        assert_eq!(normalize_axis((f32::NEG_INFINITY, 5.0)), (0.0, 0.0));
    }

    #[test]
    fn axis_value_label_formats_zero_integral_and_fractional_values() {
        assert_eq!(axis_value_label(0.0), "0");
        assert_eq!(axis_value_label(-0.0), "0");
        assert_eq!(axis_value_label(12.0), "12");
        assert_eq!(axis_value_label(-7.0), "-7");
        assert_eq!(axis_value_label(3.25), "3.25");
    }

    #[test]
    fn live_intent_labels_format_compactly() {
        assert_eq!(axis_label((0.0, -2.5)), "0,-2.5");
        assert_eq!(axis_label((3.0, 4.0)), "3,4");
        assert_eq!(tile_label(None), "none");
        assert_eq!(tile_label(Some((7, 9))), "7,9");
        assert_eq!(bool_label(true), "on");
        assert_eq!(bool_label(false), "off");
    }

    #[test]
    fn runtime_intent_tracker_ignores_non_finite_axes_in_apply_key() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        tracker.state.move_axis = (f32::NAN, 1.0);
        tracker.state.aim_axis = (2.0, f32::INFINITY);

        assert!(!tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: Vec::new(),
        }));
        assert_eq!(tracker.state.move_axis, (0.0, 0.0));
        assert_eq!(tracker.state.aim_axis, (0.0, 0.0));
    }

    #[test]
    fn runtime_snapshot_apply_key_normalizes_nonfinite_axes_and_preserves_action_order() {
        let state = LiveIntentState {
            move_axis: (f32::NAN, 12.0),
            aim_axis: (24.0, f32::NEG_INFINITY),
            active_actions: vec![BinaryAction::Chat, BinaryAction::Boost, BinaryAction::Fire],
            ..LiveIntentState::default()
        };

        let key = runtime_snapshot_apply_key(&state);

        assert_eq!(key.0, (0.0, 0.0));
        assert_eq!(key.1, (0.0, 0.0));
        assert_eq!(
            key.8,
            vec![BinaryAction::Chat, BinaryAction::Boost, BinaryAction::Fire]
        );
    }

    #[test]
    fn runtime_snapshot_apply_key_distinguishes_transient_counts() {
        let base = LiveIntentState {
            move_axis: (1.0, 2.0),
            aim_axis: (3.0, 4.0),
            mining_tile: Some((5, 6)),
            building: true,
            last_config_tap_tile: Some((7, 8)),
            config_tap_count: 1,
            last_build_pulse: Some(BuildPulse {
                tile: (9, 10),
                breaking: false,
            }),
            build_pulse_count: 1,
            active_actions: vec![BinaryAction::Fire],
            pressed_actions: Vec::new(),
            released_actions: Vec::new(),
        };
        let bumped_counts = LiveIntentState {
            config_tap_count: 2,
            build_pulse_count: 3,
            ..base.clone()
        };

        assert_ne!(
            runtime_snapshot_apply_key(&base),
            runtime_snapshot_apply_key(&bumped_counts)
        );
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
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Chat],
        }));

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (2.0, 3.0),
            aim_axis: (40.0, 50.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
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
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Boost],
        }));
        assert_eq!(tracker.state().move_axis, (0.5, -0.5));

        tracker.set_override_snapshot(None);
        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (5.0, 6.0),
            aim_axis: (60.0, 70.0),
            mining_tile: Some((8, 9)),
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Boost],
        }));
        assert_eq!(tracker.state().move_axis, (5.0, 6.0));
        assert_eq!(tracker.state().aim_axis, (60.0, 70.0));
        assert_eq!(tracker.state().mining_tile, Some((8, 9)));
        assert!(!tracker.state().building);
        assert!(tracker.state().is_action_active(BinaryAction::Boost));
    }

    #[test]
    fn runtime_intent_tracker_supports_one_shot_override_without_persisting() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let runtime_snapshot = InputSnapshot {
            move_axis: (2.0, 3.0),
            aim_axis: (40.0, 50.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Fire],
        };
        let override_snapshot = InputSnapshot {
            move_axis: (0.5, -0.5),
            aim_axis: (10.0, 20.0),
            mining_tile: Some((3, 4)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Chat],
        };

        assert!(
            tracker.sample_runtime_snapshot_with_override(&runtime_snapshot, &override_snapshot)
        );
        assert_eq!(tracker.state().move_axis, (0.5, -0.5));
        assert!(tracker.state().is_action_active(BinaryAction::Chat));
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));

        assert!(tracker.sample_runtime_snapshot(&runtime_snapshot));
        assert_eq!(tracker.state().move_axis, (2.0, 3.0));
        assert_eq!(tracker.state().aim_axis, (40.0, 50.0));
        assert_eq!(tracker.state().mining_tile, None);
        assert!(!tracker.state().building);
        assert!(tracker.state().is_action_active(BinaryAction::Fire));
        assert!(!tracker.state().is_action_active(BinaryAction::Chat));
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Chat]);
    }

    #[test]
    fn runtime_intent_tracker_detects_building_state_changes() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: Vec::new(),
        }));
        assert!(tracker.state().building);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: Vec::new(),
        }));
        assert!(!tracker.state().building);
    }

    #[test]
    fn runtime_intent_tracker_batch_preserves_transient_edges() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let batch = vec![
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (2.0, 2.0),
                mining_tile: None,
                building: true,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (3.0, 4.0),
                mining_tile: Some((7, 8)),
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];

        assert!(tracker.sample_runtime_snapshot_batch(&batch));
        assert_eq!(tracker.state().move_axis, (0.0, 0.0));
        assert_eq!(tracker.state().aim_axis, (3.0, 4.0));
        assert_eq!(tracker.state().mining_tile, Some((7, 8)));
        assert!(!tracker.state().building);
        assert_eq!(tracker.state().pressed_actions, vec![BinaryAction::Fire]);
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Fire]);
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));

        tracker.state.last_config_tap_tile = Some((9, 10));
        tracker.state.last_build_pulse = Some(BuildPulse {
            tile: (11, 12),
            breaking: true,
        });

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
        assert!(tracker.state().last_config_tap_tile.is_none());
        assert!(tracker.state().last_build_pulse.is_none());
        assert!(tracker.state().pressed_actions.is_empty());
        assert!(tracker.state().released_actions.is_empty());
    }

    #[test]
    fn runtime_intent_tracker_transient_batch_keeps_runtime_state_authoritative() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let transient = vec![
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (16.0, 24.0),
                mining_tile: None,
                building: true,
                config_tap_tile: Some((6, 7)),
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (32.0, 48.0),
                mining_tile: Some((9, 10)),
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];
        let runtime_snapshot = InputSnapshot {
            move_axis: (9.0, 9.0),
            aim_axis: (99.0, 99.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Boost],
        };

        assert!(tracker.sample_runtime_snapshot_with_transient_batch(&transient, &runtime_snapshot));
        assert_eq!(tracker.state().move_axis, (9.0, 9.0));
        assert_eq!(tracker.state().aim_axis, (99.0, 99.0));
        assert_eq!(tracker.state().mining_tile, None);
        assert!(!tracker.state().building);
        assert_eq!(tracker.state().last_config_tap_tile, Some((6, 7)));
        assert_eq!(tracker.state().config_tap_count, 1);
        assert_eq!(
            tracker.state().pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Fire]);
        assert!(tracker.state().is_action_active(BinaryAction::Boost));
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn runtime_intent_tracker_records_config_tap_pulses_even_on_same_tile() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (1.0, 1.0),
            mining_tile: None,
            building: false,
            config_tap_tile: Some((6, 7)),
            build_pulse: None,
            active_actions: Vec::new(),
        }));
        assert_eq!(tracker.state().last_config_tap_tile, Some((6, 7)));
        assert_eq!(tracker.state().config_tap_count, 1);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (1.0, 1.0),
            mining_tile: None,
            building: false,
            config_tap_tile: Some((6, 7)),
            build_pulse: None,
            active_actions: Vec::new(),
        }));
        assert_eq!(tracker.state().last_config_tap_tile, Some((6, 7)));
        assert_eq!(tracker.state().config_tap_count, 2);
    }

    #[test]
    fn runtime_intent_tracker_records_build_pulses_from_transient_batch() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let transient = vec![InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (16.0, 24.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: Some(BuildPulse {
                tile: (9, 10),
                breaking: true,
            }),
            active_actions: vec![BinaryAction::Interact],
        }];
        let runtime_snapshot = InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (32.0, 48.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![],
        };

        assert!(tracker.sample_runtime_snapshot_with_transient_batch(&transient, &runtime_snapshot));
        assert_eq!(
            tracker.state().last_build_pulse,
            Some(BuildPulse {
                tile: (9, 10),
                breaking: true,
            })
        );
        assert_eq!(tracker.state().build_pulse_count, 1);
        assert!(!tracker.state().building);
    }

    #[test]
    fn runtime_intent_tracker_batch_counts_multiple_build_pulses_and_keeps_last_value() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let batch = vec![
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (1.0, 1.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: Some(BuildPulse {
                    tile: (2, 3),
                    breaking: false,
                }),
                active_actions: vec![],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (2.0, 2.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: Some(BuildPulse {
                    tile: (4, 5),
                    breaking: true,
                }),
                active_actions: vec![],
            },
            InputSnapshot {
                move_axis: (3.0, 4.0),
                aim_axis: (5.0, 6.0),
                mining_tile: Some((7, 8)),
                building: true,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];

        assert!(tracker.sample_runtime_snapshot_batch(&batch));
        assert_eq!(tracker.state().move_axis, (3.0, 4.0));
        assert_eq!(tracker.state().aim_axis, (5.0, 6.0));
        assert_eq!(tracker.state().mining_tile, Some((7, 8)));
        assert!(tracker.state().building);
        assert_eq!(
            tracker.state().last_build_pulse,
            Some(BuildPulse {
                tile: (4, 5),
                breaking: true,
            })
        );
        assert_eq!(tracker.state().build_pulse_count, 2);
    }

    #[test]
    fn runtime_intent_tracker_transient_batch_counts_multiple_build_pulses_and_keeps_last_value() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let transient = vec![
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (16.0, 24.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: Some(BuildPulse {
                    tile: (9, 10),
                    breaking: true,
                }),
                active_actions: vec![],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (32.0, 48.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: Some(BuildPulse {
                    tile: (11, 12),
                    breaking: false,
                }),
                active_actions: vec![],
            },
        ];
        let runtime_snapshot = InputSnapshot {
            move_axis: (1.0, 2.0),
            aim_axis: (3.0, 4.0),
            mining_tile: Some((5, 6)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![],
        };

        assert!(tracker.sample_runtime_snapshot_with_transient_batch(&transient, &runtime_snapshot));
        assert_eq!(tracker.state().move_axis, (1.0, 2.0));
        assert_eq!(tracker.state().aim_axis, (3.0, 4.0));
        assert_eq!(tracker.state().mining_tile, Some((5, 6)));
        assert!(tracker.state().building);
        assert_eq!(
            tracker.state().last_build_pulse,
            Some(BuildPulse {
                tile: (11, 12),
                breaking: false,
            })
        );
        assert_eq!(tracker.state().build_pulse_count, 2);
    }

    #[test]
    fn runtime_intent_tracker_batch_uses_persistent_override_snapshot_when_present() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        tracker.set_override_snapshot(Some(InputSnapshot {
            move_axis: (-0.5, 0.25),
            aim_axis: (10.0, 20.0),
            mining_tile: Some((3, 4)),
            building: true,
            config_tap_tile: Some((7, 8)),
            build_pulse: None,
            active_actions: vec![BinaryAction::Chat],
        }));
        let batch = vec![
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (2.0, 2.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (3.0, 4.0),
                mining_tile: Some((9, 10)),
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];

        assert!(tracker.sample_runtime_snapshot_batch(&batch));
        assert_eq!(tracker.state().move_axis, (-0.5, 0.25));
        assert_eq!(tracker.state().aim_axis, (10.0, 20.0));
        assert_eq!(tracker.state().mining_tile, Some((3, 4)));
        assert!(tracker.state().building);
        assert_eq!(tracker.state().last_config_tap_tile, Some((7, 8)));
        assert_eq!(tracker.state().config_tap_count, 1);
        assert_eq!(
            tracker.state().pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Chat]
        );
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Fire]);
        assert!(tracker.state().is_action_active(BinaryAction::Chat));
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn runtime_intent_tracker_batch_with_override_preserves_transient_edges() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let runtime_batch = vec![
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (2.0, 2.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (3.0, 4.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];
        let override_snapshot = InputSnapshot {
            move_axis: (-1.0, -2.0),
            aim_axis: (11.0, 12.0),
            mining_tile: Some((5, 6)),
            building: true,
            config_tap_tile: Some((13, 14)),
            build_pulse: None,
            active_actions: vec![BinaryAction::Boost],
        };

        assert!(
            tracker.sample_runtime_snapshot_batch_with_override(&runtime_batch, &override_snapshot)
        );
        assert_eq!(tracker.state().move_axis, (-1.0, -2.0));
        assert_eq!(tracker.state().aim_axis, (11.0, 12.0));
        assert_eq!(tracker.state().mining_tile, Some((5, 6)));
        assert!(tracker.state().building);
        assert_eq!(tracker.state().last_config_tap_tile, Some((13, 14)));
        assert_eq!(tracker.state().config_tap_count, 1);
        assert_eq!(
            tracker.state().pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Fire]);
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));
        assert!(tracker.state().is_action_active(BinaryAction::Boost));

        assert!(tracker.sample_runtime_snapshot_batch(&runtime_batch));
        assert_eq!(tracker.state().move_axis, (0.0, 0.0));
        assert_eq!(tracker.state().aim_axis, (3.0, 4.0));
        assert_eq!(tracker.state().mining_tile, None);
        assert!(!tracker.state().building);
        assert_eq!(tracker.state().pressed_actions, vec![BinaryAction::Fire]);
        assert_eq!(
            tracker.state().released_actions,
            vec![BinaryAction::Boost, BinaryAction::Fire]
        );
        assert!(!tracker.state().is_action_active(BinaryAction::Boost));
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn runtime_intent_tracker_batch_allows_override_without_runtime_samples() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        tracker.set_override_snapshot(Some(InputSnapshot {
            move_axis: (0.25, -0.25),
            aim_axis: (6.0, 7.0),
            mining_tile: Some((1, 2)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Interact],
        }));

        assert!(tracker.sample_runtime_snapshot_batch(&[]));
        assert_eq!(tracker.state().move_axis, (0.25, -0.25));
        assert_eq!(tracker.state().aim_axis, (6.0, 7.0));
        assert_eq!(tracker.state().mining_tile, Some((1, 2)));
        assert!(tracker.state().building);
        assert_eq!(
            tracker.state().pressed_actions,
            vec![BinaryAction::Interact]
        );
        assert!(tracker.state().is_action_active(BinaryAction::Interact));

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
    }

    #[test]
    fn runtime_intent_tracker_empty_batch_clears_pressed_edges_without_dropping_active_actions() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot_batch(&[InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (2.0, 3.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Fire],
        }]));
        assert_eq!(tracker.state().pressed_actions, vec![BinaryAction::Fire]);
        assert!(tracker.state().released_actions.is_empty());
        assert!(tracker.state().is_action_active(BinaryAction::Fire));

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
        assert!(tracker.state().pressed_actions.is_empty());
        assert!(tracker.state().released_actions.is_empty());
        assert!(tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn runtime_intent_tracker_empty_batch_clears_released_edges_after_batch_release() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot_batch(&[InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (2.0, 3.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Fire],
        }]));

        assert!(tracker.sample_runtime_snapshot_batch(&[
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (2.0, 3.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (4.0, 5.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ]));
        assert!(tracker.state().pressed_actions.is_empty());
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Fire]);
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
        assert!(tracker.state().pressed_actions.is_empty());
        assert!(tracker.state().released_actions.is_empty());
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn runtime_intent_tracker_empty_batch_clears_transient_edges_without_desyncing_mapper_state() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let active_snapshot = InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (2.0, 3.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Fire],
        };

        assert!(tracker.sample_runtime_snapshot_batch(&[active_snapshot.clone()]));
        assert_eq!(tracker.state().pressed_actions, vec![BinaryAction::Fire]);
        assert!(tracker.state().released_actions.is_empty());
        assert!(tracker.state().is_action_active(BinaryAction::Fire));

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
        assert!(tracker.state().pressed_actions.is_empty());
        assert!(tracker.state().released_actions.is_empty());
        assert!(tracker.state().is_action_active(BinaryAction::Fire));

        assert!(tracker.sample_runtime_snapshot_batch(&[active_snapshot]));
        assert_eq!(tracker.state().pressed_actions, vec![BinaryAction::Fire]);
        assert!(tracker.state().released_actions.is_empty());
        assert!(tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn runtime_intent_tracker_empty_batch_preserves_active_actions_and_counts() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (2.0, 3.0),
            mining_tile: Some((4, 5)),
            building: true,
            config_tap_tile: Some((6, 7)),
            build_pulse: Some(BuildPulse {
                tile: (8, 9),
                breaking: true,
            }),
            active_actions: vec![BinaryAction::Fire],
        }));
        assert_eq!(tracker.state().config_tap_count, 1);
        assert_eq!(tracker.state().build_pulse_count, 1);
        assert!(tracker.state().is_action_active(BinaryAction::Fire));
        assert_eq!(tracker.state().pressed_actions, vec![BinaryAction::Fire]);

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
        assert_eq!(tracker.state().config_tap_count, 1);
        assert_eq!(tracker.state().build_pulse_count, 1);
        assert!(tracker.state().is_action_active(BinaryAction::Fire));
        assert!(tracker.state().pressed_actions.is_empty());
        assert!(tracker.state().released_actions.is_empty());
        assert!(tracker.state().last_config_tap_tile.is_none());
        assert!(tracker.state().last_build_pulse.is_none());
        assert!(!tracker.binding_profile().has_transient_signals());
    }

    #[test]
    fn runtime_intent_tracker_empty_batch_then_inactive_snapshot_releases_previous_actions() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot_batch(&[InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (2.0, 3.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Fire],
        }]));
        assert!(tracker.state().is_action_active(BinaryAction::Fire));

        assert!(!tracker.sample_runtime_snapshot_batch(&[]));
        assert!(tracker.state().is_action_active(BinaryAction::Fire));

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (4.0, 5.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![],
        }));
        assert_eq!(tracker.state().released_actions, vec![BinaryAction::Fire]);
        assert!(!tracker.state().is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn probe_sampled_snapshot_roundtrips_through_live_tracker_without_semantic_drift() {
        let sample = RuntimeInputSample {
            position: Some((5.0, 6.0)),
            pointer: Some((16.0, 24.0)),
            velocity: (1.5, -2.5),
            mining_tile: Some((9, 11)),
            building: true,
            shooting: true,
            boosting: true,
            chatting: false,
        };
        let sampled_snapshot = sampled_probe_snapshot(sample);
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_probe_input(sample));

        let profile = tracker.binding_profile();
        assert_eq!(profile.move_axis, sampled_snapshot.move_axis);
        assert_eq!(profile.aim_axis, sampled_snapshot.aim_axis);
        assert_eq!(profile.mining_tile, sampled_snapshot.mining_tile);
        assert_eq!(profile.building, sampled_snapshot.building);
        assert_eq!(profile.active_actions, sampled_snapshot.active_actions);
        assert_eq!(profile.pressed_actions, sampled_snapshot.active_actions);
        assert!(profile.released_actions.is_empty());
    }

    #[test]
    fn probe_non_finite_sample_remains_idle_after_mapper_and_live_tracker_normalization() {
        let sample = RuntimeInputSample {
            position: Some((f32::INFINITY, 6.0)),
            pointer: Some((f32::NAN, 24.0)),
            velocity: (f32::NEG_INFINITY, f32::NAN),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        };
        let sampled_snapshot = sampled_probe_snapshot(sample);
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert_eq!(classify_runtime_input_sample(sample).label(), "idle");
        assert_eq!(sampled_snapshot, InputSnapshot::default());
        assert!(!tracker.sample_probe_input(sample));
        assert_eq!(tracker.binding_profile().move_axis, (0.0, 0.0));
        assert_eq!(tracker.binding_profile().aim_axis, (0.0, 0.0));
        assert!(tracker.binding_profile().active_actions.is_empty());
        assert!(tracker.binding_profile().is_idle());
    }

    #[test]
    fn transient_probe_batch_keeps_final_runtime_state_while_preserving_action_edges() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);
        let transient_probe_samples = vec![
            RuntimeInputSample {
                position: Some((2.0, 3.0)),
                pointer: Some((10.0, 20.0)),
                velocity: (1.0, 0.0),
                mining_tile: Some((3, 4)),
                building: true,
                shooting: true,
                boosting: false,
                chatting: false,
            },
            RuntimeInputSample {
                position: Some((4.0, 5.0)),
                pointer: Some((30.0, 40.0)),
                velocity: (0.0, 0.0),
                mining_tile: None,
                building: false,
                shooting: false,
                boosting: false,
                chatting: false,
            },
        ];
        let runtime_probe_sample = RuntimeInputSample {
            position: Some((7.0, 8.0)),
            pointer: Some((50.0, 60.0)),
            velocity: (-0.5, 0.25),
            mining_tile: Some((9, 11)),
            building: true,
            shooting: false,
            boosting: true,
            chatting: true,
        };
        let runtime_snapshot = sampled_probe_snapshot(runtime_probe_sample);

        assert!(tracker.sample_probe_input_with_transient_batch(
            &transient_probe_samples,
            runtime_probe_sample,
        ));

        let profile = tracker.binding_profile();
        assert_eq!(profile.move_axis, runtime_snapshot.move_axis);
        assert_eq!(profile.aim_axis, runtime_snapshot.aim_axis);
        assert_eq!(profile.mining_tile, runtime_snapshot.mining_tile);
        assert_eq!(profile.building, runtime_snapshot.building);
        assert_eq!(profile.active_actions, runtime_snapshot.active_actions);
        assert_eq!(
            profile.pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost, BinaryAction::Chat]
        );
        assert_eq!(profile.released_actions, vec![BinaryAction::Fire]);
    }

    #[test]
    fn binding_profile_reflects_live_state_and_transient_edges() {
        let mut tracker = RuntimeIntentTracker::new(IntentSamplingMode::LiveSampling);

        assert!(tracker.sample_runtime_snapshot(&InputSnapshot {
            move_axis: (1.0, -1.0),
            aim_axis: (8.0, 12.0),
            mining_tile: Some((7, 8)),
            building: true,
            config_tap_tile: Some((3, 4)),
            build_pulse: Some(BuildPulse {
                tile: (9, 10),
                breaking: true,
            }),
            active_actions: vec![BinaryAction::Fire, BinaryAction::Boost],
        }));

        let profile = tracker.binding_profile();
        assert_eq!(profile.move_axis, (1.0, -1.0));
        assert_eq!(profile.aim_axis, (8.0, 12.0));
        assert_eq!(profile.mining_tile, Some((7, 8)));
        assert!(profile.building);
        assert_eq!(profile.last_config_tap_tile, Some((3, 4)));
        assert_eq!(
            profile.last_build_pulse,
            Some(BuildPulse {
                tile: (9, 10),
                breaking: true,
            })
        );
        assert_eq!(
            profile.active_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert!(profile.has_motion());
        assert!(profile.has_aim());
        assert!(profile.has_transient_signals());
        assert!(!profile.is_idle());
        assert_eq!(
            profile.summary_label(),
            "move=1,-1 aim=8,12 mining=7,8 building=on active=2 transient=tap=3,4 pulse=9,10,break pressed=2"
        );
    }

    #[test]
    fn binding_profile_summary_label_for_idle_profile_is_minimal() {
        let profile = LiveIntentBindingProfile {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            last_config_tap_tile: None,
            last_build_pulse: None,
            active_actions: Vec::new(),
            pressed_actions: Vec::new(),
            released_actions: Vec::new(),
        };

        assert_eq!(
            profile.summary_label(),
            "move=0,0 aim=0,0 mining=none building=off active=0 transient=none"
        );
        assert!(profile.is_idle());
        assert!(!profile.has_motion());
        assert!(!profile.has_aim());
        assert!(!profile.has_transient_signals());
    }

    #[test]
    fn binding_profile_summary_label_reflects_motion_without_transient_edges() {
        let profile = LiveIntentBindingProfile {
            move_axis: (1.0, -1.0),
            aim_axis: (8.0, 12.0),
            mining_tile: Some((7, 8)),
            building: true,
            last_config_tap_tile: None,
            last_build_pulse: None,
            active_actions: vec![BinaryAction::Fire],
            pressed_actions: Vec::new(),
            released_actions: Vec::new(),
        };

        assert_eq!(
            profile.summary_label(),
            "move=1,-1 aim=8,12 mining=7,8 building=on active=1 transient=none"
        );
        assert!(profile.has_motion());
        assert!(profile.has_aim());
        assert!(!profile.has_transient_signals());
        assert!(!profile.is_idle());
    }

    #[test]
    fn binding_profile_summary_label_reflects_transient_edges() {
        let profile = LiveIntentBindingProfile {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            last_config_tap_tile: Some((3, 4)),
            last_build_pulse: Some(BuildPulse {
                tile: (9, 10),
                breaking: false,
            }),
            active_actions: vec![BinaryAction::Boost, BinaryAction::Chat],
            pressed_actions: vec![BinaryAction::Boost],
            released_actions: vec![BinaryAction::Chat],
        };

        assert_eq!(
            profile.summary_label(),
            "move=0,0 aim=0,0 mining=none building=off active=2 transient=tap=3,4 pulse=9,10,place pressed=1 released=1"
        );
        assert!(profile.has_transient_signals());
        assert!(!profile.is_idle());
    }

    #[test]
    fn transient_label_formats_all_edges_in_order_and_handles_empty_state() {
        let profile = LiveIntentBindingProfile {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            last_config_tap_tile: Some((3, 4)),
            last_build_pulse: Some(BuildPulse {
                tile: (9, 10),
                breaking: true,
            }),
            active_actions: vec![BinaryAction::Fire],
            pressed_actions: vec![BinaryAction::Boost, BinaryAction::Chat],
            released_actions: vec![BinaryAction::Chat],
        };

        assert_eq!(
            transient_label(&profile),
            "tap=3,4 pulse=9,10,break pressed=2 released=1"
        );
        assert_eq!(
            profile.summary_label(),
            "move=0,0 aim=0,0 mining=none building=off active=1 transient=tap=3,4 pulse=9,10,break pressed=2 released=1"
        );

        let empty_profile = LiveIntentBindingProfile {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: false,
            last_config_tap_tile: None,
            last_build_pulse: None,
            active_actions: Vec::new(),
            pressed_actions: Vec::new(),
            released_actions: Vec::new(),
        };

        assert_eq!(transient_label(&empty_profile), "none");
        assert_eq!(
            empty_profile.summary_label(),
            "move=0,0 aim=0,0 mining=none building=off active=0 transient=none"
        );
    }

    #[test]
    fn clear_transient_edges_clears_only_transient_state() {
        let mut state = LiveIntentState {
            move_axis: (1.0, -1.0),
            aim_axis: (8.0, 12.0),
            mining_tile: Some((7, 8)),
            building: true,
            last_config_tap_tile: Some((3, 4)),
            config_tap_count: 2,
            last_build_pulse: Some(BuildPulse {
                tile: (9, 10),
                breaking: true,
            }),
            build_pulse_count: 3,
            active_actions: vec![BinaryAction::Fire, BinaryAction::Boost],
            pressed_actions: vec![BinaryAction::Chat],
            released_actions: vec![BinaryAction::Interact],
        };

        state.clear_transient_edges();

        assert_eq!(state.move_axis, (1.0, -1.0));
        assert_eq!(state.aim_axis, (8.0, 12.0));
        assert_eq!(state.mining_tile, Some((7, 8)));
        assert!(state.building);
        assert_eq!(state.config_tap_count, 2);
        assert_eq!(state.build_pulse_count, 3);
        assert_eq!(
            state.active_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert!(state.last_config_tap_tile.is_none());
        assert!(state.last_build_pulse.is_none());
        assert!(state.pressed_actions.is_empty());
        assert!(state.released_actions.is_empty());
    }

    #[test]
    fn push_unique_and_remove_action_keep_action_lists_deduped() {
        let mut actions = vec![BinaryAction::Fire];

        push_unique(&mut actions, BinaryAction::Fire);
        push_unique(&mut actions, BinaryAction::Boost);
        push_unique(&mut actions, BinaryAction::Boost);
        assert_eq!(actions, vec![BinaryAction::Fire, BinaryAction::Boost]);

        remove_action(&mut actions, BinaryAction::Fire);
        assert_eq!(actions, vec![BinaryAction::Boost]);

        remove_action(&mut actions, BinaryAction::Fire);
        assert_eq!(actions, vec![BinaryAction::Boost]);

        remove_action(&mut actions, BinaryAction::Boost);
        assert!(actions.is_empty());
    }
}
