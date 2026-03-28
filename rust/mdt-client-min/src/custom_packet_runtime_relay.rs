use crate::client_session::{
    ClientLogicDataTransport, ClientPacketTransport, ClientSession, ClientSessionEvent,
};
use mdt_typeio::TypeIoObject;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

const MAX_PENDING_ENTRIES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeCustomPacketRelayEncoding {
    Text,
    Binary,
    LogicData,
}

impl RuntimeCustomPacketRelayEncoding {
    fn label(self) -> &'static str {
        match self {
            RuntimeCustomPacketRelayEncoding::Text => "text",
            RuntimeCustomPacketRelayEncoding::Binary => "binary",
            RuntimeCustomPacketRelayEncoding::LogicData => "logic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeCustomPacketRelayTransport {
    Packet(ClientPacketTransport),
    LogicData(ClientLogicDataTransport),
}

impl RuntimeCustomPacketRelayTransport {
    fn label(self) -> &'static str {
        match self {
            RuntimeCustomPacketRelayTransport::Packet(ClientPacketTransport::Tcp) => "tcp",
            RuntimeCustomPacketRelayTransport::Packet(ClientPacketTransport::Udp) => "udp",
            RuntimeCustomPacketRelayTransport::LogicData(ClientLogicDataTransport::Reliable) => {
                "reliable"
            }
            RuntimeCustomPacketRelayTransport::LogicData(ClientLogicDataTransport::Unreliable) => {
                "unreliable"
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeCustomPacketRelaySpec {
    Text {
        inbound_type: String,
        outbound_type: String,
        transport: ClientPacketTransport,
    },
    Binary {
        inbound_type: String,
        outbound_type: String,
        transport: ClientPacketTransport,
    },
    LogicData {
        inbound_channel: String,
        outbound_channel: String,
        transport: ClientLogicDataTransport,
    },
}

impl RuntimeCustomPacketRelaySpec {
    fn encoding(&self) -> RuntimeCustomPacketRelayEncoding {
        match self {
            RuntimeCustomPacketRelaySpec::Text { .. } => RuntimeCustomPacketRelayEncoding::Text,
            RuntimeCustomPacketRelaySpec::Binary { .. } => RuntimeCustomPacketRelayEncoding::Binary,
            RuntimeCustomPacketRelaySpec::LogicData { .. } => {
                RuntimeCustomPacketRelayEncoding::LogicData
            }
        }
    }

    fn inbound_key(&self) -> &str {
        match self {
            RuntimeCustomPacketRelaySpec::Text { inbound_type, .. }
            | RuntimeCustomPacketRelaySpec::Binary { inbound_type, .. } => inbound_type,
            RuntimeCustomPacketRelaySpec::LogicData {
                inbound_channel, ..
            } => inbound_channel,
        }
    }

    fn route_state(&self) -> RuntimeCustomPacketRelayRouteState {
        match self {
            RuntimeCustomPacketRelaySpec::Text {
                outbound_type,
                transport,
                ..
            }
            | RuntimeCustomPacketRelaySpec::Binary {
                outbound_type,
                transport,
                ..
            } => RuntimeCustomPacketRelayRouteState {
                outbound_key: outbound_type.clone(),
                transport: RuntimeCustomPacketRelayTransport::Packet(*transport),
                handler_count: 0,
                event_reliable_count: 0,
                event_unreliable_count: 0,
                last_preview: None,
            },
            RuntimeCustomPacketRelaySpec::LogicData {
                outbound_channel,
                transport,
                ..
            } => RuntimeCustomPacketRelayRouteState {
                outbound_key: outbound_channel.clone(),
                transport: RuntimeCustomPacketRelayTransport::LogicData(*transport),
                handler_count: 0,
                event_reliable_count: 0,
                event_unreliable_count: 0,
                last_preview: None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeCustomPacketRelayAction {
    Text {
        packet_type: String,
        contents: String,
        transport: ClientPacketTransport,
    },
    Binary {
        packet_type: String,
        contents: Vec<u8>,
        transport: ClientPacketTransport,
    },
    LogicData {
        channel: String,
        value: TypeIoObject,
        transport: ClientLogicDataTransport,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCustomPacketRelayEntry {
    pub action: RuntimeCustomPacketRelayAction,
    pub line: String,
}

#[derive(Debug)]
pub struct RuntimeCustomPacketRelays {
    state: Rc<RefCell<RuntimeCustomPacketRelayState>>,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketRelayState {
    text_routes: BTreeMap<String, Vec<RuntimeCustomPacketRelayRouteState>>,
    binary_routes: BTreeMap<String, Vec<RuntimeCustomPacketRelayRouteState>>,
    logic_routes: BTreeMap<String, Vec<RuntimeCustomPacketRelayRouteState>>,
    pending_entries: VecDeque<RuntimeCustomPacketRelayEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeCustomPacketRelayRouteState {
    outbound_key: String,
    transport: RuntimeCustomPacketRelayTransport,
    handler_count: usize,
    event_reliable_count: usize,
    event_unreliable_count: usize,
    last_preview: Option<String>,
}

impl RuntimeCustomPacketRelays {
    pub fn observe_events(&self, events: &[ClientSessionEvent]) {
        self.state.borrow_mut().observe_events(events);
    }

    pub fn drain_entries(&self) -> Vec<RuntimeCustomPacketRelayEntry> {
        self.state.borrow_mut().drain_entries()
    }

    pub fn summary_lines(&self) -> Vec<String> {
        self.state.borrow().summary_lines()
    }
}

impl RuntimeCustomPacketRelayState {
    fn register(&mut self, spec: &RuntimeCustomPacketRelaySpec) {
        let routes = match spec.encoding() {
            RuntimeCustomPacketRelayEncoding::Text => {
                self.text_routes.entry(spec.inbound_key().to_string())
            }
            RuntimeCustomPacketRelayEncoding::Binary => {
                self.binary_routes.entry(spec.inbound_key().to_string())
            }
            RuntimeCustomPacketRelayEncoding::LogicData => {
                self.logic_routes.entry(spec.inbound_key().to_string())
            }
        }
        .or_default();
        routes.push(spec.route_state());
    }

    fn record_text_handler(&mut self, inbound_key: &str, text: &str) {
        let Some(routes) = self.text_routes.get_mut(inbound_key) else {
            return;
        };
        let preview = truncate_for_preview(&text.escape_default().to_string(), 96);
        let mut queued_entries = Vec::with_capacity(routes.len());
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            route.last_preview = Some(preview.clone());
            let transport = match route.transport {
                RuntimeCustomPacketRelayTransport::Packet(transport) => transport,
                RuntimeCustomPacketRelayTransport::LogicData(_) => continue,
            };
            queued_entries.push(RuntimeCustomPacketRelayEntry {
                action: RuntimeCustomPacketRelayAction::Text {
                    packet_type: route.outbound_key.clone(),
                    contents: text.to_string(),
                    transport,
                },
                line: format!(
                    "runtime_custom_packet_relay: encoding=text inbound={inbound_key:?} outbound={:?} count={} transport={} len={} preview={preview:?}",
                    route.outbound_key,
                    route.handler_count,
                    route.transport.label(),
                    text.len()
                ),
            });
        }
        for entry in queued_entries {
            self.enqueue_pending_entry(entry);
        }
    }

    fn record_binary_handler(&mut self, inbound_key: &str, bytes: &[u8]) {
        let Some(routes) = self.binary_routes.get_mut(inbound_key) else {
            return;
        };
        let prefix_len = bytes.len().min(16);
        let preview = encode_hex_prefix(&bytes[..prefix_len]);
        let mut queued_entries = Vec::with_capacity(routes.len());
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            route.last_preview = Some(preview.clone());
            let transport = match route.transport {
                RuntimeCustomPacketRelayTransport::Packet(transport) => transport,
                RuntimeCustomPacketRelayTransport::LogicData(_) => continue,
            };
            queued_entries.push(RuntimeCustomPacketRelayEntry {
                action: RuntimeCustomPacketRelayAction::Binary {
                    packet_type: route.outbound_key.clone(),
                    contents: bytes.to_vec(),
                    transport,
                },
                line: format!(
                    "runtime_custom_packet_relay: encoding=binary inbound={inbound_key:?} outbound={:?} count={} transport={} len={} hex_prefix={preview}",
                    route.outbound_key,
                    route.handler_count,
                    route.transport.label(),
                    bytes.len()
                ),
            });
        }
        for entry in queued_entries {
            self.enqueue_pending_entry(entry);
        }
    }

    fn record_logic_data_handler(&mut self, inbound_key: &str, value: &TypeIoObject) {
        let Some(routes) = self.logic_routes.get_mut(inbound_key) else {
            return;
        };
        let preview = truncate_for_preview(&format!("{value:?}"), 96);
        let mut queued_entries = Vec::with_capacity(routes.len());
        for route in routes {
            route.handler_count = route.handler_count.saturating_add(1);
            route.last_preview = Some(preview.clone());
            let transport = match route.transport {
                RuntimeCustomPacketRelayTransport::LogicData(transport) => transport,
                RuntimeCustomPacketRelayTransport::Packet(_) => continue,
            };
            queued_entries.push(RuntimeCustomPacketRelayEntry {
                action: RuntimeCustomPacketRelayAction::LogicData {
                    channel: route.outbound_key.clone(),
                    value: value.clone(),
                    transport,
                },
                line: format!(
                    "runtime_custom_packet_relay: encoding=logic inbound={inbound_key:?} outbound={:?} count={} transport={} kind={:?} preview={preview:?}",
                    route.outbound_key,
                    route.handler_count,
                    route.transport.label(),
                    value.kind()
                ),
            });
        }
        for entry in queued_entries {
            self.enqueue_pending_entry(entry);
        }
    }

    fn observe_events(&mut self, events: &[ClientSessionEvent]) {
        for event in events {
            match event {
                ClientSessionEvent::ClientPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerPacketReliable { packet_type, .. } => {
                    self.record_event(RuntimeCustomPacketRelayEncoding::Text, packet_type, true);
                }
                ClientSessionEvent::ClientPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerPacketUnreliable { packet_type, .. } => {
                    self.record_event(RuntimeCustomPacketRelayEncoding::Text, packet_type, false);
                }
                ClientSessionEvent::ClientBinaryPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketReliable { packet_type, .. } => {
                    self.record_event(RuntimeCustomPacketRelayEncoding::Binary, packet_type, true);
                }
                ClientSessionEvent::ClientBinaryPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketUnreliable { packet_type, .. } => {
                    self.record_event(RuntimeCustomPacketRelayEncoding::Binary, packet_type, false);
                }
                ClientSessionEvent::ClientLogicDataReliable { channel, .. } => {
                    self.record_event(RuntimeCustomPacketRelayEncoding::LogicData, channel, true);
                }
                ClientSessionEvent::ClientLogicDataUnreliable { channel, .. } => {
                    self.record_event(RuntimeCustomPacketRelayEncoding::LogicData, channel, false);
                }
                _ => {}
            }
        }
    }

    fn record_event(
        &mut self,
        encoding: RuntimeCustomPacketRelayEncoding,
        key: &str,
        reliable: bool,
    ) {
        let routes = match encoding {
            RuntimeCustomPacketRelayEncoding::Text => self.text_routes.get_mut(key),
            RuntimeCustomPacketRelayEncoding::Binary => self.binary_routes.get_mut(key),
            RuntimeCustomPacketRelayEncoding::LogicData => self.logic_routes.get_mut(key),
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

    fn enqueue_pending_entry(&mut self, entry: RuntimeCustomPacketRelayEntry) {
        while self.pending_entries.len() >= MAX_PENDING_ENTRIES {
            self.pending_entries.pop_front();
        }
        self.pending_entries.push_back(entry);
    }

    fn drain_entries(&mut self) -> Vec<RuntimeCustomPacketRelayEntry> {
        self.pending_entries.drain(..).collect()
    }

    fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        append_summary_lines(
            &mut lines,
            RuntimeCustomPacketRelayEncoding::Text,
            &self.text_routes,
        );
        append_summary_lines(
            &mut lines,
            RuntimeCustomPacketRelayEncoding::Binary,
            &self.binary_routes,
        );
        append_summary_lines(
            &mut lines,
            RuntimeCustomPacketRelayEncoding::LogicData,
            &self.logic_routes,
        );
        lines
    }
}

pub fn install_runtime_custom_packet_relays(
    session: &mut ClientSession,
    specs: &[RuntimeCustomPacketRelaySpec],
) -> Option<RuntimeCustomPacketRelays> {
    if specs.is_empty() {
        return None;
    }

    let state = Rc::new(RefCell::new(RuntimeCustomPacketRelayState::default()));
    for spec in specs {
        state.borrow_mut().register(spec);
        match spec {
            RuntimeCustomPacketRelaySpec::Text { inbound_type, .. } => {
                let inbound_key = inbound_type.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_packet_handler(inbound_type.clone(), move |contents| {
                    shared_state
                        .borrow_mut()
                        .record_text_handler(&inbound_key, contents);
                });
            }
            RuntimeCustomPacketRelaySpec::Binary { inbound_type, .. } => {
                let inbound_key = inbound_type.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_binary_packet_handler(inbound_type.clone(), move |contents| {
                    shared_state
                        .borrow_mut()
                        .record_binary_handler(&inbound_key, contents);
                });
            }
            RuntimeCustomPacketRelaySpec::LogicData {
                inbound_channel, ..
            } => {
                let inbound_key = inbound_channel.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_logic_data_handler(inbound_channel.clone(), move |_, value| {
                    shared_state
                        .borrow_mut()
                        .record_logic_data_handler(&inbound_key, value);
                });
            }
        }
    }

    Some(RuntimeCustomPacketRelays { state })
}

pub fn build_runtime_custom_packet_relay_specs(
    text_specs: &[String],
    binary_specs: &[String],
    logic_specs: &[String],
) -> Result<Vec<RuntimeCustomPacketRelaySpec>, String> {
    let mut specs = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in text_specs {
        let spec = parse_packet_relay_spec("--relay-client-packet", raw, false)?;
        let dedupe_key = format!("text\0{}\0{}\0{}", spec.0, spec.1, spec.2.label());
        if seen.insert(dedupe_key) {
            specs.push(RuntimeCustomPacketRelaySpec::Text {
                inbound_type: spec.0,
                outbound_type: spec.1,
                transport: match spec.2 {
                    RuntimeCustomPacketRelayTransport::Packet(transport) => transport,
                    RuntimeCustomPacketRelayTransport::LogicData(_) => unreachable!(),
                },
            });
        }
    }
    for raw in binary_specs {
        let spec = parse_packet_relay_spec("--relay-client-binary-packet", raw, false)?;
        let dedupe_key = format!("binary\0{}\0{}\0{}", spec.0, spec.1, spec.2.label());
        if seen.insert(dedupe_key) {
            specs.push(RuntimeCustomPacketRelaySpec::Binary {
                inbound_type: spec.0,
                outbound_type: spec.1,
                transport: match spec.2 {
                    RuntimeCustomPacketRelayTransport::Packet(transport) => transport,
                    RuntimeCustomPacketRelayTransport::LogicData(_) => unreachable!(),
                },
            });
        }
    }
    for raw in logic_specs {
        let spec = parse_packet_relay_spec("--relay-client-logic-data", raw, true)?;
        let dedupe_key = format!("logic\0{}\0{}\0{}", spec.0, spec.1, spec.2.label());
        if seen.insert(dedupe_key) {
            specs.push(RuntimeCustomPacketRelaySpec::LogicData {
                inbound_channel: spec.0,
                outbound_channel: spec.1,
                transport: match spec.2 {
                    RuntimeCustomPacketRelayTransport::LogicData(transport) => transport,
                    RuntimeCustomPacketRelayTransport::Packet(_) => unreachable!(),
                },
            });
        }
    }
    Ok(specs)
}

fn parse_packet_relay_spec(
    flag: &str,
    raw: &str,
    logic_data: bool,
) -> Result<(String, String, RuntimeCustomPacketRelayTransport), String> {
    let parts = raw.split('@').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(format!(
            "invalid {flag}, expected <inbound@outbound@reliable|unreliable|tcp|udp>"
        ));
    }

    let inbound = parts[0].trim();
    let outbound = parts[1].trim();
    let transport = parts[2].trim();
    if inbound.is_empty() || outbound.is_empty() || transport.is_empty() {
        return Err(format!(
            "invalid {flag}, expected <inbound@outbound@reliable|unreliable|tcp|udp>"
        ));
    }

    let transport = if logic_data {
        parse_logic_transport(flag, transport)?
    } else {
        parse_packet_transport(flag, transport)?
    };
    Ok((inbound.to_string(), outbound.to_string(), transport))
}

fn parse_packet_transport(
    flag: &str,
    value: &str,
) -> Result<RuntimeCustomPacketRelayTransport, String> {
    if value.eq_ignore_ascii_case("reliable") || value.eq_ignore_ascii_case("tcp") {
        return Ok(RuntimeCustomPacketRelayTransport::Packet(
            ClientPacketTransport::Tcp,
        ));
    }
    if value.eq_ignore_ascii_case("unreliable") || value.eq_ignore_ascii_case("udp") {
        return Ok(RuntimeCustomPacketRelayTransport::Packet(
            ClientPacketTransport::Udp,
        ));
    }
    Err(format!(
        "invalid {flag} transport, expected <reliable|unreliable|tcp|udp>"
    ))
}

fn parse_logic_transport(
    flag: &str,
    value: &str,
) -> Result<RuntimeCustomPacketRelayTransport, String> {
    if value.eq_ignore_ascii_case("reliable") || value.eq_ignore_ascii_case("tcp") {
        return Ok(RuntimeCustomPacketRelayTransport::LogicData(
            ClientLogicDataTransport::Reliable,
        ));
    }
    if value.eq_ignore_ascii_case("unreliable") || value.eq_ignore_ascii_case("udp") {
        return Ok(RuntimeCustomPacketRelayTransport::LogicData(
            ClientLogicDataTransport::Unreliable,
        ));
    }
    Err(format!(
        "invalid {flag} transport, expected <reliable|unreliable|tcp|udp>"
    ))
}

fn append_summary_lines(
    lines: &mut Vec<String>,
    encoding: RuntimeCustomPacketRelayEncoding,
    routes: &BTreeMap<String, Vec<RuntimeCustomPacketRelayRouteState>>,
) {
    for (inbound_key, route_states) in routes {
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
                "runtime_custom_packet_relay_summary: encoding={} inbound={inbound_key:?} outbound={:?} transport={} count={} event_reliable={} event_unreliable={} event_total={} parity={parity} last={:?}",
                encoding.label(),
                route.outbound_key,
                route.transport.label(),
                route.handler_count,
                route.event_reliable_count,
                route.event_unreliable_count,
                event_total,
                route.last_preview
            ));
        }
    }
}

fn encode_hex_prefix(bytes: &[u8]) -> String {
    bytes
        .iter()
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

    #[test]
    fn build_runtime_custom_packet_relay_specs_parses_and_deduplicates() {
        let specs = build_runtime_custom_packet_relay_specs(
            &[
                "custom.ping@custom.pong@tcp".to_string(),
                "custom.ping@custom.pong@reliable".to_string(),
            ],
            &["bin.ping@bin.pong@udp".to_string()],
            &[
                "logic.ping@logic.pong@reliable".to_string(),
                "logic.ping@logic.pong@tcp".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(
            specs,
            vec![
                RuntimeCustomPacketRelaySpec::Text {
                    inbound_type: "custom.ping".to_string(),
                    outbound_type: "custom.pong".to_string(),
                    transport: ClientPacketTransport::Tcp,
                },
                RuntimeCustomPacketRelaySpec::Binary {
                    inbound_type: "bin.ping".to_string(),
                    outbound_type: "bin.pong".to_string(),
                    transport: ClientPacketTransport::Udp,
                },
                RuntimeCustomPacketRelaySpec::LogicData {
                    inbound_channel: "logic.ping".to_string(),
                    outbound_channel: "logic.pong".to_string(),
                    transport: ClientLogicDataTransport::Reliable,
                },
            ]
        );
    }

    #[test]
    fn runtime_custom_packet_relay_state_tracks_text_binary_and_logic_actions() {
        let mut state = RuntimeCustomPacketRelayState::default();
        state.register(&RuntimeCustomPacketRelaySpec::Text {
            inbound_type: "custom.ping".to_string(),
            outbound_type: "custom.pong".to_string(),
            transport: ClientPacketTransport::Tcp,
        });
        state.register(&RuntimeCustomPacketRelaySpec::Binary {
            inbound_type: "bin.ping".to_string(),
            outbound_type: "bin.pong".to_string(),
            transport: ClientPacketTransport::Udp,
        });
        state.register(&RuntimeCustomPacketRelaySpec::LogicData {
            inbound_channel: "logic.ping".to_string(),
            outbound_channel: "logic.pong".to_string(),
            transport: ClientLogicDataTransport::Unreliable,
        });

        state.record_text_handler("custom.ping", "wave ready");
        state.record_binary_handler("bin.ping", &[0xAA, 0xBB, 0xCC]);
        state.record_logic_data_handler("logic.ping", &TypeIoObject::Int(7));
        state.observe_events(&[
            ClientSessionEvent::ServerPacketReliable {
                packet_type: "custom.ping".to_string(),
                contents: "wave ready".to_string(),
            },
            ClientSessionEvent::ServerBinaryPacketUnreliable {
                packet_type: "bin.ping".to_string(),
                contents: vec![0xAA, 0xBB, 0xCC],
            },
            ClientSessionEvent::ClientLogicDataReliable {
                channel: "logic.ping".to_string(),
                value: TypeIoObject::Int(7),
            },
        ]);

        let entries = state.drain_entries();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].line.contains("encoding=text"));
        assert!(entries[0].line.contains("inbound=\"custom.ping\""));
        assert!(entries[0].line.contains("outbound=\"custom.pong\""));
        assert_eq!(
            entries[0].action,
            RuntimeCustomPacketRelayAction::Text {
                packet_type: "custom.pong".to_string(),
                contents: "wave ready".to_string(),
                transport: ClientPacketTransport::Tcp,
            }
        );
        assert!(entries[1].line.contains("encoding=binary"));
        assert!(entries[1].line.contains("hex_prefix=aabbcc"));
        assert_eq!(
            entries[1].action,
            RuntimeCustomPacketRelayAction::Binary {
                packet_type: "bin.pong".to_string(),
                contents: vec![0xAA, 0xBB, 0xCC],
                transport: ClientPacketTransport::Udp,
            }
        );
        assert!(entries[2].line.contains("encoding=logic"));
        assert!(entries[2].line.contains("kind=\"int\""));
        assert_eq!(
            entries[2].action,
            RuntimeCustomPacketRelayAction::LogicData {
                channel: "logic.pong".to_string(),
                value: TypeIoObject::Int(7),
                transport: ClientLogicDataTransport::Unreliable,
            }
        );

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 3);
        assert!(summaries[0].contains("encoding=text"));
        assert!(summaries[0].contains("parity=ok"));
        assert!(summaries[1].contains("event_unreliable=1"));
        assert!(summaries[2].contains("transport=unreliable"));
        assert!(summaries[2].contains("event_reliable=1"));
    }

    #[test]
    fn runtime_custom_packet_relays_bounds_pending_entries_growth() {
        let mut state = RuntimeCustomPacketRelayState::default();
        state.register(&RuntimeCustomPacketRelaySpec::Text {
            inbound_type: "custom.ping".to_string(),
            outbound_type: "custom.pong".to_string(),
            transport: ClientPacketTransport::Tcp,
        });

        let total_events = MAX_PENDING_ENTRIES + 44;
        for index in 0..total_events {
            state.record_text_handler("custom.ping", &format!("wave-{index:03}"));
        }

        assert_eq!(state.pending_entries.len(), MAX_PENDING_ENTRIES);

        let entries = state.drain_entries();
        assert_eq!(entries.len(), MAX_PENDING_ENTRIES);
        assert_eq!(
            entries.first().unwrap().action,
            RuntimeCustomPacketRelayAction::Text {
                packet_type: "custom.pong".to_string(),
                contents: "wave-044".to_string(),
                transport: ClientPacketTransport::Tcp,
            }
        );
        assert_eq!(
            entries.last().unwrap().action,
            RuntimeCustomPacketRelayAction::Text {
                packet_type: "custom.pong".to_string(),
                contents: format!("wave-{index:03}", index = total_events - 1),
                transport: ClientPacketTransport::Tcp,
            }
        );
    }
}
