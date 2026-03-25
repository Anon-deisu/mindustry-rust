use crate::client_session::{ClientLogicDataTransport, ClientSession, ClientSessionEvent};
use mdt_typeio::{unpack_point2, TypeIoObject};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeCustomPacketSemanticEncoding {
    Text,
    Binary,
    LogicData,
}

impl RuntimeCustomPacketSemanticEncoding {
    fn label(self) -> &'static str {
        match self {
            RuntimeCustomPacketSemanticEncoding::Text => "text",
            RuntimeCustomPacketSemanticEncoding::Binary => "binary",
            RuntimeCustomPacketSemanticEncoding::LogicData => "logic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeCustomPacketSemanticKind {
    ServerMessage,
    ChatMessage,
    HudText,
    Announce,
    Clipboard,
    OpenUri,
    WorldPos,
    BuildPos,
    UnitId,
    Team,
    Bool,
    Number,
}

impl RuntimeCustomPacketSemanticKind {
    fn label(self) -> &'static str {
        match self {
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
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeCustomPacketSemanticSpec {
    pub key: String,
    pub encoding: RuntimeCustomPacketSemanticEncoding,
    pub semantic: RuntimeCustomPacketSemanticKind,
}

#[derive(Debug)]
pub struct RuntimeCustomPacketSemantics {
    state: Rc<RefCell<RuntimeCustomPacketSemanticsState>>,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketSemanticsState {
    text_routes: BTreeMap<String, Vec<RuntimeCustomPacketSemanticRouteState>>,
    binary_routes: BTreeMap<String, Vec<RuntimeCustomPacketSemanticRouteState>>,
    logic_routes: BTreeMap<String, Vec<RuntimeCustomPacketSemanticRouteState>>,
    pending_lines: VecDeque<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeCustomPacketSemanticRouteState {
    semantic: RuntimeCustomPacketSemanticKind,
    handler_count: usize,
    event_reliable_count: usize,
    event_unreliable_count: usize,
    decode_error_count: usize,
    last_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedSemantic {
    detail: String,
    stable_value: String,
}

const NATIVE_SERVER_MESSAGE_KEY: &str = "sendMessage";
const NATIVE_CHAT_MESSAGE_KEY: &str = "sendMessageWithSender";
const NATIVE_SET_HUD_TEXT_KEY: &str = "setHudText";
const NATIVE_SET_HUD_TEXT_RELIABLE_KEY: &str = "setHudTextReliable";
const NATIVE_ANNOUNCE_KEY: &str = "announce";
const NATIVE_CLIPBOARD_KEY: &str = "copyToClipboard";
const NATIVE_OPEN_URI_KEY: &str = "openURI";

impl RuntimeCustomPacketSemantics {
    pub fn observe_events(&self, events: &[ClientSessionEvent]) {
        self.state.borrow_mut().observe_events(events);
    }

    pub fn drain_lines(&self) -> Vec<String> {
        self.state.borrow_mut().drain_lines()
    }

    pub fn summary_lines(&self) -> Vec<String> {
        self.state.borrow().summary_lines()
    }
}

impl RuntimeCustomPacketSemanticsState {
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
        routes.push(RuntimeCustomPacketSemanticRouteState {
            semantic: spec.semantic,
            handler_count: 0,
            event_reliable_count: 0,
            event_unreliable_count: 0,
            decode_error_count: 0,
            last_value: None,
        });
    }

    fn record_text_handler(&mut self, key: &str, text: &str) {
        let Some(routes) = self.text_routes.get_mut(key) else {
            return;
        };
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            match render_text_semantic(route.semantic, text) {
                Ok(rendered) => {
                    route.last_value = Some(rendered.stable_value.clone());
                    self.pending_lines.push_back(format!(
                        "runtime_custom_packet_semantic: encoding=text key={key:?} semantic={} count={} {}",
                        route.semantic.label(),
                        route.handler_count,
                        rendered.detail
                    ));
                }
                Err(reason) => {
                    route.decode_error_count = route.decode_error_count.saturating_add(1);
                    self.pending_lines.push_back(format!(
                        "runtime_custom_packet_semantic_decode_error: encoding=text key={key:?} semantic={} count={} reason={reason:?} preview={:?}",
                        route.semantic.label(),
                        route.decode_error_count,
                        truncate_for_preview(&text.escape_default().to_string(), 96)
                    ));
                }
            }
        }
    }

    fn record_binary_handler(&mut self, key: &str, bytes: &[u8]) {
        let Some(routes) = self.binary_routes.get_mut(key) else {
            return;
        };
        let text = std::str::from_utf8(bytes).ok();
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            let Some(text) = text else {
                route.decode_error_count = route.decode_error_count.saturating_add(1);
                self.pending_lines.push_back(format!(
                    "runtime_custom_packet_semantic_decode_error: encoding=binary key={key:?} semantic={} count={} reason=\"invalid_utf8\" len={} hex_prefix={}",
                    route.semantic.label(),
                    route.decode_error_count,
                    bytes.len(),
                    encode_hex_prefix(bytes)
                ));
                continue;
            };
            match render_text_semantic(route.semantic, text) {
                Ok(rendered) => {
                    route.last_value = Some(rendered.stable_value.clone());
                    self.pending_lines.push_back(format!(
                        "runtime_custom_packet_semantic: encoding=binary key={key:?} semantic={} count={} {}",
                        route.semantic.label(),
                        route.handler_count,
                        rendered.detail
                    ));
                }
                Err(reason) => {
                    route.decode_error_count = route.decode_error_count.saturating_add(1);
                    self.pending_lines.push_back(format!(
                        "runtime_custom_packet_semantic_decode_error: encoding=binary key={key:?} semantic={} count={} reason={reason:?} preview={:?}",
                        route.semantic.label(),
                        route.decode_error_count,
                        truncate_for_preview(&text.escape_default().to_string(), 96)
                    ));
                }
            }
        }
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
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            match render_logic_semantic(route.semantic, value) {
                Ok(rendered) => {
                    route.last_value = Some(rendered.stable_value.clone());
                    self.pending_lines.push_back(format!(
                        "runtime_custom_packet_semantic: encoding=logic key={key:?} semantic={} count={} transport={} {}",
                        route.semantic.label(),
                        route.handler_count,
                        logic_data_transport_label(transport),
                        rendered.detail
                    ));
                }
                Err(reason) => {
                    route.decode_error_count = route.decode_error_count.saturating_add(1);
                    self.pending_lines.push_back(format!(
                        "runtime_custom_packet_semantic_decode_error: encoding=logic key={key:?} semantic={} count={} transport={} reason={reason:?} kind={:?} preview={:?}",
                        route.semantic.label(),
                        route.decode_error_count,
                        logic_data_transport_label(transport),
                        value.kind(),
                        truncate_for_preview(&format!("{value:?}"), 96)
                    ));
                }
            }
        }
    }

    fn observe_events(&mut self, events: &[ClientSessionEvent]) {
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
                | ClientSessionEvent::ServerPacketUnreliable { packet_type, .. } => {
                    self.record_event(
                        RuntimeCustomPacketSemanticEncoding::Text,
                        packet_type,
                        false,
                    );
                }
                ClientSessionEvent::ClientBinaryPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketReliable { packet_type, .. } => {
                    self.record_event(
                        RuntimeCustomPacketSemanticEncoding::Binary,
                        packet_type,
                        true,
                    );
                }
                ClientSessionEvent::ClientBinaryPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketUnreliable { packet_type, .. } => {
                    self.record_event(
                        RuntimeCustomPacketSemanticEncoding::Binary,
                        packet_type,
                        false,
                    );
                }
                ClientSessionEvent::ClientLogicDataReliable { channel, .. } => {
                    self.record_event(
                        RuntimeCustomPacketSemanticEncoding::LogicData,
                        channel,
                        true,
                    );
                }
                ClientSessionEvent::ClientLogicDataUnreliable { channel, .. } => {
                    self.record_event(
                        RuntimeCustomPacketSemanticEncoding::LogicData,
                        channel,
                        false,
                    );
                }
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
                if route.last_value.take().is_some() {
                    cleared = cleared.saturating_add(1);
                }
            }
        }
        if cleared > 0 {
            self.pending_lines.push_back(format!(
                "runtime_custom_packet_semantic_reset: reason={reason} cleared_routes={cleared}"
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
}

pub fn install_runtime_custom_packet_semantics(
    session: &mut ClientSession,
    specs: &[RuntimeCustomPacketSemanticSpec],
) -> Option<RuntimeCustomPacketSemantics> {
    if specs.is_empty() {
        return None;
    }

    let state = Rc::new(RefCell::new(RuntimeCustomPacketSemanticsState::default()));
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

    Some(RuntimeCustomPacketSemantics { state })
}

pub fn build_runtime_custom_packet_semantic_specs(
    text_specs: &[String],
    binary_specs: &[String],
    logic_specs: &[String],
) -> Result<Vec<RuntimeCustomPacketSemanticSpec>, String> {
    let mut specs = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in text_specs {
        let spec = parse_semantic_spec(
            "--consume-client-packet",
            raw,
            RuntimeCustomPacketSemanticEncoding::Text,
        )?;
        if seen.insert(spec.clone()) {
            specs.push(spec);
        }
    }
    for raw in binary_specs {
        let spec = parse_semantic_spec(
            "--consume-client-binary-packet",
            raw,
            RuntimeCustomPacketSemanticEncoding::Binary,
        )?;
        if seen.insert(spec.clone()) {
            specs.push(spec);
        }
    }
    for raw in logic_specs {
        let spec = parse_semantic_spec(
            "--consume-client-logic-data",
            raw,
            RuntimeCustomPacketSemanticEncoding::LogicData,
        )?;
        if seen.insert(spec.clone()) {
            specs.push(spec);
        }
    }
    Ok(specs)
}

fn parse_semantic_spec(
    flag: &str,
    raw: &str,
    encoding: RuntimeCustomPacketSemanticEncoding,
) -> Result<RuntimeCustomPacketSemanticSpec, String> {
    let mut parts = raw.splitn(2, '@');
    let key = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid {flag}, expected <type@semantic>"))?;
    let semantic = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid {flag}, expected <type@semantic>"))?;
    Ok(RuntimeCustomPacketSemanticSpec {
        key: key.to_string(),
        encoding,
        semantic: parse_semantic_kind(flag, semantic)?,
    })
}

fn parse_semantic_kind(flag: &str, raw: &str) -> Result<RuntimeCustomPacketSemanticKind, String> {
    match raw {
        "server-message" => Ok(RuntimeCustomPacketSemanticKind::ServerMessage),
        "chat-message" => Ok(RuntimeCustomPacketSemanticKind::ChatMessage),
        "hud-text" => Ok(RuntimeCustomPacketSemanticKind::HudText),
        "announce" => Ok(RuntimeCustomPacketSemanticKind::Announce),
        "clipboard" => Ok(RuntimeCustomPacketSemanticKind::Clipboard),
        "open-uri" => Ok(RuntimeCustomPacketSemanticKind::OpenUri),
        "world-pos" => Ok(RuntimeCustomPacketSemanticKind::WorldPos),
        "build-pos" => Ok(RuntimeCustomPacketSemanticKind::BuildPos),
        "unit-id" => Ok(RuntimeCustomPacketSemanticKind::UnitId),
        "team" => Ok(RuntimeCustomPacketSemanticKind::Team),
        "bool" => Ok(RuntimeCustomPacketSemanticKind::Bool),
        "number" => Ok(RuntimeCustomPacketSemanticKind::Number),
        _ => Err(format!(
            "invalid {flag} semantic {raw:?}, expected one of server-message|chat-message|hud-text|announce|clipboard|open-uri|world-pos|build-pos|unit-id|team|bool|number"
        )),
    }
}

fn append_summary_lines(
    lines: &mut Vec<String>,
    encoding: RuntimeCustomPacketSemanticEncoding,
    routes: &BTreeMap<String, Vec<RuntimeCustomPacketSemanticRouteState>>,
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
                "runtime_custom_packet_semantic_summary: encoding={} key={key:?} semantic={} count={} event_reliable={} event_unreliable={} event_total={} decode_errors={} parity={parity} last={:?}",
                encoding.label(),
                route.semantic.label(),
                route.handler_count,
                route.event_reliable_count,
                route.event_unreliable_count,
                event_total,
                route.decode_error_count,
                route.last_value
            ));
        }
    }
}

fn render_text_semantic(
    semantic: RuntimeCustomPacketSemanticKind,
    text: &str,
) -> Result<RenderedSemantic, &'static str> {
    match semantic {
        RuntimeCustomPacketSemanticKind::ServerMessage
        | RuntimeCustomPacketSemanticKind::ChatMessage
        | RuntimeCustomPacketSemanticKind::HudText
        | RuntimeCustomPacketSemanticKind::Announce
        | RuntimeCustomPacketSemanticKind::Clipboard
        | RuntimeCustomPacketSemanticKind::OpenUri => render_message_like_text(text),
        RuntimeCustomPacketSemanticKind::WorldPos => render_text_world_pos(text),
        RuntimeCustomPacketSemanticKind::BuildPos => render_text_i32(text, "build_pos"),
        RuntimeCustomPacketSemanticKind::UnitId => render_text_i32(text, "unit_id"),
        RuntimeCustomPacketSemanticKind::Team => render_text_u8(text, "team"),
        RuntimeCustomPacketSemanticKind::Bool => render_text_bool(text),
        RuntimeCustomPacketSemanticKind::Number => render_text_number(text),
    }
}

fn render_logic_semantic(
    semantic: RuntimeCustomPacketSemanticKind,
    value: &TypeIoObject,
) -> Result<RenderedSemantic, &'static str> {
    match semantic {
        RuntimeCustomPacketSemanticKind::ServerMessage
        | RuntimeCustomPacketSemanticKind::ChatMessage
        | RuntimeCustomPacketSemanticKind::HudText
        | RuntimeCustomPacketSemanticKind::Announce
        | RuntimeCustomPacketSemanticKind::Clipboard
        | RuntimeCustomPacketSemanticKind::OpenUri => {
            let text = extract_logic_string(value).ok_or("no_string_payload")?;
            render_message_like_text(&text)
        }
        RuntimeCustomPacketSemanticKind::WorldPos => extract_logic_world_pos(value),
        RuntimeCustomPacketSemanticKind::BuildPos => extract_logic_build_pos(value),
        RuntimeCustomPacketSemanticKind::UnitId => extract_logic_unit_id(value),
        RuntimeCustomPacketSemanticKind::Team => extract_logic_team(value),
        RuntimeCustomPacketSemanticKind::Bool => extract_logic_bool(value),
        RuntimeCustomPacketSemanticKind::Number => extract_logic_number(value),
    }
}

fn render_message_like_text(text: &str) -> Result<RenderedSemantic, &'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("empty_text");
    }
    let preview = truncate_for_preview(&trimmed.escape_default().to_string(), 96);
    Ok(RenderedSemantic {
        detail: format!("message={preview:?}"),
        stable_value: preview,
    })
}

fn render_text_world_pos(text: &str) -> Result<RenderedSemantic, &'static str> {
    let (x, y, source) = parse_text_world_pos(text).ok_or("invalid_world_pos")?;
    Ok(RenderedSemantic {
        detail: format!("x={x} y={y} source={source}"),
        stable_value: format!("{x},{y}"),
    })
}

fn render_text_i32(text: &str, label: &str) -> Result<RenderedSemantic, &'static str> {
    let value = parse_text_i32(text).ok_or("invalid_integer")?;
    Ok(RenderedSemantic {
        detail: format!("{label}={value}"),
        stable_value: value.to_string(),
    })
}

fn render_text_u8(text: &str, label: &str) -> Result<RenderedSemantic, &'static str> {
    let value = parse_text_u8(text).ok_or("invalid_u8")?;
    Ok(RenderedSemantic {
        detail: format!("{label}={value}"),
        stable_value: value.to_string(),
    })
}

fn render_text_bool(text: &str) -> Result<RenderedSemantic, &'static str> {
    let value = parse_text_bool(text).ok_or("invalid_bool")?;
    Ok(RenderedSemantic {
        detail: format!("value={value}"),
        stable_value: value.to_string(),
    })
}

fn render_text_number(text: &str) -> Result<RenderedSemantic, &'static str> {
    let value = parse_text_f64(text).ok_or("invalid_number")?;
    Ok(RenderedSemantic {
        detail: format!("value={value}"),
        stable_value: value.to_string(),
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

fn extract_logic_world_pos(value: &TypeIoObject) -> Result<RenderedSemantic, &'static str> {
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
    Ok(RenderedSemantic {
        detail: format!("x={x} y={y} source={source}"),
        stable_value: format!("{x},{y}"),
    })
}

fn extract_logic_build_pos(value: &TypeIoObject) -> Result<RenderedSemantic, &'static str> {
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
    Ok(RenderedSemantic {
        detail: format!("build_pos={build_pos} source={source}"),
        stable_value: build_pos.to_string(),
    })
}

fn extract_logic_unit_id(value: &TypeIoObject) -> Result<RenderedSemantic, &'static str> {
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
    Ok(RenderedSemantic {
        detail: format!("unit_id={unit_id} source={source}"),
        stable_value: unit_id.to_string(),
    })
}

fn extract_logic_team(value: &TypeIoObject) -> Result<RenderedSemantic, &'static str> {
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
    Ok(RenderedSemantic {
        detail: format!("team={team} source={source}"),
        stable_value: team.to_string(),
    })
}

fn extract_logic_bool(value: &TypeIoObject) -> Result<RenderedSemantic, &'static str> {
    let direct = match value {
        TypeIoObject::Bool(flag) => Some((*flag, "bool")),
        TypeIoObject::ObjectArray(_) => None,
        _ => None,
    };
    let (flag, source) = match direct {
        Some(value) => value,
        None => value
            .find_first_dfs(|object| matches!(object, TypeIoObject::Bool(_)))
            .and_then(|matched| match matched.value {
                TypeIoObject::Bool(flag) => Some((*flag, "bool_nested")),
                _ => None,
            })
            .ok_or("no_bool_payload")?,
    };
    Ok(RenderedSemantic {
        detail: format!("value={flag} source={source}"),
        stable_value: flag.to_string(),
    })
}

fn extract_logic_number(value: &TypeIoObject) -> Result<RenderedSemantic, &'static str> {
    let number = logic_number_value(value)
        .or_else(|| {
            value
                .find_first_dfs(|object| logic_number_value(object).is_some())
                .and_then(|matched| logic_number_value(matched.value))
        })
        .ok_or("no_numeric_payload")?;
    Ok(RenderedSemantic {
        detail: format!("value={number}"),
        stable_value: number,
    })
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
    let x = extract_json_number_field(trimmed, "x")?;
    let y = extract_json_number_field(trimmed, "y")?;
    Some((x, y, "json_xy"))
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

    #[test]
    fn build_runtime_custom_packet_semantic_specs_parses_and_deduplicates() {
        let specs = build_runtime_custom_packet_semantic_specs(
            &[
                "custom.status@hud-text".to_string(),
                "custom.status@hud-text".to_string(),
            ],
            &["custom.uri@open-uri".to_string()],
            &[
                "logic.pos@world-pos".to_string(),
                "logic.pos@world-pos".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(
            specs,
            vec![
                RuntimeCustomPacketSemanticSpec {
                    key: "custom.status".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::Text,
                    semantic: RuntimeCustomPacketSemanticKind::HudText,
                },
                RuntimeCustomPacketSemanticSpec {
                    key: "custom.uri".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::Binary,
                    semantic: RuntimeCustomPacketSemanticKind::OpenUri,
                },
                RuntimeCustomPacketSemanticSpec {
                    key: "logic.pos".to_string(),
                    encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
                    semantic: RuntimeCustomPacketSemanticKind::WorldPos,
                },
            ]
        );
    }

    #[test]
    fn runtime_custom_packet_semantics_state_tracks_text_binary_and_logic_routes() {
        let mut state = RuntimeCustomPacketSemanticsState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "custom.status".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Text,
            semantic: RuntimeCustomPacketSemanticKind::HudText,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "custom.uri".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Binary,
            semantic: RuntimeCustomPacketSemanticKind::OpenUri,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "logic.pos".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::WorldPos,
        });

        state.record_text_handler("custom.status", "wave ready");
        state.record_binary_handler("custom.uri", b"https://example.invalid/path");
        state.record_logic_data_handler(
            "logic.pos",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::ObjectArray(vec![TypeIoObject::Point2 { x: 7, y: 9 }]),
        );
        state.observe_events(&[
            ClientSessionEvent::ServerPacketReliable {
                packet_type: "custom.status".to_string(),
                contents: "wave ready".to_string(),
            },
            ClientSessionEvent::ServerBinaryPacketUnreliable {
                packet_type: "custom.uri".to_string(),
                contents: b"https://example.invalid/path".to_vec(),
            },
            ClientSessionEvent::ClientLogicDataReliable {
                channel: "logic.pos".to_string(),
                value: TypeIoObject::Point2 { x: 7, y: 9 },
            },
        ]);

        let lines = state.drain_lines();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("encoding=text"));
        assert!(lines[0].contains("semantic=hud_text"));
        assert!(lines[0].contains("message=\"wave ready\""));
        assert!(lines[1].contains("encoding=binary"));
        assert!(lines[1].contains("semantic=open_uri"));
        assert!(lines[1].contains("https://example.invalid/path"));
        assert!(lines[2].contains("encoding=logic"));
        assert!(lines[2].contains("semantic=world_pos"));
        assert!(lines[2].contains("x=7"));
        assert!(lines[2].contains("y=9"));

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 3);
        assert!(summaries[0].contains("semantic=hud_text"));
        assert!(summaries[0].contains("parity=ok"));
        assert!(summaries[1].contains("event_unreliable=1"));
        assert!(summaries[2].contains("event_reliable=1"));
        assert!(summaries[2].contains("last=Some(\"7,9\")"));
    }

    #[test]
    fn runtime_custom_packet_semantics_state_records_decode_errors() {
        let mut state = RuntimeCustomPacketSemanticsState::default();
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "custom.bool".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::Binary,
            semantic: RuntimeCustomPacketSemanticKind::Bool,
        });
        state.register(&RuntimeCustomPacketSemanticSpec {
            key: "logic.build".to_string(),
            encoding: RuntimeCustomPacketSemanticEncoding::LogicData,
            semantic: RuntimeCustomPacketSemanticKind::BuildPos,
        });

        state.record_binary_handler("custom.bool", &[0xff, 0xfe, 0xfd]);
        state.record_logic_data_handler(
            "logic.build",
            ClientLogicDataTransport::Unreliable,
            &TypeIoObject::Bool(true),
        );

        let lines = state.drain_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("decode_error"));
        assert!(lines[0].contains("invalid_utf8"));
        assert!(lines[1].contains("decode_error"));
        assert!(lines[1].contains("no_build_pos_payload"));

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].contains("decode_errors=1"));
        assert!(summaries[1].contains("decode_errors=1"));
    }

    #[test]
    fn runtime_custom_packet_semantics_state_bridges_native_remote_message_events() {
        let mut state = RuntimeCustomPacketSemanticsState::default();
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

        let lines = state.drain_lines();
        assert_eq!(lines.len(), 7);
        assert!(lines
            .iter()
            .any(|line| line.contains("semantic=server_message")));
        assert!(lines
            .iter()
            .any(|line| line.contains("message=\"server ready\"")));
        assert!(lines
            .iter()
            .any(|line| line.contains("semantic=chat_message")));
        assert!(lines.iter().any(|line| line.contains("message=\"hello\"")));
        assert!(lines.iter().any(|line| line.contains("semantic=hud_text")));
        assert!(lines.iter().any(|line| line.contains("message=\"hud-r\"")));
        assert!(lines.iter().any(|line| line.contains("semantic=open_uri")));

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 7);
        assert!(summaries.iter().any(|line| {
            line.contains("key=\"sendMessage\"")
                && line.contains("event_reliable=1")
                && line.contains("last=Some(\"server ready\")")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("key=\"sendMessageWithSender\"") && line.contains("last=Some(\"hello\")")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("key=\"setHudText\"") && line.contains("event_unreliable=1")
        }));
        assert!(summaries.iter().any(|line| {
            line.contains("key=\"setHudTextReliable\"") && line.contains("event_reliable=1")
        }));
    }

    #[test]
    fn runtime_custom_packet_semantics_state_hides_native_hud_text_routes() {
        let mut state = RuntimeCustomPacketSemanticsState::default();
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

        let lines = state.drain_lines();
        assert!(lines
            .iter()
            .any(|line| line.contains("runtime_custom_packet_semantic_reset:")));
        let summaries = state.summary_lines();
        assert!(summaries
            .iter()
            .filter(|line| line.contains("semantic=hud_text"))
            .all(|line| line.contains("last=None")));
    }
}
