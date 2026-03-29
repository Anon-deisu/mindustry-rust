use crate::command_mode::{
    CommandModeCommandSelection, CommandModeProjection, CommandModeStanceSelection,
    CommandModeProjectionSummary, CommandModeTargetProjection,
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

#[derive(Debug, Clone, PartialEq)]
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
pub enum CapabilityUnitState {
    MissingControlledUnit,
    ControlledUnitDead,
    ControlledUnitLive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityContextProjection {
    pub unit_state: CapabilityUnitState,
    pub mining_enabled: bool,
    pub building_enabled: bool,
    pub command_enabled: bool,
    pub command_mode: CommandModeProjectionSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityEvaluationProjection {
    pub context: CapabilityContextProjection,
    pub decision: CapabilityDecision,
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

    pub fn projection(&self) -> CapabilityContextProjection {
        CapabilityContextProjection {
            unit_state: if self.runtime.unit_id.is_none() {
                CapabilityUnitState::MissingControlledUnit
            } else if self.runtime.dead {
                CapabilityUnitState::ControlledUnitDead
            } else {
                CapabilityUnitState::ControlledUnitLive
            },
            mining_enabled: self.mining_enabled,
            building_enabled: self.building_enabled,
            command_enabled: self.command_enabled,
            command_mode: self.command_mode.summary(),
        }
    }

    pub fn summary(&self) -> CapabilityContextProjection {
        self.projection()
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

    pub fn label(self) -> &'static str {
        if self.allowed {
            "allowed"
        } else {
            self.reason_label()
        }
    }

    pub fn reason_label(self) -> &'static str {
        self.reason.map_or("allowed", CapabilityDenyReason::label)
    }
}

impl CapabilityUnitState {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingControlledUnit => "missing-controlled-unit",
            Self::ControlledUnitDead => "controlled-unit-dead",
            Self::ControlledUnitLive => "controlled-unit-live",
        }
    }
}

impl CapabilityContextProjection {
    pub fn has_live_controlled_unit(self) -> bool {
        matches!(self.unit_state, CapabilityUnitState::ControlledUnitLive)
    }

    pub fn summary_label(self) -> String {
        format!(
            "unit={} mining={} building={} command={} mode={}",
            self.unit_state.label(),
            on_off(self.mining_enabled),
            on_off(self.building_enabled),
            on_off(self.command_enabled),
            self.command_mode.summary_label(),
        )
    }
}

impl CapabilityEvaluationProjection {
    pub fn allowed(self) -> bool {
        self.decision.allowed
    }

    pub fn decision_label(self) -> &'static str {
        self.decision.label()
    }

    pub fn deny_reason_label(self) -> &'static str {
        self.decision.reason_label()
    }

    pub fn summary_label(self) -> String {
        format!(
            "{} decision={}",
            self.context.summary_label(),
            self.decision_label()
        )
    }
}

impl CapabilityDenyReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingControlledUnit => "missing-controlled-unit",
            Self::ControlledUnitDead => "controlled-unit-dead",
            Self::MiningDisabled => "mining-disabled",
            Self::BuildingDisabled => "building-disabled",
            Self::CommandDisabled => "command-disabled",
            Self::MissingBuildBlock => "missing-build-block",
            Self::CommandModeInactive => "command-mode-inactive",
            Self::MissingCommandTarget => "missing-command-target",
        }
    }
}

impl CapabilityGate {
    pub fn summarize(
        &self,
        context: &CapabilityContext,
        decision: CapabilityDecision,
    ) -> CapabilityEvaluationProjection {
        CapabilityEvaluationProjection {
            context: context.projection(),
            decision,
        }
    }

    pub fn evaluate_intent(
        &self,
        context: &CapabilityContext,
        intent: &PlayerIntent,
    ) -> CapabilityDecision {
        match intent {
            PlayerIntent::SetMiningTile { tile: Some(tile) } => {
                self.evaluate_mining(context, *tile)
            }
            PlayerIntent::SetBuilding { building: true } | PlayerIntent::ConfigTap { .. } => {
                self.evaluate_build_intent(context)
            }
            PlayerIntent::BuildPulse(pulse) => self.evaluate_build(
                context,
                &CapabilityBuildRequest {
                    tile: pulse.tile,
                    breaking: pulse.breaking,
                    block_id: None,
                    rotation: None,
                },
            ),
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
        if let Some(decision) = self.evaluate_build_intent_base(context) {
            decision
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

    fn evaluate_build_intent(&self, context: &CapabilityContext) -> CapabilityDecision {
        self.evaluate_build_intent_base(context)
            .unwrap_or_else(CapabilityDecision::allowed)
    }

    fn evaluate_build_intent_base(
        &self,
        context: &CapabilityContext,
    ) -> Option<CapabilityDecision> {
        if let Some(decision) = require_live_controlled_unit(context) {
            Some(decision)
        } else if !context.building_enabled {
            Some(CapabilityDecision::denied(
                CapabilityDenyReason::BuildingDisabled,
            ))
        } else {
            None
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

fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_mode::{CommandModeStanceSelection, CommandUnitRef};
    use crate::intent::{BuildPulse, PlayerIntent};

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
    fn capability_projection_and_summary_track_context_and_decision_labels() {
        let gate = CapabilityGate;
        let active_context = CapabilityContext {
            command_mode: CommandModeProjection {
                active: true,
                selected_units: vec![1, 2],
                command_buildings: vec![3],
                last_target: Some(CommandModeTargetProjection {
                    build_target: Some(9),
                    unit_target: Some(CommandUnitRef { kind: 1, value: 7 }),
                    position_target: None,
                    rect_target: None,
                }),
                last_command_selection: Some(CommandModeCommandSelection {
                    command_id: Some(4),
                }),
                last_stance_selection: Some(CommandModeStanceSelection {
                    stance_id: Some(2),
                    enabled: true,
                }),
                ..CommandModeProjection::default()
            },
            ..context()
        };

        let projection = active_context.projection();
        let evaluation = gate.summarize(
            &active_context,
            CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget),
        );

        assert!(projection.has_live_controlled_unit());
        assert_eq!(projection.unit_state.label(), "controlled-unit-live");
        assert_eq!(
            projection.summary_label(),
            "unit=controlled-unit-live mining=on building=on command=on mode=target+command+stance"
        );
        assert_eq!(projection.command_mode.summary_label(), "target+command+stance");
        assert_eq!(projection.command_mode.recent_selection_label(), "target+command+stance");
        assert_eq!(CapabilityDecision::allowed().label(), "allowed");
        assert_eq!(
            CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget).label(),
            "missing-command-target"
        );
        assert_eq!(CapabilityDenyReason::CommandDisabled.label(), "command-disabled");
        assert_eq!(evaluation.decision_label(), "missing-command-target");
        assert_eq!(evaluation.deny_reason_label(), "missing-command-target");
        assert_eq!(
            evaluation.summary_label(),
            "unit=controlled-unit-live mining=on building=on command=on mode=target+command+stance decision=missing-command-target"
        );
        assert!(!evaluation.allowed());
    }

    #[test]
    fn capability_projection_reports_missing_and_dead_control_states() {
        let missing = CapabilityContext {
            runtime: RuntimeInputState {
                unit_id: None,
                dead: false,
                position: Some((0.0, 0.0)),
                pointer: None,
            },
            ..context()
        };
        let dead = CapabilityContext {
            runtime: RuntimeInputState {
                dead: true,
                ..context().runtime
            },
            ..context()
        };

        assert_eq!(
            missing.projection().unit_state,
            CapabilityUnitState::MissingControlledUnit
        );
        assert_eq!(
            missing.projection().summary_label(),
            "unit=missing-controlled-unit mining=on building=on command=on mode=idle"
        );
        assert_eq!(
            dead.projection().unit_state,
            CapabilityUnitState::ControlledUnitDead
        );
        assert_eq!(
            dead.projection().summary_label(),
            "unit=controlled-unit-dead mining=on building=on command=on mode=idle"
        );
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
    fn building_intents_require_building_capability_but_allow_clear_toggle() {
        let gate = CapabilityGate;
        let disabled_building = CapabilityContext {
            building_enabled: false,
            ..context()
        };
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
                &disabled_building,
                &PlayerIntent::SetBuilding { building: true }
            ),
            CapabilityDecision::denied(CapabilityDenyReason::BuildingDisabled)
        );
        assert_eq!(
            gate.evaluate_intent(&disabled_building, &PlayerIntent::ConfigTap { tile: (7, 9) }),
            CapabilityDecision::denied(CapabilityDenyReason::BuildingDisabled)
        );
        assert_eq!(
            gate.evaluate_intent(
                &disabled_building,
                &PlayerIntent::BuildPulse(BuildPulse {
                    tile: (7, 9),
                    breaking: false,
                })
            ),
            CapabilityDecision::denied(CapabilityDenyReason::BuildingDisabled)
        );
        assert_eq!(
            gate.evaluate_intent(&disabled_building, &PlayerIntent::SetBuilding { building: false }),
            CapabilityDecision::allowed()
        );
        assert_eq!(
            gate.evaluate_intent(
                &missing_unit,
                &PlayerIntent::SetBuilding { building: true }
            ),
            CapabilityDecision::denied(CapabilityDenyReason::MissingControlledUnit)
        );
    }

    #[test]
    fn build_pulse_without_block_is_denied() {
        let gate = CapabilityGate;

        assert_eq!(
            gate.evaluate_intent(
                &context(),
                &PlayerIntent::BuildPulse(BuildPulse {
                    tile: (7, 9),
                    breaking: false,
                })
            ),
            CapabilityDecision::denied(CapabilityDenyReason::MissingBuildBlock)
        );
        assert_eq!(
            gate.evaluate_intent(
                &context(),
                &PlayerIntent::BuildPulse(BuildPulse {
                    tile: (7, 9),
                    breaking: true,
                })
            ),
            CapabilityDecision::allowed()
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
                    rect_target: None,
                })
            ),
            CapabilityDecision::allowed()
        );
    }

    #[test]
    fn command_selection_requests_require_active_mode_and_allow_explicit_none_values() {
        let gate = CapabilityGate;
        assert_eq!(
            gate.evaluate_command(
                &context(),
                &CapabilityCommandRequest::SetCommand(CommandModeCommandSelection {
                    command_id: None,
                })
            ),
            CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
        );
        assert_eq!(
            gate.evaluate_command(
                &context(),
                &CapabilityCommandRequest::SetStance(CommandModeStanceSelection {
                    stance_id: None,
                    enabled: true,
                })
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
