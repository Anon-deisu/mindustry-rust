use mdt_client_min::client_session::{ClientLogicDataTransport, ClientPacketTransport};
use mdt_client_min::custom_packet_runtime_relay::{
    RuntimeCustomPacketRelayAction, RuntimeCustomPacketRelayEncoding, RuntimeCustomPacketRelaySpec,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Default)]
pub struct RuntimeCustomPacketReplayBridge {
    state: RuntimeCustomPacketReplayBridgeState,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketReplayBridgeState {
    routes: BTreeMap<RuntimeCustomPacketReplayRouteKey, RuntimeCustomPacketReplayRouteState>,
    pending_lines: VecDeque<String>,
    next_update_serial: u64,
    reset_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeCustomPacketReplayTransport {
    Packet(ClientPacketTransport),
    Logic(ClientLogicDataTransport),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeCustomPacketReplayRouteKey {
    encoding: RuntimeCustomPacketRelayEncoding,
    key: String,
    transport: RuntimeCustomPacketReplayTransport,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketReplayRouteState {
    replay_count: usize,
    active: bool,
    last_preview: Option<String>,
    last_update_serial: u64,
}

impl PartialOrd for RuntimeCustomPacketReplayTransport {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RuntimeCustomPacketReplayTransport {
    fn cmp(&self, other: &Self) -> Ordering {
        transport_rank(*self)
            .cmp(&transport_rank(*other))
            .then_with(|| transport_label(*self).cmp(transport_label(*other)))
    }
}

impl PartialOrd for RuntimeCustomPacketReplayRouteKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RuntimeCustomPacketReplayRouteKey {
    fn cmp(&self, other: &Self) -> Ordering {
        encoding_rank(self.encoding)
            .cmp(&encoding_rank(other.encoding))
            .then_with(|| self.key.cmp(&other.key))
            .then_with(|| self.transport.cmp(&other.transport))
    }
}

impl RuntimeCustomPacketReplayBridge {
    pub fn from_specs(specs: &[RuntimeCustomPacketRelaySpec]) -> Option<Self> {
        if specs.is_empty() {
            return None;
        }
        let mut state = RuntimeCustomPacketReplayBridgeState::default();
        for spec in specs {
            state.routes.entry(route_key_from_spec(spec)).or_default();
        }
        Some(Self { state })
    }

    pub fn observe_action(&mut self, now_ms: u64, action: &RuntimeCustomPacketRelayAction) {
        let (route, preview) = route_key_and_preview_from_action(action);
        self.state.record_replay(now_ms, route, preview);
    }

    pub fn note_reconnect_reset(&mut self, now_ms: u64, reason: &str) {
        self.state.reset_count = self.state.reset_count.saturating_add(1);
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
                    "runtime_custom_packet_replay_bridge_summary: encoding={} key={:?} transport={} replay_count={} active={} last={:?}",
                    encoding_label(route.encoding),
                    route.key,
                    transport_label(route.transport),
                    state.replay_count,
                    state.active,
                    state.last_preview
                )
            })
            .collect::<Vec<_>>();
        lines.push(format!(
            "runtime_custom_packet_replay_bridge_state: routes={} active_routes={} resets={}",
            self.state.routes.len(),
            self.state.routes.values().filter(|route| route.active).count(),
            self.state.reset_count
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
                let preview = state.last_preview.as_ref()?;
                state.active.then_some((
                    state.last_update_serial,
                    format!(
                        "{}:{}({})#{}={}",
                        encoding_label(route.encoding),
                        route.key,
                        transport_label(route.transport),
                        state.replay_count,
                        preview
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

impl RuntimeCustomPacketReplayBridgeState {
    fn record_replay(
        &mut self,
        now_ms: u64,
        route: RuntimeCustomPacketReplayRouteKey,
        preview: String,
    ) {
        let state = self.routes.entry(route.clone()).or_default();
        state.replay_count = state.replay_count.saturating_add(1);
        state.active = true;
        state.last_preview = Some(preview.clone());
        self.next_update_serial = self.next_update_serial.saturating_add(1);
        state.last_update_serial = self.next_update_serial;
        self.pending_lines.push_back(format!(
            "runtime_custom_packet_replay_bridge_action: tick={now_ms}ms encoding={} key={:?} transport={} replay_count={} preview={preview:?}",
            encoding_label(route.encoding),
            route.key,
            transport_label(route.transport),
            state.replay_count,
        ));
    }

    fn clear_active_routes(&mut self, now_ms: u64, reason: &str) {
        let mut cleared_routes = 0usize;
        for route in self.routes.values_mut() {
            if route.active {
                cleared_routes = cleared_routes.saturating_add(1);
            }
            route.active = false;
            route.last_preview = None;
            route.last_update_serial = 0;
        }
        self.pending_lines.push_back(format!(
            "runtime_custom_packet_replay_bridge_reset: tick={now_ms}ms reason={reason} cleared_routes={cleared_routes}"
        ));
    }
}

fn route_key_from_spec(spec: &RuntimeCustomPacketRelaySpec) -> RuntimeCustomPacketReplayRouteKey {
    match spec {
        RuntimeCustomPacketRelaySpec::Text {
            outbound_type,
            transport,
            ..
        } => RuntimeCustomPacketReplayRouteKey {
            encoding: RuntimeCustomPacketRelayEncoding::Text,
            key: outbound_type.clone(),
            transport: RuntimeCustomPacketReplayTransport::Packet(*transport),
        },
        RuntimeCustomPacketRelaySpec::Binary {
            outbound_type,
            transport,
            ..
        } => RuntimeCustomPacketReplayRouteKey {
            encoding: RuntimeCustomPacketRelayEncoding::Binary,
            key: outbound_type.clone(),
            transport: RuntimeCustomPacketReplayTransport::Packet(*transport),
        },
        RuntimeCustomPacketRelaySpec::LogicData {
            outbound_channel,
            transport,
            ..
        } => RuntimeCustomPacketReplayRouteKey {
            encoding: RuntimeCustomPacketRelayEncoding::LogicData,
            key: outbound_channel.clone(),
            transport: RuntimeCustomPacketReplayTransport::Logic(*transport),
        },
    }
}

fn route_key_and_preview_from_action(
    action: &RuntimeCustomPacketRelayAction,
) -> (RuntimeCustomPacketReplayRouteKey, String) {
    match action {
        RuntimeCustomPacketRelayAction::Text {
            packet_type,
            contents,
            transport,
        } => (
            RuntimeCustomPacketReplayRouteKey {
                encoding: RuntimeCustomPacketRelayEncoding::Text,
                key: packet_type.clone(),
                transport: RuntimeCustomPacketReplayTransport::Packet(*transport),
            },
            truncate_for_preview(&contents.escape_default().to_string(), 96),
        ),
        RuntimeCustomPacketRelayAction::Binary {
            packet_type,
            contents,
            transport,
        } => {
            let prefix_len = contents.len().min(16);
            (
                RuntimeCustomPacketReplayRouteKey {
                    encoding: RuntimeCustomPacketRelayEncoding::Binary,
                    key: packet_type.clone(),
                    transport: RuntimeCustomPacketReplayTransport::Packet(*transport),
                },
                encode_hex_prefix(&contents[..prefix_len]),
            )
        }
        RuntimeCustomPacketRelayAction::LogicData {
            channel,
            value,
            transport,
        } => (
            RuntimeCustomPacketReplayRouteKey {
                encoding: RuntimeCustomPacketRelayEncoding::LogicData,
                key: channel.clone(),
                transport: RuntimeCustomPacketReplayTransport::Logic(*transport),
            },
            truncate_for_preview(&format!("{value:?}"), 96),
        ),
    }
}

fn encoding_label(encoding: RuntimeCustomPacketRelayEncoding) -> &'static str {
    match encoding {
        RuntimeCustomPacketRelayEncoding::Text => "text",
        RuntimeCustomPacketRelayEncoding::Binary => "binary",
        RuntimeCustomPacketRelayEncoding::LogicData => "logic",
    }
}

fn transport_label(transport: RuntimeCustomPacketReplayTransport) -> &'static str {
    match transport {
        RuntimeCustomPacketReplayTransport::Packet(ClientPacketTransport::Tcp) => "tcp",
        RuntimeCustomPacketReplayTransport::Packet(ClientPacketTransport::Udp) => "udp",
        RuntimeCustomPacketReplayTransport::Logic(ClientLogicDataTransport::Reliable) => {
            "reliable"
        }
        RuntimeCustomPacketReplayTransport::Logic(ClientLogicDataTransport::Unreliable) => {
            "unreliable"
        }
    }
}

fn encoding_rank(encoding: RuntimeCustomPacketRelayEncoding) -> u8 {
    match encoding {
        RuntimeCustomPacketRelayEncoding::Text => 0,
        RuntimeCustomPacketRelayEncoding::Binary => 1,
        RuntimeCustomPacketRelayEncoding::LogicData => 2,
    }
}

fn transport_rank(transport: RuntimeCustomPacketReplayTransport) -> u8 {
    match transport {
        RuntimeCustomPacketReplayTransport::Packet(ClientPacketTransport::Tcp) => 0,
        RuntimeCustomPacketReplayTransport::Packet(ClientPacketTransport::Udp) => 1,
        RuntimeCustomPacketReplayTransport::Logic(ClientLogicDataTransport::Reliable) => 2,
        RuntimeCustomPacketReplayTransport::Logic(ClientLogicDataTransport::Unreliable) => 3,
    }
}

fn truncate_for_preview(text: &str, max_chars: usize) -> String {
    let preview = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        format!("{preview}...")
    } else {
        preview
    }
}

fn encode_hex_prefix(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdt_typeio::TypeIoObject;

    #[test]
    fn replay_bridge_tracks_replays_and_business_summary() {
        let specs = vec![
            RuntimeCustomPacketRelaySpec::Text {
                inbound_type: "custom.ping".to_string(),
                outbound_type: "custom.pong".to_string(),
                transport: ClientPacketTransport::Udp,
            },
            RuntimeCustomPacketRelaySpec::LogicData {
                inbound_channel: "logic.in".to_string(),
                outbound_channel: "logic.out".to_string(),
                transport: ClientLogicDataTransport::Reliable,
            },
        ];
        let mut bridge = RuntimeCustomPacketReplayBridge::from_specs(&specs).unwrap();
        bridge.observe_action(
            42,
            &RuntimeCustomPacketRelayAction::Text {
                packet_type: "custom.pong".to_string(),
                contents: "relay ready".to_string(),
                transport: ClientPacketTransport::Udp,
            },
        );
        bridge.observe_action(
            43,
            &RuntimeCustomPacketRelayAction::LogicData {
                channel: "logic.out".to_string(),
                value: TypeIoObject::Int(7),
                transport: ClientLogicDataTransport::Reliable,
            },
        );

        let lines = bridge.drain_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("encoding=text"));
        assert!(lines[0].contains("key=\"custom.pong\""));
        assert!(lines[1].contains("encoding=logic"));
        assert_eq!(
            bridge.business_summary_text(4),
            Some(
                "logic:logic.out(reliable)#1=Int(7) | text:custom.pong(udp)#1=relay ready"
                    .to_string()
            )
        );
    }

    #[test]
    fn replay_bridge_clears_active_routes_on_reset() {
        let specs = vec![RuntimeCustomPacketRelaySpec::Binary {
            inbound_type: "custom.in".to_string(),
            outbound_type: "custom.out".to_string(),
            transport: ClientPacketTransport::Tcp,
        }];
        let mut bridge = RuntimeCustomPacketReplayBridge::from_specs(&specs).unwrap();
        bridge.observe_action(
            10,
            &RuntimeCustomPacketRelayAction::Binary {
                packet_type: "custom.out".to_string(),
                contents: vec![0xAA, 0xBB],
                transport: ClientPacketTransport::Tcp,
            },
        );
        bridge.note_reconnect_reset(11, "redirect");

        let lines = bridge.drain_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("runtime_custom_packet_replay_bridge_reset:"));
        assert!(lines[1].contains("reason=redirect"));
        assert_eq!(bridge.business_summary_text(4), None);
        assert!(bridge.summary_lines()[1].contains("resets=1"));
    }
}
