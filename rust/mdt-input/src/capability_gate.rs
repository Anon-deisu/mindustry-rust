use crate::command_mode::{
    CommandModeCommandSelection, CommandModeProjection, CommandModeStanceSelection,
    CommandModeTargetProjection,
};
use crate::intent::PlayerIntent;
use crate::probe::RuntimeInputState;

/// Minimal local capability gate for runtime input actions.
///
/// This is intentionally transport-agnostic groundwork. It only answers whether a
/// local action is obviously allowed from current runtime/context state and, if not,
/// provides a structured deny reason.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapabilityGate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapabilityContext {
    pub runtime: RuntimeInputState,
    pub command_mode: CommandModeProjection,
    pub mining_enabled: bool,
    pub building_enabled: bool,
    pub command_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityBuildRequest {
    pub tile: (i32, i32),
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityCommandRequest {
    Target(CommandModeTargetProjection),
    SetCommand(CommandModeCommandSelection),
    SetStance(CommandModeStanceSelection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityDecision {
    pub allowed: bool,
    pub reason: Option<CapabilityDenyReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityDenyReason {
    MissingControlledUnit,
    ControlledUnitDead,
    MiningDisabled,
    BuildingDisabled,
    CommandDisabled,
    MissingBuildBlock,
    CommandModeInactive,
    MissingCommandTarget,
}

impl CapabilityContext {
    pub fn has_live_controlled_unit(&self) -> bool {
        self.runtime.unit_id.is_some() && !self.runtime.dead
    }
}

impl CapabilityDecision {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }

    pub fn denied(reason: CapabilityDenyReason) -> Self {
        Self {
            allowed: false,
            reason: Some(reason),
        }
    }
}

impl CapabilityGate {
    pub fn evaluate_intent(
        &self,
        context: &CapabilityContext,
        intent: &PlayerIntent,
    ) -> CapabilityDecision {
        match intent {
            PlayerIntent::SetMiningTile { tile: Some(tile) } => {
                self.evaluate_mining(context, *tile)
            }
            _ => CapabilityDecision::allowed(),
        }
    }

    pub fn evaluate_mining(
        &self,
        context: &CapabilityContext,
        _tile: (i32, i32),
    ) -> CapabilityDecision {
        if let Some(decision) = require_live_controlled_unit(context) {
            decision
        } else if !context.mining_enabled {
            CapabilityDecision::denied(CapabilityDenyReason::MiningDisabled)
        } else {
            CapabilityDecision::allowed()
        }
    }

    pub fn evaluate_build(
        &self,
        context: &CapabilityContext,
        request: &CapabilityBuildRequest,
    ) -> CapabilityDecision {
        if let Some(decision) = require_live_controlled_unit(context) {
            decision
        } else if !context.building_enabled {
            CapabilityDecision::denied(CapabilityDenyReason::BuildingDisabled)
        } else if !request.breaking && request.block_id.is_none() {
            CapabilityDecision::denied(CapabilityDenyReason::MissingBuildBlock)
        } else {
            CapabilityDecision::allowed()
        }
    }

    pub fn evaluate_command(
        &self,
        context: &CapabilityContext,
        request: &CapabilityCommandRequest,
    ) -> CapabilityDecision {
        if let Some(decision) = require_live_controlled_unit(context) {
            decision
        } else if !context.command_enabled {
            CapabilityDecision::denied(CapabilityDenyReason::CommandDisabled)
        } else if !context.command_mode.active {
            CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
        } else if matches!(request, CapabilityCommandRequest::Target(target) if target.is_empty()) {
            CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget)
        } else {
            CapabilityDecision::allowed()
        }
    }
}

fn require_live_controlled_unit(context: &CapabilityContext) -> Option<CapabilityDecision> {
    if context.runtime.unit_id.is_none() {
        Some(CapabilityDecision::denied(
            CapabilityDenyReason::MissingControlledUnit,
        ))
    } else if !context.has_live_controlled_unit() {
        Some(CapabilityDecision::denied(
            CapabilityDenyReason::ControlledUnitDead,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_mode::{CommandModeStanceSelection, CommandUnitRef};
    use crate::intent::PlayerIntent;

    fn context() -> CapabilityContext {
        CapabilityContext {
            runtime: RuntimeInputState {
                unit_id: Some(42),
                dead: false,
                position: Some((16.0, 24.0)),
                pointer: Some((20.0, 30.0)),
            },
            command_mode: CommandModeProjection::default(),
            mining_enabled: true,
            building_enabled: true,
            command_enabled: true,
        }
    }

    #[test]
    fn mining_intent_requires_live_controlled_unit_but_clear_is_allowed() {
        let gate = CapabilityGate;
        let missing_unit = CapabilityContext {
            runtime: RuntimeInputState {
                unit_id: None,
                dead: false,
                position: Some((0.0, 0.0)),
                pointer: None,
            },
            ..context()
        };

        assert_eq!(
            gate.evaluate_intent(
                &missing_unit,
                &PlayerIntent::SetMiningTile { tile: Some((7, 9)) }
            ),
            CapabilityDecision::denied(CapabilityDenyReason::MissingControlledUnit)
        );
        assert_eq!(
            gate.evaluate_intent(&missing_unit, &PlayerIntent::SetMiningTile { tile: None }),
            CapabilityDecision::allowed()
        );
    }

    #[test]
    fn mining_and_build_requests_reject_dead_units_before_other_checks() {
        let gate = CapabilityGate;
        let dead_context = CapabilityContext {
            runtime: RuntimeInputState {
                dead: true,
                ..context().runtime
            },
            ..context()
        };

        assert_eq!(
            gate.evaluate_mining(&dead_context, (3, 4)),
            CapabilityDecision::denied(CapabilityDenyReason::ControlledUnitDead)
        );
        assert_eq!(
            gate.evaluate_build(
                &dead_context,
                &CapabilityBuildRequest {
                    tile: (3, 4),
                    breaking: true,
                    block_id: None,
                    rotation: Some(0),
                }
            ),
            CapabilityDecision::denied(CapabilityDenyReason::ControlledUnitDead)
        );
    }

    #[test]
    fn build_requests_reject_disabled_building_and_missing_placement_block() {
        let gate = CapabilityGate;
        let disabled_building = CapabilityContext {
            building_enabled: false,
            ..context()
        };

        assert_eq!(
            gate.evaluate_build(
                &disabled_building,
                &CapabilityBuildRequest {
                    tile: (10, 11),
                    breaking: false,
                    block_id: Some(5),
                    rotation: Some(2),
                }
            ),
            CapabilityDecision::denied(CapabilityDenyReason::BuildingDisabled)
        );
        assert_eq!(
            gate.evaluate_build(
                &context(),
                &CapabilityBuildRequest {
                    tile: (10, 11),
                    breaking: false,
                    block_id: None,
                    rotation: Some(2),
                }
            ),
            CapabilityDecision::denied(CapabilityDenyReason::MissingBuildBlock)
        );
    }

    #[test]
    fn command_target_requests_require_active_command_mode_and_non_empty_target() {
        let gate = CapabilityGate;

        assert_eq!(
            gate.evaluate_command(
                &context(),
                &CapabilityCommandRequest::Target(CommandModeTargetProjection::default())
            ),
            CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
        );

        let active_context = CapabilityContext {
            command_mode: CommandModeProjection {
                active: true,
                ..CommandModeProjection::default()
            },
            ..context()
        };

        assert_eq!(
            gate.evaluate_command(
                &active_context,
                &CapabilityCommandRequest::Target(CommandModeTargetProjection::default())
            ),
            CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget)
        );
        assert_eq!(
            gate.evaluate_command(
                &active_context,
                &CapabilityCommandRequest::Target(CommandModeTargetProjection {
                    build_target: None,
                    unit_target: Some(CommandUnitRef { kind: 1, value: 99 }),
                    position_target: None,
                })
            ),
            CapabilityDecision::allowed()
        );
    }

    #[test]
    fn command_selection_requests_remain_allowed_with_explicit_none_values_once_active() {
        let gate = CapabilityGate;
        let active_context = CapabilityContext {
            command_mode: CommandModeProjection {
                active: true,
                ..CommandModeProjection::default()
            },
            ..context()
        };

        assert_eq!(
            gate.evaluate_command(
                &active_context,
                &CapabilityCommandRequest::SetCommand(CommandModeCommandSelection {
                    command_id: None,
                })
            ),
            CapabilityDecision::allowed()
        );
        assert_eq!(
            gate.evaluate_command(
                &active_context,
                &CapabilityCommandRequest::SetStance(CommandModeStanceSelection {
                    stance_id: None,
                    enabled: true,
                })
            ),
            CapabilityDecision::allowed()
        );
    }
}
