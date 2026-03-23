use crate::{HudModel, RenderModel};

/// Boundary trait for scene presentation.
///
/// Implementors consume already-projected view-models and render them.
pub trait ScenePresenter {
    fn present(&mut self, scene: &RenderModel, hud: &HudModel);
}
