#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeInputState {
    pub unit_id: Option<i32>,
    pub dead: bool,
    pub position: Option<(f32, f32)>,
    pub pointer: Option<(f32, f32)>,
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
        let (x, y) = runtime.position?;
        if runtime.dead || runtime.unit_id.is_none() {
            return None;
        }

        let next = (x + self.config.step.0, y + self.config.step.1);
        let rotation_degrees = probe_heading_degrees(self.config.step);
        self.last_step_at_ms = Some(now_ms);

        Some(MovementProbeUpdate {
            position: next,
            view_center: next,
            pointer: locked_pointer.unwrap_or(next),
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

fn probe_heading_degrees(step: (f32, f32)) -> f32 {
    step.1.atan2(step.0).to_degrees()
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
}
