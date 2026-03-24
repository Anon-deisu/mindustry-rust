/// Minimal local projection of command-mode state.
///
/// This is intentionally a bounded local abstraction for Rust-side input/runtime code.
/// It tracks recent selections and an explicit activation bit without claiming full
/// Java `InputHandler` business semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandModeProjection {
    pub active: bool,
    pub last_target: Option<CommandModeTargetProjection>,
    pub last_command_selection: Option<CommandModeCommandSelection>,
    pub last_stance_selection: Option<CommandModeStanceSelection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandModeState {
    projection: CommandModeProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandUnitRef {
    pub kind: u8,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandModePositionTarget {
    pub x_bits: u32,
    pub y_bits: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandModeTargetProjection {
    pub build_target: Option<i32>,
    pub unit_target: Option<CommandUnitRef>,
    pub position_target: Option<CommandModePositionTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandModeCommandSelection {
    pub command_id: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandModeStanceSelection {
    pub stance_id: Option<u8>,
    pub enabled: bool,
}

impl CommandModeState {
    pub fn projection(&self) -> CommandModeProjection {
        self.projection
    }

    pub fn is_active(&self) -> bool {
        self.projection.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.projection.active = active;
    }

    pub fn record_unit_clear(&mut self) {
        self.projection.active = false;
    }

    pub fn record_unit_control(&mut self, target: Option<CommandUnitRef>) {
        self.record_target(CommandModeTargetProjection {
            build_target: None,
            unit_target: target,
            position_target: None,
        });
    }

    pub fn record_building_control_select(&mut self, build_target: Option<i32>) {
        self.record_target(CommandModeTargetProjection {
            build_target,
            unit_target: None,
            position_target: None,
        });
    }

    pub fn record_unit_building_control_select(
        &mut self,
        unit_target: Option<CommandUnitRef>,
        build_target: Option<i32>,
    ) {
        self.record_target(CommandModeTargetProjection {
            build_target,
            unit_target,
            position_target: None,
        });
    }

    pub fn record_command_building(&mut self, position_target: (f32, f32)) {
        self.record_target(CommandModeTargetProjection {
            build_target: None,
            unit_target: None,
            position_target: Some(CommandModePositionTarget::from_world(position_target)),
        });
    }

    pub fn record_command_units(
        &mut self,
        build_target: Option<i32>,
        unit_target: Option<CommandUnitRef>,
        position_target: Option<(f32, f32)>,
    ) {
        self.record_target(CommandModeTargetProjection {
            build_target,
            unit_target,
            position_target: position_target.map(CommandModePositionTarget::from_world),
        });
    }

    pub fn record_set_unit_command(&mut self, command_id: Option<u8>) {
        self.projection.last_command_selection = Some(CommandModeCommandSelection { command_id });
    }

    pub fn record_set_unit_stance(&mut self, stance_id: Option<u8>, enabled: bool) {
        self.projection.last_stance_selection =
            Some(CommandModeStanceSelection { stance_id, enabled });
    }

    pub fn clear_recent_selections(&mut self) {
        self.projection.last_target = None;
        self.projection.last_command_selection = None;
        self.projection.last_stance_selection = None;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn record_target(&mut self, target: CommandModeTargetProjection) {
        if !target.is_empty() {
            self.projection.last_target = Some(target);
        }
    }
}

impl CommandModeTargetProjection {
    pub fn is_empty(&self) -> bool {
        self.build_target.is_none() && self.unit_target.is_none() && self.position_target.is_none()
    }
}

impl CommandModePositionTarget {
    pub fn from_world(position: (f32, f32)) -> Self {
        Self {
            x_bits: position.0.to_bits(),
            y_bits: position.1.to_bits(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(kind: u8, value: i32) -> CommandUnitRef {
        CommandUnitRef { kind, value }
    }

    #[test]
    fn default_projection_starts_inactive_without_recent_selections() {
        let state = CommandModeState::default();

        assert_eq!(
            state.projection(),
            CommandModeProjection {
                active: false,
                last_target: None,
                last_command_selection: None,
                last_stance_selection: None,
            }
        );
        assert!(!state.is_active());
    }

    #[test]
    fn explicit_activation_is_independent_from_recent_target_tracking() {
        let mut state = CommandModeState::default();

        state.record_unit_control(Some(unit(2, 222)));
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: Some(unit(2, 222)),
                position_target: None,
            })
        );
        assert!(!state.is_active());

        state.set_active(true);
        assert!(state.is_active());

        state.record_unit_clear();
        assert!(!state.is_active());
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: Some(unit(2, 222)),
                position_target: None,
            })
        );
    }

    #[test]
    fn unit_building_and_command_targets_are_projected_without_java_only_semantics() {
        let mut state = CommandModeState::default();

        state.record_unit_building_control_select(Some(unit(1, 700)), Some(900));
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(900),
                unit_target: Some(unit(1, 700)),
                position_target: None,
            })
        );

        state.record_command_building((12.5, -4.0));
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: None,
                unit_target: None,
                position_target: Some(CommandModePositionTarget {
                    x_bits: 12.5f32.to_bits(),
                    y_bits: (-4.0f32).to_bits(),
                }),
            })
        );

        state.record_command_units(Some(901), Some(unit(2, 333)), Some((1.5, 2.5)));
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(901),
                unit_target: Some(unit(2, 333)),
                position_target: Some(CommandModePositionTarget {
                    x_bits: 1.5f32.to_bits(),
                    y_bits: 2.5f32.to_bits(),
                }),
            })
        );
    }

    #[test]
    fn command_and_stance_selections_preserve_explicit_none_values() {
        let mut state = CommandModeState::default();

        state.record_set_unit_command(None);
        state.record_set_unit_stance(Some(7), false);
        assert_eq!(
            state.projection().last_command_selection,
            Some(CommandModeCommandSelection { command_id: None })
        );
        assert_eq!(
            state.projection().last_stance_selection,
            Some(CommandModeStanceSelection {
                stance_id: Some(7),
                enabled: false,
            })
        );

        state.record_set_unit_command(Some(12));
        state.record_set_unit_stance(None, true);
        assert_eq!(
            state.projection().last_command_selection,
            Some(CommandModeCommandSelection {
                command_id: Some(12),
            })
        );
        assert_eq!(
            state.projection().last_stance_selection,
            Some(CommandModeStanceSelection {
                stance_id: None,
                enabled: true,
            })
        );
    }

    #[test]
    fn empty_target_updates_do_not_clobber_recent_target_and_clear_helpers_reset_state() {
        let mut state = CommandModeState::default();
        state.set_active(true);
        state.record_command_units(Some(11), Some(unit(2, 44)), Some((3.0, 4.0)));
        state.record_command_units(None, None, None);
        assert_eq!(
            state.projection().last_target,
            Some(CommandModeTargetProjection {
                build_target: Some(11),
                unit_target: Some(unit(2, 44)),
                position_target: Some(CommandModePositionTarget {
                    x_bits: 3.0f32.to_bits(),
                    y_bits: 4.0f32.to_bits(),
                }),
            })
        );

        state.clear_recent_selections();
        assert_eq!(
            state.projection(),
            CommandModeProjection {
                active: true,
                last_target: None,
                last_command_selection: None,
                last_stance_selection: None,
            }
        );

        state.clear();
        assert_eq!(state.projection(), CommandModeProjection::default());
    }
}
