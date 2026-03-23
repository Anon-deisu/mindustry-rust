/// UI/HUD-specific view-model data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HudModel {
    pub title: String,
    pub status_text: String,
    pub fps: Option<f32>,
    pub summary: Option<HudSummary>,
}

/// Structured HUD summary that mirrors core status fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudSummary {
    pub player_name: String,
    pub team_id: u8,
    pub selected_block: String,
    pub plan_count: usize,
    pub marker_count: usize,
    pub map_width: usize,
    pub map_height: usize,
}

impl HudModel {
    pub fn hidden() -> Self {
        Self::default()
    }

    pub fn is_hidden(&self) -> bool {
        self.title.is_empty()
            && self.status_text.is_empty()
            && self.fps.is_none()
            && self.summary.is_none()
    }

    pub fn is_visible(&self) -> bool {
        !self.is_hidden()
    }
}
