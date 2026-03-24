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
pub enum RenderObjectSemanticFamily {
    Player,
    Runtime,
    Marker,
    Plan,
    Block,
    Terrain,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderObjectSemanticKind {
    Player,
    Marker,
    MarkerLine,
    MarkerLineEnd,
    Plan,
    Block,
    Terrain,
    RuntimeBuilding,
    RuntimeSnapshotHead,
    RuntimeDeconstruct,
    RuntimeConfig,
    RuntimeConfigParseFail,
    RuntimeConfigNoApply,
    RuntimeConfigRollback,
    RuntimeConfigPendingMismatch,
    RuntimeHealth,
    RuntimeEffect,
    RuntimeBreak,
    RuntimePlace,
    Runtime,
    Unknown,
}

impl RenderObject {
    pub fn semantic_kind(&self) -> RenderObjectSemanticKind {
        RenderObjectSemanticKind::from_id(&self.id)
    }

    pub fn semantic_family(&self) -> RenderObjectSemanticFamily {
        self.semantic_kind().family()
    }
}

impl RenderObjectSemanticKind {
    pub fn from_id(id: &str) -> Self {
        let segments = id.split(':').collect::<Vec<_>>();
        if segments.is_empty() {
            return Self::Unknown;
        }
        let prefix = segments[0];
        let second = segments.get(1).copied().unwrap_or_default();

        if matches!(prefix, "marker" | "hint") && segments.last() == Some(&"line-end") {
            return Self::MarkerLineEnd;
        }

        match prefix {
            "player" | "unit" => Self::Player,
            "marker" | "hint" => marker_semantic_kind(second),
            "plan" | "build-plan" => plan_semantic_kind(second),
            "block" | "building" => block_semantic_kind(second),
            "terrain" | "tile" => terrain_semantic_kind(second),
            _ => Self::Unknown,
        }
    }

    pub fn family(self) -> RenderObjectSemanticFamily {
        match self {
            Self::Player => RenderObjectSemanticFamily::Player,
            Self::Marker | Self::MarkerLine | Self::MarkerLineEnd => {
                RenderObjectSemanticFamily::Marker
            }
            Self::Plan => RenderObjectSemanticFamily::Plan,
            Self::Block => RenderObjectSemanticFamily::Block,
            Self::Terrain => RenderObjectSemanticFamily::Terrain,
            Self::RuntimeBuilding
            | Self::RuntimeSnapshotHead
            | Self::RuntimeDeconstruct
            | Self::RuntimeConfig
            | Self::RuntimeConfigParseFail
            | Self::RuntimeConfigNoApply
            | Self::RuntimeConfigRollback
            | Self::RuntimeConfigPendingMismatch
            | Self::RuntimeHealth
            | Self::RuntimeEffect
            | Self::RuntimeBreak
            | Self::RuntimePlace
            | Self::Runtime => RenderObjectSemanticFamily::Runtime,
            Self::Unknown => RenderObjectSemanticFamily::Unknown,
        }
    }

    pub fn detail_label(self) -> Option<&'static str> {
        match self {
            Self::MarkerLine => Some("marker-line"),
            Self::MarkerLineEnd => Some("marker-line-end"),
            Self::RuntimeBuilding => Some("runtime-building"),
            Self::RuntimeSnapshotHead => Some("runtime-snapshot-head"),
            Self::RuntimeDeconstruct => Some("runtime-deconstruct"),
            Self::RuntimeConfig => Some("runtime-config"),
            Self::RuntimeConfigParseFail => Some("runtime-config-parse-fail"),
            Self::RuntimeConfigNoApply => Some("runtime-config-noapply"),
            Self::RuntimeConfigRollback => Some("runtime-config-rollback"),
            Self::RuntimeConfigPendingMismatch => Some("runtime-config-pending-mismatch"),
            Self::RuntimeHealth => Some("runtime-health"),
            Self::RuntimeEffect => Some("runtime-effect"),
            Self::RuntimeBreak => Some("runtime-break"),
            Self::RuntimePlace => Some("runtime-place"),
            Self::Runtime => Some("runtime"),
            Self::Player
            | Self::Marker
            | Self::Plan
            | Self::Block
            | Self::Terrain
            | Self::Unknown => None,
        }
    }
}

fn marker_semantic_kind(second: &str) -> RenderObjectSemanticKind {
    match second {
        "line" => RenderObjectSemanticKind::MarkerLine,
        value if value.starts_with("runtime-config-parse-fail") => {
            RenderObjectSemanticKind::RuntimeConfigParseFail
        }
        value if value.starts_with("runtime-config-noapply") => {
            RenderObjectSemanticKind::RuntimeConfigNoApply
        }
        value if value.starts_with("runtime-config-rollback") => {
            RenderObjectSemanticKind::RuntimeConfigRollback
        }
        value if value.starts_with("runtime-config-pending-mismatch") => {
            RenderObjectSemanticKind::RuntimeConfigPendingMismatch
        }
        value if value.starts_with("runtime-config") => RenderObjectSemanticKind::RuntimeConfig,
        "runtime-health" => RenderObjectSemanticKind::RuntimeHealth,
        "runtime-effect" => RenderObjectSemanticKind::RuntimeEffect,
        "runtime-break" => RenderObjectSemanticKind::RuntimeBreak,
        value if value.starts_with("runtime") => RenderObjectSemanticKind::Runtime,
        _ => RenderObjectSemanticKind::Marker,
    }
}

fn plan_semantic_kind(second: &str) -> RenderObjectSemanticKind {
    match second {
        "runtime-place" => RenderObjectSemanticKind::RuntimePlace,
        value if value.starts_with("runtime") => RenderObjectSemanticKind::Runtime,
        _ => RenderObjectSemanticKind::Plan,
    }
}

fn block_semantic_kind(second: &str) -> RenderObjectSemanticKind {
    match second {
        "runtime-building" => RenderObjectSemanticKind::RuntimeBuilding,
        "runtime-snapshot-head" => RenderObjectSemanticKind::RuntimeSnapshotHead,
        value if value.starts_with("runtime") => RenderObjectSemanticKind::Runtime,
        _ => RenderObjectSemanticKind::Block,
    }
}

fn terrain_semantic_kind(second: &str) -> RenderObjectSemanticKind {
    match second {
        "runtime-deconstruct" => RenderObjectSemanticKind::RuntimeDeconstruct,
        value if value.starts_with("runtime") => RenderObjectSemanticKind::Runtime,
        _ => RenderObjectSemanticKind::Terrain,
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderObject, RenderObjectSemanticFamily, RenderObjectSemanticKind};

    #[test]
    fn semantic_kind_from_id_supports_known_prefixes_aliases_and_runtime_patterns() {
        assert_eq!(
            RenderObjectSemanticKind::from_id("player:7"),
            RenderObjectSemanticKind::Player
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("unit:7"),
            RenderObjectSemanticKind::Player
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-config:3:2:string"),
            RenderObjectSemanticKind::RuntimeConfig
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-config-parse-fail:3:2:string"),
            RenderObjectSemanticKind::RuntimeConfigParseFail
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-config-noapply:3:2:string"),
            RenderObjectSemanticKind::RuntimeConfigNoApply
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-config-rollback:3:2:string"),
            RenderObjectSemanticKind::RuntimeConfigRollback
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-config-pending-mismatch:3:2:string"),
            RenderObjectSemanticKind::RuntimeConfigPendingMismatch
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("block:runtime-building:12:6:258"),
            RenderObjectSemanticKind::RuntimeBuilding
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("block:runtime-snapshot-head:12:6:258"),
            RenderObjectSemanticKind::RuntimeSnapshotHead
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("terrain:runtime-deconstruct:9:4"),
            RenderObjectSemanticKind::RuntimeDeconstruct
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-health:1:2"),
            RenderObjectSemanticKind::RuntimeHealth
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-effect:reliable:7:0x1:0x2:1"),
            RenderObjectSemanticKind::RuntimeEffect
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:runtime-break:0:8:9"),
            RenderObjectSemanticKind::RuntimeBreak
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("plan:runtime-place:0:8:9"),
            RenderObjectSemanticKind::RuntimePlace
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:1"),
            RenderObjectSemanticKind::Marker
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:line:77"),
            RenderObjectSemanticKind::MarkerLine
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:77:line-end"),
            RenderObjectSemanticKind::MarkerLineEnd
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:line:77:line-end"),
            RenderObjectSemanticKind::MarkerLineEnd
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
    fn semantic_kind_exposes_coarse_family_and_detail_labels() {
        assert_eq!(
            RenderObjectSemanticKind::MarkerLine.family(),
            RenderObjectSemanticFamily::Marker
        );
        assert_eq!(
            RenderObjectSemanticKind::RuntimeConfig.family(),
            RenderObjectSemanticFamily::Runtime
        );
        assert_eq!(
            RenderObjectSemanticKind::RuntimeConfig.detail_label(),
            Some("runtime-config")
        );
        assert_eq!(
            RenderObjectSemanticKind::RuntimeConfigRollback.detail_label(),
            Some("runtime-config-rollback")
        );
        assert_eq!(RenderObjectSemanticKind::Marker.detail_label(), None);
    }

    #[test]
    fn render_object_exposes_semantic_kind_and_family() {
        let marker = RenderObject {
            id: "marker:11".to_string(),
            layer: 30,
            x: 0.0,
            y: 0.0,
        };
        assert_eq!(marker.semantic_kind(), RenderObjectSemanticKind::Marker);
        assert_eq!(marker.semantic_family(), RenderObjectSemanticFamily::Marker);

        let line_end = RenderObject {
            id: "marker:line:11:line-end".to_string(),
            layer: 30,
            x: 0.0,
            y: 0.0,
        };
        assert_eq!(
            line_end.semantic_kind(),
            RenderObjectSemanticKind::MarkerLineEnd
        );
        assert_eq!(
            line_end.semantic_family(),
            RenderObjectSemanticFamily::Marker
        );

        let runtime_marker = RenderObject {
            id: "marker:runtime-health:1:2".to_string(),
            layer: 30,
            x: 0.0,
            y: 0.0,
        };
        assert_eq!(
            runtime_marker.semantic_kind(),
            RenderObjectSemanticKind::RuntimeHealth
        );
        assert_eq!(
            runtime_marker.semantic_family(),
            RenderObjectSemanticFamily::Runtime
        );
    }
}
