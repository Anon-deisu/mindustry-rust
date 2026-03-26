//! Domain boundary for player input and intent mapping.
//! This crate is intentionally transport-agnostic.

pub mod builder_queue;
pub mod capability_gate;
pub mod command_mode;
pub mod intent;
pub mod live_intent;
pub mod mapper;
pub mod placement_rules;
pub mod plan_editor;
pub mod probe;

pub use builder_queue::{
    BuilderQueueActivityObservation, BuilderQueueActivityState, BuilderQueueBuildSelection,
    BuilderQueueEntry, BuilderQueueEntryObservation, BuilderQueueFrontPromotion,
    BuilderQueueHeadExecutionAction, BuilderQueueHeadExecutionObservation,
    BuilderQueueHeadExecutionResult, BuilderQueueLocalStepResult, BuilderQueueSkipReason,
    BuilderQueueStage, BuilderQueueStateMachine, BuilderQueueTileStateObservation,
    BuilderQueueTransition, BuilderQueueValidationRemovalReason, BuilderQueueValidationResult,
};
pub use capability_gate::{
    CapabilityBuildRequest, CapabilityCommandRequest, CapabilityContext, CapabilityDecision,
    CapabilityDenyReason, CapabilityGate,
};
pub use command_mode::{
    merge_selected_units, CommandModeCommandSelection, CommandModeControlGroupProjection,
    CommandModePositionTarget, CommandModeProjection, CommandModeRectProjection,
    CommandModeSelectionOp, CommandModeStanceSelection, CommandModeState,
    CommandModeTargetProjection, CommandUnitRef,
};
pub use intent::{BinaryAction, BuildPulse, PlayerIntent};
pub use live_intent::{LiveIntentBindingProfile, LiveIntentState};
pub use mapper::{InputSnapshot, IntentMapper, IntentSamplingMode, StatelessIntentMapper};
pub use placement_rules::{
    repair_derelict_candidate, valid_place_against_local_plans, LocalPlanPlacement,
    PlacementRequest, RepairDerelictBuildObservation, RepairDerelictCandidate,
    RepairDerelictObservation,
};
pub use plan_editor::{
    block_offset, flip_plans, rotate_plans, PlanBlockMeta, PlanEditable, PlanEditorPlan, PlanPoint,
    PlanPointConfig, TILE_SIZE,
};
pub use probe::{
    sample_runtime_input_snapshot, MovementProbeConfig, MovementProbeController,
    MovementProbeUpdate, RuntimeInputSample, RuntimeInputState,
};
