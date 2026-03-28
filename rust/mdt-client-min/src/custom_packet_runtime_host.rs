use mdt_client_min::custom_packet_runtime::{
    RuntimeCustomPacketSemanticEncoding, RuntimeCustomPacketSemanticKind,
    RuntimeCustomPacketSemanticSpec,
};
use mdt_client_min::custom_packet_runtime_surface::{
    RuntimeCustomPacketOverlayMarker, RuntimeCustomPacketSurface,
    RuntimeCustomPacketSurfaceSummaryEntry,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeCustomPacketHostActionKind {
    BuildingControlSelect,
    RequestBuildPayload,
    ClearItems,
    ClearLiquids,
    TransferInventory,
    TileTap,
    UnitControl,
    RequestUnitPayload,
    RequestDropPayload,
}

impl RuntimeCustomPacketHostActionKind {
    pub fn label(self) -> &'static str {
        match self {
            RuntimeCustomPacketHostActionKind::BuildingControlSelect => "building-control-select",
            RuntimeCustomPacketHostActionKind::RequestBuildPayload => "request-build-payload",
            RuntimeCustomPacketHostActionKind::ClearItems => "clear-items",
            RuntimeCustomPacketHostActionKind::ClearLiquids => "clear-liquids",
            RuntimeCustomPacketHostActionKind::TransferInventory => "transfer-inventory",
            RuntimeCustomPacketHostActionKind::TileTap => "tile-tap",
            RuntimeCustomPacketHostActionKind::UnitControl => "unit-control",
            RuntimeCustomPacketHostActionKind::RequestUnitPayload => "request-unit-payload",
            RuntimeCustomPacketHostActionKind::RequestDropPayload => "request-drop-payload",
        }
    }

    pub fn supports_semantic(self, semantic: RuntimeCustomPacketSemanticKind) -> bool {
        match self {
            RuntimeCustomPacketHostActionKind::BuildingControlSelect
            | RuntimeCustomPacketHostActionKind::RequestBuildPayload
            | RuntimeCustomPacketHostActionKind::ClearItems
            | RuntimeCustomPacketHostActionKind::ClearLiquids
            | RuntimeCustomPacketHostActionKind::TransferInventory
            | RuntimeCustomPacketHostActionKind::TileTap => {
                semantic == RuntimeCustomPacketSemanticKind::BuildPos
            }
            RuntimeCustomPacketHostActionKind::UnitControl
            | RuntimeCustomPacketHostActionKind::RequestUnitPayload => {
                semantic == RuntimeCustomPacketSemanticKind::UnitId
            }
            RuntimeCustomPacketHostActionKind::RequestDropPayload => {
                semantic == RuntimeCustomPacketSemanticKind::WorldPos
            }
        }
    }

    pub fn expected_semantic_labels(self) -> &'static str {
        match self {
            RuntimeCustomPacketHostActionKind::BuildingControlSelect
            | RuntimeCustomPacketHostActionKind::RequestBuildPayload
            | RuntimeCustomPacketHostActionKind::ClearItems
            | RuntimeCustomPacketHostActionKind::ClearLiquids
            | RuntimeCustomPacketHostActionKind::TransferInventory
            | RuntimeCustomPacketHostActionKind::TileTap => "build-pos",
            RuntimeCustomPacketHostActionKind::UnitControl
            | RuntimeCustomPacketHostActionKind::RequestUnitPayload => "unit-id",
            RuntimeCustomPacketHostActionKind::RequestDropPayload => "world-pos",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeCustomPacketHostActionSpec {
    pub key: String,
    pub encoding: RuntimeCustomPacketSemanticEncoding,
    pub semantic: RuntimeCustomPacketSemanticKind,
    pub action: RuntimeCustomPacketHostActionKind,
}

impl RuntimeCustomPacketHostActionSpec {
    pub fn route_spec(&self) -> RuntimeCustomPacketSemanticSpec {
        RuntimeCustomPacketSemanticSpec {
            key: self.key.clone(),
            encoding: self.encoding,
            semantic: self.semantic,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeCustomPacketHostAction {
    BuildingControlSelect { key: String, build_pos: i32 },
    RequestBuildPayload { key: String, build_pos: i32 },
    ClearItems { key: String, build_pos: i32 },
    ClearLiquids { key: String, build_pos: i32 },
    TransferInventory { key: String, build_pos: i32 },
    TileTap { key: String, tile_pos: i32 },
    UnitControl { key: String, unit_id: i32 },
    RequestUnitPayload { key: String, unit_id: i32 },
    RequestDropPayload { key: String, x: f32, y: f32 },
}

impl RuntimeCustomPacketHostAction {
    pub fn label(&self) -> &'static str {
        match self {
            RuntimeCustomPacketHostAction::BuildingControlSelect { .. } => {
                "building-control-select"
            }
            RuntimeCustomPacketHostAction::RequestBuildPayload { .. } => "request-build-payload",
            RuntimeCustomPacketHostAction::ClearItems { .. } => "clear-items",
            RuntimeCustomPacketHostAction::ClearLiquids { .. } => "clear-liquids",
            RuntimeCustomPacketHostAction::TransferInventory { .. } => "transfer-inventory",
            RuntimeCustomPacketHostAction::TileTap { .. } => "tile-tap",
            RuntimeCustomPacketHostAction::UnitControl { .. } => "unit-control",
            RuntimeCustomPacketHostAction::RequestUnitPayload { .. } => "request-unit-payload",
            RuntimeCustomPacketHostAction::RequestDropPayload { .. } => "request-drop-payload",
        }
    }

    pub fn source_key(&self) -> &str {
        match self {
            RuntimeCustomPacketHostAction::BuildingControlSelect { key, .. }
            | RuntimeCustomPacketHostAction::RequestBuildPayload { key, .. }
            | RuntimeCustomPacketHostAction::ClearItems { key, .. }
            | RuntimeCustomPacketHostAction::ClearLiquids { key, .. }
            | RuntimeCustomPacketHostAction::TransferInventory { key, .. }
            | RuntimeCustomPacketHostAction::TileTap { key, .. }
            | RuntimeCustomPacketHostAction::UnitControl { key, .. }
            | RuntimeCustomPacketHostAction::RequestUnitPayload { key, .. }
            | RuntimeCustomPacketHostAction::RequestDropPayload { key, .. } => key,
        }
    }
}

#[derive(Debug, Default)]
pub struct RuntimeCustomPacketHost {
    state: RuntimeCustomPacketHostState,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketHostState {
    routes: BTreeMap<RuntimeCustomPacketHostRouteKey, RuntimeCustomPacketHostRouteState>,
    action_bindings:
        BTreeMap<RuntimeCustomPacketHostRouteKey, Vec<RuntimeCustomPacketHostActionKind>>,
    pending_lines: VecDeque<String>,
    pending_actions: VecDeque<RuntimeCustomPacketHostAction>,
    next_update_serial: u64,
    surface_reset_count: usize,
    reconnect_reset_count: usize,
    manual_clear_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeCustomPacketHostRouteKey {
    key: String,
    encoding: RuntimeCustomPacketSemanticEncoding,
    semantic: RuntimeCustomPacketSemanticKind,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketHostRouteState {
    apply_count: usize,
    active: bool,
    last_stable_value: Option<String>,
    last_marker: Option<RuntimeCustomPacketOverlayMarker>,
    last_update_serial: u64,
}

impl RuntimeCustomPacketHost {
    pub fn from_specs(specs: &[RuntimeCustomPacketSemanticSpec]) -> Option<Self> {
        Self::from_specs_with_actions(specs, &[])
    }

    pub fn from_specs_with_actions(
        specs: &[RuntimeCustomPacketSemanticSpec],
        action_specs: &[RuntimeCustomPacketHostActionSpec],
    ) -> Option<Self> {
        if specs.is_empty() && action_specs.is_empty() {
            return None;
        }
        let mut state = RuntimeCustomPacketHostState::default();
        for spec in specs {
            state.routes.entry(route_key_from_spec(spec)).or_default();
        }
        for action_spec in action_specs {
            let route = route_key_from_host_action_spec(action_spec);
            state.routes.entry(route.clone()).or_default();
            state
                .action_bindings
                .entry(route)
                .or_default()
                .push(action_spec.action);
        }
        Some(Self { state })
    }

    pub fn observe_summary_entries(
        &mut self,
        now_ms: u64,
        entries: &[RuntimeCustomPacketSurfaceSummaryEntry],
    ) {
        self.state.observe_summary_entries(now_ms, entries);
    }

    pub fn observe_surface(
        &mut self,
        now_ms: u64,
        surface: &RuntimeCustomPacketSurface,
        max_entries: usize,
    ) {
        let entries = surface.latest_summary_entries(max_entries);
        self.observe_summary_entries(now_ms, &entries);
    }

    pub fn note_surface_reset(&mut self, now_ms: u64, reason: &str) {
        self.state.surface_reset_count = self.state.surface_reset_count.saturating_add(1);
        self.state
            .clear_active_routes(now_ms, &format!("surface:{reason}"));
    }

    pub fn note_reconnect_reset(&mut self, now_ms: u64, reason: &str) {
        self.state.reconnect_reset_count = self.state.reconnect_reset_count.saturating_add(1);
        self.state
            .clear_active_routes(now_ms, &format!("reconnect:{reason}"));
    }

    pub fn clear(&mut self, now_ms: u64, reason: &str) {
        self.state.manual_clear_count = self.state.manual_clear_count.saturating_add(1);
        self.state.clear_active_routes(now_ms, reason);
    }

    pub fn drain_lines(&mut self) -> Vec<String> {
        self.state.pending_lines.drain(..).collect()
    }

    pub fn drain_actions(&mut self) -> Vec<RuntimeCustomPacketHostAction> {
        self.state.pending_actions.drain(..).collect()
    }

    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = self
            .state
            .routes
            .iter()
            .map(|(route, state)| {
                format!(
                    "runtime_custom_packet_host_summary: encoding={} key={:?} semantic={} apply_count={} active={} last={:?}",
                    encoding_label(route.encoding),
                    route.key,
                    semantic_label(route.semantic),
                    state.apply_count,
                    state.active,
                    state.last_stable_value
                )
            })
            .collect::<Vec<_>>();
        lines.push(format!(
            "runtime_custom_packet_host_state: routes={} hook_routes={} pending_actions={} active_routes={} surface_resets={} reconnect_resets={} manual_clears={}",
            self.state.routes.len(),
            self.state.action_bindings.len(),
            self.state.pending_actions.len(),
            self.state.routes.values().filter(|route| route.active).count(),
            self.state.surface_reset_count,
            self.state.reconnect_reset_count,
            self.state.manual_clear_count,
        ));
        lines
    }

    pub fn business_summary_text(&self, max_entries: usize) -> Option<String> {
        if max_entries == 0 {
            return None;
        }
        let mut entries = self
            .state
            .routes
            .iter()
            .filter_map(|(route, state)| {
                let stable_value = state.last_stable_value.as_ref()?;
                state.active.then_some((
                    state.last_update_serial,
                    format_host_business_entry(
                        route,
                        state.apply_count,
                        stable_value,
                        state.last_marker.as_ref(),
                    ),
                ))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let summary = entries
            .into_iter()
            .take(max_entries)
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>()
            .join(" | ");
        (!summary.is_empty()).then_some(summary)
    }
}

impl RuntimeCustomPacketHostState {
    fn observe_summary_entries(
        &mut self,
        now_ms: u64,
        entries: &[RuntimeCustomPacketSurfaceSummaryEntry],
    ) {
        for entry in entries {
            let route = route_key_from_surface_entry(entry);
            let Some(state) = self.routes.get_mut(&route) else {
                self.pending_lines.push_back(format!(
                    "runtime_custom_packet_host_unregistered_surface_entry: tick={now_ms}ms encoding={} key={:?} semantic={} value={:?}",
                    encoding_label(entry.encoding),
                    entry.key,
                    semantic_label(entry.semantic),
                    entry.stable_value,
                ));
                continue;
            };
            if state.active
                && state.last_stable_value.as_deref() == Some(entry.stable_value.as_str())
                && state.last_marker == entry.marker
            {
                continue;
            }
            state.apply_count = state.apply_count.saturating_add(1);
            state.active = true;
            state.last_stable_value = Some(entry.stable_value.clone());
            state.last_marker = entry.marker.clone();
            self.next_update_serial = self.next_update_serial.saturating_add(1);
            state.last_update_serial = self.next_update_serial;
            self.pending_lines.push_back(format!(
                "runtime_custom_packet_host_apply: tick={now_ms}ms encoding={} key={:?} semantic={} apply_count={} value={:?} marker={}",
                encoding_label(entry.encoding),
                entry.key,
                semantic_label(entry.semantic),
                state.apply_count,
                entry.stable_value,
                format_marker(entry.marker.as_ref()),
            ));
            let action_kinds = self
                .action_bindings
                .get(&route)
                .cloned()
                .unwrap_or_default();
            for action_kind in action_kinds {
                match build_host_action(&route, entry, action_kind) {
                    Ok(action) => {
                        self.pending_lines.push_back(format!(
                            "runtime_custom_packet_host_action: tick={now_ms}ms encoding={} key={:?} semantic={} action={} payload={}",
                            encoding_label(route.encoding),
                            route.key,
                            semantic_label(route.semantic),
                            action_kind.label(),
                            format_host_action_payload(&action),
                        ));
                        self.pending_actions.push_back(action);
                    }
                    Err(reason) => {
                        self.pending_lines.push_back(format!(
                            "runtime_custom_packet_host_action_skipped: tick={now_ms}ms encoding={} key={:?} semantic={} action={} reason={reason}",
                            encoding_label(route.encoding),
                            route.key,
                            semantic_label(route.semantic),
                            action_kind.label(),
                        ));
                    }
                }
            }
        }
    }

    fn clear_active_routes(&mut self, now_ms: u64, reason: &str) {
        let mut cleared_routes = 0usize;
        for route in self.routes.values_mut() {
            if route.active {
                cleared_routes = cleared_routes.saturating_add(1);
            }
            route.active = false;
            route.last_stable_value = None;
            route.last_marker = None;
            route.last_update_serial = 0;
        }
        let dropped_actions = self.pending_actions.len();
        self.pending_actions.clear();
        if cleared_routes > 0 {
            self.pending_lines.push_back(format!(
                "runtime_custom_packet_host_clear: tick={now_ms}ms reason={reason} cleared_routes={cleared_routes}"
            ));
        }
        if dropped_actions > 0 {
            self.pending_lines.push_back(format!(
                "runtime_custom_packet_host_action_queue_clear: tick={now_ms}ms reason={reason} dropped_actions={dropped_actions}"
            ));
        }
    }
}

fn route_key_from_spec(spec: &RuntimeCustomPacketSemanticSpec) -> RuntimeCustomPacketHostRouteKey {
    RuntimeCustomPacketHostRouteKey {
        key: spec.key.clone(),
        encoding: spec.encoding,
        semantic: spec.semantic,
    }
}

fn route_key_from_host_action_spec(
    spec: &RuntimeCustomPacketHostActionSpec,
) -> RuntimeCustomPacketHostRouteKey {
    RuntimeCustomPacketHostRouteKey {
        key: spec.key.clone(),
        encoding: spec.encoding,
        semantic: spec.semantic,
    }
}

fn route_key_from_surface_entry(
    entry: &RuntimeCustomPacketSurfaceSummaryEntry,
) -> RuntimeCustomPacketHostRouteKey {
    RuntimeCustomPacketHostRouteKey {
        key: entry.key.clone(),
        encoding: entry.encoding,
        semantic: entry.semantic,
    }
}

fn build_host_action(
    route: &RuntimeCustomPacketHostRouteKey,
    entry: &RuntimeCustomPacketSurfaceSummaryEntry,
    action_kind: RuntimeCustomPacketHostActionKind,
) -> Result<RuntimeCustomPacketHostAction, &'static str> {
    match action_kind {
        RuntimeCustomPacketHostActionKind::BuildingControlSelect => {
            Ok(RuntimeCustomPacketHostAction::BuildingControlSelect {
                key: route.key.clone(),
                build_pos: parse_surface_i32(&entry.stable_value).ok_or("invalid_build_pos")?,
            })
        }
        RuntimeCustomPacketHostActionKind::RequestBuildPayload => {
            Ok(RuntimeCustomPacketHostAction::RequestBuildPayload {
                key: route.key.clone(),
                build_pos: parse_surface_i32(&entry.stable_value).ok_or("invalid_build_pos")?,
            })
        }
        RuntimeCustomPacketHostActionKind::ClearItems => {
            Ok(RuntimeCustomPacketHostAction::ClearItems {
                key: route.key.clone(),
                build_pos: parse_surface_i32(&entry.stable_value).ok_or("invalid_build_pos")?,
            })
        }
        RuntimeCustomPacketHostActionKind::ClearLiquids => {
            Ok(RuntimeCustomPacketHostAction::ClearLiquids {
                key: route.key.clone(),
                build_pos: parse_surface_i32(&entry.stable_value).ok_or("invalid_build_pos")?,
            })
        }
        RuntimeCustomPacketHostActionKind::TransferInventory => {
            Ok(RuntimeCustomPacketHostAction::TransferInventory {
                key: route.key.clone(),
                build_pos: parse_surface_i32(&entry.stable_value).ok_or("invalid_build_pos")?,
            })
        }
        RuntimeCustomPacketHostActionKind::TileTap => Ok(RuntimeCustomPacketHostAction::TileTap {
            key: route.key.clone(),
            tile_pos: parse_surface_i32(&entry.stable_value).ok_or("invalid_tile_pos")?,
        }),
        RuntimeCustomPacketHostActionKind::UnitControl => {
            Ok(RuntimeCustomPacketHostAction::UnitControl {
                key: route.key.clone(),
                unit_id: parse_surface_i32(&entry.stable_value).ok_or("invalid_unit_id")?,
            })
        }
        RuntimeCustomPacketHostActionKind::RequestUnitPayload => {
            Ok(RuntimeCustomPacketHostAction::RequestUnitPayload {
                key: route.key.clone(),
                unit_id: parse_surface_i32(&entry.stable_value).ok_or("invalid_unit_id")?,
            })
        }
        RuntimeCustomPacketHostActionKind::RequestDropPayload => {
            let (x, y) = entry
                .marker
                .as_ref()
                .and_then(|marker| finite_surface_world_pos(marker.x, marker.y))
                .or_else(|| {
                    parse_surface_world_pos(&entry.stable_value)
                        .and_then(|(x, y)| finite_surface_world_pos(x, y))
                })
                .ok_or("invalid_world_pos")?;
            Ok(RuntimeCustomPacketHostAction::RequestDropPayload {
                key: route.key.clone(),
                x,
                y,
            })
        }
    }
}

fn format_host_action_payload(action: &RuntimeCustomPacketHostAction) -> String {
    match action {
        RuntimeCustomPacketHostAction::BuildingControlSelect { build_pos, .. }
        | RuntimeCustomPacketHostAction::RequestBuildPayload { build_pos, .. }
        | RuntimeCustomPacketHostAction::ClearItems { build_pos, .. }
        | RuntimeCustomPacketHostAction::ClearLiquids { build_pos, .. }
        | RuntimeCustomPacketHostAction::TransferInventory { build_pos, .. } => {
            format!("build_pos={build_pos}")
        }
        RuntimeCustomPacketHostAction::TileTap { tile_pos, .. } => {
            format!("tile_pos={tile_pos}")
        }
        RuntimeCustomPacketHostAction::UnitControl { unit_id, .. }
        | RuntimeCustomPacketHostAction::RequestUnitPayload { unit_id, .. } => {
            format!("unit_id={unit_id}")
        }
        RuntimeCustomPacketHostAction::RequestDropPayload { x, y, .. } => {
            format!("x={} y={}", format_coord(*x), format_coord(*y))
        }
    }
}

fn parse_surface_i32(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok()
}

fn parse_surface_world_pos(value: &str) -> Option<(f32, f32)> {
    let trimmed = value.trim();
    let (left, right) = trimmed
        .split_once(',')
        .or_else(|| trimmed.split_once(':'))?;
    let x = left.trim().parse::<f32>().ok()?;
    let y = right.trim().parse::<f32>().ok()?;
    Some((x, y))
}

fn finite_surface_world_pos(x: f32, y: f32) -> Option<(f32, f32)> {
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn format_host_business_entry(
    route: &RuntimeCustomPacketHostRouteKey,
    apply_count: usize,
    stable_value: &str,
    marker: Option<&RuntimeCustomPacketOverlayMarker>,
) -> String {
    format!(
        "{}:{}({})#{}={}{}",
        encoding_label(route.encoding),
        route.key,
        semantic_label(route.semantic),
        apply_count,
        stable_value,
        marker_suffix(marker),
    )
}

fn marker_suffix(marker: Option<&RuntimeCustomPacketOverlayMarker>) -> String {
    marker
        .map(|marker| format!("@{},{}", format_coord(marker.x), format_coord(marker.y)))
        .unwrap_or_default()
}

fn format_marker(marker: Option<&RuntimeCustomPacketOverlayMarker>) -> String {
    marker
        .map(|marker| {
            format!(
                "{}@{},{}",
                marker.key,
                format_coord(marker.x),
                format_coord(marker.y)
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn format_coord(value: f32) -> String {
    let rendered = value.to_string();
    if rendered.contains('.') {
        rendered
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        rendered
    }
}

fn encoding_label(encoding: RuntimeCustomPacketSemanticEncoding) -> &'static str {
    match encoding {
        RuntimeCustomPacketSemanticEncoding::Text => "text",
        RuntimeCustomPacketSemanticEncoding::Binary => "binary",
        RuntimeCustomPacketSemanticEncoding::LogicData => "logic",
    }
}

fn semantic_label(semantic: RuntimeCustomPacketSemanticKind) -> &'static str {
    match semantic {
        RuntimeCustomPacketSemanticKind::ServerMessage => "server_message",
        RuntimeCustomPacketSemanticKind::ChatMessage => "chat_message",
        RuntimeCustomPacketSemanticKind::HudText => "hud_text",
        RuntimeCustomPacketSemanticKind::Announce => "announce",
        RuntimeCustomPacketSemanticKind::Clipboard => "clipboard",
        RuntimeCustomPacketSemanticKind::OpenUri => "open_uri",
        RuntimeCustomPacketSemanticKind::WorldPos => "world_pos",
        RuntimeCustomPacketSemanticKind::BuildPos => "build_pos",
        RuntimeCustomPacketSemanticKind::UnitId => "unit_id",
        RuntimeCustomPacketSemanticKind::Team => "team",
        RuntimeCustomPacketSemanticKind::Bool => "bool",
        RuntimeCustomPacketSemanticKind::Number => "number",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logic_pos_spec() -> RuntimeCustomPacketSemanticSpec {
        RuntimeCustomPacketSemanticSpec {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
        }
    }

    fn status_spec() -> RuntimeCustomPacketSemanticSpec {
        RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        }
    }

    fn logic_pos_entry(value: &str, x: f32, y: f32) -> RuntimeCustomPacketSurfaceSummaryEntry {
        RuntimeCustomPacketSurfaceSummaryEntry {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
            stable_value: value.to_string(),
            marker: Some(RuntimeCustomPacketOverlayMarker {
                key: "logic.pos".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                x,
                y,
            }),
        }
    }

    fn status_entry(value: &str) -> RuntimeCustomPacketSurfaceSummaryEntry {
        RuntimeCustomPacketSurfaceSummaryEntry {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
            stable_value: value.to_string(),
            marker: None,
        }
    }

    fn build_spec() -> RuntimeCustomPacketSemanticSpec {
        RuntimeCustomPacketSemanticSpec {
            key: "build.select".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        }
    }

    fn build_action_spec() -> RuntimeCustomPacketHostActionSpec {
        RuntimeCustomPacketHostActionSpec {
            key: "build.select".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
            action: RuntimeCustomPacketHostActionKind::BuildingControlSelect,
        }
    }

    fn drop_action_spec() -> RuntimeCustomPacketHostActionSpec {
        RuntimeCustomPacketHostActionSpec {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
            action: RuntimeCustomPacketHostActionKind::RequestDropPayload,
        }
    }

    fn build_entry(value: &str) -> RuntimeCustomPacketSurfaceSummaryEntry {
        RuntimeCustomPacketSurfaceSummaryEntry {
            key: "build.select".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
            stable_value: value.to_string(),
            marker: None,
        }
    }

    #[test]
    fn runtime_custom_packet_host_tracks_changed_business_entries_only() {
        let mut host =
            RuntimeCustomPacketHost::from_specs(&[logic_pos_spec(), status_spec()]).unwrap();

        host.observe_summary_entries(
            42,
            &[logic_pos_entry("7,9", 7.0, 9.0), status_entry("wave ready")],
        );
        assert_eq!(
            host.business_summary_text(4),
            Some(
                "text:custom.status(hud_text)#1=wave ready | logic:logic.pos(world_pos)#1=7,9@7,9"
                    .to_string()
            )
        );

        host.drain_lines();
        host.observe_summary_entries(
            43,
            &[logic_pos_entry("7,9", 7.0, 9.0), status_entry("wave ready")],
        );
        assert!(host.drain_lines().is_empty());
        assert_eq!(
            host.business_summary_text(4),
            Some(
                "text:custom.status(hud_text)#1=wave ready | logic:logic.pos(world_pos)#1=7,9@7,9"
                    .to_string()
            )
        );

        host.observe_summary_entries(
            44,
            &[
                logic_pos_entry("7,9", 7.0, 9.0),
                status_entry("wave resumed"),
            ],
        );
        assert_eq!(
            host.business_summary_text(4),
            Some(
                "text:custom.status(hud_text)#2=wave resumed | logic:logic.pos(world_pos)#1=7,9@7,9"
                    .to_string()
            )
        );
    }

    #[test]
    fn runtime_custom_packet_host_clears_for_surface_and_reconnect_resets() {
        let mut host =
            RuntimeCustomPacketHost::from_specs(&[logic_pos_spec(), status_spec()]).unwrap();
        host.observe_summary_entries(
            42,
            &[logic_pos_entry("7,9", 7.0, 9.0), status_entry("wave ready")],
        );
        host.drain_lines();

        host.note_surface_reset(43, "world_data_begin");
        assert_eq!(
            host.drain_lines(),
            vec![
                "runtime_custom_packet_host_clear: tick=43ms reason=surface:world_data_begin cleared_routes=2"
                    .to_string()
            ]
        );
        assert_eq!(host.business_summary_text(4), None);

        host.observe_summary_entries(44, &[status_entry("wave resumed")]);
        assert_eq!(
            host.business_summary_text(4),
            Some("text:custom.status(hud_text)#2=wave resumed".to_string())
        );

        host.note_reconnect_reset(45, "redirect");
        assert_eq!(
            host.drain_lines(),
            vec![
                "runtime_custom_packet_host_apply: tick=44ms encoding=text key=\"custom.status\" semantic=hud_text apply_count=2 value=\"wave resumed\" marker=none".to_string(),
                "runtime_custom_packet_host_clear: tick=45ms reason=reconnect:redirect cleared_routes=1"
                    .to_string()
            ]
        );
        assert_eq!(host.business_summary_text(4), None);
    }

    #[test]
    fn runtime_custom_packet_host_manual_clear_preserves_apply_counts() {
        let mut host = RuntimeCustomPacketHost::from_specs(&[status_spec()]).unwrap();
        host.observe_summary_entries(10, &[status_entry("hello")]);
        host.drain_lines();

        host.clear(11, "manual:test");
        assert_eq!(
            host.drain_lines(),
            vec![
                "runtime_custom_packet_host_clear: tick=11ms reason=manual:test cleared_routes=1"
                    .to_string()
            ]
        );
        assert_eq!(host.business_summary_text(4), None);

        host.observe_summary_entries(12, &[status_entry("hello again")]);
        assert_eq!(
            host.business_summary_text(4),
            Some("text:custom.status(hud_text)#2=hello again".to_string())
        );
        assert!(host
            .summary_lines()
            .last()
            .unwrap()
            .contains("manual_clears=1"));
    }

    #[test]
    fn runtime_custom_packet_host_queues_bound_actions_only_for_changed_entries() {
        let mut host = RuntimeCustomPacketHost::from_specs_with_actions(
            &[logic_pos_spec(), build_spec()],
            &[drop_action_spec(), build_action_spec()],
        )
        .unwrap();

        host.observe_summary_entries(42, &[logic_pos_entry("7,9", 7.0, 9.0), build_entry("91")]);
        assert_eq!(
            host.drain_actions(),
            vec![
                RuntimeCustomPacketHostAction::RequestDropPayload {
                    key: "logic.pos".to_string(),
                    x: 7.0,
                    y: 9.0,
                },
                RuntimeCustomPacketHostAction::BuildingControlSelect {
                    key: "build.select".to_string(),
                    build_pos: 91,
                },
            ]
        );

        host.drain_lines();
        host.observe_summary_entries(43, &[logic_pos_entry("7,9", 7.0, 9.0), build_entry("91")]);
        assert!(host.drain_actions().is_empty());

        host.observe_summary_entries(44, &[logic_pos_entry("11,13", 11.0, 13.0)]);
        assert_eq!(
            host.drain_actions(),
            vec![RuntimeCustomPacketHostAction::RequestDropPayload {
                key: "logic.pos".to_string(),
                x: 11.0,
                y: 13.0,
            }]
        );
    }

    #[test]
    fn runtime_custom_packet_host_rejects_non_finite_world_pos_drop_actions() {
        let mut host = RuntimeCustomPacketHost::from_specs_with_actions(
            &[logic_pos_spec()],
            &[drop_action_spec()],
        )
        .unwrap();

        let mut entry = logic_pos_entry("7,9", 7.0, 9.0);
        entry.marker.as_mut().unwrap().x = f32::NAN;
        host.observe_summary_entries(42, &[entry]);
        assert_eq!(
            host.drain_actions(),
            vec![RuntimeCustomPacketHostAction::RequestDropPayload {
                key: "logic.pos".to_string(),
                x: 7.0,
                y: 9.0,
            }]
        );

        host.drain_lines();
        let mut invalid_entry = logic_pos_entry("NaN,9", 7.0, 9.0);
        invalid_entry.marker.as_mut().unwrap().x = f32::NAN;
        host.observe_summary_entries(43, &[invalid_entry]);
        assert!(host.drain_actions().is_empty());
    }

    #[test]
    fn runtime_custom_packet_host_parses_colon_separated_world_pos_drop_actions() {
        let mut host = RuntimeCustomPacketHost::from_specs_with_actions(
            &[logic_pos_spec()],
            &[drop_action_spec()],
        )
        .unwrap();

        host.observe_summary_entries(
            42,
            &[RuntimeCustomPacketSurfaceSummaryEntry {
                key: "logic.pos".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                stable_value: "7:9".to_string(),
                marker: None,
            }],
        );

        assert_eq!(
            host.drain_actions(),
            vec![RuntimeCustomPacketHostAction::RequestDropPayload {
                key: "logic.pos".to_string(),
                x: 7.0,
                y: 9.0,
            }]
        );
    }
}
