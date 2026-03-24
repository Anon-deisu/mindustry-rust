use crate::custom_packet_runtime::{
    RuntimeCustomPacketSemanticEncoding, RuntimeCustomPacketSemanticKind,
    RuntimeCustomPacketSemanticSpec,
};
use crate::custom_packet_runtime_surface::{
    RuntimeCustomPacketOverlayMarker, RuntimeCustomPacketSurfaceSummaryEntry,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Default)]
pub struct RuntimeCustomPacketBridge {
    state: RuntimeCustomPacketBridgeState,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketBridgeState {
    routes: BTreeMap<RuntimeCustomPacketBridgeRouteKey, RuntimeCustomPacketBridgeRouteState>,
    pending_lines: VecDeque<String>,
    next_update_serial: u64,
    surface_reset_count: usize,
    reconnect_reset_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeCustomPacketBridgeRouteKey {
    key: String,
    encoding: RuntimeCustomPacketSemanticEncoding,
    semantic: RuntimeCustomPacketSemanticKind,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketBridgeRouteState {
    apply_count: usize,
    active: bool,
    last_stable_value: Option<String>,
    last_marker: Option<RuntimeCustomPacketOverlayMarker>,
    last_update_serial: u64,
}

struct ParsedSurfaceUpdate {
    route: RuntimeCustomPacketBridgeRouteKey,
}

struct ParsedSurfaceReset<'a> {
    reason: &'a str,
}

impl RuntimeCustomPacketBridge {
    pub fn from_specs(specs: &[RuntimeCustomPacketSemanticSpec]) -> Option<Self> {
        if specs.is_empty() {
            return None;
        }
        let mut state = RuntimeCustomPacketBridgeState::default();
        for spec in specs {
            state
                .routes
                .entry(RuntimeCustomPacketBridgeRouteKey {
                    key: spec.key.clone(),
                    encoding: spec.encoding,
                    semantic: spec.semantic,
                })
                .or_default();
        }
        Some(Self { state })
    }

    pub fn observe_surface_activity(
        &mut self,
        now_ms: u64,
        lines: &[String],
        entries: &[RuntimeCustomPacketSurfaceSummaryEntry],
    ) {
        let entry_map = entries
            .iter()
            .map(|entry| {
                (
                    RuntimeCustomPacketBridgeRouteKey {
                        key: entry.key.clone(),
                        encoding: entry.encoding,
                        semantic: entry.semantic,
                    },
                    entry,
                )
            })
            .collect::<BTreeMap<_, _>>();
        for line in lines {
            if let Some(reset) = parse_surface_reset(line) {
                self.state.surface_reset_count = self.state.surface_reset_count.saturating_add(1);
                self.state
                    .clear_active_routes(now_ms, &format!("surface:{}", reset.reason));
                continue;
            }
            let Some(update) = parse_surface_update(line) else {
                continue;
            };
            let Some(entry) = entry_map.get(&update.route) else {
                self.state.pending_lines.push_back(format!(
                    "runtime_custom_packet_bridge_missing_surface_entry: tick={now_ms}ms encoding={} key={:?} semantic={}",
                    encoding_label(update.route.encoding),
                    update.route.key,
                    semantic_label(update.route.semantic),
                ));
                continue;
            };
            self.state.record_update(now_ms, &update.route, entry);
        }
    }

    pub fn note_reconnect_reset(&mut self, now_ms: u64, reason: &str) {
        self.state.reconnect_reset_count = self.state.reconnect_reset_count.saturating_add(1);
        self.state
            .clear_active_routes(now_ms, &format!("reconnect:{reason}"));
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
                    "runtime_custom_packet_bridge_summary: encoding={} key={:?} semantic={} apply_count={} active={} last={:?}",
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
            "runtime_custom_packet_bridge_state: routes={} active_routes={} surface_resets={} reconnect_resets={}",
            self.state.routes.len(),
            self.state.routes.values().filter(|route| route.active).count(),
            self.state.surface_reset_count,
            self.state.reconnect_reset_count,
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
                    format_bridge_business_entry(
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

impl RuntimeCustomPacketBridgeState {
    fn record_update(
        &mut self,
        now_ms: u64,
        route: &RuntimeCustomPacketBridgeRouteKey,
        entry: &RuntimeCustomPacketSurfaceSummaryEntry,
    ) {
        let state = self.routes.entry(route.clone()).or_default();
        state.apply_count = state.apply_count.saturating_add(1);
        state.active = true;
        state.last_stable_value = Some(entry.stable_value.clone());
        state.last_marker = entry.marker.clone();
        self.next_update_serial = self.next_update_serial.saturating_add(1);
        state.last_update_serial = self.next_update_serial;
        self.pending_lines.push_back(format!(
            "runtime_custom_packet_bridge_action: tick={now_ms}ms encoding={} key={:?} semantic={} apply_count={} value={:?} marker={}",
            encoding_label(route.encoding),
            route.key,
            semantic_label(route.semantic),
            state.apply_count,
            entry.stable_value,
            format_marker(entry.marker.as_ref()),
        ));
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
        self.pending_lines.push_back(format!(
            "runtime_custom_packet_bridge_reset: tick={now_ms}ms reason={reason} cleared_routes={cleared_routes}"
        ));
    }
}

fn parse_surface_update(line: &str) -> Option<ParsedSurfaceUpdate> {
    let prefix = "runtime_custom_packet_surface_update: encoding=";
    let rest = line.strip_prefix(prefix)?;
    let (encoding, rest) = rest.split_once(" key=")?;
    let (key, rest) = rest.split_once(" semantic=")?;
    let (semantic, _) = rest.split_once(" count=")?;
    Some(ParsedSurfaceUpdate {
        route: RuntimeCustomPacketBridgeRouteKey {
            key: parse_debug_string(key)?,
            encoding: parse_encoding(encoding)?,
            semantic: parse_semantic(semantic)?,
        },
    })
}

fn parse_surface_reset(line: &str) -> Option<ParsedSurfaceReset<'_>> {
    let prefix = "runtime_custom_packet_surface_reset: reason=";
    let rest = line.strip_prefix(prefix)?;
    let (reason, _) = rest.split_once(" cleared_routes=")?;
    Some(ParsedSurfaceReset { reason })
}

fn parse_debug_string(value: &str) -> Option<String> {
    if !(value.starts_with('"') && value.ends_with('"')) {
        return None;
    }
    let mut chars = value[1..value.len().saturating_sub(1)].chars();
    let mut parsed = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            parsed.push(ch);
            continue;
        }
        match chars.next()? {
            '\\' => parsed.push('\\'),
            '"' => parsed.push('"'),
            'n' => parsed.push('\n'),
            'r' => parsed.push('\r'),
            't' => parsed.push('\t'),
            other => parsed.push(other),
        }
    }
    Some(parsed)
}

fn parse_encoding(value: &str) -> Option<RuntimeCustomPacketSemanticEncoding> {
    match value {
        "text" => Some(RuntimeCustomPacketSemanticEncoding::Text),
        "binary" => Some(RuntimeCustomPacketSemanticEncoding::Binary),
        "logic" => Some(RuntimeCustomPacketSemanticEncoding::LogicData),
        _ => None,
    }
}

fn parse_semantic(value: &str) -> Option<RuntimeCustomPacketSemanticKind> {
    match value {
        "server_message" => Some(RuntimeCustomPacketSemanticKind::ServerMessage),
        "chat_message" => Some(RuntimeCustomPacketSemanticKind::ChatMessage),
        "hud_text" => Some(RuntimeCustomPacketSemanticKind::HudText),
        "announce" => Some(RuntimeCustomPacketSemanticKind::Announce),
        "clipboard" => Some(RuntimeCustomPacketSemanticKind::Clipboard),
        "open_uri" => Some(RuntimeCustomPacketSemanticKind::OpenUri),
        "world_pos" => Some(RuntimeCustomPacketSemanticKind::WorldPos),
        "build_pos" => Some(RuntimeCustomPacketSemanticKind::BuildPos),
        "unit_id" => Some(RuntimeCustomPacketSemanticKind::UnitId),
        "team" => Some(RuntimeCustomPacketSemanticKind::Team),
        "bool" => Some(RuntimeCustomPacketSemanticKind::Bool),
        "number" => Some(RuntimeCustomPacketSemanticKind::Number),
        _ => None,
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

fn format_bridge_business_entry(
    route: &RuntimeCustomPacketBridgeRouteKey,
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
        marker
            .map(|marker| format!("@{},{}", marker.x, marker.y))
            .unwrap_or_default()
    )
}

fn format_marker(marker: Option<&RuntimeCustomPacketOverlayMarker>) -> String {
    marker
        .map(|marker| format!("@{},{}", marker.x, marker.y))
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_tracks_surface_updates_and_resets() {
        let specs = vec![
            RuntimeCustomPacketSemanticSpec {
                key: "logic.pos".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                semantic: RuntimeCustomPacketSemanticKind::WorldPos,
            },
            RuntimeCustomPacketSemanticSpec {
                key: "custom.status".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
            },
        ];
        let mut bridge = RuntimeCustomPacketBridge::from_specs(&specs).unwrap();

        bridge.observe_surface_activity(
            42,
            &[
                "runtime_custom_packet_surface_update: encoding=logic key=\"logic.pos\" semantic=world_pos count=1 transport=reliable x=7 y=9 source=point2".to_string(),
                "runtime_custom_packet_surface_update: encoding=text key=\"custom.status\" semantic=hud_text count=1 message=\"wave ready\"".to_string(),
            ],
            &[
                RuntimeCustomPacketSurfaceSummaryEntry {
                    key: "logic.pos".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                    semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                    stable_value: "7,9".to_string(),
                    marker: Some(RuntimeCustomPacketOverlayMarker {
                        key: "logic.pos".to_string(),
                        encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                        semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                        x: 7.0,
                        y: 9.0,
                    }),
                },
                RuntimeCustomPacketSurfaceSummaryEntry {
                    key: "custom.status".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::Text,
                    semantic: RuntimeCustomPacketSemanticKind::HudText,
                    stable_value: "wave ready".to_string(),
                    marker: None,
                },
            ],
        );
        let lines = bridge.drain_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("runtime_custom_packet_bridge_action:"));
        assert!(lines[1].contains("runtime_custom_packet_bridge_action:"));
        assert_eq!(
            bridge.business_summary_text(4).as_deref(),
            Some(
                "text:custom.status(hud_text)#1=wave ready | logic:logic.pos(world_pos)#1=7,9@7,9"
            )
        );

        bridge.observe_surface_activity(
            43,
            &[
                "runtime_custom_packet_surface_reset: reason=world_data_begin cleared_routes=2"
                    .to_string(),
            ],
            &[],
        );
        let lines = bridge.drain_lines();
        assert_eq!(
            lines,
            vec![
                "runtime_custom_packet_bridge_reset: tick=43ms reason=surface:world_data_begin cleared_routes=2"
                    .to_string()
            ]
        );
        assert_eq!(bridge.business_summary_text(4), None);
    }

    #[test]
    fn bridge_reconnect_reset_clears_active_values_but_keeps_apply_counts() {
        let specs = vec![RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        }];
        let mut bridge = RuntimeCustomPacketBridge::from_specs(&specs).unwrap();
        bridge.observe_surface_activity(
            10,
            &[
                "runtime_custom_packet_surface_update: encoding=text key=\"custom.status\" semantic=hud_text count=1 message=\"wave ready\"".to_string(),
            ],
            &[RuntimeCustomPacketSurfaceSummaryEntry {
                key: "custom.status".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
                stable_value: "wave ready".to_string(),
                marker: None,
            }],
        );
        let _ = bridge.drain_lines();

        bridge.note_reconnect_reset(20, "redirect");
        assert_eq!(
            bridge.drain_lines(),
            vec![
                "runtime_custom_packet_bridge_reset: tick=20ms reason=reconnect:redirect cleared_routes=1"
                    .to_string()
            ]
        );
        assert_eq!(bridge.business_summary_text(4), None);

        bridge.observe_surface_activity(
            30,
            &[
                "runtime_custom_packet_surface_update: encoding=text key=\"custom.status\" semantic=hud_text count=1 message=\"wave resumed\"".to_string(),
            ],
            &[RuntimeCustomPacketSurfaceSummaryEntry {
                key: "custom.status".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
                stable_value: "wave resumed".to_string(),
                marker: None,
            }],
        );
        let lines = bridge.drain_lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("apply_count=2"));
        assert_eq!(
            bridge.business_summary_text(4).as_deref(),
            Some("text:custom.status(hud_text)#2=wave resumed")
        );
    }
}
