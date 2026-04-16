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
    reason: std::borrow::Cow<'a, str>,
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
            "runtime_custom_packet_bridge_reset: tick={now_ms}ms reason={reason:?} cleared_routes={cleared_routes}"
        ));
    }
}

fn parse_surface_update(line: &str) -> Option<ParsedSurfaceUpdate> {
    let prefix = "runtime_custom_packet_surface_update: encoding=";
    let rest = line.strip_prefix(prefix)?;
    let (encoding, rest) = rest.split_once(" key=")?;
    let (key, rest) = parse_debug_string_prefix(rest)?;
    let rest = rest.strip_prefix(" semantic=")?;
    let (semantic, rest) = rest.split_once(" count=")?;
    let (_, tail) = parse_decimal_prefix(rest)?;
    if !tail.is_empty() && !tail.starts_with(' ') {
        return None;
    }
    Some(ParsedSurfaceUpdate {
        route: RuntimeCustomPacketBridgeRouteKey {
            key,
            encoding: parse_encoding(encoding)?,
            semantic: parse_semantic(semantic)?,
        },
    })
}

fn parse_surface_reset(line: &str) -> Option<ParsedSurfaceReset<'_>> {
    let prefix = "runtime_custom_packet_surface_reset: reason=";
    let rest = line.strip_prefix(prefix)?;
    if rest.starts_with('"') {
        let (reason, rest) = parse_debug_string_prefix(rest)?;
        let rest = rest.strip_prefix(" cleared_routes=")?;
        let (_, tail) = parse_decimal_prefix(rest)?;
        if !tail.is_empty() {
            return None;
        }
        return Some(ParsedSurfaceReset {
            reason: std::borrow::Cow::Owned(reason),
        });
    }

    let (reason, _) = rest.rsplit_once(" cleared_routes=")?;
    let (_, tail) = parse_decimal_prefix(rest.rsplit_once(" cleared_routes=")?.1)?;
    if !tail.is_empty() {
        return None;
    }
    Some(ParsedSurfaceReset {
        reason: std::borrow::Cow::Borrowed(reason),
    })
}

fn parse_decimal_prefix(value: &str) -> Option<(&str, &str)> {
    let digits = value
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    (digits > 0).then_some(value.split_at(digits))
}

fn parse_debug_string_prefix(value: &str) -> Option<(String, &str)> {
    if !value.starts_with('"') {
        return None;
    }

    let bytes = value.as_bytes();
    let mut idx = 1usize;
    let mut escaped = false;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if escaped {
            escaped = false;
            idx += 1;
            continue;
        }

        match byte {
            b'\\' => {
                escaped = true;
                idx += 1;
            }
            b'"' => {
                let parsed = parse_debug_string(&value[..=idx])?;
                return Some((parsed, &value[idx + 1..]));
            }
            _ => idx += 1,
        }
    }

    None
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
            '0' => parsed.push('\0'),
            'n' => parsed.push('\n'),
            'r' => parsed.push('\r'),
            't' => parsed.push('\t'),
            'u' => parsed.push(parse_unicode_escape(&mut chars)?),
            _ => return None,
        }
    }
    Some(parsed)
}

fn parse_unicode_escape(chars: &mut std::str::Chars<'_>) -> Option<char> {
    if chars.next()? != '{' {
        return None;
    }

    let mut value = 0u32;
    let mut digit_count = 0usize;
    loop {
        let ch = chars.next()?;
        if ch == '}' {
            break;
        }
        let digit = ch.to_digit(16)?;
        value = value.checked_mul(16)?.checked_add(digit)?;
        digit_count += 1;
    }

    if digit_count == 0 {
        return None;
    }

    char::from_u32(value)
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
                "runtime_custom_packet_surface_reset: reason=\"world_data_begin\" cleared_routes=2"
                    .to_string(),
            ],
            &[],
        );
        let lines = bridge.drain_lines();
        assert_eq!(
            lines,
            vec![
                "runtime_custom_packet_bridge_reset: tick=43ms reason=\"surface:world_data_begin\" cleared_routes=2"
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
                "runtime_custom_packet_bridge_reset: tick=20ms reason=\"reconnect:redirect\" cleared_routes=1"
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

    #[test]
    fn bridge_reset_counters_stay_isolated_by_path() {
        let specs = vec![RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        }];
        let mut bridge = RuntimeCustomPacketBridge::from_specs(&specs).unwrap();

        bridge.observe_surface_activity(
            10,
            &[String::from(
                "runtime_custom_packet_surface_reset: reason=\"world_data_begin\" cleared_routes=1",
            )],
            &[],
        );
        assert_eq!(
            bridge.summary_lines().last().map(String::as_str),
            Some("runtime_custom_packet_bridge_state: routes=1 active_routes=0 surface_resets=1 reconnect_resets=0")
        );

        bridge.note_reconnect_reset(20, "redirect");
        assert_eq!(
            bridge.summary_lines().last().map(String::as_str),
            Some("runtime_custom_packet_bridge_state: routes=1 active_routes=0 surface_resets=1 reconnect_resets=1")
        );

        bridge.observe_surface_activity(
            30,
            &[String::from(
                "runtime_custom_packet_surface_reset: reason=\"world_data_begin\" cleared_routes=1",
            )],
            &[],
        );
        assert_eq!(
            bridge.summary_lines().last().map(String::as_str),
            Some("runtime_custom_packet_bridge_state: routes=1 active_routes=0 surface_resets=2 reconnect_resets=1")
        );
    }

    #[test]
    fn bridge_reports_missing_surface_entry_diagnostic() {
        let specs = vec![RuntimeCustomPacketSemanticSpec {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
        }];
        let mut bridge = RuntimeCustomPacketBridge::from_specs(&specs).unwrap();

        bridge.observe_surface_activity(
            99,
            &[String::from(
                "runtime_custom_packet_surface_update: encoding=logic key=\"logic.pos\" semantic=world_pos count=1 transport=reliable x=7 y=9 source=point2",
            )],
            &[],
        );

        assert_eq!(
            bridge.drain_lines(),
            vec![
                "runtime_custom_packet_bridge_missing_surface_entry: tick=99ms encoding=logic key=\"logic.pos\" semantic=world_pos"
                    .to_string()
            ]
        );
        assert_eq!(bridge.business_summary_text(4), None);
    }

    #[test]
    fn parse_debug_string_decodes_unicode_escape_sequences() {
        assert_eq!(
            parse_debug_string(r#""emoji-\u{1f680}""#),
            Some("emoji-🚀".to_string())
        );
        assert_eq!(
            parse_surface_update(
                "runtime_custom_packet_surface_update: encoding=text key=\"team-\\u{3a9}\" semantic=hud_text count=1 message=\"ok\""
            )
            .map(|update| update.route.key),
            Some("team-Ω".to_string())
        );
    }

    #[test]
    fn parse_debug_string_decodes_nul_escape() {
        assert_eq!(parse_debug_string(r#""a\0b""#), Some("a\0b".to_string()));
    }

    #[test]
    fn parse_debug_string_rejects_unknown_escape_sequences() {
        assert_eq!(parse_debug_string(r#""a\qb""#), None);
        assert_eq!(parse_debug_string(r#""\q""#), None);
    }

    #[test]
    fn parse_debug_string_rejects_trailing_backslash() {
        assert_eq!(parse_debug_string(r#""broken\"#), None);
        assert_eq!(parse_debug_string(r#""also broken\"#), None);
    }

    #[test]
    fn parse_surface_update_handles_separator_substrings_inside_quoted_key() {
        assert_eq!(
            parse_surface_update(
                "runtime_custom_packet_surface_update: encoding=text key=\"alpha key= beta semantic= gamma count= delta cleared_routes= epsilon\" semantic=hud_text count=1 message=\"ok\""
            )
            .map(|update| update.route.key),
            Some("alpha key= beta semantic= gamma count= delta cleared_routes= epsilon".to_string())
        );
        assert_eq!(
            parse_surface_reset(
                "runtime_custom_packet_surface_reset: reason=\"route cleared_routes=ignored\" cleared_routes=3"
            )
            .map(|reset| reset.reason),
            Some(std::borrow::Cow::Borrowed("route cleared_routes=ignored"))
        );
    }

    #[test]
    fn parse_encoding_and_semantic_map_known_and_unknown_tokens() {
        assert_eq!(
            parse_encoding("text"),
            Some(RuntimeCustomPacketSemanticEncoding::Text)
        );
        assert_eq!(
            parse_encoding("binary"),
            Some(RuntimeCustomPacketSemanticEncoding::Binary)
        );
        assert_eq!(
            parse_encoding("logic"),
            Some(RuntimeCustomPacketSemanticEncoding::LogicData)
        );
        assert_eq!(parse_encoding("json"), None);

        assert_eq!(
            parse_semantic("server_message"),
            Some(RuntimeCustomPacketSemanticKind::ServerMessage)
        );
        assert_eq!(
            parse_semantic("world_pos"),
            Some(RuntimeCustomPacketSemanticKind::WorldPos)
        );
        assert_eq!(
            parse_semantic("number"),
            Some(RuntimeCustomPacketSemanticKind::Number)
        );
        assert_eq!(parse_semantic("unknown"), None);
    }

    #[test]
    fn parse_surface_reset_rejects_trailing_junk_and_unterminated_quotes() {
        assert!(
            parse_surface_reset(
                "runtime_custom_packet_surface_reset: reason=\"world_data_begin\" cleared_routes=1junk"
            )
            .is_none()
        );
        assert!(
            parse_surface_reset(
                r#"runtime_custom_packet_surface_reset: reason="world_data_begin cleared_routes=1"#
            )
            .is_none()
        );

        let specs = vec![RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        }];
        let mut bridge = RuntimeCustomPacketBridge::from_specs(&specs).unwrap();
        bridge.observe_surface_activity(
            7,
            &[String::from(
                "runtime_custom_packet_surface_reset: reason=\"world_data_begin\" cleared_routes=1junk",
            )],
            &[],
        );

        assert!(bridge.drain_lines().is_empty());
        assert_eq!(bridge.business_summary_text(4), None);
        assert_eq!(
            bridge.summary_lines().last().map(String::as_str),
            Some("runtime_custom_packet_bridge_state: routes=1 active_routes=0 surface_resets=0 reconnect_resets=0")
        );
    }

    #[test]
    fn parse_surface_update_rejects_trailing_junk_and_unterminated_quotes() {
        assert!(
            parse_surface_update(
                "runtime_custom_packet_surface_update: encoding=text key=\"custom.status\" semantic=hud_text count=1junk message=\"ok\""
            )
            .is_none()
        );
        assert!(
            parse_surface_update(
                r#"runtime_custom_packet_surface_update: encoding=text key="custom.status semantic=hud_text count=1 message="ok""#
            )
            .is_none()
        );

        let specs = vec![RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        }];
        let mut bridge = RuntimeCustomPacketBridge::from_specs(&specs).unwrap();
        bridge.observe_surface_activity(
            7,
            &[String::from(
                "runtime_custom_packet_surface_update: encoding=text key=\"custom.status\" semantic=hud_text count=1junk message=\"ok\"",
            )],
            &[RuntimeCustomPacketSurfaceSummaryEntry {
                key: "custom.status".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
                stable_value: "ok".to_string(),
                marker: None,
            }],
        );

        assert!(bridge.drain_lines().is_empty());
        assert_eq!(bridge.business_summary_text(4), None);
        assert_eq!(
            bridge.summary_lines().last().map(String::as_str),
            Some("runtime_custom_packet_bridge_state: routes=1 active_routes=0 surface_resets=0 reconnect_resets=0")
        );
    }
}
