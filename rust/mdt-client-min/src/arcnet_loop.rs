use crate::bootstrap_flow::ConnectPacketEnvelope;
use crate::client_session::{
    ClientPacketTransport, ClientSession, ClientSessionAction, ClientSessionError,
    ClientSessionEvent,
};
use crate::session_state::{ReconnectReasonKind, SessionTimeoutKind};
use mdt_protocol::{
    decode_framework_message, encode_framework_message, FrameworkCodecError, FrameworkMessage,
};
use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

pub(crate) fn transport_timeout_kind(session: &ClientSession) -> Option<SessionTimeoutKind> {
    session
        .state()
        .last_timeout
        .map(|projection| projection.kind)
        .or(Some(SessionTimeoutKind::ConnectOrLoading))
}

// One length-prefixed TCP frame: 2-byte header plus the largest u16 payload.
const MAX_TCP_READ_BUFFER_BYTES: usize = u16::MAX as usize + 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingTcpFrame {
    packet_id: Option<u8>,
    remaining_bytes: usize,
}

#[derive(Debug, Default)]
struct PendingTcpWrite {
    bytes: Vec<u8>,
    offset: usize,
    frames: VecDeque<PendingTcpFrame>,
}

impl PendingTcpWrite {
    fn enqueue_frame(
        &mut self,
        payload: &[u8],
        packet_id: Option<u8>,
    ) -> Result<(), ArcNetLoopError> {
        let len = u16::try_from(payload.len())
            .map_err(|_| ArcNetLoopError::FrameTooLarge(payload.len()))?;
        let frame_len = usize::from(len).saturating_add(2);
        self.bytes.extend_from_slice(&len.to_be_bytes());
        self.bytes.extend_from_slice(payload);
        self.frames.push_back(PendingTcpFrame {
            packet_id,
            remaining_bytes: frame_len,
        });
        Ok(())
    }

    fn flush<W: Write>(&mut self, writer: &mut W) -> Result<Vec<u8>, ArcNetLoopError> {
        let mut completed_packet_ids = Vec::new();
        while self.offset < self.bytes.len() {
            match writer.write(&self.bytes[self.offset..]) {
                Ok(0) => return Err(ArcNetLoopError::TcpClosed),
                Ok(written) => {
                    self.offset += written;
                    self.record_flushed_bytes(written, &mut completed_packet_ids);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(completed_packet_ids);
                }
                Err(error) => return Err(ArcNetLoopError::Io(error)),
            }
        }

        if self.offset > 0 {
            self.bytes.clear();
            self.offset = 0;
        }
        debug_assert!(self.frames.is_empty());
        Ok(completed_packet_ids)
    }

    fn record_flushed_bytes(&mut self, mut written: usize, completed_packet_ids: &mut Vec<u8>) {
        while written > 0 {
            let Some(frame) = self.frames.front_mut() else {
                debug_assert_eq!(written, 0);
                return;
            };
            let consumed = frame.remaining_bytes.min(written);
            frame.remaining_bytes -= consumed;
            written -= consumed;
            if frame.remaining_bytes == 0 {
                let frame = self.frames.pop_front().expect("pending TCP frame metadata");
                if let Some(packet_id) = frame.packet_id {
                    completed_packet_ids.push(packet_id);
                }
            }
        }
    }

    fn clear(&mut self) {
        self.bytes.clear();
        self.offset = 0;
        self.frames.clear();
    }
}

#[derive(Debug)]
pub struct ArcNetSessionDriver {
    tcp: TcpStream,
    udp: UdpSocket,
    tcp_read_buffer: Vec<u8>,
    pending_tcp_write: PendingTcpWrite,
    completed_tcp_packet_ids: VecDeque<u8>,
    connection_id: Option<i32>,
    udp_registered: bool,
    connect_sent: bool,
    pending_connect: Option<Vec<u8>>,
}

impl ArcNetSessionDriver {
    pub fn discover_first_server(
        probe_targets: &[SocketAddr],
        probe_timeout: Duration,
    ) -> Result<Option<SocketAddr>, ArcNetLoopError> {
        let discover_payload = encode_framework_message(&FrameworkMessage::DiscoverHost);
        for target in probe_targets {
            if let Some(found) =
                Self::probe_discover_target(*target, probe_timeout, &discover_payload)?
            {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn probe_discover_target(
        target: SocketAddr,
        probe_timeout: Duration,
        discover_payload: &[u8],
    ) -> Result<Option<SocketAddr>, ArcNetLoopError> {
        let bind_addr = if target.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(bind_addr)?;
        if target.is_ipv4() {
            socket.set_broadcast(true)?;
        }
        socket.set_read_timeout(Some(probe_timeout))?;
        socket.send_to(discover_payload, target)?;

        let mut response = [0u8; 65_536];
        loop {
            match socket.recv_from(&mut response) {
                Ok((len, responder)) => {
                    let Ok(message) = decode_framework_message(&response[..len]) else {
                        continue;
                    };
                    if Self::is_valid_discovery_reply(&message) {
                        return Ok(Some(responder));
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(ArcNetLoopError::Io(error)),
            }
        }
    }

    fn is_valid_discovery_reply(message: &FrameworkMessage) -> bool {
        matches!(
            message,
            FrameworkMessage::DiscoverHost | FrameworkMessage::Ping { is_reply: true, .. }
        )
    }

    pub fn connect(server_addr: SocketAddr) -> Result<Self, ArcNetLoopError> {
        let tcp = TcpStream::connect(server_addr)?;
        tcp.set_nodelay(true)?;
        tcp.set_nonblocking(true)?;

        let bind_addr = if server_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let udp = UdpSocket::bind(bind_addr)?;
        udp.connect(server_addr)?;
        udp.set_nonblocking(true)?;

        Ok(Self {
            tcp,
            udp,
            tcp_read_buffer: Vec::new(),
            pending_tcp_write: PendingTcpWrite::default(),
            completed_tcp_packet_ids: VecDeque::new(),
            connection_id: None,
            udp_registered: false,
            connect_sent: false,
            pending_connect: None,
        })
    }

    pub fn reconnect(
        &mut self,
        server_addr: SocketAddr,
        connect: &ConnectPacketEnvelope,
    ) -> Result<(), ArcNetLoopError> {
        self.quiet_reset_transport_state();
        let mut replacement = Self::connect(server_addr)?;
        replacement.send_connect(connect)?;
        *self = replacement;
        Ok(())
    }

    pub fn tcp_local_addr(&self) -> Result<SocketAddr, ArcNetLoopError> {
        Ok(self.tcp.local_addr()?)
    }

    pub fn udp_local_addr(&self) -> Result<SocketAddr, ArcNetLoopError> {
        Ok(self.udp.local_addr()?)
    }

    pub fn send_connect(&mut self, connect: &ConnectPacketEnvelope) -> Result<(), ArcNetLoopError> {
        self.connect_sent = false;
        self.pending_connect = Some(connect.encoded_packet.clone());
        Ok(())
    }

    pub fn tick(
        &mut self,
        session: &mut ClientSession,
        now_ms: u64,
        max_tcp_frames: usize,
        max_udp_packets: usize,
    ) -> Result<ArcNetTickReport, ArcNetLoopError> {
        self.tick_with_post_ingest_hook(session, now_ms, max_tcp_frames, max_udp_packets, |_| {})
    }

    pub fn tick_with_post_ingest_hook<F>(
        &mut self,
        session: &mut ClientSession,
        now_ms: u64,
        max_tcp_frames: usize,
        max_udp_packets: usize,
        mut post_ingest: F,
    ) -> Result<ArcNetTickReport, ArcNetLoopError>
    where
        F: FnMut(&mut ClientSession),
    {
        session.set_clock_ms(now_ms);

        let mut report = ArcNetTickReport::default();
        self.fill_tcp_read_buffer()?;
        self.drain_tcp_frames(session, max_tcp_frames, &mut report)?;
        self.recv_udp_packets(session, max_udp_packets, &mut report)?;
        post_ingest(session);
        self.flush_pending_tcp_payload()?;
        self.drain_completed_tcp_packets(session, now_ms);

        if self.udp_registered && !self.connect_sent {
            if let Some(connect) = self.pending_connect.take() {
                self.send_tcp_payload(&connect)?;
                self.connect_sent = true;
                report.connect_sent = true;
                report.outbound_tcp_frames += 1;
            }
        }

        if self.connect_sent {
            for action in session.advance_time(now_ms)? {
                match action {
                    ClientSessionAction::SendPacket {
                        packet_id,
                        transport,
                        bytes,
                    } => match transport {
                        ClientPacketTransport::Tcp => {
                            self.send_tcp_packet_payload(packet_id, &bytes)?;
                            self.drain_completed_tcp_packets(session, now_ms);
                            report.outbound_tcp_frames += 1;
                        }
                        ClientPacketTransport::Udp => {
                            self.udp.send(&bytes)?;
                            report.outbound_udp_packets += 1;
                        }
                    },
                    ClientSessionAction::SendFramework { bytes, .. } => {
                        self.send_tcp_payload(&bytes)?;
                        self.drain_completed_tcp_packets(session, now_ms);
                        report.outbound_tcp_frames += 1;
                        report.outbound_framework_messages += 1;
                    }
                    ClientSessionAction::TimedOut { idle_ms } => {
                        report.timed_out = Some(idle_ms);
                        report.timed_out_reason = Some(ReconnectReasonKind::Timeout);
                        report.timed_out_kind = transport_timeout_kind(session);
                    }
                }
            }
        }

        report.udp_registered = self.udp_registered;
        Ok(report)
    }

    fn fill_tcp_read_buffer(&mut self) -> Result<(), ArcNetLoopError> {
        let mut chunk = [0u8; 4096];
        loop {
            if self.tcp_read_buffer.len() >= MAX_TCP_READ_BUFFER_BYTES {
                break;
            }
            let read_len =
                (MAX_TCP_READ_BUFFER_BYTES - self.tcp_read_buffer.len()).min(chunk.len());
            match self.tcp.read(&mut chunk[..read_len]) {
                Ok(0) => {
                    self.tcp_read_buffer.clear();
                    return Err(ArcNetLoopError::TcpClosed);
                }
                Ok(len) => self.tcp_read_buffer.extend_from_slice(&chunk[..len]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(ArcNetLoopError::Io(error)),
            }
        }
        Ok(())
    }

    fn drain_tcp_frames(
        &mut self,
        session: &mut ClientSession,
        max_tcp_frames: usize,
        report: &mut ArcNetTickReport,
    ) -> Result<(), ArcNetLoopError> {
        for _ in 0..max_tcp_frames {
            let Some(frame) = self.try_take_tcp_frame()? else {
                break;
            };
            self.handle_inbound_payload(session, &frame, true, report)?;
        }
        Ok(())
    }

    fn recv_udp_packets(
        &mut self,
        session: &mut ClientSession,
        max_udp_packets: usize,
        report: &mut ArcNetTickReport,
    ) -> Result<(), ArcNetLoopError> {
        let mut packet = [0u8; 65_536];
        for _ in 0..max_udp_packets {
            match self.udp.recv(&mut packet) {
                Ok(len) => self.handle_inbound_payload(session, &packet[..len], false, report)?,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(ArcNetLoopError::Io(error)),
            }
        }
        Ok(())
    }

    fn handle_inbound_payload(
        &mut self,
        session: &mut ClientSession,
        payload: &[u8],
        from_tcp: bool,
        report: &mut ArcNetTickReport,
    ) -> Result<(), ArcNetLoopError> {
        if from_tcp {
            report.inbound_tcp_frames += 1;
        } else {
            report.inbound_udp_packets += 1;
        }

        if let Ok(message) = decode_framework_message(payload) {
            report.inbound_framework_messages += 1;
            self.handle_framework_message(message, from_tcp, report)?;
            return Ok(());
        }

        let event = session.ingest_packet_bytes(payload)?;
        if matches!(event, ClientSessionEvent::WorldStreamReady { .. }) {
            report.events.extend(session.take_replayed_loading_events());
        }
        report.events.push(event);
        Ok(())
    }

    fn handle_framework_message(
        &mut self,
        message: FrameworkMessage,
        from_tcp: bool,
        report: &mut ArcNetTickReport,
    ) -> Result<(), ArcNetLoopError> {
        match message {
            FrameworkMessage::RegisterUdp { .. } => {
                self.udp_registered = true;
            }
            FrameworkMessage::RegisterTcp { connection_id } => {
                self.connection_id = Some(connection_id);
                let register_udp = FrameworkMessage::RegisterUdp { connection_id };
                let bytes = encode_framework_message(&register_udp);
                self.udp.send(&bytes)?;
                report.outbound_udp_packets += 1;
                report.outbound_framework_messages += 1;
            }
            FrameworkMessage::Ping {
                id,
                is_reply: false,
            } => {
                report.inbound_ping_requests += 1;
                let reply = FrameworkMessage::Ping { id, is_reply: true };
                if from_tcp {
                    let bytes = encode_framework_message(&reply);
                    self.send_tcp_payload(&bytes)?;
                    report.outbound_tcp_frames += 1;
                } else {
                    let bytes = encode_framework_message(&reply);
                    self.udp.send(&bytes)?;
                    report.outbound_udp_packets += 1;
                }
                report.outbound_framework_messages += 1;
            }
            FrameworkMessage::Ping { is_reply: true, .. } => {
                if from_tcp {
                    report.inbound_tcp_ping_replies += 1;
                } else {
                    report.inbound_udp_ping_replies += 1;
                }
            }
            FrameworkMessage::KeepAlive | FrameworkMessage::DiscoverHost => {}
        }
        Ok(())
    }

    fn try_take_tcp_frame(&mut self) -> Result<Option<Vec<u8>>, ArcNetLoopError> {
        if self.tcp_read_buffer.len() < 2 {
            return Ok(None);
        }

        let payload_len =
            u16::from_be_bytes([self.tcp_read_buffer[0], self.tcp_read_buffer[1]]) as usize;
        let frame_len = payload_len.saturating_add(2);
        if self.tcp_read_buffer.len() < frame_len {
            return Ok(None);
        }

        let payload = self.tcp_read_buffer[2..frame_len].to_vec();
        self.tcp_read_buffer.drain(..frame_len);
        Ok(Some(payload))
    }

    fn flush_pending_tcp_payload(&mut self) -> Result<(), ArcNetLoopError> {
        let completed_packet_ids = self.pending_tcp_write.flush(&mut self.tcp)?;
        self.completed_tcp_packet_ids.extend(completed_packet_ids);
        Ok(())
    }

    fn send_tcp_payload(&mut self, payload: &[u8]) -> Result<(), ArcNetLoopError> {
        self.pending_tcp_write.enqueue_frame(payload, None)?;
        self.flush_pending_tcp_payload()
    }

    fn send_tcp_packet_payload(
        &mut self,
        packet_id: u8,
        payload: &[u8],
    ) -> Result<(), ArcNetLoopError> {
        self.pending_tcp_write
            .enqueue_frame(payload, Some(packet_id))?;
        self.flush_pending_tcp_payload()
    }

    fn drain_completed_tcp_packets(&mut self, session: &mut ClientSession, now_ms: u64) {
        while let Some(packet_id) = self.completed_tcp_packet_ids.pop_front() {
            session.mark_tcp_packet_flushed(packet_id, now_ms);
        }
    }

    fn quiet_reset_transport_state(&mut self) {
        self.tcp_read_buffer.clear();
        self.pending_tcp_write.clear();
        self.completed_tcp_packet_ids.clear();
        self.connection_id = None;
        self.udp_registered = false;
        self.connect_sent = false;
        self.pending_connect = None;
    }
}

#[derive(Debug, Default)]
pub struct ArcNetTickReport {
    pub outbound_tcp_frames: usize,
    pub outbound_udp_packets: usize,
    pub inbound_tcp_frames: usize,
    pub inbound_udp_packets: usize,
    pub inbound_framework_messages: usize,
    pub outbound_framework_messages: usize,
    pub inbound_ping_requests: usize,
    pub inbound_tcp_ping_replies: usize,
    pub inbound_udp_ping_replies: usize,
    pub udp_registered: bool,
    pub connect_sent: bool,
    pub timed_out: Option<u64>,
    pub timed_out_reason: Option<ReconnectReasonKind>,
    pub timed_out_kind: Option<SessionTimeoutKind>,
    pub events: Vec<ClientSessionEvent>,
}

#[derive(Debug)]
pub enum ArcNetLoopError {
    Io(io::Error),
    Session(ClientSessionError),
    Framework(FrameworkCodecError),
    FrameTooLarge(usize),
    TcpClosed,
}

impl fmt::Display for ArcNetLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "arcnet io error: {error}"),
            Self::Session(error) => write!(f, "client session error: {error}"),
            Self::Framework(error) => write!(f, "framework decode error: {error}"),
            Self::FrameTooLarge(len) => write!(f, "tcp frame too large for arcnet: {len} bytes"),
            Self::TcpClosed => write!(f, "tcp connection closed"),
        }
    }
}

impl std::error::Error for ArcNetLoopError {}

impl From<io::Error> for ArcNetLoopError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ClientSessionError> for ArcNetLoopError {
    fn from(value: ClientSessionError) -> Self {
        Self::Session(value)
    }
}

impl From<FrameworkCodecError> for ArcNetLoopError {
    fn from(value: FrameworkCodecError) -> Self {
        Self::Framework(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap_flow::{encode_world_stream_packets, LoginBootstrap};
    use crate::client_session::{ClientSession, ClientSessionTiming};
    use mdt_protocol::{decode_packet, encode_packet, CONNECT_PACKET_ID};
    use mdt_remote::{read_remote_manifest, HighFrequencyRemoteMethod};
    use std::io::{self, Write};
    use std::net::{TcpListener, UdpSocket};
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

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
        let mut payload = Vec::new();
        let bytes = text.as_bytes();
        payload.push(1);
        payload.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        payload.extend_from_slice(bytes);
        payload
    }

    fn sample_snapshot_packet(key: &str) -> Vec<u8> {
        let text = include_str!("../../../tests/src/test/resources/snapshot-goldens.txt");
        let hex = text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("missing snapshot golden key: {key}"));
        decode_hex_text(hex)
    }

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    const LOCAL_ARCNET_BIND_ATTEMPTS: usize = 512;

    fn bind_local_arcnet_server() -> (TcpListener, UdpSocket, SocketAddr) {
        for _ in 0..LOCAL_ARCNET_BIND_ATTEMPTS {
            let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let server_addr = tcp_listener.local_addr().unwrap();
            if let Ok(udp_socket) = UdpSocket::bind(server_addr) {
                return (tcp_listener, udp_socket, server_addr);
            }
            drop(tcp_listener);
            thread::sleep(Duration::from_millis(1));
        }
        panic!("failed to bind local TCP+UDP sockets on the same port");
    }

    fn write_tcp_frame(stream: &mut TcpStream, payload: &[u8]) {
        let len = u16::try_from(payload.len()).unwrap();
        stream.write_all(&len.to_be_bytes()).unwrap();
        stream.write_all(payload).unwrap();
    }

    fn read_tcp_frame(stream: &mut TcpStream) -> Vec<u8> {
        let mut len = [0u8; 2];
        stream.read_exact(&mut len).unwrap();
        let payload_len = u16::from_be_bytes(len) as usize;
        let mut payload = vec![0u8; payload_len];
        stream.read_exact(&mut payload).unwrap();
        payload
    }

    fn sample_connect_envelope() -> ConnectPacketEnvelope {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap()
    }

    #[test]
    fn transport_timeout_kind_defaults_to_connect_or_loading_when_missing() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let session = ClientSession::from_remote_manifest_with_timing(
            &manifest,
            "fr",
            ClientSessionTiming::default(),
        )
        .unwrap();

        assert_eq!(
            transport_timeout_kind(&session),
            Some(SessionTimeoutKind::ConnectOrLoading)
        );
    }

    #[test]
    fn discover_first_server_returns_udp_responder_addr() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut packet = [0u8; 64];
            let (len, from) = server.recv_from(&mut packet).unwrap();
            assert_eq!(
                decode_framework_message(&packet[..len]).unwrap(),
                FrameworkMessage::DiscoverHost
            );
            server
                .send_to(
                    &encode_framework_message(&FrameworkMessage::Ping {
                        id: 7,
                        is_reply: true,
                    }),
                    from,
                )
                .unwrap();
        });

        let found =
            ArcNetSessionDriver::discover_first_server(&[server_addr], Duration::from_millis(250))
                .unwrap();

        assert_eq!(found, Some(server_addr));
        handle.join().unwrap();
    }

    #[test]
    fn discover_first_server_returns_none_on_timeout() {
        let _silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target = _silent.local_addr().unwrap();

        let found =
            ArcNetSessionDriver::discover_first_server(&[target], Duration::from_millis(50))
                .unwrap();

        assert_eq!(found, None);
    }

    #[test]
    fn discover_first_server_rejects_non_framework_replies() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let server_addr = server.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut packet = [0u8; 64];
            let (len, from) = server.recv_from(&mut packet).unwrap();
            assert_eq!(
                decode_framework_message(&packet[..len]).unwrap(),
                FrameworkMessage::DiscoverHost
            );
            server.send_to(b"plain-udp-noise", from).unwrap();
            thread::sleep(Duration::from_millis(25));
            server
                .send_to(
                    &encode_framework_message(&FrameworkMessage::Ping {
                        id: 99,
                        is_reply: true,
                    }),
                    from,
                )
                .unwrap();
        });

        let found =
            ArcNetSessionDriver::discover_first_server(&[server_addr], Duration::from_millis(250))
                .unwrap();

        assert_eq!(found, Some(server_addr));
        handle.join().unwrap();
    }

    #[test]
    fn discover_first_server_skips_silent_targets_and_returns_later_responder() {
        let _silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let silent_addr = _silent.local_addr().unwrap();
        let responder = UdpSocket::bind("127.0.0.1:0").unwrap();
        let responder_addr = responder.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut packet = [0u8; 64];
            let (len, from) = responder.recv_from(&mut packet).unwrap();
            assert_eq!(
                decode_framework_message(&packet[..len]).unwrap(),
                FrameworkMessage::DiscoverHost
            );
            responder
                .send_to(
                    &encode_framework_message(&FrameworkMessage::Ping {
                        id: 11,
                        is_reply: true,
                    }),
                    from,
                )
                .unwrap();
        });

        let found = ArcNetSessionDriver::discover_first_server(
            &[silent_addr, responder_addr],
            Duration::from_millis(100),
        )
        .unwrap();

        assert_eq!(found, Some(responder_addr));
        handle.join().unwrap();
    }

    #[test]
    fn send_connect_resets_connect_sent_gate() {
        let (tcp_listener, _udp_socket, server_addr) = bind_local_arcnet_server();
        let _accept = thread::spawn(move || {
            let _ = tcp_listener.accept();
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.connect_sent = true;
        let connect = sample_connect_envelope();
        driver.send_connect(&connect).unwrap();

        assert!(!driver.connect_sent);
        assert_eq!(
            driver.pending_connect.as_deref(),
            Some(connect.encoded_packet.as_slice())
        );
    }

    #[test]
    fn reconnect_failure_quiet_resets_transport_state() {
        let (tcp_listener, _udp_socket, server_addr) = bind_local_arcnet_server();
        let _accept = thread::spawn(move || {
            let _ = tcp_listener.accept();
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.connection_id = Some(777);
        driver.udp_registered = true;
        driver.connect_sent = true;
        driver.pending_connect = Some(vec![1, 2, 3]);
        driver.tcp_read_buffer.extend_from_slice(&[0, 3, 9, 9, 9]);
        driver
            .pending_tcp_write
            .enqueue_frame(&[0xAA, 0xBB], Some(77))
            .unwrap();
        driver.completed_tcp_packet_ids.push_back(77);

        let connect = sample_connect_envelope();
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let unreachable_addr = probe.local_addr().unwrap();
        drop(probe);

        let reconnect = driver.reconnect(unreachable_addr, &connect);
        assert!(reconnect.is_err());
        assert_eq!(driver.connection_id, None);
        assert!(!driver.udp_registered);
        assert!(!driver.connect_sent);
        assert!(driver.pending_connect.is_none());
        assert!(driver.tcp_read_buffer.is_empty());
        assert!(driver.pending_tcp_write.bytes.is_empty());
        assert_eq!(driver.pending_tcp_write.offset, 0);
        assert!(driver.pending_tcp_write.frames.is_empty());
        assert!(driver.completed_tcp_packet_ids.is_empty());
    }

    #[test]
    fn send_tcp_payload_handles_nonblocking_backpressure() {
        #[derive(Debug)]
        struct ScriptedWriter {
            max_bytes_per_write: usize,
            block_after_write: bool,
            blocked: bool,
            written: Vec<u8>,
        }

        impl Write for ScriptedWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if self.blocked {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }

                let written = buf.len().min(self.max_bytes_per_write);
                self.written.extend_from_slice(&buf[..written]);
                if self.block_after_write {
                    self.blocked = true;
                }
                Ok(written)
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut pending = PendingTcpWrite::default();
        let payload = vec![0x5a; 32];
        pending.enqueue_frame(&payload, Some(77)).unwrap();

        let mut writer = ScriptedWriter {
            max_bytes_per_write: 5,
            block_after_write: true,
            blocked: false,
            written: Vec::new(),
        };

        let completed_packet_ids = pending.flush(&mut writer).unwrap();
        assert!(!pending.bytes.is_empty());
        assert!(pending.offset > 0);
        assert_eq!(writer.written.len(), 5);
        assert!(completed_packet_ids.is_empty());

        writer.blocked = false;
        writer.block_after_write = false;
        writer.max_bytes_per_write = usize::MAX;

        let completed_packet_ids = pending.flush(&mut writer).unwrap();
        assert!(pending.bytes.is_empty());
        assert_eq!(pending.offset, 0);
        assert_eq!(writer.written.len(), payload.len() + 2);
        assert_eq!(&writer.written[..2], &(payload.len() as u16).to_be_bytes());
        assert_eq!(&writer.written[2..], payload.as_slice());
        assert_eq!(completed_packet_ids, vec![77]);
    }

    #[test]
    fn pending_tcp_write_tracks_completed_packet_ids_across_frame_boundaries() {
        let mut pending = PendingTcpWrite::default();
        pending.enqueue_frame(&[0xaa], Some(11)).unwrap();
        pending.enqueue_frame(&[0xbb, 0xcc], Some(22)).unwrap();

        let mut completed_packet_ids = Vec::new();
        pending.record_flushed_bytes(4, &mut completed_packet_ids);

        assert_eq!(completed_packet_ids, vec![11]);
        assert_eq!(pending.frames.len(), 1);
        assert_eq!(pending.frames.front().map(|frame| frame.remaining_bytes), Some(3));

        pending.record_flushed_bytes(3, &mut completed_packet_ids);

        assert_eq!(completed_packet_ids, vec![11, 22]);
        assert!(pending.frames.is_empty());
    }

    #[test]
    fn pending_tcp_write_rejects_frames_larger_than_u16_max() {
        let mut pending = PendingTcpWrite::default();
        let payload = vec![0u8; usize::from(u16::MAX) + 1];

        let error = pending
            .enqueue_frame(&payload, Some(77))
            .expect_err("oversized frame should fail");

        assert!(matches!(
            error,
            ArcNetLoopError::FrameTooLarge(len) if len == payload.len()
        ));
        assert!(pending.bytes.is_empty());
        assert_eq!(pending.offset, 0);
        assert!(pending.frames.is_empty());
    }

    #[test]
    fn test_cap_tcp_read_buffer_growth_on_fragmented_frames() {
        let (tcp_listener, _udp_socket, server_addr) = bind_local_arcnet_server();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            let mut frame = Vec::with_capacity(MAX_TCP_READ_BUFFER_BYTES + 4);
            frame.extend_from_slice(&u16::MAX.to_be_bytes());
            frame.extend(std::iter::repeat(0x5au8).take(u16::MAX as usize));
            frame.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]);

            for chunk in frame.chunks(512) {
                tcp_stream.write_all(chunk).unwrap();
            }
            ready_tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(100));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        driver.fill_tcp_read_buffer().unwrap();
        assert_eq!(driver.tcp_read_buffer.len(), MAX_TCP_READ_BUFFER_BYTES);
        server.join().unwrap();
    }

    #[test]
    fn tcp_closed_clears_partial_read_buffer() {
        let (tcp_listener, _udp_socket, server_addr) = bind_local_arcnet_server();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream.write_all(&[0, 3, 0xaa]).unwrap();
            ready_tx.send(()).unwrap();
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        server.join().unwrap();
        let error = driver.fill_tcp_read_buffer().unwrap_err();

        assert!(matches!(error, ArcNetLoopError::TcpClosed));
        assert!(driver.tcp_read_buffer.is_empty());
    }

    #[test]
    fn reconnect_success_sends_connect_over_new_transport() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();

        let (old_tcp_listener, _old_udp_socket, old_server_addr) = bind_local_arcnet_server();
        let _old_server = thread::spawn(move || {
            let _ = old_tcp_listener.accept();
            thread::sleep(Duration::from_millis(200));
        });

        let mut driver = ArcNetSessionDriver::connect(old_server_addr).unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 902 }),
            );
            ready_tx.send(()).unwrap();

            let mut udp_buf = [0u8; 1024];
            let (udp_len, client_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 902 }
            );

            udp_socket
                .send_to(
                    &encode_framework_message(&FrameworkMessage::RegisterUdp {
                        connection_id: 902,
                    }),
                    client_addr,
                )
                .unwrap();

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);
        });

        driver.reconnect(server_addr, &connect).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        thread::sleep(Duration::from_millis(25));
        assert_eq!(driver.connection_id, None);
        assert!(!driver.udp_registered);
        assert!(!driver.connect_sent);
        assert_eq!(
            driver.pending_connect.as_deref(),
            Some(connect.encoded_packet.as_slice())
        );

        let mut saw_connect_sent = false;
        for tick in 0..200u64 {
            match driver.tick(&mut session, tick * 100, 32, 32) {
                Ok(report) => {
                    if report.connect_sent {
                        saw_connect_sent = true;
                        break;
                    }
                }
                Err(ArcNetLoopError::Io(error))
                    if error.kind() == std::io::ErrorKind::ConnectionAborted =>
                {
                    eprintln!("tick {tick}: connection aborted");
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("tick {tick}: {error:?}"),
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_connect_sent);
        assert!(driver.udp_registered);
        assert!(driver.connect_sent);
        assert!(driver.pending_connect.is_none());
        server.join().unwrap();
    }

    #[test]
    fn completes_arcnet_register_and_world_stream_over_local_sockets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 10_000,
            client_snapshot_interval_ms: 10_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let _ = session.advance_time(0).unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 777 }),
            );

            let mut udp_buf = [0u8; 1024];
            let (udp_len, _) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 777 }
            );

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterUdp { connection_id: 0 }),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(&mut tcp_stream, &begin_packet);
            for chunk in &chunk_packets {
                write_tcp_frame(&mut tcp_stream, chunk);
            }

            thread::sleep(Duration::from_millis(100));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        for tick in 0..50u64 {
            let report = driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            if session.state().world_stream_loaded {
                assert!(report.udp_registered);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(session.state().world_stream_loaded);
        assert_eq!(session.state().world_map_width, 8);
        assert_eq!(session.state().world_map_height, 8);
        assert!(session.state().connect_confirm_sent);
        assert!(session.state().connect_confirm_flushed);
        server.join().unwrap();
    }

    #[test]
    fn tick_with_post_ingest_hook_observes_world_stream_before_connect_confirm_flush() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 10_000,
            client_snapshot_interval_ms: 10_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 777 }),
            );

            let mut udp_buf = [0u8; 1024];
            let (udp_len, _) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 777 }
            );

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterUdp { connection_id: 0 }),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(&mut tcp_stream, &begin_packet);
            for chunk in &chunk_packets {
                write_tcp_frame(&mut tcp_stream, chunk);
            }

            thread::sleep(Duration::from_millis(100));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();
        let mut observed_boundary = false;

        for tick in 0..50u64 {
            let report = driver
                .tick_with_post_ingest_hook(&mut session, tick * 100, 32, 32, |session| {
                    if session.state().world_stream_loaded {
                        observed_boundary = true;
                        assert!(session.state().connect_confirm_sent);
                        assert!(!session.state().connect_confirm_flushed);
                    }
                })
                .unwrap();
            if session.state().world_stream_loaded {
                assert!(report.udp_registered);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(observed_boundary);
        assert!(session.state().world_stream_loaded);
        assert!(session.state().connect_confirm_sent);
        assert!(session.state().connect_confirm_flushed);
        server.join().unwrap();
    }

    #[test]
    fn sends_client_snapshot_over_udp_after_world_ready() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 500,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let expected_connect_confirm_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connectConfirm")
            .unwrap()
            .packet_id;
        let expected_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientSnapshot")
            .unwrap()
            .packet_id;
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let login = LoginBootstrap::from_stream_packets(
            &sample_connect_payload(),
            &begin_packet,
            &chunk_packets,
            "fr",
        )
        .unwrap();
        let expected_unit_id = i32::try_from(login.bootstrap.player_unit_value).unwrap();
        let expected_x_bits = login.bootstrap.player_x_bits;
        let expected_y_bits = login.bootstrap.player_y_bits;

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 888 }),
            );

            let mut udp_buf = [0u8; 2048];
            let (udp_len, client_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 888 }
            );

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterUdp { connection_id: 0 }),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(&mut tcp_stream, &begin_packet);
            for chunk in &chunk_packets {
                write_tcp_frame(&mut tcp_stream, chunk);
            }

            let mut saw_connect_confirm = false;
            for _ in 0..4 {
                let frame = read_tcp_frame(&mut tcp_stream);
                if let Ok(packet) = decode_packet(&frame) {
                    if packet.packet_id == expected_connect_confirm_packet_id {
                        assert!(packet.payload.is_empty());
                        saw_connect_confirm = true;
                        break;
                    }
                } else {
                    assert_eq!(
                        decode_framework_message(&frame).unwrap(),
                        FrameworkMessage::KeepAlive
                    );
                }
            }
            assert!(saw_connect_confirm);

            let (snapshot_len, snapshot_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(snapshot_addr, client_addr);
            let snapshot_packet = decode_packet(&udp_buf[..snapshot_len]).unwrap();
            assert_eq!(snapshot_packet.packet_id, expected_snapshot_packet_id);
            assert_eq!(&snapshot_packet.payload[0..4], &1i32.to_be_bytes());
            assert_eq!(
                &snapshot_packet.payload[4..8],
                &expected_unit_id.to_be_bytes()
            );
            assert_eq!(snapshot_packet.payload[8], 0);
            assert_eq!(
                &snapshot_packet.payload[9..13],
                &expected_x_bits.to_be_bytes()
            );
            assert_eq!(
                &snapshot_packet.payload[13..17],
                &expected_y_bits.to_be_bytes()
            );
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        for tick in 0..100u64 {
            driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            if session.state().sent_client_snapshot_count > 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(session.state().world_stream_loaded);
        assert!(session.state().connect_confirm_sent);
        assert!(session.state().connect_confirm_flushed);
        assert_eq!(session.state().sent_client_snapshot_count, 1);
        assert_eq!(session.state().last_sent_client_snapshot_id, Some(1));
        server.join().unwrap();
    }

    #[test]
    fn flushes_connect_confirm_before_queued_chat_on_first_arcnet_ready_tick() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        session
            .queue_send_chat_message("/sync".to_string())
            .unwrap();
        let expected_connect_confirm_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connectConfirm")
            .unwrap()
            .packet_id;
        let expected_chat_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendChatMessage")
            .unwrap()
            .packet_id;
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 891 }),
            );

            let mut udp_buf = [0u8; 2048];
            let (udp_len, _) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 891 }
            );

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterUdp { connection_id: 0 }),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(&mut tcp_stream, &begin_packet);
            for chunk in &chunk_packets {
                write_tcp_frame(&mut tcp_stream, chunk);
            }

            let mut saw_connect_confirm = false;
            let mut saw_chat = false;
            for _ in 0..6 {
                let frame = read_tcp_frame(&mut tcp_stream);
                if let Ok(packet) = decode_packet(&frame) {
                    if !saw_connect_confirm {
                        if packet.packet_id == expected_connect_confirm_packet_id {
                            assert!(packet.payload.is_empty());
                            saw_connect_confirm = true;
                            continue;
                        }
                    } else if packet.packet_id == expected_chat_packet_id {
                        assert_eq!(packet.payload, encode_typeio_string_payload("/sync"));
                        saw_chat = true;
                        break;
                    }
                    panic!(
                        "unexpected TCP packet after world ready: {}",
                        packet.packet_id
                    );
                }
                assert_eq!(
                    decode_framework_message(&frame).unwrap(),
                    FrameworkMessage::KeepAlive
                );
            }
            assert!(saw_connect_confirm);
            assert!(saw_chat);
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        for tick in 0..100u64 {
            driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            if session.state().connect_confirm_flushed
                && session.state().last_connect_confirm_flushed_at_ms.is_some()
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(session.state().world_stream_loaded);
        assert!(session.state().connect_confirm_sent);
        assert!(session.state().connect_confirm_flushed);
        server.join().unwrap();
    }

    #[test]
    fn accepts_udp_register_on_udp_and_counts_framework_traffic() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 901 }),
            );

            let mut udp_buf = [0u8; 1024];
            let (udp_len, client_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 901 }
            );

            udp_socket
                .send_to(
                    &encode_framework_message(&FrameworkMessage::RegisterUdp {
                        connection_id: 901,
                    }),
                    client_addr,
                )
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::KeepAlive),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);
            thread::sleep(Duration::from_millis(200));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        let mut saw_connect_sent = false;
        let mut inbound_tcp_frames = 0usize;
        let mut inbound_udp_packets = 0usize;
        let mut inbound_framework_messages = 0usize;

        for tick in 0..50u64 {
            let report = driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            inbound_tcp_frames += report.inbound_tcp_frames;
            inbound_udp_packets += report.inbound_udp_packets;
            inbound_framework_messages += report.inbound_framework_messages;
            if report.connect_sent {
                saw_connect_sent = true;
            }
            if saw_connect_sent {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_connect_sent);
        assert!(driver.udp_registered);
        assert!(inbound_tcp_frames >= 1);
        assert!(inbound_udp_packets >= 1);
        assert!(inbound_framework_messages >= 2);
        server.join().unwrap();
    }

    #[test]
    fn ping_replies_follow_inbound_transport_and_count_framework_traffic() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 903 }),
            );

            let mut udp_buf = [0u8; 1024];
            let (udp_len, client_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 903 }
            );

            udp_socket
                .send_to(
                    &encode_framework_message(&FrameworkMessage::RegisterUdp {
                        connection_id: 903,
                    }),
                    client_addr,
                )
                .unwrap();

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::Ping {
                    id: 41,
                    is_reply: false,
                }),
            );
            let mut tcp_reply = None;
            for _ in 0..4 {
                let frame = read_tcp_frame(&mut tcp_stream);
                if let Ok(message) = decode_framework_message(&frame) {
                    if matches!(message, FrameworkMessage::KeepAlive) {
                        continue;
                    }
                    tcp_reply = Some(message);
                    break;
                }
            }

            udp_socket
                .send_to(
                    &encode_framework_message(&FrameworkMessage::Ping {
                        id: 42,
                        is_reply: false,
                    }),
                    client_addr,
                )
                .unwrap();
            let (udp_reply_len, udp_reply_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(udp_reply_addr, client_addr);
            let udp_reply = decode_framework_message(&udp_buf[..udp_reply_len]).unwrap();

            done_tx
                .send((tcp_reply.expect("expected tcp ping reply"), udp_reply))
                .unwrap();
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        let mut outbound_tcp_frames = 0usize;
        let mut outbound_udp_packets = 0usize;
        let mut outbound_framework_messages = 0usize;
        let mut replies = None;

        for tick in 0..100u64 {
            if let Ok(done) = done_rx.try_recv() {
                replies = Some(done);
                break;
            }
            let report = driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            outbound_tcp_frames += report.outbound_tcp_frames;
            outbound_udp_packets += report.outbound_udp_packets;
            outbound_framework_messages += report.outbound_framework_messages;
            if let Ok(done) = done_rx.try_recv() {
                replies = Some(done);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let (tcp_reply, udp_reply) = replies.expect("expected ping replies");
        assert_eq!(
            tcp_reply,
            FrameworkMessage::Ping {
                id: 41,
                is_reply: true,
            }
        );
        assert_eq!(
            udp_reply,
            FrameworkMessage::Ping {
                id: 42,
                is_reply: true,
            }
        );
        assert!(outbound_tcp_frames >= 2);
        assert!(outbound_udp_packets >= 2);
        assert!(outbound_framework_messages >= 3);
        server.join().unwrap();
    }

    #[test]
    fn ping_replies_are_transport_observable() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 904 }),
            );

            let mut udp_buf = [0u8; 1024];
            let (udp_len, client_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 904 }
            );

            udp_socket
                .send_to(
                    &encode_framework_message(&FrameworkMessage::RegisterUdp {
                        connection_id: 904,
                    }),
                    client_addr,
                )
                .unwrap();

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::Ping {
                    id: 51,
                    is_reply: true,
                }),
            );
            udp_socket
                .send_to(
                    &encode_framework_message(&FrameworkMessage::Ping {
                        id: 52,
                        is_reply: true,
                    }),
                    client_addr,
                )
                .unwrap();

            let _ = done_rx.recv_timeout(Duration::from_secs(5));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        let mut inbound_tcp_ping_replies = 0usize;
        let mut inbound_udp_ping_replies = 0usize;

        for tick in 0..100u64 {
            let report = driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            inbound_tcp_ping_replies += report.inbound_tcp_ping_replies;
            inbound_udp_ping_replies += report.inbound_udp_ping_replies;
            if inbound_tcp_ping_replies >= 1 && inbound_udp_ping_replies >= 1 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(inbound_tcp_ping_replies, 1);
        assert_eq!(inbound_udp_ping_replies, 1);
        done_tx.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn surfaces_ready_snapshot_stall_timeout_kind_on_arcnet_timeout() {
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

        let (tcp_listener, _udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (_tcp_stream, _) = tcp_listener.accept().unwrap();
            thread::sleep(Duration::from_millis(200));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.connect_sent = true;

        let report = driver.tick(&mut session, 2_400, 32, 32).unwrap();

        assert_eq!(report.timed_out, Some(2_400));
        assert_eq!(report.timed_out_reason, Some(ReconnectReasonKind::Timeout));
        assert_eq!(
            report.timed_out_kind,
            Some(SessionTimeoutKind::ReadySnapshotStall)
        );
        server.join().unwrap();
    }

    #[test]
    fn entity_snapshot_overrides_wrong_input_before_next_udp_client_snapshot() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 10_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let expected_connect_confirm_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connectConfirm")
            .unwrap()
            .packet_id;
        let expected_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientSnapshot")
            .unwrap()
            .packet_id;
        let expected_entity_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "entitySnapshot")
            .unwrap()
            .packet_id;
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let entity_snapshot_wire = encode_packet(
            expected_entity_snapshot_packet_id,
            &sample_snapshot_packet("entitySnapshot.packet"),
            false,
        )
        .unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 889 }),
            );

            let mut udp_buf = [0u8; 4096];
            let (udp_len, client_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 889 }
            );

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterUdp { connection_id: 0 }),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(&mut tcp_stream, &begin_packet);
            for chunk in &chunk_packets {
                write_tcp_frame(&mut tcp_stream, chunk);
            }

            let mut saw_connect_confirm = false;
            for _ in 0..4 {
                let frame = read_tcp_frame(&mut tcp_stream);
                if let Ok(packet) = decode_packet(&frame) {
                    if packet.packet_id == expected_connect_confirm_packet_id {
                        saw_connect_confirm = true;
                        break;
                    }
                } else {
                    assert_eq!(
                        decode_framework_message(&frame).unwrap(),
                        FrameworkMessage::KeepAlive
                    );
                }
            }
            assert!(saw_connect_confirm);

            let (snapshot_len, snapshot_addr) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(snapshot_addr, client_addr);
            let snapshot_packet = decode_packet(&udp_buf[..snapshot_len]).unwrap();
            assert_eq!(snapshot_packet.packet_id, expected_snapshot_packet_id);
            assert_eq!(&snapshot_packet.payload[4..8], &100i32.to_be_bytes());
            assert_eq!(
                &snapshot_packet.payload[9..13],
                &0.0f32.to_bits().to_be_bytes()
            );
            assert_eq!(
                &snapshot_packet.payload[13..17],
                &0.0f32.to_bits().to_be_bytes()
            );
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        let mut injected_wrong_input = false;
        for tick in 0..200u64 {
            driver.tick(&mut session, tick * 100, 32, 32).unwrap();

            if session.state().world_stream_loaded && !injected_wrong_input {
                let input = session.snapshot_input_mut();
                input.unit_id = Some(999);
                input.position = Some((999.0, 999.0));
                input.view_center = Some((999.0, 999.0));
                let event = session.ingest_packet_bytes(&entity_snapshot_wire).unwrap();
                assert_eq!(
                    event,
                    ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
                );
                injected_wrong_input = true;
            }

            if session.state().sent_client_snapshot_count > 0
                && session.state().world_player_unit_value == Some(100)
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(injected_wrong_input);
        assert_eq!(session.state().sent_client_snapshot_count, 1);
        assert_eq!(session.state().last_sent_client_snapshot_id, Some(1));
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, Some(100));
        assert_eq!(input.position, Some((0.0, 0.0)));
        assert_eq!(input.view_center, Some((0.0, 0.0)));
        server.join().unwrap();
    }

    #[test]
    fn surfaces_build_health_update_over_tcp_after_world_ready() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
            timeout_ms: 60_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let expected_connect_confirm_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connectConfirm")
            .unwrap()
            .packet_id;
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let build_health_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "buildHealthUpdate")
            .unwrap()
            .packet_id;
        let mut build_health_payload = Vec::new();
        build_health_payload.extend_from_slice(&2i32.to_be_bytes());
        build_health_payload.extend_from_slice(&321i32.to_be_bytes());
        build_health_payload.extend_from_slice(&(12.5f32.to_bits() as i32).to_be_bytes());
        let build_health_packet =
            encode_packet(build_health_packet_id, &build_health_payload, false).unwrap();

        let (tcp_listener, udp_socket, server_addr) = bind_local_arcnet_server();
        let server = thread::spawn(move || {
            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            tcp_stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            tcp_stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterTcp { connection_id: 890 }),
            );

            let mut udp_buf = [0u8; 2048];
            let (udp_len, _) = udp_socket.recv_from(&mut udp_buf).unwrap();
            assert_eq!(
                decode_framework_message(&udp_buf[..udp_len]).unwrap(),
                FrameworkMessage::RegisterUdp { connection_id: 890 }
            );

            write_tcp_frame(
                &mut tcp_stream,
                &encode_framework_message(&FrameworkMessage::RegisterUdp { connection_id: 0 }),
            );

            let connect_frame = read_tcp_frame(&mut tcp_stream);
            let connect_packet = decode_packet(&connect_frame).unwrap();
            assert_eq!(connect_packet.packet_id, CONNECT_PACKET_ID);

            write_tcp_frame(&mut tcp_stream, &begin_packet);
            for chunk in &chunk_packets {
                write_tcp_frame(&mut tcp_stream, chunk);
            }

            let mut saw_connect_confirm = false;
            for _ in 0..4 {
                let frame = read_tcp_frame(&mut tcp_stream);
                if let Ok(packet) = decode_packet(&frame) {
                    if packet.packet_id == expected_connect_confirm_packet_id {
                        saw_connect_confirm = true;
                        break;
                    }
                } else {
                    assert_eq!(
                        decode_framework_message(&frame).unwrap(),
                        FrameworkMessage::KeepAlive
                    );
                }
            }
            assert!(saw_connect_confirm);
            write_tcp_frame(&mut tcp_stream, &build_health_packet);
            thread::sleep(Duration::from_millis(100));
        });

        let mut driver = ArcNetSessionDriver::connect(server_addr).unwrap();
        driver.send_connect(&connect).unwrap();

        let mut saw_build_health_update = false;
        for tick in 0..100u64 {
            let report = driver.tick(&mut session, tick * 100, 32, 32).unwrap();
            if report.events.iter().any(|event| {
                matches!(
                    event,
                    ClientSessionEvent::BuildHealthUpdate {
                        pair_count: 1,
                        first_build_pos: Some(321),
                        first_health_bits: Some(bits),
                        ..
                    } if *bits == 12.5f32.to_bits()
                )
            }) {
                saw_build_health_update = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_build_health_update);
        assert_eq!(session.state().received_build_health_update_count, 1);
        assert_eq!(session.state().received_build_health_update_pair_count, 1);
        assert_eq!(session.state().last_build_health_update_pair_count, 1);
        assert_eq!(
            session.state().last_build_health_update_first_build_pos,
            Some(321)
        );
        assert_eq!(
            session.state().last_build_health_update_first_health_bits,
            Some(12.5f32.to_bits())
        );
        server.join().unwrap();
    }
}
