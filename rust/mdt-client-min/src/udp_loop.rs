use crate::arcnet_loop::transport_timeout_kind;
use crate::bootstrap_flow::ConnectPacketEnvelope;
use crate::client_session::{
    ClientPacketTransport, ClientSession, ClientSessionAction, ClientSessionError,
    ClientSessionEvent,
};
use crate::session_state::{ReconnectReasonKind, SessionTimeoutKind};
use mdt_protocol::{
    decode_framework_message, encode_framework_message, FrameworkCodecError, FrameworkMessage,
};
use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};

#[derive(Debug)]
pub struct UdpSessionDriver {
    socket: UdpSocket,
    server_addr: SocketAddr,
}

impl UdpSessionDriver {
    pub fn new(socket: UdpSocket, server_addr: SocketAddr) -> Result<Self, UdpLoopError> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            server_addr,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, UdpLoopError> {
        Ok(self.socket.local_addr()?)
    }

    pub fn send_connect(&self, connect: &ConnectPacketEnvelope) -> Result<usize, UdpLoopError> {
        Ok(self
            .socket
            .send_to(&connect.encoded_packet, self.server_addr)?)
    }

    pub fn tick(
        &self,
        session: &mut ClientSession,
        now_ms: u64,
        max_recv_packets: usize,
    ) -> Result<UdpTickReport, UdpLoopError> {
        let mut report = UdpTickReport::default();
        session.set_clock_ms(now_ms);

        let mut inbound = [0u8; 65_536];
        for _ in 0..max_recv_packets {
            match self.socket.recv_from(&mut inbound) {
                Ok((len, from)) => {
                    if from != self.server_addr {
                        continue;
                    }

                    let packet = &inbound[..len];
                    if let Ok(message) = decode_framework_message(packet) {
                        report.inbound_framework_messages += 1;
                        self.handle_framework_message(message, &mut report)?;
                        continue;
                    }

                    let event = session.ingest_packet_bytes(packet)?;
                    if matches!(event, ClientSessionEvent::WorldStreamReady { .. }) {
                        report.events.extend(session.take_replayed_loading_events());
                    }
                    report.inbound_packets += 1;
                    report.events.push(event);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(UdpLoopError::Io(error)),
            }
        }

        let actions = session.advance_time_for_transport_scope(now_ms, false, true)?;

        for action in actions {
            match action {
                ClientSessionAction::SendPacket {
                    transport, bytes, ..
                } => match transport {
                    ClientPacketTransport::Udp => {
                        self.socket.send_to(&bytes, self.server_addr)?;
                        report.outbound_packets += 1;
                    }
                    ClientPacketTransport::Tcp => {
                        return Err(UdpLoopError::UnsupportedTransport(
                            ClientPacketTransport::Tcp,
                        ));
                    }
                },
                ClientSessionAction::SendFramework { bytes, .. } => {
                    self.socket.send_to(&bytes, self.server_addr)?;
                    report.outbound_framework_messages += 1;
                }
                ClientSessionAction::TimedOut { idle_ms } => {
                    report.timed_out = Some(idle_ms);
                    report.timed_out_reason = Some(ReconnectReasonKind::Timeout);
                    report.timed_out_kind = transport_timeout_kind(session);
                }
            }
        }

        Ok(report)
    }

    fn handle_framework_message(
        &self,
        message: FrameworkMessage,
        report: &mut UdpTickReport,
    ) -> Result<(), UdpLoopError> {
        match message {
            FrameworkMessage::RegisterUdp { .. } => {}
            FrameworkMessage::Ping {
                id,
                is_reply: false,
            } => {
                let reply = encode_framework_message(&FrameworkMessage::Ping {
                    id,
                    is_reply: true,
                });
                self.socket.send_to(&reply, self.server_addr)?;
                report.outbound_framework_messages += 1;
            }
            FrameworkMessage::DiscoverHost
            | FrameworkMessage::KeepAlive
            | FrameworkMessage::RegisterTcp { .. }
            | FrameworkMessage::Ping { .. } => {}
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct UdpTickReport {
    pub outbound_packets: usize,
    pub outbound_framework_messages: usize,
    pub inbound_packets: usize,
    pub inbound_framework_messages: usize,
    pub timed_out: Option<u64>,
    pub timed_out_reason: Option<ReconnectReasonKind>,
    pub timed_out_kind: Option<SessionTimeoutKind>,
    pub events: Vec<ClientSessionEvent>,
}

#[derive(Debug)]
pub enum UdpLoopError {
    Io(io::Error),
    Session(ClientSessionError),
    Framework(FrameworkCodecError),
    UnsupportedTransport(ClientPacketTransport),
}

impl fmt::Display for UdpLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "udp io error: {error}"),
            Self::Session(error) => write!(f, "client session error: {error}"),
            Self::Framework(error) => write!(f, "framework message decode error: {error}"),
            Self::UnsupportedTransport(transport) => {
                write!(f, "unsupported udp loop transport: {transport:?}")
            }
        }
    }
}

impl std::error::Error for UdpLoopError {}

impl From<io::Error> for UdpLoopError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ClientSessionError> for UdpLoopError {
    fn from(value: ClientSessionError) -> Self {
        Self::Session(value)
    }
}

impl From<FrameworkCodecError> for UdpLoopError {
    fn from(value: FrameworkCodecError) -> Self {
        Self::Framework(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap_flow::encode_world_stream_packets;
    use crate::client_session::ClientSessionTiming;
    use mdt_protocol::{
        decode_framework_message, decode_packet, encode_framework_message, encode_packet,
        FrameworkMessage,
    };
    use mdt_remote::read_remote_manifest;
    use std::net::UdpSocket;
    use std::path::PathBuf;

    fn decode_hex_text(text: &str) -> Vec<u8> {
        let cleaned = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
            .collect()
    }

    fn sample_connect_payload() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../tests/src/test/resources/connect-packet.hex"
        ))
    }

    fn sample_world_stream_bytes() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../tests/src/test/resources/world-stream.hex"
        ))
    }

    fn encode_typeio_string_payload(text: &str) -> Vec<u8> {
        let bytes = text.as_bytes();
        let mut payload = Vec::with_capacity(1 + 2 + bytes.len());
        payload.push(1);
        payload.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        payload.extend_from_slice(bytes);
        payload
    }

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn world_stream_ready_over_udp_surfaces_pending_tcp_connect_confirm() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 120_000,
            timeout_ms: 120_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let driver = UdpSessionDriver::new(client, server_addr).unwrap();
        let client_addr = driver.local_addr().unwrap();

        driver.send_connect(&connect).unwrap();
        let mut recv = [0u8; 4096];
        let (len, from) = server.recv_from(&mut recv).unwrap();
        assert_eq!(from, client_addr);
        let connect_packet = decode_packet(&recv[..len]).unwrap();
        assert_eq!(connect_packet.packet_id, 3);

        server.send_to(&begin_packet, client_addr).unwrap();
        for chunk in &chunk_packets {
            server.send_to(chunk, client_addr).unwrap();
        }

        let report = driver.tick(&mut session, 1, 32).unwrap();
        assert!(report.inbound_packets >= 2);
        assert_eq!(report.outbound_packets, 0);
        assert_eq!(report.outbound_framework_messages, 0);
        assert!(session.state().world_stream_loaded);
        assert_eq!(session.state().world_map_width, 8);
        assert_eq!(session.state().world_map_height, 8);
        assert!(session.state().connect_confirm_sent);
        assert!(!session.state().connect_confirm_flushed);
    }

    #[test]
    fn world_stream_ready_over_udp_replays_loading_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 120_000,
            timeout_ms: 120_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let queued_message = encode_packet(
            packet_id,
            &encode_typeio_string_payload("[accent]queued over udp"),
            false,
        )
        .unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let driver = UdpSessionDriver::new(client, server_addr).unwrap();
        let client_addr = driver.local_addr().unwrap();

        server.send_to(&begin_packet, client_addr).unwrap();
        server.send_to(&queued_message, client_addr).unwrap();
        for chunk in &chunk_packets {
            server.send_to(chunk, client_addr).unwrap();
        }

        let report = driver.tick(&mut session, 1, 32).unwrap();
        assert!(report
            .events
            .iter()
            .any(|event| matches!(event, ClientSessionEvent::ServerMessage { message } if message == "[accent]queued over udp")));
        assert!(report
            .events
            .iter()
            .any(|event| matches!(event, ClientSessionEvent::WorldStreamReady { .. })));
    }

    #[test]
    fn processes_inbound_before_timeout_check() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 1_000,
            timeout_ms: 1_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, _) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.set_clock_ms(0);
        session.ingest_packet_bytes(&begin_packet).unwrap();

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr().unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let driver = UdpSessionDriver::new(client, server_addr).unwrap();
        let client_addr = driver.local_addr().unwrap();

        server.send_to(&begin_packet, client_addr).unwrap();
        let report = driver.tick(&mut session, 1_201, 32).unwrap();

        assert!(report.timed_out.is_none());
        assert!(report.timed_out_kind.is_none());
        assert_eq!(report.inbound_packets, 1);
        assert!(!session.state().connection_timed_out);
    }

    #[test]
    fn surfaces_timeout_kind_on_udp_timeout() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 1_200,
            timeout_ms: 1_200,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        assert!(session.state().ready_to_enter_world);
        assert!(session.state().client_loaded);
        assert!(session.state().connect_confirm_sent);
        assert!(!session.state().connect_confirm_flushed);

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr().unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let driver = UdpSessionDriver::new(client, server_addr).unwrap();

        let report = driver.tick(&mut session, 2_400, 32).unwrap();

        assert_eq!(report.timed_out, Some(2_400));
        assert_eq!(report.timed_out_reason, Some(ReconnectReasonKind::Timeout));
        assert_eq!(
            report.timed_out_kind,
            Some(SessionTimeoutKind::ConnectOrLoading)
        );
    }

    #[test]
    fn handles_udp_framework_register_and_ping() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 120_000,
            timeout_ms: 120_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let driver = UdpSessionDriver::new(client, server_addr).unwrap();
        let client_addr = driver.local_addr().unwrap();

        server
            .send_to(
                &encode_framework_message(&FrameworkMessage::RegisterUdp {
                    connection_id: 77,
                }),
                client_addr,
            )
            .unwrap();
        server
            .send_to(
                &encode_framework_message(&FrameworkMessage::Ping {
                    id: 123,
                    is_reply: false,
                }),
                client_addr,
            )
            .unwrap();

        let report = driver.tick(&mut session, 1, 32).unwrap();
        assert_eq!(report.inbound_framework_messages, 2);
        assert_eq!(report.outbound_framework_messages, 1);
        assert_eq!(report.inbound_packets, 0);

        let mut recv = [0u8; 1024];
        let (len, from) = server.recv_from(&mut recv).unwrap();
        assert_eq!(from, client_addr);
        assert_eq!(
            decode_framework_message(&recv[..len]).unwrap(),
            FrameworkMessage::Ping {
                id: 123,
                is_reply: true,
            }
        );
    }

    #[test]
    fn ignores_udp_framework_ping_reply_without_echoing() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 120_000,
            timeout_ms: 120_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let driver = UdpSessionDriver::new(client, server_addr).unwrap();
        let client_addr = driver.local_addr().unwrap();

        server
            .send_to(
                &encode_framework_message(&FrameworkMessage::Ping {
                    id: 123,
                    is_reply: true,
                }),
                client_addr,
            )
            .unwrap();

        let report = driver.tick(&mut session, 1, 32).unwrap();
        assert_eq!(report.inbound_framework_messages, 1);
        assert_eq!(report.outbound_framework_messages, 0);
        assert_eq!(report.inbound_packets, 0);

        let mut recv = [0u8; 1024];
        assert_eq!(
            server.recv_from(&mut recv).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
