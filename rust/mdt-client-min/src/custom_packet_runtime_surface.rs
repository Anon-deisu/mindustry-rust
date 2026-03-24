use crate::client_session::{ClientLogicDataTransport, ClientSession, ClientSessionEvent};
use crate::custom_packet_runtime::{
    RuntimeCustomPacketSemanticEncoding, RuntimeCustomPacketSemanticKind,
    RuntimeCustomPacketSemanticSpec,
};
use mdt_typeio::{unpack_point2, TypeIoObject};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

#[derive(Debug)]
pub struct RuntimeCustomPacketSurface {
    state: Rc<RefCell<RuntimeCustomPacketSurfaceState>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCustomPacketOverlayMarker {
    pub key: String,
    pub encoding: RuntimeCustomPacketSemanticEncoding,
    pub semantic: RuntimeCustomPacketSemanticKind,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCustomPacketSurfaceSummaryEntry {
    pub key: String,
    pub encoding: RuntimeCustomPacketSemanticEncoding,
    pub semantic: RuntimeCustomPacketSemanticKind,
    pub stable_value: String,
    pub marker: Option<RuntimeCustomPacketOverlayMarker>,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketSurfaceState {
    text_routes: BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
    binary_routes: BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
    logic_routes: BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
    pending_lines: VecDeque<String>,
    next_update_serial: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimeCustomPacketSurfaceRouteState {
    semantic: RuntimeCustomPacketSemanticKind,
    handler_count: usize,
    event_reliable_count: usize,
    event_unreliable_count: usize,
    decode_error_count: usize,
    last_overlay_value: Option<String>,
    last_stable_value: Option<String>,
    last_marker: Option<RuntimeCustomPacketOverlayMarker>,
    last_update_serial: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct RenderedSurfaceValue {
    detail: String,
    stable_value: String,
    overlay_value: String,
    marker: Option<RuntimeCustomPacketOverlayMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OverlayEntry {
    serial: u64,
    key: String,
    encoding: RuntimeCustomPacketSemanticEncoding,
    overlay_value: String,
}

#[derive(Debug, Clone, PartialEq)]
struct SummaryEntry {
    serial: u64,
    entry: RuntimeCustomPacketSurfaceSummaryEntry,
}

#[derive(Debug, Clone, PartialEq)]
struct OverlayMarkerEntry {
    serial: u64,
    marker: RuntimeCustomPacketOverlayMarker,
}

impl RuntimeCustomPacketSurface {
    pub fn observe_events(&self, events: &[ClientSessionEvent]) {
        self.state.borrow_mut().observe_events(events);
    }

    pub fn drain_lines(&self) -> Vec<String> {
        self.state.borrow_mut().drain_lines()
    }

    pub fn summary_lines(&self) -> Vec<String> {
        self.state.borrow().summary_lines()
    }

    pub fn overlay_summary_text(&self, max_entries: usize) -> Option<String> {
        self.state.borrow().overlay_summary_text(max_entries)
    }

    pub fn overlay_markers(&self, max_entries: usize) -> Vec<RuntimeCustomPacketOverlayMarker> {
        self.state.borrow().overlay_markers(max_entries)
    }

    pub fn latest_summary_entries(
        &self,
        max_entries: usize,
    ) -> Vec<RuntimeCustomPacketSurfaceSummaryEntry> {
        self.state.borrow().latest_summary_entries(max_entries)
    }
}

impl RuntimeCustomPacketSurfaceState {
    fn register(&mut self, spec: &RuntimeCustomPacketSemanticSpec) {
        let routes = match spec.encoding {
            RuntimeCustomPacketSemanticEncoding::Text => self.text_routes.entry(spec.key.clone()),
            RuntimeCustomPacketSemanticEncoding::Binary => {
                self.binary_routes.entry(spec.key.clone())
            }
            RuntimeCustomPacketSemanticEncoding::LogicData => {
                self.logic_routes.entry(spec.key.clone())
            }
        }
        .or_default();
        routes.push(RuntimeCustomPacketSurfaceRouteState {
            semantic: spec.semantic,
            handler_count: 0,
            event_reliable_count: 0,
            event_unreliable_count: 0,
            decode_error_count: 0,
            last_overlay_value: None,
            last_stable_value: None,
            last_marker: None,
            last_update_serial: 0,
        });
    }

    fn record_text_handler(&mut self, key: &str, text: &str) {
        let Some(routes) = self.text_routes.get_mut(key) else {
            return;
        };
        let mut next_update_serial = self.next_update_serial;
        let mut queued_lines = Vec::with_capacity(routes.len());
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            match render_text_surface(route.semantic, text) {
                Ok(rendered) => {
                    next_update_serial = next_update_serial.saturating_add(1);
                    route.last_overlay_value = Some(rendered.overlay_value.clone());
                    route.last_stable_value = Some(rendered.stable_value.clone());
                    route.last_marker = attach_overlay_marker(
                        key,
                        RuntimeCustomPacketSemanticEncoding::Text,
                        rendered.marker.clone(),
                    );
                    route.last_update_serial = next_update_serial;
                    queued_lines.push(format!(
                        "runtime_custom_packet_surface_update: encoding={} key={key:?} semantic={} count={} {}",
                        encoding_label(RuntimeCustomPacketSemanticEncoding::Text),
                        semantic_label(route.semantic),
                        route.handler_count,
                        rendered.detail
                    ));
                }
                Err(reason) => {
                    route.decode_error_count = route.decode_error_count.saturating_add(1);
                    queued_lines.push(format!(
                        "runtime_custom_packet_surface_decode_error: encoding={} key={key:?} semantic={} count={} reason={reason:?} preview={:?}",
                        encoding_label(RuntimeCustomPacketSemanticEncoding::Text),
                        semantic_label(route.semantic),
                        route.decode_error_count,
                        truncate_for_preview(&text.escape_default().to_string(), 96),
                    ));
                }
            }
        }
        self.next_update_serial = next_update_serial;
        self.pending_lines.extend(queued_lines);
    }

    fn record_binary_handler(&mut self, key: &str, bytes: &[u8]) {
        let Some(routes) = self.binary_routes.get_mut(key) else {
            return;
        };
        let text = std::str::from_utf8(bytes).ok();
        let mut next_update_serial = self.next_update_serial;
        let mut queued_lines = Vec::with_capacity(routes.len());
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            let Some(text) = text else {
                route.decode_error_count = route.decode_error_count.saturating_add(1);
                queued_lines.push(format!(
                    "runtime_custom_packet_surface_decode_error: encoding=binary key={key:?} semantic={} count={} reason=\"invalid_utf8\" len={} hex_prefix={}",
                    semantic_label(route.semantic),
                    route.decode_error_count,
                    bytes.len(),
                    encode_hex_prefix(bytes)
                ));
                continue;
            };
            match render_text_surface(route.semantic, text) {
                Ok(rendered) => {
                    next_update_serial = next_update_serial.saturating_add(1);
                    route.last_overlay_value = Some(rendered.overlay_value.clone());
                    route.last_stable_value = Some(rendered.stable_value.clone());
                    route.last_marker = attach_overlay_marker(
                        key,
                        RuntimeCustomPacketSemanticEncoding::Binary,
                        rendered.marker.clone(),
                    );
                    route.last_update_serial = next_update_serial;
                    queued_lines.push(format!(
                        "runtime_custom_packet_surface_update: encoding={} key={key:?} semantic={} count={} {}",
                        encoding_label(RuntimeCustomPacketSemanticEncoding::Binary),
                        semantic_label(route.semantic),
                        route.handler_count,
                        rendered.detail
                    ));
                }
                Err(reason) => {
                    route.decode_error_count = route.decode_error_count.saturating_add(1);
                    queued_lines.push(format!(
                        "runtime_custom_packet_surface_decode_error: encoding={} key={key:?} semantic={} count={} reason={reason:?} preview={:?}",
                        encoding_label(RuntimeCustomPacketSemanticEncoding::Binary),
                        semantic_label(route.semantic),
                        route.decode_error_count,
                        truncate_for_preview(&text.escape_default().to_string(), 96),
                    ));
                }
            }
        }
        self.next_update_serial = next_update_serial;
        self.pending_lines.extend(queued_lines);
    }

    fn record_logic_data_handler(
        &mut self,
        key: &str,
        transport: ClientLogicDataTransport,
        value: &TypeIoObject,
    ) {
        let Some(routes) = self.logic_routes.get_mut(key) else {
            return;
        };
        let mut next_update_serial = self.next_update_serial;
        let mut queued_lines = Vec::with_capacity(routes.len());
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            match render_logic_surface(route.semantic, value) {
                Ok(mut rendered) => {
                    rendered.detail = format!(
                        "transport={} {}",
                        logic_data_transport_label(transport),
                        rendered.detail
                    );
                    next_update_serial = next_update_serial.saturating_add(1);
                    route.last_overlay_value = Some(rendered.overlay_value.clone());
                    route.last_stable_value = Some(rendered.stable_value.clone());
                    route.last_marker = attach_overlay_marker(
                        key,
                        RuntimeCustomPacketSemanticEncoding::LogicData,
                        rendered.marker.clone(),
                    );
                    route.last_update_serial = next_update_serial;
                    queued_lines.push(format!(
                        "runtime_custom_packet_surface_update: encoding={} key={key:?} semantic={} count={} {}",
                        encoding_label(RuntimeCustomPacketSemanticEncoding::LogicData),
                        semantic_label(route.semantic),
                        route.handler_count,
                        rendered.detail
                    ));
                }
                Err(reason) => {
                    route.decode_error_count = route.decode_error_count.saturating_add(1);
                    queued_lines.push(format!(
                        "runtime_custom_packet_surface_decode_error: encoding=logic key={key:?} semantic={} count={} transport={} reason={reason:?} kind={:?} preview={:?}",
                        semantic_label(route.semantic),
                        route.decode_error_count,
                        logic_data_transport_label(transport),
                        value.kind(),
                        truncate_for_preview(&format!("{value:?}"), 96)
                    ));
                }
            }
        }
        self.next_update_serial = next_update_serial;
        self.pending_lines.extend(queued_lines);
    }

    fn observe_events(&mut self, events: &[ClientSessionEvent]) {
        if events
            .iter()
            .any(|event| matches!(event, ClientSessionEvent::WorldDataBegin))
        {
            self.clear_last_values("world_data_begin");
        }
        for event in events {
            match event {
                ClientSessionEvent::ClientPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerPacketReliable { packet_type, .. } => {
                    self.record_event(RuntimeCustomPacketSemanticEncoding::Text, packet_type, true);
                }
                ClientSessionEvent::ClientPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerPacketUnreliable { packet_type, .. } => self
                    .record_event(
                        RuntimeCustomPacketSemanticEncoding::Text,
                        packet_type,
                        false,
                    ),
                ClientSessionEvent::ClientBinaryPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketReliable { packet_type, .. } => self
                    .record_event(
                        RuntimeCustomPacketSemanticEncoding::Binary,
                        packet_type,
                        true,
                    ),
                ClientSessionEvent::ClientBinaryPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketUnreliable { packet_type, .. } => self
                    .record_event(
                        RuntimeCustomPacketSemanticEncoding::Binary,
                        packet_type,
                        false,
                    ),
                ClientSessionEvent::ClientLogicDataReliable { channel, .. } => self.record_event(
                    RuntimeCustomPacketSemanticEncoding::LogicData,
                    channel,
                    true,
                ),
                ClientSessionEvent::ClientLogicDataUnreliable { channel, .. } => self.record_event(
                    RuntimeCustomPacketSemanticEncoding::LogicData,
                    channel,
                    false,
                ),
                _ => {}
            }
        }
    }

    fn record_event(
        &mut self,
        encoding: RuntimeCustomPacketSemanticEncoding,
        key: &str,
        reliable: bool,
    ) {
        let routes = match encoding {
            RuntimeCustomPacketSemanticEncoding::Text => self.text_routes.get_mut(key),
            RuntimeCustomPacketSemanticEncoding::Binary => self.binary_routes.get_mut(key),
            RuntimeCustomPacketSemanticEncoding::LogicData => self.logic_routes.get_mut(key),
        };
        let Some(routes) = routes else {
            return;
        };
        for route in routes {
            if reliable {
                route.event_reliable_count = route.event_reliable_count.saturating_add(1);
            } else {
                route.event_unreliable_count = route.event_unreliable_count.saturating_add(1);
            }
        }
    }

    fn clear_last_values(&mut self, reason: &str) {
        let mut cleared = 0usize;
        for routes in self
            .text_routes
            .values_mut()
            .chain(self.binary_routes.values_mut())
            .chain(self.logic_routes.values_mut())
        {
            for route in routes {
                if route.last_overlay_value.take().is_some() {
                    cleared = cleared.saturating_add(1);
                }
                route.last_stable_value = None;
                route.last_marker = None;
                route.last_update_serial = 0;
            }
        }
        if cleared > 0 {
            self.pending_lines.push_back(format!(
                "runtime_custom_packet_surface_reset: reason={reason} cleared_routes={cleared}"
            ));
        }
    }

    fn drain_lines(&mut self) -> Vec<String> {
        self.pending_lines.drain(..).collect()
    }

    fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        append_summary_lines(
            &mut lines,
            RuntimeCustomPacketSemanticEncoding::Text,
            &self.text_routes,
        );
        append_summary_lines(
            &mut lines,
            RuntimeCustomPacketSemanticEncoding::Binary,
            &self.binary_routes,
        );
        append_summary_lines(
            &mut lines,
            RuntimeCustomPacketSemanticEncoding::LogicData,
            &self.logic_routes,
        );
        lines
    }

    fn overlay_summary_text(&self, max_entries: usize) -> Option<String> {
        if max_entries == 0 {
            return None;
        }
        let mut entries = Vec::new();
        collect_overlay_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::Text,
            &self.text_routes,
        );
        collect_overlay_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::Binary,
            &self.binary_routes,
        );
        collect_overlay_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::LogicData,
            &self.logic_routes,
        );
        if entries.is_empty() {
            return None;
        }
        entries.sort_by(|left, right| {
            right
                .serial
                .cmp(&left.serial)
                .then_with(|| left.key.cmp(&right.key))
                .then_with(|| encoding_label(left.encoding).cmp(encoding_label(right.encoding)))
        });
        let text = entries
            .into_iter()
            .take(max_entries)
            .map(|entry| {
                format!(
                    "{}:{}={}",
                    encoding_overlay_prefix(entry.encoding),
                    entry.key,
                    entry.overlay_value
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        (!text.is_empty()).then_some(text)
    }

    fn overlay_markers(&self, max_entries: usize) -> Vec<RuntimeCustomPacketOverlayMarker> {
        if max_entries == 0 {
            return Vec::new();
        }
        let mut entries = Vec::new();
        collect_overlay_marker_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::Text,
            &self.text_routes,
        );
        collect_overlay_marker_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::Binary,
            &self.binary_routes,
        );
        collect_overlay_marker_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::LogicData,
            &self.logic_routes,
        );
        entries.sort_by(|left, right| {
            right
                .serial
                .cmp(&left.serial)
                .then_with(|| left.marker.key.cmp(&right.marker.key))
                .then_with(|| {
                    encoding_label(left.marker.encoding).cmp(encoding_label(right.marker.encoding))
                })
        });
        entries
            .into_iter()
            .take(max_entries)
            .map(|entry| entry.marker)
            .collect()
    }

    fn latest_summary_entries(
        &self,
        max_entries: usize,
    ) -> Vec<RuntimeCustomPacketSurfaceSummaryEntry> {
        if max_entries == 0 {
            return Vec::new();
        }
        let mut entries = Vec::new();
        collect_summary_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::Text,
            &self.text_routes,
        );
        collect_summary_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::Binary,
            &self.binary_routes,
        );
        collect_summary_entries(
            &mut entries,
            RuntimeCustomPacketSemanticEncoding::LogicData,
            &self.logic_routes,
        );
        entries.sort_by(|left, right| {
            right
                .serial
                .cmp(&left.serial)
                .then_with(|| left.entry.key.cmp(&right.entry.key))
                .then_with(|| {
                    encoding_label(left.entry.encoding).cmp(encoding_label(right.entry.encoding))
                })
                .then_with(|| {
                    semantic_label(left.entry.semantic).cmp(semantic_label(right.entry.semantic))
                })
        });
        entries
            .into_iter()
            .take(max_entries)
            .map(|entry| entry.entry)
            .collect()
    }
}

pub fn install_runtime_custom_packet_surface(
    session: &mut ClientSession,
    specs: &[RuntimeCustomPacketSemanticSpec],
) -> Option<RuntimeCustomPacketSurface> {
    if specs.is_empty() {
        return None;
    }

    let state = Rc::new(RefCell::new(RuntimeCustomPacketSurfaceState::default()));
    for spec in specs {
        state.borrow_mut().register(spec);
        match spec.encoding {
            RuntimeCustomPacketSemanticEncoding::Text => {
                let key = spec.key.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_packet_handler(spec.key.clone(), move |contents| {
                    shared_state
                        .borrow_mut()
                        .record_text_handler(&key, contents);
                });
            }
            RuntimeCustomPacketSemanticEncoding::Binary => {
                let key = spec.key.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_binary_packet_handler(spec.key.clone(), move |contents| {
                    shared_state
                        .borrow_mut()
                        .record_binary_handler(&key, contents);
                });
            }
            RuntimeCustomPacketSemanticEncoding::LogicData => {
                let key = spec.key.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_logic_data_handler(spec.key.clone(), move |transport, value| {
                    shared_state
                        .borrow_mut()
                        .record_logic_data_handler(&key, transport, value);
                });
            }
        }
    }

    Some(RuntimeCustomPacketSurface { state })
}

fn append_summary_lines(
    lines: &mut Vec<String>,
    encoding: RuntimeCustomPacketSemanticEncoding,
    routes: &BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
) {
    for (key, route_states) in routes {
        for route in route_states {
            let event_total = route
                .event_reliable_count
                .saturating_add(route.event_unreliable_count);
            let parity = if route.handler_count == event_total {
                "ok"
            } else {
                "mismatch"
            };
            lines.push(format!(
                "runtime_custom_packet_surface_summary: encoding={} key={key:?} semantic={} count={} event_reliable={} event_unreliable={} event_total={} decode_errors={} parity={parity} last={:?}",
                encoding_label(encoding),
                semantic_label(route.semantic),
                route.handler_count,
                route.event_reliable_count,
                route.event_unreliable_count,
                event_total,
                route.decode_error_count,
                route.last_stable_value
            ));
        }
    }
}

fn collect_overlay_entries(
    entries: &mut Vec<OverlayEntry>,
    encoding: RuntimeCustomPacketSemanticEncoding,
    routes: &BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
) {
    for (key, route_states) in routes {
        for route in route_states {
            let Some(overlay_value) = route.last_overlay_value.as_ref() else {
                continue;
            };
            if route.last_update_serial == 0 {
                continue;
            }
            entries.push(OverlayEntry {
                serial: route.last_update_serial,
                key: key.clone(),
                encoding,
                overlay_value: overlay_value.clone(),
            });
        }
    }
}

fn collect_overlay_marker_entries(
    entries: &mut Vec<OverlayMarkerEntry>,
    _encoding: RuntimeCustomPacketSemanticEncoding,
    routes: &BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
) {
    for route_states in routes.values() {
        for route in route_states {
            let Some(marker) = route.last_marker.as_ref() else {
                continue;
            };
            if route.last_update_serial == 0 {
                continue;
            }
            entries.push(OverlayMarkerEntry {
                serial: route.last_update_serial,
                marker: marker.clone(),
            });
        }
    }
}

fn collect_summary_entries(
    entries: &mut Vec<SummaryEntry>,
    encoding: RuntimeCustomPacketSemanticEncoding,
    routes: &BTreeMap<String, Vec<RuntimeCustomPacketSurfaceRouteState>>,
) {
    for (key, route_states) in routes {
        for route in route_states {
            let Some(stable_value) = route.last_stable_value.as_ref() else {
                continue;
            };
            if route.last_update_serial == 0 {
                continue;
            }
            entries.push(SummaryEntry {
                serial: route.last_update_serial,
                entry: RuntimeCustomPacketSurfaceSummaryEntry {
                    key: key.clone(),
                    encoding,
                    semantic: route.semantic,
                    stable_value: stable_value.clone(),
                    marker: route.last_marker.clone(),
                },
            });
        }
    }
}

fn attach_overlay_marker(
    key: &str,
    encoding: RuntimeCustomPacketSemanticEncoding,
    marker: Option<RuntimeCustomPacketOverlayMarker>,
) -> Option<RuntimeCustomPacketOverlayMarker> {
    marker.map(|mut marker| {
        marker.key = key.to_string();
        marker.encoding = encoding;
        marker
    })
}

fn render_text_surface(
    semantic: RuntimeCustomPacketSemanticKind,
    text: &str,
) -> Result<RenderedSurfaceValue, &'static str> {
    match semantic {
        RuntimeCustomPacketSemanticKind::ServerMessage
        | RuntimeCustomPacketSemanticKind::ChatMessage
        | RuntimeCustomPacketSemanticKind::HudText
        | RuntimeCustomPacketSemanticKind::Announce
        | RuntimeCustomPacketSemanticKind::Clipboard
        | RuntimeCustomPacketSemanticKind::OpenUri => render_message_like_surface(text),
        RuntimeCustomPacketSemanticKind::WorldPos => render_text_world_pos(text),
        RuntimeCustomPacketSemanticKind::BuildPos => render_text_build_pos(text),
        RuntimeCustomPacketSemanticKind::UnitId => render_text_i32(text, "unit_id"),
        RuntimeCustomPacketSemanticKind::Team => render_text_u8(text, "team"),
        RuntimeCustomPacketSemanticKind::Bool => render_text_bool(text),
        RuntimeCustomPacketSemanticKind::Number => render_text_number(text),
    }
}

fn render_logic_surface(
    semantic: RuntimeCustomPacketSemanticKind,
    value: &TypeIoObject,
) -> Result<RenderedSurfaceValue, &'static str> {
    match semantic {
        RuntimeCustomPacketSemanticKind::ServerMessage
        | RuntimeCustomPacketSemanticKind::ChatMessage
        | RuntimeCustomPacketSemanticKind::HudText
        | RuntimeCustomPacketSemanticKind::Announce
        | RuntimeCustomPacketSemanticKind::Clipboard
        | RuntimeCustomPacketSemanticKind::OpenUri => {
            render_message_like_surface(&extract_logic_string(value).ok_or("no_string_payload")?)
        }
        RuntimeCustomPacketSemanticKind::WorldPos => extract_logic_world_pos(value),
        RuntimeCustomPacketSemanticKind::BuildPos => extract_logic_build_pos(value),
        RuntimeCustomPacketSemanticKind::UnitId => extract_logic_unit_id(value),
        RuntimeCustomPacketSemanticKind::Team => extract_logic_team(value),
        RuntimeCustomPacketSemanticKind::Bool => extract_logic_bool(value),
        RuntimeCustomPacketSemanticKind::Number => extract_logic_number(value),
    }
}

fn render_message_like_surface(text: &str) -> Result<RenderedSurfaceValue, &'static str> {
    if text.is_empty() {
        return Err("empty_message");
    }
    let escaped = text.escape_default().to_string();
    Ok(RenderedSurfaceValue {
        detail: format!("message={:?}", truncate_for_preview(&escaped, 96)),
        stable_value: text.to_string(),
        overlay_value: truncate_for_preview(&escaped, 32),
        marker: None,
    })
}

fn render_text_world_pos(text: &str) -> Result<RenderedSurfaceValue, &'static str> {
    let (x, y, source) = parse_text_world_pos(text).ok_or("invalid_world_pos")?;
    let overlay = format_compact_world_pos(x, y);
    Ok(RenderedSurfaceValue {
        detail: format!("x={x} y={y} source={source}"),
        stable_value: format!("{x},{y}"),
        overlay_value: overlay,
        marker: Some(position_overlay_marker(
            RuntimeCustomPacketSemanticKind::WorldPos,
            x as f32,
            y as f32,
        )),
    })
}

fn render_text_build_pos(text: &str) -> Result<RenderedSurfaceValue, &'static str> {
    let build_pos = parse_text_i32(text).ok_or("invalid_integer")?;
    let (world_x, world_y) = build_pos_world_pos(build_pos);
    Ok(RenderedSurfaceValue {
        detail: format!(
            "build_pos={build_pos} tile_x={} tile_y={}",
            world_x / 8.0,
            world_y / 8.0
        ),
        stable_value: build_pos.to_string(),
        overlay_value: build_pos.to_string(),
        marker: Some(position_overlay_marker(
            RuntimeCustomPacketSemanticKind::BuildPos,
            world_x,
            world_y,
        )),
    })
}

fn render_text_i32(text: &str, label: &str) -> Result<RenderedSurfaceValue, &'static str> {
    let value = parse_text_i32(text).ok_or("invalid_integer")?;
    Ok(RenderedSurfaceValue {
        detail: format!("{label}={value}"),
        stable_value: value.to_string(),
        overlay_value: value.to_string(),
        marker: None,
    })
}

fn render_text_u8(text: &str, label: &str) -> Result<RenderedSurfaceValue, &'static str> {
    let value = parse_text_u8(text).ok_or("invalid_u8")?;
    Ok(RenderedSurfaceValue {
        detail: format!("{label}={value}"),
        stable_value: value.to_string(),
        overlay_value: value.to_string(),
        marker: None,
    })
}

fn render_text_bool(text: &str) -> Result<RenderedSurfaceValue, &'static str> {
    let value = parse_text_bool(text).ok_or("invalid_bool")?;
    Ok(RenderedSurfaceValue {
        detail: format!("value={value}"),
        stable_value: value.to_string(),
        overlay_value: value.to_string(),
        marker: None,
    })
}

fn render_text_number(text: &str) -> Result<RenderedSurfaceValue, &'static str> {
    let value = parse_text_f64(text).ok_or("invalid_number")?;
    let rendered = value.to_string();
    Ok(RenderedSurfaceValue {
        detail: format!("value={rendered}"),
        stable_value: rendered.clone(),
        overlay_value: rendered,
        marker: None,
    })
}

fn extract_logic_string(value: &TypeIoObject) -> Option<String> {
    match value {
        TypeIoObject::String(Some(text)) => Some(text.clone()),
        TypeIoObject::ObjectArray(_) => value
            .find_first_dfs(|object| matches!(object, TypeIoObject::String(Some(_))))
            .and_then(|matched| match matched.value {
                TypeIoObject::String(Some(text)) => Some(text.clone()),
                _ => None,
            }),
        _ => None,
    }
}

fn extract_logic_world_pos(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let direct = match value {
        TypeIoObject::Point2 { x, y } => Some((*x as f64, *y as f64, "point2")),
        TypeIoObject::Vec2 { x, y } => Some((*x as f64, *y as f64, "vec2")),
        TypeIoObject::PackedPoint2Array(values) => values.first().map(|packed| {
            let (x, y) = unpack_point2(*packed);
            (x as f64, y as f64, "point2_array_first")
        }),
        TypeIoObject::Vec2Array(values) => values
            .first()
            .map(|(x, y)| (*x as f64, *y as f64, "vec2_array_first")),
        TypeIoObject::ObjectArray(_) => None,
        _ => None,
    };
    let (x, y, source) = match direct {
        Some(value) => value,
        None => value
            .find_first_dfs(|object| match object {
                TypeIoObject::Point2 { .. } | TypeIoObject::Vec2 { .. } => true,
                TypeIoObject::PackedPoint2Array(values) => !values.is_empty(),
                TypeIoObject::Vec2Array(values) => !values.is_empty(),
                _ => false,
            })
            .and_then(|matched| match matched.value {
                TypeIoObject::Point2 { x, y } => Some((*x as f64, *y as f64, "point2_nested")),
                TypeIoObject::Vec2 { x, y } => Some((*x as f64, *y as f64, "vec2_nested")),
                TypeIoObject::PackedPoint2Array(values) => values.first().map(|packed| {
                    let (x, y) = unpack_point2(*packed);
                    (x as f64, y as f64, "point2_array_first_nested")
                }),
                TypeIoObject::Vec2Array(values) => values
                    .first()
                    .map(|(x, y)| (*x as f64, *y as f64, "vec2_array_first_nested")),
                _ => None,
            })
            .ok_or("no_world_pos_payload")?,
    };
    let overlay = format_compact_world_pos(x, y);
    Ok(RenderedSurfaceValue {
        detail: format!("x={x} y={y} source={source}"),
        stable_value: format!("{x},{y}"),
        overlay_value: overlay,
        marker: Some(position_overlay_marker(
            RuntimeCustomPacketSemanticKind::WorldPos,
            x as f32,
            y as f32,
        )),
    })
}

fn extract_logic_build_pos(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let build_pos = match value {
        TypeIoObject::BuildingPos(build_pos) => Some((*build_pos, "building_pos")),
        TypeIoObject::Int(build_pos) => Some((*build_pos, "int")),
        TypeIoObject::Long(build_pos) => {
            i32::try_from(*build_pos).ok().map(|value| (value, "long"))
        }
        TypeIoObject::ObjectArray(_) => None,
        _ => None,
    };
    let (build_pos, source) = match build_pos {
        Some(value) => value,
        None => value
            .find_first_dfs(|object| {
                matches!(
                    object,
                    TypeIoObject::BuildingPos(_) | TypeIoObject::Int(_) | TypeIoObject::Long(_)
                )
            })
            .and_then(|matched| match matched.value {
                TypeIoObject::BuildingPos(build_pos) => Some((*build_pos, "building_pos_nested")),
                TypeIoObject::Int(build_pos) => Some((*build_pos, "int_nested")),
                TypeIoObject::Long(build_pos) => i32::try_from(*build_pos)
                    .ok()
                    .map(|value| (value, "long_nested")),
                _ => None,
            })
            .ok_or("no_build_pos_payload")?,
    };
    let (world_x, world_y) = build_pos_world_pos(build_pos);
    Ok(RenderedSurfaceValue {
        detail: format!("build_pos={build_pos} source={source}"),
        stable_value: build_pos.to_string(),
        overlay_value: build_pos.to_string(),
        marker: Some(position_overlay_marker(
            RuntimeCustomPacketSemanticKind::BuildPos,
            world_x,
            world_y,
        )),
    })
}

fn extract_logic_unit_id(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let unit_id = match value {
        TypeIoObject::UnitId(unit_id) => Some((*unit_id, "unit_id")),
        TypeIoObject::Int(unit_id) => Some((*unit_id, "int")),
        TypeIoObject::Long(unit_id) => i32::try_from(*unit_id).ok().map(|value| (value, "long")),
        TypeIoObject::ObjectArray(_) => None,
        _ => None,
    };
    let (unit_id, source) = match unit_id {
        Some(value) => value,
        None => value
            .find_first_dfs(|object| {
                matches!(
                    object,
                    TypeIoObject::UnitId(_) | TypeIoObject::Int(_) | TypeIoObject::Long(_)
                )
            })
            .and_then(|matched| match matched.value {
                TypeIoObject::UnitId(unit_id) => Some((*unit_id, "unit_id_nested")),
                TypeIoObject::Int(unit_id) => Some((*unit_id, "int_nested")),
                TypeIoObject::Long(unit_id) => i32::try_from(*unit_id)
                    .ok()
                    .map(|value| (value, "long_nested")),
                _ => None,
            })
            .ok_or("no_unit_id_payload")?,
    };
    Ok(RenderedSurfaceValue {
        detail: format!("unit_id={unit_id} source={source}"),
        stable_value: unit_id.to_string(),
        overlay_value: unit_id.to_string(),
        marker: None,
    })
}

fn extract_logic_team(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let team = match value {
        TypeIoObject::Team(team) => Some((*team, "team")),
        TypeIoObject::Int(team) => u8::try_from(*team).ok().map(|value| (value, "int")),
        TypeIoObject::Long(team) => u8::try_from(*team).ok().map(|value| (value, "long")),
        TypeIoObject::ObjectArray(_) => None,
        _ => None,
    };
    let (team, source) = match team {
        Some(value) => value,
        None => value
            .find_first_dfs(|object| {
                matches!(
                    object,
                    TypeIoObject::Team(_) | TypeIoObject::Int(_) | TypeIoObject::Long(_)
                )
            })
            .and_then(|matched| match matched.value {
                TypeIoObject::Team(team) => Some((*team, "team_nested")),
                TypeIoObject::Int(team) => {
                    u8::try_from(*team).ok().map(|value| (value, "int_nested"))
                }
                TypeIoObject::Long(team) => {
                    u8::try_from(*team).ok().map(|value| (value, "long_nested"))
                }
                _ => None,
            })
            .ok_or("no_team_payload")?,
    };
    Ok(RenderedSurfaceValue {
        detail: format!("team={team} source={source}"),
        stable_value: team.to_string(),
        overlay_value: team.to_string(),
        marker: None,
    })
}

fn extract_logic_bool(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let flag = match value {
        TypeIoObject::Bool(flag) => Some((*flag, "bool")),
        TypeIoObject::ObjectArray(_) => None,
        _ => None,
    };
    let (flag, source) = match flag {
        Some(value) => value,
        None => value
            .find_first_dfs(|object| matches!(object, TypeIoObject::Bool(_)))
            .and_then(|matched| match matched.value {
                TypeIoObject::Bool(flag) => Some((*flag, "bool_nested")),
                _ => None,
            })
            .ok_or("no_bool_payload")?,
    };
    Ok(RenderedSurfaceValue {
        detail: format!("value={flag} source={source}"),
        stable_value: flag.to_string(),
        overlay_value: flag.to_string(),
        marker: None,
    })
}

fn extract_logic_number(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let number = logic_number_value(value)
        .or_else(|| {
            value
                .find_first_dfs(|object| logic_number_value(object).is_some())
                .and_then(|matched| logic_number_value(matched.value))
        })
        .ok_or("no_numeric_payload")?;
    Ok(RenderedSurfaceValue {
        detail: format!("value={number}"),
        stable_value: number.clone(),
        overlay_value: number,
        marker: None,
    })
}

fn position_overlay_marker(
    semantic: RuntimeCustomPacketSemanticKind,
    x: f32,
    y: f32,
) -> RuntimeCustomPacketOverlayMarker {
    RuntimeCustomPacketOverlayMarker {
        key: String::new(),
        encoding: RuntimeCustomPacketSemanticEncoding::Text,
        semantic,
        x,
        y,
    }
}

fn build_pos_world_pos(build_pos: i32) -> (f32, f32) {
    let (tile_x, tile_y) = unpack_point2(build_pos);
    (tile_x as f32 * 8.0, tile_y as f32 * 8.0)
}

fn logic_number_value(value: &TypeIoObject) -> Option<String> {
    match value {
        TypeIoObject::Int(number) => Some(number.to_string()),
        TypeIoObject::Long(number) => Some(number.to_string()),
        TypeIoObject::Float(number) => Some(number.to_string()),
        TypeIoObject::Double(number) => Some(number.to_string()),
        _ => None,
    }
}

fn parse_text_world_pos(text: &str) -> Option<(f64, f64, &'static str)> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        let x = extract_json_number_field(trimmed, "x")?;
        let y = extract_json_number_field(trimmed, "y")?;
        return Some((x, y, "json_xy"));
    }
    if let Some((left, right)) = trimmed.split_once(':') {
        return Some((
            left.trim().parse().ok()?,
            right.trim().parse().ok()?,
            "pair_colon",
        ));
    }
    if let Some((left, right)) = trimmed.split_once(',') {
        return Some((
            left.trim().parse().ok()?,
            right.trim().parse().ok()?,
            "pair_comma",
        ));
    }
    None
}

fn parse_text_i32(text: &str) -> Option<i32> {
    let trimmed = text.trim();
    trimmed
        .parse::<i32>()
        .ok()
        .or_else(|| extract_json_number_field(trimmed, "value").and_then(f64_to_i32))
        .or_else(|| extract_json_number_field(trimmed, "id").and_then(f64_to_i32))
        .or_else(|| extract_json_number_field(trimmed, "buildPos").and_then(f64_to_i32))
        .or_else(|| extract_json_number_field(trimmed, "unitId").and_then(f64_to_i32))
}

fn parse_text_u8(text: &str) -> Option<u8> {
    let trimmed = text.trim();
    trimmed
        .parse::<u8>()
        .ok()
        .or_else(|| extract_json_number_field(trimmed, "value").and_then(f64_to_u8))
        .or_else(|| extract_json_number_field(trimmed, "team").and_then(f64_to_u8))
}

fn parse_text_bool(text: &str) -> Option<bool> {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("true") || trimmed == "1" {
        return Some(true);
    }
    if trimmed.eq_ignore_ascii_case("false") || trimmed == "0" {
        return Some(false);
    }
    extract_json_bool_field(trimmed, "value")
}

fn parse_text_f64(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    trimmed
        .parse::<f64>()
        .ok()
        .or_else(|| extract_json_number_field(trimmed, "value"))
        .or_else(|| extract_json_number_field(trimmed, "number"))
}

fn extract_json_number_field(text: &str, field: &str) -> Option<f64> {
    let needle = format!("\"{field}\"");
    let index = text.find(&needle)?;
    let rest = &text[index + needle.len()..];
    let colon = rest.find(':')?;
    let mut value = rest[colon + 1..].trim_start();
    let mut end = 0usize;
    for (idx, ch) in value.char_indices() {
        if idx == 0 && (ch == '-' || ch == '+') {
            end = idx + ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            end = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    if end == 0 {
        return None;
    }
    value = &value[..end];
    value.parse::<f64>().ok()
}

fn extract_json_bool_field(text: &str, field: &str) -> Option<bool> {
    let needle = format!("\"{field}\"");
    let index = text.find(&needle)?;
    let rest = &text[index + needle.len()..];
    let colon = rest.find(':')?;
    let value = rest[colon + 1..].trim_start();
    if value.starts_with("true") {
        Some(true)
    } else if value.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn f64_to_i32(value: f64) -> Option<i32> {
    (value.fract() == 0.0 && value >= i32::MIN as f64 && value <= i32::MAX as f64)
        .then_some(value as i32)
}

fn f64_to_u8(value: f64) -> Option<u8> {
    (value.fract() == 0.0 && value >= u8::MIN as f64 && value <= u8::MAX as f64)
        .then_some(value as u8)
}

fn format_compact_world_pos(x: f64, y: f64) -> String {
    format!("{},{}", trim_trailing_zeroes(x), trim_trailing_zeroes(y))
}

fn trim_trailing_zeroes(value: f64) -> String {
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

fn encoding_label(encoding: RuntimeCustomPacketSemanticEncoding) -> &'static str {
    match encoding {
        RuntimeCustomPacketSemanticEncoding::Text => "text",
        RuntimeCustomPacketSemanticEncoding::Binary => "binary",
        RuntimeCustomPacketSemanticEncoding::LogicData => "logic",
    }
}

fn encoding_overlay_prefix(encoding: RuntimeCustomPacketSemanticEncoding) -> &'static str {
    encoding_label(encoding)
}

fn logic_data_transport_label(transport: ClientLogicDataTransport) -> &'static str {
    match transport {
        ClientLogicDataTransport::Reliable => "reliable",
        ClientLogicDataTransport::Unreliable => "unreliable",
    }
}

fn encode_hex_prefix(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(16)
        .map(|value| format!("{value:02x}"))
        .collect::<String>()
}

fn truncate_for_preview(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdt_typeio::pack_point2;

    #[test]
    fn runtime_custom_packet_surface_overlay_summary_tracks_latest_updates_and_reset() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
        });

        state.record_text_handler("custom.status", "wave ready");
        state.record_logic_data_handler(
            "logic.pos",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Point2 { x: 7, y: 9 },
        );

        assert_eq!(
            state.overlay_summary_text(4),
            Some("logic:logic.pos=7,9 | text:custom.status=wave ready".to_string())
        );

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].contains("runtime_custom_packet_surface_summary:"));
        assert!(summaries[0].contains("last=Some(\"wave ready\")"));
        assert!(summaries[1].contains("last=Some(\"7,9\")"));

        state.observe_events(&[ClientSessionEvent::WorldDataBegin]);

        let lines = state.drain_lines();
        assert!(lines
            .iter()
            .any(|line| line.contains("runtime_custom_packet_surface_reset:")));
        assert_eq!(state.overlay_summary_text(4), None);
    }

    #[test]
    fn runtime_custom_packet_surface_overlay_markers_export_world_and_build_positions() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "text.world".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "text.build".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "logic.build".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        });

        state.record_text_handler("text.world", "{\"x\":12.5,\"y\":-4}");
        state.record_text_handler("text.build", &pack_point2(3, 5).to_string());
        state.record_logic_data_handler(
            "logic.build",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::BuildingPos(pack_point2(-2, 7)),
        );

        assert_eq!(
            state.overlay_markers(4),
            vec![
                RuntimeCustomPacketOverlayMarker {
                    key: "logic.build".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                    semantic: RuntimeCustomPacketSemanticKind::BuildPos,
                    x: -16.0,
                    y: 56.0,
                },
                RuntimeCustomPacketOverlayMarker {
                    key: "text.build".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::Text,
                    semantic: RuntimeCustomPacketSemanticKind::BuildPos,
                    x: 24.0,
                    y: 40.0,
                },
                RuntimeCustomPacketOverlayMarker {
                    key: "text.world".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::Text,
                    semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                    x: 12.5,
                    y: -4.0,
                },
            ]
        );
    }

    #[test]
    fn runtime_custom_packet_surface_overlay_markers_reset_on_world_data_begin() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "text.build".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        });

        state.record_text_handler("text.build", &pack_point2(4, 6).to_string());
        assert_eq!(state.overlay_markers(4).len(), 1);

        state.observe_events(&[ClientSessionEvent::WorldDataBegin]);

        assert!(state.overlay_markers(4).is_empty());
    }

    #[test]
    fn runtime_custom_packet_surface_latest_summary_entries_export_stable_values_and_markers() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
        });

        state.record_text_handler("custom.status", "wave ready");
        state.record_logic_data_handler(
            "logic.pos",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Point2 { x: 7, y: 9 },
        );

        assert_eq!(
            state.latest_summary_entries(4),
            vec![
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
            ]
        );

        state.observe_events(&[ClientSessionEvent::WorldDataBegin]);

        assert!(state.latest_summary_entries(4).is_empty());
    }
}
