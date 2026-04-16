use crate::intent::BinaryAction;
use crate::mapper::InputSnapshot;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeInputState {
    pub unit_id: Option<i32>,
    pub dead: bool,
    pub position: Option<(f32, f32)>,
    pub pointer: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeInputSample {
    pub position: Option<(f32, f32)>,
    pub pointer: Option<(f32, f32)>,
    pub velocity: (f32, f32),
    pub mining_tile: Option<(i32, i32)>,
    pub building: bool,
    pub shooting: bool,
    pub boosting: bool,
    pub chatting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeInputSampleKind {
    Idle,
    MovementOnly,
    ActionOnly,
    Mixed,
}

impl RuntimeInputSampleKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::MovementOnly => "movement-only",
            Self::ActionOnly => "action-only",
            Self::Mixed => "mixed",
        }
    }
}

impl RuntimeInputSample {
    pub fn has_movement(self) -> bool {
        is_finite_vector(self.velocity) && self.velocity != (0.0, 0.0)
    }

    pub fn has_actions(self) -> bool {
        self.mining_tile.is_some()
            || self.building
            || self.shooting
            || self.boosting
            || self.chatting
    }

    pub fn kind(self) -> RuntimeInputSampleKind {
        match (self.has_movement(), self.has_actions()) {
            (false, false) => RuntimeInputSampleKind::Idle,
            (true, false) => RuntimeInputSampleKind::MovementOnly,
            (false, true) => RuntimeInputSampleKind::ActionOnly,
            (true, true) => RuntimeInputSampleKind::Mixed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementProbeConfig {
    pub step: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementProbeUpdate {
    pub position: (f32, f32),
    pub view_center: (f32, f32),
    pub pointer: (f32, f32),
    pub velocity: (f32, f32),
    pub rotation_degrees: f32,
    pub base_rotation_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MovementProbeController {
    config: MovementProbeConfig,
    last_step_at_ms: Option<u64>,
}

impl MovementProbeController {
    pub fn new(config: MovementProbeConfig) -> Self {
        Self {
            config,
            last_step_at_ms: None,
        }
    }

    pub fn config(&self) -> MovementProbeConfig {
        self.config
    }

    pub fn last_step_at_ms(&self) -> Option<u64> {
        self.last_step_at_ms
    }

    pub fn advance(
        &mut self,
        runtime: RuntimeInputState,
        now_ms: u64,
        min_step_interval_ms: u64,
        locked_pointer: Option<(f32, f32)>,
    ) -> Option<MovementProbeUpdate> {
        if !self.should_step(now_ms, min_step_interval_ms) {
            return None;
        }
        if self.config.step == (0.0, 0.0) {
            return None;
        }
        let (x, y) = runtime.position?;
        if !is_finite_vector((x, y)) || !is_finite_vector(self.config.step) {
            return None;
        }
        if runtime.dead || runtime.unit_id.is_none() {
            return None;
        }

        let next = (x + self.config.step.0, y + self.config.step.1);
        if !is_finite_vector(next) {
            return None;
        }
        let rotation_degrees = probe_heading_degrees(self.config.step);
        let pointer = resolve_probe_pointer(locked_pointer, runtime.pointer, next);
        self.last_step_at_ms = Some(now_ms);

        Some(MovementProbeUpdate {
            position: next,
            view_center: next,
            pointer,
            velocity: self.config.step,
            rotation_degrees,
            base_rotation_degrees: rotation_degrees,
        })
    }

    fn should_step(&self, now_ms: u64, min_step_interval_ms: u64) -> bool {
        match self.last_step_at_ms {
            Some(last) => now_ms.saturating_sub(last) >= min_step_interval_ms,
            None => true,
        }
    }
}

pub fn sample_runtime_input_snapshot(sample: RuntimeInputSample) -> InputSnapshot {
    let mut active_actions = Vec::with_capacity(3);
    if sample.shooting {
        active_actions.push(BinaryAction::Fire);
    }
    if sample.boosting {
        active_actions.push(BinaryAction::Boost);
    }
    if sample.chatting {
        active_actions.push(BinaryAction::Chat);
    }

    InputSnapshot {
        move_axis: normalize_vector(sample.velocity),
        aim_axis: resolve_aim_axis(sample.pointer, sample.position),
        mining_tile: sample.mining_tile,
        building: sample.building,
        config_tap_tile: None,
        build_pulse: None,
        active_actions,
    }
}

pub fn classify_runtime_input_sample(sample: RuntimeInputSample) -> RuntimeInputSampleKind {
    sample.kind()
}

fn probe_heading_degrees(step: (f32, f32)) -> f32 {
    step.1.atan2(step.0).to_degrees()
}

fn resolve_aim_axis(
    pointer: Option<(f32, f32)>,
    position: Option<(f32, f32)>,
) -> (f32, f32) {
    pointer
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .or_else(|| position.filter(|(x, y)| x.is_finite() && y.is_finite()))
        .unwrap_or((0.0, 0.0))
}

fn resolve_probe_pointer(
    locked_pointer: Option<(f32, f32)>,
    runtime_pointer: Option<(f32, f32)>,
    fallback: (f32, f32),
) -> (f32, f32) {
    locked_pointer
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .or_else(|| runtime_pointer.filter(|(x, y)| x.is_finite() && y.is_finite()))
        .unwrap_or(fallback)
}

fn is_finite_vector(value: (f32, f32)) -> bool {
    value.0.is_finite() && value.1.is_finite()
}

fn normalize_vector(value: (f32, f32)) -> (f32, f32) {
    if is_finite_vector(value) {
        value
    } else {
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_returns_none_without_live_positioned_unit() {
        let mut controller = MovementProbeController::new(MovementProbeConfig { step: (1.0, 0.0) });

        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: None,
                    dead: false,
                    position: Some((1.0, 2.0)),
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );
        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: true,
                    position: Some((1.0, 2.0)),
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );
        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: None,
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );
    }

    #[test]
    fn advance_steps_once_per_interval_and_tracks_time() {
        let mut controller = MovementProbeController::new(MovementProbeConfig { step: (2.0, 3.0) });
        let runtime = RuntimeInputState {
            unit_id: Some(77),
            dead: false,
            position: Some((10.0, 20.0)),
            pointer: None,
        };

        let first = controller.advance(runtime, 1_000, 500, None).unwrap();
        assert_eq!(first.position, (12.0, 23.0));
        assert_eq!(first.view_center, (12.0, 23.0));
        assert_eq!(first.pointer, (12.0, 23.0));
        assert_eq!(first.velocity, (2.0, 3.0));
        assert_eq!(first.rotation_degrees, probe_heading_degrees((2.0, 3.0)));
        assert_eq!(
            first.base_rotation_degrees,
            probe_heading_degrees((2.0, 3.0))
        );
        assert_eq!(controller.last_step_at_ms(), Some(1_000));

        assert_eq!(controller.advance(runtime, 1_200, 500, None), None);

        let second = controller.advance(runtime, 1_500, 500, None).unwrap();
        assert_eq!(second.position, (12.0, 23.0));
        assert_eq!(controller.last_step_at_ms(), Some(1_500));
    }

    #[test]
    fn advance_keeps_locked_pointer_when_present() {
        let mut controller =
            MovementProbeController::new(MovementProbeConfig { step: (1.0, -2.0) });
        let update = controller
            .advance(
                RuntimeInputState {
                    unit_id: Some(99),
                    dead: false,
                    position: Some((5.0, 6.0)),
                    pointer: Some((100.0, 200.0)),
                },
                250,
                100,
                Some((100.0, 200.0)),
            )
            .unwrap();

        assert_eq!(update.position, (6.0, 4.0));
        assert_eq!(update.pointer, (100.0, 200.0));
    }

    #[test]
    fn advance_preserves_runtime_pointer_when_no_locked_pointer_is_configured() {
        let mut controller =
            MovementProbeController::new(MovementProbeConfig { step: (1.0, -2.0) });
        let update = controller
            .advance(
                RuntimeInputState {
                    unit_id: Some(99),
                    dead: false,
                    position: Some((5.0, 6.0)),
                    pointer: Some((40.0, 50.0)),
                },
                250,
                100,
                None,
            )
            .unwrap();

        assert_eq!(update.position, (6.0, 4.0));
        assert_eq!(update.view_center, (6.0, 4.0));
        assert_eq!(update.pointer, (40.0, 50.0));
    }

    #[test]
    fn advance_falls_back_to_next_position_when_runtime_pointer_is_non_finite() {
        let mut controller = MovementProbeController::new(MovementProbeConfig { step: (2.0, 1.0) });
        let update = controller
            .advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: Some((10.0, 20.0)),
                    pointer: Some((f32::NAN, 99.0)),
                },
                100,
                50,
                None,
            )
            .unwrap();

        assert_eq!(update.position, (12.0, 21.0));
        assert_eq!(update.pointer, (12.0, 21.0));
    }

    #[test]
    fn advance_falls_back_to_next_position_when_locked_and_runtime_pointers_are_non_finite() {
        let mut controller = MovementProbeController::new(MovementProbeConfig { step: (2.0, 1.0) });
        let update = controller
            .advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: Some((10.0, 20.0)),
                    pointer: Some((f32::NAN, 99.0)),
                },
                100,
                50,
                Some((f32::INFINITY, 200.0)),
            )
            .unwrap();

        assert_eq!(update.position, (12.0, 21.0));
        assert_eq!(update.view_center, (12.0, 21.0));
        assert_eq!(update.pointer, (12.0, 21.0));
    }

    #[test]
    fn advance_ignores_non_finite_position_or_step() {
        let mut controller = MovementProbeController::new(MovementProbeConfig {
            step: (f32::NAN, 1.0),
        });
        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: Some((10.0, 20.0)),
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );

        let mut controller = MovementProbeController::new(MovementProbeConfig { step: (0.0, 0.0) });
        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: Some((10.0, 20.0)),
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );

        let mut controller = MovementProbeController::new(MovementProbeConfig { step: (1.0, 2.0) });
        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: Some((f32::INFINITY, 20.0)),
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );
    }

    #[test]
    fn advance_rejects_overflowing_step_results() {
        let mut controller =
            MovementProbeController::new(MovementProbeConfig { step: (f32::MAX, 0.0) });

        assert_eq!(
            controller.advance(
                RuntimeInputState {
                    unit_id: Some(7),
                    dead: false,
                    position: Some((f32::MAX, 1.0)),
                    pointer: None,
                },
                100,
                50,
                None,
            ),
            None
        );
        assert_eq!(controller.last_step_at_ms(), None);
    }

    #[test]
    fn movement_probe_controller_config_roundtrips_initial_step() {
        let controller = MovementProbeController::new(MovementProbeConfig {
            step: (2.5, -3.75),
        });

        assert_eq!(
            controller.config(),
            MovementProbeConfig {
                step: (2.5, -3.75),
            }
        );
        assert_eq!(controller.last_step_at_ms(), None);
    }

    #[test]
    fn sample_runtime_input_snapshot_emits_fire_boost_chat_in_stable_order() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((1.0, 2.0)),
            pointer: Some((3.0, 4.0)),
            velocity: (0.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: true,
            boosting: true,
            chatting: true,
        });

        assert_eq!(
            snapshot.active_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost, BinaryAction::Chat]
        );
    }

    #[test]
    fn has_movement_distinguishes_zero_and_non_finite_axes() {
        assert!(!RuntimeInputSample {
            position: None,
            pointer: None,
            velocity: (0.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        }
        .has_movement());
        assert!(RuntimeInputSample {
            position: None,
            pointer: None,
            velocity: (1.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        }
        .has_movement());
        assert!(!RuntimeInputSample {
            position: None,
            pointer: None,
            velocity: (f32::NAN, 1.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        }
        .has_movement());
        assert!(!RuntimeInputSample {
            position: None,
            pointer: None,
            velocity: (1.0, f32::INFINITY),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        }
        .has_movement());
    }

    #[test]
    fn runtime_input_sample_kind_label_formats_all_variants() {
        assert_eq!(RuntimeInputSampleKind::Idle.label(), "idle");
        assert_eq!(
            RuntimeInputSampleKind::MovementOnly.label(),
            "movement-only"
        );
        assert_eq!(RuntimeInputSampleKind::ActionOnly.label(), "action-only");
        assert_eq!(RuntimeInputSampleKind::Mixed.label(), "mixed");
    }

    #[test]
    fn sample_runtime_input_snapshot_prefers_pointer_and_maps_actions() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((5.0, 6.0)),
            pointer: Some((16.0, 24.0)),
            velocity: (1.5, -2.5),
            mining_tile: Some((9, 11)),
            building: true,
            shooting: true,
            boosting: true,
            chatting: false,
        });

        assert_eq!(snapshot.move_axis, (1.5, -2.5));
        assert_eq!(snapshot.aim_axis, (16.0, 24.0));
        assert_eq!(snapshot.mining_tile, Some((9, 11)));
        assert!(snapshot.building);
        assert_eq!(
            snapshot.active_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
    }

    #[test]
    fn sample_runtime_input_snapshot_falls_back_to_position_for_aim_axis() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((5.0, 6.0)),
            pointer: None,
            velocity: (0.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: true,
        });

        assert_eq!(snapshot.aim_axis, (5.0, 6.0));
        assert_eq!(snapshot.active_actions, vec![BinaryAction::Chat]);
        assert_eq!(snapshot.config_tap_tile, None);
    }

    #[test]
    fn sample_runtime_input_snapshot_defaults_aim_axis_when_runtime_has_no_position_or_pointer() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: None,
            pointer: None,
            velocity: (0.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        });

        assert_eq!(snapshot.aim_axis, (0.0, 0.0));
        assert!(snapshot.active_actions.is_empty());
    }

    #[test]
    fn sample_runtime_input_snapshot_preserves_mining_tile_and_building_flags() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((1.0, 2.0)),
            pointer: None,
            velocity: (0.0, 0.0),
            mining_tile: Some((9, 11)),
            building: true,
            shooting: false,
            boosting: false,
            chatting: false,
        });

        assert_eq!(snapshot.mining_tile, Some((9, 11)));
        assert!(snapshot.building);
        assert_eq!(snapshot.config_tap_tile, None);
        assert_eq!(snapshot.build_pulse, None);
    }

    #[test]
    fn sample_runtime_input_snapshot_uses_position_when_pointer_is_non_finite_and_actions_are_empty() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((5.0, 6.0)),
            pointer: Some((f32::NAN, 24.0)),
            velocity: (0.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        });

        assert_eq!(snapshot.aim_axis, (5.0, 6.0));
        assert!(snapshot.active_actions.is_empty());
    }

    #[test]
    fn sample_runtime_input_snapshot_keeps_pointer_priority_and_empty_actions_stable() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((5.0, 6.0)),
            pointer: Some((16.0, 24.0)),
            velocity: (0.0, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        });

        assert_eq!(snapshot.move_axis, (0.0, 0.0));
        assert_eq!(snapshot.aim_axis, (16.0, 24.0));
        assert!(snapshot.active_actions.is_empty());
    }

    #[test]
    fn sample_runtime_input_snapshot_ignores_non_finite_pointer_and_position() {
        let snapshot = sample_runtime_input_snapshot(RuntimeInputSample {
            position: Some((f32::INFINITY, 6.0)),
            pointer: Some((f32::NAN, 24.0)),
            velocity: (f32::NAN, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        });

        assert_eq!(snapshot.aim_axis, (0.0, 0.0));
        assert_eq!(snapshot.move_axis, (0.0, 0.0));
        assert!(!(RuntimeInputSample {
            position: None,
            pointer: None,
            velocity: (f32::NAN, 0.0),
            mining_tile: None,
            building: false,
            shooting: false,
            boosting: false,
            chatting: false,
        }
        .has_movement()));
    }

    #[test]
    fn resolve_aim_axis_prefers_finite_pointer_then_position_then_zero_vector() {
        assert_eq!(
            resolve_aim_axis(Some((12.0, 34.0)), Some((56.0, 78.0))),
            (12.0, 34.0)
        );
        assert_eq!(
            resolve_aim_axis(Some((f32::INFINITY, 34.0)), Some((56.0, 78.0))),
            (56.0, 78.0)
        );
        assert_eq!(
            resolve_aim_axis(Some((f32::NAN, 34.0)), Some((f32::INFINITY, 78.0))),
            (0.0, 0.0)
        );
    }

    #[test]
    fn normalize_vector_rejects_non_finite_and_keeps_finite_axes() {
        assert_eq!(normalize_vector((12.0, 34.0)), (12.0, 34.0));
        assert_eq!(normalize_vector((f32::INFINITY, 34.0)), (0.0, 0.0));
        assert_eq!(normalize_vector((12.0, f32::NAN)), (0.0, 0.0));
    }

    #[test]
    fn resolve_probe_pointer_prefers_locked_then_runtime_then_fallback() {
        assert_eq!(
            resolve_probe_pointer(Some((12.0, 34.0)), Some((56.0, 78.0)), (90.0, 91.0)),
            (12.0, 34.0)
        );
        assert_eq!(
            resolve_probe_pointer(Some((f32::INFINITY, 34.0)), Some((56.0, 78.0)), (90.0, 91.0)),
            (56.0, 78.0)
        );
        assert_eq!(
            resolve_probe_pointer(Some((f32::NAN, 34.0)), Some((f32::INFINITY, 78.0)), (90.0, 91.0)),
            (90.0, 91.0)
        );
    }

    #[test]
    fn probe_heading_degrees_matches_basic_direction_angles() {
        assert!((probe_heading_degrees((1.0, 0.0)) - 0.0).abs() < 0.000_001);
        assert!((probe_heading_degrees((0.0, 1.0)) - 90.0).abs() < 0.000_001);
        assert!((probe_heading_degrees((-1.0, 0.0)) - 180.0).abs() < 0.000_001);
    }

    #[test]
    fn classify_runtime_input_sample_ignores_non_finite_velocity_for_movement() {
        assert_eq!(
            classify_runtime_input_sample(RuntimeInputSample {
                position: Some((1.0, 2.0)),
                pointer: None,
                velocity: (f32::INFINITY, 0.0),
                mining_tile: None,
                building: false,
                shooting: false,
                boosting: false,
                chatting: false,
            }),
            RuntimeInputSampleKind::Idle
        );
    }

    #[test]
    fn classify_runtime_input_sample_tracks_idle_movement_action_and_mixed_states() {
        assert_eq!(
            classify_runtime_input_sample(RuntimeInputSample {
                position: None,
                pointer: None,
                velocity: (0.0, 0.0),
                mining_tile: None,
                building: false,
                shooting: false,
                boosting: false,
                chatting: false,
            }),
            RuntimeInputSampleKind::Idle
        );
        assert_eq!(
            classify_runtime_input_sample(RuntimeInputSample {
                position: Some((1.0, 2.0)),
                pointer: None,
                velocity: (1.0, 0.0),
                mining_tile: None,
                building: false,
                shooting: false,
                boosting: false,
                chatting: false,
            }),
            RuntimeInputSampleKind::MovementOnly
        );
        assert_eq!(
            classify_runtime_input_sample(RuntimeInputSample {
                position: None,
                pointer: None,
                velocity: (0.0, 0.0),
                mining_tile: Some((3, 4)),
                building: true,
                shooting: false,
                boosting: true,
                chatting: false,
            }),
            RuntimeInputSampleKind::ActionOnly
        );
        assert_eq!(
            classify_runtime_input_sample(RuntimeInputSample {
                position: Some((5.0, 6.0)),
                pointer: Some((7.0, 8.0)),
                velocity: (0.5, -0.25),
                mining_tile: Some((9, 10)),
                building: false,
                shooting: true,
                boosting: false,
                chatting: true,
            }),
            RuntimeInputSampleKind::Mixed
        );
        assert_eq!(RuntimeInputSampleKind::Idle.label(), "idle");
        assert_eq!(RuntimeInputSampleKind::MovementOnly.label(), "movement-only");
        assert_eq!(RuntimeInputSampleKind::ActionOnly.label(), "action-only");
        assert_eq!(RuntimeInputSampleKind::Mixed.label(), "mixed");
    }
}
