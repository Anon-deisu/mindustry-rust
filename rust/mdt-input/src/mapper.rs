use crate::intent::{BinaryAction, PlayerIntent};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputSnapshot {
    pub move_axis: (f32, f32),
    pub aim_axis: (f32, f32),
    pub active_actions: Vec<BinaryAction>,
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
            Vec::new()
        }
    }
}

impl IntentMapper for StatelessIntentMapper {
    fn map_snapshot(&mut self, snapshot: &InputSnapshot) -> Vec<PlayerIntent> {
        let active_actions = canonicalize_actions(&snapshot.active_actions);
        let mut intents =
            Vec::with_capacity(2 + active_actions.len() + self.active_actions_prev.len());

        intents.push(PlayerIntent::SetMoveAxis {
            x: snapshot.move_axis.0,
            y: snapshot.move_axis.1,
        });
        intents.push(PlayerIntent::SetAimAxis {
            x: snapshot.aim_axis.0,
            y: snapshot.aim_axis.1,
        });

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
        BinaryAction::Use => 5,
        BinaryAction::Pause => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(
        move_axis: (f32, f32),
        aim_axis: (f32, f32),
        active_actions: &[BinaryAction],
    ) -> InputSnapshot {
        InputSnapshot {
            move_axis,
            aim_axis,
            active_actions: active_actions.to_vec(),
        }
    }

    #[test]
    fn press_hold_release_edges_are_emitted() {
        let mut mapper = StatelessIntentMapper::default();

        assert_eq!(
            mapper.map_snapshot(&snapshot((1.0, 0.0), (0.0, 1.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 1.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 1.0 },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((1.0, 0.0), (0.0, 1.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 1.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 1.0 },
                PlayerIntent::ActionHeld(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (0.0, 1.0), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 1.0 },
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
                    BinaryAction::Use,
                    BinaryAction::Fire,
                ]
            )),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 2.0, y: 3.0 },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Use),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot(
                (0.0, 0.0),
                (2.0, 3.0),
                &[BinaryAction::Use, BinaryAction::Use, BinaryAction::Fire,]
            )),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 2.0, y: 3.0 },
                PlayerIntent::ActionHeld(BinaryAction::Fire),
                PlayerIntent::ActionHeld(BinaryAction::Use),
            ]
        );
    }

    #[test]
    fn action_edge_order_is_stable_across_input_permutations() {
        let mut mapper = StatelessIntentMapper::default();

        let first = mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (1.0, 1.0),
            &[BinaryAction::Pause, BinaryAction::Use, BinaryAction::Fire],
        ));
        assert_eq!(
            first,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 1.0 },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
                PlayerIntent::ActionPressed(BinaryAction::Use),
                PlayerIntent::ActionPressed(BinaryAction::Pause),
            ]
        );

        let second = mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (1.0, 1.0),
            &[BinaryAction::Use, BinaryAction::Fire, BinaryAction::Pause],
        ));
        assert_eq!(
            second,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 1.0 },
                PlayerIntent::ActionHeld(BinaryAction::Fire),
                PlayerIntent::ActionHeld(BinaryAction::Use),
                PlayerIntent::ActionHeld(BinaryAction::Pause),
            ]
        );
    }

    #[test]
    fn released_action_order_is_stable_when_multiple_actions_drop() {
        let mut mapper = StatelessIntentMapper::default();

        mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (0.0, 0.0),
            &[BinaryAction::Pause, BinaryAction::Use, BinaryAction::Fire],
        ));

        let released = mapper.map_snapshot(&snapshot((0.0, 0.0), (0.0, 0.0), &[]));
        assert_eq!(
            released,
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::ActionReleased(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Use),
                PlayerIntent::ActionReleased(BinaryAction::Pause),
            ]
        );
    }

    #[test]
    fn released_actions_emit_when_snapshot_becomes_empty() {
        let mut mapper = StatelessIntentMapper::default();

        mapper.map_snapshot(&snapshot(
            (0.0, 0.0),
            (0.0, 0.0),
            &[BinaryAction::Fire, BinaryAction::Use],
        ));

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (0.0, 0.0), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 0.0, y: 0.0 },
                PlayerIntent::ActionReleased(BinaryAction::Fire),
                PlayerIntent::ActionReleased(BinaryAction::Use),
            ]
        );
    }

    #[test]
    fn axis_intents_are_always_emitted_even_without_action_changes() {
        let mut mapper = StatelessIntentMapper::default();

        let first = mapper.map_snapshot(&snapshot((0.5, -0.5), (4.0, 8.0), &[BinaryAction::Pause]));
        let second =
            mapper.map_snapshot(&snapshot((-1.0, 1.0), (9.0, 10.0), &[BinaryAction::Pause]));

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
        assert_eq!(second[2], PlayerIntent::ActionHeld(BinaryAction::Pause));
    }

    #[test]
    fn live_sampling_mode_emits_only_press_and_release_edges() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (1.0, 2.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 2.0 },
                PlayerIntent::ActionPressed(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (1.0, 2.0), &[BinaryAction::Fire])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 1.0, y: 2.0 },
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (3.0, 4.0), &[BinaryAction::Use])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
                PlayerIntent::ActionPressed(BinaryAction::Use),
                PlayerIntent::ActionReleased(BinaryAction::Fire),
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (3.0, 4.0), &[BinaryAction::Use])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 3.0, y: 4.0 },
            ]
        );

        assert_eq!(
            mapper.map_snapshot(&snapshot((0.0, 0.0), (5.0, 6.0), &[])),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.0, y: 0.0 },
                PlayerIntent::SetAimAxis { x: 5.0, y: 6.0 },
                PlayerIntent::ActionReleased(BinaryAction::Use),
            ]
        );
    }

    #[test]
    fn map_latest_snapshot_uses_last_sample_in_batch() {
        let mut mapper = StatelessIntentMapper::new(IntentSamplingMode::LiveSampling);

        let batch = vec![
            snapshot((0.0, 0.0), (1.0, 1.0), &[BinaryAction::Fire]),
            snapshot((0.5, 0.5), (2.0, 2.0), &[BinaryAction::Use]),
        ];
        assert_eq!(
            mapper.map_latest_snapshot(&batch),
            vec![
                PlayerIntent::SetMoveAxis { x: 0.5, y: 0.5 },
                PlayerIntent::SetAimAxis { x: 2.0, y: 2.0 },
                PlayerIntent::ActionPressed(BinaryAction::Use),
            ]
        );

        assert!(mapper.map_latest_snapshot(&[]).is_empty());
    }
}
