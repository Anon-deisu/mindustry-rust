use mdt_client_min::custom_packet_runtime::{
    RuntimeCustomPacketSemanticEncoding, RuntimeCustomPacketSemanticKind,
    RuntimeCustomPacketSemanticSpec,
};
use mdt_client_min::custom_packet_runtime_surface::{
    RuntimeCustomPacketOverlayMarker, RuntimeCustomPacketSurface,
    RuntimeCustomPacketSurfaceSummaryEntry,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Default)]
pub struct RuntimeCustomPacketHost {
    state: RuntimeCustomPacketHostState,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketHostState {
    routes: BTreeMap<RuntimeCustomPacketHostRouteKey, RuntimeCustomPacketHostRouteState>,
    pending_lines: VecDeque<String>,
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
        if specs.is_empty() {
            return None;
        }
        let mut state = RuntimeCustomPacketHostState::default();
        for spec in specs {
            state
                .routes
                .entry(RuntimeCustomPacketHostRouteKey {
                    key: spec.key.clone(),
                    encoding: spec.encoding,
                    semantic: spec.semantic,
                })
                .or_default();
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
            "runtime_custom_packet_host_state: routes={} active_routes={} surface_resets={} reconnect_resets={} manual_clears={}",
            self.state.routes.len(),
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
            let route = RuntimeCustomPacketHostRouteKey {
                key: entry.key.clone(),
                encoding: entry.encoding,
                semantic: entry.semantic,
            };
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
        if cleared_routes > 0 {
            self.pending_lines.push_back(format!(
                "runtime_custom_packet_host_clear: tick={now_ms}ms reason={reason} cleared_routes={cleared_routes}"
            ));
        }
    }
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
}
