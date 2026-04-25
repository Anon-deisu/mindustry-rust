use crate::client_session::{ClientLogicDataTransport, ClientSession, ClientSessionEvent};
use crate::custom_packet_runtime::{
    RuntimeCustomPacketSemanticEncoding, RuntimeCustomPacketSemanticKind,
    RuntimeCustomPacketSemanticSpec,
};
use crate::custom_packet_runtime_logic as logic_helpers;
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

const NATIVE_SERVER_MESSAGE_KEY: &str = "sendMessage";
const NATIVE_CHAT_MESSAGE_KEY: &str = "sendMessageWithSender";
const NATIVE_SET_HUD_TEXT_KEY: &str = "setHudText";
const NATIVE_SET_HUD_TEXT_RELIABLE_KEY: &str = "setHudTextReliable";
const NATIVE_ANNOUNCE_KEY: &str = "announce";
const NATIVE_CLIPBOARD_KEY: &str = "copyToClipboard";
const NATIVE_OPEN_URI_KEY: &str = "openURI";

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
            .any(|event| {
                matches!(
                    event,
                    ClientSessionEvent::WorldDataBegin
                        | ClientSessionEvent::WorldStreamStarted { .. }
                        | ClientSessionEvent::ConnectRedirectRequested { .. }
                )
            })
        {
            let reason = if events.iter().any(|event| {
                matches!(
                    event,
                    ClientSessionEvent::ConnectRedirectRequested { .. }
                )
            }) {
                "connect_redirect"
            } else if events.iter().any(|event| {
                matches!(event, ClientSessionEvent::WorldStreamStarted { .. })
            }) {
                "world_stream_started"
            } else {
                "world_data_begin"
            };
            self.clear_last_values(reason);
        }
        for event in events {
            if let Some((key, reliable, text)) = native_text_event(event) {
                self.record_event(RuntimeCustomPacketSemanticEncoding::Text, key, reliable);
                if let Some(text) = text {
                    self.record_text_handler(key, text);
                }
                continue;
            }
            match event {
                ClientSessionEvent::HideHudText => self.clear_native_hud_text_values(),
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

    fn clear_native_hud_text_values(&mut self) {
        self.clear_text_route_last_values(
            &[NATIVE_SET_HUD_TEXT_KEY, NATIVE_SET_HUD_TEXT_RELIABLE_KEY],
            "hide_hud_text",
        );
    }

    fn clear_text_route_last_values(&mut self, keys: &[&str], reason: &str) {
        let mut cleared = 0usize;
        for key in keys {
            let Some(routes) = self.text_routes.get_mut(*key) else {
                continue;
            };
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
                "runtime_custom_packet_surface_reset: reason={reason:?} cleared_routes={cleared}"
            ));
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
                "runtime_custom_packet_surface_reset: reason={reason:?} cleared_routes={cleared}"
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
    let (x, y, source) = parse_text_world_pos(text)
        .and_then(|(x, y, source)| finite_world_pos(x, y).map(|(x, y)| (x, y, source)))
        .ok_or("invalid_world_pos")?;
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
    if !value.is_finite() {
        return Err("invalid_number");
    }
    let rendered = value.to_string();
    Ok(RenderedSurfaceValue {
        detail: format!("value={rendered}"),
        stable_value: rendered.clone(),
        overlay_value: rendered,
        marker: None,
    })
}

fn extract_logic_string(value: &TypeIoObject) -> Option<String> {
    logic_helpers::extract_logic_string(value)
}

fn extract_logic_world_pos(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let extracted = logic_helpers::extract_logic_world_pos(value).ok_or_else(|| {
        if logic_helpers::has_logic_world_pos_payload(value) {
            "invalid_world_pos"
        } else {
            "no_world_pos_payload"
        }
    })?;
    let (x, y, source) = (extracted.value.0, extracted.value.1, extracted.source);
    let (x, y) = finite_world_pos(x, y).ok_or("invalid_world_pos")?;
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
    let extracted = logic_helpers::extract_logic_build_pos(value).ok_or("no_build_pos_payload")?;
    let (build_pos, source) = (extracted.value, extracted.source);
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

fn finite_world_pos(x: f64, y: f64) -> Option<(f64, f64)> {
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn extract_logic_unit_id(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let extracted = logic_helpers::extract_logic_unit_id(value).ok_or("no_unit_id_payload")?;
    let (unit_id, source) = (extracted.value, extracted.source);
    Ok(RenderedSurfaceValue {
        detail: format!("unit_id={unit_id} source={source}"),
        stable_value: unit_id.to_string(),
        overlay_value: unit_id.to_string(),
        marker: None,
    })
}

fn extract_logic_team(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let extracted = logic_helpers::extract_logic_team(value).ok_or("no_team_payload")?;
    let (team, source) = (extracted.value, extracted.source);
    Ok(RenderedSurfaceValue {
        detail: format!("team={team} source={source}"),
        stable_value: team.to_string(),
        overlay_value: team.to_string(),
        marker: None,
    })
}

fn extract_logic_bool(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let extracted = logic_helpers::extract_logic_bool(value).ok_or("no_bool_payload")?;
    let (flag, source) = (extracted.value, extracted.source);
    Ok(RenderedSurfaceValue {
        detail: format!("value={flag} source={source}"),
        stable_value: flag.to_string(),
        overlay_value: flag.to_string(),
        marker: None,
    })
}

fn extract_logic_number(value: &TypeIoObject) -> Result<RenderedSurfaceValue, &'static str> {
    let number = logic_helpers::extract_logic_number(value).ok_or("no_numeric_payload")?;
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
    let value = extract_json_field_value(text, field)?;
    let mut end = 0usize;
    for (idx, ch) in value.char_indices() {
        if idx == 0 && (ch == '-' || ch == '+') {
            end = idx + ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-') {
            end = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    if end == 0 {
        return None;
    }
    json_literal_terminated(value, end).then(|| value[..end].parse::<f64>().ok())?
}

fn extract_json_bool_field(text: &str, field: &str) -> Option<bool> {
    let value = extract_json_field_value(text, field)?;
    if value.starts_with("true") && json_literal_terminated(value, "true".len()) {
        return Some(true);
    }
    if value.starts_with("false") && json_literal_terminated(value, "false".len()) {
        return Some(false);
    }
    None
}

fn extract_json_field_value<'a>(text: &'a str, field: &str) -> Option<&'a str> {
    let needle = format!("\"{field}\"");
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => {
                if depth == 1
                    && text[index..].starts_with(&needle)
                    && json_field_has_object_key_boundary(text, index)
                {
                    let rest = text[index + needle.len()..].trim_start();
                    if let Some(value) = rest.strip_prefix(':') {
                        return Some(value.trim_start());
                    }
                }
                in_string = true;
            }
            '{' | '[' => depth = depth.saturating_add(1),
            '}' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn json_field_has_object_key_boundary(text: &str, index: usize) -> bool {
    let bytes = text.as_bytes();
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
        cursor -= 1;
    }
    if cursor == 0 {
        return false;
    }
    matches!(bytes[cursor - 1], b'{' | b',')
}

fn json_literal_terminated(value: &str, parsed_len: usize) -> bool {
    value[parsed_len..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_ascii_whitespace() || matches!(ch, ',' | '}' | ']'))
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

fn native_text_event(event: &ClientSessionEvent) -> Option<(&'static str, bool, Option<&str>)> {
    match event {
        ClientSessionEvent::ServerMessage { message } => {
            Some((NATIVE_SERVER_MESSAGE_KEY, true, Some(message.as_str())))
        }
        ClientSessionEvent::ChatMessage {
            message,
            unformatted,
            ..
        } => Some((
            NATIVE_CHAT_MESSAGE_KEY,
            true,
            unformatted.as_deref().or(Some(message.as_str())),
        )),
        ClientSessionEvent::SetHudText { message } => {
            Some((NATIVE_SET_HUD_TEXT_KEY, false, message.as_deref()))
        }
        ClientSessionEvent::SetHudTextReliable { message } => {
            Some((NATIVE_SET_HUD_TEXT_RELIABLE_KEY, true, message.as_deref()))
        }
        ClientSessionEvent::Announce { message } => {
            Some((NATIVE_ANNOUNCE_KEY, true, message.as_deref()))
        }
        ClientSessionEvent::CopyToClipboard { text } => {
            Some((NATIVE_CLIPBOARD_KEY, true, text.as_deref()))
        }
        ClientSessionEvent::OpenUri { uri } => Some((NATIVE_OPEN_URI_KEY, true, uri.as_deref())),
        _ => None,
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
    fn runtime_custom_packet_surface_handles_team_bool_and_number_semantics() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        for (key, encoding, semantic) in [
            (
                "text.team",
                RuntimeCustomPacketSemanticEncoding::Text,
                RuntimeCustomPacketSemanticKind::Team,
            ),
            (
                "logic.team",
                RuntimeCustomPacketSemanticEncoding::LogicData,
                RuntimeCustomPacketSemanticKind::Team,
            ),
            (
                "text.bool",
                RuntimeCustomPacketSemanticEncoding::Text,
                RuntimeCustomPacketSemanticKind::Bool,
            ),
            (
                "logic.bool",
                RuntimeCustomPacketSemanticEncoding::LogicData,
                RuntimeCustomPacketSemanticKind::Bool,
            ),
            (
                "text.number",
                RuntimeCustomPacketSemanticEncoding::Text,
                RuntimeCustomPacketSemanticKind::Number,
            ),
            (
                "logic.number",
                RuntimeCustomPacketSemanticEncoding::LogicData,
                RuntimeCustomPacketSemanticKind::Number,
            ),
        ] {
            state.register(&RuntimeCustomPacketSemanticSpec {
                key: key.to_string(),
                encoding,
                semantic,
            });
        }

        state.record_event(RuntimeCustomPacketSemanticEncoding::Text, "text.team", true);
        state.record_text_handler("text.team", "7");
        state.record_event(
            RuntimeCustomPacketSemanticEncoding::LogicData,
            "logic.team",
            true,
        );
        state.record_logic_data_handler(
            "logic.team",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Team(7),
        );

        state.record_event(RuntimeCustomPacketSemanticEncoding::Text, "text.bool", true);
        state.record_text_handler("text.bool", "true");
        state.record_event(
            RuntimeCustomPacketSemanticEncoding::LogicData,
            "logic.bool",
            true,
        );
        state.record_logic_data_handler(
            "logic.bool",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Bool(true),
        );

        state.record_event(
            RuntimeCustomPacketSemanticEncoding::Text,
            "text.number",
            true,
        );
        state.record_text_handler("text.number", "12.5");
        state.record_event(
            RuntimeCustomPacketSemanticEncoding::LogicData,
            "logic.number",
            true,
        );
        state.record_logic_data_handler(
            "logic.number",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Float(12.5),
        );

        let lines = state.drain_lines();
        assert_eq!(lines.len(), 6);
        assert!(lines.iter().any(|line| {
            line.contains("encoding=text")
                && line.contains("semantic=team")
                && line.contains("key=\"text.team\"")
                && line.contains("team=7")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("semantic=team")
                && line.contains("key=\"logic.team\"")
                && line.contains("team=7")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("encoding=text")
                && line.contains("semantic=bool")
                && line.contains("key=\"text.bool\"")
                && line.contains("value=true")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("semantic=bool")
                && line.contains("key=\"logic.bool\"")
                && line.contains("value=true")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("encoding=text")
                && line.contains("semantic=number")
                && line.contains("key=\"text.number\"")
                && line.contains("value=12.5")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("semantic=number")
                && line.contains("key=\"logic.number\"")
                && line.contains("value=12.5")
        }));

        let summary_text = state
            .overlay_summary_text(6)
            .expect("expected a compact surface summary");
        assert!(summary_text.contains("text:text.team=7"));
        assert!(summary_text.contains("logic:logic.team=7"));
        assert!(summary_text.contains("text:text.bool=true"));
        assert!(summary_text.contains("logic:logic.bool=true"));
        assert!(summary_text.contains("text:text.number=12.5"));
        assert!(summary_text.contains("logic:logic.number=12.5"));

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 6);
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=text")
                && line.contains("semantic=team")
                && line.contains("last=Some(\"7\")")
                && line.contains("parity=ok")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("semantic=team")
                && line.contains("last=Some(\"7\")")
                && line.contains("parity=ok")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=text")
                && line.contains("semantic=bool")
                && line.contains("last=Some(\"true\")")
                && line.contains("parity=ok")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("semantic=bool")
                && line.contains("last=Some(\"true\")")
                && line.contains("parity=ok")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=text")
                && line.contains("semantic=number")
                && line.contains("last=Some(\"12.5\")")
                && line.contains("parity=ok")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("semantic=number")
                && line.contains("last=Some(\"12.5\")")
                && line.contains("parity=ok")
        }));

        let latest = state.latest_summary_entries(6);
        assert_eq!(latest.len(), 6);
        assert!(latest.iter().all(|entry| entry.marker.is_none()));
        assert!(latest.iter().any(|entry| {
            entry.key == "text.team"
                && entry.encoding == RuntimeCustomPacketSemanticEncoding::Text
                && entry.semantic == RuntimeCustomPacketSemanticKind::Team
                && entry.stable_value == "7"
        }));
        assert!(latest.iter().any(|entry| {
            entry.key == "logic.team"
                && entry.encoding == RuntimeCustomPacketSemanticEncoding::LogicData
                && entry.semantic == RuntimeCustomPacketSemanticKind::Team
                && entry.stable_value == "7"
        }));
        assert!(latest.iter().any(|entry| {
            entry.key == "text.bool"
                && entry.encoding == RuntimeCustomPacketSemanticEncoding::Text
                && entry.semantic == RuntimeCustomPacketSemanticKind::Bool
                && entry.stable_value == "true"
        }));
        assert!(latest.iter().any(|entry| {
            entry.key == "logic.bool"
                && entry.encoding == RuntimeCustomPacketSemanticEncoding::LogicData
                && entry.semantic == RuntimeCustomPacketSemanticKind::Bool
                && entry.stable_value == "true"
        }));
        assert!(latest.iter().any(|entry| {
            entry.key == "text.number"
                && entry.encoding == RuntimeCustomPacketSemanticEncoding::Text
                && entry.semantic == RuntimeCustomPacketSemanticKind::Number
                && entry.stable_value == "12.5"
        }));
        assert!(latest.iter().any(|entry| {
            entry.key == "logic.number"
                && entry.encoding == RuntimeCustomPacketSemanticEncoding::LogicData
                && entry.semantic == RuntimeCustomPacketSemanticKind::Number
                && entry.stable_value == "12.5"
        }));

        assert!(state.overlay_markers(6).is_empty());
    }

    #[test]
    fn runtime_custom_packet_surface_reports_binary_utf8_and_logic_string_decode_errors() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "bin.hud".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Binary,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "logic.hud".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        });

        state.record_binary_handler("bin.hud", &[0xff, 0xfe, 0xfd]);
        state.record_logic_data_handler(
            "logic.hud",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Int(7),
        );

        let lines = state.drain_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().any(|line| {
            line.contains("encoding=binary")
                && line.contains("key=\"bin.hud\"")
                && line.contains("reason=\"invalid_utf8\"")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("key=\"logic.hud\"")
                && line.contains("reason=\"no_string_payload\"")
                && line.contains("preview=")
        }));

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=binary")
                && line.contains("decode_errors=1")
                && line.contains("last=None")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("encoding=logic")
                && line.contains("decode_errors=1")
                && line.contains("last=None")
        }));
        assert_eq!(state.overlay_summary_text(4), None);
        assert!(state.latest_summary_entries(4).is_empty());
    }

    #[test]
    fn parse_text_f64_accepts_scientific_notation() {
        assert_eq!(parse_text_f64("1e3"), Some(1000.0));
        assert_eq!(parse_text_f64("{\"value\":1e3}"), Some(1000.0));

        let parsed = parse_text_f64("{\"number\":-2.5E-4}").unwrap();
        assert!((parsed + 0.00025).abs() < f64::EPSILON);
    }

    #[test]
    fn build_pos_world_pos_converts_packed_build_pos_into_8x_world_coordinates() {
        assert_eq!(build_pos_world_pos(pack_point2(3, 5)), (24.0, 40.0));
        assert_eq!(build_pos_world_pos(pack_point2(0, 0)), (0.0, 0.0));
        assert_eq!(build_pos_world_pos(pack_point2(-1, -2)), (-8.0, -16.0));
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
    fn runtime_custom_packet_surface_overlay_markers_reset_on_connect_redirect() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "text.build".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        });

        state.record_text_handler("text.build", &pack_point2(4, 6).to_string());
        assert_eq!(state.overlay_markers(4).len(), 1);
        assert!(state.overlay_summary_text(4).is_some());
        assert_eq!(state.latest_summary_entries(4).len(), 1);

        state.observe_events(&[ClientSessionEvent::ConnectRedirectRequested {
            ip: "127.0.0.1".to_string(),
            port: 6568,
        }]);

        let lines = state.drain_lines();
        assert!(lines.iter().any(|line| {
            line.contains("runtime_custom_packet_surface_reset:")
                && line.contains("reason=\"connect_redirect\"")
        }));
        assert!(state.overlay_markers(4).is_empty());
        assert_eq!(state.overlay_summary_text(4), None);
        assert!(state.latest_summary_entries(4).is_empty());
    }

    #[test]
    fn runtime_custom_packet_surface_overlay_markers_reset_on_world_stream_started() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "text.build".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        });

        state.record_text_handler("text.build", &pack_point2(4, 6).to_string());
        assert_eq!(state.overlay_markers(4).len(), 1);
        assert!(state.overlay_summary_text(4).is_some());
        assert_eq!(state.latest_summary_entries(4).len(), 1);

        state.observe_events(&[ClientSessionEvent::WorldStreamStarted {
            stream_id: 3,
            total_bytes: 1024,
        }]);

        let lines = state.drain_lines();
        assert!(lines.iter().any(|line| {
            line.contains("runtime_custom_packet_surface_reset:")
                && line.contains("reason=\"world_stream_started\"")
        }));
        assert!(state.overlay_markers(4).is_empty());
        assert_eq!(state.overlay_summary_text(4), None);
        assert!(state.latest_summary_entries(4).is_empty());
    }

    #[test]
    fn runtime_custom_packet_surface_resets_overlay_and_summary_state_consistently_on_world_reload_events(
    ) {
        for (reason, event) in [
            ("world_data_begin", ClientSessionEvent::WorldDataBegin),
            (
                "world_stream_started",
                ClientSessionEvent::WorldStreamStarted {
                    stream_id: 3,
                    total_bytes: 1024,
                },
            ),
            (
                "connect_redirect",
                ClientSessionEvent::ConnectRedirectRequested {
                    ip: "127.0.0.1".to_string(),
                    port: 6568,
                },
            ),
        ] {
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

            assert_eq!(state.overlay_markers(4).len(), 1);
            assert!(state.overlay_summary_text(4).is_some());
            assert_eq!(state.latest_summary_entries(4).len(), 2);

            state.observe_events(&[event]);

            let lines = state.drain_lines();
            assert!(
                lines.iter().any(|line| {
                    line.contains("runtime_custom_packet_surface_reset:")
                        && line.contains(&format!("reason={reason:?}"))
                        && line.contains("cleared_routes=2")
                }),
                "missing reset line for reason={reason}: {lines:?}"
            );
            assert!(
                state.overlay_markers(4).is_empty(),
                "overlay markers were not cleared for reason={reason}"
            );
            assert_eq!(
                state.overlay_summary_text(4),
                None,
                "overlay summary was not cleared for reason={reason}"
            );
            assert!(
                state.latest_summary_entries(4).is_empty(),
                "latest summary entries were not cleared for reason={reason}"
            );
        }
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

    #[test]
    fn runtime_custom_packet_surface_latest_summary_entries_break_same_serial_ties_stably() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        state.text_routes.insert(
            "same.key".to_string(),
            vec![
                RuntimeCustomPacketSurfaceRouteState {
                    semantic: RuntimeCustomPacketSemanticKind::HudText,
                    handler_count: 1,
                    event_reliable_count: 0,
                    event_unreliable_count: 0,
                    decode_error_count: 0,
                    last_overlay_value: Some("first".to_string()),
                    last_stable_value: Some("first".to_string()),
                    last_marker: None,
                    last_update_serial: 7,
                },
                RuntimeCustomPacketSurfaceRouteState {
                    semantic: RuntimeCustomPacketSemanticKind::HudText,
                    handler_count: 1,
                    event_reliable_count: 0,
                    event_unreliable_count: 0,
                    decode_error_count: 0,
                    last_overlay_value: Some("second".to_string()),
                    last_stable_value: Some("second".to_string()),
                    last_marker: None,
                    last_update_serial: 7,
                },
            ],
        );

        let expected = vec![
            RuntimeCustomPacketSurfaceSummaryEntry {
                key: "same.key".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
                stable_value: "first".to_string(),
                marker: None,
            },
            RuntimeCustomPacketSurfaceSummaryEntry {
                key: "same.key".to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
                stable_value: "second".to_string(),
                marker: None,
            },
        ];

        assert_eq!(state.latest_summary_entries(4), expected);
        assert_eq!(state.latest_summary_entries(4), expected);
    }

    #[test]
    fn runtime_custom_packet_surface_rejects_non_finite_world_positions() {
        assert_eq!(render_text_world_pos("NaN,9"), Err("invalid_world_pos"));
        assert_eq!(
            extract_logic_world_pos(&TypeIoObject::Vec2 {
                x: f32::INFINITY,
                y: 9.0,
            }),
            Err("invalid_world_pos")
        );
    }

    #[test]
    fn runtime_custom_packet_surface_rejects_non_finite_numbers() {
        assert_eq!(render_text_number("NaN"), Err("invalid_number"));
        assert_eq!(render_text_number("inf"), Err("invalid_number"));
    }

    #[test]
    fn render_text_world_pos_accepts_json_xy_before_pair_syntax() {
        let rendered = render_text_world_pos("{\"x\":12.5,\"y\":-4}").unwrap();
        assert_eq!(rendered.stable_value, "12.5,-4");
        assert!(rendered.detail.contains("source=json_xy"));
    }

    #[test]
    fn parse_text_number_fields_require_exact_json_keys() {
        assert_eq!(parse_text_world_pos("{\"xCoord\":12,\"yCoord\":-4}"), None);
        assert_eq!(parse_text_i32("{\"idValue\":7}"), None);
        assert_eq!(parse_text_u8("{\"teamValue\":3}"), None);
        assert_eq!(parse_text_f64("{\"numberValue\":1.5}"), None);
    }

    #[test]
    fn parse_text_json_fields_ignore_prose_strings_that_mention_keys() {
        assert_eq!(parse_text_bool("note \"value\": false, trailing"), None);
        assert_eq!(parse_text_f64("prefix \"number\": 12.5, suffix"), None);
        assert_eq!(parse_text_i32("prefix \"buildPos\": 7, suffix"), None);
        assert_eq!(parse_text_u8("prefix \"team\": 3, suffix"), None);
    }

    #[test]
    fn runtime_custom_packet_surface_labels_are_stable_for_encodings_semantics_and_transports() {
        assert_eq!(encoding_label(RuntimeCustomPacketSemanticEncoding::Text), "text");
        assert_eq!(encoding_label(RuntimeCustomPacketSemanticEncoding::Binary), "binary");
        assert_eq!(encoding_label(RuntimeCustomPacketSemanticEncoding::LogicData), "logic");

        assert_eq!(
            semantic_label(RuntimeCustomPacketSemanticKind::ServerMessage),
            "server_message"
        );
        assert_eq!(
            semantic_label(RuntimeCustomPacketSemanticKind::ChatMessage),
            "chat_message"
        );
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::HudText), "hud_text");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::Announce), "announce");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::Clipboard), "clipboard");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::OpenUri), "open_uri");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::WorldPos), "world_pos");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::BuildPos), "build_pos");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::Team), "team");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::Bool), "bool");
        assert_eq!(semantic_label(RuntimeCustomPacketSemanticKind::Number), "number");

        assert_eq!(
            logic_data_transport_label(ClientLogicDataTransport::Reliable),
            "reliable"
        );
        assert_eq!(
            logic_data_transport_label(ClientLogicDataTransport::Unreliable),
            "unreliable"
        );
    }

    #[test]
    fn parse_text_bool_accepts_numeric_and_case_insensitive_literals() {
        assert_eq!(parse_text_bool(" TRUE "), Some(true));
        assert_eq!(parse_text_bool("false"), Some(false));
        assert_eq!(parse_text_bool("1"), Some(true));
        assert_eq!(parse_text_bool("0"), Some(false));
        assert_eq!(parse_text_bool("{\"value\":true}"), Some(true));
        assert_eq!(parse_text_bool("{\"value\":0}"), None);
    }

    #[test]
    fn parse_text_i32_u8_and_compact_world_pos_cover_json_fallbacks_and_trimming() {
        assert_eq!(parse_text_i32("{\"id\":7}"), Some(7));
        assert_eq!(parse_text_i32("{\"buildPos\":-12}"), Some(-12));
        assert_eq!(parse_text_i32("{\"unitId\":42}"), Some(42));
        assert_eq!(parse_text_u8("{\"team\":3}"), Some(3));
        assert_eq!(parse_text_u8("{\"value\":255}"), Some(255));
        assert_eq!(format_compact_world_pos(12.0, -4.5), "12,-4.5");
        assert_eq!(format_compact_world_pos(0.25, 8.0), "0.25,8");
    }

    #[test]
    fn parse_text_i32_and_u8_accept_trimmed_literals_and_reject_out_of_range_values() {
        assert_eq!(parse_text_i32("  -17 "), Some(-17));
        assert_eq!(parse_text_i32("2147483648"), None);
        assert_eq!(parse_text_u8(" 08 "), Some(8));
        assert_eq!(parse_text_u8("256"), None);
    }

    #[test]
    fn parse_text_world_pos_accepts_trimmed_pair_syntax_and_reports_source() {
        assert_eq!(
            parse_text_world_pos(" 12 : -4 "),
            Some((12.0, -4.0, "pair_colon"))
        );
        assert_eq!(
            parse_text_world_pos(" 7 , 8 "),
            Some((7.0, 8.0, "pair_comma"))
        );
    }

    #[test]
    fn parse_text_world_pos_reads_top_level_json_xy_and_rejects_non_finite_values() {
        assert_eq!(
            parse_text_world_pos("{\"x\":12.5,\"y\":-4}"),
            Some((12.5, -4.0, "json_xy"))
        );
        assert_eq!(
            parse_text_world_pos("{\"x\":null,\"y\":-4}"),
            None
        );
        assert_eq!(
            parse_text_world_pos("{\"x\":12,\"y\":Infinity}"),
            None
        );
    }

    #[test]
    fn format_compact_world_pos_trims_integer_and_fractional_boundaries() {
        assert_eq!(format_compact_world_pos(3.0, 4.5000), "3,4.5");
        assert_eq!(format_compact_world_pos(10.1000, -2.0000), "10.1,-2");
        assert_eq!(format_compact_world_pos(-0.0, 0.0), "-0,0");
    }

    #[test]
    fn parse_text_world_pos_rejects_malformed_pair_syntax() {
        assert_eq!(parse_text_world_pos("12:"), None);
        assert_eq!(parse_text_world_pos(":34"), None);
        assert_eq!(parse_text_world_pos("12,34,56"), None);
    }

    #[test]
    fn parse_text_world_pos_ignores_nested_fields() {
        assert_eq!(
            parse_text_world_pos("{\"nested\":{\"x\":12,\"y\":-4}}"),
            None
        );
    }

    #[test]
    fn parse_text_world_pos_requires_top_level_json_x_and_y() {
        assert_eq!(
            parse_text_world_pos("{\"x\":12.5,\"y\":-4}"),
            Some((12.5, -4.0, "json_xy"))
        );
        assert_eq!(parse_text_world_pos("{\"x\":12.5}"), None);
        assert_eq!(parse_text_world_pos("{\"y\":-4}"), None);
    }

    #[test]
    fn json_field_boundary_helpers_reject_prefix_collisions() {
        let top_level = "{\"x\":12.5,\"foobar\":1,\"nested\":{\"x\":99},\"prose\":\"the \\\"x\\\": 7 should not count\"}";

        let top_level_x = top_level.find("\"x\"").unwrap();
        assert!(json_field_has_object_key_boundary(top_level, top_level_x));
        assert_eq!(
            extract_json_field_value(top_level, "x").map(|value| value.starts_with("12.5")),
            Some(true)
        );
        assert_eq!(extract_json_field_value(top_level, "foo"), None);
        assert_eq!(
            extract_json_field_value(top_level, "foobar").map(|value| value.starts_with("1")),
            Some(true)
        );

        let prose_quote = top_level.find("\\\"x\\\"").unwrap() + 1;
        assert!(!json_field_has_object_key_boundary(top_level, prose_quote));

        let nested_only = "{\"nested\":{\"x\":99},\"prose\":\"the \\\"x\\\": 7 should not count\"}";
        assert_eq!(extract_json_field_value(nested_only, "x"), None);
        assert_eq!(
            parse_text_world_pos("{\"x\":12.5,\"y\":-4}"),
            Some((12.5, -4.0, "json_xy"))
        );
        assert_eq!(parse_text_world_pos("{\"nested\":{\"x\":12,\"y\":-4}}"), None);
        assert_eq!(parse_text_world_pos(" 7 , 8 "), Some((7.0, 8.0, "pair_comma")));
    }

    #[test]
    fn parse_text_literals_reject_trailing_garbage() {
        assert_eq!(parse_text_bool("{\"value\":falsehood}"), None);
        assert_eq!(parse_text_f64("{\"value\":12abc}"), None);
    }

    #[test]
    fn runtime_custom_packet_surface_bridges_native_remote_message_events() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        for (key, semantic) in [
            (
                NATIVE_SERVER_MESSAGE_KEY,
                RuntimeCustomPacketSemanticKind::ServerMessage,
            ),
            (
                NATIVE_CHAT_MESSAGE_KEY,
                RuntimeCustomPacketSemanticKind::ChatMessage,
            ),
            (
                NATIVE_SET_HUD_TEXT_KEY,
                RuntimeCustomPacketSemanticKind::HudText,
            ),
            (
                NATIVE_SET_HUD_TEXT_RELIABLE_KEY,
                RuntimeCustomPacketSemanticKind::HudText,
            ),
            (
                NATIVE_ANNOUNCE_KEY,
                RuntimeCustomPacketSemanticKind::Announce,
            ),
            (
                NATIVE_CLIPBOARD_KEY,
                RuntimeCustomPacketSemanticKind::Clipboard,
            ),
            (
                NATIVE_OPEN_URI_KEY,
                RuntimeCustomPacketSemanticKind::OpenUri,
            ),
        ] {
            state.register(&RuntimeCustomPacketSemanticSpec {
                key: key.to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic,
            });
        }

        state.observe_events(&[
            ClientSessionEvent::ServerMessage {
                message: "server ready".to_string(),
            },
            ClientSessionEvent::ChatMessage {
                message: "[cyan]hello".to_string(),
                unformatted: Some("hello".to_string()),
                sender_entity_id: Some(7),
            },
            ClientSessionEvent::SetHudText {
                message: Some("hud-u".to_string()),
            },
            ClientSessionEvent::SetHudTextReliable {
                message: Some("hud-r".to_string()),
            },
            ClientSessionEvent::Announce {
                message: Some("announce".to_string()),
            },
            ClientSessionEvent::CopyToClipboard {
                text: Some("copied".to_string()),
            },
            ClientSessionEvent::OpenUri {
                uri: Some("https://example.invalid".to_string()),
            },
        ]);

        let summaries = state.latest_summary_entries(8);
        assert_eq!(summaries.len(), 7);
        assert!(summaries.iter().any(|entry| {
            entry.key == NATIVE_SERVER_MESSAGE_KEY && entry.stable_value == "server ready"
        }));
        assert!(summaries.iter().any(|entry| {
            entry.key == NATIVE_CHAT_MESSAGE_KEY && entry.stable_value == "hello"
        }));
        assert!(summaries.iter().any(|entry| {
            entry.key == NATIVE_SET_HUD_TEXT_KEY && entry.stable_value == "hud-u"
        }));
        assert!(summaries.iter().any(|entry| {
            entry.key == NATIVE_SET_HUD_TEXT_RELIABLE_KEY && entry.stable_value == "hud-r"
        }));
        assert!(state
            .summary_lines()
            .iter()
            .any(|line| line.contains("key=\"openURI\"") && line.contains("event_reliable=1")));
    }

    #[test]
    fn runtime_custom_packet_surface_hides_native_hud_text_routes() {
        let mut state = RuntimeCustomPacketSurfaceState::default();
        for key in [NATIVE_SET_HUD_TEXT_KEY, NATIVE_SET_HUD_TEXT_RELIABLE_KEY] {
            state.register(&RuntimeCustomPacketSemanticSpec {
                key: key.to_string(),
                encoding: RuntimeCustomPacketSemanticEncoding::Text,
                semantic: RuntimeCustomPacketSemanticKind::HudText,
            });
        }

        state.observe_events(&[
            ClientSessionEvent::SetHudText {
                message: Some("hud-u".to_string()),
            },
            ClientSessionEvent::SetHudTextReliable {
                message: Some("hud-r".to_string()),
            },
            ClientSessionEvent::HideHudText,
        ]);

        assert!(state.overlay_summary_text(4).is_none());
        let lines = state.drain_lines();
        assert!(lines
            .iter()
            .any(|line| line.contains("runtime_custom_packet_surface_reset:")));
    }
}
