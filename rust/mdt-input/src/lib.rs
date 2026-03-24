//! Domain boundary for player input and intent mapping.
//! This crate is intentionally transport-agnostic.

pub mod builder_queue;
pub mod capability_gate;
pub mod command_mode;
pub mod intent;
pub mod live_intent;
pub mod mapper;
pub mod plan_editor;
pub mod probe;

pub use builder_queue::{
    BuilderQueueEntry, BuilderQueueEntryObservation, BuilderQueueStage, BuilderQueueStateMachine,
    BuilderQueueTransition,
};
pub use capability_gate::{
    CapabilityBuildRequest, CapabilityCommandRequest, CapabilityContext, CapabilityDecision,
    CapabilityDenyReason, CapabilityGate,
};
pub use command_mode::{
    CommandModeCommandSelection, CommandModePositionTarget, CommandModeProjection,
    CommandModeStanceSelection, CommandModeState, CommandModeTargetProjection, CommandUnitRef,
};
pub use intent::{BinaryAction, PlayerIntent};
pub use live_intent::LiveIntentState;
pub use mapper::{InputSnapshot, IntentMapper, IntentSamplingMode, StatelessIntentMapper};
pub use plan_editor::{
    block_offset, flip_plans, rotate_plans, PlanBlockMeta, PlanEditable, PlanEditorPlan, PlanPoint,
    PlanPointConfig, TILE_SIZE,
};
pub use probe::{
    MovementProbeConfig, MovementProbeController, MovementProbeUpdate, RuntimeInputState,
};
