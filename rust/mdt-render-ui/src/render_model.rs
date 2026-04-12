/// Render-facing projection of world state for UI drawing.
///
/// This crate intentionally avoids protocol parsing and transport concerns.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderModel {
    pub viewport: Viewport,
    pub view_window: Option<RenderViewWindow>,
    pub objects: Vec<RenderObject>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderPrimitive {
    Line {
        id: String,
        layer: i32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    Text {
        id: String,
        kind: RenderObjectSemanticKind,
        layer: i32,
        x: f32,
        y: f32,
        text: String,
    },
    Rect {
        id: String,
        family: String,
        layer: i32,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        line_ids: Vec<String>,
    },
    Icon {
        id: String,
        family: RenderIconPrimitiveFamily,
        variant: String,
        layer: i32,
        x: f32,
        y: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPrimitiveKind {
    Line,
    Text,
    Rect,
    Icon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPrimitivePayload {
    pub label: String,
    pub fields: BTreeMap<&'static str, RenderPrimitivePayloadValue>,
}

impl RenderPrimitivePayload {
    pub fn field(&self, name: &str) -> Option<&RenderPrimitivePayloadValue> {
        self.fields.get(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderPrimitivePayloadValue {
    Bool(bool),
    I16(i16),
    I32(i32),
    I32List(Vec<i32>),
    U8(u8),
    U8List(Vec<u8>),
    U32(u32),
    Usize(usize),
    Text(String),
    TextList(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderIconPrimitiveFamily {
    RuntimeEffect,
    RuntimeEffectMarker,
    RuntimeBuildConfig,
    RuntimeConfig,
    RuntimeConfigParseFail,
    RuntimeConfigNoApply,
    RuntimeConfigRollback,
    RuntimeConfigPendingMismatch,
    RuntimeHealth,
    RuntimeCommand,
    RuntimePlace,
    RuntimeUnitAssemblerProgress,
    RuntimeUnitAssemblerCommand,
    RuntimeBreak,
    RuntimeBullet,
    RuntimeLogicExplosion,
    RuntimeSoundAt,
    RuntimeTileAction,
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
    RuntimeUnit,
    RuntimeFire,
    RuntimePuddle,
    RuntimeWeather,
    RuntimeWorldLabel,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderPipelineLayerSummary {
    pub layer: i32,
    pub object_count: usize,
    pub player_count: usize,
    pub marker_count: usize,
    pub plan_count: usize,
    pub block_count: usize,
    pub runtime_count: usize,
    pub terrain_count: usize,
    pub unknown_count: usize,
    pub detail_counts: Vec<RenderSemanticDetailCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderPipelineSummary {
    pub total_object_count: usize,
    pub visible_object_count: usize,
    pub clipped_object_count: usize,
    pub visible_semantics: RenderSemanticSummary,
    pub focus_tile: Option<(usize, usize)>,
    pub window: Option<RenderViewWindow>,
    pub layer_span: Option<(i32, i32)>,
    pub layers: Vec<RenderPipelineLayerSummary>,
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

        let find_focus = |kind| {
            self.objects.iter().find_map(|object| {
                if object.semantic_kind() != kind || !object.x.is_finite() || !object.y.is_finite()
                {
                    return None;
                }

                Some((
                    world_to_tile_index_floor(object.x, tile_size).max(0) as usize,
                    world_to_tile_index_floor(object.y, tile_size).max(0) as usize,
                ))
            })
        };

        find_focus(RenderObjectSemanticKind::Player)
            .or_else(|| find_focus(RenderObjectSemanticKind::RuntimeUnit))
    }

    pub fn semantic_summary(&self) -> RenderSemanticSummary {
        let mut summary = RenderSemanticSummary::default();

        for object in &self.objects {
            accumulate_semantic_summary(&mut summary, object);
        }

        sort_detail_counts(&mut summary.detail_counts);
        summary
    }

    pub fn primitives(&self) -> Vec<RenderPrimitive> {
        let line_end_objects = self
            .objects
            .iter()
            .filter_map(render_line_end_object_pair)
            .collect::<BTreeMap<_, _>>();
        let rect_primitives = render_rect_primitives(&self.objects, &line_end_objects);
        let rect_line_ids = rect_primitives
            .iter()
            .filter_map(|primitive| match primitive {
                RenderPrimitive::Rect { line_ids, .. } => Some(line_ids.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect::<BTreeSet<_>>();

        let mut primitives = self
            .objects
            .iter()
            .filter_map(|object| {
                render_primitive_for_object(object, &line_end_objects, &rect_line_ids)
            })
            .collect::<Vec<_>>();
        primitives.extend(rect_primitives);
        primitives
    }

    pub fn pipeline_summary_for_window(
        &self,
        tile_size: f32,
        window: RenderViewWindow,
    ) -> RenderPipelineSummary {
        let mut visible_semantics = RenderSemanticSummary::default();
        let mut layers = BTreeMap::<i32, RenderPipelineLayerSummary>::new();
        let mut visible_object_count = 0usize;

        for object in &self.objects {
            if !object_visible_in_window(object, tile_size, window) {
                continue;
            }

            visible_object_count += 1;
            accumulate_semantic_summary(&mut visible_semantics, object);
            let layer = layers
                .entry(object.layer)
                .or_insert_with(|| RenderPipelineLayerSummary {
                    layer: object.layer,
                    ..RenderPipelineLayerSummary::default()
                });
            accumulate_pipeline_layer_summary(layer, object);
        }

        let mut layers = layers.into_values().collect::<Vec<_>>();
        for layer in &mut layers {
            sort_detail_counts(&mut layer.detail_counts);
        }
        sort_detail_counts(&mut visible_semantics.detail_counts);

        let layer_span = layers
            .first()
            .zip(layers.last())
            .map(|(first, last)| (first.layer, last.layer));

        RenderPipelineSummary {
            total_object_count: self.objects.len(),
            visible_object_count,
            clipped_object_count: self.objects.len().saturating_sub(visible_object_count),
            visible_semantics,
            focus_tile: self.player_focus_tile(tile_size),
            window: Some(window),
            layer_span,
            layers,
        }
    }
}

impl RenderPrimitive {
    pub fn kind(&self) -> RenderPrimitiveKind {
        match self {
            Self::Line { .. } => RenderPrimitiveKind::Line,
            Self::Text { .. } => RenderPrimitiveKind::Text,
            Self::Rect { .. } => RenderPrimitiveKind::Rect,
            Self::Icon { .. } => RenderPrimitiveKind::Icon,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Line { id, .. }
            | Self::Text { id, .. }
            | Self::Rect { id, .. }
            | Self::Icon { id, .. } => id,
        }
    }

    pub fn layer(&self) -> i32 {
        match self {
            Self::Line { layer, .. }
            | Self::Text { layer, .. }
            | Self::Rect { layer, .. }
            | Self::Icon { layer, .. } => *layer,
        }
    }

    pub fn payload(&self) -> Option<RenderPrimitivePayload> {
        match self {
            Self::Line { id, .. } => render_line_payload(id),
            Self::Text { kind, text, .. } => {
                let mut fields = BTreeMap::new();
                fields.insert("text", RenderPrimitivePayloadValue::Text(text.clone()));
                Some(RenderPrimitivePayload {
                    label: kind.detail_label().unwrap_or("render-text").to_string(),
                    fields,
                })
            }
            Self::Rect {
                id,
                family,
                line_ids,
                ..
            } => render_rect_payload(id, family, line_ids),
            Self::Icon { id, .. } => render_icon_payload(id).map(ParsedRenderIconPayload::finish),
        }
    }
}

impl RenderIconPrimitiveFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::RuntimeEffect => "runtime-effect-icon",
            Self::RuntimeEffectMarker => "runtime-effect",
            Self::RuntimeBuildConfig => "runtime-build-config-icon",
            Self::RuntimeConfig => "runtime-config",
            Self::RuntimeConfigParseFail => "runtime-config-parse-fail",
            Self::RuntimeConfigNoApply => "runtime-config-noapply",
            Self::RuntimeConfigRollback => "runtime-config-rollback",
            Self::RuntimeConfigPendingMismatch => "runtime-config-pending-mismatch",
            Self::RuntimeHealth => "runtime-health",
            Self::RuntimeCommand => "runtime-command",
            Self::RuntimePlace => "runtime-place",
            Self::RuntimeUnitAssemblerProgress => "runtime-unit-assembler-progress",
            Self::RuntimeUnitAssemblerCommand => "runtime-unit-assembler-command",
            Self::RuntimeBreak => "runtime-break",
            Self::RuntimeBullet => "runtime-bullet",
            Self::RuntimeLogicExplosion => "runtime-logic-explosion",
            Self::RuntimeSoundAt => "runtime-sound-at",
            Self::RuntimeTileAction => "runtime-tile-action",
        }
    }
}

fn render_primitive_for_object(
    object: &RenderObject,
    line_end_objects: &BTreeMap<String, &RenderObject>,
    rect_line_ids: &BTreeSet<String>,
) -> Option<RenderPrimitive> {
    match object.semantic_kind() {
        RenderObjectSemanticKind::MarkerLine => {
            if rect_line_ids.contains(&object.id) {
                return None;
            }
            let line_end = line_end_objects.get(&object.id)?;
            Some(RenderPrimitive::Line {
                id: object.id.clone(),
                layer: object.layer,
                x0: object.x,
                y0: object.y,
                x1: line_end.x,
                y1: line_end.y,
            })
        }
        RenderObjectSemanticKind::RuntimeWorldLabel
        | RenderObjectSemanticKind::MarkerText
        | RenderObjectSemanticKind::MarkerShapeText => render_text_primitive_for_object(object),
        _ => render_icon_primitive_for_object(object),
    }
}

fn render_line_end_object_pair(object: &RenderObject) -> Option<(String, &RenderObject)> {
    if object.semantic_kind() != RenderObjectSemanticKind::MarkerLineEnd {
        return None;
    }
    object
        .id
        .strip_suffix(":line-end")
        .map(|base_id| (base_id.to_string(), object))
}

fn render_text_primitive_for_object(object: &RenderObject) -> Option<RenderPrimitive> {
    let (_, encoded_text) = object.id.rsplit_once(":text:")?;
    let text = decode_render_text(encoded_text)?;
    if text.is_empty() {
        return None;
    }
    Some(RenderPrimitive::Text {
        id: object.id.clone(),
        kind: object.semantic_kind(),
        layer: object.layer,
        x: object.x,
        y: object.y,
        text,
    })
}

fn render_icon_primitive_for_object(object: &RenderObject) -> Option<RenderPrimitive> {
    let payload = render_icon_payload(&object.id)?;
    Some(RenderPrimitive::Icon {
        id: object.id.clone(),
        family: payload.family,
        variant: payload.variant,
        layer: object.layer,
        x: object.x,
        y: object.y,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRenderIconPayload {
    family: RenderIconPrimitiveFamily,
    variant: String,
    fields: BTreeMap<&'static str, RenderPrimitivePayloadValue>,
}

impl ParsedRenderIconPayload {
    fn new(family: RenderIconPrimitiveFamily, variant: impl Into<String>) -> Self {
        let variant = variant.into();
        let mut fields = BTreeMap::new();
        fields.insert(
            "variant",
            RenderPrimitivePayloadValue::Text(variant.clone()),
        );
        Self {
            family,
            variant,
            fields,
        }
    }

    fn with_field(mut self, name: &'static str, value: RenderPrimitivePayloadValue) -> Self {
        self.fields.insert(name, value);
        self
    }

    fn finish(self) -> RenderPrimitivePayload {
        RenderPrimitivePayload {
            label: self.family.label().to_string(),
            fields: self.fields,
        }
    }
}

fn render_icon_payload(id: &str) -> Option<ParsedRenderIconPayload> {
    if let Some(icon) = render_unit_assembler_icon_payload(id) {
        return Some(icon);
    }
    if let Some(icon) = render_runtime_world_event_icon_payload(id) {
        return Some(icon);
    }

    let segments = id.split(':').collect::<Vec<_>>();
    match segments.as_slice() {
        ["marker", config_kind, tile_x, tile_y, value]
            if !value.is_empty()
                && tile_x.parse::<i32>().is_ok()
                && tile_y.parse::<i32>().is_ok() =>
        {
            let family = match *config_kind {
                "runtime-config" => RenderIconPrimitiveFamily::RuntimeConfig,
                "runtime-config-parse-fail" => RenderIconPrimitiveFamily::RuntimeConfigParseFail,
                "runtime-config-noapply" => RenderIconPrimitiveFamily::RuntimeConfigNoApply,
                "runtime-config-rollback" => RenderIconPrimitiveFamily::RuntimeConfigRollback,
                "runtime-config-pending-mismatch" => {
                    RenderIconPrimitiveFamily::RuntimeConfigPendingMismatch
                }
                _ => return None,
            };
            Some(
                ParsedRenderIconPayload::new(family, *value)
                    .with_field(
                        "tile_x",
                        RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                    )
                    .with_field(
                        "tile_y",
                        RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                    ),
            )
        }
        ["marker", "runtime-health", tile_x, tile_y]
            if tile_x.parse::<i32>().is_ok() && tile_y.parse::<i32>().is_ok() =>
        {
            Some(
                ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeHealth, "health")
                    .with_field(
                        "tile_x",
                        RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                    )
                    .with_field(
                        "tile_y",
                        RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                    ),
            )
        }
        ["marker", "runtime-effect", delivery, effect_id, x_bits, y_bits, has_data]
            if matches!(*delivery, "normal" | "reliable")
                && effect_id.parse::<i16>().is_ok()
                && parse_prefixed_hex_u32(x_bits).is_some()
                && parse_prefixed_hex_u32(y_bits).is_some()
                && matches!(*has_data, "0" | "1") =>
        {
            Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeEffectMarker,
                    *delivery,
                )
                .with_field(
                    "delivery",
                    RenderPrimitivePayloadValue::Text((*delivery).to_string()),
                )
                .with_field(
                    "effect_id",
                    RenderPrimitivePayloadValue::I16(effect_id.parse().ok()?),
                )
                .with_field(
                    "x_bits",
                    RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(x_bits)?),
                )
                .with_field(
                    "y_bits",
                    RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(y_bits)?),
                )
                .with_field(
                    "has_data",
                    RenderPrimitivePayloadValue::Bool(*has_data == "1"),
                ),
            )
        }
        ["marker", "runtime-command-building", tile_x, tile_y]
            if tile_x.parse::<i32>().is_ok() && tile_y.parse::<i32>().is_ok() =>
        {
            Some(
                ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeCommand, "building")
                    .with_field(
                        "tile_x",
                        RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                    )
                    .with_field(
                        "tile_y",
                        RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                    ),
            )
        }
        ["plan", "runtime-place", index, tile_x, tile_y]
            if index.parse::<usize>().is_ok()
                && tile_x.parse::<i32>().is_ok()
                && tile_y.parse::<i32>().is_ok() =>
        {
            Some(
                ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimePlace, "place")
                    .with_field(
                        "index",
                        RenderPrimitivePayloadValue::Usize(index.parse().ok()?),
                    )
                    .with_field(
                        "tile_x",
                        RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                    )
                    .with_field(
                        "tile_y",
                        RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                    ),
            )
        }
        ["marker", "runtime-command-selected-unit", value] if value.parse::<i32>().is_ok() => Some(
            ParsedRenderIconPayload::new(
                RenderIconPrimitiveFamily::RuntimeCommand,
                "selected-unit",
            )
            .with_field(
                "unit_id",
                RenderPrimitivePayloadValue::I32(value.parse().ok()?),
            ),
        ),
        ["marker", "runtime-command-build-target", tile_x, tile_y]
            if tile_x.parse::<i32>().is_ok() && tile_y.parse::<i32>().is_ok() =>
        {
            Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeCommand,
                    "build-target",
                )
                .with_field(
                    "tile_x",
                    RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                )
                .with_field(
                    "tile_y",
                    RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                ),
            )
        }
        ["marker", "runtime-command-position-target", x_bits, y_bits]
            if parse_prefixed_hex_u32(x_bits).is_some()
                && parse_prefixed_hex_u32(y_bits).is_some() =>
        {
            Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeCommand,
                    "position-target",
                )
                .with_field(
                    "x_bits",
                    RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(x_bits)?),
                )
                .with_field(
                    "y_bits",
                    RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(y_bits)?),
                ),
            )
        }
        ["marker", "runtime-command-unit-target", kind, value]
            if kind.parse::<i16>().is_ok() && value.parse::<i32>().is_ok() =>
        {
            Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeCommand,
                    "unit-target",
                )
                .with_field("kind", RenderPrimitivePayloadValue::I16(kind.parse().ok()?))
                .with_field(
                    "value",
                    RenderPrimitivePayloadValue::I32(value.parse().ok()?),
                ),
            )
        }
        ["marker", "runtime-effect-icon", kind, delivery, effect_id, content_type, content_id, x_bits, y_bits]
            if !kind.is_empty()
                && matches!(*delivery, "normal" | "reliable")
                && effect_id.parse::<i16>().is_ok()
                && content_type.parse::<u8>().is_ok()
                && content_id.parse::<i16>().is_ok()
                && parse_prefixed_hex_u32(x_bits).is_some()
                && parse_prefixed_hex_u32(y_bits).is_some() =>
        {
            Some(
                ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeEffect, *kind)
                    .with_field(
                        "delivery",
                        RenderPrimitivePayloadValue::Text((*delivery).to_string()),
                    )
                    .with_field(
                        "effect_id",
                        RenderPrimitivePayloadValue::I16(effect_id.parse().ok()?),
                    )
                    .with_field(
                        "content_type",
                        RenderPrimitivePayloadValue::U8(content_type.parse().ok()?),
                    )
                    .with_field(
                        "content_id",
                        RenderPrimitivePayloadValue::I16(content_id.parse().ok()?),
                    )
                    .with_field(
                        "x_bits",
                        RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(x_bits)?),
                    )
                    .with_field(
                        "y_bits",
                        RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(y_bits)?),
                    ),
            )
        }
        ["marker", "runtime-build-config-icon", family, tile_x, tile_y, content_type, content_id]
            if !family.is_empty()
                && tile_x.parse::<i32>().is_ok()
                && tile_y.parse::<i32>().is_ok()
                && content_type.parse::<u8>().is_ok()
                && content_id.parse::<i16>().is_ok() =>
        {
            Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeBuildConfig,
                    *family,
                )
                .with_field(
                    "tile_x",
                    RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                )
                .with_field(
                    "tile_y",
                    RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                )
                .with_field(
                    "content_type",
                    RenderPrimitivePayloadValue::U8(content_type.parse().ok()?),
                )
                .with_field(
                    "content_id",
                    RenderPrimitivePayloadValue::I16(content_id.parse().ok()?),
                ),
            )
        }
        _ => None,
    }
}

fn render_unit_assembler_icon_payload(id: &str) -> Option<ParsedRenderIconPayload> {
    if let Some(rest) = id.strip_prefix("marker:runtime-unit-assembler-progress:") {
        let parts = rest.split(':').collect::<Vec<_>>();
        if parts.len() < 9 {
            return None;
        }
        let block_name = parts[0];
        let tile_x = parts[1];
        let tile_y = parts[2];
        let progress_bits = parts[3];
        let unit_count = parts[4];
        let block_count = parts[5];
        let payload_present = parts[parts.len() - 2];
        let pay_rotation_bits = parts[parts.len() - 1];
        let sample = &parts[6..parts.len() - 2];
        let sample_valid = matches!(sample, ["none"])
            || matches!(sample, [kind, id] if !kind.is_empty() && id.parse::<i16>().is_ok());
        if !block_name.is_empty()
            && tile_x.parse::<i32>().is_ok()
            && tile_y.parse::<i32>().is_ok()
            && parse_prefixed_hex_u32(progress_bits).is_some()
            && unit_count.parse::<usize>().is_ok()
            && block_count.parse::<usize>().is_ok()
            && matches!(payload_present, "0" | "1")
            && parse_prefixed_hex_u32(pay_rotation_bits).is_some()
            && sample_valid
        {
            let mut payload = ParsedRenderIconPayload::new(
                RenderIconPrimitiveFamily::RuntimeUnitAssemblerProgress,
                block_name,
            )
            .with_field(
                "tile_x",
                RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
            )
            .with_field(
                "tile_y",
                RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
            )
            .with_field(
                "progress_bits",
                RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(progress_bits)?),
            )
            .with_field(
                "unit_count",
                RenderPrimitivePayloadValue::Usize(unit_count.parse().ok()?),
            )
            .with_field(
                "block_count",
                RenderPrimitivePayloadValue::Usize(block_count.parse().ok()?),
            )
            .with_field(
                "payload_present",
                RenderPrimitivePayloadValue::Bool(payload_present == "1"),
            )
            .with_field(
                "pay_rotation_bits",
                RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(pay_rotation_bits)?),
            )
            .with_field(
                "sample_present",
                RenderPrimitivePayloadValue::Bool(!matches!(sample, ["none"])),
            );
            if let [kind, id] = sample {
                payload = payload
                    .with_field(
                        "sample_kind",
                        RenderPrimitivePayloadValue::Text((*kind).to_string()),
                    )
                    .with_field(
                        "sample_id",
                        RenderPrimitivePayloadValue::I16(id.parse().ok()?),
                    );
            }
            return Some(payload);
        }
        return None;
    }

    let rest = id.strip_prefix("marker:runtime-unit-assembler-command:")?;
    let parts = rest.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [block_name, tile_x, tile_y, x_bits, y_bits]
            if !block_name.is_empty()
                && tile_x.parse::<i32>().is_ok()
                && tile_y.parse::<i32>().is_ok()
                && parse_prefixed_hex_u32(x_bits).is_some()
                && parse_prefixed_hex_u32(y_bits).is_some() =>
        {
            Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeUnitAssemblerCommand,
                    *block_name,
                )
                .with_field(
                    "tile_x",
                    RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                )
                .with_field(
                    "tile_y",
                    RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                )
                .with_field(
                    "x_bits",
                    RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(x_bits)?),
                )
                .with_field(
                    "y_bits",
                    RenderPrimitivePayloadValue::U32(parse_prefixed_hex_u32(y_bits)?),
                ),
            )
        }
        _ => None,
    }
}

fn render_runtime_world_event_icon_payload(id: &str) -> Option<ParsedRenderIconPayload> {
    if let Some(rest) = id.strip_prefix("marker:runtime-break:") {
        return Some(
            ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeBreak, "break")
                .with_field(
                    "values",
                    RenderPrimitivePayloadValue::I32List(parse_runtime_icon_i32_values(rest, 3)?),
                ),
        );
    }
    if let Some(rest) = id.strip_prefix("marker:runtime-bullet:") {
        return Some(
            ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeBullet, "bullet")
                .with_field(
                    "values",
                    RenderPrimitivePayloadValue::I32List(parse_runtime_icon_i32_values(rest, 3)?),
                ),
        );
    }
    if let Some(rest) = id.strip_prefix("marker:runtime-sound-at:") {
        return Some(
            ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeSoundAt, "sound-at")
                .with_field(
                    "values",
                    RenderPrimitivePayloadValue::I32List(parse_runtime_icon_i32_values(rest, 2)?),
                ),
        );
    }
    if let Some(rest) = id.strip_prefix("marker:runtime-logic-explosion:") {
        let mut parts = rest.split(':');
        let tile_x = parts.next()?.parse::<i32>().ok()?;
        let tile_y = parts.next()?.parse::<i32>().ok()?;
        let radius_bits = parse_prefixed_hex_u32(parts.next()?)?;
        let mut flags = Vec::with_capacity(4);
        for _ in 0..4 {
            flags.push(parts.next()?.parse::<u8>().ok()?);
        }
        if parts.next().is_none() {
            return Some(
                ParsedRenderIconPayload::new(
                    RenderIconPrimitiveFamily::RuntimeLogicExplosion,
                    "logic-explosion",
                )
                .with_field("tile_x", RenderPrimitivePayloadValue::I32(tile_x))
                .with_field("tile_y", RenderPrimitivePayloadValue::I32(tile_y))
                .with_field("radius_bits", RenderPrimitivePayloadValue::U32(radius_bits))
                .with_field("flags", RenderPrimitivePayloadValue::U8List(flags)),
            );
        }
    }
    render_runtime_tile_action_icon_payload(id)
}

fn render_runtime_tile_action_icon_payload(id: &str) -> Option<ParsedRenderIconPayload> {
    for (prefix, field_count, variant) in [
        (
            "marker:runtime-unit-block-spawn:",
            3usize,
            "unit-block-spawn",
        ),
        (
            "marker:runtime-unit-tether-block-spawned:",
            4usize,
            "unit-tether-block-spawned",
        ),
        (
            "marker:runtime-auto-door-toggle:",
            4usize,
            "auto-door-toggle",
        ),
        (
            "marker:runtime-landing-pad-landed:",
            3usize,
            "landing-pad-landed",
        ),
        (
            "marker:runtime-assembler-drone-spawned:",
            4usize,
            "assembler-drone-spawned",
        ),
        (
            "marker:runtime-assembler-unit-spawned:",
            3usize,
            "assembler-unit-spawned",
        ),
    ] {
        if let Some(rest) = id.strip_prefix(prefix) {
            let values = parse_runtime_icon_i32_values(rest, field_count)?;
            let mut payload =
                ParsedRenderIconPayload::new(RenderIconPrimitiveFamily::RuntimeTileAction, variant)
                    .with_field("overlay_key", RenderPrimitivePayloadValue::I32(values[0]))
                    .with_field("tile_x", RenderPrimitivePayloadValue::I32(values[1]))
                    .with_field("tile_y", RenderPrimitivePayloadValue::I32(values[2]));
            if let Some(value) = values.get(3).copied() {
                payload = match variant {
                    "auto-door-toggle" => {
                        payload.with_field("open", RenderPrimitivePayloadValue::Bool(value != 0))
                    }
                    "unit-tether-block-spawned" | "assembler-drone-spawned" => {
                        payload.with_field("unit_id", RenderPrimitivePayloadValue::I32(value))
                    }
                    _ => payload,
                };
            }
            return Some(payload);
        }
    }
    None
}

fn parse_runtime_icon_i32_values(rest: &str, field_count: usize) -> Option<Vec<i32>> {
    let values = rest
        .split(':')
        .map(|part| part.parse::<i32>().ok())
        .collect::<Option<Vec<_>>>()?;
    (values.len() == field_count).then_some(values)
}

fn parse_prefixed_hex_u32(text: &str) -> Option<u32> {
    u32::from_str_radix(text.strip_prefix("0x")?, 16).ok()
}

#[derive(Debug, Clone, PartialEq)]
struct RectPrimitiveCandidate {
    family: String,
    id_prefix: String,
    layer: i32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    line_ids: Vec<String>,
    edges: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RectPrimitiveLineDescriptor {
    family: String,
    id_prefix: String,
    edge: String,
}

fn render_rect_primitives(
    objects: &[RenderObject],
    line_end_objects: &BTreeMap<String, &RenderObject>,
) -> Vec<RenderPrimitive> {
    let mut candidates = BTreeMap::<(String, i32), RectPrimitiveCandidate>::new();

    for object in objects {
        if object.semantic_kind() != RenderObjectSemanticKind::MarkerLine {
            continue;
        }
        let Some(line_end) = line_end_objects.get(&object.id) else {
            continue;
        };
        let Some(descriptor) = render_rect_descriptor(&object.id) else {
            continue;
        };
        let left = object.x.min(line_end.x);
        let top = object.y.min(line_end.y);
        let right = object.x.max(line_end.x);
        let bottom = object.y.max(line_end.y);
        let key = (descriptor.id_prefix.clone(), object.layer);
        let candidate = candidates
            .entry(key)
            .or_insert_with(|| RectPrimitiveCandidate {
                family: descriptor.family.clone(),
                id_prefix: descriptor.id_prefix.clone(),
                layer: object.layer,
                left,
                top,
                right,
                bottom,
                line_ids: Vec::new(),
                edges: BTreeSet::new(),
            });
        candidate.left = candidate.left.min(left);
        candidate.top = candidate.top.min(top);
        candidate.right = candidate.right.max(right);
        candidate.bottom = candidate.bottom.max(bottom);
        candidate.line_ids.push(object.id.clone());
        candidate.edges.insert(descriptor.edge);
    }

    candidates
        .into_values()
        .filter(|candidate| {
            candidate.edges.len() == 4
                && candidate.edges.contains("top")
                && candidate.edges.contains("right")
                && candidate.edges.contains("bottom")
                && candidate.edges.contains("left")
                && candidate.line_ids.len() == 4
        })
        .map(|mut candidate| {
            candidate.line_ids.sort();
            RenderPrimitive::Rect {
                id: format!(
                    "marker:rect:{}:{}:{}:{}:{}",
                    candidate.id_prefix,
                    candidate.left.to_bits(),
                    candidate.top.to_bits(),
                    candidate.right.to_bits(),
                    candidate.bottom.to_bits()
                ),
                family: candidate.family,
                layer: candidate.layer,
                left: candidate.left,
                top: candidate.top,
                right: candidate.right,
                bottom: candidate.bottom,
                line_ids: candidate.line_ids,
            }
        })
        .collect()
}

fn render_rect_descriptor(id: &str) -> Option<RectPrimitiveLineDescriptor> {
    let mut parts = id.strip_prefix("marker:line:")?.split(':');
    let family = parts.next()?;
    match family {
        "runtime-command-rect" | "runtime-command-target-rect" | "runtime-break-rect" => {
            let edge = parts.next()?;
            matches!(edge, "top" | "right" | "bottom" | "left").then(|| {
                RectPrimitiveLineDescriptor {
                    family: family.to_string(),
                    id_prefix: family.to_string(),
                    edge: edge.to_string(),
                }
            })
        }
        "runtime-unit-assembler-area" => {
            let block_name = parts.next()?;
            let tile_x = parts.next()?;
            let tile_y = parts.next()?;
            let edge = parts.next()?;
            tile_x.parse::<i32>().ok()?;
            tile_y.parse::<i32>().ok()?;
            matches!(edge, "top" | "right" | "bottom" | "left").then(|| {
                RectPrimitiveLineDescriptor {
                    family: "runtime-unit-assembler-area".to_string(),
                    id_prefix: format!(
                        "runtime-unit-assembler-area:{block_name}:{tile_x}:{tile_y}"
                    ),
                    edge: edge.to_string(),
                }
            })
        }
        _ => None,
    }
}

fn render_line_payload(id: &str) -> Option<RenderPrimitivePayload> {
    if let Some(descriptor) = render_rect_descriptor(id) {
        let mut fields = BTreeMap::new();
        fields.insert("edge", RenderPrimitivePayloadValue::Text(descriptor.edge));
        if let Some(rest) = id.strip_prefix("marker:line:runtime-unit-assembler-area:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if let [block_name, tile_x, tile_y, _, ..] = parts.as_slice() {
                fields.insert(
                    "block_name",
                    RenderPrimitivePayloadValue::Text((*block_name).to_string()),
                );
                fields.insert(
                    "tile_x",
                    RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
                );
                fields.insert(
                    "tile_y",
                    RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
                );
            }
        }
        return Some(RenderPrimitivePayload {
            label: descriptor.family,
            fields,
        });
    }

    id.strip_prefix("marker:line:").map(|marker_id| {
        let mut fields = BTreeMap::new();
        fields.insert(
            "marker_id",
            RenderPrimitivePayloadValue::Text(marker_id.to_string()),
        );
        RenderPrimitivePayload {
            label: "marker-line".to_string(),
            fields,
        }
    })
}

fn render_rect_payload(
    id: &str,
    family: &str,
    line_ids: &[String],
) -> Option<RenderPrimitivePayload> {
    let mut fields = BTreeMap::new();
    fields.insert(
        "line_ids",
        RenderPrimitivePayloadValue::TextList(line_ids.to_vec()),
    );
    if let Some(rest) = id.strip_prefix("marker:rect:runtime-unit-assembler-area:") {
        let parts = rest.split(':').collect::<Vec<_>>();
        if let [block_name, tile_x, tile_y, left, top, right, bottom] = parts.as_slice() {
            parse_prefixed_or_decimal_u32_bits(left)?;
            parse_prefixed_or_decimal_u32_bits(top)?;
            parse_prefixed_or_decimal_u32_bits(right)?;
            parse_prefixed_or_decimal_u32_bits(bottom)?;
            fields.insert(
                "block_name",
                RenderPrimitivePayloadValue::Text((*block_name).to_string()),
            );
            fields.insert(
                "tile_x",
                RenderPrimitivePayloadValue::I32(tile_x.parse().ok()?),
            );
            fields.insert(
                "tile_y",
                RenderPrimitivePayloadValue::I32(tile_y.parse().ok()?),
            );
        } else {
            return None;
        }
    }
    Some(RenderPrimitivePayload {
        label: family.to_string(),
        fields,
    })
}

fn parse_prefixed_or_decimal_u32_bits(text: &str) -> Option<u32> {
    parse_prefixed_hex_u32(text).or_else(|| text.parse::<u32>().ok())
}

pub(crate) fn encode_render_text(text: &str) -> String {
    text.as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_render_text(encoded_text: &str) -> Option<String> {
    if encoded_text.is_empty() || encoded_text.len() % 2 != 0 {
        return None;
    }

    let bytes = encoded_text
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect::<Option<Vec<_>>>()?;

    String::from_utf8(bytes).ok()
}

impl RenderSemanticSummary {
    pub fn family_text(&self) -> String {
        format!(
            "players={} markers={} plans={} blocks={} runtime={} terrain={} unknown={}",
            self.player_count,
            self.marker_count,
            self.plan_count,
            self.block_count,
            self.runtime_count,
            self.terrain_count,
            self.unknown_count,
        )
    }

    pub fn family_and_detail_text(&self) -> String {
        let mut text = self.family_text();
        if let Some(detail_text) = self.detail_text() {
            text.push_str(" detail=");
            text.push_str(&detail_text);
        }
        text
    }

    pub fn detail_text(&self) -> Option<String> {
        detail_counts_text(&self.detail_counts)
    }
}

impl RenderPipelineLayerSummary {
    pub fn family_text(&self) -> String {
        format!(
            "players={} markers={} plans={} blocks={} runtime={} terrain={} unknown={}",
            self.player_count,
            self.marker_count,
            self.plan_count,
            self.block_count,
            self.runtime_count,
            self.terrain_count,
            self.unknown_count,
        )
    }

    pub fn family_and_detail_text(&self) -> String {
        let mut text = self.family_text();
        if let Some(detail_text) = self.detail_text() {
            text.push_str(" detail=");
            text.push_str(&detail_text);
        }
        text
    }

    pub fn detail_text(&self) -> Option<String> {
        detail_counts_text(&self.detail_counts)
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
            "player" => Self::Player,
            "unit" => Self::RuntimeUnit,
            "fire" => Self::RuntimeFire,
            "puddle" => Self::RuntimePuddle,
            "weather" => Self::RuntimeWeather,
            "world-label" => Self::RuntimeWorldLabel,
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
            Self::RuntimeUnit
            | Self::RuntimeFire
            | Self::RuntimePuddle
            | Self::RuntimeWeather
            | Self::RuntimeWorldLabel
            | Self::RuntimeBuilding
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
            Self::RuntimeUnit => Some("runtime-unit"),
            Self::RuntimeFire => Some("runtime-fire"),
            Self::RuntimePuddle => Some("runtime-puddle"),
            Self::RuntimeWeather => Some("runtime-weather"),
            Self::RuntimeWorldLabel => Some("runtime-world-label"),
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
    if !world_position.is_finite() || !tile_size.is_finite() || tile_size <= 0.0 {
        return 0;
    }
    (world_position / tile_size).floor() as i32
}

fn accumulate_semantic_summary(summary: &mut RenderSemanticSummary, object: &RenderObject) {
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
    increment_detail_count(
        &mut summary.detail_counts,
        object.semantic_kind().detail_label(),
    );
}

fn accumulate_pipeline_layer_summary(
    summary: &mut RenderPipelineLayerSummary,
    object: &RenderObject,
) {
    summary.object_count += 1;
    match object.semantic_family() {
        RenderObjectSemanticFamily::Player => summary.player_count += 1,
        RenderObjectSemanticFamily::Marker => summary.marker_count += 1,
        RenderObjectSemanticFamily::Plan => summary.plan_count += 1,
        RenderObjectSemanticFamily::Block => summary.block_count += 1,
        RenderObjectSemanticFamily::Runtime => summary.runtime_count += 1,
        RenderObjectSemanticFamily::Terrain => summary.terrain_count += 1,
        RenderObjectSemanticFamily::Unknown => summary.unknown_count += 1,
    }
    increment_detail_count(
        &mut summary.detail_counts,
        object.semantic_kind().detail_label(),
    );
}

fn increment_detail_count(
    detail_counts: &mut Vec<RenderSemanticDetailCount>,
    label: Option<&'static str>,
) {
    let Some(label) = label else {
        return;
    };

    if let Some(existing) = detail_counts
        .iter_mut()
        .find(|existing| existing.label == label)
    {
        existing.count += 1;
    } else {
        detail_counts.push(RenderSemanticDetailCount { label, count: 1 });
    }
}

fn sort_detail_counts(detail_counts: &mut [RenderSemanticDetailCount]) {
    detail_counts.sort_by(|left, right| left.label.cmp(right.label));
}

fn detail_counts_text(detail_counts: &[RenderSemanticDetailCount]) -> Option<String> {
    if detail_counts.is_empty() {
        return None;
    }

    Some(
        detail_counts
            .iter()
            .map(|detail| format!("{}:{}", detail.label, detail.count))
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn object_visible_in_window(
    object: &RenderObject,
    tile_size: f32,
    window: RenderViewWindow,
) -> bool {
    if !tile_size.is_finite() || tile_size <= 0.0 {
        return false;
    }

    let tile_x = world_to_tile_index_floor(object.x, tile_size);
    let tile_y = world_to_tile_index_floor(object.y, tile_size);
    if tile_x < 0 || tile_y < 0 {
        return false;
    }

    let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
    tile_x >= window.origin_x
        && tile_y >= window.origin_y
        && tile_x < window.origin_x.saturating_add(window.width)
        && tile_y < window.origin_y.saturating_add(window.height)
}

#[cfg(test)]
mod tests {
    use super::{
        RenderIconPrimitiveFamily, RenderModel, RenderObject, RenderObjectSemanticFamily,
        RenderObjectSemanticKind, RenderPipelineLayerSummary, RenderPipelineSummary,
        RenderPrimitive, RenderPrimitiveKind, RenderPrimitivePayloadValue,
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
            RenderObjectSemanticKind::RuntimeUnit
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("fire:7"),
            RenderObjectSemanticKind::RuntimeFire
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("puddle:7"),
            RenderObjectSemanticKind::RuntimePuddle
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("weather:7"),
            RenderObjectSemanticKind::RuntimeWeather
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("world-label:7"),
            RenderObjectSemanticKind::RuntimeWorldLabel
        );
        assert_eq!(
            RenderObjectSemanticKind::from_id("world-label:7:text:72756e74696d65"),
            RenderObjectSemanticKind::RuntimeWorldLabel
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
            RenderObjectSemanticKind::RuntimeUnit.family(),
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
        assert_eq!(
            RenderObjectSemanticKind::RuntimeWorldLabel.detail_label(),
            Some("runtime-world-label")
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

        let runtime_unit = RenderObject {
            id: "unit:11".to_string(),
            layer: 40,
            x: 0.0,
            y: 0.0,
        };
        assert_eq!(
            runtime_unit.semantic_kind(),
            RenderObjectSemanticKind::RuntimeUnit
        );
        assert_eq!(
            runtime_unit.semantic_family(),
            RenderObjectSemanticFamily::Runtime
        );
    }

    #[test]
    fn render_primitives_expose_a_stable_kind() {
        assert_eq!(
            RenderPrimitive::Line {
                id: "marker:line:1".to_string(),
                layer: 1,
                x0: 0.0,
                y0: 0.0,
                x1: 1.0,
                y1: 1.0,
            }
            .kind(),
            RenderPrimitiveKind::Line
        );
        assert_eq!(
            RenderPrimitive::Text {
                id: "marker:text:1:text:61".to_string(),
                kind: RenderObjectSemanticKind::MarkerText,
                layer: 1,
                x: 0.0,
                y: 0.0,
                text: "a".to_string(),
            }
            .kind(),
            RenderPrimitiveKind::Text
        );
        assert_eq!(
            RenderPrimitive::Rect {
                id: "marker:rect:1".to_string(),
                family: "runtime-command-rect".to_string(),
                layer: 1,
                left: 0.0,
                top: 0.0,
                right: 1.0,
                bottom: 1.0,
                line_ids: vec!["marker:line:1".to_string()],
            }
            .kind(),
            RenderPrimitiveKind::Rect
        );
        assert_eq!(
            RenderPrimitive::Icon {
                id: "marker:runtime-health:1:2".to_string(),
                family: RenderIconPrimitiveFamily::RuntimeHealth,
                variant: "health".to_string(),
                layer: 1,
                x: 0.0,
                y: 0.0,
            }
            .kind(),
            RenderPrimitiveKind::Icon
        );
    }

    #[test]
    fn render_primitives_expose_structured_payload_fields() {
        let line_payload = RenderPrimitive::Line {
            id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top".to_string(),
            layer: 1,
            x0: 0.0,
            y0: 0.0,
            x1: 8.0,
            y1: 0.0,
        }
        .payload()
        .expect("line payload");
        assert_eq!(line_payload.label, "runtime-unit-assembler-area");
        assert_eq!(
            line_payload.field("edge"),
            Some(&RenderPrimitivePayloadValue::Text("top".to_string()))
        );
        assert_eq!(
            line_payload.field("block_name"),
            Some(&RenderPrimitivePayloadValue::Text(
                "tank-assembler".to_string()
            ))
        );
        assert_eq!(
            line_payload.field("tile_x"),
            Some(&RenderPrimitivePayloadValue::I32(30))
        );
        assert_eq!(
            line_payload.field("tile_y"),
            Some(&RenderPrimitivePayloadValue::I32(40))
        );

        let text_payload = RenderPrimitive::Text {
            id: "marker:text:1:text:61".to_string(),
            kind: RenderObjectSemanticKind::MarkerText,
            layer: 1,
            x: 0.0,
            y: 0.0,
            text: "a".to_string(),
        }
        .payload()
        .expect("text payload");
        assert_eq!(text_payload.label, "marker-text");
        assert_eq!(
            text_payload.field("text"),
            Some(&RenderPrimitivePayloadValue::Text("a".to_string()))
        );

        let rect_payload = RenderPrimitive::Rect {
            id: "marker:rect:runtime-unit-assembler-area:tank-assembler:30:40:1065353216:1073741824:1077936128:1082130432".to_string(),
            family: "runtime-unit-assembler-area".to_string(),
            layer: 1,
            left: 1.0,
            top: 2.0,
            right: 3.0,
            bottom: 4.0,
            line_ids: vec![
                "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top".to_string(),
                "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:right".to_string(),
            ],
        }
        .payload()
        .expect("rect payload");
        assert_eq!(rect_payload.label, "runtime-unit-assembler-area");
        assert_eq!(
            rect_payload.field("block_name"),
            Some(&RenderPrimitivePayloadValue::Text(
                "tank-assembler".to_string()
            ))
        );
        assert_eq!(
            rect_payload.field("tile_x"),
            Some(&RenderPrimitivePayloadValue::I32(30))
        );
        assert_eq!(
            rect_payload.field("tile_y"),
            Some(&RenderPrimitivePayloadValue::I32(40))
        );
        assert_eq!(
            rect_payload.field("line_ids"),
            Some(&RenderPrimitivePayloadValue::TextList(vec![
                "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top".to_string(),
                "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:right".to_string(),
            ]))
        );

        let icon_payload = RenderPrimitive::Icon {
            id: "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:1:0x40800000".to_string(),
            family: RenderIconPrimitiveFamily::RuntimeUnitAssemblerProgress,
            variant: "tank-assembler".to_string(),
            layer: 1,
            x: 0.0,
            y: 0.0,
        }
        .payload()
        .expect("icon payload");
        assert_eq!(icon_payload.label, "runtime-unit-assembler-progress");
        assert_eq!(
            icon_payload.field("variant"),
            Some(&RenderPrimitivePayloadValue::Text(
                "tank-assembler".to_string()
            ))
        );
        assert_eq!(
            icon_payload.field("tile_x"),
            Some(&RenderPrimitivePayloadValue::I32(30))
        );
        assert_eq!(
            icon_payload.field("tile_y"),
            Some(&RenderPrimitivePayloadValue::I32(40))
        );
        assert_eq!(
            icon_payload.field("progress_bits"),
            Some(&RenderPrimitivePayloadValue::U32(0x3f400000))
        );
        assert_eq!(
            icon_payload.field("unit_count"),
            Some(&RenderPrimitivePayloadValue::Usize(2))
        );
        assert_eq!(
            icon_payload.field("block_count"),
            Some(&RenderPrimitivePayloadValue::Usize(4))
        );
        assert_eq!(
            icon_payload.field("sample_present"),
            Some(&RenderPrimitivePayloadValue::Bool(true))
        );
        assert_eq!(
            icon_payload.field("sample_kind"),
            Some(&RenderPrimitivePayloadValue::Text("b".to_string()))
        );
        assert_eq!(
            icon_payload.field("sample_id"),
            Some(&RenderPrimitivePayloadValue::I16(9))
        );
        assert_eq!(
            icon_payload.field("payload_present"),
            Some(&RenderPrimitivePayloadValue::Bool(true))
        );
        assert_eq!(
            icon_payload.field("pay_rotation_bits"),
            Some(&RenderPrimitivePayloadValue::U32(0x40800000))
        );
    }

    #[test]
    fn runtime_event_icon_payloads_use_named_fields() {
        let payload = RenderPrimitive::Icon {
            id: "marker:runtime-auto-door-toggle:4:3:4:1".to_string(),
            family: RenderIconPrimitiveFamily::RuntimeTileAction,
            variant: "auto-door-toggle".to_string(),
            layer: 1,
            x: 0.0,
            y: 0.0,
        }
        .payload()
        .expect("runtime tile action payload");

        assert_eq!(payload.label, "runtime-tile-action");
        assert_eq!(
            payload.field("variant"),
            Some(&RenderPrimitivePayloadValue::Text(
                "auto-door-toggle".to_string()
            ))
        );
        assert_eq!(
            payload.field("overlay_key"),
            Some(&RenderPrimitivePayloadValue::I32(4))
        );
        assert_eq!(
            payload.field("tile_x"),
            Some(&RenderPrimitivePayloadValue::I32(3))
        );
        assert_eq!(
            payload.field("tile_y"),
            Some(&RenderPrimitivePayloadValue::I32(4))
        );
        assert_eq!(
            payload.field("open"),
            Some(&RenderPrimitivePayloadValue::Bool(true))
        );

        let tether_payload = RenderPrimitive::Icon {
            id: "marker:runtime-unit-tether-block-spawned:5:6:7:44".to_string(),
            family: RenderIconPrimitiveFamily::RuntimeTileAction,
            variant: "unit-tether-block-spawned".to_string(),
            layer: 1,
            x: 0.0,
            y: 0.0,
        }
        .payload()
        .expect("runtime tether payload");
        assert_eq!(
            tether_payload.field("overlay_key"),
            Some(&RenderPrimitivePayloadValue::I32(5))
        );
        assert_eq!(
            tether_payload.field("tile_x"),
            Some(&RenderPrimitivePayloadValue::I32(6))
        );
        assert_eq!(
            tether_payload.field("tile_y"),
            Some(&RenderPrimitivePayloadValue::I32(7))
        );
        assert_eq!(
            tether_payload.field("unit_id"),
            Some(&RenderPrimitivePayloadValue::I32(44))
        );
        assert_eq!(tether_payload.field("open"), None);

        let block_spawn_payload = RenderPrimitive::Icon {
            id: "marker:runtime-unit-block-spawn:8:9:10".to_string(),
            family: RenderIconPrimitiveFamily::RuntimeTileAction,
            variant: "unit-block-spawn".to_string(),
            layer: 1,
            x: 0.0,
            y: 0.0,
        }
        .payload()
        .expect("runtime block spawn payload");
        assert_eq!(
            block_spawn_payload.field("overlay_key"),
            Some(&RenderPrimitivePayloadValue::I32(8))
        );
        assert_eq!(
            block_spawn_payload.field("tile_x"),
            Some(&RenderPrimitivePayloadValue::I32(9))
        );
        assert_eq!(
            block_spawn_payload.field("tile_y"),
            Some(&RenderPrimitivePayloadValue::I32(10))
        );
        assert_eq!(block_spawn_payload.field("unit_id"), None);
        assert_eq!(block_spawn_payload.field("open"), None);
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
    fn render_model_tracks_projected_view_window_and_runtime_unit_focus_tile() {
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
            objects: vec![
                RenderObject {
                    id: "unit:7".to_string(),
                    layer: 40,
                    x: 28.0,
                    y: 33.0,
                },
                RenderObject {
                    id: "player:7".to_string(),
                    layer: 41,
                    x: 40.0,
                    y: 48.0,
                },
            ],
        };

        assert_eq!(scene.player_focus_tile(8.0), Some((5, 6)));
    }

    #[test]
    fn render_model_fails_closed_for_invalid_tile_size_in_window_visibility() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:7".to_string(),
                    layer: 40,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:point:1".to_string(),
                    layer: 20,
                    x: 16.0,
                    y: 16.0,
                },
            ],
        };

        let summary = scene.pipeline_summary_for_window(
            0.0,
            RenderViewWindow {
                origin_x: 0,
                origin_y: 0,
                width: 16,
                height: 16,
            },
        );

        assert_eq!(summary.visible_object_count, 0);
        assert_eq!(summary.clipped_object_count, 2);
        assert_eq!(summary.focus_tile, None);
    }

    #[test]
    fn render_model_skips_non_finite_player_focus_candidates() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "player:7".to_string(),
                    layer: 40,
                    x: f32::NAN,
                    y: 32.0,
                },
                RenderObject {
                    id: "unit:8".to_string(),
                    layer: 41,
                    x: 24.0,
                    y: 40.0,
                },
            ],
        };

        assert_eq!(scene.player_focus_tile(8.0), Some((3, 5)));

        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![RenderObject {
                id: "player:7".to_string(),
                layer: 40,
                x: f32::INFINITY,
                y: f32::NEG_INFINITY,
            }],
        };

        assert_eq!(scene.player_focus_tile(8.0), None);
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
                    id: "unit:9".to_string(),
                    layer: 39,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "fire:10".to_string(),
                    layer: 38,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "puddle:11".to_string(),
                    layer: 37,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "weather:12".to_string(),
                    layer: 36,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "world-label:13".to_string(),
                    layer: 35,
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
                total_count: 14,
                player_count: 1,
                marker_count: 3,
                plan_count: 1,
                block_count: 0,
                runtime_count: 7,
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
                    RenderSemanticDetailCount {
                        label: "runtime-fire",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "runtime-puddle",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "runtime-unit",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "runtime-weather",
                        count: 1,
                    },
                    RenderSemanticDetailCount {
                        label: "runtime-world-label",
                        count: 1,
                    },
                ],
            }
        );
        assert_eq!(
            summary.detail_text().as_deref(),
            Some(
                "marker-line:1,marker-line-end:1,marker-text:1,plan-build:1,runtime-building:1,runtime-config:1,runtime-fire:1,runtime-puddle:1,runtime-unit:1,runtime-weather:1,runtime-world-label:1"
            )
        );
        assert_eq!(
            summary.family_text(),
            "players=1 markers=3 plans=1 blocks=0 runtime=7 terrain=1 unknown=1"
        );
        assert_eq!(
            summary.family_and_detail_text(),
            "players=1 markers=3 plans=1 blocks=0 runtime=7 terrain=1 unknown=1 detail=marker-line:1,marker-line-end:1,marker-text:1,plan-build:1,runtime-building:1,runtime-config:1,runtime-fire:1,runtime-puddle:1,runtime-unit:1,runtime-weather:1,runtime-world-label:1"
        );
    }

    #[test]
    fn render_model_counts_runtime_live_entity_prefixes_in_runtime_family_summary() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "unit:7".to_string(),
                    layer: 40,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "fire:8".to_string(),
                    layer: 39,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "puddle:9".to_string(),
                    layer: 38,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "weather:10".to_string(),
                    layer: 37,
                    x: 0.0,
                    y: 0.0,
                },
                RenderObject {
                    id: "world-label:11".to_string(),
                    layer: 36,
                    x: 0.0,
                    y: 0.0,
                },
            ],
        };

        let summary = scene.semantic_summary();
        assert_eq!(summary.total_count, 5);
        assert_eq!(summary.player_count, 0);
        assert_eq!(summary.runtime_count, 5);
        assert_eq!(summary.unknown_count, 0);
    }

    #[test]
    fn render_model_derives_line_primitives_from_marker_line_pairs() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:line:runtime-demo".to_string(),
                    layer: 25,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:line:runtime-demo:line-end".to_string(),
                    layer: 25,
                    x: 32.0,
                    y: 40.0,
                },
                RenderObject {
                    id: "marker:text:ignored".to_string(),
                    layer: 26,
                    x: 0.0,
                    y: 0.0,
                },
            ],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Line {
                id: "marker:line:runtime-demo".to_string(),
                layer: 25,
                x0: 8.0,
                y0: 16.0,
                x1: 32.0,
                y1: 40.0,
            }]
        );
    }

    #[test]
    fn render_model_derives_text_primitives_from_runtime_world_label_objects() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "world-label:404:text:48656c6c6f".to_string(),
                    layer: 39,
                    x: 56.0,
                    y: 72.0,
                },
                RenderObject {
                    id: "world-label:405".to_string(),
                    layer: 39,
                    x: 8.0,
                    y: 16.0,
                },
            ],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Text {
                id: "world-label:404:text:48656c6c6f".to_string(),
                kind: RenderObjectSemanticKind::RuntimeWorldLabel,
                layer: 39,
                x: 56.0,
                y: 72.0,
                text: "Hello".to_string(),
            }]
        );
    }

    #[test]
    fn render_model_derives_text_primitives_from_marker_text_objects() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:text:42:text:48656c6c6f".to_string(),
                layer: 30,
                x: 24.0,
                y: 32.0,
            }],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Text {
                id: "marker:text:42:text:48656c6c6f".to_string(),
                kind: RenderObjectSemanticKind::MarkerText,
                layer: 30,
                x: 24.0,
                y: 32.0,
                text: "Hello".to_string(),
            }]
        );
    }

    #[test]
    fn render_model_derives_text_primitives_from_marker_shape_text_objects() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![RenderObject {
                id: "marker:shape-text:42:text:48656c6c6f".to_string(),
                layer: 30,
                x: 24.0,
                y: 32.0,
            }],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Text {
                id: "marker:shape-text:42:text:48656c6c6f".to_string(),
                kind: RenderObjectSemanticKind::MarkerShapeText,
                layer: 30,
                x: 24.0,
                y: 32.0,
                text: "Hello".to_string(),
            }]
        );
    }

    #[test]
    fn render_model_derives_icon_primitives_from_runtime_icon_markers() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:runtime-effect-icon:content-icon:normal:-1:6:9:0x00000008:0x00000010"
                        .to_string(),
                    layer: 31,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-build-config-icon:payload-source:21:43:1:7".to_string(),
                    layer: 32,
                    x: 168.0,
                    y: 344.0,
                },
                RenderObject {
                    id: "marker:runtime-health:4:5".to_string(),
                    layer: 33,
                    x: 32.0,
                    y: 40.0,
                },
                RenderObject {
                    id: "marker:runtime-effect:normal:13:0x41000000:0x41800000:1".to_string(),
                    layer: 26,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config:1:2:string".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config-parse-fail:2:2:int".to_string(),
                    layer: 34,
                    x: 16.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config-noapply:3:2:content".to_string(),
                    layer: 35,
                    x: 24.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config-rollback:4:2:unit".to_string(),
                    layer: 36,
                    x: 32.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-config-pending-mismatch:5:2:payload".to_string(),
                    layer: 37,
                    x: 40.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:runtime-command-position-target:0x42c00000:0x42f00000".to_string(),
                    layer: 29,
                    x: 96.0,
                    y: 120.0,
                },
                RenderObject {
                    id: "marker:runtime-command-selected-unit:22".to_string(),
                    layer: 29,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:0:0x40800000".to_string(),
                    layer: 16,
                    x: 240.0,
                    y: 320.0,
                },
                RenderObject {
                    id: "marker:runtime-unit-assembler-command:tank-assembler:30:40:0x42200000:0x42700000".to_string(),
                    layer: 16,
                    x: 40.0,
                    y: 60.0,
                },
                RenderObject {
                    id: "marker:runtime-break:0:3:4".to_string(),
                    layer: 14,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: "marker:runtime-bullet:1:17:4".to_string(),
                    layer: 28,
                    x: 48.0,
                    y: 56.0,
                },
                RenderObject {
                    id: "marker:runtime-logic-explosion:2:2:0x42800000:1:1:0:1".to_string(),
                    layer: 28,
                    x: 64.0,
                    y: 72.0,
                },
                RenderObject {
                    id: "marker:runtime-sound-at:3:11".to_string(),
                    layer: 28,
                    x: 80.0,
                    y: 88.0,
                },
                RenderObject {
                    id: "marker:runtime-auto-door-toggle:4:3:4:1".to_string(),
                    layer: 28,
                    x: 96.0,
                    y: 104.0,
                },
                RenderObject {
                    id: "marker:runtime-effect-icon:content-icon:normal:bad".to_string(),
                    layer: 33,
                    x: 0.0,
                    y: 0.0,
                },
            ],
        };

        assert_eq!(
            scene.primitives(),
            vec![
                RenderPrimitive::Icon {
                    id: "marker:runtime-effect-icon:content-icon:normal:-1:6:9:0x00000008:0x00000010"
                        .to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeEffect,
                    variant: "content-icon".to_string(),
                    layer: 31,
                    x: 8.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-build-config-icon:payload-source:21:43:1:7".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeBuildConfig,
                    variant: "payload-source".to_string(),
                    layer: 32,
                    x: 168.0,
                    y: 344.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-health:4:5".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeHealth,
                    variant: "health".to_string(),
                    layer: 33,
                    x: 32.0,
                    y: 40.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-effect:normal:13:0x41000000:0x41800000:1".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeEffectMarker,
                    variant: "normal".to_string(),
                    layer: 26,
                    x: 8.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-config:1:2:string".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeConfig,
                    variant: "string".to_string(),
                    layer: 30,
                    x: 8.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-config-parse-fail:2:2:int".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeConfigParseFail,
                    variant: "int".to_string(),
                    layer: 34,
                    x: 16.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-config-noapply:3:2:content".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeConfigNoApply,
                    variant: "content".to_string(),
                    layer: 35,
                    x: 24.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-config-rollback:4:2:unit".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeConfigRollback,
                    variant: "unit".to_string(),
                    layer: 36,
                    x: 32.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-config-pending-mismatch:5:2:payload".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeConfigPendingMismatch,
                    variant: "payload".to_string(),
                    layer: 37,
                    x: 40.0,
                    y: 16.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-command-position-target:0x42c00000:0x42f00000".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeCommand,
                    variant: "position-target".to_string(),
                    layer: 29,
                    x: 96.0,
                    y: 120.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-command-selected-unit:22".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeCommand,
                    variant: "selected-unit".to_string(),
                    layer: 29,
                    x: 24.0,
                    y: 32.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-unit-assembler-progress:tank-assembler:30:40:0x3f400000:2:4:b:9:0:0x40800000".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeUnitAssemblerProgress,
                    variant: "tank-assembler".to_string(),
                    layer: 16,
                    x: 240.0,
                    y: 320.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-unit-assembler-command:tank-assembler:30:40:0x42200000:0x42700000".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeUnitAssemblerCommand,
                    variant: "tank-assembler".to_string(),
                    layer: 16,
                    x: 40.0,
                    y: 60.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-break:0:3:4".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeBreak,
                    variant: "break".to_string(),
                    layer: 14,
                    x: 24.0,
                    y: 32.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-bullet:1:17:4".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeBullet,
                    variant: "bullet".to_string(),
                    layer: 28,
                    x: 48.0,
                    y: 56.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-logic-explosion:2:2:0x42800000:1:1:0:1".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeLogicExplosion,
                    variant: "logic-explosion".to_string(),
                    layer: 28,
                    x: 64.0,
                    y: 72.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-sound-at:3:11".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeSoundAt,
                    variant: "sound-at".to_string(),
                    layer: 28,
                    x: 80.0,
                    y: 88.0,
                },
                RenderPrimitive::Icon {
                    id: "marker:runtime-auto-door-toggle:4:3:4:1".to_string(),
                    family: RenderIconPrimitiveFamily::RuntimeTileAction,
                    variant: "auto-door-toggle".to_string(),
                    layer: 28,
                    x: 96.0,
                    y: 104.0,
                },
            ]
        );
    }

    #[test]
    fn render_model_derives_icon_primitive_from_runtime_place_plan_objects() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![RenderObject {
                id: "plan:runtime-place:0:8:9".to_string(),
                layer: 21,
                x: 64.0,
                y: 72.0,
            }],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Icon {
                id: "plan:runtime-place:0:8:9".to_string(),
                family: RenderIconPrimitiveFamily::RuntimePlace,
                variant: "place".to_string(),
                layer: 21,
                x: 64.0,
                y: 72.0,
            }]
        );
    }

    #[test]
    fn render_model_derives_rect_primitives_from_runtime_command_rect_line_families() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:top:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 16.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:top:{}:{}:{}:{}:line-end",
                        8.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 16.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:right:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 16.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:right:{}:{}:{}:{}:line-end",
                        24.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:bottom:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 24.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:bottom:{}:{}:{}:{}:line-end",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:left:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 32.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-command-rect:left:{}:{}:{}:{}:line-end",
                        8.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    layer: 29,
                    x: 8.0,
                    y: 16.0,
                },
            ],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Rect {
                id: format!(
                    "marker:rect:runtime-command-rect:{}:{}:{}:{}",
                    8.0f32.to_bits(),
                    16.0f32.to_bits(),
                    24.0f32.to_bits(),
                    32.0f32.to_bits()
                ),
                family: "runtime-command-rect".to_string(),
                layer: 29,
                left: 8.0,
                top: 16.0,
                right: 24.0,
                bottom: 32.0,
                line_ids: vec![
                    format!(
                        "marker:line:runtime-command-rect:bottom:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    format!(
                        "marker:line:runtime-command-rect:left:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        32.0f32.to_bits(),
                        8.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                    format!(
                        "marker:line:runtime-command-rect:right:{}:{}:{}:{}",
                        24.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        32.0f32.to_bits()
                    ),
                    format!(
                        "marker:line:runtime-command-rect:top:{}:{}:{}:{}",
                        8.0f32.to_bits(),
                        16.0f32.to_bits(),
                        24.0f32.to_bits(),
                        16.0f32.to_bits()
                    ),
                ],
            }]
        );
    }

    #[test]
    fn render_model_derives_rect_primitives_from_runtime_break_rect_line_families() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}:line-end",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}",
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 40.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}:line-end",
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}",
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 40.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}:line-end",
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 48.0,
                },
                RenderObject {
                    id: format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}:line-end",
                        32.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    layer: 30,
                    x: 32.0,
                    y: 40.0,
                },
            ],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Rect {
                id: format!(
                    "marker:rect:runtime-break-rect:{}:{}:{}:{}",
                    32.0f32.to_bits(),
                    40.0f32.to_bits(),
                    40.0f32.to_bits(),
                    48.0f32.to_bits()
                ),
                family: "runtime-break-rect".to_string(),
                layer: 30,
                left: 32.0,
                top: 40.0,
                right: 40.0,
                bottom: 48.0,
                line_ids: vec![
                    format!(
                        "marker:line:runtime-break-rect:bottom:{}:{}:{}:{}",
                        40.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    format!(
                        "marker:line:runtime-break-rect:left:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        48.0f32.to_bits(),
                        32.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                    format!(
                        "marker:line:runtime-break-rect:right:{}:{}:{}:{}",
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        48.0f32.to_bits()
                    ),
                    format!(
                        "marker:line:runtime-break-rect:top:{}:{}:{}:{}",
                        32.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits(),
                        40.0f32.to_bits()
                    ),
                ],
            }]
        );
    }

    #[test]
    fn render_model_derives_rect_primitives_from_runtime_unit_assembler_area_line_families() {
        let scene = RenderModel {
            viewport: Viewport::default(),
            view_window: None,
            objects: vec![
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top"
                        .to_string(),
                    layer: 15,
                    x: 216.0,
                    y: 280.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top:line-end"
                        .to_string(),
                    layer: 15,
                    x: 256.0,
                    y: 280.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:right"
                        .to_string(),
                    layer: 15,
                    x: 256.0,
                    y: 280.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:right:line-end"
                        .to_string(),
                    layer: 15,
                    x: 256.0,
                    y: 320.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:bottom"
                        .to_string(),
                    layer: 15,
                    x: 256.0,
                    y: 320.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:bottom:line-end"
                        .to_string(),
                    layer: 15,
                    x: 216.0,
                    y: 320.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:left"
                        .to_string(),
                    layer: 15,
                    x: 216.0,
                    y: 320.0,
                },
                RenderObject {
                    id: "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:left:line-end"
                        .to_string(),
                    layer: 15,
                    x: 216.0,
                    y: 280.0,
                },
            ],
        };

        assert_eq!(
            scene.primitives(),
            vec![RenderPrimitive::Rect {
                id: format!(
                    "marker:rect:runtime-unit-assembler-area:tank-assembler:30:40:{}:{}:{}:{}",
                    216.0f32.to_bits(),
                    280.0f32.to_bits(),
                    256.0f32.to_bits(),
                    320.0f32.to_bits()
                ),
                family: "runtime-unit-assembler-area".to_string(),
                layer: 15,
                left: 216.0,
                top: 280.0,
                right: 256.0,
                bottom: 320.0,
                line_ids: vec![
                    "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:bottom"
                        .to_string(),
                    "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:left".to_string(),
                    "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:right"
                        .to_string(),
                    "marker:line:runtime-unit-assembler-area:tank-assembler:30:40:top".to_string(),
                ],
            }]
        );
    }

    #[test]
    fn render_model_pipeline_summary_tracks_visible_window_layers_and_clipped_objects() {
        let scene = RenderModel {
            viewport: Viewport {
                width: 64.0,
                height: 64.0,
                zoom: 1.0,
            },
            view_window: Some(RenderViewWindow {
                origin_x: 1,
                origin_y: 1,
                width: 3,
                height: 3,
            }),
            objects: vec![
                RenderObject {
                    id: "terrain:1".to_string(),
                    layer: 0,
                    x: 8.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "marker:line:77".to_string(),
                    layer: 30,
                    x: 16.0,
                    y: 16.0,
                },
                RenderObject {
                    id: "marker:line:77:line-end".to_string(),
                    layer: 30,
                    x: 24.0,
                    y: 24.0,
                },
                RenderObject {
                    id: "player:7".to_string(),
                    layer: 40,
                    x: 24.0,
                    y: 8.0,
                },
                RenderObject {
                    id: "plan:build:1:6:6:257".to_string(),
                    layer: 20,
                    x: 48.0,
                    y: 48.0,
                },
                RenderObject {
                    id: "marker:runtime-config:3:2:string".to_string(),
                    layer: 35,
                    x: 56.0,
                    y: 56.0,
                },
            ],
        };

        let summary = scene.pipeline_summary_for_window(
            8.0,
            RenderViewWindow {
                origin_x: 1,
                origin_y: 1,
                width: 3,
                height: 3,
            },
        );

        assert_eq!(
            summary,
            RenderPipelineSummary {
                total_object_count: 6,
                visible_object_count: 4,
                clipped_object_count: 2,
                visible_semantics: RenderSemanticSummary {
                    total_count: 4,
                    player_count: 1,
                    marker_count: 2,
                    plan_count: 0,
                    block_count: 0,
                    runtime_count: 0,
                    terrain_count: 1,
                    unknown_count: 0,
                    detail_counts: vec![
                        RenderSemanticDetailCount {
                            label: "marker-line",
                            count: 1,
                        },
                        RenderSemanticDetailCount {
                            label: "marker-line-end",
                            count: 1,
                        },
                    ],
                },
                focus_tile: Some((3, 1)),
                window: Some(RenderViewWindow {
                    origin_x: 1,
                    origin_y: 1,
                    width: 3,
                    height: 3,
                }),
                layer_span: Some((0, 40)),
                layers: vec![
                    RenderPipelineLayerSummary {
                        layer: 0,
                        object_count: 1,
                        player_count: 0,
                        marker_count: 0,
                        plan_count: 0,
                        block_count: 0,
                        runtime_count: 0,
                        terrain_count: 1,
                        unknown_count: 0,
                        detail_counts: Vec::new(),
                    },
                    RenderPipelineLayerSummary {
                        layer: 30,
                        object_count: 2,
                        player_count: 0,
                        marker_count: 2,
                        plan_count: 0,
                        block_count: 0,
                        runtime_count: 0,
                        terrain_count: 0,
                        unknown_count: 0,
                        detail_counts: vec![
                            RenderSemanticDetailCount {
                                label: "marker-line",
                                count: 1,
                            },
                            RenderSemanticDetailCount {
                                label: "marker-line-end",
                                count: 1,
                            },
                        ],
                    },
                    RenderPipelineLayerSummary {
                        layer: 40,
                        object_count: 1,
                        player_count: 1,
                        marker_count: 0,
                        plan_count: 0,
                        block_count: 0,
                        runtime_count: 0,
                        terrain_count: 0,
                        unknown_count: 0,
                        detail_counts: Vec::new(),
                    },
                ],
            }
        );
        assert_eq!(
            summary.visible_semantics.family_text(),
            "players=1 markers=2 plans=0 blocks=0 runtime=0 terrain=1 unknown=0"
        );
        assert_eq!(
            summary.layers[1].family_and_detail_text(),
            "players=0 markers=2 plans=0 blocks=0 runtime=0 terrain=0 unknown=0 detail=marker-line:1,marker-line-end:1"
        );
    }
}
