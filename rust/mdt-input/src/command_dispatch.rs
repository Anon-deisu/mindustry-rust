use crate::capability_gate::{
    CapabilityCommandRequest, CapabilityContext, CapabilityDecision, CapabilityGate,
};
use crate::command_mode::{
    CommandModeCommandSelection, CommandModeProjection, CommandModeStanceSelection,
    CommandModeTargetProjection, CommandUnitRef,
};

pub type CommandDispatchRequest = CapabilityCommandRequest;

/// Pure local command-dispatch planner.
///
/// This layer intentionally stops at preflight plus local plan assembly:
/// - it reuses the existing capability gate for deny/allow semantics;
/// - it snapshots the currently selected local units/buildings into a transport-agnostic plan;
/// - it does not encode packets or touch runtime transport state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandDispatchPlanner {
    gate: CapabilityGate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchPlan {
    Target {
        unit_ids: Vec<i32>,
        target: CommandModeTargetProjection,
    },
    CommandBuilding {
        building_ids: Vec<i32>,
        target: CommandModeTargetProjection,
    },
    UnitControl {
        target: CommandUnitRef,
    },
    SetCommand {
        unit_ids: Vec<i32>,
        selection: CommandModeCommandSelection,
    },
    SetStance {
        unit_ids: Vec<i32>,
        selection: CommandModeStanceSelection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDispatchPlanningResult {
    pub request: CommandDispatchRequest,
    pub decision: CapabilityDecision,
    pub plan: Option<CommandDispatchPlan>,
}

impl CommandDispatchPlan {
    pub fn family_label(&self) -> &'static str {
        match self {
            Self::Target { .. } => "target",
            Self::CommandBuilding { .. } => "command-building",
            Self::UnitControl { .. } => "unit-control",
            Self::SetCommand { .. } => "set-command",
            Self::SetStance { .. } => "set-stance",
        }
    }

    pub fn summary_label(&self) -> String {
        match self {
            Self::Target { unit_ids, target } => {
                format!(
                    "target units={} target={}",
                    unit_ids.len(),
                    target.summary_label()
                )
            }
            Self::CommandBuilding {
                building_ids,
                target,
            } => format!(
                "command-building buildings={} target={}",
                building_ids.len(),
                target.summary_label()
            ),
            Self::UnitControl { target } => {
                format!("unit-control target={}:{}", target.kind, target.value)
            }
            Self::SetCommand {
                unit_ids,
                selection,
            } => format!(
                "set-command units={} command={}",
                unit_ids.len(),
                selection
                    .command_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            Self::SetStance {
                unit_ids,
                selection,
            } => format!(
                "set-stance units={} stance={} enabled={}",
                unit_ids.len(),
                selection
                    .stance_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                if selection.enabled { "on" } else { "off" }
            ),
        }
    }
}

impl CommandDispatchPlanningResult {
    pub fn allowed(&self) -> bool {
        self.decision.allowed
    }

    pub fn deny_reason_label(&self) -> &'static str {
        self.decision.reason_label()
    }

    pub fn summary_label(&self) -> String {
        let plan_label = self
            .plan
            .as_ref()
            .map(CommandDispatchPlan::summary_label)
            .unwrap_or_else(|| "none".to_string());
        format!(
            "request={} decision={} plan={}",
            self.request.summary_label(),
            self.decision.label(),
            plan_label
        )
    }
}

impl CommandDispatchPlanner {
    /// Derives a conservative local command-dispatch request from recent command-mode state.
    ///
    /// This intentionally refuses ambiguous recent selections:
    /// - stance takes precedence over command selection;
    /// - command selection takes precedence over target-based inference;
    /// - building-only + position-only target maps to `CommandBuilding`;
    /// - no selection + unit-only target maps to `UnitControl`;
    /// - mixed unit/building selection and selection-like targets (for example rect-only or
    ///   unit-only targets with selected units) return `None`.
    pub fn request_from_context(
        &self,
        context: &CapabilityContext,
    ) -> Option<CommandDispatchRequest> {
        request_from_command_mode(&context.command_mode)
    }

    pub fn plan_recent(
        &self,
        context: &CapabilityContext,
    ) -> Option<CommandDispatchPlanningResult> {
        self.request_from_context(context)
            .map(|request| self.plan(context, request))
    }

    pub fn preflight(
        &self,
        context: &CapabilityContext,
        request: &CommandDispatchRequest,
    ) -> CapabilityDecision {
        self.gate.evaluate_command(context, request)
    }

    pub fn plan(
        &self,
        context: &CapabilityContext,
        request: CommandDispatchRequest,
    ) -> CommandDispatchPlanningResult {
        let decision = self.preflight(context, &request);
        let plan = decision
            .allowed
            .then(|| allowed_dispatch_plan(context, request));
        CommandDispatchPlanningResult {
            request,
            decision,
            plan,
        }
    }
}

fn request_from_command_mode(
    command_mode: &CommandModeProjection,
) -> Option<CommandDispatchRequest> {
    if let Some(selection) = command_mode.last_stance_selection {
        return Some(CommandDispatchRequest::SetStance(selection));
    }

    if let Some(selection) = command_mode.last_command_selection {
        return Some(CommandDispatchRequest::SetCommand(selection));
    }

    let target = command_mode
        .last_target
        .filter(|target| !target.is_empty())?;
    let has_units = !command_mode.selected_units.is_empty();
    let has_buildings = !command_mode.command_buildings.is_empty();
    let target_is_unit_only = target.unit_target.is_some()
        && target.build_target.is_none()
        && target.position_target.is_none()
        && target.rect_target.is_none();
    let target_is_position_only = target.position_target.is_some()
        && target.build_target.is_none()
        && target.unit_target.is_none()
        && target.rect_target.is_none();

    match (has_units, has_buildings) {
        (true, true) => None,
        (false, true) if target_is_position_only => {
            Some(CommandDispatchRequest::CommandBuilding(target))
        }
        (true, false) if target.rect_target.is_none() && !target_is_unit_only => {
            Some(CommandDispatchRequest::Target(target))
        }
        (false, false) if target_is_unit_only => {
            Some(CommandDispatchRequest::UnitControl(target.unit_target))
        }
        _ => None,
    }
}

fn allowed_dispatch_plan(
    context: &CapabilityContext,
    request: CommandDispatchRequest,
) -> CommandDispatchPlan {
    match request {
        CapabilityCommandRequest::Target(target) => CommandDispatchPlan::Target {
            unit_ids: context.command_mode.selected_units.clone(),
            target,
        },
        CapabilityCommandRequest::CommandBuilding(target) => CommandDispatchPlan::CommandBuilding {
            building_ids: context.command_mode.command_buildings.clone(),
            target,
        },
        CapabilityCommandRequest::UnitControl(Some(target)) => {
            CommandDispatchPlan::UnitControl { target }
        }
        CapabilityCommandRequest::SetCommand(selection) => CommandDispatchPlan::SetCommand {
            unit_ids: context.command_mode.selected_units.clone(),
            selection,
        },
        CapabilityCommandRequest::SetStance(selection) => CommandDispatchPlan::SetStance {
            unit_ids: context.command_mode.selected_units.clone(),
            selection,
        },
        CapabilityCommandRequest::UnitControl(None) => {
            unreachable!("allowed unit-control planning requires a concrete target after preflight")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_gate::CapabilityDenyReason;
    use crate::command_mode::{
        CommandModePositionTarget, CommandModeProjection, CommandModeRectProjection,
    };
    use crate::probe::RuntimeInputState;

    fn live_context() -> CapabilityContext {
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

    fn context_with_command_mode(command_mode: CommandModeProjection) -> CapabilityContext {
        CapabilityContext {
            command_mode,
            ..live_context()
        }
    }

    fn active_command_mode() -> CommandModeProjection {
        CommandModeProjection {
            active: true,
            ..CommandModeProjection::default()
        }
    }

    fn active_unit_context() -> CapabilityContext {
        context_with_command_mode(CommandModeProjection {
            selected_units: vec![11, 22],
            ..active_command_mode()
        })
    }

    fn active_building_context() -> CapabilityContext {
        context_with_command_mode(CommandModeProjection {
            command_buildings: vec![101, 202],
            ..active_command_mode()
        })
    }

    fn position_target(x: f32, y: f32) -> CommandModePositionTarget {
        CommandModePositionTarget {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        }
    }

    fn unit_ref(kind: u8, value: i32) -> CommandUnitRef {
        CommandUnitRef { kind, value }
    }

    fn target_projection(
        build_target: Option<i32>,
        unit_target: Option<CommandUnitRef>,
        position: Option<(f32, f32)>,
        rect_target: Option<CommandModeRectProjection>,
    ) -> CommandModeTargetProjection {
        CommandModeTargetProjection {
            build_target,
            unit_target,
            position_target: position.map(|(x, y)| position_target(x, y)),
            rect_target,
        }
    }

    fn target_with_position(x: f32, y: f32) -> CommandModeTargetProjection {
        target_projection(None, None, Some((x, y)), None)
    }

    fn target_with_unit(kind: u8, value: i32) -> CommandModeTargetProjection {
        target_projection(None, Some(unit_ref(kind, value)), None, None)
    }

    fn rect_target(x0: i32, y0: i32, x1: i32, y1: i32) -> CommandModeRectProjection {
        CommandModeRectProjection { x0, y0, x1, y1 }
    }

    fn target_with_rect(x0: i32, y0: i32, x1: i32, y1: i32) -> CommandModeTargetProjection {
        target_projection(None, None, None, Some(rect_target(x0, y0, x1, y1)))
    }

    fn command_selection(command_id: Option<u8>) -> CommandModeCommandSelection {
        CommandModeCommandSelection { command_id }
    }

    fn stance_selection(stance_id: Option<u8>, enabled: bool) -> CommandModeStanceSelection {
        CommandModeStanceSelection { stance_id, enabled }
    }

    fn assert_preflight_matches_gate(
        planner: &CommandDispatchPlanner,
        context: &CapabilityContext,
        request: &CommandDispatchRequest,
    ) {
        let gate = CapabilityGate;
        assert_eq!(
            planner.preflight(context, request),
            gate.evaluate_command(context, request)
        );
    }

    #[test]
    fn target_planning_reuses_preflight_and_captures_selected_units() {
        let planner = CommandDispatchPlanner::default();
        let request = CommandDispatchRequest::Target(target_projection(
            Some(99),
            Some(unit_ref(2, 303)),
            Some((12.5, -4.0)),
            None,
        ));
        let context = active_unit_context();

        assert_preflight_matches_gate(&planner, &context, &request);

        let plan = planner.plan(&context, request);
        assert!(plan.allowed());
        assert_eq!(
            plan.plan,
            Some(CommandDispatchPlan::Target {
                unit_ids: vec![11, 22],
                target: target_projection(
                    Some(99),
                    Some(unit_ref(2, 303)),
                    Some((12.5, -4.0)),
                    None,
                ),
            })
        );
    }

    #[test]
    fn command_building_planning_reuses_preflight_and_captures_selected_buildings() {
        let planner = CommandDispatchPlanner::default();
        let request = CommandDispatchRequest::CommandBuilding(target_with_position(8.0, 16.0));
        let context = active_building_context();

        assert_preflight_matches_gate(&planner, &context, &request);

        let plan = planner.plan(&context, request);
        assert!(plan.allowed());
        assert_eq!(
            plan.plan,
            Some(CommandDispatchPlan::CommandBuilding {
                building_ids: vec![101, 202],
                target: target_with_position(8.0, 16.0),
            })
        );
    }

    #[test]
    fn unit_control_planning_preserves_existing_deny_reason_and_only_plans_concrete_targets() {
        let planner = CommandDispatchPlanner::default();
        let missing_target_request = CommandDispatchRequest::UnitControl(None);
        let denied = planner.plan(&live_context(), missing_target_request);

        assert_preflight_matches_gate(&planner, &live_context(), &missing_target_request);
        assert!(!denied.allowed());
        assert_eq!(
            denied.decision,
            CapabilityDecision::denied(CapabilityDenyReason::MissingUnitControlTarget)
        );
        assert_eq!(denied.plan, None);

        let allowed_request = CommandDispatchRequest::UnitControl(Some(unit_ref(1, 404)));
        let allowed = planner.plan(&live_context(), allowed_request);
        assert!(allowed.allowed());
        assert_eq!(
            allowed.plan,
            Some(CommandDispatchPlan::UnitControl {
                target: unit_ref(1, 404),
            })
        );
    }

    #[test]
    fn command_selection_and_stance_planning_use_active_selected_units_without_changing_denials() {
        let planner = CommandDispatchPlanner::default();
        let inactive_context = live_context();
        let active_context = active_unit_context();
        let set_command = CommandDispatchRequest::SetCommand(command_selection(Some(7)));
        let set_stance = CommandDispatchRequest::SetStance(stance_selection(None, false));

        assert_preflight_matches_gate(&planner, &inactive_context, &set_command);
        assert_eq!(
            planner.plan(&inactive_context, set_command).decision,
            CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
        );
        assert_preflight_matches_gate(&planner, &inactive_context, &set_stance);
        assert_eq!(
            planner.plan(&inactive_context, set_stance).decision,
            CapabilityDecision::denied(CapabilityDenyReason::CommandModeInactive)
        );

        let command_plan = planner.plan(
            &active_context,
            CommandDispatchRequest::SetCommand(command_selection(Some(7))),
        );
        assert_eq!(
            command_plan.plan,
            Some(CommandDispatchPlan::SetCommand {
                unit_ids: vec![11, 22],
                selection: command_selection(Some(7)),
            })
        );

        let stance_plan = planner.plan(
            &active_context,
            CommandDispatchRequest::SetStance(stance_selection(None, false)),
        );
        assert_eq!(
            stance_plan.plan,
            Some(CommandDispatchPlan::SetStance {
                unit_ids: vec![11, 22],
                selection: stance_selection(None, false),
            })
        );
    }

    #[test]
    fn planning_result_summary_reports_request_decision_and_plan_state() {
        let planner = CommandDispatchPlanner::default();
        let result = planner.plan(
            &active_unit_context(),
            CommandDispatchRequest::SetCommand(command_selection(None)),
        );
        assert_eq!(
            result.summary_label(),
            "request=command=none decision=allowed plan=set-command units=2 command=none"
        );

        let denied = planner.plan(
            &live_context(),
            CommandDispatchRequest::Target(Default::default()),
        );
        assert_eq!(
            denied.summary_label(),
            "request=target=none decision=command-mode-inactive plan=none"
        );
        assert_eq!(denied.deny_reason_label(), "command-mode-inactive");
    }

    #[test]
    fn request_from_context_prefers_recent_stance_then_command_over_target() {
        let planner = CommandDispatchPlanner::default();
        let stance = stance_selection(Some(9), true);
        let command = command_selection(Some(7));
        let target = target_projection(Some(88), Some(unit_ref(2, 303)), Some((6.0, 12.0)), None);
        let context = context_with_command_mode(CommandModeProjection {
            selected_units: vec![11, 22],
            last_target: Some(target),
            last_command_selection: Some(command),
            last_stance_selection: Some(stance),
            ..active_command_mode()
        });

        assert_eq!(
            planner.request_from_context(&context),
            Some(CommandDispatchRequest::SetStance(stance))
        );
        assert_eq!(
            planner.plan_recent(&context),
            Some(CommandDispatchPlanningResult {
                request: CommandDispatchRequest::SetStance(stance),
                decision: CapabilityDecision::allowed(),
                plan: Some(CommandDispatchPlan::SetStance {
                    unit_ids: vec![11, 22],
                    selection: stance,
                }),
            })
        );

        let command_context = context_with_command_mode(CommandModeProjection {
            last_stance_selection: None,
            ..context.command_mode.clone()
        });
        assert_eq!(
            planner.request_from_context(&command_context),
            Some(CommandDispatchRequest::SetCommand(command))
        );
    }

    #[test]
    fn request_from_context_maps_building_only_recent_position_to_command_building() {
        let planner = CommandDispatchPlanner::default();
        let target = target_with_position(8.0, 16.0);
        let context = context_with_command_mode(CommandModeProjection {
            command_buildings: vec![101, 202],
            last_target: Some(target),
            ..active_command_mode()
        });

        assert_eq!(
            planner.request_from_context(&context),
            Some(CommandDispatchRequest::CommandBuilding(target))
        );

        let result = planner.plan_recent(&context);
        assert!(result
            .as_ref()
            .is_some_and(CommandDispatchPlanningResult::allowed));
        assert_eq!(
            result.and_then(|value| value.plan),
            Some(CommandDispatchPlan::CommandBuilding {
                building_ids: vec![101, 202],
                target,
            })
        );
    }

    #[test]
    fn request_from_context_maps_unselected_recent_unit_target_to_unit_control() {
        let planner = CommandDispatchPlanner::default();
        let target = unit_ref(1, 404);
        let context = context_with_command_mode(CommandModeProjection {
            last_target: Some(target_with_unit(1, 404)),
            ..CommandModeProjection::default()
        });

        assert_eq!(
            planner.request_from_context(&context),
            Some(CommandDispatchRequest::UnitControl(Some(target)))
        );
        assert_eq!(
            planner.plan_recent(&context).and_then(|value| value.plan),
            Some(CommandDispatchPlan::UnitControl { target })
        );
    }

    #[test]
    fn request_from_context_returns_none_for_ambiguous_or_selection_like_recent_targets() {
        let planner = CommandDispatchPlanner::default();

        let mixed_selection = context_with_command_mode(CommandModeProjection {
            selected_units: vec![11],
            command_buildings: vec![101],
            last_target: Some(target_with_position(1.0, 2.0)),
            ..active_command_mode()
        });
        assert_eq!(planner.request_from_context(&mixed_selection), None);
        assert_eq!(planner.plan_recent(&mixed_selection), None);

        let rect_only = context_with_command_mode(CommandModeProjection {
            selected_units: vec![11, 22],
            last_target: Some(target_with_rect(1, 2, 3, 4)),
            ..active_command_mode()
        });
        assert_eq!(planner.request_from_context(&rect_only), None);

        let selected_unit_only = context_with_command_mode(CommandModeProjection {
            selected_units: vec![11, 22],
            last_target: Some(target_with_unit(2, 33)),
            ..active_command_mode()
        });
        assert_eq!(planner.request_from_context(&selected_unit_only), None);
    }
}
