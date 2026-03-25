pub mod ascii_presenter;
pub mod bin_support;
mod build_user_flow;
pub mod hud_model;
mod minimap_user_flow;
mod panel_model;
mod presenter_view;
pub mod projection;
pub mod render_model;
pub mod scene_present;
pub mod window_presenter;

pub use ascii_presenter::AsciiScenePresenter;
pub use bin_support::{decode_hex, read_world_stream_bytes};
pub use hud_model::{
    BuildConfigAuthoritySourceObservability, BuildConfigInspectorEntryObservability,
    BuildConfigOutcomeObservability, BuildConfigRollbackStripObservability,
    BuildQueueHeadObservability, BuildQueueHeadStage, BuildUiObservability, HudMinimapSummary,
    HudModel, HudViewWindowSummary, RuntimeAdminObservability, RuntimeChatObservability,
    RuntimeCommandControlGroupObservability, RuntimeCommandModeObservability,
    RuntimeCommandRectObservability, RuntimeCommandSelectionObservability,
    RuntimeCommandStanceObservability, RuntimeCommandTargetObservability,
    RuntimeCommandUnitRefObservability, RuntimeHudTextObservability, RuntimeKickObservability,
    RuntimeLiveEffectPositionSource, RuntimeLiveEffectSummaryObservability,
    RuntimeLiveEntitySummaryObservability, RuntimeLiveSummaryObservability,
    RuntimeLoadingObservability, RuntimeMenuObservability, RuntimeReconnectObservability,
    RuntimeReconnectPhaseObservability, RuntimeReconnectReasonKind, RuntimeRulesObservability,
    RuntimeSessionObservability, RuntimeSessionResetKind, RuntimeSessionTimeoutKind,
    RuntimeTextInputObservability, RuntimeToastObservability, RuntimeUiObservability,
    RuntimeWorldLabelObservability, RuntimeWorldPositionObservability,
    RuntimeWorldReloadObservability,
};
pub use projection::{
    project_hud_model, project_render_model, project_render_model_with_player_position,
    project_render_model_with_view_window, project_scene_models,
    project_scene_models_with_player_position, project_scene_models_with_view_window,
};
pub use render_model::{
    RenderModel, RenderObject, RenderSemanticDetailCount, RenderSemanticSummary, RenderViewWindow,
    Viewport,
};
pub use scene_present::ScenePresenter;
pub use window_presenter::{
    BackendSignal, MinifbWindowBackend, PpmSequenceBackend, WindowBackend, WindowFrame,
    WindowPresenter, WindowRunStats,
};
