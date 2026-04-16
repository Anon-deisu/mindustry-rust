use crate::{HudModel, RenderModel};

/// Boundary trait for scene presentation.
///
/// Implementors consume already-projected view-models and render them.
pub trait ScenePresenter {
    fn present(&mut self, scene: &RenderModel, hud: &HudModel);
}

#[cfg(test)]
mod tests {
    use super::ScenePresenter;
    use crate::{HudModel, RenderModel};

    struct DummyPresenter {
        calls: usize,
    }

    impl ScenePresenter for DummyPresenter {
        fn present(&mut self, _scene: &RenderModel, _hud: &HudModel) {
            self.calls += 1;
        }
    }

    fn drive_presenter<P: ScenePresenter>(presenter: &mut P, scene: &RenderModel, hud: &HudModel) {
        presenter.present(scene, hud);
    }

    #[test]
    fn scene_present_trait_compiles_with_render_and_hud_refs() {
        let mut presenter = DummyPresenter { calls: 0 };
        let scene = RenderModel::default();
        let hud = HudModel::default();

        drive_presenter(&mut presenter, &scene, &hud);

        assert_eq!(presenter.calls, 1);
    }
}
