/// Render-facing projection of world state for UI drawing.
///
/// This crate intentionally avoids protocol parsing and transport concerns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderModel {
    pub viewport: Viewport,
    pub view_window: Option<RenderViewWindow>,
    pub objects: Vec<RenderObject>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderViewWindow {
    pub origin_x: usize,
    pub origin_y: usize,
    pub width: usize,
    pub height: usize,
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
    MarkerPoint,
    MarkerText,
    MarkerShape,
    MarkerShapeText,
    MarkerLine,
    MarkerLineEnd,
    MarkerTexture,
    MarkerQuad,
    MarkerUnknown,
    Plan,
    PlanBuild,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSemanticDetailCount {
    pub label: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderSemanticSummary {
    pub total_count: usize,
    pub player_count: usize,
    pub marker_count: usize,
    pub plan_count: usize,
    pub block_count: usize,
    pub runtime_count: usize,
    pub terrain_count: usize,
    pub unknown_count: usize,
    pub detail_counts: Vec<RenderSemanticDetailCount>,
}

impl RenderObject {
    pub fn semantic_kind(&self) -> RenderObjectSemanticKind {
        RenderObjectSemanticKind::from_id(&self.id)
    }

    pub fn semantic_family(&self) -> RenderObjectSemanticFamily {
        self.semantic_kind().family()
    }
}

impl RenderModel {
    pub fn player_focus_tile(&self, tile_size: f32) -> Option<(usize, usize)> {
        if !tile_size.is_finite() || tile_size <= 0.0 {
            return None;
        }

        self.objects
            .iter()
            .find(|object| object.semantic_kind() == RenderObjectSemanticKind::Player)
            .map(|object| {
                (
                    world_to_tile_index_floor(object.x, tile_size).max(0) as usize,
                    world_to_tile_index_floor(object.y, tile_size).max(0) as usize,
                )
            })
    }

    pub fn semantic_summary(&self) -> RenderSemanticSummary {
        let mut summary = RenderSemanticSummary::default();

        for object in &self.objects {
            summary.total_count += 1;
            match object.semantic_family() {
                RenderObjectSemanticFamily::Player => summary.player_count += 1,
                RenderObjectSemanticFamily::Marker => summary.marker_count += 1,
                RenderObjectSemanticFamily::Plan => summary.plan_count += 1,
                RenderObjectSemanticFamily::Block => summary.block_count += 1,
                RenderObjectSemanticFamily::Runtime => summary.runtime_count += 1,
                RenderObjectSemanticFamily::Terrain => summary.terrain_count += 1,
                RenderObjectSemanticFamily::Unknown => summary.unknown_count += 1,
            }

            let Some(label) = object.semantic_kind().detail_label() else {
                continue;
            };
            if let Some(existing) = summary
                .detail_counts
                .iter_mut()
                .find(|existing| existing.label == label)
            {
                existing.count += 1;
            } else {
                summary
                    .detail_counts
                    .push(RenderSemanticDetailCount { label, count: 1 });
            }
        }

        summary
            .detail_counts
            .sort_by(|left, right| left.label.cmp(right.label));
        summary
    }
}

impl RenderSemanticSummary {
    pub fn detail_text(&self) -> Option<String> {
        if self.detail_counts.is_empty() {
            return None;
        }

        Some(
            self.detail_counts
                .iter()
                .map(|detail| format!("{}:{}", detail.label, detail.count))
                .collect::<Vec<_>>()
                .join(","),
        )
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
            Self::Marker
            | Self::MarkerPoint
            | Self::MarkerText
            | Self::MarkerShape
            | Self::MarkerShapeText
            | Self::MarkerLine
            | Self::MarkerLineEnd
            | Self::MarkerTexture
            | Self::MarkerQuad
            | Self::MarkerUnknown => RenderObjectSemanticFamily::Marker,
            Self::Plan | Self::PlanBuild => RenderObjectSemanticFamily::Plan,
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
            Self::MarkerPoint => Some("marker-point"),
            Self::MarkerText => Some("marker-text"),
            Self::MarkerShape => Some("marker-shape"),
            Self::MarkerShapeText => Some("marker-shape-text"),
            Self::MarkerLine => Some("marker-line"),
            Self::MarkerLineEnd => Some("marker-line-end"),
            Self::MarkerTexture => Some("marker-texture"),
            Self::MarkerQuad => Some("marker-quad"),
            Self::MarkerUnknown => Some("marker-unknown"),
            Self::PlanBuild => Some("plan-build"),
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
        "point" => RenderObjectSemanticKind::MarkerPoint,
        "text" => RenderObjectSemanticKind::MarkerText,
        "shape" => RenderObjectSemanticKind::MarkerShape,
        "shape-text" => RenderObjectSemanticKind::MarkerShapeText,
        "line" => RenderObjectSemanticKind::MarkerLine,
        "texture" => RenderObjectSemanticKind::MarkerTexture,
        "quad" => RenderObjectSemanticKind::MarkerQuad,
        "unknown" => RenderObjectSemanticKind::MarkerUnknown,
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
        "build" => RenderObjectSemanticKind::PlanBuild,
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

fn world_to_tile_index_floor(world_position: f32, tile_size: f32) -> i32 {
    if !world_position.is_finite() {
        return 0;
    }
    (world_position / tile_size).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::{
        RenderModel, RenderObject, RenderObjectSemanticFamily, RenderObjectSemanticKind,
        RenderSemanticDetailCount, RenderSemanticSummary, RenderViewWindow, Viewport,
    };

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
            RenderObjectSemanticKind::from_id("marker:point:42"),
            RenderObjectSemanticKind::MarkerPoint
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:text:42"),
            RenderObjectSemanticKind::MarkerText
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:shape:42"),
            RenderObjectSemanticKind::MarkerShape
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:shape-text:42"),
            RenderObjectSemanticKind::MarkerShapeText
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
            RenderObjectSemanticKind::from_id("marker:texture:77"),
            RenderObjectSemanticKind::MarkerTexture
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:quad:77"),
            RenderObjectSemanticKind::MarkerQuad
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("marker:unknown:77"),
            RenderObjectSemanticKind::MarkerUnknown
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
            RenderObjectSemanticKind::from_id("plan:build:1:2:3:257"),
            RenderObjectSemanticKind::PlanBuild
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
            RenderObjectSemanticKind::MarkerText.family(),
            RenderObjectSemanticFamily::Marker
        );
        assert_eq!(
            RenderObjectSemanticKind::MarkerText.detail_label(),
            Some("marker-text")
        );
        assert_eq!(
            RenderObjectSemanticKind::RuntimeConfig.family(),
            RenderObjectSemanticFamily::Runtime
        );
        assert_eq!(
            RenderObjectSemanticKind::PlanBuild.family(),
            RenderObjectSemanticFamily::Plan
        );
        assert_eq!(
            RenderObjectSemanticKind::PlanBuild.detail_label(),
            Some("plan-build")
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

    #[test]
    fn render_model_tracks_projected_view_window_and_player_focus_tile() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(RenderViewWindow {
                origin_x: 2,
                origin_y: 3,
                width: 4,
                height: 5,
            }),
            objects: vec![RenderObject {
                id: "player:7".to_string(),
                layer: 40,
                x: 28.0,
                y: 33.0,
            }],
        };

        assert_eq!(
            scene.view_window,
            Some(RenderViewWindow {
                origin_x: 2,
                origin_y: 3,
                width: 4,
                height: 5,
            })
        );
        assert_eq!(scene.player_focus_tile(8.0), Some((3, 4)));
    }

    #[test]
    fn render_model_tracks_projected_view_window_and_unit_focus_tile_alias() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(RenderViewWindow {
                origin_x: 2,
                origin_y: 3,
                width: 4,
                height: 5,
            }),
            objects: vec![RenderObject {
                id: "unit:7".to_string(),
                layer: 40,
                x: 28.0,
                y: 33.0,
            }],
        };

        assert_eq!(scene.player_focus_tile(8.0), Some((3, 4)));
    }

    #[test]
    fn render_model_summarizes_semantic_families_and_detail_counts() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:7".to_string(),
                    layer: 40,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:line:77".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:line:77:line-end".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:text:18".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "marker:runtime-config:3:2:string".to_string(),
                    layer: 30,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "plan:build:1:2:3:257".to_string(),
                    layer: 20,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "block:runtime-building:12:6:258".to_string(),
                    layer: 10,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "terrain:8".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "unknown".to_string(),
                    layer: 0,
                    x: 0.0,
                    y: 0.0,
                },
            ],
        };

        let summary = scene.semantic_summary();

        assert_eq!(
            summary,
            RenderSemanticSummary {
                total_count: 9,
                player_count: 1,
                marker_count: 3,
                plan_count: 1,
                block_count: 0,
                runtime_count: 2,
                terrain_count: 1,
                unknown_count: 1,
                detail_counts: vec![
                    RenderSemanticDetailCount {
                        label: "marker-line",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "marker-line-end",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "marker-text",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "plan-build",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "runtime-building",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "runtime-config",
                        count: 1,
                    },
                ],
            }
        );
        assert_eq!(
            summary.detail_text().as_deref(),
            Some(
                "marker-line:1,marker-line-end:1,marker-text:1,plan-build:1,runtime-building:1,runtime-config:1"
            )
        );
    }

    #[test]
    fn render_model_counts_unit_alias_in_player_family_summary() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![RenderObject {
                id: "unit:7".to_string(),
                layer: 40,
                x: 0.0,
                y: 0.0,
            }],
        };

        let summary = scene.semantic_summary();
        assert_eq!(summary.total_count, 1);
        assert_eq!(summary.player_count, 1);
        assert_eq!(summary.unknown_count, 0);
    }
}
