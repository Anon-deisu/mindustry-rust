use crate::intent::{BinaryAction, BuildPulse, PlayerIntent};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputSnapshot {
    pub move_axis: (f32, f32),
    pub aim_axis: (f32, f32),
    pub mining_tile: Option<(i32, i32)>,
    pub building: bool,
    pub config_tap_tile: Option<(i32, i32)>,
    pub build_pulse: Option<BuildPulse>,
    pub active_actions: Vec<BinaryAction>,
}

impl InputSnapshot {
    pub fn summary_label(&self) -> String {
        format!(
            "move=1 aim=1 mining={} building={} config={} build-pulse={} action={}",
            option_count(self.mining_tile.is_some()),
            bool_count(self.building),
            option_count(self.config_tap_tile.is_some()),
            option_count(self.build_pulse.is_some()),
            self.active_actions.len(),
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IntentSamplingMode {
    #[default]
    EdgeMapped,
    LiveSampling,
}

pub trait IntentMapper {
    fn map_snapshot(&mut self, snapshot: &InputSnapshot) -> Vec<PlayerIntent>;
}

#[derive(Debug, Default)]
pub struct StatelessIntentMapper {
    active_actions_prev: Vec<BinaryAction>,
    sampling_mode: IntentSamplingMode,
}

impl StatelessIntentMapper {
    pub fn new(sampling_mode: IntentSamplingMode) -> Self {
        Self {
            active_actions_prev: Vec::new(),
            sampling_mode,
        }
    }

    pub fn sampling_mode(&self) -> IntentSamplingMode {
        self.sampling_mode
    }

    pub fn map_latest_snapshot(&mut self, snapshots: &[InputSnapshot]) -> Vec<PlayerIntent> {
        if let Some(snapshot) = snapshots.last() {
            self.map_snapshot(snapshot)
        } else {
            self.active_actions_prev.clear();
            Vec::new()
        }
    }

    pub fn map_snapshot_batch(&mut self, snapshots: &[InputSnapshot]) -> Vec<PlayerIntent> {
        let Some(last_snapshot) = snapshots.last() else {
            self.active_actions_prev.clear();
            return Vec::new();
        };
        self.map_snapshot_batch_with_final_snapshot(
            &snapshots[..snapshots.len().saturating_sub(1)],
            last_snapshot,
        )
    }

    pub fn map_snapshot_batch_with_final_snapshot(
        &mut self,
        snapshots: &[InputSnapshot],
        final_snapshot: &InputSnapshot,
    ) -> Vec<PlayerIntent> {
        let mut edge_intents = Vec::new();
        for snapshot in snapshots {
            let mapped = self.map_snapshot(snapshot);
            edge_intents.extend(mapped.into_iter().skip(4));
        }

        let mut combined = self.map_snapshot(final_snapshot);
        let final_edges = combined.split_off(4);
        combined.extend(edge_intents);
        combined.extend(final_edges);
        combined
    }

    pub fn map_snapshot_batch_or_override(
        &mut self,
        snapshots: &[InputSnapshot],
        override_snapshot: Option<&InputSnapshot>,
    ) -> Vec<PlayerIntent> {
        if let Some(snapshot) = override_snapshot {
            self.map_snapshot(snapshot)
        } else {
            self.map_snapshot_batch(snapshots)
        }
    }
}

impl IntentMapper for StatelessIntentMapper {
    fn map_snapshot(&mut self, snapshot: &InputSnapshot) -> Vec<PlayerIntent> {
        let active_actions = canonicalize_actions(&snapshot.active_actions);
        let mut intents =
            Vec::with_capacity(4 + active_actions.len() + self.active_actions_prev.len());

        let move_axis = normalize_axis(snapshot.move_axis);
        let aim_axis = normalize_axis(snapshot.aim_axis);
        intents.push(PlayerIntent::SetMoveAxis {
            x: move_axis.0,
            y: move_axis.1,
        });
        intents.push(PlayerIntent::SetAimAxis {
            x: aim_axis.0,
            y: aim_axis.1,
        });
        intents.push(PlayerIntent::SetMiningTile {
            tile: snapshot.mining_tile,
        });
        intents.push(PlayerIntent::SetBuilding {
            building: snapshot.building,
        });
        if let Some(tile) = snapshot.config_tap_tile {
            intents.push(PlayerIntent::ConfigTap { tile });
        }
        if let Some(pulse) = snapshot.build_pulse {
            intents.push(PlayerIntent::BuildPulse(pulse));
        }

        match self.sampling_mode {
            IntentSamplingMode::EdgeMapped => {
                for action in &active_actions {
                    let intent = if self.active_actions_prev.contains(action) {
                        PlayerIntent::ActionHeld(*action)
                    } else {
                        PlayerIntent::ActionPressed(*action)
                    };
                    intents.push(intent);
                }
            }
            IntentSamplingMode::LiveSampling => {
                for action in &active_actions {
                    if !self.active_actions_prev.contains(action) {
                        intents.push(PlayerIntent::ActionPressed(*action));
                    }
                }
            }
        }

        for action in &self.active_actions_prev {
            if !active_actions.contains(action) {
                intents.push(PlayerIntent::ActionReleased(*action));
            }
        }

        self.active_actions_prev = active_actions;

        intents
    }
}

fn canonicalize_actions(actions: &[BinaryAction]) -> Vec<BinaryAction> {
    let mut canonical = Vec::with_capacity(actions.len());
    for action in actions {
        if !canonical.contains(action) {
            canonical.push(*action);
        }
    }
    canonical.sort_by_key(action_order_key);
    canonical
}

fn action_order_key(action: &BinaryAction) -> u8 {
    match action {
        BinaryAction::MoveUp => 0,
        BinaryAction::MoveDown => 1,
        BinaryAction::MoveLeft => 2,
        BinaryAction::MoveRight => 3,
        BinaryAction::Fire => 4,
        BinaryAction::Boost => 5,
        BinaryAction::Chat => 6,
        BinaryAction::Interact => 7,
    }
}

fn normalize_axis(axis: (f32, f32)) -> (f32, f32) {
    if axis.0.is_finite() && axis.1.is_finite() {
        axis
    } else {
        (0.0, 0.0)
    }
}

fn bool_count(value: bool) -> usize {
    usize::from(value)
}

fn option_count(value: bool) -> usize {
    usize::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(
        move_axis: (f32, f32),
        aim_axis: (f32, f32),
        active_actions: &[BinaryAction],
    ) -> InputSnapshot {
        snapshot_with_details(move_axis, aim_axis, None, false, active_actions)
    }

    fn snapshot_with_details(
        move_axis: (f32, f32),
        aim_axis: (f32, f32),
        mining_tile: Option<(i32, i32)>,
        building: bool,
        active_actions: &[BinaryAction],
    ) -> InputSnapshot {
        InputSnapshot {
            move_axis,
            aim_axis,
            mining_tile,
            building,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: active_actions.to_vec(),
        }
    }

    #[test]
    fn input_snapshot_summary_label_counts_present_fields_and_actions() {
        let snapshot = InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (0.0, 1.0),
            mining_tile: Some((7, 9)),
            building: true,
            config_tap_tile: Some((11, 13)),
            build_pulse: Some(BuildPulse {
                tile: (3, 4),
                breaking: false,
            }),
            active_actions: vec![BinaryAction::Fire, BinaryAction::Boost],
        };

        assert_eq!(
            snapshot.summary_label(),
            "move=1 aim=1 mining=1 building=1 config=1 build-pulse=1 action=2"
        );
    }

    #[test]
    fn press_hold_release_edges_are_emitted() {
        let mut mapper = StatelessIntentMapper::default();

        assert_eq!(
            mapper.map_snapshot(&snapshot((1.0, 0.0), (0.0, 1.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 1.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 1.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((1.0, 0.0), (0.0, 1.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 1.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 1.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionHeld(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (0.0, 1.0), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 1.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn duplicate_actions_are_deduplicated_before_edge_mapping() {
        let mut mapper = StatelessIntentMapper::default();

        assert_eq!(
            mapper.map_snapshot(&snapshot(
                (0.0, 0.0),
                (2.0, 3.0),
                &[
                    BinaryAction::Fire,
                    BinaryAction::Fire,
                    BinaryAction::Boost,
                    BinaryAction::Fire,
                ]
            )),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 2.0, y: 3.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Boost),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot(
                (0.0, 0.0),
                (2.0, 3.0),
                &[BinaryAction::Boost, BinaryAction::Boost, BinaryAction::Fire,]
            )),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 2.0, y: 3.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionHeld(BinaryAction::Fire),
                PlayerIntent::ActionHeld(BinaryAction::Boost),
            ]
        );
    }

    #[test]
    fn action_edge_order_is_stable_across_input_permutations() {
        let mut mapper = StatelessIntentMapper::default();

        let first = mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (1.0, 1.0),
            &[BinaryAction::Chat, BinaryAction::Boost, BinaryAction::Fire],
        ));
        assert_eq!(
            first,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 1.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Boost),
                PlayerIntent::ActionPressed(BinaryAction::Chat),
            ]
        );

        let second = mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (1.0, 1.0),
            &[BinaryAction::Boost, BinaryAction::Fire, BinaryAction::Chat],
        ));
        assert_eq!(
            second,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 1.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionHeld(BinaryAction::Fire),
                PlayerIntent::ActionHeld(BinaryAction::Boost),
                PlayerIntent::ActionHeld(BinaryAction::Chat),
            ]
        );
    }

    #[test]
    fn released_action_order_is_stable_when_multiple_actions_drop() {
        let mut mapper = StatelessIntentMapper::default();

        mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (0.0, 0.0),
            &[BinaryAction::Chat, BinaryAction::Boost, BinaryAction::Fire],
        ));

        let released = mapper.map_snapshot(&snapshot((0.0, 0.0), (0.0, 0.0), &[]));
        assert_eq!(
            released,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionReleased(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Boost),
                PlayerIntent::ActionReleased(BinaryAction::Chat),
            ]
        );
    }

    #[test]
    fn released_actions_emit_when_snapshot_becomes_empty() {
        let mut mapper = StatelessIntentMapper::default();

        mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (0.0, 0.0),
            &[BinaryAction::Fire, BinaryAction::Boost],
        ));

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (0.0, 0.0), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionReleased(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Boost),
            ]
        );
    }

    #[test]
    fn axis_intents_are_always_emitted_even_without_action_changes() {
        let mut mapper = StatelessIntentMapper::default();

        let first = mapper.map_snapshot(&snapshot((0.5, -0.5), (4.0, 8.0), &[BinaryAction::Chat]));
        let second =
            mapper.map_snapshot(&snapshot((-1.0, 1.0), (9.0, 10.0), &[BinaryAction::Chat]));

        assert_eq!(
            first[0..2],
            [
                PlayerIntent::SetMoveAxis { x: 0.5, y: -0.5 },
                PlayerIntent::SetAimAxis { x: 4.0, y: 8.0 },
            ]
        );
        assert_eq!(
            second[0..2],
            [
                PlayerIntent::SetMoveAxis { x: -1.0, y: 1.0 },
                PlayerIntent::SetAimAxis { x: 9.0, y: 10.0 },
            ]
        );
        assert_eq!(second[2], PlayerIntent::SetMiningTile { tile: None });
        assert_eq!(second[3], PlayerIntent::SetBuilding { building: false });
        assert_eq!(second[4], PlayerIntent::ActionHeld(BinaryAction::Chat));
    }

    #[test]
    fn non_finite_axes_are_normalized_before_mapping() {
        let mut mapper = StatelessIntentMapper::default();

        assert_eq!(
            mapper.map_snapshot(&snapshot((f32::NAN, 1.0), (2.0, f32::INFINITY), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
            ]
        );
    }

    #[test]
    fn live_sampling_mode_emits_only_press_and_release_edges() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (1.0, 2.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 2.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (1.0, 2.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 2.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (3.0, 4.0), &[BinaryAction::Boost])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Boost),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (3.0, 4.0), &[BinaryAction::Boost])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (5.0, 6.0), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 5.0, y: 6.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionReleased(BinaryAction::Boost),
            ]
        );
    }

    #[test]
    fn map_latest_snapshot_uses_last_sample_in_batch() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        let batch = vec![
            snapshot((0.0, 0.0), (1.0, 1.0), &[BinaryAction::Fire]),
            snapshot((0.5, 0.5), (2.0, 2.0), &[BinaryAction::Boost]),
        ];
        assert_eq!(
            mapper.map_latest_snapshot(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.5, y: 0.5 },
                PlayerIntent::SetAimAxis { x: 2.0, y: 2.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Boost),
            ]
        );

        assert!(mapper.map_latest_snapshot(&[]).is_empty());
    }

    #[test]
    fn map_latest_snapshot_empty_batch_clears_action_history() {
        let mut mapper = StatelessIntentMapper::default();
        let batch = vec![snapshot((0.0, 0.0), (0.0, 0.0), &[BinaryAction::Fire])];

        assert_eq!(
            mapper.map_latest_snapshot(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );

        assert!(mapper.map_latest_snapshot(&[]).is_empty());

        assert_eq!(
            mapper.map_latest_snapshot(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_preserves_transient_edges_with_final_runtime_axes() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        let batch = vec![
            snapshot((1.0, 0.0), (2.0, 2.0), &[BinaryAction::Fire]),
            snapshot((0.0, 0.0), (3.0, 4.0), &[]),
        ];
        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_empty_batch_resets_action_history() {
        let mut mapper = StatelessIntentMapper::default();
        let batch = vec![snapshot((0.0, 0.0), (0.0, 0.0), &[BinaryAction::Fire])];

        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );

        assert!(mapper.map_snapshot_batch(&[]).is_empty());

        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_keeps_edge_order_across_multiple_samples() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        let batch = vec![
            snapshot((0.0, 0.0), (1.0, 1.0), &[BinaryAction::Fire]),
            snapshot(
                (0.5, 0.5),
                (2.0, 2.0),
                &[BinaryAction::Fire, BinaryAction::Boost],
            ),
            snapshot((0.5, 0.5), (3.0, 3.0), &[BinaryAction::Boost]),
        ];
        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.5, y: 0.5 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 3.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Boost),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_keeps_final_building_bit_with_transient_edges() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        let batch = vec![
            snapshot_with_details(
                (1.0, 0.0),
                (2.0, 2.0),
                Some((7, 8)),
                true,
                &[BinaryAction::Fire],
            ),
            snapshot_with_details((0.0, 0.0), (3.0, 4.0), None, false, &[]),
        ];
        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn mining_tile_is_emitted_as_structured_intent() {
        let mut mapper = StatelessIntentMapper::default();

        let intents = mapper.map_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (3.0, 4.0),
            mining_tile: Some((7, 9)),
            building: false,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Interact],
        });

        assert_eq!(
            intents,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::SetMiningTile { tile: Some((7, 9)) },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Interact),
            ]
        );
    }

    #[test]
    fn building_is_emitted_as_structured_intent() {
        let mut mapper = StatelessIntentMapper::default();

        let intents = mapper.map_snapshot(&InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (1.0, 2.0),
            mining_tile: None,
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: Vec::new(),
        });

        assert_eq!(
            intents,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 2.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: true },
            ]
        );
    }

    #[test]
    fn config_tap_is_emitted_as_transient_structured_intent() {
        let mut mapper = StatelessIntentMapper::default();

        let intents = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, -1.0),
            aim_axis: (5.0, 6.0),
            mining_tile: None,
            building: false,
            config_tap_tile: Some((11, 13)),
            build_pulse: None,
            active_actions: vec![BinaryAction::Interact],
        });

        assert_eq!(
            intents,
            vec![
                PlayerIntent::SetMoveAxis { x: 1.0, y: -1.0 },
                PlayerIntent::SetAimAxis { x: 5.0, y: 6.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ConfigTap { tile: (11, 13) },
                PlayerIntent::ActionPressed(BinaryAction::Interact),
            ]
        );
    }

    #[test]
    fn build_pulse_is_emitted_as_transient_structured_intent() {
        let mut mapper = StatelessIntentMapper::default();

        let intents = mapper.map_snapshot(&InputSnapshot {
            move_axis: (1.0, -1.0),
            aim_axis: (5.0, 6.0),
            mining_tile: None,
            building: false,
            config_tap_tile: None,
            build_pulse: Some(BuildPulse {
                tile: (11, 13),
                breaking: true,
            }),
            active_actions: vec![BinaryAction::Interact],
        });

        assert_eq!(
            intents,
            vec![
                PlayerIntent::SetMoveAxis { x: 1.0, y: -1.0 },
                PlayerIntent::SetAimAxis { x: 5.0, y: 6.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::BuildPulse(BuildPulse {
                    tile: (11, 13),
                    breaking: true,
                }),
                PlayerIntent::ActionPressed(BinaryAction::Interact),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_preserves_transient_config_tap_from_earlier_sample() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let batch = vec![
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (1.0, 2.0),
                mining_tile: None,
                building: false,
                config_tap_tile: Some((3, 4)),
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (7.0, 8.0),
                aim_axis: (9.0, 10.0),
                mining_tile: Some((5, 6)),
                building: true,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];

        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 7.0, y: 8.0 },
                PlayerIntent::SetAimAxis { x: 9.0, y: 10.0 },
                PlayerIntent::SetMiningTile { tile: Some((5, 6)) },
                PlayerIntent::SetBuilding { building: true },
                PlayerIntent::ConfigTap { tile: (3, 4) },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_preserves_transient_build_pulse_from_earlier_sample() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let batch = vec![
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (1.0, 2.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: Some(BuildPulse {
                    tile: (3, 4),
                    breaking: true,
                }),
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (7.0, 8.0),
                aim_axis: (9.0, 10.0),
                mining_tile: Some((5, 6)),
                building: true,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];

        assert_eq!(
            mapper.map_snapshot_batch(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 7.0, y: 8.0 },
                PlayerIntent::SetAimAxis { x: 9.0, y: 10.0 },
                PlayerIntent::SetMiningTile { tile: Some((5, 6)) },
                PlayerIntent::SetBuilding { building: true },
                PlayerIntent::BuildPulse(BuildPulse {
                    tile: (3, 4),
                    breaking: true,
                }),
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_with_final_snapshot_preserves_both_transient_structured_intents() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let transient = vec![InputSnapshot {
            move_axis: (1.0, 0.0),
            aim_axis: (16.0, 24.0),
            mining_tile: Some((3, 4)),
            building: true,
            config_tap_tile: Some((5, 6)),
            build_pulse: Some(BuildPulse {
                tile: (7, 8),
                breaking: false,
            }),
            active_actions: vec![BinaryAction::Fire],
        }];
        let runtime_snapshot = InputSnapshot {
            move_axis: (9.0, 9.0),
            aim_axis: (99.0, 99.0),
            mining_tile: Some((7, 8)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![],
        };

        assert_eq!(
            mapper.map_snapshot_batch_with_final_snapshot(&transient, &runtime_snapshot),
            vec![
                PlayerIntent::SetMoveAxis { x: 9.0, y: 9.0 },
                PlayerIntent::SetAimAxis { x: 99.0, y: 99.0 },
                PlayerIntent::SetMiningTile { tile: Some((7, 8)) },
                PlayerIntent::SetBuilding { building: true },
                PlayerIntent::ConfigTap { tile: (5, 6) },
                PlayerIntent::BuildPulse(BuildPulse {
                    tile: (7, 8),
                    breaking: false,
                }),
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_with_final_snapshot_keeps_runtime_state_and_transient_edges() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let transient = vec![
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (16.0, 24.0),
                mining_tile: Some((3, 4)),
                building: true,
                config_tap_tile: Some((5, 6)),
                build_pulse: None,
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (32.0, 48.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];
        let runtime_snapshot = InputSnapshot {
            move_axis: (9.0, 9.0),
            aim_axis: (99.0, 99.0),
            mining_tile: Some((7, 8)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Boost],
        };

        assert_eq!(
            mapper.map_snapshot_batch_with_final_snapshot(&transient, &runtime_snapshot),
            vec![
                PlayerIntent::SetMoveAxis { x: 9.0, y: 9.0 },
                PlayerIntent::SetAimAxis { x: 99.0, y: 99.0 },
                PlayerIntent::SetMiningTile { tile: Some((7, 8)) },
                PlayerIntent::SetBuilding { building: true },
                PlayerIntent::ConfigTap { tile: (5, 6) },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Boost),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_with_final_snapshot_preserves_transient_build_pulse() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let transient = vec![
            InputSnapshot {
                move_axis: (1.0, 0.0),
                aim_axis: (16.0, 24.0),
                mining_tile: Some((3, 4)),
                building: true,
                config_tap_tile: None,
                build_pulse: Some(BuildPulse {
                    tile: (5, 6),
                    breaking: false,
                }),
                active_actions: vec![BinaryAction::Fire],
            },
            InputSnapshot {
                move_axis: (0.0, 0.0),
                aim_axis: (32.0, 48.0),
                mining_tile: None,
                building: false,
                config_tap_tile: None,
                build_pulse: None,
                active_actions: vec![],
            },
        ];
        let runtime_snapshot = InputSnapshot {
            move_axis: (9.0, 9.0),
            aim_axis: (99.0, 99.0),
            mining_tile: Some((7, 8)),
            building: true,
            config_tap_tile: None,
            build_pulse: None,
            active_actions: vec![BinaryAction::Boost],
        };

        assert_eq!(
            mapper.map_snapshot_batch_with_final_snapshot(&transient, &runtime_snapshot),
            vec![
                PlayerIntent::SetMoveAxis { x: 9.0, y: 9.0 },
                PlayerIntent::SetAimAxis { x: 99.0, y: 99.0 },
                PlayerIntent::SetMiningTile { tile: Some((7, 8)) },
                PlayerIntent::SetBuilding { building: true },
                PlayerIntent::BuildPulse(BuildPulse {
                    tile: (5, 6),
                    breaking: false,
                }),
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Boost),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_or_override_prefers_override_snapshot_over_runtime_batch() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let batch = vec![
            snapshot((1.0, 0.0), (2.0, 2.0), &[BinaryAction::Fire]),
            snapshot((0.0, 0.0), (3.0, 4.0), &[]),
        ];
        let override_snapshot = InputSnapshot {
            move_axis: (-1.0, 0.5),
            aim_axis: (9.0, 10.0),
            mining_tile: Some((6, 7)),
            building: true,
            config_tap_tile: Some((11, 12)),
            build_pulse: None,
            active_actions: vec![BinaryAction::Chat],
        };

        assert_eq!(
            mapper.map_snapshot_batch_or_override(&batch, Some(&override_snapshot)),
            vec![
                PlayerIntent::SetMoveAxis { x: -1.0, y: 0.5 },
                PlayerIntent::SetAimAxis { x: 9.0, y: 10.0 },
                PlayerIntent::SetMiningTile { tile: Some((6, 7)) },
                PlayerIntent::SetBuilding { building: true },
                PlayerIntent::ConfigTap { tile: (11, 12) },
                PlayerIntent::ActionPressed(BinaryAction::Chat),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_or_override_falls_back_to_batch_when_override_is_absent() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let batch = vec![
            snapshot((1.0, 0.0), (2.0, 2.0), &[BinaryAction::Fire]),
            snapshot((0.0, 0.0), (3.0, 4.0), &[]),
        ];

        assert_eq!(
            mapper.map_snapshot_batch_or_override(&batch, None),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );
    }

    #[test]
    fn map_snapshot_batch_or_override_keeps_mapper_history_in_sync_after_override() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);
        let batch = vec![snapshot((0.0, 0.0), (1.0, 1.0), &[BinaryAction::Fire])];
        let override_snapshot = snapshot((2.0, 3.0), (4.0, 5.0), &[BinaryAction::Chat]);

        assert_eq!(
            mapper.map_snapshot_batch_or_override(&batch, Some(&override_snapshot)),
            vec![
                PlayerIntent::SetMoveAxis { x: 2.0, y: 3.0 },
                PlayerIntent::SetAimAxis { x: 4.0, y: 5.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Chat),
            ]
        );

        assert_eq!(
            mapper.map_snapshot_batch_or_override(&batch, None),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 1.0 },
                PlayerIntent::SetMiningTile { tile: None },
                PlayerIntent::SetBuilding { building: false },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Chat),
            ]
        );
    }
}
