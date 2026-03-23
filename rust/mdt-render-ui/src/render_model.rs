/// Render-facing projection of world state for UI drawing.
///
/// This crate intentionally avoids protocol parsing and transport concerns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderModel {
    pub viewport: Viewport,
    pub objects: Vec<RenderObject>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderObject {
    pub id: String,
    pub layer: i32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderObjectSemanticKind {
    Player,
    Marker,
    Plan,
    Block,
    Terrain,
    Unknown,
}

impl RenderObject {
    pub fn semantic_kind(&self) -> RenderObjectSemanticKind {
        RenderObjectSemanticKind::from_id(&self.id)
    }
}

impl RenderObjectSemanticKind {
    pub fn from_id(id: &str) -> Self {
        let prefix = id.split_once(':').map(|(head, _)| head).unwrap_or(id);
        match prefix {
            "player" | "unit" => Self::Player,
            "marker" | "hint" => Self::Marker,
            "plan" | "build-plan" => Self::Plan,
            "block" | "building" => Self::Block,
            "terrain" | "tile" => Self::Terrain,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderObject, RenderObjectSemanticKind};

    #[test]
    fn semantic_kind_from_id_supports_known_prefixes_and_aliases() {
        assert_eq!(
            RenderObjectSemanticKind::from_id("player:7"),
            RenderObjectSemanticKind::Player
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("unit:7"),
            RenderObjectSemanticKind::Player
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:1"),
            RenderObjectSemanticKind::Marker
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("hint:1"),
            RenderObjectSemanticKind::Marker
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("plan:2"),
            RenderObjectSemanticKind::Plan
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("build-plan:2"),
            RenderObjectSemanticKind::Plan
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("block:3:4"),
            RenderObjectSemanticKind::Block
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("building:3:4"),
            RenderObjectSemanticKind::Block
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("terrain:9"),
            RenderObjectSemanticKind::Terrain
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("tile:9"),
            RenderObjectSemanticKind::Terrain
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("unknown"),
            RenderObjectSemanticKind::Unknown
        );
    }

    #[test]
    fn render_object_exposes_semantic_kind() {
        let marker = RenderObject {
            id: "marker:11".to_string(),
            layer: 30,
            x: 0.0,
            y: 0.0,
        };
        assert_eq!(marker.semantic_kind(), RenderObjectSemanticKind::Marker);
    }
}
