use crate::command_mode::{
    CommandModeCommandSelection, CommandModeProjection, CommandModeProjectionSummary,
    CommandModeStanceSelection, CommandModeTargetProjection, CommandUnitRef,
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
    CommandBuilding(CommandModeTargetProjection),
    UnitControl(Option<CommandUnitRef>),
    SetCommand(CommandModeCommandSelection),
    SetStance(CommandModeStanceSelection),
}

impl CapabilityCommandRequest {
    pub fn summary_label(self) -> String {
        match self {
            Self::Target(target) => format!("target={}", target.summary_label()),
            Self::CommandBuilding(target) => {
                format!("command-building={}", target.summary_label())
            }
            Self::UnitControl(target) => format!(
                "unit-control={}",
                target
                    .map(|target| format!("{}:{}", target.kind, target.value))
                    .unwrap_or_else(|| "none".to_string())
            ),
            Self::SetCommand(command) => {
                format!("command={}", optional_u8_label(command.command_id))
            }
            Self::SetStance(stance) => format!(
                "stance={}{}",
                optional_u8_label(stance.stance_id),
                if stance.enabled { ":on" } else { ":off" }
            ),
        }
    }
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
    MissingSelectedCommandUnits,
    MissingSelectedCommandBuildings,
    MissingUnitControlTarget,
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
            Self::MissingSelectedCommandUnits => "missing-selected-command-units",
            Self::MissingSelectedCommandBuildings => "missing-selected-command-buildings",
            Self::MissingUnitControlTarget => "missing-unit-control-target",
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
        } else {
            match request {
                CapabilityCommandRequest::UnitControl(target) => {
                    if target.is_none() {
                        CapabilityDecision::denied(CapabilityDenyReason::MissingUnitControlTarget)
                    } else {
                        CapabilityDecision::allowed()
                    }
                }
                CapabilityCommandRequest::SetCommand(_)
                | CapabilityCommandRequest::SetStance(_) => {
                    if !context.command_mode.active {
                        CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
                    } else if context.command_mode.selected_units.is_empty() {
                        CapabilityDecision::denied(
                            CapabilityDenyReason::MissingSelectedCommandUnits,
                        )
                    } else {
                        CapabilityDecision::allowed()
                    }
                }
                CapabilityCommandRequest::Target(target) => {
                    if !context.command_mode.active {
                        CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
                    } else if context.command_mode.selected_units.is_empty() {
                        CapabilityDecision::denied(
                            CapabilityDenyReason::MissingSelectedCommandUnits,
                        )
                    } else if target.is_empty() {
                        CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget)
                    } else {
                        CapabilityDecision::allowed()
                    }
                }
                CapabilityCommandRequest::CommandBuilding(target) => {
                    if !context.command_mode.active {
                        CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
                    } else if context.command_mode.command_buildings.is_empty() {
                        CapabilityDecision::denied(
                            CapabilityDenyReason::MissingSelectedCommandBuildings,
                        )
                    } else if target.position_target.is_none() {
                        CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget)
                    } else {
                        CapabilityDecision::allowed()
                    }
                }
            }
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

fn optional_u8_label(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_mode::{CommandModePositionTarget, CommandModeStanceSelection};
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

    fn active_command_context() -> CapabilityContext {
        CapabilityContext {
            command_mode: CommandModeProjection {
                active: true,
                ..CommandModeProjection::default()
            },
            ..context()
        }
    }

    fn missing_unit_context() -> CapabilityContext {
        CapabilityContext {
            runtime: RuntimeInputState {
                unit_id: None,
                dead: false,
                position: Some((0.0, 0.0)),
                pointer: None,
            },
            ..context()
        }
    }

    fn dead_context() -> CapabilityContext {
        CapabilityContext {
            runtime: RuntimeInputState {
                dead: true,
                ..context().runtime
            },
            ..context()
        }
    }

    fn mining_disabled_context() -> CapabilityContext {
        CapabilityContext {
            mining_enabled: false,
            ..context()
        }
    }

    fn building_disabled_context() -> CapabilityContext {
        CapabilityContext {
            building_enabled: false,
            ..context()
        }
    }

    fn command_disabled_context() -> CapabilityContext {
        CapabilityContext {
            command_enabled: false,
            ..context()
        }
    }

    fn active_command_context_with_units(selected_units: Vec<i32>) -> CapabilityContext {
        let active_context = active_command_context();
        CapabilityContext {
            command_mode: CommandModeProjection {
                selected_units,
                ..active_context.command_mode.clone()
            },
            ..active_context
        }
    }

    fn active_command_context_with_buildings(command_buildings: Vec<i32>) -> CapabilityContext {
        let active_context = active_command_context();
        CapabilityContext {
            command_mode: CommandModeProjection {
                command_buildings,
                ..active_context.command_mode.clone()
            },
            ..active_context
        }
    }

    fn unit_target(kind: u8, value: i32) -> CommandUnitRef {
        CommandUnitRef { kind, value }
    }

    fn position_target() -> CommandModeTargetProjection {
        CommandModeTargetProjection {
            build_target: None,
            unit_target: None,
            position_target: Some(CommandModePositionTarget {
                x_bits: 12.5f32.to_bits(),
                y_bits: (-4.0f32).to_bits(),
            }),
            rect_target: None,
        }
    }

    fn build_request(
        tile: (i32, i32),
        breaking: bool,
        block_id: Option<i16>,
        rotation: Option<u8>,
    ) -> CapabilityBuildRequest {
        CapabilityBuildRequest {
            tile,
            breaking,
            block_id,
            rotation,
        }
    }

    fn mining_intent(tile: Option<(i32, i32)>) -> PlayerIntent {
        PlayerIntent::SetMiningTile { tile }
    }

    fn set_building_intent(building: bool) -> PlayerIntent {
        PlayerIntent::SetBuilding { building }
    }

    fn config_tap_intent(tile: (i32, i32)) -> PlayerIntent {
        PlayerIntent::ConfigTap { tile }
    }

    fn build_pulse_intent(tile: (i32, i32), breaking: bool) -> PlayerIntent {
        PlayerIntent::BuildPulse(BuildPulse { tile, breaking })
    }

    fn empty_target_request() -> CapabilityCommandRequest {
        CapabilityCommandRequest::Target(CommandModeTargetProjection::default())
    }

    fn target_request(target: CommandModeTargetProjection) -> CapabilityCommandRequest {
        CapabilityCommandRequest::Target(target)
    }

    fn command_building_request(target: CommandModeTargetProjection) -> CapabilityCommandRequest {
        CapabilityCommandRequest::CommandBuilding(target)
    }

    fn set_command_request(command_id: Option<u8>) -> CapabilityCommandRequest {
        CapabilityCommandRequest::SetCommand(CommandModeCommandSelection { command_id })
    }

    fn set_stance_request(stance_id: Option<u8>, enabled: bool) -> CapabilityCommandRequest {
        CapabilityCommandRequest::SetStance(CommandModeStanceSelection { stance_id, enabled })
    }

    fn unit_control_request(target: Option<CommandUnitRef>) -> CapabilityCommandRequest {
        CapabilityCommandRequest::UnitControl(target)
    }

    fn assert_allowed(decision: CapabilityDecision) {
        assert_eq!(decision, CapabilityDecision::allowed());
    }

    fn assert_denied(decision: CapabilityDecision, reason: CapabilityDenyReason) {
        assert_eq!(decision, CapabilityDecision::denied(reason));
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
                    unit_target: Some(unit_target(1, 7)),
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
        assert_eq!(
            projection.command_mode.summary_label(),
            "target+command+stance"
        );
        assert_eq!(
            projection.command_mode.recent_selection_label(),
            "target+command+stance"
        );
        assert_eq!(
            empty_target_request().summary_label(),
            "target=none"
        );
        assert_eq!(
            target_request(CommandModeTargetProjection {
                build_target: Some(9),
                unit_target: Some(unit_target(1, 7)),
                position_target: None,
                rect_target: None,
            })
            .summary_label(),
            "target=build+unit"
        );
        assert_eq!(
            command_building_request(position_target()).summary_label(),
            "command-building=position"
        );
        assert_eq!(
            unit_control_request(Some(unit_target(2, 99))).summary_label(),
            "unit-control=2:99"
        );
        assert_eq!(
            set_command_request(Some(4)).summary_label(),
            "command=4"
        );
        assert_eq!(
            set_stance_request(None, true).summary_label(),
            "stance=none:on"
        );
        assert_eq!(CapabilityDecision::allowed().label(), "allowed");
        assert_eq!(
            CapabilityDecision::denied(CapabilityDenyReason::MissingCommandTarget).label(),
            "missing-command-target"
        );
        assert_eq!(
            CapabilityDenyReason::CommandDisabled.label(),
            "command-disabled"
        );
        assert_eq!(
            CapabilityDenyReason::MissingSelectedCommandUnits.label(),
            "missing-selected-command-units"
        );
        assert_eq!(
            CapabilityDenyReason::MissingSelectedCommandBuildings.label(),
            "missing-selected-command-buildings"
        );
        assert_eq!(
            CapabilityDenyReason::MissingUnitControlTarget.label(),
            "missing-unit-control-target"
        );
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
        let missing = missing_unit_context();
        let dead = dead_context();

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
        let missing_unit = missing_unit_context();

        assert_denied(
            gate.evaluate_intent(&missing_unit, &mining_intent(Some((7, 9)))),
            CapabilityDenyReason::MissingControlledUnit,
        );
        assert_allowed(gate.evaluate_intent(&missing_unit, &mining_intent(None)));
    }

    #[test]
    fn mining_requests_reject_when_mining_disabled_with_live_unit() {
        let gate = CapabilityGate;
        let mining_disabled = mining_disabled_context();

        assert_denied(
            gate.evaluate_mining(&mining_disabled, (7, 9)),
            CapabilityDenyReason::MiningDisabled,
        );
    }

    #[test]
    fn mining_and_build_requests_reject_dead_units_before_other_checks() {
        let gate = CapabilityGate;
        let dead_context = dead_context();

        assert_denied(
            gate.evaluate_mining(&dead_context, (3, 4)),
            CapabilityDenyReason::ControlledUnitDead,
        );
        assert_denied(
            gate.evaluate_build(
                &dead_context,
                &build_request((3, 4), true, None, Some(0))
            ),
            CapabilityDenyReason::ControlledUnitDead,
        );
    }

    #[test]
    fn build_requests_reject_disabled_building_and_missing_placement_block() {
        let gate = CapabilityGate;
        let disabled_building = building_disabled_context();

        assert_denied(
            gate.evaluate_build(
                &disabled_building,
                &build_request((10, 11), false, Some(5), Some(2))
            ),
            CapabilityDenyReason::BuildingDisabled,
        );
        assert_denied(
            gate.evaluate_build(
                &context(),
                &build_request((10, 11), false, None, Some(2))
            ),
            CapabilityDenyReason::MissingBuildBlock,
        );
    }

    #[test]
    fn building_intents_require_building_capability_but_allow_clear_toggle() {
        let gate = CapabilityGate;
        let disabled_building = building_disabled_context();
        let missing_unit = missing_unit_context();

        assert_denied(
            gate.evaluate_intent(&disabled_building, &set_building_intent(true)),
            CapabilityDenyReason::BuildingDisabled,
        );
        assert_denied(
            gate.evaluate_intent(&disabled_building, &config_tap_intent((7, 9))),
            CapabilityDenyReason::BuildingDisabled,
        );
        assert_denied(
            gate.evaluate_intent(&disabled_building, &build_pulse_intent((7, 9), false)),
            CapabilityDenyReason::BuildingDisabled,
        );
        assert_allowed(gate.evaluate_intent(&disabled_building, &set_building_intent(false)));
        assert_denied(
            gate.evaluate_intent(&missing_unit, &set_building_intent(true)),
            CapabilityDenyReason::MissingControlledUnit,
        );
    }

    #[test]
    fn build_pulse_without_block_is_denied() {
        let gate = CapabilityGate;

        assert_denied(
            gate.evaluate_intent(&context(), &build_pulse_intent((7, 9), false)),
            CapabilityDenyReason::MissingBuildBlock,
        );
        assert_allowed(gate.evaluate_intent(&context(), &build_pulse_intent((7, 9), true)));
    }

    #[test]
    fn command_requests_short_circuit_when_command_capability_is_disabled() {
        let gate = CapabilityGate;
        let disabled_command = command_disabled_context();

        assert_denied(
            gate.evaluate_command(&disabled_command, &empty_target_request()),
            CapabilityDenyReason::CommandDisabled,
        );
    }

    #[test]
    fn command_target_requests_require_active_command_mode_and_non_empty_target() {
        let gate = CapabilityGate;

        assert_denied(
            gate.evaluate_command(&context(), &empty_target_request()),
            CapabilityDenyReason::CommandModeInactive,
        );

        let active_context = active_command_context();

        assert_denied(
            gate.evaluate_command(&active_context, &empty_target_request()),
            CapabilityDenyReason::MissingSelectedCommandUnits,
        );
        assert_allowed(
            gate.evaluate_command(
                &active_command_context_with_units(vec![77]),
                &target_request(CommandModeTargetProjection {
                    build_target: None,
                    unit_target: Some(unit_target(1, 99)),
                    position_target: None,
                    rect_target: None,
                })
            ),
        );
    }

    #[test]
    fn command_selection_requests_require_selected_units_after_mode_activation() {
        let gate = CapabilityGate;
        assert_denied(
            gate.evaluate_command(&context(), &set_command_request(None)),
            CapabilityDenyReason::CommandModeInactive,
        );
        assert_denied(
            gate.evaluate_command(&context(), &set_stance_request(None, true)),
            CapabilityDenyReason::CommandModeInactive,
        );

        let active_context = active_command_context();

        assert_denied(
            gate.evaluate_command(&active_context, &set_command_request(None)),
            CapabilityDenyReason::MissingSelectedCommandUnits,
        );
        assert_denied(
            gate.evaluate_command(&active_context, &set_stance_request(None, true)),
            CapabilityDenyReason::MissingSelectedCommandUnits,
        );

        let selected_context = active_command_context_with_units(vec![42, 77]);

        assert_allowed(gate.evaluate_command(&selected_context, &set_command_request(None)));
        assert_allowed(gate.evaluate_command(
            &selected_context,
            &set_stance_request(None, true),
        ));
    }

    #[test]
    fn command_building_requests_require_selected_buildings_and_position_target() {
        let gate = CapabilityGate;
        let position_target = position_target();

        assert_denied(
            gate.evaluate_command(&context(), &command_building_request(position_target)),
            CapabilityDenyReason::CommandModeInactive,
        );

        let active_context = active_command_context();
        assert_denied(
            gate.evaluate_command(&active_context, &command_building_request(position_target)),
            CapabilityDenyReason::MissingSelectedCommandBuildings,
        );

        let building_context = active_command_context_with_buildings(vec![3]);

        assert_denied(
            gate.evaluate_command(
                &building_context,
                &command_building_request(CommandModeTargetProjection::default())
            ),
            CapabilityDenyReason::MissingCommandTarget,
        );
        assert_allowed(
            gate.evaluate_command(&building_context, &command_building_request(position_target)),
        );
    }

    #[test]
    fn unit_control_requests_require_explicit_target_without_command_mode_activation() {
        let gate = CapabilityGate;

        assert_denied(
            gate.evaluate_command(&context(), &unit_control_request(None)),
            CapabilityDenyReason::MissingUnitControlTarget,
        );
        assert_allowed(
            gate.evaluate_command(
                &context(),
                &unit_control_request(Some(unit_target(2, 404)))
            ),
        );
    }
}
