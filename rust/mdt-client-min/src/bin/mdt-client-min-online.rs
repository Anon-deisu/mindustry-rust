use mdt_client_min::arcnet_loop::ArcNetSessionDriver;
use mdt_client_min::client_session::{
    ClientBuildPlan, ClientBuildPlanConfig, ClientLogicDataTransport, ClientPacketTransport,
    ClientSession, ClientSessionEvent, ClientSessionTiming, ClientUnitRef,
    StateSnapshotAppliedProjection, KICK_REASON_SERVER_RESTARTING_ORDINAL,
};
use mdt_client_min::connect_packet::{
    default_connect_build, default_connect_version_type, ConnectCompatibilityWarning,
    ConnectPacketSpec,
};
use mdt_client_min::render_runtime::RenderRuntimeAdapter;
use mdt_input::live_intent::RuntimeIntentTracker;
use mdt_input::{
    flip_plans, rotate_plans, BinaryAction, CommandModeRectProjection, CommandModeState,
    CommandUnitRef, InputSnapshot, IntentSamplingMode, LiveIntentState, MovementProbeConfig,
    MovementProbeController, PlanBlockMeta, PlanEditable, PlanPoint, RuntimeInputState,
};
use mdt_remote::HighFrequencyRemoteMethod;
use mdt_remote::{read_remote_manifest, RemoteManifest};
use mdt_render_ui::{
    project_scene_models_with_view_window, AsciiScenePresenter, MinifbWindowBackend, RenderObject,
    ScenePresenter, WindowPresenter,
};
use mdt_typeio::{pack_point2, TypeIoObject};
use mdt_world::{LoadedWorldState, ParsedBuildingTail, WorldGraph};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

#[path = "../custom_packet_runtime.rs"]
mod custom_packet_runtime;
#[path = "../custom_packet_runtime_relay.rs"]
mod custom_packet_runtime_relay;

use custom_packet_runtime::{
    build_runtime_custom_packet_semantic_specs, install_runtime_custom_packet_semantics,
    RuntimeCustomPacketSemanticSpec, RuntimeCustomPacketSemantics,
};
use custom_packet_runtime_relay::{
    build_runtime_custom_packet_relay_specs, install_runtime_custom_packet_relays,
    RuntimeCustomPacketRelayAction, RuntimeCustomPacketRelaySpec, RuntimeCustomPacketRelays,
};

const LIVE_VIEW_TILES: (usize, usize) = (64, 32);
const SERVER_RESTART_RETRY_BACKOFF_MS: u64 = 1_000;
const DEFAULT_DISCOVER_PORT: u16 = 6567;
const DEFAULT_DISCOVER_TIMEOUT_MS: u64 = 1_500;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_args = env::args().collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }
    let args = parse_args(raw_args)?;
    let manifest = read_remote_manifest(&args.manifest_path)?;
    let timing = resolve_session_timing(&args);
    let mut session =
        ClientSession::from_remote_manifest_with_timing(&manifest, args.locale.clone(), timing)?;
    apply_snapshot_overrides(&mut session, &args);
    let mut custom_packet_watch = install_runtime_custom_packet_watch(&mut session, &args);
    let mut custom_packet_semantics = install_runtime_custom_packet_semantics(
        &mut session,
        &args.runtime_custom_packet_semantics,
    );
    let mut custom_packet_relays =
        install_runtime_custom_packet_relays(&mut session, &args.runtime_custom_packet_relays);
    let connect_payload = load_connect_payload(&args.connect)?;
    let connect = session.prepare_connect_packet(&connect_payload)?;

    let mut current_server_addr = resolve_initial_server_addr(&args)?;
    let mut driver = ArcNetSessionDriver::connect(current_server_addr)?;
    let mut movement_probe = args.movement_probe.map(MovementProbeController::new);
    let mut live_intent_mapper = build_live_intent_mapper(&args);
    let mut runtime_plan_edit_loop = args
        .runtime_plan_edit_loop
        .clone()
        .map(RuntimePlanEditLoopState::new);
    let mut runtime_command_mode = CommandModeState::default();
    apply_runtime_command_mode_cli_ops(&mut runtime_command_mode, &args.command_mode_ops);
    let mut relative_build_plans_applied = false;
    let mut auto_build_plans_applied = false;
    let mut ascii_scene_printed = false;
    let mut world_stream_dumped = false;
    let mut render_runtime_adapter = RenderRuntimeAdapter::default();
    let mut window_scene_presenter = args.render_window_live.then(|| {
        WindowPresenter::new(MinifbWindowBackend::new(12, "mdt-client-min-online"))
            .with_max_view_tiles(64, 32)
    });
    let mut window_scene_disabled = false;
    let mut last_runtime_input = None;
    let mut next_chat_index = 0usize;
    let mut next_outbound_action_index = 0usize;
    let mut pending_restart_reconnect_at_ms = None;
    let tcp_local_addr = driver.tcp_local_addr()?;
    let udp_local_addr = driver.udp_local_addr()?;
    driver.send_connect(&connect)?;
    println!(
        "connected: tcp_local={}, udp_local={}, server={}, connect_packet_len={}",
        tcp_local_addr,
        udp_local_addr,
        current_server_addr,
        connect.encoded_packet.len()
    );

    let start = Instant::now();
    loop {
        let elapsed = start.elapsed();
        if elapsed >= args.duration {
            break;
        }

        let now_ms = elapsed.as_millis() as u64;
        let report = driver.tick_with_post_ingest_hook(
            &mut session,
            now_ms,
            args.max_recv_packets,
            args.max_recv_packets,
            |session| {
                maybe_apply_runtime_snapshot_overrides(
                    session,
                    &args,
                    movement_probe.as_mut(),
                    live_intent_mapper.as_mut(),
                    timing.client_snapshot_interval_ms,
                    now_ms,
                );
            },
        )?;
        let pending_redirect = first_connect_redirect_target(&report.events);
        let has_pending_redirect = pending_redirect.is_some();
        if let Some(delay_ms) = first_server_restart_reconnect_delay_ms(&report.events) {
            pending_restart_reconnect_at_ms = Some(now_ms.saturating_add(delay_ms));
            println!(
                "restart_reconnect_scheduled: server={} delay_ms={}",
                current_server_addr, delay_ms
            );
        }
        if report.outbound_tcp_frames > 0
            || report.outbound_udp_packets > 0
            || report.inbound_tcp_frames > 0
            || report.inbound_udp_packets > 0
            || report.outbound_framework_messages > 0
            || report.inbound_framework_messages > 0
            || report.timed_out.is_some()
            || !report.events.is_empty()
            || report.connect_sent
        {
            println!(
                "tick={}ms tcp(out/in)={}/{} udp(out/in)={}/{} fw(out/in)={}/{} registered={} connect_sent={} timeout={:?} events={:?}",
                now_ms,
                report.outbound_tcp_frames,
                report.inbound_tcp_frames,
                report.outbound_udp_packets,
                report.inbound_udp_packets,
                report.outbound_framework_messages,
                report.inbound_framework_messages,
                report.udp_registered,
                report.connect_sent,
                report.timed_out,
                report.events
            );
        }
        if report
            .events
            .iter()
            .any(|event| matches!(event, ClientSessionEvent::WorldDataBegin))
        {
            relative_build_plans_applied = false;
            auto_build_plans_applied = false;
            runtime_command_mode.clear();
            apply_runtime_command_mode_cli_ops(&mut runtime_command_mode, &args.command_mode_ops);
        }
        render_runtime_adapter.observe_events(&report.events);
        maybe_print_runtime_input(
            &mut session,
            &args,
            &report.events,
            now_ms,
            &mut last_runtime_input,
        );
        maybe_print_client_packets(&args, &report.events);
        maybe_print_custom_packet_watch_events(custom_packet_watch.as_mut(), &report.events);
        maybe_print_custom_packet_semantic_events(custom_packet_semantics.as_mut(), &report.events);
        maybe_queue_runtime_custom_packet_relays(
            &mut session,
            custom_packet_relays.as_mut(),
            &report.events,
            now_ms,
        )?;
        if let Some((redirect_ip, redirect_port)) = pending_redirect {
            if let Some(redirect_addr) = resolve_redirect_server_addr(&redirect_ip, redirect_port) {
                println!(
                    "redirect_requested: from={} to={} (source={}:{})",
                    current_server_addr, redirect_addr, redirect_ip, redirect_port
                );
                match reconnect_runtime_session(
                    &mut driver,
                    &manifest,
                    &args,
                    timing,
                    &connect_payload,
                    redirect_addr,
                ) {
                    Ok((redirected_session, redirected_watch, redirected_semantics)) => {
                        current_server_addr = redirect_addr;
                        session = redirected_session;
                        custom_packet_watch = redirected_watch;
                        custom_packet_semantics = redirected_semantics;
                        custom_packet_relays = install_runtime_custom_packet_relays(
                            &mut session,
                            &args.runtime_custom_packet_relays,
                        );
                        relative_build_plans_applied = false;
                        auto_build_plans_applied = false;
                        ascii_scene_printed = false;
                        world_stream_dumped = false;
                        render_runtime_adapter = RenderRuntimeAdapter::default();
                        runtime_command_mode.clear();
                        apply_runtime_command_mode_cli_ops(
                            &mut runtime_command_mode,
                            &args.command_mode_ops,
                        );
                        last_runtime_input = None;
                        pending_restart_reconnect_at_ms = None;
                        let tcp_local_addr = driver.tcp_local_addr()?;
                        let udp_local_addr = driver.udp_local_addr()?;
                        println!(
                            "redirect_connected: tcp_local={}, udp_local={}, server={}",
                            tcp_local_addr, udp_local_addr, current_server_addr
                        );
                        continue;
                    }
                    Err(error) => {
                        println!(
                            "redirect_connect_failed: target={} error={error}",
                            redirect_addr
                        );
                    }
                }
            } else {
                println!(
                    "redirect_ignored_unresolvable_target: source={}:{}",
                    redirect_ip, redirect_port
                );
            }
        }
        if !has_pending_redirect
            && pending_restart_reconnect_at_ms
                .is_some_and(|reconnect_at_ms| now_ms >= reconnect_at_ms)
        {
            println!("restart_reconnect_attempt: server={}", current_server_addr);
            match reconnect_runtime_session(
                &mut driver,
                &manifest,
                &args,
                timing,
                &connect_payload,
                current_server_addr,
            ) {
                Ok((reconnected_session, reconnected_watch, reconnected_semantics)) => {
                    session = reconnected_session;
                    custom_packet_watch = reconnected_watch;
                    custom_packet_semantics = reconnected_semantics;
                    custom_packet_relays = install_runtime_custom_packet_relays(
                        &mut session,
                        &args.runtime_custom_packet_relays,
                    );
                    relative_build_plans_applied = false;
                    auto_build_plans_applied = false;
                    ascii_scene_printed = false;
                    world_stream_dumped = false;
                    render_runtime_adapter = RenderRuntimeAdapter::default();
                    runtime_command_mode.clear();
                    apply_runtime_command_mode_cli_ops(
                        &mut runtime_command_mode,
                        &args.command_mode_ops,
                    );
                    last_runtime_input = None;
                    pending_restart_reconnect_at_ms = None;
                    let tcp_local_addr = driver.tcp_local_addr()?;
                    let udp_local_addr = driver.udp_local_addr()?;
                    println!(
                        "restart_reconnected: tcp_local={}, udp_local={}, server={}",
                        tcp_local_addr, udp_local_addr, current_server_addr
                    );
                    continue;
                }
                Err(error) => {
                    pending_restart_reconnect_at_ms =
                        Some(now_ms.saturating_add(SERVER_RESTART_RETRY_BACKOFF_MS));
                    println!(
                        "restart_reconnect_failed: target={} retry_in_ms={} error={error}",
                        current_server_addr, SERVER_RESTART_RETRY_BACKOFF_MS
                    );
                }
            }
        }
        maybe_print_ascii_scene(
            &session,
            &args,
            &report.events,
            &render_runtime_adapter,
            &runtime_command_mode,
            &mut ascii_scene_printed,
        );
        maybe_dump_world_stream_hex(&session, &args, &mut world_stream_dumped)?;
        maybe_present_window_scene(
            &session,
            &args,
            &report.events,
            &render_runtime_adapter,
            &runtime_command_mode,
            &mut window_scene_presenter,
            &mut window_scene_disabled,
        );
        maybe_apply_relative_build_plans(
            &mut session,
            &args,
            &report.events,
            &mut relative_build_plans_applied,
        );
        maybe_apply_auto_build_plans(
            &mut session,
            &args,
            &report.events,
            &mut auto_build_plans_applied,
        );
        maybe_apply_runtime_plan_edit_loop(&mut session, runtime_plan_edit_loop.as_mut(), now_ms);
        sync_runtime_build_selection_state(&mut session, &args);
        maybe_queue_chat_messages(&mut session, &args, now_ms, &mut next_chat_index)?;
        maybe_queue_outbound_actions(
            &mut session,
            &args,
            now_ms,
            &mut next_outbound_action_index,
            &mut runtime_command_mode,
        )?;

        thread::sleep(args.tick);
    }

    maybe_print_final_ascii_scene(&session, &args, &render_runtime_adapter, &runtime_command_mode);
    maybe_print_custom_packet_watch_summary(custom_packet_watch.as_ref());
    maybe_print_custom_packet_semantic_summary(custom_packet_semantics.as_ref());
    maybe_print_runtime_custom_packet_relay_summary(custom_packet_relays.as_ref());
    let final_input = session.snapshot_input_mut().clone();
    println!(
        "final: packets_seen={} snapshots={} world_loaded={} ready={} keepalive_sent={} client_snapshot_sent={} timed_out={}",
        session.stats().packets_seen,
        session.stats().snapshot_packets_seen,
        session.state().world_stream_loaded,
        session.state().ready_to_enter_world,
        session.state().sent_keepalive_count,
        session.state().sent_client_snapshot_count,
        session.state().connection_timed_out
    );
    println!(
        "{}",
        format_final_kick_summary(
            session.kicked(),
            session.last_kick_reason_text(),
            session.last_kick_reason_ordinal(),
            session.last_kick_duration_ms(),
        )
    );
    println!(
        "final_input: unit_id={:?} dead={} position={:?} velocity=({:.3},{:.3}) pointer={:?}",
        final_input.unit_id,
        final_input.dead,
        final_input.position,
        final_input.velocity.0,
        final_input.velocity.1,
        final_input.pointer
    );
    Ok(())
}

fn first_connect_redirect_target(events: &[ClientSessionEvent]) -> Option<(String, i32)> {
    events.iter().find_map(|event| match event {
        ClientSessionEvent::ConnectRedirectRequested { ip, port } => Some((ip.clone(), *port)),
        _ => None,
    })
}

fn first_server_restart_reconnect_delay_ms(events: &[ClientSessionEvent]) -> Option<u64> {
    events.iter().find_map(|event| match event {
        ClientSessionEvent::Kicked {
            reason_ordinal: Some(KICK_REASON_SERVER_RESTARTING_ORDINAL),
            duration_ms,
            ..
        } => Some(duration_ms.unwrap_or(0)),
        _ => None,
    })
}

fn resolve_redirect_server_addr(ip: &str, port: i32) -> Option<SocketAddr> {
    let port = u16::try_from(port).ok()?;
    (ip, port).to_socket_addrs().ok()?.next()
}

fn resolve_initial_server_addr(args: &CliArgs) -> Result<SocketAddr, String> {
    match &args.server_target {
        ServerTarget::Direct(server_addr) => Ok(*server_addr),
        ServerTarget::Discover(config) => {
            let targets = resolve_discovery_targets(&config.hosts, config.port)?;
            let discovered =
                ArcNetSessionDriver::discover_first_server(&targets, config.timeout)
                    .map_err(|error| format!("failed to probe discovery targets: {error}"))?;
            let Some(server_addr) = discovered else {
                return Err(format!(
                    "host discovery timed out after {}ms for targets: {}",
                    config.timeout.as_millis(),
                    format_socket_addrs(&targets)
                ));
            };
            println!(
                "discover_selected: server={} timeout_ms={} candidates={}",
                server_addr,
                config.timeout.as_millis(),
                targets.len()
            );
            Ok(server_addr)
        }
    }
}

fn resolve_discovery_targets(hosts: &[String], port: u16) -> Result<Vec<SocketAddr>, String> {
    let mut targets = Vec::new();
    for host in hosts {
        let mut resolved_count = 0usize;
        let resolved = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|error| format!("failed to resolve --discover-host {host}: {error}"))?;
        for addr in resolved {
            resolved_count += 1;
            if !targets.contains(&addr) {
                targets.push(addr);
            }
        }
        if resolved_count == 0 {
            return Err(format!(
                "failed to resolve --discover-host {host}: no addresses returned"
            ));
        }
    }

    if targets.is_empty() {
        return Err("no discovery targets resolved".to_string());
    }
    Ok(targets)
}

fn format_socket_addrs(addrs: &[SocketAddr]) -> String {
    addrs
        .iter()
        .map(|addr| addr.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn reconnect_runtime_session(
    driver: &mut ArcNetSessionDriver,
    manifest: &RemoteManifest,
    args: &CliArgs,
    timing: ClientSessionTiming,
    connect_payload: &[u8],
    server_addr: SocketAddr,
) -> Result<
    (
        ClientSession,
        Option<RuntimeCustomPacketWatch>,
        Option<RuntimeCustomPacketSemantics>,
    ),
    Box<dyn std::error::Error>,
> {
    let mut session =
        ClientSession::from_remote_manifest_with_timing(manifest, args.locale.clone(), timing)?;
    apply_snapshot_overrides(&mut session, args);
    let connect = session.prepare_connect_packet(connect_payload)?;
    driver.reconnect(server_addr, &connect)?;
    let custom_packet_watch = install_runtime_custom_packet_watch(&mut session, args);
    let custom_packet_semantics = install_runtime_custom_packet_semantics(
        &mut session,
        &args.runtime_custom_packet_semantics,
    );
    Ok((session, custom_packet_watch, custom_packet_semantics))
}

struct CliArgs {
    manifest_path: PathBuf,
    server_target: ServerTarget,
    locale: String,
    duration: Duration,
    tick: Duration,
    max_recv_packets: usize,
    snapshot_pointer: Option<(f32, f32)>,
    snapshot_mining_tile: Option<(i32, i32)>,
    snapshot_boosting: Option<bool>,
    snapshot_shooting: Option<bool>,
    snapshot_chatting: Option<bool>,
    snapshot_building: Option<bool>,
    snapshot_view_size: Option<(f32, f32)>,
    snapshot_interval_ms: Option<u64>,
    movement_probe: Option<MovementProbeConfig>,
    enable_live_intent_runtime_capture: bool,
    live_intent_sampling_mode: IntentSamplingMode,
    live_intent_schedule: Vec<ScheduledIntentSnapshot>,
    command_mode_ops: Vec<CommandModeCliOp>,
    runtime_plan_edit_loop: Option<PlanEditLoopConfig>,
    build_plans: Vec<ClientBuildPlan>,
    relative_build_plans: Vec<RelativeBuildPlan>,
    auto_break_near_player: bool,
    auto_place_near_player: Vec<AutoPlacePlan>,
    auto_place_conflict_near_player: Vec<AutoPlacePlan>,
    render_ascii_on_world_ready: bool,
    print_client_packets: bool,
    watched_client_packet_types: Vec<String>,
    watched_client_binary_packet_types: Vec<String>,
    watched_client_logic_data_channels: Vec<String>,
    runtime_custom_packet_semantics: Vec<RuntimeCustomPacketSemanticSpec>,
    runtime_custom_packet_relays: Vec<RuntimeCustomPacketRelaySpec>,
    render_window_live: bool,
    dump_world_stream_hex: Option<PathBuf>,
    chat_schedule: Vec<ScheduledChatEntry>,
    outbound_action_schedule: Vec<ScheduledOutboundAction>,
    connect: ConnectSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ServerTarget {
    Direct(SocketAddr),
    Discover(DiscoveryConfig),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiscoveryConfig {
    hosts: Vec<String>,
    port: u16,
    timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScheduledChatEntry {
    not_before_ms: u64,
    text: String,
}

#[derive(Clone, Debug, PartialEq)]
struct ScheduledOutboundAction {
    not_before_ms: u64,
    action: OutboundAction,
}

#[derive(Clone, Debug, PartialEq)]
struct ScheduledIntentSnapshot {
    not_before_ms: u64,
    snapshot: InputSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CommandModeCliOp {
    BindGroup { index: u8, unit_ids: Vec<i32> },
    RecallGroup { index: u8 },
    ClearGroup { index: u8 },
    SetRect(Option<CommandModeRectProjection>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlanEditLoopConfig {
    ops: Vec<PlanEditOp>,
    delay_ms: u64,
    spacing_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
enum OutboundAction {
    RequestItem {
        build_pos: Option<i32>,
        item_id: Option<i16>,
        amount: i32,
    },
    RequestUnitPayload {
        target: ClientUnitRef,
    },
    UnitClear,
    UnitControl {
        target: ClientUnitRef,
    },
    UnitBuildingControlSelect {
        target: ClientUnitRef,
        build_pos: Option<i32>,
    },
    BuildingControlSelect {
        build_pos: Option<i32>,
    },
    ClearItems {
        build_pos: Option<i32>,
    },
    ClearLiquids {
        build_pos: Option<i32>,
    },
    TransferInventory {
        build_pos: Option<i32>,
    },
    RequestBuildPayload {
        build_pos: Option<i32>,
    },
    RequestDropPayload {
        x: f32,
        y: f32,
    },
    RotateBlock {
        build_pos: Option<i32>,
        direction: bool,
    },
    DropItem {
        angle: f32,
    },
    TileConfig {
        build_pos: Option<i32>,
        value: TypeIoObject,
    },
    TileTap {
        tile_pos: Option<i32>,
    },
    DeletePlans {
        positions: Vec<i32>,
    },
    CommandBuilding {
        buildings: Vec<i32>,
        x: f32,
        y: f32,
    },
    CommandUnits {
        unit_ids: Vec<i32>,
        build_target: Option<i32>,
        unit_target: ClientUnitRef,
        pos_target: Option<(f32, f32)>,
        queue_command: bool,
        final_batch: bool,
    },
    CommandUnitsChunked {
        unit_ids: Vec<i32>,
        build_target: Option<i32>,
        unit_target: ClientUnitRef,
        pos_target: Option<(f32, f32)>,
        queue_command: bool,
    },
    SetUnitCommand {
        unit_ids: Vec<i32>,
        command_id: Option<u8>,
    },
    SetUnitStance {
        unit_ids: Vec<i32>,
        stance_id: Option<u8>,
        enable: bool,
    },
    BeginBreak {
        builder: ClientUnitRef,
        team_id: u8,
        x: i32,
        y: i32,
    },
    BeginPlace {
        builder: ClientUnitRef,
        block_id: Option<i16>,
        team_id: u8,
        x: i32,
        y: i32,
        rotation: i32,
        place_config: TypeIoObject,
    },
    MenuChoose {
        menu_id: i32,
        option: i32,
    },
    TextInputResult {
        text_input_id: i32,
        text: Option<String>,
    },
    ClientPacket {
        packet_type: String,
        contents: String,
        transport: ClientPacketTransport,
    },
    ClientBinaryPacket {
        packet_type: String,
        contents: Vec<u8>,
        transport: ClientPacketTransport,
    },
    ClientLogicData {
        channel: String,
        value: TypeIoObject,
        transport: ClientLogicDataTransport,
    },
}

#[derive(Debug)]
struct LiveIntentMapperController {
    tracker: RuntimeIntentTracker,
    schedule: Vec<ScheduledIntentSnapshot>,
    next_snapshot_index: usize,
}

impl LiveIntentMapperController {
    fn new(schedule: Vec<ScheduledIntentSnapshot>, sampling_mode: IntentSamplingMode) -> Self {
        Self {
            tracker: RuntimeIntentTracker::new(sampling_mode),
            schedule,
            next_snapshot_index: 0,
        }
    }

    fn state(&self) -> &LiveIntentState {
        self.tracker.state()
    }

    fn advance(&mut self, runtime_snapshot: &InputSnapshot, now_ms: u64) -> bool {
        let due =
            collect_due_intent_snapshots(&self.schedule, now_ms, &mut self.next_snapshot_index);
        if due.is_empty() {
            return self.tracker.sample_runtime_snapshot(runtime_snapshot);
        }
        let snapshots = due
            .into_iter()
            .map(|entry| entry.snapshot)
            .collect::<Vec<_>>();
        self.tracker.sample_runtime_snapshot_batch(&snapshots)
    }
}

fn build_live_intent_mapper(args: &CliArgs) -> Option<LiveIntentMapperController> {
    args.enable_live_intent_runtime_capture.then(|| {
        LiveIntentMapperController::new(
            args.live_intent_schedule.clone(),
            args.live_intent_sampling_mode,
        )
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RelativeBuildPlan {
    tile_offset: (i32, i32),
    breaking: bool,
    block_id: Option<i16>,
    rotation: u8,
    config: ClientBuildPlanConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutoPlacePlan {
    block: AutoBlockChoice,
    rotation: Option<u8>,
    config: ClientBuildPlanConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AutoBlockChoice {
    Selected,
    Fixed(i16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PlanEditOp {
    Rotate { origin: (i32, i32), direction: i32 },
    Flip { origin: (i32, i32), flip_x: bool },
}

#[derive(Debug)]
struct RuntimePlanEditLoopState {
    config: PlanEditLoopConfig,
    next_due_ms: Option<u64>,
    next_op_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimePlanEditLoopEntry {
    op_index: usize,
    scheduled_ms: u64,
    op: PlanEditOp,
}

impl RuntimePlanEditLoopState {
    fn new(config: PlanEditLoopConfig) -> Self {
        Self {
            config,
            next_due_ms: None,
            next_op_index: 0,
        }
    }

    fn collect_due_entries(&mut self, now_ms: u64) -> Vec<RuntimePlanEditLoopEntry> {
        if self.config.ops.is_empty() {
            return Vec::new();
        }

        let mut due = Vec::new();
        let next_due_ms = self.next_due_ms.get_or_insert(self.config.delay_ms);
        while now_ms >= *next_due_ms {
            let op_index = self.next_op_index;
            due.push(RuntimePlanEditLoopEntry {
                op_index,
                scheduled_ms: *next_due_ms,
                op: self.config.ops[op_index].clone(),
            });
            self.next_op_index = (self.next_op_index + 1) % self.config.ops.len();
            *next_due_ms = next_due_ms.saturating_add(self.config.spacing_ms);
        }
        due
    }
}

#[derive(Clone)]
enum ConnectSource {
    HexFile(PathBuf),
    Generated(ConnectPacketSpec),
}

fn parse_args(args: Vec<String>) -> Result<CliArgs, String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut server_addr: Option<SocketAddr> = None;
    let mut discover_hosts = Vec::new();
    let mut discover_port = DEFAULT_DISCOVER_PORT;
    let mut discover_timeout_ms = DEFAULT_DISCOVER_TIMEOUT_MS;
    let mut locale = String::from("en_US");
    let mut duration = Duration::from_secs(30);
    let mut tick = Duration::from_millis(50);
    let mut max_recv_packets: usize = 64;
    let mut name = String::from("mdt-client-min");
    let mut version_type = default_connect_version_type().to_string();
    let mut build = default_connect_build();
    let mut uuid: Option<String> = None;
    let mut usid: Option<String> = None;
    let mut mobile = false;
    let mut color = -1i32;
    let mut mods = Vec::new();
    let mut connect_hex_path: Option<PathBuf> = None;
    let mut aim_x: Option<f32> = None;
    let mut aim_y: Option<f32> = None;
    let mut snapshot_mining_tile: Option<(i32, i32)> = None;
    let mut snapshot_boosting: Option<bool> = None;
    let mut snapshot_shooting: Option<bool> = None;
    let mut snapshot_chatting: Option<bool> = None;
    let mut snapshot_building: Option<bool> = None;
    let mut snapshot_view_size: Option<(f32, f32)> = None;
    let mut snapshot_interval_ms: Option<u64> = None;
    let mut move_step_x: Option<f32> = None;
    let mut move_step_y: Option<f32> = None;
    let mut build_plans = Vec::new();
    let mut relative_build_plans = Vec::new();
    let mut auto_break_near_player = false;
    let mut auto_place_near_player = Vec::new();
    let mut auto_place_conflict_near_player = Vec::new();
    let mut render_ascii_on_world_ready = false;
    let mut print_client_packets = false;
    let mut watched_client_packet_types = Vec::new();
    let mut watched_client_binary_packet_types = Vec::new();
    let mut watched_client_logic_data_channels = Vec::new();
    let mut consumed_client_packet_types = Vec::new();
    let mut consumed_client_binary_packet_types = Vec::new();
    let mut consumed_client_logic_data_channels = Vec::new();
    let mut relayed_client_packet_types = Vec::new();
    let mut relayed_client_binary_packet_types = Vec::new();
    let mut relayed_client_logic_data_channels = Vec::new();
    let mut render_window_live = false;
    let mut dump_world_stream_hex = None;
    let mut live_intent_snapshots = Vec::new();
    let mut enable_live_intent_runtime_capture = false;
    let mut live_intent_sampling_mode = IntentSamplingMode::LiveSampling;
    let mut live_intent_delay_ms = 1_000u64;
    let mut live_intent_spacing_ms = 1_000u64;
    let mut command_mode_ops = Vec::new();
    let mut chat_messages = Vec::new();
    let mut chat_delay_ms = 1_000u64;
    let mut chat_spacing_ms = 1_000u64;
    let mut plan_edit_ops = Vec::new();
    let mut plan_edit_loop = false;
    let mut plan_edit_delay_ms = 1_000u64;
    let mut plan_edit_spacing_ms = 1_000u64;
    let mut outbound_actions = Vec::new();
    let mut action_delay_ms = 1_000u64;
    let mut action_spacing_ms = 1_000u64;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --manifest")?;
                manifest_path = Some(PathBuf::from(value));
            }
            "--connect-hex" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --connect-hex")?;
                connect_hex_path = Some(PathBuf::from(value));
            }
            "--name" => {
                i += 1;
                name = args.get(i).ok_or("missing value for --name")?.to_string();
            }
            "--version-type" => {
                i += 1;
                version_type = args
                    .get(i)
                    .ok_or("missing value for --version-type")?
                    .to_string();
            }
            "--build" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --build")?;
                build = value
                    .parse::<i32>()
                    .map_err(|e| format!("invalid --build: {e}"))?;
            }
            "--uuid" => {
                i += 1;
                uuid = Some(args.get(i).ok_or("missing value for --uuid")?.to_string());
            }
            "--usid" => {
                i += 1;
                usid = Some(args.get(i).ok_or("missing value for --usid")?.to_string());
            }
            "--mobile" => {
                mobile = true;
            }
            "--color-rgba" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --color-rgba")?;
                color = parse_color_rgba(value)?;
            }
            "--mod" => {
                i += 1;
                mods.push(args.get(i).ok_or("missing value for --mod")?.to_string());
            }
            "--server" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --server")?;
                server_addr = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|e| format!("invalid --server address: {e}"))?,
                );
            }
            "--discover-host" => {
                i += 1;
                discover_hosts.push(
                    args.get(i)
                        .ok_or("missing value for --discover-host")?
                        .to_string(),
                );
            }
            "--discover-port" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --discover-port")?;
                discover_port = value
                    .parse::<u16>()
                    .map_err(|e| format!("invalid --discover-port: {e}"))?;
            }
            "--discover-timeout-ms" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --discover-timeout-ms")?;
                discover_timeout_ms = parse_u64_arg("--discover-timeout-ms", value)?;
                if discover_timeout_ms == 0 {
                    return Err("invalid --discover-timeout-ms, expected integer >= 1".to_string());
                }
            }
            "--locale" => {
                i += 1;
                locale = args.get(i).ok_or("missing value for --locale")?.to_string();
            }
            "--duration-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --duration-ms")?;
                let ms = value
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --duration-ms: {e}"))?;
                duration = Duration::from_millis(ms);
            }
            "--tick-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --tick-ms")?;
                let ms = value
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --tick-ms: {e}"))?;
                tick = Duration::from_millis(ms);
            }
            "--max-recv-packets" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --max-recv-packets")?;
                max_recv_packets = value
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --max-recv-packets: {e}"))?;
            }
            "--aim-x" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --aim-x")?;
                aim_x = Some(parse_f32_arg("--aim-x", value)?);
            }
            "--aim-y" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --aim-y")?;
                aim_y = Some(parse_f32_arg("--aim-y", value)?);
            }
            "--mine-tile" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --mine-tile")?;
                snapshot_mining_tile = Some(parse_i32_pair_colon_arg("--mine-tile", value)?);
            }
            "--snapshot-boosting" => {
                snapshot_boosting = Some(true);
            }
            "--snapshot-no-boosting" => {
                snapshot_boosting = Some(false);
            }
            "--snapshot-shooting" => {
                snapshot_shooting = Some(true);
            }
            "--snapshot-no-shooting" => {
                snapshot_shooting = Some(false);
            }
            "--snapshot-chatting" => {
                snapshot_chatting = Some(true);
            }
            "--snapshot-no-chatting" => {
                snapshot_chatting = Some(false);
            }
            "--snapshot-building" => {
                snapshot_building = Some(true);
            }
            "--snapshot-no-building" => {
                snapshot_building = Some(false);
            }
            "--view-size" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --view-size")?;
                snapshot_view_size = Some(parse_f32_pair_colon_arg("--view-size", value)?);
            }
            "--snapshot-interval-ms" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --snapshot-interval-ms")?;
                snapshot_interval_ms = Some(parse_u64_arg("--snapshot-interval-ms", value)?);
            }
            "--move-step-x" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --move-step-x")?;
                move_step_x = Some(parse_f32_arg("--move-step-x", value)?);
            }
            "--move-step-y" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --move-step-y")?;
                move_step_y = Some(parse_f32_arg("--move-step-y", value)?);
            }
            "--intent-snapshot" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --intent-snapshot")?;
                enable_live_intent_runtime_capture = true;
                live_intent_snapshots.push(parse_intent_snapshot_arg(value)?);
            }
            "--intent-live-sampling" => {
                enable_live_intent_runtime_capture = true;
                live_intent_sampling_mode = IntentSamplingMode::LiveSampling;
            }
            "--intent-edge-mapped" => {
                enable_live_intent_runtime_capture = true;
                live_intent_sampling_mode = IntentSamplingMode::EdgeMapped;
            }
            "--intent-delay-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --intent-delay-ms")?;
                live_intent_delay_ms = parse_u64_arg("--intent-delay-ms", value)?;
            }
            "--intent-spacing-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --intent-spacing-ms")?;
                live_intent_spacing_ms = parse_u64_arg("--intent-spacing-ms", value)?;
            }
            "--command-mode-bind-group" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --command-mode-bind-group")?;
                let (index, unit_ids) = parse_command_mode_bind_group_arg(value)?;
                command_mode_ops.push(CommandModeCliOp::BindGroup { index, unit_ids });
            }
            "--command-mode-recall-group" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --command-mode-recall-group")?;
                let index = parse_u8_arg("--command-mode-recall-group", value)?;
                command_mode_ops.push(CommandModeCliOp::RecallGroup { index });
            }
            "--command-mode-clear-group" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --command-mode-clear-group")?;
                let index = parse_u8_arg("--command-mode-clear-group", value)?;
                command_mode_ops.push(CommandModeCliOp::ClearGroup { index });
            }
            "--command-mode-rect" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --command-mode-rect")?;
                command_mode_ops.push(CommandModeCliOp::SetRect(parse_command_mode_rect_arg(
                    value,
                )?));
            }
            "--chat-message" => {
                i += 1;
                chat_messages.push(
                    args.get(i)
                        .ok_or("missing value for --chat-message")?
                        .to_string(),
                );
            }
            "--plan-place" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --plan-place")?;
                build_plans.push(parse_plan_place_arg(value)?);
            }
            "--plan-break" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --plan-break")?;
                build_plans.push(parse_plan_break_arg(value)?);
            }
            "--plan-place-relative" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --plan-place-relative")?;
                relative_build_plans.push(parse_relative_plan_place_arg(value)?);
            }
            "--plan-break-relative" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --plan-break-relative")?;
                relative_build_plans.push(parse_relative_plan_break_arg(value)?);
            }
            "--plan-rotate" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --plan-rotate")?;
                plan_edit_ops.push(parse_plan_rotate_arg(value)?);
            }
            "--plan-flip-x" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --plan-flip-x")?;
                plan_edit_ops.push(parse_plan_flip_arg("--plan-flip-x", value, true)?);
            }
            "--plan-flip-y" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --plan-flip-y")?;
                plan_edit_ops.push(parse_plan_flip_arg("--plan-flip-y", value, false)?);
            }
            "--plan-edit-loop" => {
                plan_edit_loop = true;
            }
            "--plan-edit-delay-ms" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --plan-edit-delay-ms")?;
                plan_edit_delay_ms = parse_u64_arg("--plan-edit-delay-ms", value)?;
            }
            "--plan-edit-spacing-ms" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --plan-edit-spacing-ms")?;
                plan_edit_spacing_ms = parse_u64_arg("--plan-edit-spacing-ms", value)?;
            }
            "--plan-place-near-player" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --plan-place-near-player")?;
                auto_place_near_player
                    .push(parse_auto_place_arg("--plan-place-near-player", value)?);
            }
            "--plan-place-conflict-near-player" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --plan-place-conflict-near-player")?;
                auto_place_conflict_near_player.push(parse_auto_place_arg(
                    "--plan-place-conflict-near-player",
                    value,
                )?);
            }
            "--plan-break-near-player" => {
                auto_break_near_player = true;
            }
            "--render-ascii-on-world-ready" => {
                render_ascii_on_world_ready = true;
            }
            "--print-client-packets" => {
                print_client_packets = true;
            }
            "--watch-client-packet" => {
                i += 1;
                watched_client_packet_types.push(
                    args.get(i)
                        .ok_or("missing value for --watch-client-packet")?
                        .to_string(),
                );
            }
            "--watch-client-binary-packet" => {
                i += 1;
                watched_client_binary_packet_types.push(
                    args.get(i)
                        .ok_or("missing value for --watch-client-binary-packet")?
                        .to_string(),
                );
            }
            "--watch-client-logic-data" => {
                i += 1;
                watched_client_logic_data_channels.push(
                    args.get(i)
                        .ok_or("missing value for --watch-client-logic-data")?
                        .to_string(),
                );
            }
            "--consume-client-packet" => {
                i += 1;
                consumed_client_packet_types.push(
                    args.get(i)
                        .ok_or("missing value for --consume-client-packet")?
                        .to_string(),
                );
            }
            "--consume-client-binary-packet" => {
                i += 1;
                consumed_client_binary_packet_types.push(
                    args.get(i)
                        .ok_or("missing value for --consume-client-binary-packet")?
                        .to_string(),
                );
            }
            "--consume-client-logic-data" => {
                i += 1;
                consumed_client_logic_data_channels.push(
                    args.get(i)
                        .ok_or("missing value for --consume-client-logic-data")?
                        .to_string(),
                );
            }
            "--relay-client-packet" => {
                i += 1;
                relayed_client_packet_types.push(
                    args.get(i)
                        .ok_or("missing value for --relay-client-packet")?
                        .to_string(),
                );
            }
            "--relay-client-binary-packet" => {
                i += 1;
                relayed_client_binary_packet_types.push(
                    args.get(i)
                        .ok_or("missing value for --relay-client-binary-packet")?
                        .to_string(),
                );
            }
            "--relay-client-logic-data" => {
                i += 1;
                relayed_client_logic_data_channels.push(
                    args.get(i)
                        .ok_or("missing value for --relay-client-logic-data")?
                        .to_string(),
                );
            }
            "--render-window-live" => {
                render_window_live = true;
            }
            "--dump-world-stream-hex" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --dump-world-stream-hex")?;
                dump_world_stream_hex = Some(PathBuf::from(value));
            }
            "--chat-delay-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --chat-delay-ms")?;
                chat_delay_ms = parse_u64_arg("--chat-delay-ms", value)?;
            }
            "--chat-spacing-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --chat-spacing-ms")?;
                chat_spacing_ms = parse_u64_arg("--chat-spacing-ms", value)?;
            }
            "--action-delay-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --action-delay-ms")?;
                action_delay_ms = parse_u64_arg("--action-delay-ms", value)?;
            }
            "--action-spacing-ms" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --action-spacing-ms")?;
                action_spacing_ms = parse_u64_arg("--action-spacing-ms", value)?;
            }
            "--action-transfer-inventory" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-transfer-inventory")?;
                outbound_actions.push(OutboundAction::TransferInventory {
                    build_pos: parse_optional_build_pos_arg("--action-transfer-inventory", value)?,
                });
            }
            "--action-request-item" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-request-item")?;
                let (build_pos, item_id, amount) = parse_action_request_item_arg(value)?;
                outbound_actions.push(OutboundAction::RequestItem {
                    build_pos,
                    item_id,
                    amount,
                });
            }
            "--action-request-unit-payload" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-request-unit-payload")?;
                let target = parse_action_request_unit_payload_arg(value)?;
                outbound_actions.push(OutboundAction::RequestUnitPayload { target });
            }
            "--action-unit-clear" => {
                outbound_actions.push(OutboundAction::UnitClear);
            }
            "--action-unit-control" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-unit-control")?;
                let target = parse_action_request_unit_payload_arg(value)?;
                outbound_actions.push(OutboundAction::UnitControl { target });
            }
            "--action-unit-building-control-select" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-unit-building-control-select")?;
                let (target, build_pos) = parse_action_unit_building_control_select_arg(value)?;
                outbound_actions
                    .push(OutboundAction::UnitBuildingControlSelect { target, build_pos });
            }
            "--action-building-control-select" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-building-control-select")?;
                outbound_actions.push(OutboundAction::BuildingControlSelect {
                    build_pos: parse_optional_build_pos_arg(
                        "--action-building-control-select",
                        value,
                    )?,
                });
            }
            "--action-clear-items" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-clear-items")?;
                outbound_actions.push(OutboundAction::ClearItems {
                    build_pos: parse_optional_build_pos_arg("--action-clear-items", value)?,
                });
            }
            "--action-clear-liquids" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-clear-liquids")?;
                outbound_actions.push(OutboundAction::ClearLiquids {
                    build_pos: parse_optional_build_pos_arg("--action-clear-liquids", value)?,
                });
            }
            "--action-request-build-payload" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-request-build-payload")?;
                outbound_actions.push(OutboundAction::RequestBuildPayload {
                    build_pos: parse_optional_build_pos_arg(
                        "--action-request-build-payload",
                        value,
                    )?,
                });
            }
            "--action-request-drop-payload" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-request-drop-payload")?;
                let (x, y) = parse_f32_pair_colon_arg("--action-request-drop-payload", value)?;
                outbound_actions.push(OutboundAction::RequestDropPayload { x, y });
            }
            "--action-rotate-block" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-rotate-block")?;
                let (build_pos, direction) = parse_action_rotate_block_arg(value)?;
                outbound_actions.push(OutboundAction::RotateBlock {
                    build_pos,
                    direction,
                });
            }
            "--action-drop-item" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --action-drop-item")?;
                outbound_actions.push(OutboundAction::DropItem {
                    angle: parse_f32_arg("--action-drop-item", value)?,
                });
            }
            "--action-tile-config" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-tile-config")?;
                let (build_pos, value) = parse_action_tile_config_arg(value)?;
                outbound_actions.push(OutboundAction::TileConfig { build_pos, value });
            }
            "--action-tile-tap" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --action-tile-tap")?;
                outbound_actions.push(OutboundAction::TileTap {
                    tile_pos: parse_optional_build_pos_arg("--action-tile-tap", value)?,
                });
            }
            "--action-delete-plans" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-delete-plans")?;
                outbound_actions.push(OutboundAction::DeletePlans {
                    positions: parse_action_delete_plans_arg(value)?,
                });
            }
            "--action-command-building" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-command-building")?;
                let (buildings, x, y) = parse_action_command_building_arg(value)?;
                outbound_actions.push(OutboundAction::CommandBuilding { buildings, x, y });
            }
            "--action-command-units" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-command-units")?;
                outbound_actions.push(parse_action_command_units_outbound_action(value)?);
            }
            "--action-set-unit-command" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-set-unit-command")?;
                let (unit_ids, command_id) = parse_action_set_unit_command_arg(value)?;
                outbound_actions.push(OutboundAction::SetUnitCommand {
                    unit_ids,
                    command_id,
                });
            }
            "--action-set-unit-stance" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-set-unit-stance")?;
                let (unit_ids, stance_id, enable) = parse_action_set_unit_stance_arg(value)?;
                outbound_actions.push(OutboundAction::SetUnitStance {
                    unit_ids,
                    stance_id,
                    enable,
                });
            }
            "--action-begin-break" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-begin-break")?;
                let (builder, team_id, x, y) = parse_action_begin_break_arg(value)?;
                outbound_actions.push(OutboundAction::BeginBreak {
                    builder,
                    team_id,
                    x,
                    y,
                });
            }
            "--action-begin-place" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-begin-place")?;
                let (builder, block_id, team_id, x, y, rotation, place_config) =
                    parse_action_begin_place_arg(value)?;
                outbound_actions.push(OutboundAction::BeginPlace {
                    builder,
                    block_id,
                    team_id,
                    x,
                    y,
                    rotation,
                    place_config,
                });
            }
            "--action-menu-choose" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-menu-choose")?;
                let (menu_id, option) = parse_action_menu_choose_arg(value)?;
                outbound_actions.push(OutboundAction::MenuChoose { menu_id, option });
            }
            "--action-text-input-result" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-text-input-result")?;
                let (text_input_id, text) = parse_action_text_input_result_arg(value)?;
                outbound_actions.push(OutboundAction::TextInputResult {
                    text_input_id,
                    text,
                });
            }
            "--action-client-packet" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-client-packet")?;
                let (packet_type, contents, transport) = parse_action_client_packet_arg(value)?;
                outbound_actions.push(OutboundAction::ClientPacket {
                    packet_type,
                    contents,
                    transport,
                });
            }
            "--action-client-binary-packet" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-client-binary-packet")?;
                let (packet_type, contents, transport) =
                    parse_action_client_binary_packet_arg(value)?;
                outbound_actions.push(OutboundAction::ClientBinaryPacket {
                    packet_type,
                    contents,
                    transport,
                });
            }
            "--action-client-logic-data" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("missing value for --action-client-logic-data")?;
                let (channel, value, transport) = parse_action_client_logic_data_arg(value)?;
                outbound_actions.push(OutboundAction::ClientLogicData {
                    channel,
                    value,
                    transport,
                });
            }
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
        i += 1;
    }

    let snapshot_pointer = match (aim_x, aim_y) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => {
            return Err(
                "both --aim-x and --aim-y are required when overriding clientSnapshot pointer"
                    .to_string(),
            )
        }
    };
    let movement_probe =
        match (move_step_x, move_step_y) {
            (Some(x), Some(y)) => Some(MovementProbeConfig { step: (x, y) }),
            (None, None) => None,
            _ => return Err(
                "both --move-step-x and --move-step-y are required when enabling movement probe"
                    .to_string(),
            ),
        };

    if !plan_edit_ops.is_empty() {
        apply_plan_edits_to_build_plans(&mut build_plans, &plan_edit_ops);
        apply_plan_edits_to_relative_build_plans(&mut relative_build_plans, &plan_edit_ops);
    }
    if plan_edit_loop && plan_edit_spacing_ms == 0 {
        return Err(
            "--plan-edit-spacing-ms must be positive when --plan-edit-loop is enabled".to_string(),
        );
    }
    let runtime_plan_edit_loop = plan_edit_loop.then(|| PlanEditLoopConfig {
        ops: plan_edit_ops.clone(),
        delay_ms: plan_edit_delay_ms,
        spacing_ms: plan_edit_spacing_ms,
    });

    let connect = match connect_hex_path {
        Some(path) => ConnectSource::HexFile(path),
        None => {
            let mut spec = ConnectPacketSpec::new_default(locale.clone());
            spec.version = build;
            spec.version_type = version_type;
            spec.name = name;
            spec.locale = locale.clone();
            if let Some(value) = uuid {
                spec.uuid = value;
            }
            if let Some(value) = usid {
                spec.usid = value;
            }
            spec.mobile = mobile;
            spec.color = color;
            spec.mods = mods;
            ConnectSource::Generated(spec)
        }
    };

    let server_target = match server_addr {
        Some(addr) => ServerTarget::Direct(addr),
        None if !discover_hosts.is_empty() => ServerTarget::Discover(DiscoveryConfig {
            hosts: discover_hosts,
            port: discover_port,
            timeout: Duration::from_millis(discover_timeout_ms),
        }),
        None => return Err(format!("missing --server or --discover-host\n{}", usage())),
    };
    let runtime_custom_packet_semantics = build_runtime_custom_packet_semantic_specs(
        &consumed_client_packet_types,
        &consumed_client_binary_packet_types,
        &consumed_client_logic_data_channels,
    )?;
    let runtime_custom_packet_relays = build_runtime_custom_packet_relay_specs(
        &relayed_client_packet_types,
        &relayed_client_binary_packet_types,
        &relayed_client_logic_data_channels,
    )?;

    Ok(CliArgs {
        manifest_path: manifest_path.ok_or(format!("missing --manifest\n{}", usage()))?,
        server_target,
        locale: locale_for_session(&connect, &locale),
        duration,
        tick,
        max_recv_packets,
        snapshot_pointer,
        snapshot_mining_tile,
        snapshot_boosting,
        snapshot_shooting,
        snapshot_chatting,
        snapshot_building,
        snapshot_view_size,
        snapshot_interval_ms,
        movement_probe,
        enable_live_intent_runtime_capture,
        live_intent_sampling_mode,
        live_intent_schedule: build_intent_schedule(
            live_intent_snapshots,
            live_intent_delay_ms,
            live_intent_spacing_ms,
        ),
        command_mode_ops,
        runtime_plan_edit_loop,
        build_plans,
        relative_build_plans,
        auto_break_near_player,
        auto_place_near_player,
        auto_place_conflict_near_player,
        render_ascii_on_world_ready,
        print_client_packets,
        watched_client_packet_types,
        watched_client_binary_packet_types,
        watched_client_logic_data_channels,
        runtime_custom_packet_semantics,
        runtime_custom_packet_relays,
        render_window_live,
        dump_world_stream_hex,
        chat_schedule: build_chat_schedule(chat_messages, chat_delay_ms, chat_spacing_ms),
        outbound_action_schedule: build_outbound_action_schedule(
            outbound_actions,
            action_delay_ms,
            action_spacing_ms,
        ),
        connect,
    })
}

fn usage() -> String {
    String::from(
        "Usage: mdt-client-min-online --manifest <path> (--server <host:port> | --discover-host <host> [--discover-host <host> ...] [--discover-port <port>] [--discover-timeout-ms <ms>]) [--connect-hex <path> | --name <name> --uuid <base64> --usid <base64> --build <build> --version-type <type> --mobile --color-rgba <rgba> --mod <name:version> ...] [--locale <locale>] [--duration-ms <ms>] [--tick-ms <ms>] [--max-recv-packets <n>] [--snapshot-interval-ms <ms>] [--aim-x <f32> --aim-y <f32>] [--mine-tile <x:y>] [--snapshot-boosting|--snapshot-no-boosting] [--snapshot-shooting|--snapshot-no-shooting] [--snapshot-chatting|--snapshot-no-chatting] [--snapshot-building|--snapshot-no-building] [--view-size <w:h>] [--move-step-x <f32> --move-step-y <f32>] [--intent-snapshot <moveX:moveY:aimX:aimY:actions[:mineX,mineY|none]> ...] [--intent-live-sampling|--intent-edge-mapped] [--intent-delay-ms <ms>] [--intent-spacing-ms <ms>] [--command-mode-bind-group <index@unitId[,unitId...]> ...] [--command-mode-recall-group <index> ...] [--command-mode-clear-group <index> ...] [--command-mode-rect <x0:y0:x1:y1|none> ...] [--plan-place <x:y:block[:rotation][;config]> ...] [--plan-break <x:y> ...] [--plan-place-relative <dx:dy:block[:rotation][;config]> ...] [--plan-break-relative <dx:dy> ...] config=<none|int=<i32>|long=<i64>|float=<f32>|bool=<true|false|1|0>|int-seq=<i32[,i32...]>|point2=<x:y>|point2-array=<x:y[,x:y...]>|string=<text>|content=<contentType:contentId>|tech-node-raw=<contentType:contentId>|double=<f64>|building-pos=<i32>|laccess=<i16>|bytes=<hex>|legacy-unit-command-null=<u8>|bool-array=<bool[,bool...]>|unit-id=<i32>|vec2-array=<x:y[,x:y...]>|vec2=<x:y>|team=<u8>|int-array=<i32[,i32...]>|object-array=<value[|value...]>|unit-command=<u16>> [--plan-rotate <x:y:dir> ...] [--plan-flip-x <x:y> ...] [--plan-flip-y <x:y> ...] [--plan-edit-loop] [--plan-edit-delay-ms <ms>] [--plan-edit-spacing-ms <ms>] [--plan-break-near-player] [--plan-place-near-player <block[:rotation][;config]|selected[:rotation][;config]> ...] [--plan-place-conflict-near-player <block[:rotation][;config]|selected[:rotation][;config]> ...] [--render-ascii-on-world-ready] [--print-client-packets] [--watch-client-packet <type> ...] [--watch-client-binary-packet <type> ...] [--watch-client-logic-data <channel> ...] [--consume-client-packet <type@semantic> ...] [--consume-client-binary-packet <type@semantic> ...] [--consume-client-logic-data <channel@semantic> ...] [--relay-client-packet <inbound@outbound@reliable|unreliable> ...] [--relay-client-binary-packet <inbound@outbound@reliable|unreliable> ...] [--relay-client-logic-data <inbound@outbound@reliable|unreliable> ...] semantic=<server-message|chat-message|hud-text|announce|clipboard|open-uri|world-pos|build-pos|unit-id|team|bool|number> [--render-window-live] [--dump-world-stream-hex <path>] [--chat-delay-ms <ms>] [--chat-spacing-ms <ms>] [--chat-message <text> ...] [--action-delay-ms <ms>] [--action-spacing-ms <ms>] [--action-request-item <buildPos|none:itemId|none:amount> ...] [--action-request-unit-payload <none|unit:<id>|block:<pos>|<id>> ...] [--action-unit-clear ...] [--action-unit-control <none|unit:<id>|block:<pos>|<id>> ...] [--action-unit-building-control-select <none|unit:<id>|block:<pos>|<id>@buildPos|none> ...] [--action-building-control-select <buildPos|none> ...] [--action-clear-items <buildPos|none> ...] [--action-clear-liquids <buildPos|none> ...] [--action-transfer-inventory <buildPos|none> ...] [--action-request-build-payload <buildPos|none> ...] [--action-request-drop-payload <x:y> ...] [--action-rotate-block <buildPos|none:direction> ...] [--action-drop-item <angle> ...] [--action-tile-config <buildPos|none:value> ...] [--action-tile-tap <tilePos|none> ...] [--action-delete-plans <x:y[,x:y...]|none> ...] [--action-command-building <x:y[,x:y...]|none@x:y> ...] [--action-command-units <unitId[,unitId...]|none@buildPos@unitTarget@x:y@queueCommand[@finalBatch]> ...] [--action-set-unit-command <unitId[,unitId...]|none@commandId|none> ...] [--action-set-unit-stance <unitId[,unitId...]|none@stanceId|none@enable> ...] [--action-begin-break <none|unit:<id>|block:<pos>|<id>@teamId@x:y> ...] [--action-begin-place <none|unit:<id>|block:<pos>|<id>@blockId|none@teamId@x:y@rotation@value> ...] [--action-menu-choose <menuId@option> ...] [--action-text-input-result <textInputId@text|none> ...] [--action-client-packet <type@contents@reliable|unreliable> ...] [--action-client-binary-packet <type@hex@reliable|unreliable> ...] [--action-client-logic-data <channel@value@reliable|unreliable> ...] value=<null|int=<i32>|long=<i64>|float=<f32>|bool=<true|false|1|0>|int-seq=<i32[,i32...]>|string=<text>|content=<contentType:contentId>|tech-node-raw=<contentType:contentId>|point2=<x:y>|point2-array=<x:y[,x:y...]>|double=<f64>|building-pos=<i32>|laccess=<i16>|vec2=<x:y>|vec2-array=<x:y[,x:y...]>|team=<u8>|bytes=<hex>|legacy-unit-command-null=<u8>|bool-array=<bool[,bool...]>|unit-id=<i32>|int-array=<i32[,i32...]>|object-array=<value>|unit-command=<u16>|...>",
    )
}

fn apply_runtime_command_mode_cli_ops(
    runtime_command_mode: &mut CommandModeState,
    ops: &[CommandModeCliOp],
) {
    for op in ops {
        match op {
            CommandModeCliOp::BindGroup { index, unit_ids } => {
                runtime_command_mode.bind_control_group(*index, unit_ids);
            }
            CommandModeCliOp::RecallGroup { index } => {
                runtime_command_mode.recall_control_group(*index);
            }
            CommandModeCliOp::ClearGroup { index } => {
                runtime_command_mode.clear_control_group(*index);
            }
            CommandModeCliOp::SetRect(rect) => {
                runtime_command_mode.set_command_rect(*rect);
            }
        }
    }
}

fn resolve_session_timing(args: &CliArgs) -> ClientSessionTiming {
    let mut timing = ClientSessionTiming::default();
    if let Some(snapshot_interval_ms) = args.snapshot_interval_ms {
        timing.client_snapshot_interval_ms = snapshot_interval_ms;
    }
    timing
}

fn locale_for_session(source: &ConnectSource, fallback: &str) -> String {
    match source {
        ConnectSource::HexFile(_) => fallback.to_string(),
        ConnectSource::Generated(spec) => spec.locale.clone(),
    }
}

fn load_connect_payload(source: &ConnectSource) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match source {
        ConnectSource::HexFile(path) => load_hex_file(path),
        ConnectSource::Generated(spec) => {
            for ConnectCompatibilityWarning { code, message } in spec.compatibility_warnings() {
                eprintln!(
                    "connect_spec_warning: code={} message={}",
                    code.as_str(),
                    message
                );
            }
            println!(
                "connect_spec: build={} version_type={} name={} locale={} uuid={} usid={} server_uuid={} mobile={} color=0x{:08x} mods={:?}",
                spec.version,
                spec.version_type,
                spec.name,
                spec.locale,
                spec.uuid,
                spec.usid,
                spec.server_observed_uuid()?,
                spec.mobile,
                spec.color as u32,
                spec.mods
            );
            Ok(spec.encode_payload()?)
        }
    }
}

fn load_hex_file(path: &PathBuf) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    decode_hex_text(&text).map_err(|e| e.into())
}

fn maybe_dump_world_stream_hex(
    session: &ClientSession,
    args: &CliArgs,
    world_stream_dumped: &mut bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if *world_stream_dumped {
        return Ok(());
    }
    let Some(path) = &args.dump_world_stream_hex else {
        return Ok(());
    };
    let Some(bundle) = session.loaded_world_bundle() else {
        return Ok(());
    };

    fs::write(path, encode_hex_text(&bundle.compressed))?;
    println!(
        "dumped_world_stream_hex: path={} bytes={}",
        path.display(),
        bundle.compressed.len()
    );
    *world_stream_dumped = true;
    Ok(())
}

fn decode_hex_text(text: &str) -> Result<Vec<u8>, String> {
    let cleaned = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    if cleaned.len() % 2 != 0 {
        return Err("hex payload length must be even".into());
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|e| format!("invalid hex at byte {}: {e}", i / 2))
        })
        .collect()
}

fn encode_hex_text(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2 + bytes.len() / 32);
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && index % 32 == 0 {
            out.push('\n');
        }
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn parse_color_rgba(value: &str) -> Result<i32, String> {
    let parsed = if let Some(stripped) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(stripped, 16).map_err(|e| format!("invalid --color-rgba: {e}"))?
    } else {
        value
            .parse::<u32>()
            .map_err(|e| format!("invalid --color-rgba: {e}"))?
    };
    Ok(parsed as i32)
}

fn parse_f32_arg(flag: &str, value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_f64_arg(flag: &str, value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_u64_arg(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_i64_arg(flag: &str, value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_i32_arg(flag: &str, value: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_i32_pair_colon_arg(flag: &str, value: &str) -> Result<(i32, i32), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(format!("invalid {flag}, expected <x:y>"));
    }
    let x = parse_i32_arg(&format!("{flag} x"), parts[0])?;
    let y = parse_i32_arg(&format!("{flag} y"), parts[1])?;
    Ok((x, y))
}

fn parse_command_mode_bind_group_arg(value: &str) -> Result<(u8, Vec<i32>), String> {
    let Some((index, units)) = value.split_once('@') else {
        return Err(
            "invalid --command-mode-bind-group, expected <index@unitId[,unitId...]>".to_string(),
        );
    };
    if units.is_empty() {
        return Err(
            "invalid --command-mode-bind-group, expected <index@unitId[,unitId...]>".to_string(),
        );
    }

    let index = parse_u8_arg("--command-mode-bind-group index", index)?;
    let mut unit_ids = Vec::new();
    for (unit_index, unit_id) in units.split(',').enumerate() {
        if unit_id.is_empty() {
            return Err(format!(
                "invalid --command-mode-bind-group, empty unit id at index {unit_index}"
            ));
        }
        unit_ids.push(parse_i32_arg(
            &format!("--command-mode-bind-group unitId[{unit_index}]"),
            unit_id,
        )?);
    }
    Ok((index, unit_ids))
}

fn parse_command_mode_rect_arg(value: &str) -> Result<Option<CommandModeRectProjection>, String> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err("invalid --command-mode-rect, expected <x0:y0:x1:y1|none>".to_string());
    }
    Ok(Some(CommandModeRectProjection {
        x0: parse_i32_arg("--command-mode-rect x0", parts[0])?,
        y0: parse_i32_arg("--command-mode-rect y0", parts[1])?,
        x1: parse_i32_arg("--command-mode-rect x1", parts[2])?,
        y1: parse_i32_arg("--command-mode-rect y1", parts[3])?,
    }))
}

fn parse_f32_pair_colon_arg(flag: &str, value: &str) -> Result<(f32, f32), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(format!("invalid {flag}, expected <x:y>"));
    }
    let x = parse_f32_arg(&format!("{flag} x"), parts[0])?;
    let y = parse_f32_arg(&format!("{flag} y"), parts[1])?;
    Ok((x, y))
}

fn parse_intent_snapshot_arg(value: &str) -> Result<InputSnapshot, String> {
    let parts = value.splitn(6, ':').collect::<Vec<_>>();
    if !(parts.len() == 5 || parts.len() == 6) {
        return Err(
            "invalid --intent-snapshot, expected <moveX:moveY:aimX:aimY:actions[:mineX,mineY|none]>"
                .to_string(),
        );
    }

    let move_axis = (
        parse_f32_arg("--intent-snapshot moveX", parts[0])?,
        parse_f32_arg("--intent-snapshot moveY", parts[1])?,
    );
    let aim_axis = (
        parse_f32_arg("--intent-snapshot aimX", parts[2])?,
        parse_f32_arg("--intent-snapshot aimY", parts[3])?,
    );
    let actions_raw = parts[4].trim();
    let active_actions = if actions_raw.is_empty() || actions_raw.eq_ignore_ascii_case("none") {
        Vec::new()
    } else {
        actions_raw
            .split(',')
            .map(parse_binary_action_arg)
            .collect::<Result<Vec<_>, _>>()?
    };
    let mining_tile = if let Some(raw) = parts.get(5) {
        parse_optional_mining_tile_arg("--intent-snapshot mine", raw.trim())?
    } else {
        None
    };

    Ok(InputSnapshot {
        move_axis,
        aim_axis,
        mining_tile,
        building: false,
        active_actions,
    })
}

fn parse_binary_action_arg(value: &str) -> Result<BinaryAction, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "up" | "moveup" | "move-up" => Ok(BinaryAction::MoveUp),
        "down" | "movedown" | "move-down" => Ok(BinaryAction::MoveDown),
        "left" | "moveleft" | "move-left" => Ok(BinaryAction::MoveLeft),
        "right" | "moveright" | "move-right" => Ok(BinaryAction::MoveRight),
        "fire" => Ok(BinaryAction::Fire),
        "boost" => Ok(BinaryAction::Boost),
        "chat" | "pause" => Ok(BinaryAction::Chat),
        "interact" | "use" => Ok(BinaryAction::Interact),
        other => Err(format!(
            "invalid --intent-snapshot action '{other}', expected one of up,down,left,right,fire,boost,chat,interact"
        )),
    }
}

fn parse_optional_mining_tile_arg(flag: &str, value: &str) -> Result<Option<(i32, i32)>, String> {
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let Some((x, y)) = value.split_once(',') else {
        return Err(format!("invalid {flag}, expected <x,y> or none"));
    };
    Ok(Some((
        parse_i32_arg(&format!("{flag} x"), x)?,
        parse_i32_arg(&format!("{flag} y"), y)?,
    )))
}

fn parse_i16_like_arg(flag: &str, value: &str) -> Result<i16, String> {
    let parsed = if let Some(stripped) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        i32::from_str_radix(stripped, 16).map_err(|e| format!("invalid {flag}: {e}"))?
    } else {
        value
            .parse::<i32>()
            .map_err(|e| format!("invalid {flag}: {e}"))?
    };
    i16::try_from(parsed).map_err(|_| format!("{flag} out of i16 range: {parsed}"))
}

fn parse_plan_place_arg(value: &str) -> Result<ClientBuildPlan, String> {
    let (plan_payload, config_payload) = if let Some((prefix, config)) = value.split_once(';') {
        (prefix, Some(config))
    } else {
        (value, None)
    };

    let parts = plan_payload.split(':').collect::<Vec<_>>();
    if !(parts.len() == 3 || parts.len() == 4) {
        return Err("invalid --plan-place, expected <x:y:block[:rotation][;config]>".to_string());
    }

    let x = parse_i32_arg("--plan-place x", parts[0])?;
    let y = parse_i32_arg("--plan-place y", parts[1])?;
    let block_id = parse_i16_like_arg("--plan-place block", parts[2])?;
    let rotation = if parts.len() == 4 {
        let value = parse_i32_arg("--plan-place rotation", parts[3])?;
        u8::try_from(value)
            .map_err(|_| format!("--plan-place rotation out of u8 range: {value}"))?
    } else {
        0
    };
    let config = if let Some(config_payload) = config_payload {
        parse_plan_place_config_arg("--plan-place config", config_payload)?
    } else {
        ClientBuildPlanConfig::None
    };

    Ok(ClientBuildPlan {
        tile: (x, y),
        breaking: false,
        block_id: Some(block_id),
        rotation,
        config,
    })
}

fn parse_plan_break_arg(value: &str) -> Result<ClientBuildPlan, String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("invalid --plan-break, expected <x:y>".to_string());
    }

    Ok(ClientBuildPlan {
        tile: (
            parse_i32_arg("--plan-break x", parts[0])?,
            parse_i32_arg("--plan-break y", parts[1])?,
        ),
        breaking: true,
        block_id: None,
        rotation: 0,
        config: ClientBuildPlanConfig::None,
    })
}

fn parse_relative_plan_place_arg(value: &str) -> Result<RelativeBuildPlan, String> {
    let place = parse_plan_place_arg(value)?;
    Ok(RelativeBuildPlan {
        tile_offset: place.tile,
        breaking: false,
        block_id: place.block_id,
        rotation: place.rotation,
        config: place.config,
    })
}

fn parse_relative_plan_break_arg(value: &str) -> Result<RelativeBuildPlan, String> {
    let plan = parse_plan_break_arg(value)?;
    Ok(RelativeBuildPlan {
        tile_offset: plan.tile,
        breaking: true,
        block_id: None,
        rotation: 0,
        config: ClientBuildPlanConfig::None,
    })
}

fn parse_plan_place_config_arg(flag: &str, value: &str) -> Result<ClientBuildPlanConfig, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(ClientBuildPlanConfig::None);
    }
    let object = parse_typeio_object_subset_arg(flag, value)?;
    build_plan_config_from_typeio_object(flag, object)
}

fn parse_content_config_arg(flag: &str, value: &str) -> Result<(u8, i16), String> {
    let Some((content_type, content_id)) = value.split_once(':') else {
        return Err(format!(
            "invalid {flag}, expected content=<contentType:contentId>"
        ));
    };
    Ok((
        parse_u8_arg(&format!("{flag} contentType"), content_type)?,
        parse_i16_like_arg(&format!("{flag} contentId"), content_id)?,
    ))
}

fn parse_point2_array_config_arg(flag: &str, value: &str) -> Result<Vec<(i32, i32)>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }

    let mut points = Vec::new();
    for (index, entry) in value.split(',').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid {flag}, empty point2-array item at index {index}"
            ));
        }
        points.push(parse_i32_pair_colon_arg(
            &format!("{flag} point2-array[{index}]"),
            entry,
        )?);
    }
    Ok(points)
}

fn unpack_plan_point2(value: i32) -> (i32, i32) {
    let raw = value as u32;
    let x = ((raw >> 16) as u16) as i16;
    let y = (raw as u16) as i16;
    (i32::from(x), i32::from(y))
}

fn build_plan_config_from_typeio_object(
    flag: &str,
    object: TypeIoObject,
) -> Result<ClientBuildPlanConfig, String> {
    match object {
        TypeIoObject::Null => Ok(ClientBuildPlanConfig::None),
        TypeIoObject::Int(value) => Ok(ClientBuildPlanConfig::Int(value)),
        TypeIoObject::Long(value) => Ok(ClientBuildPlanConfig::Long(value)),
        TypeIoObject::Float(value) => Ok(ClientBuildPlanConfig::FloatBits(value.to_bits())),
        TypeIoObject::String(Some(value)) => Ok(ClientBuildPlanConfig::String(value)),
        TypeIoObject::String(None) => Ok(ClientBuildPlanConfig::None),
        TypeIoObject::ContentRaw {
            content_type,
            content_id,
        } => Ok(ClientBuildPlanConfig::Content {
            content_type,
            content_id,
        }),
        TypeIoObject::IntSeq(values) => Ok(ClientBuildPlanConfig::IntSeq(values)),
        TypeIoObject::Point2 { x, y } => Ok(ClientBuildPlanConfig::Point2 { x, y }),
        TypeIoObject::PackedPoint2Array(values) => Ok(ClientBuildPlanConfig::Point2Array(
            values
                .into_iter()
                .map(unpack_plan_point2)
                .collect::<Vec<_>>(),
        )),
        TypeIoObject::TechNodeRaw {
            content_type,
            content_id,
        } => Ok(ClientBuildPlanConfig::TechNodeRaw {
            content_type,
            content_id,
        }),
        TypeIoObject::Bool(value) => Ok(ClientBuildPlanConfig::Bool(value)),
        TypeIoObject::Double(value) => Ok(ClientBuildPlanConfig::DoubleBits(value.to_bits())),
        TypeIoObject::BuildingPos(value) => Ok(ClientBuildPlanConfig::BuildingPos(value)),
        TypeIoObject::LAccess(value) => Ok(ClientBuildPlanConfig::LAccess(value)),
        TypeIoObject::Bytes(values) => Ok(ClientBuildPlanConfig::Bytes(values)),
        TypeIoObject::LegacyUnitCommandNull(value) => {
            Ok(ClientBuildPlanConfig::LegacyUnitCommandNull(value))
        }
        TypeIoObject::BoolArray(values) => Ok(ClientBuildPlanConfig::BoolArray(values)),
        TypeIoObject::UnitId(value) => Ok(ClientBuildPlanConfig::UnitId(value)),
        TypeIoObject::Vec2Array(values) => Ok(ClientBuildPlanConfig::Vec2Array(
            values
                .into_iter()
                .map(|(x, y)| (x.to_bits(), y.to_bits()))
                .collect::<Vec<_>>(),
        )),
        TypeIoObject::Vec2 { x, y } => Ok(ClientBuildPlanConfig::Vec2 {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        }),
        TypeIoObject::Team(value) => Ok(ClientBuildPlanConfig::Team(value)),
        TypeIoObject::IntArray(values) => Ok(ClientBuildPlanConfig::IntArray(values)),
        TypeIoObject::ObjectArray(values) => {
            let mut out = Vec::with_capacity(values.len());
            for (index, value) in values.into_iter().enumerate() {
                out.push(build_plan_config_from_typeio_object(
                    &format!("{flag} object-array[{index}]"),
                    value,
                )?);
            }
            Ok(ClientBuildPlanConfig::ObjectArray(out))
        }
        TypeIoObject::UnitCommand(value) => Ok(ClientBuildPlanConfig::UnitCommand(value)),
    }
}

fn parse_plan_rotate_arg(value: &str) -> Result<PlanEditOp, String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err("invalid --plan-rotate, expected <x:y:direction>".to_string());
    }
    let origin = (
        parse_i32_arg("--plan-rotate x", parts[0])?,
        parse_i32_arg("--plan-rotate y", parts[1])?,
    );
    let direction = parse_i32_arg("--plan-rotate direction", parts[2])?;
    if direction == 0 {
        return Err("--plan-rotate direction must be non-zero".to_string());
    }
    Ok(PlanEditOp::Rotate { origin, direction })
}

fn parse_plan_flip_arg(flag: &str, value: &str, flip_x: bool) -> Result<PlanEditOp, String> {
    let origin = parse_i32_pair_colon_arg(flag, value)?;
    Ok(PlanEditOp::Flip { origin, flip_x })
}

fn parse_optional_build_pos_arg(flag: &str, value: &str) -> Result<Option<i32>, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    Ok(Some(parse_i32_arg(flag, value)?))
}

fn parse_optional_i16_like_arg(flag: &str, value: &str) -> Result<Option<i16>, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    Ok(Some(parse_i16_like_arg(flag, value)?))
}

fn parse_optional_f32_pair_colon_arg(
    flag: &str,
    value: &str,
) -> Result<Option<(f32, f32)>, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    Ok(Some(parse_f32_pair_colon_arg(flag, value)?))
}

fn parse_bool_arg(flag: &str, value: &str) -> Result<bool, String> {
    if value.eq_ignore_ascii_case("true") || value == "1" {
        return Ok(true);
    }
    if value.eq_ignore_ascii_case("false") || value == "0" {
        return Ok(false);
    }
    Err(format!("invalid {flag}, expected <true|false|1|0>"))
}

fn parse_client_packet_transport_arg(
    flag: &str,
    value: &str,
) -> Result<ClientPacketTransport, String> {
    if value.eq_ignore_ascii_case("reliable") || value.eq_ignore_ascii_case("tcp") {
        return Ok(ClientPacketTransport::Tcp);
    }
    if value.eq_ignore_ascii_case("unreliable") || value.eq_ignore_ascii_case("udp") {
        return Ok(ClientPacketTransport::Udp);
    }
    Err(format!(
        "invalid {flag}, expected <reliable|unreliable|tcp|udp>"
    ))
}

fn parse_client_logic_data_transport_arg(
    flag: &str,
    value: &str,
) -> Result<ClientLogicDataTransport, String> {
    if value.eq_ignore_ascii_case("reliable") || value.eq_ignore_ascii_case("tcp") {
        return Ok(ClientLogicDataTransport::Reliable);
    }
    if value.eq_ignore_ascii_case("unreliable") || value.eq_ignore_ascii_case("udp") {
        return Ok(ClientLogicDataTransport::Unreliable);
    }
    Err(format!(
        "invalid {flag}, expected <reliable|unreliable|tcp|udp>"
    ))
}

fn parse_action_client_packet_arg(
    value: &str,
) -> Result<(String, String, ClientPacketTransport), String> {
    let parts = value.splitn(3, '@').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(
            "invalid --action-client-packet, expected <type@contents@reliable|unreliable>"
                .to_string(),
        );
    }
    Ok((
        parts[0].to_string(),
        parts[1].to_string(),
        parse_client_packet_transport_arg("--action-client-packet transport", parts[2])?,
    ))
}

fn parse_action_client_binary_packet_arg(
    value: &str,
) -> Result<(String, Vec<u8>, ClientPacketTransport), String> {
    let parts = value.splitn(3, '@').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(
            "invalid --action-client-binary-packet, expected <type@hex@reliable|unreliable>"
                .to_string(),
        );
    }
    Ok((
        parts[0].to_string(),
        decode_hex_text(parts[1])?,
        parse_client_packet_transport_arg("--action-client-binary-packet transport", parts[2])?,
    ))
}

fn parse_action_client_logic_data_arg(
    value: &str,
) -> Result<(String, TypeIoObject, ClientLogicDataTransport), String> {
    let parts = value.splitn(3, '@').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(
            "invalid --action-client-logic-data, expected <channel@value@reliable|unreliable>"
                .to_string(),
        );
    }
    Ok((
        parts[0].to_string(),
        parse_typeio_object_subset_arg("--action-client-logic-data value", parts[1])?,
        parse_client_logic_data_transport_arg("--action-client-logic-data transport", parts[2])?,
    ))
}

fn parse_action_rotate_block_arg(value: &str) -> Result<(Option<i32>, bool), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(
            "invalid --action-rotate-block, expected <buildPos|none:direction>".to_string(),
        );
    }
    let build_pos = parse_optional_build_pos_arg("--action-rotate-block buildPos", parts[0])?;
    let direction = parse_bool_arg("--action-rotate-block direction", parts[1])?;
    Ok((build_pos, direction))
}

fn parse_action_request_item_arg(value: &str) -> Result<(Option<i32>, Option<i16>, i32), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(
            "invalid --action-request-item, expected <buildPos|none:itemId|none:amount>"
                .to_string(),
        );
    }
    let build_pos = parse_optional_build_pos_arg("--action-request-item buildPos", parts[0])?;
    let item_id = parse_optional_i16_like_arg("--action-request-item itemId", parts[1])?;
    let amount = parse_i32_arg("--action-request-item amount", parts[2])?;
    Ok((build_pos, item_id, amount))
}

fn parse_action_request_unit_payload_arg(value: &str) -> Result<ClientUnitRef, String> {
    parse_action_unit_ref_arg(
        "--action-request-unit-payload",
        "<none|unit:<id>|block:<pos>|<id>>",
        value,
    )
}

fn parse_action_unit_ref_arg(
    flag: &str,
    expected: &str,
    value: &str,
) -> Result<ClientUnitRef, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(ClientUnitRef::None);
    }

    let parts = value.splitn(2, ':').collect::<Vec<_>>();
    if parts.len() == 2 {
        let kind = parts[0];
        let raw = parts[1];
        if kind.eq_ignore_ascii_case("block") {
            return Ok(ClientUnitRef::Block(parse_i32_arg(
                &format!("{flag} blockPos"),
                raw,
            )?));
        }
        if kind.eq_ignore_ascii_case("unit") || kind.eq_ignore_ascii_case("standard") {
            return Ok(ClientUnitRef::Standard(parse_i32_arg(
                &format!("{flag} unitId"),
                raw,
            )?));
        }
        return Err(format!("invalid {flag}, expected {expected}"));
    }

    Ok(ClientUnitRef::Standard(parse_i32_arg(
        &format!("{flag} unitId"),
        value,
    )?))
}

fn parse_action_tile_config_arg(value: &str) -> Result<(Option<i32>, TypeIoObject), String> {
    let parts = value.splitn(2, ':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("invalid --action-tile-config, expected <buildPos|none:value>".to_string());
    }
    let build_pos = parse_optional_build_pos_arg("--action-tile-config buildPos", parts[0])?;
    let value = parse_typeio_object_subset_arg("--action-tile-config value", parts[1])?;
    Ok((build_pos, value))
}

fn parse_action_delete_plans_arg(value: &str) -> Result<Vec<i32>, String> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut positions = Vec::new();
    for (index, entry) in value.split(',').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid --action-delete-plans, empty tile at index {index}"
            ));
        }
        let (x, y) =
            parse_i32_pair_colon_arg(&format!("--action-delete-plans position[{index}]"), entry)?;
        positions.push(pack_point2(x, y));
    }
    Ok(positions)
}

fn parse_action_unit_building_control_select_arg(
    value: &str,
) -> Result<(ClientUnitRef, Option<i32>), String> {
    let Some((target, build_pos)) = value.split_once('@') else {
        return Err(
            "invalid --action-unit-building-control-select, expected <none|unit:<id>|block:<pos>|<id>@buildPos|none>"
                .to_string(),
        );
    };
    Ok((
        parse_action_request_unit_payload_arg(target)?,
        parse_optional_build_pos_arg("--action-unit-building-control-select buildPos", build_pos)?,
    ))
}

fn parse_action_command_building_arg(value: &str) -> Result<(Vec<i32>, f32, f32), String> {
    let Some((buildings, target)) = value.split_once('@') else {
        return Err(
            "invalid --action-command-building, expected <x:y[,x:y...]|none@x:y>".to_string(),
        );
    };
    let buildings = parse_action_delete_plans_arg(buildings)?;
    let (x, y) = parse_f32_pair_colon_arg("--action-command-building target", target)?;
    Ok((buildings, x, y))
}

fn parse_action_command_units_arg(
    value: &str,
) -> Result<
    (
        Vec<i32>,
        Option<i32>,
        ClientUnitRef,
        Option<(f32, f32)>,
        bool,
        bool,
    ),
    String,
> {
    let parts = value.split('@').collect::<Vec<_>>();
    if parts.len() != 6 {
        return Err("invalid --action-command-units, expected <unitId[,unitId...]|none@buildPos|none@unitTarget@x:y|none@queueCommand@finalBatch>".to_string());
    }
    let unit_ids = parse_action_unit_ids_arg(parts[0])?;
    let build_target =
        parse_optional_build_pos_arg("--action-command-units buildTarget", parts[1])?;
    let unit_target = parse_action_unit_ref_arg(
        "--action-command-units unitTarget",
        "<none|unit:<id>|block:<pos>|<id>>",
        parts[2],
    )?;
    let pos_target =
        parse_optional_f32_pair_colon_arg("--action-command-units posTarget", parts[3])?;
    let queue_command = parse_bool_arg("--action-command-units queueCommand", parts[4])?;
    let final_batch = parse_bool_arg("--action-command-units finalBatch", parts[5])?;
    Ok((
        unit_ids,
        build_target,
        unit_target,
        pos_target,
        queue_command,
        final_batch,
    ))
}

fn parse_action_command_units_outbound_action(value: &str) -> Result<OutboundAction, String> {
    let parts = value.split('@').collect::<Vec<_>>();
    match parts.len() {
        5 => {
            let unit_ids = parse_action_unit_ids_arg(parts[0])?;
            let build_target =
                parse_optional_build_pos_arg("--action-command-units buildTarget", parts[1])?;
            let unit_target = parse_action_unit_ref_arg(
                "--action-command-units unitTarget",
                "<none|unit:<id>|block:<pos>|<id>>",
                parts[2],
            )?;
            let pos_target =
                parse_optional_f32_pair_colon_arg("--action-command-units posTarget", parts[3])?;
            let queue_command =
                parse_bool_arg("--action-command-units queueCommand", parts[4])?;
            Ok(OutboundAction::CommandUnitsChunked {
                unit_ids,
                build_target,
                unit_target,
                pos_target,
                queue_command,
            })
        }
        6 => {
            let (unit_ids, build_target, unit_target, pos_target, queue_command, final_batch) =
                parse_action_command_units_arg(value)?;
            Ok(OutboundAction::CommandUnits {
                unit_ids,
                build_target,
                unit_target,
                pos_target,
                queue_command,
                final_batch,
            })
        }
        _ => Err("invalid --action-command-units, expected <unitId[,unitId...]|none@buildPos@unitTarget@x:y@queueCommand[@finalBatch]>".to_string()),
    }
}

fn parse_optional_u8_token(flag: &str, value: &str) -> Result<Option<u8>, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    Ok(Some(parse_u8_arg(flag, value)?))
}

fn parse_action_set_unit_command_arg(value: &str) -> Result<(Vec<i32>, Option<u8>), String> {
    let Some((unit_ids, command_id)) = value.split_once('@') else {
        return Err(
            "invalid --action-set-unit-command, expected <unitId[,unitId...]|none@commandId|none>"
                .to_string(),
        );
    };
    Ok((
        parse_action_unit_ids_arg(unit_ids)?,
        parse_optional_u8_token("--action-set-unit-command commandId", command_id)?,
    ))
}

fn parse_action_set_unit_stance_arg(value: &str) -> Result<(Vec<i32>, Option<u8>, bool), String> {
    let parts = value.split('@').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(
            "invalid --action-set-unit-stance, expected <unitId[,unitId...]|none@stanceId|none@enable>"
                .to_string(),
        );
    }
    Ok((
        parse_action_unit_ids_arg(parts[0])?,
        parse_optional_u8_token("--action-set-unit-stance stanceId", parts[1])?,
        parse_bool_arg("--action-set-unit-stance enable", parts[2])?,
    ))
}

fn parse_action_begin_break_arg(value: &str) -> Result<(ClientUnitRef, u8, i32, i32), String> {
    let parts = value.split('@').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(
            "invalid --action-begin-break, expected <none|unit:<id>|block:<pos>|<id>@teamId@x:y>"
                .to_string(),
        );
    }
    let builder = parse_action_unit_ref_arg(
        "--action-begin-break builder",
        "<none|unit:<id>|block:<pos>|<id>>",
        parts[0],
    )?;
    let team_id = parse_u8_arg("--action-begin-break teamId", parts[1])?;
    let (x, y) = parse_i32_pair_colon_arg("--action-begin-break tile", parts[2])?;
    Ok((builder, team_id, x, y))
}

fn parse_action_begin_place_arg(
    value: &str,
) -> Result<(ClientUnitRef, Option<i16>, u8, i32, i32, i32, TypeIoObject), String> {
    let parts = value.split('@').collect::<Vec<_>>();
    if parts.len() != 6 {
        return Err("invalid --action-begin-place, expected <none|unit:<id>|block:<pos>|<id>@blockId|none@teamId@x:y@rotation@value>".to_string());
    }
    let builder = parse_action_unit_ref_arg(
        "--action-begin-place builder",
        "<none|unit:<id>|block:<pos>|<id>>",
        parts[0],
    )?;
    let block_id = parse_optional_i16_like_arg("--action-begin-place blockId", parts[1])?;
    let team_id = parse_u8_arg("--action-begin-place teamId", parts[2])?;
    let (x, y) = parse_i32_pair_colon_arg("--action-begin-place tile", parts[3])?;
    let rotation = parse_i32_arg("--action-begin-place rotation", parts[4])?;
    let place_config = parse_typeio_object_subset_arg("--action-begin-place value", parts[5])?;
    Ok((builder, block_id, team_id, x, y, rotation, place_config))
}

fn parse_optional_text_token(_flag: &str, value: &str) -> Result<Option<String>, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn parse_action_menu_choose_arg(value: &str) -> Result<(i32, i32), String> {
    let Some((menu_id, option)) = value.split_once('@') else {
        return Err("invalid --action-menu-choose, expected <menuId@option>".to_string());
    };
    Ok((
        parse_i32_arg("--action-menu-choose menuId", menu_id)?,
        parse_i32_arg("--action-menu-choose option", option)?,
    ))
}

fn parse_action_text_input_result_arg(value: &str) -> Result<(i32, Option<String>), String> {
    let Some((text_input_id, text)) = value.split_once('@') else {
        return Err(
            "invalid --action-text-input-result, expected <textInputId@text|none>".to_string(),
        );
    };
    Ok((
        parse_i32_arg("--action-text-input-result textInputId", text_input_id)?,
        parse_optional_text_token("--action-text-input-result text", text)?,
    ))
}

fn parse_action_unit_ids_arg(value: &str) -> Result<Vec<i32>, String> {
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("null") {
        return Ok(Vec::new());
    }
    let mut unit_ids = Vec::new();
    for (index, entry) in value.split(',').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid --action-command-units, empty unit id at index {index}"
            ));
        }
        unit_ids.push(parse_i32_arg(
            &format!("--action-command-units unitId[{index}]"),
            entry,
        )?);
    }
    Ok(unit_ids)
}

fn parse_typeio_object_subset_arg(flag: &str, value: &str) -> Result<TypeIoObject, String> {
    if value.eq_ignore_ascii_case("null") {
        return Ok(TypeIoObject::Null);
    }

    let Some((kind, payload)) = value.split_once('=') else {
        return Err(format!(
            "invalid {flag}, expected <null|int=<i32>|long=<i64>|float=<f32>|bool=<true|false|1|0>|int-seq=<i32[,i32...]>|string=<text>|content=<contentType:contentId>|tech-node-raw=<contentType:contentId>|point2=<x:y>|point2-array=<x:y[,x:y...]>|double=<f64>|building-pos=<i32>|laccess=<i16>|vec2=<x:y>|vec2-array=<x:y[,x:y...]>|team=<u8>|bytes=<hex>|legacy-unit-command-null=<u8>|bool-array=<bool[,bool...]>|unit-id=<i32>|int-array=<i32[,i32...]>|object-array=<value>|unit-command=<u16>|...>"
        ));
    };

    match kind {
        "int" => Ok(TypeIoObject::Int(parse_i32_arg(flag, payload)?)),
        "long" => Ok(TypeIoObject::Long(parse_i64_arg(flag, payload)?)),
        "float" => Ok(TypeIoObject::Float(parse_f32_arg(flag, payload)?)),
        "bool" => Ok(TypeIoObject::Bool(parse_bool_arg(flag, payload)?)),
        "int-seq" | "int_seq" => parse_i32_sequence_values(flag, payload, "int-seq")
            .map(TypeIoObject::IntSeq),
        "string" => Ok(TypeIoObject::String(Some(payload.to_string()))),
        "content" => {
            let (content_type, content_id) = parse_content_config_arg(flag, payload)?;
            Ok(TypeIoObject::ContentRaw {
                content_type,
                content_id,
            })
        }
        "tech-node-raw" | "tech_node_raw" | "technode-raw" | "technode_raw" => {
            let (content_type, content_id) = parse_content_config_arg(flag, payload)?;
            Ok(TypeIoObject::TechNodeRaw {
                content_type,
                content_id,
            })
        }
        "point2" => {
            let (x, y) = parse_i32_pair_colon_arg(flag, payload)?;
            Ok(TypeIoObject::Point2 { x, y })
        }
        "point2-array" | "point2_array" => parse_point2_array_config_arg(flag, payload).map(
            |points| TypeIoObject::PackedPoint2Array(
                points
                    .into_iter()
                    .map(|(x, y)| pack_point2(x, y))
                    .collect::<Vec<_>>(),
            ),
        ),
        "double" => Ok(TypeIoObject::Double(parse_f64_arg(flag, payload)?)),
        "building-pos" | "building_pos" => {
            Ok(TypeIoObject::BuildingPos(parse_i32_arg(flag, payload)?))
        }
        "laccess" => Ok(TypeIoObject::LAccess(parse_i16_like_arg(flag, payload)?)),
        "vec2" => {
            let (x, y) = parse_f32_pair_colon_arg(flag, payload)?;
            Ok(TypeIoObject::Vec2 { x, y })
        }
        "vec2-array" | "vec2_array" => parse_vec2_array_subset_arg(flag, payload),
        "team" => Ok(TypeIoObject::Team(parse_u8_arg(flag, payload)?)),
        "bytes" => Ok(TypeIoObject::Bytes(decode_hex_text(payload)?)),
        "legacy-unit-command-null" | "legacy_unit_command_null" => Ok(
            TypeIoObject::LegacyUnitCommandNull(parse_u8_arg(flag, payload)?),
        ),
        "bool-array" | "bool_array" => parse_bool_array_subset_arg(flag, payload),
        "unit-id" | "unit_id" => Ok(TypeIoObject::UnitId(parse_i32_arg(flag, payload)?)),
        "int-array" => parse_i32_array_subset_arg(flag, payload),
        "object-array" => parse_typeio_object_array_subset_arg(flag, payload),
        "unit-command" | "unit_command" => {
            Ok(TypeIoObject::UnitCommand(parse_u16_arg(flag, payload)?))
        }
        _ => Err(format!(
            "invalid {flag}, expected <null|int=<i32>|long=<i64>|float=<f32>|bool=<true|false|1|0>|int-seq=<i32[,i32...]>|string=<text>|content=<contentType:contentId>|tech-node-raw=<contentType:contentId>|point2=<x:y>|point2-array=<x:y[,x:y...]>|double=<f64>|building-pos=<i32>|laccess=<i16>|vec2=<x:y>|vec2-array=<x:y[,x:y...]>|team=<u8>|bytes=<hex>|legacy-unit-command-null=<u8>|bool-array=<bool[,bool...]>|unit-id=<i32>|int-array=<i32[,i32...]>|object-array=<value>|unit-command=<u16>|...>"
        )),
    }
}

fn parse_i32_array_subset_arg(flag: &str, value: &str) -> Result<TypeIoObject, String> {
    parse_i32_sequence_values(flag, value, "int-array").map(TypeIoObject::IntArray)
}

fn parse_i32_sequence_values(flag: &str, value: &str, label: &str) -> Result<Vec<i32>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for (index, entry) in value.split(',').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid {flag}, empty {label} item at index {index}"
            ));
        }
        values.push(parse_i32_arg(&format!("{flag} {label}[{index}]"), entry)?);
    }
    Ok(values)
}

fn parse_bool_array_subset_arg(flag: &str, value: &str) -> Result<TypeIoObject, String> {
    if value.is_empty() {
        return Ok(TypeIoObject::BoolArray(Vec::new()));
    }
    let mut values = Vec::new();
    for (index, entry) in value.split(',').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid {flag}, empty bool-array item at index {index}"
            ));
        }
        values.push(parse_bool_arg(
            &format!("{flag} bool-array[{index}]"),
            entry,
        )?);
    }
    Ok(TypeIoObject::BoolArray(values))
}

fn parse_vec2_array_subset_arg(flag: &str, value: &str) -> Result<TypeIoObject, String> {
    if value.is_empty() {
        return Ok(TypeIoObject::Vec2Array(Vec::new()));
    }
    let mut values = Vec::new();
    for (index, entry) in value.split(',').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid {flag}, empty vec2-array item at index {index}"
            ));
        }
        values.push(parse_f32_pair_colon_arg(
            &format!("{flag} vec2-array[{index}]"),
            entry,
        )?);
    }
    Ok(TypeIoObject::Vec2Array(values))
}

fn parse_typeio_object_array_subset_arg(flag: &str, value: &str) -> Result<TypeIoObject, String> {
    if value.is_empty() {
        return Ok(TypeIoObject::ObjectArray(Vec::new()));
    }
    let mut values = Vec::new();
    for (index, entry) in value.split('|').enumerate() {
        if entry.is_empty() {
            return Err(format!(
                "invalid {flag}, empty object-array item at index {index}"
            ));
        }
        let element_flag = format!("{flag} object-array[{index}]");
        let parsed = parse_typeio_object_subset_arg(&element_flag, entry)?;
        if matches!(parsed, TypeIoObject::ObjectArray(_)) {
            return Err(format!(
                "invalid {flag}, nested object-array is not supported"
            ));
        }
        values.push(parsed);
    }
    Ok(TypeIoObject::ObjectArray(values))
}

fn parse_rotation_arg(flag: &str, value: &str) -> Result<u8, String> {
    let value = parse_i32_arg(flag, value)?;
    u8::try_from(value).map_err(|_| format!("{flag} rotation out of u8 range: {value}"))
}

fn parse_u8_arg(flag: &str, value: &str) -> Result<u8, String> {
    value
        .parse::<u8>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_u16_arg(flag: &str, value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|e| format!("invalid {flag}: {e}"))
}

fn parse_auto_place_arg(flag: &str, value: &str) -> Result<AutoPlacePlan, String> {
    let (place_payload, config_payload) = if let Some((prefix, config)) = value.split_once(';') {
        (prefix, Some(config))
    } else {
        (value, None)
    };

    let parts = place_payload.split(':').collect::<Vec<_>>();
    if !(parts.len() == 1 || parts.len() == 2) {
        return Err(format!(
            "invalid {flag}, expected <block[:rotation][;config]> or <selected[:rotation][;config]>"
        ));
    }

    let block = if parts[0].eq_ignore_ascii_case("selected") {
        AutoBlockChoice::Selected
    } else {
        AutoBlockChoice::Fixed(parse_i16_like_arg(flag, parts[0])?)
    };
    let rotation = if parts.len() == 2 {
        Some(parse_rotation_arg(flag, parts[1])?)
    } else {
        None
    };
    let config = if let Some(config_payload) = config_payload {
        parse_plan_place_config_arg(&format!("{flag} config"), config_payload)?
    } else {
        ClientBuildPlanConfig::None
    };

    Ok(AutoPlacePlan {
        block,
        rotation,
        config,
    })
}

fn apply_plan_edits_to_build_plans(plans: &mut [ClientBuildPlan], ops: &[PlanEditOp]) {
    if plans.is_empty() || ops.is_empty() {
        return;
    }
    let mut editable = plans
        .iter()
        .cloned()
        .map(EditableClientBuildPlan::from)
        .collect::<Vec<_>>();
    apply_plan_edit_ops(&mut editable, ops);
    for (plan, edited) in plans.iter_mut().zip(editable.into_iter()) {
        *plan = edited.into_plan();
    }
}

fn apply_plan_edits_to_relative_build_plans(plans: &mut [RelativeBuildPlan], ops: &[PlanEditOp]) {
    if plans.is_empty() || ops.is_empty() {
        return;
    }
    let mut editable = plans
        .iter()
        .cloned()
        .map(EditableRelativeBuildPlan::from)
        .collect::<Vec<_>>();
    apply_plan_edit_ops(&mut editable, ops);
    for (plan, edited) in plans.iter_mut().zip(editable.into_iter()) {
        *plan = edited.into_plan();
    }
}

fn apply_plan_edit_ops<P: PlanEditable>(plans: &mut [P], ops: &[PlanEditOp]) {
    for op in ops {
        match op {
            PlanEditOp::Rotate { origin, direction } => {
                rotate_plans(plans, *origin, *direction);
            }
            PlanEditOp::Flip { origin, flip_x } => {
                flip_plans(plans, *origin, *flip_x);
            }
        }
    }
}

fn maybe_apply_runtime_plan_edit_loop(
    session: &mut ClientSession,
    runtime_plan_edit_loop: Option<&mut RuntimePlanEditLoopState>,
    now_ms: u64,
) {
    let Some(runtime_plan_edit_loop) = runtime_plan_edit_loop else {
        return;
    };
    if !session.state().ready_to_enter_world || !session.state().connect_confirm_sent {
        return;
    }
    if !session
        .snapshot_input()
        .plans
        .as_ref()
        .is_some_and(|plans| !plans.is_empty())
    {
        return;
    }

    let due_entries = runtime_plan_edit_loop.collect_due_entries(now_ms);
    if due_entries.is_empty() {
        return;
    }

    let input = session.snapshot_input_mut();
    let Some(plans) = input.plans.as_mut() else {
        return;
    };
    for entry in due_entries {
        apply_plan_edits_to_build_plans(plans.as_mut_slice(), std::slice::from_ref(&entry.op));
        println!(
            "plan_edit_loop_applied: index={} tick={}ms scheduled={}ms op={:?}",
            entry.op_index, now_ms, entry.scheduled_ms, entry.op
        );
    }
}

#[derive(Clone)]
struct EditableClientBuildPlan {
    plan: ClientBuildPlan,
}

impl EditableClientBuildPlan {
    fn into_plan(self) -> ClientBuildPlan {
        self.plan
    }
}

impl From<ClientBuildPlan> for EditableClientBuildPlan {
    fn from(plan: ClientBuildPlan) -> Self {
        Self { plan }
    }
}

impl PlanEditable for EditableClientBuildPlan {
    fn is_breaking(&self) -> bool {
        self.plan.breaking
    }

    fn tile(&self) -> (i32, i32) {
        self.plan.tile
    }

    fn set_tile(&mut self, x: i32, y: i32) {
        self.plan.tile = (x, y);
    }

    fn rotation(&self) -> i32 {
        i32::from(self.plan.rotation)
    }

    fn set_rotation(&mut self, rotation: i32) {
        self.plan.rotation = rotation.rem_euclid(256) as u8;
    }

    fn block_meta(&self) -> PlanBlockMeta {
        PlanBlockMeta::default()
    }

    fn map_point_config<F>(&mut self, mut mapper: F)
    where
        F: FnMut(&mut PlanPoint),
    {
        match &mut self.plan.config {
            ClientBuildPlanConfig::Point2 { x, y } => {
                let mut point = PlanPoint { x: *x, y: *y };
                mapper(&mut point);
                *x = point.x;
                *y = point.y;
            }
            ClientBuildPlanConfig::Point2Array(points) => {
                for (x, y) in points {
                    let mut point = PlanPoint { x: *x, y: *y };
                    mapper(&mut point);
                    *x = point.x;
                    *y = point.y;
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct EditableRelativeBuildPlan {
    plan: RelativeBuildPlan,
}

impl EditableRelativeBuildPlan {
    fn into_plan(self) -> RelativeBuildPlan {
        self.plan
    }
}

impl From<RelativeBuildPlan> for EditableRelativeBuildPlan {
    fn from(plan: RelativeBuildPlan) -> Self {
        Self { plan }
    }
}

impl PlanEditable for EditableRelativeBuildPlan {
    fn is_breaking(&self) -> bool {
        self.plan.breaking
    }

    fn tile(&self) -> (i32, i32) {
        self.plan.tile_offset
    }

    fn set_tile(&mut self, x: i32, y: i32) {
        self.plan.tile_offset = (x, y);
    }

    fn rotation(&self) -> i32 {
        i32::from(self.plan.rotation)
    }

    fn set_rotation(&mut self, rotation: i32) {
        self.plan.rotation = rotation.rem_euclid(256) as u8;
    }

    fn block_meta(&self) -> PlanBlockMeta {
        PlanBlockMeta::default()
    }

    fn map_point_config<F>(&mut self, mut mapper: F)
    where
        F: FnMut(&mut PlanPoint),
    {
        match &mut self.plan.config {
            ClientBuildPlanConfig::Point2 { x, y } => {
                let mut point = PlanPoint { x: *x, y: *y };
                mapper(&mut point);
                *x = point.x;
                *y = point.y;
            }
            ClientBuildPlanConfig::Point2Array(points) => {
                for (x, y) in points {
                    let mut point = PlanPoint { x: *x, y: *y };
                    mapper(&mut point);
                    *x = point.x;
                    *y = point.y;
                }
            }
            _ => {}
        }
    }
}

fn build_chat_schedule(
    messages: Vec<String>,
    delay_ms: u64,
    spacing_ms: u64,
) -> Vec<ScheduledChatEntry> {
    messages
        .into_iter()
        .enumerate()
        .map(|(index, text)| ScheduledChatEntry {
            not_before_ms: delay_ms.saturating_add((index as u64).saturating_mul(spacing_ms)),
            text,
        })
        .collect()
}

fn build_intent_schedule(
    snapshots: Vec<InputSnapshot>,
    delay_ms: u64,
    spacing_ms: u64,
) -> Vec<ScheduledIntentSnapshot> {
    snapshots
        .into_iter()
        .enumerate()
        .map(|(index, snapshot)| ScheduledIntentSnapshot {
            not_before_ms: delay_ms.saturating_add((index as u64).saturating_mul(spacing_ms)),
            snapshot,
        })
        .collect()
}

fn build_outbound_action_schedule(
    actions: Vec<OutboundAction>,
    delay_ms: u64,
    spacing_ms: u64,
) -> Vec<ScheduledOutboundAction> {
    actions
        .into_iter()
        .enumerate()
        .map(|(index, action)| ScheduledOutboundAction {
            not_before_ms: delay_ms.saturating_add((index as u64).saturating_mul(spacing_ms)),
            action,
        })
        .collect()
}

fn collect_due_intent_snapshots(
    schedule: &[ScheduledIntentSnapshot],
    now_ms: u64,
    next_index: &mut usize,
) -> Vec<ScheduledIntentSnapshot> {
    let start = *next_index;
    while let Some(entry) = schedule.get(*next_index) {
        if now_ms < entry.not_before_ms {
            break;
        }
        *next_index += 1;
    }
    schedule[start..*next_index].to_vec()
}

fn collect_due_chat_messages(
    schedule: &[ScheduledChatEntry],
    now_ms: u64,
    next_index: &mut usize,
) -> Vec<ScheduledChatEntry> {
    let start = *next_index;
    while let Some(entry) = schedule.get(*next_index) {
        if now_ms < entry.not_before_ms {
            break;
        }
        *next_index += 1;
    }
    schedule[start..*next_index].to_vec()
}

fn collect_due_outbound_actions(
    schedule: &[ScheduledOutboundAction],
    now_ms: u64,
    next_index: &mut usize,
) -> Vec<ScheduledOutboundAction> {
    let start = *next_index;
    while let Some(entry) = schedule.get(*next_index) {
        if now_ms < entry.not_before_ms {
            break;
        }
        *next_index += 1;
    }
    schedule[start..*next_index].to_vec()
}

fn apply_snapshot_overrides(session: &mut ClientSession, args: &CliArgs) {
    let input = session.snapshot_input_mut();
    if let Some(pointer) = args.snapshot_pointer {
        input.pointer = Some(pointer);
    }
    if let Some(mining_tile) = args.snapshot_mining_tile {
        input.mining_tile = Some(mining_tile);
    }
    if let Some(boosting) = args.snapshot_boosting {
        input.boosting = boosting;
    }
    if let Some(shooting) = args.snapshot_shooting {
        input.shooting = shooting;
    }
    if let Some(chatting) = args.snapshot_chatting {
        input.chatting = chatting;
    }
    if let Some(view_size) = args.snapshot_view_size {
        input.view_size = Some(view_size);
    }
    if !args.build_plans.is_empty() {
        input.building = true;
        input.plans = Some(args.build_plans.clone());
        if let Some(plan) = args.build_plans.iter().find(|plan| !plan.breaking) {
            input.selected_block_id = plan.block_id;
            input.selected_rotation = i32::from(plan.rotation);
        }
    }
    if let Some(building) = args.snapshot_building {
        input.building = building;
    }
}

fn render_snapshot_input(
    session: &ClientSession,
    runtime_command_mode: &CommandModeState,
) -> mdt_client_min::client_session::ClientSnapshotInputState {
    let mut input = session.snapshot_input().clone();
    input.command_mode = runtime_command_mode.projection();
    input
}

fn maybe_print_ascii_scene(
    session: &ClientSession,
    args: &CliArgs,
    events: &[ClientSessionEvent],
    render_runtime_adapter: &RenderRuntimeAdapter,
    runtime_command_mode: &CommandModeState,
    ascii_scene_printed: &mut bool,
) {
    if *ascii_scene_printed || !args.render_ascii_on_world_ready {
        return;
    }
    if !should_render_ascii_on_events(events) {
        return;
    }
    let Some(bundle) = session.loaded_world_bundle() else {
        return;
    };
    let Ok(loaded_session) = bundle.loaded_session() else {
        return;
    };
    let Some(runtime_view_center) = resolved_runtime_view_center(
        events,
        session.snapshot_input().view_center,
        session.snapshot_input().position,
        loaded_session.state().player_position(),
    ) else {
        return;
    };

    let (mut scene, mut hud) = project_scene_models_with_view_window(
        &loaded_session,
        &args.locale,
        Some(runtime_view_center),
        LIVE_VIEW_TILES,
    );
    let input = render_snapshot_input(session, runtime_command_mode);
    render_runtime_adapter.apply(&mut scene, &mut hud, &input, session.state());
    let mut presenter =
        AsciiScenePresenter::with_max_view_tiles(LIVE_VIEW_TILES.0, LIVE_VIEW_TILES.1);
    presenter.present(&scene, &hud);
    println!("ascii_scene:\n{}", presenter.last_frame());
    *ascii_scene_printed = true;
}

fn maybe_print_final_ascii_scene(
    session: &ClientSession,
    args: &CliArgs,
    render_runtime_adapter: &RenderRuntimeAdapter,
    runtime_command_mode: &CommandModeState,
) {
    if !args.render_ascii_on_world_ready {
        return;
    }
    let Some(bundle) = session.loaded_world_bundle() else {
        return;
    };
    let Ok(loaded_session) = bundle.loaded_session() else {
        return;
    };
    let runtime_view_center = session
        .snapshot_input()
        .view_center
        .or(session.snapshot_input().position)
        .or(Some(loaded_session.state().player_position()));
    let (mut scene, mut hud) = project_scene_models_with_view_window(
        &loaded_session,
        &args.locale,
        runtime_view_center,
        LIVE_VIEW_TILES,
    );
    let input = render_snapshot_input(session, runtime_command_mode);
    render_runtime_adapter.apply(&mut scene, &mut hud, &input, session.state());
    let runtime_object_ids = collect_authoritative_runtime_scene_object_ids(&scene.objects);
    let mut presenter =
        AsciiScenePresenter::with_max_view_tiles(LIVE_VIEW_TILES.0, LIVE_VIEW_TILES.1);
    presenter.present(&scene, &hud);
    println!("ascii_scene_final:\n{}", presenter.last_frame());
    println!("ascii_scene_final_runtime_objects={runtime_object_ids:?}");
}

fn collect_authoritative_runtime_scene_object_ids(objects: &[RenderObject]) -> Vec<String> {
    objects
        .iter()
        .filter_map(|object| {
            if object.id.starts_with("block:runtime-construct:")
                || object.id.starts_with("terrain:runtime-deconstruct:")
                || object.id.starts_with("marker:runtime-health:")
            {
                Some(object.id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn maybe_present_window_scene(
    session: &ClientSession,
    args: &CliArgs,
    events: &[ClientSessionEvent],
    render_runtime_adapter: &RenderRuntimeAdapter,
    runtime_command_mode: &CommandModeState,
    window_scene_presenter: &mut Option<WindowPresenter<MinifbWindowBackend>>,
    window_scene_disabled: &mut bool,
) {
    if *window_scene_disabled || !args.render_window_live {
        return;
    }
    let Some(bundle) = session.loaded_world_bundle() else {
        return;
    };
    let Ok(loaded_session) = bundle.loaded_session() else {
        return;
    };
    let runtime_view_center = resolved_runtime_view_center(
        events,
        session.snapshot_input().view_center,
        session.snapshot_input().position,
        loaded_session.state().player_position(),
    );

    let (mut scene, mut hud) = project_scene_models_with_view_window(
        &loaded_session,
        &args.locale,
        runtime_view_center,
        LIVE_VIEW_TILES,
    );
    let input = render_snapshot_input(session, runtime_command_mode);
    render_runtime_adapter.apply(&mut scene, &mut hud, &input, session.state());
    let Some(presenter) = window_scene_presenter.as_mut() else {
        return;
    };

    match presenter.present_once(&scene, &hud) {
        Ok(mdt_render_ui::BackendSignal::Continue) => {}
        Ok(mdt_render_ui::BackendSignal::Close) => {
            println!("render_window_closed");
            *window_scene_disabled = true;
            *window_scene_presenter = None;
        }
        Err(error) => {
            println!("render_window_error: {error}");
            *window_scene_disabled = true;
            *window_scene_presenter = None;
        }
    }
}

fn latest_runtime_view_center(
    events: &[ClientSessionEvent],
    snapshot_view_center: Option<(f32, f32)>,
    snapshot_position: Option<(f32, f32)>,
) -> Option<(f32, f32)> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            ClientSessionEvent::CameraPositionUpdated { x, y }
            | ClientSessionEvent::PlayerSpawned { x, y, .. }
            | ClientSessionEvent::PlayerPositionUpdated { x, y } => Some((*x, *y)),
            _ => None,
        })
        .or(snapshot_view_center)
        .or(snapshot_position)
}

fn resolved_runtime_view_center(
    events: &[ClientSessionEvent],
    snapshot_view_center: Option<(f32, f32)>,
    snapshot_position: Option<(f32, f32)>,
    loaded_player_position: (f32, f32),
) -> Option<(f32, f32)> {
    latest_runtime_view_center(events, snapshot_view_center, snapshot_position)
        .or(Some(loaded_player_position))
}

fn should_render_ascii_on_events(events: &[ClientSessionEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            ClientSessionEvent::WorldStreamReady { .. }
                | ClientSessionEvent::PlayerSpawned { .. }
                | ClientSessionEvent::PlayerPositionUpdated { .. }
                | ClientSessionEvent::CameraPositionUpdated { .. }
        )
    })
}

fn maybe_apply_relative_build_plans(
    session: &mut ClientSession,
    args: &CliArgs,
    events: &[ClientSessionEvent],
    relative_build_plans_applied: &mut bool,
) {
    if *relative_build_plans_applied || args.relative_build_plans.is_empty() {
        return;
    }
    if !events.iter().any(is_runtime_build_plan_event) {
        return;
    }

    const TILE_SIZE: f32 = 8.0;

    let Some((x, y)) = latest_build_plan_origin(events) else {
        return;
    };
    let base_tile = (
        (x / TILE_SIZE).round() as i32,
        (y / TILE_SIZE).round() as i32,
    );
    let resolved = args
        .relative_build_plans
        .iter()
        .map(|plan| ClientBuildPlan {
            tile: (
                base_tile.0 + plan.tile_offset.0,
                base_tile.1 + plan.tile_offset.1,
            ),
            breaking: plan.breaking,
            block_id: plan.block_id,
            rotation: plan.rotation,
            config: plan.config.clone(),
        })
        .collect::<Vec<_>>();

    let input = session.snapshot_input_mut();
    let plans = merge_build_plan_queue_tail(input.plans.as_deref(), &resolved);
    input.building = true;
    input.plans = Some(plans);
    if input.selected_block_id.is_none() {
        if let Some(plan) = resolved.iter().find(|plan| !plan.breaking) {
            input.selected_block_id = plan.block_id;
            input.selected_rotation = i32::from(plan.rotation);
        }
    }

    *relative_build_plans_applied = true;
    println!(
        "build_plans_applied: origin_tile={:?} plans={:?}",
        base_tile, resolved
    );
}

fn maybe_apply_auto_build_plans(
    session: &mut ClientSession,
    args: &CliArgs,
    events: &[ClientSessionEvent],
    auto_build_plans_applied: &mut bool,
) {
    if *auto_build_plans_applied
        || (!args.auto_break_near_player
            && args.auto_place_near_player.is_empty()
            && args.auto_place_conflict_near_player.is_empty())
    {
        return;
    }
    if !events.iter().any(is_runtime_build_plan_event) {
        return;
    }
    let Some(origin) = latest_build_plan_origin(events) else {
        return;
    };
    let Some(origin_tile) = world_to_tile(origin) else {
        return;
    };

    let resolved = {
        let Some(world) = session.loaded_world_state() else {
            return;
        };
        let selected_block_id = session.snapshot_input().selected_block_id;
        let selected_rotation = u8::try_from(session.snapshot_input().selected_rotation)
            .ok()
            .unwrap_or(0);
        let mut plans = Vec::new();

        if args.auto_break_near_player {
            if let Some(tile) = select_break_near_player_tile(&world, origin_tile) {
                plans.push(ClientBuildPlan {
                    tile,
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                });
            }
        }

        for request in &args.auto_place_conflict_near_player {
            let Some((block_id, rotation)) =
                resolve_auto_place_request(selected_block_id, selected_rotation, request)
            else {
                continue;
            };
            let Some(tile) = select_conflict_place_near_player_tile(&world, origin_tile, block_id)
            else {
                continue;
            };
            plans.push(ClientBuildPlan {
                tile,
                breaking: false,
                block_id: Some(block_id),
                rotation,
                config: request.config.clone(),
            });
        }

        for request in &args.auto_place_near_player {
            let Some((block_id, rotation)) =
                resolve_auto_place_request(selected_block_id, selected_rotation, request)
            else {
                continue;
            };
            let Some(tile) = select_place_near_player_tile(&world, origin_tile) else {
                continue;
            };
            plans.push(ClientBuildPlan {
                tile,
                breaking: false,
                block_id: Some(block_id),
                rotation,
                config: request.config.clone(),
            });
        }

        plans
    };

    if resolved.is_empty() {
        return;
    }

    let input = session.snapshot_input_mut();
    let plans = merge_build_plan_queue_tail(input.plans.as_deref(), &resolved);
    input.building = true;
    input.plans = Some(plans);
    if input.selected_block_id.is_none() {
        if let Some(plan) = resolved.iter().find(|plan| !plan.breaking) {
            input.selected_block_id = plan.block_id;
            input.selected_rotation = i32::from(plan.rotation);
        }
    }

    *auto_build_plans_applied = true;
    println!(
        "build_plans_auto_applied: origin_tile={:?} plans={:?}",
        origin_tile, resolved
    );
}

fn sync_runtime_build_selection_state(session: &mut ClientSession, args: &CliArgs) {
    let input = session.snapshot_input_mut();
    let has_plans = input.plans.as_ref().is_some_and(|plans| !plans.is_empty());

    // Keep explicit snapshot-building flags authoritative. Under live runtime capture,
    // also preserve the just-sampled building bit through the queue-sync pass.
    input.building = args
        .snapshot_building
        .unwrap_or(has_plans || (args.enable_live_intent_runtime_capture && input.building));

    if let Some(plan) = input.plans.as_ref().and_then(|plans| {
        plans
            .iter()
            .rev()
            .find(|plan| !plan.breaking && plan.block_id.is_some())
    }) {
        input.selected_block_id = plan.block_id;
        input.selected_rotation = i32::from(plan.rotation);
    }
}

fn merge_build_plan_queue_tail(
    existing: Option<&[ClientBuildPlan]>,
    incoming: &[ClientBuildPlan],
) -> Vec<ClientBuildPlan> {
    let mut merged = Vec::with_capacity(
        existing
            .map_or(0, |plans| plans.len())
            .saturating_add(incoming.len()),
    );
    if let Some(existing) = existing {
        for plan in existing {
            enqueue_build_plan_tail(&mut merged, plan.clone());
        }
    }
    for plan in incoming {
        enqueue_build_plan_tail(&mut merged, plan.clone());
    }
    merged
}

fn enqueue_build_plan_tail(queue: &mut Vec<ClientBuildPlan>, plan: ClientBuildPlan) {
    if let Some(index) = queue.iter().position(|entry| entry.tile == plan.tile) {
        queue.remove(index);
    }
    queue.push(plan);
}

fn world_to_tile(position: (f32, f32)) -> Option<(i32, i32)> {
    const TILE_SIZE: f32 = 8.0;
    let (x, y) = position;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some((
        (x / TILE_SIZE).round() as i32,
        (y / TILE_SIZE).round() as i32,
    ))
}

fn resolve_auto_place_request(
    selected_block_id: Option<i16>,
    selected_rotation: u8,
    request: &AutoPlacePlan,
) -> Option<(i16, u8)> {
    match request.block {
        AutoBlockChoice::Selected => selected_block_id
            .map(|block_id| (block_id, request.rotation.unwrap_or(selected_rotation))),
        AutoBlockChoice::Fixed(block_id) => Some((block_id, request.rotation.unwrap_or(0))),
    }
}

fn select_place_near_player_tile(
    world: &LoadedWorldState<'_>,
    origin_tile: (i32, i32),
) -> Option<(i32, i32)> {
    select_place_near_player_tile_with_visibility(world, origin_tile, true)
        .or_else(|| select_place_near_player_tile_with_visibility(world, origin_tile, false))
}

fn select_place_near_player_tile_with_visibility(
    world: &LoadedWorldState<'_>,
    origin_tile: (i32, i32),
    require_visible: bool,
) -> Option<(i32, i32)> {
    let graph = world.graph();
    let player_team_id = world.player().team_id;

    graph
        .grid()
        .iter_tiles()
        .filter_map(|tile| {
            if (require_visible
                && !tile_is_visible_to_player(&graph, player_team_id, tile.x, tile.y))
                || tile.block_id != 0
                || tile.building_center_index.is_some()
                || !graph.team_plans_at(tile.x as i16, tile.y as i16).is_empty()
            {
                return None;
            }

            Some((
                (
                    adjacency_rank(&graph, player_team_id, tile.x, tile.y),
                    tile_distance_sq(origin_tile, tile.x, tile.y),
                    tile.y,
                    tile.x,
                ),
                (tile.x as i32, tile.y as i32),
            ))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, tile)| tile)
}

fn select_conflict_place_near_player_tile(
    world: &LoadedWorldState<'_>,
    origin_tile: (i32, i32),
    requested_block_id: i16,
) -> Option<(i32, i32)> {
    let graph = world.graph();
    let player_team_id = world.player().team_id;

    graph
        .grid()
        .iter_tiles()
        .filter_map(|tile| {
            if !tile_is_visible_to_player(&graph, player_team_id, tile.x, tile.y)
                || !graph.team_plans_at(tile.x as i16, tile.y as i16).is_empty()
            {
                return None;
            }

            let center = graph.building_center_at(tile.x, tile.y)?;
            if center.building.base.team_id != player_team_id
                || i16::try_from(tile.block_id).ok() == Some(requested_block_id)
            {
                return None;
            }

            let priority = match center.building.parsed_tail {
                ParsedBuildingTail::Core(_) => 0u8,
                _ => 1u8,
            };

            Some((
                (
                    priority,
                    tile_distance_sq(origin_tile, tile.x, tile.y),
                    tile.y,
                    tile.x,
                ),
                (tile.x as i32, tile.y as i32),
            ))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, tile)| tile)
}

fn select_break_near_player_tile(
    world: &LoadedWorldState<'_>,
    origin_tile: (i32, i32),
) -> Option<(i32, i32)> {
    let graph = world.graph();
    let player_team_id = world.player().team_id;

    graph
        .grid()
        .iter_tiles()
        .filter_map(|tile| {
            let center = graph.building_center_at(tile.x, tile.y)?;
            if center.building.base.team_id != player_team_id {
                return None;
            }

            let priority = match center.building.parsed_tail {
                ParsedBuildingTail::Core(_) => 0u8,
                _ => 1u8,
            };

            Some((
                (
                    priority,
                    tile_distance_sq(origin_tile, tile.x, tile.y),
                    tile.y,
                    tile.x,
                ),
                (tile.x as i32, tile.y as i32),
            ))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, tile)| tile)
}

fn tile_is_visible_to_player(
    graph: &WorldGraph<'_>,
    player_team_id: u8,
    x: usize,
    y: usize,
) -> bool {
    !matches!(graph.fog_revealed(player_team_id, x, y), Some(false))
}

fn adjacency_rank(graph: &WorldGraph<'_>, player_team_id: u8, x: usize, y: usize) -> u8 {
    let mut rank = 2u8;

    for (nx, ny) in orthogonal_neighbors(graph, x, y) {
        let Some(center) = graph.building_center_at(nx, ny) else {
            continue;
        };
        if center.building.base.team_id == player_team_id {
            return 0;
        }
        rank = 1;
    }

    rank
}

fn orthogonal_neighbors(graph: &WorldGraph<'_>, x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut neighbors = Vec::with_capacity(4);
    if x > 0 {
        neighbors.push((x - 1, y));
    }
    if y > 0 {
        neighbors.push((x, y - 1));
    }
    if x + 1 < graph.width() {
        neighbors.push((x + 1, y));
    }
    if y + 1 < graph.height() {
        neighbors.push((x, y + 1));
    }
    neighbors
}

fn tile_distance_sq(origin_tile: (i32, i32), x: usize, y: usize) -> u32 {
    let dx = i64::from(origin_tile.0) - x as i64;
    let dy = i64::from(origin_tile.1) - y as i64;
    (dx * dx + dy * dy) as u32
}

fn maybe_apply_runtime_snapshot_overrides(
    session: &mut ClientSession,
    args: &CliArgs,
    movement_probe: Option<&mut MovementProbeController>,
    live_intent_mapper: Option<&mut LiveIntentMapperController>,
    snapshot_interval_ms: u64,
    now_ms: u64,
) {
    if let Some(movement_probe) = movement_probe {
        let input = session.snapshot_input_mut();
        let runtime = RuntimeInputState {
            unit_id: input.unit_id,
            dead: input.dead,
            position: input.position,
            pointer: input.pointer,
        };
        if let Some(update) =
            movement_probe.advance(runtime, now_ms, snapshot_interval_ms, args.snapshot_pointer)
        {
            input.position = Some(update.position);
            input.view_center = Some(update.view_center);
            input.velocity = update.velocity;
            input.rotation = update.rotation_degrees;
            input.base_rotation = update.base_rotation_degrees;
            input.pointer = Some(update.pointer);
        }
    }

    if let Some(live_intent_mapper) = live_intent_mapper {
        let runtime_snapshot = sample_runtime_intent_snapshot(session);
        if live_intent_mapper.advance(&runtime_snapshot, now_ms) {
            apply_live_intents_to_snapshot(session, live_intent_mapper.state());
        }
    }
}

fn sample_runtime_intent_snapshot(session: &mut ClientSession) -> InputSnapshot {
    let input = session.snapshot_input_mut().clone();
    let aim_axis = input.pointer.or(input.position).unwrap_or((0.0, 0.0));
    let mut active_actions = Vec::with_capacity(3);
    if input.shooting {
        active_actions.push(BinaryAction::Fire);
    }
    if input.boosting {
        active_actions.push(BinaryAction::Boost);
    }
    if input.chatting {
        active_actions.push(BinaryAction::Chat);
    }

    InputSnapshot {
        move_axis: input.velocity,
        aim_axis,
        mining_tile: input.mining_tile,
        building: input.building,
        active_actions,
    }
}

fn apply_live_intents_to_snapshot(session: &mut ClientSession, state: &LiveIntentState) {
    let input = session.snapshot_input_mut();
    input.velocity = state.move_axis;
    if state.move_axis != (0.0, 0.0) {
        let heading = state.move_axis.1.atan2(state.move_axis.0).to_degrees();
        input.rotation = heading;
        input.base_rotation = heading;
    }
    input.pointer = Some(state.aim_axis);
    input.mining_tile = state.mining_tile;
    input.building = state.building;
    input.shooting = state.is_action_active(BinaryAction::Fire);
    input.boosting = state.is_action_active(BinaryAction::Boost);
    input.chatting = state.is_action_active(BinaryAction::Chat);
}

fn maybe_print_runtime_input(
    session: &mut ClientSession,
    args: &CliArgs,
    events: &[ClientSessionEvent],
    now_ms: u64,
    last_runtime_input: &mut Option<(Option<i32>, bool, Option<(u32, u32)>)>,
) {
    if args.movement_probe.is_none() || !events.iter().any(is_runtime_refresh_event) {
        return;
    }

    let input = session.snapshot_input_mut();
    let current = (
        input.unit_id,
        input.dead,
        input.position.map(|(x, y)| (x.to_bits(), y.to_bits())),
    );
    if *last_runtime_input == Some(current) {
        return;
    }
    *last_runtime_input = Some(current);

    println!(
        "runtime_input: tick={}ms unit_id={:?} dead={} position={:?} velocity=({:.3},{:.3}) pointer={:?}",
        now_ms,
        input.unit_id,
        input.dead,
        input.position,
        input.velocity.0,
        input.velocity.1,
        input.pointer
    );
}

fn maybe_print_client_packets(args: &CliArgs, events: &[ClientSessionEvent]) {
    if !args.print_client_packets {
        return;
    }

    for line in summarize_client_packet_events(events) {
        println!("{line}");
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CustomPacketWatchEncoding {
    Text,
    Binary,
    LogicData,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CustomPacketWatchSpec {
    packet_type: String,
    encoding: CustomPacketWatchEncoding,
}

#[derive(Debug)]
struct RuntimeCustomPacketWatch {
    state: Rc<RefCell<RuntimeCustomPacketWatchState>>,
}

#[derive(Debug, Default)]
struct RuntimeCustomPacketWatchState {
    text_stats: BTreeMap<String, RuntimeCustomPacketTextStats>,
    binary_stats: BTreeMap<String, RuntimeCustomPacketBinaryStats>,
    logic_data_stats: BTreeMap<String, RuntimeCustomPacketLogicDataStats>,
    pending_lines: VecDeque<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct RuntimeCustomPacketTextStats {
    handler_count: usize,
    event_reliable_count: usize,
    event_unreliable_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct RuntimeCustomPacketBinaryStats {
    handler_count: usize,
    event_reliable_count: usize,
    event_unreliable_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct RuntimeCustomPacketLogicDataStats {
    handler_count: usize,
    event_reliable_count: usize,
    event_unreliable_count: usize,
}

impl RuntimeCustomPacketWatch {
    fn observe_events(&self, events: &[ClientSessionEvent]) {
        self.state.borrow_mut().observe_events(events);
    }

    fn drain_lines(&self) -> Vec<String> {
        self.state.borrow_mut().drain_lines()
    }

    fn summary_lines(&self) -> Vec<String> {
        self.state.borrow().summary_lines()
    }
}

impl RuntimeCustomPacketWatchState {
    fn register_text_type(&mut self, packet_type: &str) {
        self.text_stats.entry(packet_type.to_string()).or_default();
    }

    fn register_binary_type(&mut self, packet_type: &str) {
        self.binary_stats
            .entry(packet_type.to_string())
            .or_default();
    }

    fn register_logic_data_channel(&mut self, channel: &str) {
        self.logic_data_stats
            .entry(channel.to_string())
            .or_default();
    }

    fn record_text_handler(&mut self, packet_type: &str, contents: &str) {
        let stats = self.text_stats.entry(packet_type.to_string()).or_default();
        stats.handler_count = stats.handler_count.saturating_add(1);
        let escaped = contents.escape_default().to_string();
        let preview = truncate_for_preview(&escaped, 96);
        self.pending_lines.push_back(format!(
            "client_packet_handler: type={packet_type:?} count={} len={} preview={preview:?}",
            stats.handler_count,
            contents.len()
        ));
    }

    fn record_binary_handler(&mut self, packet_type: &str, contents: &[u8]) {
        let stats = self
            .binary_stats
            .entry(packet_type.to_string())
            .or_default();
        stats.handler_count = stats.handler_count.saturating_add(1);
        let prefix_len = contents.len().min(16);
        let hex_prefix = encode_hex_text(&contents[..prefix_len]);
        self.pending_lines.push_back(format!(
            "client_binary_packet_handler: type={packet_type:?} count={} len={} hex_prefix={hex_prefix}",
            stats.handler_count,
            contents.len()
        ));
    }

    fn record_logic_data_handler(
        &mut self,
        channel: &str,
        transport: ClientLogicDataTransport,
        value: &TypeIoObject,
    ) {
        let stats = self
            .logic_data_stats
            .entry(channel.to_string())
            .or_default();
        stats.handler_count = stats.handler_count.saturating_add(1);
        let preview = truncate_for_preview(&format!("{value:?}"), 96);
        self.pending_lines.push_back(format!(
            "client_logic_data_handler: channel={channel:?} count={} transport={} kind={:?} preview={preview:?}",
            stats.handler_count,
            logic_data_transport_label(transport),
            value.kind()
        ));
    }

    fn observe_events(&mut self, events: &[ClientSessionEvent]) {
        for event in events {
            match event {
                ClientSessionEvent::ClientPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerPacketReliable { packet_type, .. } => {
                    self.record_text_event(packet_type, true);
                }
                ClientSessionEvent::ClientPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerPacketUnreliable { packet_type, .. } => {
                    self.record_text_event(packet_type, false);
                }
                ClientSessionEvent::ClientBinaryPacketReliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketReliable { packet_type, .. } => {
                    self.record_binary_event(packet_type, true);
                }
                ClientSessionEvent::ClientBinaryPacketUnreliable { packet_type, .. }
                | ClientSessionEvent::ServerBinaryPacketUnreliable { packet_type, .. } => {
                    self.record_binary_event(packet_type, false);
                }
                ClientSessionEvent::ClientLogicDataReliable { channel, .. } => {
                    self.record_logic_data_event(channel, true);
                }
                ClientSessionEvent::ClientLogicDataUnreliable { channel, .. } => {
                    self.record_logic_data_event(channel, false);
                }
                _ => {}
            }
        }
    }

    fn record_text_event(&mut self, packet_type: &str, reliable: bool) {
        let Some(stats) = self.text_stats.get_mut(packet_type) else {
            return;
        };
        if reliable {
            stats.event_reliable_count = stats.event_reliable_count.saturating_add(1);
        } else {
            stats.event_unreliable_count = stats.event_unreliable_count.saturating_add(1);
        }
    }

    fn record_binary_event(&mut self, packet_type: &str, reliable: bool) {
        let Some(stats) = self.binary_stats.get_mut(packet_type) else {
            return;
        };
        if reliable {
            stats.event_reliable_count = stats.event_reliable_count.saturating_add(1);
        } else {
            stats.event_unreliable_count = stats.event_unreliable_count.saturating_add(1);
        }
    }

    fn record_logic_data_event(&mut self, channel: &str, reliable: bool) {
        let Some(stats) = self.logic_data_stats.get_mut(channel) else {
            return;
        };
        if reliable {
            stats.event_reliable_count = stats.event_reliable_count.saturating_add(1);
        } else {
            stats.event_unreliable_count = stats.event_unreliable_count.saturating_add(1);
        }
    }

    fn drain_lines(&mut self) -> Vec<String> {
        self.pending_lines.drain(..).collect()
    }

    fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (packet_type, stats) in &self.text_stats {
            let event_total = stats
                .event_reliable_count
                .saturating_add(stats.event_unreliable_count);
            let parity = if stats.handler_count == event_total {
                "ok"
            } else {
                "mismatch"
            };
            lines.push(format!(
                "client_packet_handler_summary: type={packet_type:?} count={} event_reliable={} event_unreliable={} event_total={} parity={parity}",
                stats.handler_count,
                stats.event_reliable_count,
                stats.event_unreliable_count,
                event_total
            ));
        }
        for (packet_type, stats) in &self.binary_stats {
            let event_total = stats
                .event_reliable_count
                .saturating_add(stats.event_unreliable_count);
            let parity = if stats.handler_count == event_total {
                "ok"
            } else {
                "mismatch"
            };
            lines.push(format!(
                "client_binary_packet_handler_summary: type={packet_type:?} count={} event_reliable={} event_unreliable={} event_total={} parity={parity}",
                stats.handler_count,
                stats.event_reliable_count,
                stats.event_unreliable_count,
                event_total
            ));
        }
        for (channel, stats) in &self.logic_data_stats {
            let event_total = stats
                .event_reliable_count
                .saturating_add(stats.event_unreliable_count);
            let parity = if stats.handler_count == event_total {
                "ok"
            } else {
                "mismatch"
            };
            lines.push(format!(
                "client_logic_data_handler_summary: channel={channel:?} count={} event_reliable={} event_unreliable={} event_total={} parity={parity}",
                stats.handler_count,
                stats.event_reliable_count,
                stats.event_unreliable_count,
                event_total
            ));
        }
        lines
    }
}

fn install_runtime_custom_packet_watch(
    session: &mut ClientSession,
    args: &CliArgs,
) -> Option<RuntimeCustomPacketWatch> {
    let watch_specs = build_runtime_custom_packet_watch_specs(args);
    if watch_specs.is_empty() {
        return None;
    }

    let state = Rc::new(RefCell::new(RuntimeCustomPacketWatchState::default()));
    for spec in watch_specs {
        match spec.encoding {
            CustomPacketWatchEncoding::Text => {
                state.borrow_mut().register_text_type(&spec.packet_type);
                let packet_type = spec.packet_type.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_packet_handler(spec.packet_type, move |contents| {
                    shared_state
                        .borrow_mut()
                        .record_text_handler(&packet_type, contents);
                });
            }
            CustomPacketWatchEncoding::Binary => {
                state.borrow_mut().register_binary_type(&spec.packet_type);
                let packet_type = spec.packet_type.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_binary_packet_handler(spec.packet_type, move |contents| {
                    shared_state
                        .borrow_mut()
                        .record_binary_handler(&packet_type, contents);
                });
            }
            CustomPacketWatchEncoding::LogicData => {
                state
                    .borrow_mut()
                    .register_logic_data_channel(&spec.packet_type);
                let channel = spec.packet_type.clone();
                let shared_state = Rc::clone(&state);
                session.add_client_logic_data_handler(spec.packet_type, move |transport, value| {
                    shared_state
                        .borrow_mut()
                        .record_logic_data_handler(&channel, transport, value);
                });
            }
        }
    }

    Some(RuntimeCustomPacketWatch { state })
}

fn maybe_print_custom_packet_watch_events(
    custom_packet_watch: Option<&mut RuntimeCustomPacketWatch>,
    events: &[ClientSessionEvent],
) {
    let Some(custom_packet_watch) = custom_packet_watch else {
        return;
    };
    custom_packet_watch.observe_events(events);
    for line in custom_packet_watch.drain_lines() {
        println!("{line}");
    }
}

fn maybe_print_custom_packet_watch_summary(custom_packet_watch: Option<&RuntimeCustomPacketWatch>) {
    let Some(custom_packet_watch) = custom_packet_watch else {
        return;
    };
    for line in custom_packet_watch.summary_lines() {
        println!("{line}");
    }
}

fn maybe_print_custom_packet_semantic_events(
    custom_packet_semantics: Option<&mut RuntimeCustomPacketSemantics>,
    events: &[ClientSessionEvent],
) {
    let Some(custom_packet_semantics) = custom_packet_semantics else {
        return;
    };
    custom_packet_semantics.observe_events(events);
    for line in custom_packet_semantics.drain_lines() {
        println!("{line}");
    }
}

fn maybe_print_custom_packet_semantic_summary(
    custom_packet_semantics: Option<&RuntimeCustomPacketSemantics>,
) {
    let Some(custom_packet_semantics) = custom_packet_semantics else {
        return;
    };
    for line in custom_packet_semantics.summary_lines() {
        println!("{line}");
    }
}

fn maybe_queue_runtime_custom_packet_relays(
    session: &mut ClientSession,
    custom_packet_relays: Option<&mut RuntimeCustomPacketRelays>,
    events: &[ClientSessionEvent],
    now_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(custom_packet_relays) = custom_packet_relays else {
        return Ok(());
    };
    custom_packet_relays.observe_events(events);
    for entry in custom_packet_relays.drain_entries() {
        queue_runtime_custom_packet_relay_action(session, &entry.action)?;
        println!("{} tick={}ms", entry.line, now_ms);
        for line in summarize_runtime_custom_packet_relay_action(&entry.action) {
            println!(
                "runtime_custom_packet_relay_replay: tick={}ms {line}",
                now_ms
            );
        }
    }
    Ok(())
}

fn queue_runtime_custom_packet_relay_action(
    session: &mut ClientSession,
    action: &RuntimeCustomPacketRelayAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        RuntimeCustomPacketRelayAction::Text {
            packet_type,
            contents,
            transport,
        } => {
            session.queue_client_packet(packet_type, contents, *transport)?;
        }
        RuntimeCustomPacketRelayAction::Binary {
            packet_type,
            contents,
            transport,
        } => {
            session.queue_client_binary_packet(packet_type, contents, *transport)?;
        }
        RuntimeCustomPacketRelayAction::LogicData {
            channel,
            value,
            transport,
        } => {
            session.queue_client_logic_data(channel, value, *transport)?;
        }
    }
    Ok(())
}

fn maybe_print_runtime_custom_packet_relay_summary(
    custom_packet_relays: Option<&RuntimeCustomPacketRelays>,
) {
    let Some(custom_packet_relays) = custom_packet_relays else {
        return;
    };
    for line in custom_packet_relays.summary_lines() {
        println!("{line}");
    }
}

fn build_runtime_custom_packet_watch_specs(args: &CliArgs) -> Vec<CustomPacketWatchSpec> {
    dedupe_packet_watch_types(&args.watched_client_packet_types)
        .into_iter()
        .map(|packet_type| CustomPacketWatchSpec {
            packet_type,
            encoding: CustomPacketWatchEncoding::Text,
        })
        .chain(
            dedupe_packet_watch_types(&args.watched_client_binary_packet_types)
                .into_iter()
                .map(|packet_type| CustomPacketWatchSpec {
                    packet_type,
                    encoding: CustomPacketWatchEncoding::Binary,
                }),
        )
        .chain(
            dedupe_packet_watch_types(&args.watched_client_logic_data_channels)
                .into_iter()
                .map(|packet_type| CustomPacketWatchSpec {
                    packet_type,
                    encoding: CustomPacketWatchEncoding::LogicData,
                }),
        )
        .collect()
}

fn logic_data_transport_label(transport: ClientLogicDataTransport) -> &'static str {
    match transport {
        ClientLogicDataTransport::Reliable => "reliable",
        ClientLogicDataTransport::Unreliable => "unreliable",
    }
}

fn dedupe_packet_watch_types(packet_types: &[String]) -> Vec<String> {
    let mut deduped = Vec::new();
    let mut seen = BTreeSet::new();
    for packet_type in packet_types {
        if seen.insert(packet_type.clone()) {
            deduped.push(packet_type.clone());
        }
    }
    deduped
}

fn format_final_kick_summary(
    kicked: bool,
    reason_text: Option<&str>,
    reason_ordinal: Option<i32>,
    duration_ms: Option<u64>,
) -> String {
    mdt_client_min::event_summary::format_final_kick_summary(
        kicked,
        reason_text,
        reason_ordinal,
        duration_ms,
    )
}

fn summarize_client_packet_events(events: &[ClientSessionEvent]) -> Vec<String> {
    mdt_client_min::event_summary::summarize_client_packet_events(events)
}

fn summarize_custom_packet_outbound_action(action: &OutboundAction) -> Vec<String> {
    custom_packet_event_from_outbound_action(action)
        .map(|event| summarize_client_packet_events(&[event]))
        .unwrap_or_default()
}

fn summarize_runtime_custom_packet_relay_action(
    action: &RuntimeCustomPacketRelayAction,
) -> Vec<String> {
    summarize_client_packet_events(&[custom_packet_event_from_relay_action(action)])
}

fn custom_packet_event_from_outbound_action(action: &OutboundAction) -> Option<ClientSessionEvent> {
    match action {
        OutboundAction::ClientPacket {
            packet_type,
            contents,
            transport,
        } => Some(match transport {
            ClientPacketTransport::Tcp => ClientSessionEvent::ClientPacketReliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
            ClientPacketTransport::Udp => ClientSessionEvent::ClientPacketUnreliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
        }),
        OutboundAction::ClientBinaryPacket {
            packet_type,
            contents,
            transport,
        } => Some(match transport {
            ClientPacketTransport::Tcp => ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
            ClientPacketTransport::Udp => ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
        }),
        OutboundAction::ClientLogicData {
            channel,
            value,
            transport,
        } => Some(match transport {
            ClientLogicDataTransport::Reliable => ClientSessionEvent::ClientLogicDataReliable {
                channel: channel.clone(),
                value: value.clone(),
            },
            ClientLogicDataTransport::Unreliable => ClientSessionEvent::ClientLogicDataUnreliable {
                channel: channel.clone(),
                value: value.clone(),
            },
        }),
        _ => None,
    }
}

fn custom_packet_event_from_relay_action(
    action: &RuntimeCustomPacketRelayAction,
) -> ClientSessionEvent {
    match action {
        RuntimeCustomPacketRelayAction::Text {
            packet_type,
            contents,
            transport,
        } => match transport {
            ClientPacketTransport::Tcp => ClientSessionEvent::ClientPacketReliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
            ClientPacketTransport::Udp => ClientSessionEvent::ClientPacketUnreliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
        },
        RuntimeCustomPacketRelayAction::Binary {
            packet_type,
            contents,
            transport,
        } => match transport {
            ClientPacketTransport::Tcp => ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
            ClientPacketTransport::Udp => ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type: packet_type.clone(),
                contents: contents.clone(),
            },
        },
        RuntimeCustomPacketRelayAction::LogicData {
            channel,
            value,
            transport,
        } => match transport {
            ClientLogicDataTransport::Reliable => ClientSessionEvent::ClientLogicDataReliable {
                channel: channel.clone(),
                value: value.clone(),
            },
            ClientLogicDataTransport::Unreliable => ClientSessionEvent::ClientLogicDataUnreliable {
                channel: channel.clone(),
                value: value.clone(),
            },
        },
    }
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

fn maybe_queue_chat_messages(
    session: &mut ClientSession,
    args: &CliArgs,
    now_ms: u64,
    next_chat_index: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.chat_schedule.is_empty()
        || !session.state().ready_to_enter_world
        || !session.state().connect_confirm_sent
    {
        return Ok(());
    }

    let queued_messages = collect_due_chat_messages(&args.chat_schedule, now_ms, next_chat_index);
    let queued_start_index = next_chat_index.saturating_sub(queued_messages.len());
    for (offset, message) in queued_messages.into_iter().enumerate() {
        session.queue_send_chat_message(message.text.clone())?;
        println!(
            "chat_message_queued: index={} tick={}ms scheduled={}ms text={:?}",
            queued_start_index + offset,
            now_ms,
            message.not_before_ms,
            message.text
        );
    }
    Ok(())
}

fn maybe_queue_outbound_actions(
    session: &mut ClientSession,
    args: &CliArgs,
    now_ms: u64,
    next_action_index: &mut usize,
    runtime_command_mode: &mut CommandModeState,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.outbound_action_schedule.is_empty()
        || !session.state().ready_to_enter_world
        || !session.state().connect_confirm_sent
    {
        return Ok(());
    }

    let queued_actions =
        collect_due_outbound_actions(&args.outbound_action_schedule, now_ms, next_action_index);
    let queued_start_index = next_action_index.saturating_sub(queued_actions.len());
    for (offset, entry) in queued_actions.into_iter().enumerate() {
        queue_outbound_action_with_command_mode(session, &entry.action, runtime_command_mode)?;
        println!(
            "outbound_action_queued: index={} tick={}ms scheduled={}ms action={:?}",
            queued_start_index + offset,
            now_ms,
            entry.not_before_ms,
            entry.action
        );
        for line in summarize_custom_packet_outbound_action(&entry.action) {
            println!(
                "outbound_action_client_packet_replay: index={} tick={}ms scheduled={}ms {line}",
                queued_start_index + offset,
                now_ms,
                entry.not_before_ms,
            );
        }
    }
    Ok(())
}

fn queue_outbound_action(
    session: &mut ClientSession,
    action: &OutboundAction,
) -> Result<(), Box<dyn std::error::Error>> {
    queue_outbound_action_inner(session, action)
}

fn queue_outbound_action_with_command_mode(
    session: &mut ClientSession,
    action: &OutboundAction,
    runtime_command_mode: &mut CommandModeState,
) -> Result<(), Box<dyn std::error::Error>> {
    queue_outbound_action_inner(session, action)?;
    sync_command_mode_state_for_outbound_action(runtime_command_mode, action);
    Ok(())
}

fn queue_outbound_action_inner(
    session: &mut ClientSession,
    action: &OutboundAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        OutboundAction::RequestItem {
            build_pos,
            item_id,
            amount,
        } => {
            session.queue_request_item(*build_pos, *item_id, *amount)?;
        }
        OutboundAction::RequestUnitPayload { target } => {
            session.queue_request_unit_payload(*target)?;
        }
        OutboundAction::UnitClear => {
            session.queue_unit_clear()?;
        }
        OutboundAction::UnitControl { target } => {
            session.queue_unit_control(*target)?;
        }
        OutboundAction::UnitBuildingControlSelect { target, build_pos } => {
            session.queue_unit_building_control_select(*target, *build_pos)?;
        }
        OutboundAction::BuildingControlSelect { build_pos } => {
            session.queue_building_control_select(*build_pos)?;
        }
        OutboundAction::ClearItems { build_pos } => {
            session.queue_clear_items(*build_pos)?;
        }
        OutboundAction::ClearLiquids { build_pos } => {
            session.queue_clear_liquids(*build_pos)?;
        }
        OutboundAction::TransferInventory { build_pos } => {
            session.queue_transfer_inventory(*build_pos)?;
        }
        OutboundAction::RequestBuildPayload { build_pos } => {
            session.queue_request_build_payload(*build_pos)?;
        }
        OutboundAction::RequestDropPayload { x, y } => {
            session.queue_request_drop_payload(*x, *y)?;
        }
        OutboundAction::RotateBlock {
            build_pos,
            direction,
        } => {
            session.queue_rotate_block(*build_pos, *direction)?;
        }
        OutboundAction::DropItem { angle } => {
            session.queue_drop_item(*angle)?;
        }
        OutboundAction::TileConfig { build_pos, value } => {
            session.queue_tile_config(*build_pos, value.clone())?;
        }
        OutboundAction::TileTap { tile_pos } => {
            session.queue_tile_tap(*tile_pos)?;
        }
        OutboundAction::DeletePlans { positions } => {
            session.queue_delete_plans(positions)?;
        }
        OutboundAction::CommandBuilding { buildings, x, y } => {
            session.queue_command_building(buildings, *x, *y)?;
        }
        OutboundAction::CommandUnits {
            unit_ids,
            build_target,
            unit_target,
            pos_target,
            queue_command,
            final_batch,
        } => {
            if *final_batch && !unit_ids.is_empty() {
                session.queue_command_units_chunked(
                    unit_ids,
                    *build_target,
                    *unit_target,
                    *pos_target,
                    *queue_command,
                )?;
            } else {
                session.queue_command_units(
                    unit_ids,
                    *build_target,
                    *unit_target,
                    *pos_target,
                    *queue_command,
                    *final_batch,
                )?;
            }
        }
        OutboundAction::CommandUnitsChunked {
            unit_ids,
            build_target,
            unit_target,
            pos_target,
            queue_command,
        } => {
            session.queue_command_units_chunked(
                unit_ids,
                *build_target,
                *unit_target,
                *pos_target,
                *queue_command,
            )?;
        }
        OutboundAction::SetUnitCommand {
            unit_ids,
            command_id,
        } => {
            session.queue_set_unit_command(unit_ids, *command_id)?;
        }
        OutboundAction::SetUnitStance {
            unit_ids,
            stance_id,
            enable,
        } => {
            session.queue_set_unit_stance(unit_ids, *stance_id, *enable)?;
        }
        OutboundAction::BeginBreak {
            builder,
            team_id,
            x,
            y,
        } => {
            session.queue_begin_break(*builder, *team_id, *x, *y)?;
        }
        OutboundAction::BeginPlace {
            builder,
            block_id,
            team_id,
            x,
            y,
            rotation,
            place_config,
        } => {
            session.queue_begin_place(
                *builder,
                *block_id,
                *team_id,
                *x,
                *y,
                *rotation,
                place_config,
            )?;
        }
        OutboundAction::MenuChoose { menu_id, option } => {
            session.queue_menu_choose(*menu_id, *option)?;
        }
        OutboundAction::TextInputResult {
            text_input_id,
            text,
        } => {
            session.queue_text_input_result(*text_input_id, text.as_deref())?;
        }
        OutboundAction::ClientPacket {
            packet_type,
            contents,
            transport,
        } => {
            session.queue_client_packet(packet_type, contents, *transport)?;
        }
        OutboundAction::ClientBinaryPacket {
            packet_type,
            contents,
            transport,
        } => {
            session.queue_client_binary_packet(packet_type, contents, *transport)?;
        }
        OutboundAction::ClientLogicData {
            channel,
            value,
            transport,
        } => {
            session.queue_client_logic_data(channel, value, *transport)?;
        }
    }
    Ok(())
}

fn sync_command_mode_state_for_outbound_action(
    runtime_command_mode: &mut CommandModeState,
    action: &OutboundAction,
) {
    match action {
        OutboundAction::UnitClear => runtime_command_mode.record_unit_clear(),
        OutboundAction::UnitControl { target } => {
            if let Some(selected_unit_id) = standard_unit_id_from_client_ref(*target) {
                runtime_command_mode.record_unit_control(
                    command_unit_ref_from_client(*target),
                    &[selected_unit_id],
                );
            } else {
                runtime_command_mode
                    .record_unit_control(command_unit_ref_from_client(*target), &[]);
            }
        }
        OutboundAction::UnitBuildingControlSelect { target, build_pos } => {
            if let Some(selected_unit_id) = standard_unit_id_from_client_ref(*target) {
                runtime_command_mode.record_unit_building_control_select(
                    command_unit_ref_from_client(*target),
                    &[selected_unit_id],
                    *build_pos,
                );
            } else {
                runtime_command_mode.record_unit_building_control_select(
                    command_unit_ref_from_client(*target),
                    &[],
                    *build_pos,
                );
            }
        }
        OutboundAction::BuildingControlSelect { build_pos } => {
            runtime_command_mode.record_building_control_select(*build_pos);
        }
        OutboundAction::CommandBuilding { buildings, x, y } => {
            runtime_command_mode.record_command_building(buildings, (*x, *y));
        }
        OutboundAction::CommandUnits {
            unit_ids,
            build_target,
            unit_target,
            pos_target,
            ..
        }
        | OutboundAction::CommandUnitsChunked {
            unit_ids,
            build_target,
            unit_target,
            pos_target,
            ..
        } => runtime_command_mode.record_command_units(
            unit_ids,
            *build_target,
            command_unit_ref_from_client(*unit_target),
            *pos_target,
        ),
        OutboundAction::SetUnitCommand {
            unit_ids,
            command_id,
        } => runtime_command_mode.record_set_unit_command(unit_ids, *command_id),
        OutboundAction::SetUnitStance {
            unit_ids,
            stance_id,
            enable,
        } => runtime_command_mode.record_set_unit_stance(unit_ids, *stance_id, *enable),
        _ => {}
    }
}

fn command_unit_ref_from_client(target: ClientUnitRef) -> Option<CommandUnitRef> {
    match target {
        ClientUnitRef::None => None,
        ClientUnitRef::Block(value) => Some(CommandUnitRef { kind: 1, value }),
        ClientUnitRef::Standard(value) => Some(CommandUnitRef { kind: 2, value }),
    }
}

fn standard_unit_id_from_client_ref(target: ClientUnitRef) -> Option<i32> {
    match target {
        ClientUnitRef::Standard(value) => Some(value),
        _ => None,
    }
}

fn is_runtime_refresh_event(event: &ClientSessionEvent) -> bool {
    matches!(
        event,
        ClientSessionEvent::PlayerSpawned { .. }
            | ClientSessionEvent::PlayerPositionUpdated { .. }
            | ClientSessionEvent::SnapshotReceived(
                HighFrequencyRemoteMethod::EntitySnapshot
                    | HighFrequencyRemoteMethod::StateSnapshot
                    | HighFrequencyRemoteMethod::BlockSnapshot
                    | HighFrequencyRemoteMethod::HiddenSnapshot
            )
    )
}

fn is_runtime_build_plan_event(event: &ClientSessionEvent) -> bool {
    matches!(
        event,
        ClientSessionEvent::PlayerSpawned { .. } | ClientSessionEvent::PlayerPositionUpdated { .. }
    )
}

fn latest_build_plan_origin(events: &[ClientSessionEvent]) -> Option<(f32, f32)> {
    events.iter().rev().find_map(|event| match event {
        ClientSessionEvent::PlayerSpawned { x, y, .. }
        | ClientSessionEvent::PlayerPositionUpdated { x, y } => Some((*x, *y)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdt_client_min::bootstrap_flow::encode_world_stream_packets;
    use mdt_client_min::client_session::ClientSessionAction;

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

    fn sample_world_stream_bytes() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../../tests/src/test/resources/world-stream.hex"
        ))
    }

    fn ingest_sample_world(session: &mut ClientSession) {
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
    }

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    fn sample_args(extra: &[&str]) -> Vec<String> {
        let mut args = vec![
            "mdt-client-min-online".to_string(),
            "--manifest".to_string(),
            real_manifest_path().display().to_string(),
            "--server".to_string(),
            "127.0.0.1:6567".to_string(),
        ];
        args.extend(extra.iter().map(|value| (*value).to_string()));
        args
    }

    fn sample_discover_args(extra: &[&str]) -> Vec<String> {
        let mut args = vec![
            "mdt-client-min-online".to_string(),
            "--manifest".to_string(),
            real_manifest_path().display().to_string(),
            "--discover-host".to_string(),
            "127.0.0.1".to_string(),
        ];
        args.extend(extra.iter().map(|value| (*value).to_string()));
        args
    }

    #[test]
    fn parse_args_accepts_aim_pointer_pair() {
        let args = parse_args(sample_args(&[
            "--name", "aim-bot", "--aim-x", "123.5", "--aim-y", "-45.25",
        ]))
        .unwrap();

        assert_eq!(args.snapshot_pointer, Some((123.5, -45.25)));
        match args.connect {
            ConnectSource::Generated(spec) => assert_eq!(spec.name, "aim-bot"),
            ConnectSource::HexFile(path) => {
                panic!("expected generated connect source, got {path:?}")
            }
        }
    }

    #[test]
    fn parse_args_rejects_partial_aim_pointer_override() {
        let error = parse_args(sample_args(&["--aim-x", "1.0"]))
            .err()
            .expect("partial aim override should fail");

        assert!(error.contains("both --aim-x and --aim-y are required"));
    }

    #[test]
    fn parse_args_accepts_mine_tile() {
        let args = parse_args(sample_args(&["--mine-tile", "123:-45"])).unwrap();

        assert_eq!(args.snapshot_mining_tile, Some((123, -45)));
    }

    #[test]
    fn parse_args_rejects_invalid_mine_tile_format() {
        let error = parse_args(sample_args(&["--mine-tile", "123"]))
            .err()
            .expect("invalid mine tile format should fail");

        assert!(error.contains("invalid --mine-tile, expected <x:y>"));
    }

    #[test]
    fn parse_args_accepts_snapshot_state_overrides() {
        let args = parse_args(sample_args(&[
            "--snapshot-boosting",
            "--snapshot-no-shooting",
            "--snapshot-chatting",
            "--snapshot-no-building",
            "--view-size",
            "320.5:180.25",
            "--snapshot-interval-ms",
            "120",
        ]))
        .unwrap();

        assert_eq!(args.snapshot_boosting, Some(true));
        assert_eq!(args.snapshot_shooting, Some(false));
        assert_eq!(args.snapshot_chatting, Some(true));
        assert_eq!(args.snapshot_building, Some(false));
        assert_eq!(args.snapshot_view_size, Some((320.5, 180.25)));
        assert_eq!(args.snapshot_interval_ms, Some(120));
    }

    #[test]
    fn parse_args_rejects_invalid_view_size_format() {
        let error = parse_args(sample_args(&["--view-size", "320x180"]))
            .err()
            .expect("invalid view-size format should fail");

        assert!(error.contains("invalid --view-size, expected <x:y>"));
    }

    #[test]
    fn parse_args_rejects_invalid_snapshot_interval() {
        let error = parse_args(sample_args(&["--snapshot-interval-ms", "abc"]))
            .err()
            .expect("invalid snapshot interval should fail");

        assert!(error.contains("invalid --snapshot-interval-ms"));
    }

    #[test]
    fn parse_args_accepts_command_mode_bind_group() {
        let args = parse_args(sample_args(&["--command-mode-bind-group", "4@99,88,99"])).unwrap();

        assert_eq!(
            args.command_mode_ops,
            vec![CommandModeCliOp::BindGroup {
                index: 4,
                unit_ids: vec![99, 88, 99],
            }]
        );
    }

    #[test]
    fn parse_args_accepts_command_mode_recall_and_clear_group() {
        let args = parse_args(sample_args(&[
            "--command-mode-bind-group",
            "1@44,55",
            "--command-mode-recall-group",
            "1",
            "--command-mode-clear-group",
            "7",
        ]))
        .unwrap();

        assert_eq!(
            args.command_mode_ops,
            vec![
                CommandModeCliOp::BindGroup {
                    index: 1,
                    unit_ids: vec![44, 55],
                },
                CommandModeCliOp::RecallGroup { index: 1 },
                CommandModeCliOp::ClearGroup { index: 7 },
            ]
        );
    }

    #[test]
    fn parse_args_accepts_command_mode_rect() {
        let args = parse_args(sample_args(&[
            "--command-mode-rect",
            "1:2:3:4",
            "--command-mode-rect",
            "none",
        ]))
        .unwrap();

        assert_eq!(
            args.command_mode_ops,
            vec![
                CommandModeCliOp::SetRect(Some(CommandModeRectProjection {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                })),
                CommandModeCliOp::SetRect(None),
            ]
        );
    }

    #[test]
    fn parse_args_accepts_discovery_host_without_server() {
        let args = parse_args(sample_discover_args(&[])).unwrap();

        assert_eq!(
            args.server_target,
            ServerTarget::Discover(DiscoveryConfig {
                hosts: vec!["127.0.0.1".to_string()],
                port: DEFAULT_DISCOVER_PORT,
                timeout: Duration::from_millis(DEFAULT_DISCOVER_TIMEOUT_MS),
            })
        );
    }

    #[test]
    fn parse_args_rejects_missing_server_and_discovery() {
        let args = vec![
            "mdt-client-min-online".to_string(),
            "--manifest".to_string(),
            real_manifest_path().display().to_string(),
        ];

        let error = parse_args(args)
            .err()
            .expect("missing server/discovery should fail");

        assert!(error.contains("missing --server or --discover-host"));
    }

    #[test]
    fn parse_args_rejects_zero_discovery_timeout() {
        let error = parse_args(sample_discover_args(&["--discover-timeout-ms", "0"]))
            .err()
            .expect("zero discovery timeout should fail");

        assert!(error.contains("invalid --discover-timeout-ms"));
    }

    #[test]
    fn usage_mentions_snapshot_building_and_snapshot_interval_flags() {
        let text = usage();
        assert!(text.contains("--server <host:port> | --discover-host <host>"));
        assert!(text.contains("--discover-port <port>"));
        assert!(text.contains("--discover-timeout-ms <ms>"));
        assert!(text.contains("--snapshot-building|--snapshot-no-building"));
        assert!(text.contains("--snapshot-interval-ms <ms>"));
        assert!(text
            .contains("--intent-snapshot <moveX:moveY:aimX:aimY:actions[:mineX,mineY|none]> ..."));
        assert!(text.contains("--intent-live-sampling"));
        assert!(text.contains("--intent-edge-mapped"));
        assert!(text.contains("--intent-delay-ms <ms>"));
        assert!(text.contains("--intent-spacing-ms <ms>"));
        assert!(text.contains("--command-mode-bind-group <index@unitId[,unitId...]> ..."));
        assert!(text.contains("--command-mode-recall-group <index> ..."));
        assert!(text.contains("--command-mode-clear-group <index> ..."));
        assert!(text.contains("--command-mode-rect <x0:y0:x1:y1|none> ..."));
        assert!(text.contains("--print-client-packets"));
        assert!(text.contains("--watch-client-packet <type> ..."));
        assert!(text.contains("--watch-client-binary-packet <type> ..."));
        assert!(text.contains("--watch-client-logic-data <channel> ..."));
        assert!(text.contains("--consume-client-packet <type@semantic> ..."));
        assert!(text.contains("--consume-client-binary-packet <type@semantic> ..."));
        assert!(text.contains("--consume-client-logic-data <channel@semantic> ..."));
        assert!(text.contains("--relay-client-packet <inbound@outbound@reliable|unreliable> ..."));
        assert!(text
            .contains("--relay-client-binary-packet <inbound@outbound@reliable|unreliable> ..."));
        assert!(
            text.contains("--relay-client-logic-data <inbound@outbound@reliable|unreliable> ...")
        );
        assert!(text.contains("semantic=<server-message|chat-message|hud-text|announce|clipboard|open-uri|world-pos|build-pos|unit-id|team|bool|number>"));
        assert!(text.contains("--plan-rotate <x:y:dir>"));
        assert!(text.contains("--plan-edit-loop"));
        assert!(text.contains("--plan-edit-delay-ms <ms>"));
        assert!(text.contains("--plan-edit-spacing-ms <ms>"));
        assert!(text.contains("--plan-place <x:y:block[:rotation][;config]>"));
        assert!(text.contains("config=<none|int=<i32>|long=<i64>|float=<f32>|bool=<true|false|1|0>|int-seq=<i32[,i32...]>"));
        assert!(text.contains(
            "|tech-node-raw=<contentType:contentId>|double=<f64>|building-pos=<i32>|laccess=<i16>|"
        ));
        assert!(text.contains("|vec2-array=<x:y[,x:y...]>|vec2=<x:y>|team=<u8>|int-array=<i32[,i32...]>|object-array=<value[|value...]>|unit-command=<u16>>"));
        assert!(text.contains(
            "--plan-place-near-player <block[:rotation][;config]|selected[:rotation][;config]>",
        ));
        assert!(text.contains("--action-request-item <buildPos|none:itemId|none:amount>"));
        assert!(text.contains("--action-request-unit-payload <none|unit:<id>|block:<pos>|<id>>"));
        assert!(text.contains("--action-unit-clear"));
        assert!(text.contains("--action-unit-control <none|unit:<id>|block:<pos>|<id>>"));
        assert!(text.contains(
            "--action-unit-building-control-select <none|unit:<id>|block:<pos>|<id>@buildPos|none>",
        ));
        assert!(text.contains("--action-building-control-select <buildPos|none>"));
        assert!(text.contains("--action-clear-items <buildPos|none>"));
        assert!(text.contains("--action-clear-liquids <buildPos|none>"));
        assert!(text.contains("--action-transfer-inventory <buildPos|none>"));
        assert!(text.contains("--action-rotate-block <buildPos|none:direction>"));
        assert!(text.contains("--action-tile-config <buildPos|none:value>"));
        assert!(text.contains("--action-tile-tap <tilePos|none>"));
        assert!(text.contains("--action-delete-plans <x:y[,x:y...]|none>"));
        assert!(text.contains("--action-command-building <x:y[,x:y...]|none@x:y>"));
        assert!(text.contains("--action-command-units <unitId[,unitId...]|none@buildPos@unitTarget@x:y@queueCommand[@finalBatch]>"));
        assert!(text.contains("--action-set-unit-command <unitId[,unitId...]|none@commandId|none>"));
        assert!(text
            .contains("--action-set-unit-stance <unitId[,unitId...]|none@stanceId|none@enable>"));
        assert!(text.contains("--action-begin-break <none|unit:<id>|block:<pos>|<id>@teamId@x:y>"));
        assert!(text.contains("--action-begin-place <none|unit:<id>|block:<pos>|<id>@blockId|none@teamId@x:y@rotation@value>"));
        assert!(text.contains("--action-menu-choose <menuId@option>"));
        assert!(text.contains("--action-text-input-result <textInputId@text|none>"));
        assert!(text.contains("--action-client-packet <type@contents@reliable|unreliable>"));
        assert!(text.contains("--action-client-binary-packet <type@hex@reliable|unreliable>"));
        assert!(text.contains("--action-client-logic-data <channel@value@reliable|unreliable>"));
        assert!(text.contains("value=<null|int=<i32>|long=<i64>|float=<f32>|bool=<true|false|1|0>|int-seq=<i32[,i32...]>|string=<text>|content=<contentType:contentId>|tech-node-raw=<contentType:contentId>|point2=<x:y>|point2-array=<x:y[,x:y...]>|double=<f64>|building-pos=<i32>|laccess=<i16>|vec2=<x:y>|vec2-array=<x:y[,x:y...]>|team=<u8>|bytes=<hex>|legacy-unit-command-null=<u8>|bool-array=<bool[,bool...]>|unit-id=<i32>|int-array=<i32[,i32...]>|object-array=<value>|unit-command=<u16>|...>"));
    }

    #[test]
    fn resolve_session_timing_uses_default_when_not_overridden() {
        let args = parse_args(sample_args(&[])).unwrap();

        let timing = resolve_session_timing(&args);

        assert_eq!(
            timing.client_snapshot_interval_ms,
            ClientSessionTiming::default().client_snapshot_interval_ms
        );
    }

    #[test]
    fn resolve_session_timing_applies_snapshot_interval_override() {
        let args = parse_args(sample_args(&["--snapshot-interval-ms", "75"])).unwrap();

        let timing = resolve_session_timing(&args);

        assert_eq!(timing.client_snapshot_interval_ms, 75);
    }

    #[test]
    fn parse_args_leaves_live_intent_runtime_capture_disabled_by_default() {
        let args = parse_args(sample_args(&[])).unwrap();

        assert!(!args.enable_live_intent_runtime_capture);
    }

    #[test]
    fn parse_args_accepts_movement_probe_pair() {
        let args = parse_args(sample_args(&[
            "--name",
            "move-bot",
            "--move-step-x",
            "1.25",
            "--move-step-y",
            "-0.5",
        ]))
        .unwrap();

        assert_eq!(
            args.movement_probe,
            Some(MovementProbeConfig { step: (1.25, -0.5) })
        );
    }

    #[test]
    fn parse_args_rejects_partial_movement_probe() {
        let error = parse_args(sample_args(&["--move-step-x", "1.0"]))
            .err()
            .expect("partial movement probe should fail");

        assert!(error.contains("both --move-step-x and --move-step-y are required"));
    }

    #[test]
    fn parse_args_accepts_intent_snapshot_schedule() {
        let args = parse_args(sample_args(&[
            "--intent-delay-ms",
            "500",
            "--intent-spacing-ms",
            "250",
            "--intent-snapshot",
            "1:0:16:24:fire,boost:88,99",
            "--intent-snapshot",
            "0:0:32:48:none:none",
        ]))
        .unwrap();

        assert!(args.enable_live_intent_runtime_capture);
        assert_eq!(
            args.live_intent_sampling_mode,
            IntentSamplingMode::LiveSampling
        );
        assert_eq!(
            args.live_intent_schedule,
            vec![
                ScheduledIntentSnapshot {
                    not_before_ms: 500,
                    snapshot: InputSnapshot {
                        move_axis: (1.0, 0.0),
                        aim_axis: (16.0, 24.0),
                        mining_tile: Some((88, 99)),
                        building: false,
                        active_actions: vec![BinaryAction::Fire, BinaryAction::Boost],
                    },
                },
                ScheduledIntentSnapshot {
                    not_before_ms: 750,
                    snapshot: InputSnapshot {
                        move_axis: (0.0, 0.0),
                        aim_axis: (32.0, 48.0),
                        mining_tile: None,
                        building: false,
                        active_actions: vec![],
                    },
                },
            ]
        );
    }

    #[test]
    fn parse_intent_snapshot_accepts_legacy_use_and_pause_aliases() {
        let snapshot = parse_intent_snapshot_arg("0:1:16:24:pause,use:none").unwrap();

        assert_eq!(
            snapshot,
            InputSnapshot {
                move_axis: (0.0, 1.0),
                aim_axis: (16.0, 24.0),
                mining_tile: None,
                building: false,
                active_actions: vec![BinaryAction::Chat, BinaryAction::Interact],
            }
        );
    }

    #[test]
    fn parse_args_accepts_live_intent_sampling_flag() {
        let args = parse_args(sample_args(&["--intent-live-sampling"])).unwrap();

        assert!(args.enable_live_intent_runtime_capture);
        assert_eq!(
            args.live_intent_sampling_mode,
            IntentSamplingMode::LiveSampling
        );
    }

    #[test]
    fn parse_args_accepts_edge_mapped_intent_sampling_flag() {
        let args = parse_args(sample_args(&["--intent-edge-mapped"])).unwrap();

        assert!(args.enable_live_intent_runtime_capture);
        assert_eq!(
            args.live_intent_sampling_mode,
            IntentSamplingMode::EdgeMapped
        );
    }

    #[test]
    fn build_live_intent_mapper_returns_none_without_schedule_or_flag() {
        let args = parse_args(sample_args(&[])).unwrap();

        assert!(build_live_intent_mapper(&args).is_none());
    }

    #[test]
    fn build_live_intent_mapper_returns_runtime_capture_for_live_sampling_flag() {
        let args = parse_args(sample_args(&["--intent-live-sampling"])).unwrap();

        assert!(build_live_intent_mapper(&args).is_some());
    }

    #[test]
    fn parse_args_rejects_unknown_intent_action() {
        let error = parse_args(sample_args(&["--intent-snapshot", "1:0:16:24:jump"]))
            .err()
            .expect("unknown intent action should fail");

        assert!(error.contains("invalid --intent-snapshot action"));
    }

    #[test]
    fn parse_args_accepts_chat_message() {
        let args = parse_args(sample_args(&[
            "--name",
            "chat-bot",
            "--chat-delay-ms",
            "1250",
            "--chat-spacing-ms",
            "750",
            "--chat-message",
            "hello world",
            "--chat-message",
            "/sync",
        ]))
        .unwrap();

        assert_eq!(
            args.chat_schedule,
            vec![
                ScheduledChatEntry {
                    not_before_ms: 1_250,
                    text: "hello world".to_string(),
                },
                ScheduledChatEntry {
                    not_before_ms: 2_000,
                    text: "/sync".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_args_accepts_build_plan_flags() {
        let args = parse_args(sample_args(&[
            "--plan-place",
            "1:2:0x0101:3;point2=7:8",
            "--plan-break",
            "5:6",
        ]))
        .unwrap();

        assert_eq!(
            args.build_plans,
            vec![
                ClientBuildPlan {
                    tile: (1, 2),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 3,
                    config: ClientBuildPlanConfig::Point2 { x: 7, y: 8 },
                },
                ClientBuildPlan {
                    tile: (5, 6),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
            ]
        );
    }

    #[test]
    fn parse_args_accepts_relative_build_plan_flags() {
        let args = parse_args(sample_args(&[
            "--plan-place-relative",
            "1:0:0x0101:2;bytes=01020304",
            "--plan-break-relative",
            "-1:0",
        ]))
        .unwrap();

        assert_eq!(
            args.relative_build_plans,
            vec![
                RelativeBuildPlan {
                    tile_offset: (1, 0),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 2,
                    config: ClientBuildPlanConfig::Bytes(vec![1, 2, 3, 4]),
                },
                RelativeBuildPlan {
                    tile_offset: (-1, 0),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
            ]
        );
    }

    #[test]
    fn parse_args_accepts_extended_build_plan_config_types() {
        let args = parse_args(sample_args(&[
            "--plan-place",
            "1:1:0x0101;int=7",
            "--plan-place",
            "2:2:0x0102;bool=true",
            "--plan-place",
            "3:3:0x0103;content=1:0x0104",
            "--plan-place",
            "4:4:0x0105;point2-array=1:2,3:4",
            "--plan-place",
            "5:5:0x0106;unit-command=42",
        ]))
        .unwrap();

        assert_eq!(args.build_plans.len(), 5);
        assert_eq!(args.build_plans[0].config, ClientBuildPlanConfig::Int(7));
        assert_eq!(
            args.build_plans[1].config,
            ClientBuildPlanConfig::Bool(true)
        );
        assert_eq!(
            args.build_plans[2].config,
            ClientBuildPlanConfig::Content {
                content_type: 1,
                content_id: 0x0104,
            }
        );
        assert_eq!(
            args.build_plans[3].config,
            ClientBuildPlanConfig::Point2Array(vec![(1, 2), (3, 4)])
        );
        assert_eq!(
            args.build_plans[4].config,
            ClientBuildPlanConfig::UnitCommand(42)
        );
    }

    #[test]
    fn parse_args_applies_plan_edit_ops_to_absolute_and_relative_plans() {
        let args = parse_args(sample_args(&[
            "--plan-place",
            "2:1:0x0101:1;point2=3:2",
            "--plan-break",
            "9:9",
            "--plan-place-relative",
            "1:0:0x0102:1;point2=2:1",
            "--plan-break-relative",
            "5:6",
            "--plan-rotate",
            "0:0:1",
            "--plan-flip-x",
            "0:0",
        ]))
        .unwrap();

        assert_eq!(
            args.build_plans,
            vec![
                ClientBuildPlan {
                    tile: (1, 2),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Point2 { x: 2, y: 3 },
                },
                ClientBuildPlan {
                    tile: (9, 9),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
            ]
        );
        assert_eq!(
            args.relative_build_plans,
            vec![
                RelativeBuildPlan {
                    tile_offset: (0, 1),
                    breaking: false,
                    block_id: Some(0x0102),
                    rotation: 0,
                    config: ClientBuildPlanConfig::Point2 { x: 1, y: 2 },
                },
                RelativeBuildPlan {
                    tile_offset: (5, 6),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
            ]
        );
    }

    #[test]
    fn parse_args_applies_plan_edit_ops_to_point2_array_configs() {
        let args = parse_args(sample_args(&[
            "--plan-place",
            "2:1:0x0101:1;point2-array=1:0,2:1",
            "--plan-place-relative",
            "1:0:0x0102:1;point2-array=2:0,3:1",
            "--plan-rotate",
            "0:0:1",
            "--plan-flip-x",
            "0:0",
        ]))
        .unwrap();

        assert_eq!(
            args.build_plans[0].config,
            ClientBuildPlanConfig::Point2Array(vec![(0, 1), (1, 2)])
        );
        assert_eq!(
            args.relative_build_plans[0].config,
            ClientBuildPlanConfig::Point2Array(vec![(0, 2), (1, 3)])
        );
    }

    #[test]
    fn parse_args_accepts_runtime_plan_edit_loop_flags() {
        let args = parse_args(sample_args(&[
            "--plan-place",
            "1:0:0x0101:0",
            "--plan-rotate",
            "0:0:1",
            "--plan-edit-loop",
            "--plan-edit-delay-ms",
            "250",
            "--plan-edit-spacing-ms",
            "400",
        ]))
        .unwrap();

        assert_eq!(
            args.runtime_plan_edit_loop,
            Some(PlanEditLoopConfig {
                ops: vec![PlanEditOp::Rotate {
                    origin: (0, 0),
                    direction: 1,
                }],
                delay_ms: 250,
                spacing_ms: 400,
            })
        );
    }

    #[test]
    fn parse_args_rejects_runtime_plan_edit_loop_with_zero_spacing() {
        let error = parse_args(sample_args(&[
            "--plan-rotate",
            "0:0:1",
            "--plan-edit-loop",
            "--plan-edit-spacing-ms",
            "0",
        ]))
        .err()
        .expect("zero runtime plan-edit spacing should fail");

        assert!(error.contains("--plan-edit-spacing-ms must be positive"));
    }

    #[test]
    fn parse_args_rejects_plan_rotate_with_zero_direction() {
        let error = parse_args(sample_args(&["--plan-rotate", "0:0:0"]))
            .err()
            .expect("zero direction should fail");

        assert!(error.contains("--plan-rotate direction must be non-zero"));
    }

    #[test]
    fn parse_args_rejects_invalid_plan_place_config() {
        let error = parse_args(sample_args(&["--plan-place", "1:2:0x0101;unsupported=1"]))
            .err()
            .expect("invalid plan config should fail");

        assert!(error.contains("invalid --plan-place config"));
    }

    #[test]
    fn parse_args_accepts_auto_build_plan_flags() {
        let args = parse_args(sample_args(&[
            "--plan-break-near-player",
            "--plan-place-near-player",
            "selected:2;string=router",
            "--plan-place-conflict-near-player",
            "0x0102:1;point2=3:4",
        ]))
        .unwrap();

        assert!(args.auto_break_near_player);
        assert_eq!(
            args.auto_place_near_player,
            vec![AutoPlacePlan {
                block: AutoBlockChoice::Selected,
                rotation: Some(2),
                config: ClientBuildPlanConfig::String("router".to_string()),
            }]
        );
        assert_eq!(
            args.auto_place_conflict_near_player,
            vec![AutoPlacePlan {
                block: AutoBlockChoice::Fixed(0x0102),
                rotation: Some(1),
                config: ClientBuildPlanConfig::Point2 { x: 3, y: 4 },
            }]
        );
    }

    #[test]
    fn parse_args_rejects_invalid_auto_place_config() {
        let error = parse_args(sample_args(&[
            "--plan-place-near-player",
            "selected;unsupported=1",
        ]))
        .err()
        .expect("invalid auto-place config should fail");

        assert!(error.contains("invalid --plan-place-near-player config"));
    }

    #[test]
    fn parse_args_accepts_render_ascii_flag() {
        let args = parse_args(sample_args(&["--render-ascii-on-world-ready"])).unwrap();

        assert!(args.render_ascii_on_world_ready);
    }

    #[test]
    fn parse_args_accepts_print_client_packets_flag() {
        let args = parse_args(sample_args(&["--print-client-packets"])).unwrap();

        assert!(args.print_client_packets);
    }

    #[test]
    fn parse_args_accepts_custom_packet_watch_flags() {
        let args = parse_args(sample_args(&[
            "--watch-client-packet",
            "custom.alpha",
            "--watch-client-packet",
            "custom.alpha",
            "--watch-client-binary-packet",
            "custom.alpha",
            "--watch-client-binary-packet",
            "custom.beta",
            "--watch-client-logic-data",
            "logic.alpha",
            "--watch-client-logic-data",
            "logic.alpha",
        ]))
        .unwrap();

        assert_eq!(
            args.watched_client_packet_types,
            vec!["custom.alpha".to_string(), "custom.alpha".to_string()]
        );
        assert_eq!(
            args.watched_client_binary_packet_types,
            vec!["custom.alpha".to_string(), "custom.beta".to_string()]
        );
        assert_eq!(
            args.watched_client_logic_data_channels,
            vec!["logic.alpha".to_string(), "logic.alpha".to_string()]
        );
    }

    #[test]
    fn parse_args_accepts_custom_packet_semantic_flags() {
        let args = parse_args(sample_args(&[
            "--consume-client-packet",
            "custom.status@hud-text",
            "--consume-client-binary-packet",
            "custom.uri@open-uri",
            "--consume-client-logic-data",
            "logic.pos@world-pos",
        ]))
        .unwrap();

        assert_eq!(
            args.runtime_custom_packet_semantics,
            vec![
                RuntimeCustomPacketSemanticSpec {
                    key: "custom.status".to_string(),
                    encoding: custom_packet_runtime::RuntimeCustomPacketSemanticEncoding::Text,
                    semantic: custom_packet_runtime::RuntimeCustomPacketSemanticKind::HudText,
                },
                RuntimeCustomPacketSemanticSpec {
                    key: "custom.uri".to_string(),
                    encoding: custom_packet_runtime::RuntimeCustomPacketSemanticEncoding::Binary,
                    semantic: custom_packet_runtime::RuntimeCustomPacketSemanticKind::OpenUri,
                },
                RuntimeCustomPacketSemanticSpec {
                    key: "logic.pos".to_string(),
                    encoding: custom_packet_runtime::RuntimeCustomPacketSemanticEncoding::LogicData,
                    semantic: custom_packet_runtime::RuntimeCustomPacketSemanticKind::WorldPos,
                },
            ]
        );
    }

    #[test]
    fn parse_args_accepts_runtime_custom_packet_relay_flags() {
        let args = parse_args(sample_args(&[
            "--relay-client-packet",
            "custom.ping@custom.pong@tcp",
            "--relay-client-binary-packet",
            "bin.ping@bin.pong@udp",
            "--relay-client-logic-data",
            "logic.ping@logic.pong@reliable",
        ]))
        .unwrap();

        assert_eq!(
            args.runtime_custom_packet_relays,
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
    fn parse_args_accepts_render_window_flag() {
        let args = parse_args(sample_args(&["--render-window-live"])).unwrap();

        assert!(args.render_window_live);
    }

    #[test]
    fn parse_args_accepts_world_stream_dump_path() {
        let args = parse_args(sample_args(&[
            "--dump-world-stream-hex",
            "build/profile/archipelago-world.hex",
        ]))
        .unwrap();

        assert_eq!(
            args.dump_world_stream_hex,
            Some(PathBuf::from("build/profile/archipelago-world.hex"))
        );
    }

    #[test]
    fn parse_args_accepts_outbound_action_queue_flags() {
        let args = parse_args(sample_args(&[
            "--action-delay-ms",
            "500",
            "--action-spacing-ms",
            "200",
            "--action-request-item",
            "222:0x0009:15",
            "--action-request-unit-payload",
            "unit:444",
            "--action-delete-plans",
            "1:2,-3:4",
            "--action-unit-clear",
            "--action-unit-control",
            "block:111",
            "--action-unit-building-control-select",
            "unit:222@333",
            "--action-building-control-select",
            "555",
            "--action-clear-items",
            "666",
            "--action-clear-liquids",
            "667",
            "--action-transfer-inventory",
            "321",
            "--action-request-build-payload",
            "none",
            "--action-request-drop-payload",
            "12.5:48.0",
            "--action-rotate-block",
            "777:true",
            "--action-drop-item",
            "135.0",
            "--action-tile-config",
            "888:object-array=int=7|string=router|bool=true|point2=3:4|null",
            "--action-tile-tap",
            "999",
            "--action-command-building",
            "5:6,-7:8@12.5:-4.0",
            "--action-command-units",
            "111,222@333@unit:444@48.0:96.0@true@false",
            "--action-set-unit-command",
            "333,444@12",
            "--action-set-unit-stance",
            "555,666@7@false",
            "--action-begin-break",
            "unit:777@8@-11:22",
            "--action-begin-place",
            "block:888@999@3@44:-55@2@point2=7:-8",
            "--action-menu-choose",
            "12@-1",
            "--action-text-input-result",
            "9@router",
        ]))
        .unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![
                ScheduledOutboundAction {
                    not_before_ms: 500,
                    action: OutboundAction::RequestItem {
                        build_pos: Some(222),
                        item_id: Some(9),
                        amount: 15,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 700,
                    action: OutboundAction::RequestUnitPayload {
                        target: ClientUnitRef::Standard(444),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 900,
                    action: OutboundAction::DeletePlans {
                        positions: vec![pack_point2(1, 2), pack_point2(-3, 4)],
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 1_100,
                    action: OutboundAction::UnitClear,
                },
                ScheduledOutboundAction {
                    not_before_ms: 1_300,
                    action: OutboundAction::UnitControl {
                        target: ClientUnitRef::Block(111),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 1_500,
                    action: OutboundAction::UnitBuildingControlSelect {
                        target: ClientUnitRef::Standard(222),
                        build_pos: Some(333),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 1_700,
                    action: OutboundAction::BuildingControlSelect {
                        build_pos: Some(555),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 1_900,
                    action: OutboundAction::ClearItems {
                        build_pos: Some(666),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 2_100,
                    action: OutboundAction::ClearLiquids {
                        build_pos: Some(667),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 2_300,
                    action: OutboundAction::TransferInventory {
                        build_pos: Some(321),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 2_500,
                    action: OutboundAction::RequestBuildPayload { build_pos: None },
                },
                ScheduledOutboundAction {
                    not_before_ms: 2_700,
                    action: OutboundAction::RequestDropPayload { x: 12.5, y: 48.0 },
                },
                ScheduledOutboundAction {
                    not_before_ms: 2_900,
                    action: OutboundAction::RotateBlock {
                        build_pos: Some(777),
                        direction: true,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 3_100,
                    action: OutboundAction::DropItem { angle: 135.0 },
                },
                ScheduledOutboundAction {
                    not_before_ms: 3_300,
                    action: OutboundAction::TileConfig {
                        build_pos: Some(888),
                        value: TypeIoObject::ObjectArray(vec![
                            TypeIoObject::Int(7),
                            TypeIoObject::String(Some("router".to_string())),
                            TypeIoObject::Bool(true),
                            TypeIoObject::Point2 { x: 3, y: 4 },
                            TypeIoObject::Null,
                        ]),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 3_500,
                    action: OutboundAction::TileTap {
                        tile_pos: Some(999),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 3_700,
                    action: OutboundAction::CommandBuilding {
                        buildings: vec![pack_point2(5, 6), pack_point2(-7, 8)],
                        x: 12.5,
                        y: -4.0,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 3_900,
                    action: OutboundAction::CommandUnits {
                        unit_ids: vec![111, 222],
                        build_target: Some(333),
                        unit_target: ClientUnitRef::Standard(444),
                        pos_target: Some((48.0, 96.0)),
                        queue_command: true,
                        final_batch: false,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 4_100,
                    action: OutboundAction::SetUnitCommand {
                        unit_ids: vec![333, 444],
                        command_id: Some(12),
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 4_300,
                    action: OutboundAction::SetUnitStance {
                        unit_ids: vec![555, 666],
                        stance_id: Some(7),
                        enable: false,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 4_500,
                    action: OutboundAction::BeginBreak {
                        builder: ClientUnitRef::Standard(777),
                        team_id: 8,
                        x: -11,
                        y: 22,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 4_700,
                    action: OutboundAction::BeginPlace {
                        builder: ClientUnitRef::Block(888),
                        block_id: Some(999),
                        team_id: 3,
                        x: 44,
                        y: -55,
                        rotation: 2,
                        place_config: TypeIoObject::Point2 { x: 7, y: -8 },
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 4_900,
                    action: OutboundAction::MenuChoose {
                        menu_id: 12,
                        option: -1,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 5_100,
                    action: OutboundAction::TextInputResult {
                        text_input_id: 9,
                        text: Some("router".to_string()),
                    },
                },
            ]
        );
    }

    #[test]
    fn parse_args_accepts_custom_and_logic_outbound_action_flags() {
        let args = parse_args(sample_args(&[
            "--action-client-packet",
            "custom.text@hello world@reliable",
            "--action-client-binary-packet",
            "custom.bin@aabbcc@udp",
            "--action-client-logic-data",
            "logic.alpha@int=7@unreliable",
        ]))
        .unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![
                ScheduledOutboundAction {
                    not_before_ms: 1_000,
                    action: OutboundAction::ClientPacket {
                        packet_type: "custom.text".to_string(),
                        contents: "hello world".to_string(),
                        transport: ClientPacketTransport::Tcp,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 2_000,
                    action: OutboundAction::ClientBinaryPacket {
                        packet_type: "custom.bin".to_string(),
                        contents: vec![0xAA, 0xBB, 0xCC],
                        transport: ClientPacketTransport::Udp,
                    },
                },
                ScheduledOutboundAction {
                    not_before_ms: 3_000,
                    action: OutboundAction::ClientLogicData {
                        channel: "logic.alpha".to_string(),
                        value: TypeIoObject::Int(7),
                        transport: ClientLogicDataTransport::Unreliable,
                    },
                },
            ]
        );
    }

    #[test]
    fn parse_args_rejects_invalid_action_request_item_flag() {
        let error = parse_args(sample_args(&["--action-request-item", "1:2"]))
            .err()
            .expect("invalid request-item format should fail");

        assert!(error.contains("invalid --action-request-item"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_request_unit_payload_flag() {
        let error = parse_args(sample_args(&["--action-request-unit-payload", "player:1"]))
            .err()
            .expect("invalid request-unit-payload kind should fail");

        assert!(error.contains("invalid --action-request-unit-payload"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_set_unit_command_flag() {
        let error = parse_args(sample_args(&["--action-set-unit-command", "1,2"]))
            .err()
            .expect("invalid set-unit-command format should fail");

        assert!(error.contains("invalid --action-set-unit-command"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_set_unit_stance_flag() {
        let error = parse_args(sample_args(&["--action-set-unit-stance", "1,2@7"]))
            .err()
            .expect("invalid set-unit-stance format should fail");

        assert!(error.contains("invalid --action-set-unit-stance"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_begin_break_flag() {
        let error = parse_args(sample_args(&["--action-begin-break", "unit:7@3"]))
            .err()
            .expect("invalid begin-break format should fail");

        assert!(error.contains("invalid --action-begin-break"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_begin_place_flag() {
        let error = parse_args(sample_args(&["--action-begin-place", "unit:7@9@3@4:5@2"]))
            .err()
            .expect("invalid begin-place format should fail");

        assert!(error.contains("invalid --action-begin-place"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_menu_choose_flag() {
        let error = parse_args(sample_args(&["--action-menu-choose", "12"]))
            .err()
            .expect("invalid menu-choose format should fail");

        assert!(error.contains("invalid --action-menu-choose"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_text_input_result_flag() {
        let error = parse_args(sample_args(&["--action-text-input-result", "9"]))
            .err()
            .expect("invalid text-input-result format should fail");

        assert!(error.contains("invalid --action-text-input-result"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_rotate_block_flag() {
        let error = parse_args(sample_args(&["--action-rotate-block", "1:clockwise"]))
            .err()
            .expect("invalid rotate-block direction should fail");

        assert!(error.contains("invalid --action-rotate-block direction"));
    }

    #[test]
    fn parse_args_accepts_action_tile_config_bytes_value() {
        let args = parse_args(sample_args(&["--action-tile-config", "1:bytes=0102a0ff"])).unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![ScheduledOutboundAction {
                not_before_ms: 1_000,
                action: OutboundAction::TileConfig {
                    build_pos: Some(1),
                    value: TypeIoObject::Bytes(vec![0x01, 0x02, 0xA0, 0xFF]),
                },
            }]
        );
    }

    #[test]
    fn parse_args_accepts_action_delete_plans_none() {
        let args = parse_args(sample_args(&["--action-delete-plans", "none"])).unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![ScheduledOutboundAction {
                not_before_ms: 1_000,
                action: OutboundAction::DeletePlans {
                    positions: Vec::new(),
                },
            }]
        );
    }

    #[test]
    fn parse_args_accepts_action_command_building_none_list() {
        let args = parse_args(sample_args(&[
            "--action-command-building",
            "none@12.5:-4.0",
        ]))
        .unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![ScheduledOutboundAction {
                not_before_ms: 1_000,
                action: OutboundAction::CommandBuilding {
                    buildings: Vec::new(),
                    x: 12.5,
                    y: -4.0,
                },
            }]
        );
    }

    #[test]
    fn parse_args_accepts_action_command_units_none_targets() {
        let args = parse_args(sample_args(&[
            "--action-command-units",
            "none@none@none@none@1@0",
        ]))
        .unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![ScheduledOutboundAction {
                not_before_ms: 1_000,
                action: OutboundAction::CommandUnits {
                    unit_ids: Vec::new(),
                    build_target: None,
                    unit_target: ClientUnitRef::None,
                    pos_target: None,
                    queue_command: true,
                    final_batch: false,
                },
            }]
        );
    }

    #[test]
    fn parse_args_accepts_action_command_units_chunked_form() {
        let args = parse_args(sample_args(&[
            "--action-command-units",
            "1,2,3@none@none@48:96@1",
        ]))
        .unwrap();

        assert_eq!(
            args.outbound_action_schedule,
            vec![ScheduledOutboundAction {
                not_before_ms: 1_000,
                action: OutboundAction::CommandUnitsChunked {
                    unit_ids: vec![1, 2, 3],
                    build_target: None,
                    unit_target: ClientUnitRef::None,
                    pos_target: Some((48.0, 96.0)),
                    queue_command: true,
                },
            }]
        );
    }

    #[test]
    fn parse_typeio_object_subset_arg_accepts_extended_supported_types() {
        assert_eq!(
            parse_typeio_object_subset_arg("--flag", "long=922337203685477580").unwrap(),
            TypeIoObject::Long(922337203685477580)
        );
        assert_eq!(
            parse_typeio_object_subset_arg("--flag", "float=12.5").unwrap(),
            TypeIoObject::Float(12.5)
        );
        assert_eq!(
            parse_typeio_object_subset_arg("--flag", "vec2=-2.5:4.25").unwrap(),
            TypeIoObject::Vec2 { x: -2.5, y: 4.25 }
        );
        assert_eq!(
            parse_typeio_object_subset_arg("--flag", "team=7").unwrap(),
            TypeIoObject::Team(7)
        );
        assert_eq!(
            parse_typeio_object_subset_arg("--flag", "int-array=1,-2,3").unwrap(),
            TypeIoObject::IntArray(vec![1, -2, 3])
        );
    }

    #[test]
    fn parse_typeio_object_subset_arg_rejects_invalid_int_array_items() {
        let error = parse_typeio_object_subset_arg("--flag", "int-array=1,,3")
            .err()
            .expect("empty int-array item should fail");

        assert!(error.contains("empty int-array item"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_tile_config_flag() {
        let error = parse_args(sample_args(&["--action-tile-config", "1:bytes=xyz"]))
            .err()
            .expect("invalid tile-config hex should fail");

        assert!(error.contains("invalid hex") || error.contains("hex payload length"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_delete_plans_flag() {
        let error = parse_args(sample_args(&["--action-delete-plans", "1:2,3"]))
            .err()
            .expect("invalid delete-plans list should fail");

        assert!(error.contains("invalid --action-delete-plans"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_unit_building_control_select_flag() {
        let error = parse_args(sample_args(&[
            "--action-unit-building-control-select",
            "unit:1:2",
        ]))
        .err()
        .expect("invalid unit-building-control-select format should fail");

        assert!(error.contains("invalid --action-unit-building-control-select"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_command_building_flag() {
        let error = parse_args(sample_args(&["--action-command-building", "1:2,3:4"]))
            .err()
            .expect("invalid command-building format should fail");

        assert!(error.contains("invalid --action-command-building"));
    }

    #[test]
    fn parse_args_rejects_invalid_action_command_units_flag() {
        let error = parse_args(sample_args(&["--action-command-units", "1,2@3@unit:4@5:6"]))
            .err()
            .expect("invalid command-units format should fail");

        assert!(error.contains("invalid --action-command-units"));
    }

    #[test]
    fn build_runtime_custom_packet_watch_specs_deduplicates_by_transport() {
        let args = parse_args(sample_args(&[
            "--watch-client-packet",
            "custom.alpha",
            "--watch-client-packet",
            "custom.alpha",
            "--watch-client-binary-packet",
            "custom.alpha",
            "--watch-client-binary-packet",
            "custom.alpha",
            "--watch-client-binary-packet",
            "custom.beta",
            "--watch-client-logic-data",
            "logic.alpha",
            "--watch-client-logic-data",
            "logic.alpha",
        ]))
        .unwrap();

        assert_eq!(
            build_runtime_custom_packet_watch_specs(&args),
            vec![
                CustomPacketWatchSpec {
                    packet_type: "custom.alpha".to_string(),
                    encoding: CustomPacketWatchEncoding::Text,
                },
                CustomPacketWatchSpec {
                    packet_type: "custom.alpha".to_string(),
                    encoding: CustomPacketWatchEncoding::Binary,
                },
                CustomPacketWatchSpec {
                    packet_type: "custom.beta".to_string(),
                    encoding: CustomPacketWatchEncoding::Binary,
                },
                CustomPacketWatchSpec {
                    packet_type: "logic.alpha".to_string(),
                    encoding: CustomPacketWatchEncoding::LogicData,
                },
            ]
        );
    }

    #[test]
    fn runtime_custom_packet_watch_state_tracks_counts_and_logs() {
        let mut state = RuntimeCustomPacketWatchState::default();
        state.register_text_type("custom.text");
        state.register_binary_type("custom.bin");
        state.register_logic_data_channel("logic.alpha");

        state.record_text_handler("custom.text", "line\none");
        state.record_text_handler("custom.text", "line two");
        state.record_binary_handler("custom.bin", &[0xAA, 0xBB, 0xCC]);
        state.record_logic_data_handler(
            "logic.alpha",
            ClientLogicDataTransport::Reliable,
            &TypeIoObject::Int(7),
        );
        state.observe_events(&[
            ClientSessionEvent::ServerPacketReliable {
                packet_type: "custom.text".to_string(),
                contents: "line\none".to_string(),
            },
            ClientSessionEvent::ServerPacketUnreliable {
                packet_type: "custom.text".to_string(),
                contents: "line two".to_string(),
            },
            ClientSessionEvent::ServerBinaryPacketReliable {
                packet_type: "custom.bin".to_string(),
                contents: vec![0xAA, 0xBB, 0xCC],
            },
            ClientSessionEvent::ClientLogicDataReliable {
                channel: "logic.alpha".to_string(),
                value: TypeIoObject::Int(7),
            },
            ClientSessionEvent::ClientPacketReliable {
                packet_type: "ignored.text".to_string(),
                contents: "ignored".to_string(),
            },
        ]);

        let logs = state.drain_lines();
        assert_eq!(logs.len(), 4);
        assert!(logs[0].contains("client_packet_handler:"));
        assert!(logs[0].contains("type=\"custom.text\""));
        assert!(logs[0].contains("count=1"));
        assert!(logs[1].contains("count=2"));
        assert!(logs[2].contains("client_binary_packet_handler:"));
        assert!(logs[2].contains("type=\"custom.bin\""));
        assert!(logs[2].contains("count=1"));
        assert!(logs[2].contains("hex_prefix=aabbcc"));
        assert!(logs[3].contains("client_logic_data_handler:"));
        assert!(logs[3].contains("channel=\"logic.alpha\""));
        assert!(logs[3].contains("count=1"));
        assert!(logs[3].contains("transport=reliable"));

        let summaries = state.summary_lines();
        assert_eq!(summaries.len(), 3);
        assert!(summaries[0].contains("client_packet_handler_summary:"));
        assert!(summaries[0].contains("type=\"custom.text\""));
        assert!(summaries[0].contains("count=2"));
        assert!(summaries[0].contains("event_reliable=1"));
        assert!(summaries[0].contains("event_unreliable=1"));
        assert!(summaries[0].contains("event_total=2"));
        assert!(summaries[0].contains("parity=ok"));
        assert!(summaries[1].contains("client_binary_packet_handler_summary:"));
        assert!(summaries[1].contains("type=\"custom.bin\""));
        assert!(summaries[1].contains("count=1"));
        assert!(summaries[1].contains("event_reliable=1"));
        assert!(summaries[1].contains("event_unreliable=0"));
        assert!(summaries[1].contains("event_total=1"));
        assert!(summaries[1].contains("parity=ok"));
        assert!(summaries[2].contains("client_logic_data_handler_summary:"));
        assert!(summaries[2].contains("channel=\"logic.alpha\""));
        assert!(summaries[2].contains("count=1"));
        assert!(summaries[2].contains("event_reliable=1"));
        assert!(summaries[2].contains("event_unreliable=0"));
        assert!(summaries[2].contains("event_total=1"));
        assert!(summaries[2].contains("parity=ok"));
    }

    #[test]
    fn summarize_client_packet_events_formats_text_and_binary_variants() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::ClientPacketReliable {
                packet_type: "custom.text.r".to_string(),
                contents: "line\none".to_string(),
            },
            ClientSessionEvent::ClientPacketUnreliable {
                packet_type: "custom.text.u".to_string(),
                contents: "hello".to_string(),
            },
            ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type: "custom.bin.r".to_string(),
                contents: (0u8..20u8).collect(),
            },
            ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type: "custom.bin.u".to_string(),
                contents: vec![0xAA, 0xBB, 0xCC],
            },
            ClientSessionEvent::ServerPacketReliable {
                packet_type: "server.text.r".to_string(),
                contents: "server".to_string(),
            },
            ClientSessionEvent::ServerBinaryPacketUnreliable {
                packet_type: "server.bin.u".to_string(),
                contents: vec![0x10, 0x20],
            },
            ClientSessionEvent::ClientLogicDataReliable {
                channel: "logic.r".to_string(),
                value: TypeIoObject::String(Some("hello".to_string())),
            },
            ClientSessionEvent::ClientLogicDataUnreliable {
                channel: "logic.u".to_string(),
                value: TypeIoObject::ObjectArray(vec![
                    TypeIoObject::Int(7),
                    TypeIoObject::Bool(true),
                ]),
            },
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot),
        ]);

        assert_eq!(lines.len(), 8);
        assert!(lines[0].contains("client_packet: transport=reliable"));
        assert!(lines[0].contains("type=\"custom.text.r\""));
        assert!(lines[0].contains("len=8"));
        assert!(lines[0].contains("preview=\"line\\\\none\""));
        assert!(lines[1].contains("client_packet: transport=unreliable"));
        assert!(lines[1].contains("type=\"custom.text.u\""));
        assert!(lines[1].contains("len=5"));
        assert!(lines[2].contains("client_binary_packet: transport=reliable"));
        assert!(lines[2].contains("type=\"custom.bin.r\""));
        assert!(lines[2].contains("len=20"));
        assert!(lines[2].contains("hex_prefix=000102030405060708090a0b0c0d0e0f"));
        assert!(lines[3].contains("client_binary_packet: transport=unreliable"));
        assert!(lines[3].contains("type=\"custom.bin.u\""));
        assert!(lines[3].contains("len=3"));
        assert!(lines[3].contains("hex_prefix=aabbcc"));
        assert!(lines[4].contains("server_packet: transport=reliable"));
        assert!(lines[4].contains("type=\"server.text.r\""));
        assert!(lines[4].contains("len=6"));
        assert!(lines[5].contains("server_binary_packet: transport=unreliable"));
        assert!(lines[5].contains("type=\"server.bin.u\""));
        assert!(lines[5].contains("len=2"));
        assert!(lines[5].contains("hex_prefix=1020"));
        assert!(lines[6].contains("client_logic_data: transport=reliable"));
        assert!(lines[6].contains("channel=\"logic.r\""));
        assert!(lines[6].contains("kind=\"string\""));
        assert!(lines[6].contains("String(Some(\\\"hello\\\"))"));
        assert!(lines[7].contains("client_logic_data: transport=unreliable"));
        assert!(lines[7].contains("channel=\"logic.u\""));
        assert!(lines[7].contains("kind=\"object[]\""));
        assert!(lines[7].contains("ObjectArray([Int(7), Bool(true)])"));
    }

    #[test]
    fn summarize_client_packet_events_includes_tile_config_observability() {
        let lines = summarize_client_packet_events(&[ClientSessionEvent::TileConfig {
            build_pos: Some(123),
            config_kind: Some(99),
            config_kind_name: Some("unsupported(99)".to_string()),
            parse_failed: true,
            business_applied: false,
            cleared_pending_local: false,
            was_rollback: false,
            pending_local_match: None,
            configured_block_outcome: None,
            configured_block_name: None,
        }]);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("tile_config:"));
        assert!(lines[0].contains("build_pos=Some(123)"));
        assert!(lines[0].contains("kind=Some(99)"));
        assert!(lines[0].contains("kind_name=Some(\"unsupported(99)\")"));
        assert!(lines[0].contains("parse_failed=true"));
        assert!(lines[0].contains("business_applied=false"));
        assert!(lines[0].contains("cleared_pending_local=false"));
        assert!(lines[0].contains("rollback=false"));
        assert!(lines[0].contains("pending_local_match=None"));
        assert!(lines[0].contains("configured_outcome=None"));
        assert!(lines[0].contains("configured_block=None"));
    }

    #[test]
    fn summarize_client_packet_events_includes_info_popup_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::InfoPopup {
                reliable: false,
                popup_id: None,
                message: Some("hello".to_string()),
                duration: 1.5,
                align: 2,
                top: 3,
                left: 4,
                bottom: 5,
                right: 6,
            },
            ClientSessionEvent::InfoPopup {
                reliable: true,
                popup_id: Some("popup-id".to_string()),
                message: Some("world".to_string()),
                duration: 2.25,
                align: 7,
                top: 8,
                left: 9,
                bottom: 10,
                right: 11,
            },
        ]);

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("info_popup:"));
        assert!(lines[0].contains("reliable=false"));
        assert!(lines[0].contains("popup_id=None"));
        assert!(lines[0].contains("Some(\"hello\")"));
        assert!(lines[0].contains("0x3fc00000"));
        assert!(lines[1].contains("info_popup:"));
        assert!(lines[1].contains("reliable=true"));
        assert!(lines[1].contains("Some(\"popup-id\")"));
        assert!(lines[1].contains("Some(\"world\")"));
        assert!(lines[1].contains("0x40100000"));
    }

    #[test]
    fn summarize_client_packet_events_includes_audio_and_admin_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::SoundRequested {
                sound_id: Some(7),
                volume: 1.25,
                pitch: 0.5,
                pan: -0.25,
            },
            ClientSessionEvent::SoundAtRequested {
                sound_id: Some(11),
                x: 10.0,
                y: -3.0,
                volume: 0.75,
                pitch: 1.5,
            },
            ClientSessionEvent::TraceInfoReceived {
                player_id: Some(123456),
                ip: Some("127.0.0.1".to_string()),
                uuid: Some("uuid-1".to_string()),
                locale: Some("en_US".to_string()),
                modded: true,
                mobile: false,
                times_joined: 7,
                times_kicked: 2,
                ips: vec!["127.0.0.1".to_string(), "10.0.0.2".to_string()],
                names: vec!["alpha".to_string()],
            },
            ClientSessionEvent::DebugStatusReceived {
                reliable: true,
                value: 7,
                last_client_snapshot: 202,
                snapshots_sent: 303,
            },
            ClientSessionEvent::DebugStatusReceived {
                reliable: false,
                value: 12,
                last_client_snapshot: 404,
                snapshots_sent: 505,
            },
        ]);

        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("sound:"));
        assert!(lines[0].contains("sound_id=Some(7)"));
        assert!(lines[0].contains("0x3fa00000"));
        assert!(lines[1].contains("sound_at:"));
        assert!(lines[1].contains("sound_id=Some(11)"));
        assert!(lines[1].contains("0x41200000"));
        assert!(lines[2].contains("trace_info:"));
        assert!(lines[2].contains("player_id=Some(123456)"));
        assert!(lines[2].contains("Some(\"127.0.0.1\")"));
        assert!(lines[2].contains("ips=2"));
        assert!(lines[2].contains("names=1"));
        assert!(lines[3].contains("debug_status:"));
        assert!(lines[3].contains("reliable=true"));
        assert!(lines[3].contains("snapshots_sent=303"));
        assert!(lines[4].contains("debug_status:"));
        assert!(lines[4].contains("reliable=false"));
        assert!(lines[4].contains("last_client_snapshot=404"));
    }

    #[test]
    fn summarize_client_packet_events_includes_weather_and_spawn_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::CreateWeather {
                weather_id: Some(5),
                intensity: 0.75,
                duration: 120.0,
                wind_x: -2.5,
                wind_y: 6.0,
            },
            ClientSessionEvent::SpawnEffect {
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                unit_type_id: Some(19),
            },
            ClientSessionEvent::UnitSpawnObserved {
                unit_id: 404,
                unit_class_id: 36,
                payload_len: 8,
                consumed_bytes: 5,
                trailing_bytes: 3,
            },
            ClientSessionEvent::UnitBlockSpawn {
                tile_pos: Some(pack_point2(4, 15)),
            },
            ClientSessionEvent::UnitTetherBlockSpawned {
                tile_pos: Some(pack_point2(8, 3)),
                unit_id: 404,
            },
        ]);

        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("create_weather:"));
        assert!(lines[0].contains("weather_id=Some(5)"));
        assert!(lines[0].contains("0x3f400000"));
        assert!(lines[0].contains("0x42f00000"));
        assert!(lines[1].contains("spawn_effect:"));
        assert!(lines[1].contains("0x42020000"));
        assert!(lines[1].contains("0x42400000"));
        assert!(lines[1].contains("0x42b40000"));
        assert!(lines[1].contains("unit_type_id=Some(19)"));
        assert!(lines[2].contains("unit_spawn:"));
        assert!(lines[2].contains("unit_id=404"));
        assert!(lines[2].contains("unit_class_id=36"));
        assert!(lines[2].contains("payload_len=8"));
        assert!(lines[2].contains("consumed_bytes=5"));
        assert!(lines[2].contains("trailing_bytes=3"));
        assert!(lines[3].contains("unit_block_spawn:"));
        assert!(lines[3].contains(&format!("tile_pos=Some({})", pack_point2(4, 15))));
        assert!(lines[4].contains("unit_tether_block_spawned:"));
        assert!(lines[4].contains(&format!("tile_pos=Some({})", pack_point2(8, 3))));
        assert!(lines[4].contains("unit_id=404"));
    }

    #[test]
    fn summarize_client_packet_events_includes_logic_explosion_observability() {
        let lines = summarize_client_packet_events(&[ClientSessionEvent::LogicExplosionObserved {
            team_id: 2,
            x: 16.0,
            y: 24.0,
            radius: 64.0,
            damage: 96.0,
            air: true,
            ground: false,
            pierce: true,
            effect: true,
        }]);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("logic_explosion:"));
        assert!(lines[0].contains("team_id=2"));
        assert!(lines[0].contains("x_bits=0x41800000"));
        assert!(lines[0].contains("y_bits=0x41c00000"));
        assert!(lines[0].contains("radius_bits=0x42800000"));
        assert!(lines[0].contains("damage_bits=0x42c00000"));
        assert!(lines[0].contains("air=true"));
        assert!(lines[0].contains("ground=false"));
        assert!(lines[0].contains("pierce=true"));
        assert!(lines[0].contains("effect=true"));
    }

    #[test]
    fn summarize_client_packet_events_includes_facility_world_action_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::AutoDoorToggle {
                tile_pos: Some(pack_point2(1, 2)),
                open: true,
            },
            ClientSessionEvent::LandingPadLanded {
                tile_pos: Some(pack_point2(3, 4)),
            },
            ClientSessionEvent::AssemblerDroneSpawned {
                tile_pos: Some(pack_point2(5, 6)),
                unit_id: 707,
            },
            ClientSessionEvent::AssemblerUnitSpawned { tile_pos: None },
        ]);

        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("auto_door_toggle:"));
        assert!(lines[0].contains(&format!("tile_pos=Some({})", pack_point2(1, 2))));
        assert!(lines[0].contains("open=true"));
        assert!(lines[1].contains("landing_pad_landed:"));
        assert!(lines[1].contains(&format!("tile_pos=Some({})", pack_point2(3, 4))));
        assert!(lines[2].contains("assembler_drone_spawned:"));
        assert!(lines[2].contains(&format!("tile_pos=Some({})", pack_point2(5, 6))));
        assert!(lines[2].contains("unit_id=707"));
        assert!(lines[3].contains("assembler_unit_spawned:"));
        assert!(lines[3].contains("tile_pos=None"));
    }

    #[test]
    fn summarize_client_packet_events_includes_set_map_area_and_set_team_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::SetMapArea {
                x: 10,
                y: 20,
                w: 30,
                h: 40,
            },
            ClientSessionEvent::SetTeam {
                build_pos: Some(pack_point2(7, 8)),
                team_id: 2,
            },
        ]);

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("set_map_area:"));
        assert!(lines[0].contains("x=10"));
        assert!(lines[0].contains("y=20"));
        assert!(lines[0].contains("w=30"));
        assert!(lines[0].contains("h=40"));
        assert!(lines[1].contains("set_team:"));
        assert!(lines[1].contains(&format!("build_pos=Some({})", pack_point2(7, 8))));
        assert!(lines[1].contains("team_id=2"));
    }

    #[test]
    fn summarize_client_packet_events_includes_set_tile_blocks_floors_and_teams_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::SetTileBlocks {
                block_id: Some(11),
                team_id: 2,
                position_count: 2,
                first_position: Some(pack_point2(1, 2)),
            },
            ClientSessionEvent::SetTileFloors {
                block_id: Some(12),
                position_count: 2,
                first_position: Some(pack_point2(5, 6)),
            },
            ClientSessionEvent::SetTeams {
                team_id: 3,
                position_count: 2,
                first_position: Some(pack_point2(9, 10)),
            },
        ]);

        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("set_tile_blocks:"));
        assert!(lines[0].contains("block_id=Some(11)"));
        assert!(lines[0].contains("team_id=2"));
        assert!(lines[0].contains("count=2"));
        assert!(lines[0].contains(&format!("first_position=Some({})", pack_point2(1, 2))));
        assert!(lines[1].contains("set_tile_floors:"));
        assert!(lines[1].contains("block_id=Some(12)"));
        assert!(lines[1].contains("count=2"));
        assert!(lines[1].contains(&format!("first_position=Some({})", pack_point2(5, 6))));
        assert!(lines[2].contains("set_teams:"));
        assert!(lines[2].contains("team_id=3"));
        assert!(lines[2].contains("count=2"));
        assert!(lines[2].contains(&format!("first_position=Some({})", pack_point2(9, 10))));
    }

    #[test]
    fn summarize_client_packet_events_includes_rules_objectives_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::RulesUpdatedRaw {
                json_data: "{\"waves\":true}".to_string(),
            },
            ClientSessionEvent::ObjectivesUpdatedRaw {
                json_data: "[{\"details\":\"router\"}]".to_string(),
            },
            ClientSessionEvent::SetRuleApplied {
                rule: "pvp".to_string(),
                json_data: "true".to_string(),
            },
            ClientSessionEvent::ObjectivesCleared,
            ClientSessionEvent::ObjectiveCompleted { index: 3 },
        ]);

        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("set_rules:"));
        assert!(lines[0].contains("len=14"));
        assert!(lines[1].contains("set_objectives:"));
        assert!(lines[1].contains("router"));
        assert!(lines[2].contains("set_rule:"));
        assert!(lines[2].contains("pvp"));
        assert_eq!(lines[3], "clear_objectives");
        assert!(lines[4].contains("complete_objective:"));
        assert!(lines[4].contains("index=3"));
    }

    #[test]
    fn summarize_client_packet_events_includes_session_control_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::ConnectRedirectRequested {
                ip: "127.0.0.1".to_string(),
                port: 6567,
            },
            ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 10.0,
                y: 20.0,
            },
            ClientSessionEvent::PlayerPositionUpdated { x: 30.0, y: 40.0 },
            ClientSessionEvent::CameraPositionUpdated { x: 50.0, y: 60.0 },
            ClientSessionEvent::PlayerDisconnected {
                player_id: 7,
                cleared_local_player_sync: true,
            },
            ClientSessionEvent::ServerMessage {
                message: "hello".to_string(),
            },
            ClientSessionEvent::ChatMessage {
                message: "[scarlet]hi".to_string(),
                unformatted: Some("hi".to_string()),
                sender_entity_id: Some(99),
            },
            ClientSessionEvent::Kicked {
                reason_text: Some("server restart".to_string()),
                reason_ordinal: Some(15),
                duration_ms: Some(5000),
            },
            ClientSessionEvent::Ping {
                sent_at_ms: Some(1234),
                response_queued: true,
            },
            ClientSessionEvent::PingResponse {
                sent_at_ms: Some(1234),
                round_trip_ms: Some(56),
            },
        ]);

        assert_eq!(lines.len(), 10);
        assert!(lines[0].contains("connect_redirect:"));
        assert!(lines[0].contains("6567"));
        assert!(lines[1].contains("player_spawn:"));
        assert!(lines[1].contains("player_id=7"));
        assert!(lines[2].contains("player_position:"));
        assert!(lines[2].contains("0x41f00000"));
        assert!(lines[3].contains("camera_position:"));
        assert!(lines[3].contains("0x42480000"));
        assert!(lines[4].contains("player_disconnect:"));
        assert!(lines[4].contains("cleared_local_player_sync=true"));
        assert!(lines[5].contains("server_message:"));
        assert!(lines[5].contains("hello"));
        assert!(lines[6].contains("chat_message:"));
        assert!(lines[6].contains("sender_entity_id=Some(99)"));
        assert!(lines[7].contains("kick:"));
        assert!(lines[7].contains("Some(15)"));
        assert!(lines[7].contains("hint_category=ServerRestarting"));
        assert!(lines[7].contains("retry connection shortly"));
        assert!(lines[8].contains("ping:"));
        assert!(lines[8].contains("response_queued=true"));
        assert!(lines[9].contains("ping_response:"));
        assert!(lines[9].contains("round_trip_ms=Some(56)"));
    }

    #[test]
    fn summarize_client_packet_events_includes_state_snapshot_applied_summary() {
        let lines = summarize_client_packet_events(&[ClientSessionEvent::StateSnapshotApplied {
            projection: StateSnapshotAppliedProjection {
                wave: 8,
                enemies: 2,
                tps: 60,
                gameplay_state: mdt_client_min::session_state::GameplayStateProjection::Paused,
                gameplay_state_transition_count: 3,
                wave_advanced: true,
                wave_advance_from: Some(7),
                wave_advance_to: Some(8),
                apply_count: 2,
                net_seconds_delta: -1,
                net_seconds_rollback: true,
                time_regress_count: 1,
                wave_regress_count: 0,
                core_inventory_team_count: 2,
                core_inventory_item_entry_count: 5,
                core_inventory_total_amount: 77,
                core_inventory_changed_team_count: 2,
                core_inventory_changed_team_sample: vec![2, 7],
                core_parse_failed: true,
                core_parse_fail_count: 1,
                used_last_good_core_fallback: true,
            },
        }]);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("state_snapshot_applied:"));
        assert!(lines[0].contains("gameplay_state=pause"));
        assert!(lines[0].contains("wave_from=Some(7)"));
        assert!(lines[0].contains("rollback=true"));
        assert!(lines[0].contains("core_parse_failed=true"));
        assert!(lines[0].contains("used_last_good_core_fallback=true"));
        assert!(lines[0].contains("core_changed_sample=2,7"));
    }

    #[test]
    fn summarize_client_packet_events_surfaces_kick_hint_for_high_signal_reason_text() {
        let lines = summarize_client_packet_events(&[ClientSessionEvent::Kicked {
            reason_text: Some("typeMismatch".to_string()),
            reason_ordinal: None,
            duration_ms: Some(1200),
        }]);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("kick:"));
        assert!(lines[0].contains("hint_category=TypeMismatch"));
        assert!(lines[0].contains("version type/protocol mismatch"));
    }

    #[test]
    fn summarize_client_packet_events_surfaces_identity_conflict_hint() {
        let lines = summarize_client_packet_events(&[ClientSessionEvent::Kicked {
            reason_text: Some("idInUse".to_string()),
            reason_ordinal: Some(7),
            duration_ms: Some(30_000),
        }]);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("hint_category=IdInUse"));
        assert!(lines[0].contains("old session to clear"));
    }

    #[test]
    fn format_final_kick_summary_defaults_to_none_without_kick() {
        let line = format_final_kick_summary(false, None, None, None);

        assert_eq!(
            line,
            "final_kick: kicked=false reason_text=None reason_ordinal=None duration_ms=None hint_category=none hint_text=None"
        );
    }

    #[test]
    fn format_final_kick_summary_surfaces_high_signal_hint() {
        let line = format_final_kick_summary(
            true,
            Some("serverRestarting"),
            Some(KICK_REASON_SERVER_RESTARTING_ORDINAL),
            Some(1500),
        );

        assert!(line.contains("final_kick: kicked=true"));
        assert!(line.contains("reason_text=Some(\"serverRestarting\")"));
        assert!(line.contains("reason_ordinal=Some(15)"));
        assert!(line.contains("duration_ms=Some(1500)"));
        assert!(line.contains("hint_category=ServerRestarting"));
        assert!(line.contains("retry connection shortly"));
    }

    #[test]
    fn format_final_kick_summary_surfaces_player_limit_hint() {
        let line = format_final_kick_summary(true, Some("playerLimit"), Some(14), Some(0));

        assert!(line.contains("hint_category=PlayerLimit"));
        assert!(line.contains("server is full"));
    }

    #[test]
    fn summarize_client_packet_events_includes_command_control_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::SetHudText {
                message: Some("hud-u".to_string()),
            },
            ClientSessionEvent::SetHudTextReliable {
                message: Some("hud-r".to_string()),
            },
            ClientSessionEvent::HideHudText,
            ClientSessionEvent::Announce {
                message: Some("incoming".to_string()),
            },
            ClientSessionEvent::WorldLabel {
                reliable: false,
                label_id: None,
                message: Some("u-label".to_string()),
                duration: 2.0,
                world_x: 32.5,
                world_y: 48.0,
            },
            ClientSessionEvent::WorldLabel {
                reliable: true,
                label_id: Some(99),
                message: Some("r-id".to_string()),
                duration: 5.0,
                world_x: 6.0,
                world_y: 7.0,
            },
            ClientSessionEvent::RemoveWorldLabel { label_id: 99 },
            ClientSessionEvent::MenuShown {
                menu_id: 12,
                title: Some("title".to_string()),
                message: Some("body".to_string()),
                option_rows: 2,
                first_row_len: 2,
            },
            ClientSessionEvent::FollowUpMenuShown {
                menu_id: 21,
                title: Some("next".to_string()),
                message: Some("step".to_string()),
                option_rows: 1,
                first_row_len: 1,
            },
            ClientSessionEvent::HideFollowUpMenu { menu_id: 21 },
            ClientSessionEvent::SetItem {
                build_pos: Some(203),
                item_id: Some(7),
                amount: 25,
            },
            ClientSessionEvent::SetItems {
                build_pos: Some(607),
                stack_count: 2,
                first_item_id: Some(9),
                first_amount: Some(11),
            },
            ClientSessionEvent::SetLiquid {
                build_pos: Some(405),
                liquid_id: Some(3),
                amount: 2.5,
            },
            ClientSessionEvent::SetLiquids {
                build_pos: Some(809),
                stack_count: 2,
                first_liquid_id: Some(5),
                first_amount_bits: Some(1.25f32.to_bits()),
            },
            ClientSessionEvent::SetTileItems {
                item_id: Some(6),
                amount: 13,
                position_count: 2,
                first_position: Some(102),
            },
            ClientSessionEvent::SetTileLiquids {
                liquid_id: Some(4),
                amount_bits: 0.75f32.to_bits(),
                position_count: 2,
                first_position: Some(506),
            },
            ClientSessionEvent::InfoMessage {
                message: Some("alert".to_string()),
            },
            ClientSessionEvent::InfoToast {
                message: Some("toast".to_string()),
                duration: 1.5,
            },
            ClientSessionEvent::WarningToast {
                unicode: 0xe813,
                text: Some("warn".to_string()),
            },
            ClientSessionEvent::SetFlag {
                flag: Some("wave-start".to_string()),
                add: true,
            },
            ClientSessionEvent::GameOver { winner_team_id: 3 },
            ClientSessionEvent::UpdateGameOver { winner_team_id: 5 },
            ClientSessionEvent::SectorCapture,
            ClientSessionEvent::Researched {
                content_type: 2,
                content_id: 123,
            },
            ClientSessionEvent::SetPlayerTeamEditor { team_id: 3 },
            ClientSessionEvent::MenuChoose {
                menu_id: 12,
                option: -1,
            },
            ClientSessionEvent::TextInputResult {
                text_input_id: 9,
                text: Some("router".to_string()),
            },
            ClientSessionEvent::RequestItem {
                build_pos: Some(404),
                item_id: Some(6),
                amount: 17,
            },
            ClientSessionEvent::RotateBlock {
                build_pos: Some(321),
                direction: true,
            },
            ClientSessionEvent::TransferInventory {
                build_pos: Some(222),
            },
            ClientSessionEvent::ClearItems {
                build_pos: Some(333),
            },
            ClientSessionEvent::ClearLiquids {
                build_pos: Some(444),
            },
            ClientSessionEvent::RequestBuildPayload {
                build_pos: Some(111),
            },
            ClientSessionEvent::RequestDropPayload { x: 12.5, y: 48.0 },
            ClientSessionEvent::RequestUnitPayload {
                target: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 2,
                    value: 99,
                }),
            },
            ClientSessionEvent::DropItem { angle: 135.0 },
            ClientSessionEvent::DeletePlans {
                positions: vec![7, 8],
            },
            ClientSessionEvent::TileTap {
                tile_pos: Some(909),
            },
            ClientSessionEvent::BuildingControlSelect {
                build_pos: Some(123),
            },
            ClientSessionEvent::UnitClear,
            ClientSessionEvent::UnitControl {
                target: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 2,
                    value: 77,
                }),
            },
            ClientSessionEvent::UnitBuildingControlSelect {
                target: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 1,
                    value: 88,
                }),
                build_pos: Some(66),
            },
            ClientSessionEvent::CommandBuilding {
                buildings: vec![11, 22],
                x: 12.5,
                y: -4.0,
            },
            ClientSessionEvent::CommandUnits {
                unit_ids: vec![333, 444],
                build_target: Some(55),
                unit_target: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 1,
                    value: 66,
                }),
                x: 1.0,
                y: 2.0,
                queue_command: true,
                final_batch: false,
            },
            ClientSessionEvent::SetUnitCommand {
                unit_ids: vec![555, 666],
                command_id: Some(12),
            },
            ClientSessionEvent::SetUnitStance {
                unit_ids: vec![777, 888],
                stance_id: Some(7),
                enable: false,
            },
            ClientSessionEvent::CopyToClipboard {
                text: Some("copied".to_string()),
            },
            ClientSessionEvent::OpenUri {
                uri: Some("https://example.com".to_string()),
            },
            ClientSessionEvent::TextInput {
                text_input_id: 10,
                title: Some("Digits".to_string()),
                message: None,
                text_length: 16,
                default_text: Some("123".to_string()),
                numeric: true,
                allow_empty: true,
            },
        ]);

        assert_eq!(lines.len(), 49);
        assert!(lines[0].contains("set_hud_text:"));
        assert!(lines[0].contains("Some(\"hud-u\")"));
        assert!(lines[1].contains("set_hud_text_reliable:"));
        assert!(lines[1].contains("Some(\"hud-r\")"));
        assert_eq!(lines[2], "hide_hud_text");
        assert!(lines[3].contains("announce:"));
        assert!(lines[3].contains("Some(\"incoming\")"));
        assert!(lines[4].contains("world_label:"));
        assert!(lines[4].contains("reliable=false"));
        assert!(lines[4].contains("label_id=None"));
        assert!(lines[4].contains("Some(\"u-label\")"));
        assert!(lines[5].contains("world_label:"));
        assert!(lines[5].contains("reliable=true"));
        assert!(lines[5].contains("label_id=Some(99)"));
        assert!(lines[5].contains("Some(\"r-id\")"));
        assert!(lines[6].contains("remove_world_label:"));
        assert!(lines[6].contains("label_id=99"));
        assert!(lines[7].contains("menu:"));
        assert!(lines[7].contains("menu_id=12"));
        assert!(lines[7].contains("rows=2"));
        assert!(lines[7].contains("first_row_len=2"));
        assert!(lines[8].contains("follow_up_menu:"));
        assert!(lines[8].contains("menu_id=21"));
        assert!(lines[8].contains("rows=1"));
        assert!(lines[8].contains("first_row_len=1"));
        assert!(lines[9].contains("hide_follow_up_menu:"));
        assert!(lines[9].contains("menu_id=21"));
        assert!(lines[10].contains("set_item:"));
        assert!(lines[10].contains("build_pos=Some(203)"));
        assert!(lines[10].contains("item_id=Some(7)"));
        assert!(lines[10].contains("amount=25"));
        assert!(lines[11].contains("set_items:"));
        assert!(lines[11].contains("build_pos=Some(607)"));
        assert!(lines[11].contains("count=2"));
        assert!(lines[11].contains("first_item_id=Some(9)"));
        assert!(lines[11].contains("first_amount=Some(11)"));
        assert!(lines[12].contains("set_liquid:"));
        assert!(lines[12].contains("build_pos=Some(405)"));
        assert!(lines[12].contains("liquid_id=Some(3)"));
        assert!(lines[12].contains("0x40200000"));
        assert!(lines[13].contains("set_liquids:"));
        assert!(lines[13].contains("build_pos=Some(809)"));
        assert!(lines[13].contains("count=2"));
        assert!(lines[13].contains("first_liquid_id=Some(5)"));
        assert!(lines[13].contains("Some(1067450368)"));
        assert!(lines[14].contains("set_tile_items:"));
        assert!(lines[14].contains("item_id=Some(6)"));
        assert!(lines[14].contains("amount=13"));
        assert!(lines[14].contains("count=2"));
        assert!(lines[14].contains("first_position=Some(102)"));
        assert!(lines[15].contains("set_tile_liquids:"));
        assert!(lines[15].contains("liquid_id=Some(4)"));
        assert!(lines[15].contains("0x3f400000"));
        assert!(lines[15].contains("count=2"));
        assert!(lines[15].contains("first_position=Some(506)"));
        assert!(lines[16].contains("info_message:"));
        assert!(lines[16].contains("Some(\"alert\")"));
        assert!(lines[17].contains("info_toast:"));
        assert!(lines[17].contains("Some(\"toast\")"));
        assert!(lines[17].contains("0x3fc00000"));
        assert!(lines[18].contains("warning_toast:"));
        assert!(lines[18].contains("unicode=59411"));
        assert!(lines[18].contains("Some(\"warn\")"));
        assert!(lines[19].contains("set_flag:"));
        assert!(lines[19].contains("Some(\"wave-start\")"));
        assert!(lines[19].contains("add=true"));
        assert!(lines[20].contains("game_over:"));
        assert!(lines[20].contains("winner_team_id=3"));
        assert!(lines[21].contains("update_game_over:"));
        assert!(lines[21].contains("winner_team_id=5"));
        assert_eq!(lines[22], "sector_capture");
        assert!(lines[23].contains("researched:"));
        assert!(lines[23].contains("content_type=2"));
        assert!(lines[23].contains("content_id=123"));
        assert!(lines[24].contains("set_player_team_editor:"));
        assert!(lines[24].contains("team_id=3"));
        assert!(lines[25].contains("menu_choose:"));
        assert!(lines[25].contains("menu_id=12"));
        assert!(lines[25].contains("option=-1"));
        assert!(lines[26].contains("text_input_result:"));
        assert!(lines[26].contains("text_input_id=9"));
        assert!(lines[26].contains("Some(\"router\")"));
        assert!(lines[27].contains("request_item:"));
        assert!(lines[27].contains("build_pos=Some(404)"));
        assert!(lines[27].contains("item_id=Some(6)"));
        assert!(lines[27].contains("amount=17"));
        assert!(lines[28].contains("rotate_block:"));
        assert!(lines[28].contains("build_pos=Some(321)"));
        assert!(lines[28].contains("direction=true"));
        assert!(lines[29].contains("transfer_inventory:"));
        assert!(lines[29].contains("build_pos=Some(222)"));
        assert!(lines[30].contains("clear_items:"));
        assert!(lines[30].contains("build_pos=Some(333)"));
        assert!(lines[31].contains("clear_liquids:"));
        assert!(lines[31].contains("build_pos=Some(444)"));
        assert!(lines[32].contains("request_build_payload:"));
        assert!(lines[32].contains("build_pos=Some(111)"));
        assert!(lines[33].contains("request_drop_payload:"));
        assert!(lines[33].contains("0x41480000"));
        assert!(lines[33].contains("0x42400000"));
        assert!(lines[34].contains("request_unit_payload:"));
        assert!(lines[34].contains("kind: 2"));
        assert!(lines[34].contains("value: 99"));
        assert!(lines[35].contains("drop_item:"));
        assert!(lines[35].contains("0x43070000"));
        assert!(lines[36].contains("delete_plans:"));
        assert!(lines[36].contains("count=2"));
        assert!(lines[36].contains("first_pos=Some(7)"));
        assert!(lines[37].contains("tile_tap:"));
        assert!(lines[37].contains("tile_pos=Some(909)"));
        assert!(lines[38].contains("building_control_select:"));
        assert!(lines[38].contains("build_pos=Some(123)"));
        assert_eq!(lines[39], "unit_clear");
        assert!(lines[40].contains("unit_control:"));
        assert!(lines[40].contains("kind: 2"));
        assert!(lines[40].contains("value: 77"));
        assert!(lines[41].contains("unit_building_control_select:"));
        assert!(lines[41].contains("kind: 1"));
        assert!(lines[41].contains("value: 88"));
        assert!(lines[41].contains("build_pos=Some(66)"));
        assert!(lines[42].contains("command_building:"));
        assert!(lines[42].contains("count=2"));
        assert!(lines[42].contains("first_build_pos=Some(11)"));
        assert!(lines[43].contains("command_units:"));
        assert!(lines[43].contains("count=2"));
        assert!(lines[43].contains("first_unit_id=Some(333)"));
        assert!(lines[43].contains("build_target=Some(55)"));
        assert!(lines[43].contains("queue=true"));
        assert!(lines[43].contains("final_batch=false"));
        assert!(lines[44].contains("set_unit_command:"));
        assert!(lines[44].contains("command_id=Some(12)"));
        assert!(lines[45].contains("set_unit_stance:"));
        assert!(lines[45].contains("stance_id=Some(7)"));
        assert!(lines[45].contains("enable=false"));
        assert!(lines[46].contains("copy_to_clipboard:"));
        assert!(lines[46].contains("Some(\"copied\")"));
        assert!(lines[47].contains("open_uri:"));
        assert!(lines[47].contains("Some(\"https://example.com\")"));
        assert!(lines[48].contains("text_input:"));
        assert!(lines[48].contains("text_input_id=10"));
        assert!(lines[48].contains("Some(\"Digits\")"));
        assert!(lines[48].contains("text_length=16"));
        assert!(lines[48].contains("Some(\"123\")"));
        assert!(lines[48].contains("numeric=true"));
        assert!(lines[48].contains("allow_empty=true"));
    }

    #[test]
    fn summarize_client_packet_events_includes_marker_and_ignored_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::CreateMarker {
                marker_id: 77,
                json_len: 16,
            },
            ClientSessionEvent::UpdateMarker {
                marker_id: 77,
                control: 4,
                control_name: Some("pos".to_string()),
                p1_bits: 12.5f64.to_bits(),
                p2_bits: (-3.0f64).to_bits(),
                p3_bits: 2.25f64.to_bits(),
            },
            ClientSessionEvent::UpdateMarkerText {
                marker_id: 77,
                control: 14,
                control_name: Some("flushText".to_string()),
                fetch: true,
                text: Some("logic-text".to_string()),
            },
            ClientSessionEvent::UpdateMarkerTexture {
                marker_id: 77,
                texture_kind: 4,
                texture_kind_name: "string".to_string(),
            },
            ClientSessionEvent::RemoveMarker { marker_id: 77 },
            ClientSessionEvent::DeferredPacketWhileLoading {
                packet_id: 146,
                remote: Some(mdt_client_min::client_session::IgnoredRemotePacketMeta {
                    method: "unitSpawn".to_string(),
                    packet_class: "mindustry.gen.UnitSpawnCallPacket".to_string(),
                }),
            },
            ClientSessionEvent::DroppedLowPriorityPacketWhileLoading {
                packet_id: 147,
                remote: Some(mdt_client_min::client_session::IgnoredRemotePacketMeta {
                    method: "stateSnapshot".to_string(),
                    packet_class: "mindustry.gen.StateSnapshotCallPacket".to_string(),
                }),
            },
            ClientSessionEvent::IgnoredPacket {
                packet_id: 148,
                remote: Some(mdt_client_min::client_session::IgnoredRemotePacketMeta {
                    method: "unitTetherBlockSpawned".to_string(),
                    packet_class: "mindustry.gen.UnitTetherBlockSpawnedCallPacket".to_string(),
                }),
            },
        ]);

        assert_eq!(lines.len(), 8);
        assert!(lines[0].contains("create_marker:"));
        assert!(lines[0].contains("marker_id=77"));
        assert!(lines[0].contains("json_len=16"));
        assert!(lines[1].contains("update_marker:"));
        assert!(lines[1].contains("control_name=Some(\"pos\")"));
        assert!(lines[2].contains("update_marker_text:"));
        assert!(lines[2].contains("fetch=true"));
        assert!(lines[2].contains("Some(\"logic-text\")"));
        assert!(lines[3].contains("update_marker_texture:"));
        assert!(lines[3].contains("texture_kind=4"));
        assert!(lines[3].contains("\"string\""));
        assert!(lines[4].contains("remove_marker:"));
        assert!(lines[5].contains("deferred_packet_while_loading:"));
        assert!(lines[5].contains("Some(\"unitSpawn\")"));
        assert!(lines[6].contains("dropped_low_priority_packet_while_loading:"));
        assert!(lines[6].contains("Some(\"stateSnapshot\")"));
        assert!(lines[7].contains("ignored_packet:"));
        assert!(lines[7].contains("Some(\"unitTetherBlockSpawned\")"));
    }

    #[test]
    fn summarize_client_packet_events_includes_tile_overlay_and_logic_sync_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::RemoveTile {
                tile_pos: Some(pack_point2(2, 3)),
            },
            ClientSessionEvent::SetTile {
                tile_pos: Some(pack_point2(4, 5)),
                block_id: Some(29),
                team_id: 2,
                rotation: 3,
            },
            ClientSessionEvent::SetFloor {
                tile_pos: Some(pack_point2(6, 7)),
                floor_id: Some(8),
                overlay_id: Some(9),
            },
            ClientSessionEvent::SetOverlay {
                tile_pos: Some(pack_point2(8, 9)),
                overlay_id: Some(10),
            },
            ClientSessionEvent::SetTileOverlays {
                block_id: Some(17),
                position_count: 2,
                first_position: Some(pack_point2(5, 6)),
            },
            ClientSessionEvent::SyncVariable {
                build_pos: Some(pack_point2(9, 10)),
                variable: 4,
                value_kind: 4,
                value_kind_name: "string".to_string(),
            },
        ]);

        assert_eq!(lines.len(), 6);
        assert!(lines[0].contains("remove_tile:"));
        assert!(lines[0].contains(&format!("tile_pos=Some({})", pack_point2(2, 3))));
        assert!(lines[1].contains("set_tile:"));
        assert!(lines[1].contains(&format!("tile_pos=Some({})", pack_point2(4, 5))));
        assert!(lines[1].contains("block_id=Some(29)"));
        assert!(lines[1].contains("team_id=2"));
        assert!(lines[1].contains("rotation=3"));
        assert!(lines[2].contains("set_floor:"));
        assert!(lines[2].contains(&format!("tile_pos=Some({})", pack_point2(6, 7))));
        assert!(lines[2].contains("floor_id=Some(8)"));
        assert!(lines[2].contains("overlay_id=Some(9)"));
        assert!(lines[3].contains("set_overlay:"));
        assert!(lines[3].contains(&format!("tile_pos=Some({})", pack_point2(8, 9))));
        assert!(lines[3].contains("overlay_id=Some(10)"));
        assert!(lines[4].contains("set_tile_overlays:"));
        assert!(lines[4].contains("block_id=Some(17)"));
        assert!(lines[4].contains("count=2"));
        assert!(lines[5].contains("sync_variable:"));
        assert!(lines[5].contains("variable=4"));
        assert!(lines[5].contains("value_kind=4"));
        assert!(lines[5].contains("\"string\""));
    }

    #[test]
    fn summarize_client_packet_events_includes_unit_lifecycle_observability() {
        let lines = summarize_client_packet_events(&[
            ClientSessionEvent::BuildDestroyed {
                build_pos: Some(pack_point2(3, 12)),
            },
            ClientSessionEvent::UnitDeath {
                unit_id: 701,
                removed_entity_projection: true,
            },
            ClientSessionEvent::UnitDestroy {
                unit_id: 702,
                removed_entity_projection: false,
            },
            ClientSessionEvent::UnitEnvDeath {
                unit: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 2,
                    value: 703,
                }),
                removed_entity_projection: true,
            },
            ClientSessionEvent::UnitSafeDeath {
                unit: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 1,
                    value: pack_point2(11, 12),
                }),
                removed_entity_projection: false,
            },
            ClientSessionEvent::UnitCapDeath {
                unit: Some(mdt_client_min::session_state::UnitRefProjection {
                    kind: 2,
                    value: 704,
                }),
            },
        ]);

        assert_eq!(lines.len(), 6);
        assert!(lines[0].contains("build_destroyed:"));
        assert!(lines[0].contains(&format!("Some({})", pack_point2(3, 12))));
        assert!(lines[1].contains("unit_death:"));
        assert!(lines[1].contains("unit_id=701"));
        assert!(lines[1].contains("removed_entity_projection=true"));
        assert!(lines[2].contains("unit_destroy:"));
        assert!(lines[2].contains("unit_id=702"));
        assert!(lines[2].contains("removed_entity_projection=false"));
        assert!(lines[3].contains("unit_env_death:"));
        assert!(lines[3].contains("kind: 2"));
        assert!(lines[3].contains("value: 703"));
        assert!(lines[4].contains("unit_safe_death:"));
        assert!(lines[4].contains("kind: 1"));
        assert!(lines[4].contains(&format!("value: {}", pack_point2(11, 12))));
        assert!(lines[5].contains("unit_cap_death:"));
        assert!(lines[5].contains("kind: 2"));
        assert!(lines[5].contains("value: 704"));
    }

    #[test]
    fn build_chat_schedule_assigns_default_offset_per_message() {
        let schedule = build_chat_schedule(
            vec![
                "hello".to_string(),
                "/sync".to_string(),
                "/sync".to_string(),
            ],
            1_000,
            500,
        );

        assert_eq!(
            schedule,
            vec![
                ScheduledChatEntry {
                    not_before_ms: 1_000,
                    text: "hello".to_string(),
                },
                ScheduledChatEntry {
                    not_before_ms: 1_500,
                    text: "/sync".to_string(),
                },
                ScheduledChatEntry {
                    not_before_ms: 2_000,
                    text: "/sync".to_string(),
                },
            ]
        );
    }

    #[test]
    fn collect_due_chat_messages_drains_all_ready_entries_in_order() {
        let schedule = build_chat_schedule(
            vec!["hello".to_string(), "/sync".to_string(), "done".to_string()],
            1_000,
            500,
        );
        let mut next_index = 0usize;

        assert!(collect_due_chat_messages(&schedule, 999, &mut next_index).is_empty());
        assert_eq!(next_index, 0);

        assert_eq!(
            collect_due_chat_messages(&schedule, 1_500, &mut next_index),
            vec![
                ScheduledChatEntry {
                    not_before_ms: 1_000,
                    text: "hello".to_string(),
                },
                ScheduledChatEntry {
                    not_before_ms: 1_500,
                    text: "/sync".to_string(),
                },
            ]
        );
        assert_eq!(next_index, 2);

        assert_eq!(
            collect_due_chat_messages(&schedule, 5_000, &mut next_index),
            vec![ScheduledChatEntry {
                not_before_ms: 2_000,
                text: "done".to_string(),
            }]
        );
        assert_eq!(next_index, 3);
    }

    #[test]
    fn collect_due_outbound_actions_drains_all_ready_entries_in_order() {
        let schedule = build_outbound_action_schedule(
            vec![
                OutboundAction::TransferInventory { build_pos: Some(1) },
                OutboundAction::RequestBuildPayload { build_pos: None },
                OutboundAction::DropItem { angle: 90.0 },
            ],
            1_000,
            500,
        );
        let mut next_index = 0usize;

        assert!(collect_due_outbound_actions(&schedule, 999, &mut next_index).is_empty());
        assert_eq!(next_index, 0);

        assert_eq!(
            collect_due_outbound_actions(&schedule, 1_500, &mut next_index),
            vec![
                ScheduledOutboundAction {
                    not_before_ms: 1_000,
                    action: OutboundAction::TransferInventory { build_pos: Some(1) },
                },
                ScheduledOutboundAction {
                    not_before_ms: 1_500,
                    action: OutboundAction::RequestBuildPayload { build_pos: None },
                },
            ]
        );
        assert_eq!(next_index, 2);

        assert_eq!(
            collect_due_outbound_actions(&schedule, 5_000, &mut next_index),
            vec![ScheduledOutboundAction {
                not_before_ms: 2_000,
                action: OutboundAction::DropItem { angle: 90.0 },
            }]
        );
        assert_eq!(next_index, 3);
    }

    #[test]
    fn runtime_plan_edit_loop_state_collects_entries_and_wraps() {
        let mut state = RuntimePlanEditLoopState::new(PlanEditLoopConfig {
            ops: vec![
                PlanEditOp::Rotate {
                    origin: (0, 0),
                    direction: 1,
                },
                PlanEditOp::Flip {
                    origin: (2, 3),
                    flip_x: false,
                },
            ],
            delay_ms: 100,
            spacing_ms: 50,
        });

        assert!(state.collect_due_entries(0).is_empty());
        assert!(state.collect_due_entries(99).is_empty());
        assert_eq!(
            state.collect_due_entries(100),
            vec![RuntimePlanEditLoopEntry {
                op_index: 0,
                scheduled_ms: 100,
                op: PlanEditOp::Rotate {
                    origin: (0, 0),
                    direction: 1,
                },
            }]
        );
        assert_eq!(
            state.collect_due_entries(150),
            vec![RuntimePlanEditLoopEntry {
                op_index: 1,
                scheduled_ms: 150,
                op: PlanEditOp::Flip {
                    origin: (2, 3),
                    flip_x: false,
                },
            }]
        );
        assert_eq!(
            state.collect_due_entries(200),
            vec![RuntimePlanEditLoopEntry {
                op_index: 0,
                scheduled_ms: 200,
                op: PlanEditOp::Rotate {
                    origin: (0, 0),
                    direction: 1,
                },
            }]
        );
    }

    #[test]
    fn outbound_action_script_produces_stable_client_event_signature() {
        let args = parse_args(sample_args(&[
            "--action-delay-ms",
            "100",
            "--action-spacing-ms",
            "50",
            "--action-client-packet",
            "mod.echo@hello world@reliable",
            "--action-client-binary-packet",
            "mod.bin@aabbcc@unreliable",
            "--action-client-logic-data",
            "logic.alpha@int=7@reliable",
        ]))
        .unwrap();
        let mut next_index = 0usize;
        assert!(
            collect_due_outbound_actions(&args.outbound_action_schedule, 99, &mut next_index)
                .is_empty()
        );

        let events =
            collect_due_outbound_actions(&args.outbound_action_schedule, 250, &mut next_index)
                .into_iter()
                .map(|entry| match entry.action {
                    OutboundAction::ClientPacket {
                        packet_type,
                        contents,
                        transport,
                    } => match transport {
                        ClientPacketTransport::Tcp => ClientSessionEvent::ClientPacketReliable {
                            packet_type,
                            contents,
                        },
                        ClientPacketTransport::Udp => ClientSessionEvent::ClientPacketUnreliable {
                            packet_type,
                            contents,
                        },
                    },
                    OutboundAction::ClientBinaryPacket {
                        packet_type,
                        contents,
                        transport,
                    } => match transport {
                        ClientPacketTransport::Tcp => {
                            ClientSessionEvent::ClientBinaryPacketReliable {
                                packet_type,
                                contents,
                            }
                        }
                        ClientPacketTransport::Udp => {
                            ClientSessionEvent::ClientBinaryPacketUnreliable {
                                packet_type,
                                contents,
                            }
                        }
                    },
                    OutboundAction::ClientLogicData {
                        channel,
                        value,
                        transport,
                    } => match transport {
                        ClientLogicDataTransport::Reliable => {
                            ClientSessionEvent::ClientLogicDataReliable { channel, value }
                        }
                        ClientLogicDataTransport::Unreliable => {
                            ClientSessionEvent::ClientLogicDataUnreliable { channel, value }
                        }
                    },
                    other => panic!("unexpected action in script signature regression: {other:?}"),
                })
                .collect::<Vec<_>>();

        let lines = summarize_client_packet_events(&events);
        assert_eq!(
            lines,
            vec![
                "client_packet: transport=reliable type=\"mod.echo\" len=11 preview=\"hello world\""
                    .to_string(),
                "client_binary_packet: transport=unreliable type=\"mod.bin\" len=3 hex_prefix=aabbcc"
                    .to_string(),
                "client_logic_data: transport=reliable channel=\"logic.alpha\" kind=\"int\" preview=\"Int(7)\""
                    .to_string(),
            ]
        );
    }

    #[test]
    fn summarize_custom_packet_outbound_action_formats_client_custom_variants() {
        assert_eq!(
            summarize_custom_packet_outbound_action(&OutboundAction::ClientPacket {
                packet_type: "mod.echo".to_string(),
                contents: "hello world".to_string(),
                transport: ClientPacketTransport::Tcp,
            }),
            vec![
                "client_packet: transport=reliable type=\"mod.echo\" len=11 preview=\"hello world\""
                    .to_string()
            ]
        );
        assert_eq!(
            summarize_custom_packet_outbound_action(&OutboundAction::ClientBinaryPacket {
                packet_type: "mod.bin".to_string(),
                contents: vec![0xAA, 0xBB, 0xCC],
                transport: ClientPacketTransport::Udp,
            }),
            vec![
                "client_binary_packet: transport=unreliable type=\"mod.bin\" len=3 hex_prefix=aabbcc"
                    .to_string()
            ]
        );
        assert_eq!(
            summarize_custom_packet_outbound_action(&OutboundAction::ClientLogicData {
                channel: "logic.alpha".to_string(),
                value: TypeIoObject::Int(7),
                transport: ClientLogicDataTransport::Reliable,
            }),
            vec![
                "client_logic_data: transport=reliable channel=\"logic.alpha\" kind=\"int\" preview=\"Int(7)\""
                    .to_string()
            ]
        );
        assert!(summarize_custom_packet_outbound_action(&OutboundAction::UnitClear).is_empty());
    }

    #[test]
    fn summarize_runtime_custom_packet_relay_action_formats_replayed_variants() {
        assert_eq!(
            summarize_runtime_custom_packet_relay_action(&RuntimeCustomPacketRelayAction::Text {
                packet_type: "mod.pong".to_string(),
                contents: "relay ready".to_string(),
                transport: ClientPacketTransport::Udp,
            }),
            vec![
                "client_packet: transport=unreliable type=\"mod.pong\" len=11 preview=\"relay ready\""
                    .to_string()
            ]
        );
        assert_eq!(
            summarize_runtime_custom_packet_relay_action(&RuntimeCustomPacketRelayAction::Binary {
                packet_type: "mod.bin.pong".to_string(),
                contents: vec![0xDE, 0xAD, 0xBE, 0xEF],
                transport: ClientPacketTransport::Tcp,
            }),
            vec![
                "client_binary_packet: transport=reliable type=\"mod.bin.pong\" len=4 hex_prefix=deadbeef"
                    .to_string()
            ]
        );
        assert_eq!(
            summarize_runtime_custom_packet_relay_action(
                &RuntimeCustomPacketRelayAction::LogicData {
                    channel: "logic.beta".to_string(),
                    value: TypeIoObject::Bool(true),
                    transport: ClientLogicDataTransport::Unreliable,
                }
            ),
            vec![
                "client_logic_data: transport=unreliable channel=\"logic.beta\" kind=\"bool\" preview=\"Bool(true)\""
                    .to_string()
            ]
        );
    }

    #[test]
    fn queue_outbound_action_dispatches_supported_methods() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();

        queue_outbound_action(
            &mut session,
            &OutboundAction::RequestItem {
                build_pos: Some(222),
                item_id: Some(9),
                amount: 15,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::RequestUnitPayload {
                target: ClientUnitRef::Standard(444),
            },
        )
        .unwrap();
        queue_outbound_action(&mut session, &OutboundAction::UnitClear).unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::UnitControl {
                target: ClientUnitRef::Block(111),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::UnitBuildingControlSelect {
                target: ClientUnitRef::Standard(222),
                build_pos: Some(333),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::BuildingControlSelect {
                build_pos: Some(555),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::ClearItems {
                build_pos: Some(666),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::ClearLiquids {
                build_pos: Some(667),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::TransferInventory {
                build_pos: Some(321),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::RequestBuildPayload {
                build_pos: Some(654),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::RequestDropPayload { x: 12.5, y: 48.0 },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::RotateBlock {
                build_pos: Some(777),
                direction: true,
            },
        )
        .unwrap();
        queue_outbound_action(&mut session, &OutboundAction::DropItem { angle: 135.0 }).unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::TileConfig {
                build_pos: Some(888),
                value: TypeIoObject::ObjectArray(vec![
                    TypeIoObject::Int(7),
                    TypeIoObject::String(Some("router".to_string())),
                    TypeIoObject::Bool(true),
                ]),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::TileTap {
                tile_pos: Some(999),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::DeletePlans {
                positions: vec![pack_point2(1, 2), pack_point2(-3, 4)],
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::CommandBuilding {
                buildings: vec![pack_point2(5, 6), pack_point2(-7, 8)],
                x: 12.5,
                y: -4.0,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::CommandUnits {
                unit_ids: vec![111, 222],
                build_target: Some(333),
                unit_target: ClientUnitRef::Standard(444),
                pos_target: Some((48.0, 96.0)),
                queue_command: true,
                final_batch: false,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::CommandUnitsChunked {
                unit_ids: vec![1, 2, 3],
                build_target: None,
                unit_target: ClientUnitRef::None,
                pos_target: None,
                queue_command: false,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::SetUnitCommand {
                unit_ids: vec![333, 444],
                command_id: Some(12),
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::SetUnitStance {
                unit_ids: vec![555, 666],
                stance_id: Some(7),
                enable: false,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::BeginBreak {
                builder: ClientUnitRef::Standard(777),
                team_id: 8,
                x: -11,
                y: 22,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::BeginPlace {
                builder: ClientUnitRef::Block(888),
                block_id: Some(999),
                team_id: 3,
                x: 44,
                y: -55,
                rotation: 2,
                place_config: TypeIoObject::Point2 { x: 7, y: -8 },
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::MenuChoose {
                menu_id: 12,
                option: -1,
            },
        )
        .unwrap();
        queue_outbound_action(
            &mut session,
            &OutboundAction::TextInputResult {
                text_input_id: 9,
                text: Some("router".to_string()),
            },
        )
        .unwrap();
    }

    #[test]
    fn sync_command_mode_state_for_outbound_action_tracks_selection_container() {
        let mut runtime_command_mode = CommandModeState::default();

        runtime_command_mode.bind_control_group(4, &[88, 99]);
        sync_command_mode_state_for_outbound_action(
            &mut runtime_command_mode,
            &OutboundAction::CommandUnits {
                unit_ids: vec![11, 22, 11],
                build_target: Some(333),
                unit_target: ClientUnitRef::Standard(444),
                pos_target: Some((48.0, 96.0)),
                queue_command: true,
                final_batch: false,
            },
        );
        assert!(runtime_command_mode.is_active());
        assert_eq!(
            runtime_command_mode.projection().selected_units,
            vec![11, 22]
        );
        assert_eq!(
            runtime_command_mode.projection().last_target,
            Some(mdt_input::CommandModeTargetProjection {
                build_target: Some(333),
                unit_target: Some(CommandUnitRef {
                    kind: 2,
                    value: 444,
                }),
                position_target: Some(mdt_input::CommandModePositionTarget {
                    x_bits: 48.0f32.to_bits(),
                    y_bits: 96.0f32.to_bits(),
                }),
                rect_target: None,
            })
        );

        sync_command_mode_state_for_outbound_action(
            &mut runtime_command_mode,
            &OutboundAction::CommandBuilding {
                buildings: vec![pack_point2(5, 6), pack_point2(5, 6), pack_point2(-7, 8)],
                x: 12.5,
                y: -4.0,
            },
        );
        assert_eq!(
            runtime_command_mode.projection().command_buildings,
            vec![pack_point2(5, 6), pack_point2(-7, 8)]
        );

        sync_command_mode_state_for_outbound_action(
            &mut runtime_command_mode,
            &OutboundAction::UnitClear,
        );
        assert!(!runtime_command_mode.is_active());
        assert!(runtime_command_mode.projection().selected_units.is_empty());
        assert!(runtime_command_mode
            .projection()
            .command_buildings
            .is_empty());
        assert_eq!(
            runtime_command_mode.projection().control_groups,
            vec![mdt_input::CommandModeControlGroupProjection {
                index: 4,
                unit_ids: vec![88, 99],
            }]
        );
    }

    #[test]
    fn runtime_command_mode_cli_updates_projection_control_groups() {
        let mut runtime_command_mode = CommandModeState::default();

        apply_runtime_command_mode_cli_ops(
            &mut runtime_command_mode,
            &[
                CommandModeCliOp::BindGroup {
                    index: 2,
                    unit_ids: vec![11, 22, 11],
                },
                CommandModeCliOp::BindGroup {
                    index: 4,
                    unit_ids: vec![99],
                },
                CommandModeCliOp::RecallGroup { index: 2 },
                CommandModeCliOp::ClearGroup { index: 4 },
            ],
        );

        assert!(runtime_command_mode.is_active());
        assert_eq!(
            runtime_command_mode.projection().selected_units,
            vec![11, 22]
        );
        assert_eq!(
            runtime_command_mode.projection().control_groups,
            vec![mdt_input::CommandModeControlGroupProjection {
                index: 2,
                unit_ids: vec![11, 22],
            }]
        );
    }

    #[test]
    fn runtime_command_mode_cli_updates_projection_rect() {
        let mut runtime_command_mode = CommandModeState::default();

        apply_runtime_command_mode_cli_ops(
            &mut runtime_command_mode,
            &[CommandModeCliOp::SetRect(Some(CommandModeRectProjection {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            }))],
        );
        assert!(runtime_command_mode.is_active());
        assert_eq!(
            runtime_command_mode.projection().command_rect,
            Some(CommandModeRectProjection {
                x0: -3,
                y0: 4,
                x1: 12,
                y1: 18,
            })
        );

        apply_runtime_command_mode_cli_ops(
            &mut runtime_command_mode,
            &[CommandModeCliOp::SetRect(None)],
        );
        assert!(runtime_command_mode.is_active());
        assert_eq!(runtime_command_mode.projection().command_rect, None);
    }

    #[test]
    fn queue_outbound_action_with_command_mode_updates_runtime_command_mode() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let mut runtime_command_mode = CommandModeState::default();

        queue_outbound_action_with_command_mode(
            &mut session,
            &OutboundAction::SetUnitStance {
                unit_ids: vec![555, 666, 555],
                stance_id: Some(7),
                enable: false,
            },
            &mut runtime_command_mode,
        )
        .unwrap();

        assert!(runtime_command_mode.is_active());
        assert_eq!(
            runtime_command_mode.projection().selected_units,
            vec![555, 666]
        );
        assert_eq!(
            runtime_command_mode.projection().last_stance_selection,
            Some(mdt_input::CommandModeStanceSelection {
                stance_id: Some(7),
                enabled: false,
            })
        );
    }

    #[test]
    fn queue_outbound_action_chunks_large_command_units_when_final_batch_true() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        assert!(session.state().ready_to_enter_world);
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        assert!(session.state().connect_confirm_sent);

        let command_units_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandUnits")
            .unwrap()
            .packet_id;

        let unit_ids = (0..401).collect::<Vec<_>>();
        queue_outbound_action(
            &mut session,
            &OutboundAction::CommandUnits {
                unit_ids,
                build_target: Some(pack_point2(6, 7)),
                unit_target: ClientUnitRef::Standard(444),
                pos_target: Some((48.0, 96.0)),
                queue_command: true,
                final_batch: true,
            },
        )
        .unwrap();

        let actions = session.advance_time(1).unwrap();
        let command_units_packets = actions
            .iter()
            .filter(|action| {
                matches!(
                    action,
                    ClientSessionAction::SendPacket { packet_id, .. }
                        if *packet_id == command_units_packet_id
                )
            })
            .count();
        assert_eq!(command_units_packets, 3);
    }

    #[test]
    fn apply_snapshot_overrides_sets_client_snapshot_pointer() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&["--aim-x", "16", "--aim-y", "24"])).unwrap();

        apply_snapshot_overrides(&mut session, &args);

        assert_eq!(session.snapshot_input_mut().pointer, Some((16.0, 24.0)));
    }

    #[test]
    fn apply_snapshot_overrides_sets_client_snapshot_mining_tile() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&["--mine-tile", "88:99"])).unwrap();

        apply_snapshot_overrides(&mut session, &args);

        assert_eq!(session.snapshot_input_mut().mining_tile, Some((88, 99)));
    }

    #[test]
    fn apply_snapshot_overrides_sets_snapshot_state_flags_and_view_size() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--snapshot-no-boosting",
            "--snapshot-no-shooting",
            "--snapshot-no-chatting",
            "--snapshot-building",
            "--view-size",
            "480:320",
        ]))
        .unwrap();

        let input = session.snapshot_input_mut();
        input.boosting = true;
        input.shooting = true;
        input.chatting = true;
        input.view_size = Some((1.0, 1.0));

        apply_snapshot_overrides(&mut session, &args);

        let input = session.snapshot_input_mut();
        assert!(!input.boosting);
        assert!(!input.shooting);
        assert!(!input.chatting);
        assert!(input.building);
        assert_eq!(input.view_size, Some((480.0, 320.0)));
    }

    #[test]
    fn apply_snapshot_overrides_snapshot_building_can_override_build_plan_default() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--plan-place",
            "1:2:0x0101:1",
            "--snapshot-no-building",
        ]))
        .unwrap();

        apply_snapshot_overrides(&mut session, &args);

        let input = session.snapshot_input_mut();
        assert!(!input.building);
        assert_eq!(input.selected_block_id, Some(0x0101));
        assert_eq!(input.plans.as_ref().map(|plans| plans.len()), Some(1));
    }

    #[test]
    fn apply_snapshot_overrides_sets_build_plan_queue() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--plan-place",
            "1:2:0x0101:1",
            "--plan-break",
            "5:6",
        ]))
        .unwrap();

        apply_snapshot_overrides(&mut session, &args);

        let input = session.snapshot_input_mut();
        assert!(input.building);
        assert_eq!(input.selected_block_id, Some(0x0101));
        assert_eq!(input.selected_rotation, 1);
        assert_eq!(input.plans.as_ref().map(|plans| plans.len()), Some(2));
    }

    #[test]
    fn maybe_apply_runtime_plan_edit_loop_rotates_live_queue_after_delay() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--plan-place",
            "1:0:0x0101:0",
            "--plan-rotate",
            "0:0:1",
            "--plan-edit-loop",
            "--plan-edit-delay-ms",
            "100",
            "--plan-edit-spacing-ms",
            "50",
        ]))
        .unwrap();
        let mut loop_state = RuntimePlanEditLoopState::new(
            args.runtime_plan_edit_loop
                .clone()
                .expect("runtime plan-edit loop config"),
        );

        ingest_sample_world(&mut session);
        assert!(session.state().ready_to_enter_world);
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        assert!(session.state().connect_confirm_sent);
        apply_snapshot_overrides(&mut session, &args);

        maybe_apply_runtime_plan_edit_loop(&mut session, Some(&mut loop_state), 0);
        maybe_apply_runtime_plan_edit_loop(&mut session, Some(&mut loop_state), 99);
        assert_eq!(
            session.snapshot_input().plans,
            Some(vec![ClientBuildPlan {
                tile: (0, 1),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 1,
                config: ClientBuildPlanConfig::None,
            }])
        );

        maybe_apply_runtime_plan_edit_loop(&mut session, Some(&mut loop_state), 100);
        assert_eq!(
            session.snapshot_input().plans,
            Some(vec![ClientBuildPlan {
                tile: (-1, 0),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 2,
                config: ClientBuildPlanConfig::None,
            }])
        );

        maybe_apply_runtime_plan_edit_loop(&mut session, Some(&mut loop_state), 150);
        assert_eq!(
            session.snapshot_input().plans,
            Some(vec![ClientBuildPlan {
                tile: (0, -1),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 3,
                config: ClientBuildPlanConfig::None,
            }])
        );
    }

    #[test]
    fn maybe_apply_relative_build_plans_resolves_from_runtime_position() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--plan-place-relative",
            "1:0:0x0101:2;point2=4:5",
            "--plan-break-relative",
            "-1:0",
        ]))
        .unwrap();
        let mut applied = false;

        session.snapshot_input_mut().position = Some((792.0, 792.0));
        maybe_apply_relative_build_plans(
            &mut session,
            &args,
            &[ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 792.0,
                y: 792.0,
            }],
            &mut applied,
        );

        let input = session.snapshot_input_mut();
        assert!(applied);
        assert!(input.building);
        assert_eq!(input.selected_block_id, Some(0x0101));
        assert_eq!(input.selected_rotation, 2);
        assert_eq!(
            input.plans,
            Some(vec![
                ClientBuildPlan {
                    tile: (100, 99),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 2,
                    config: ClientBuildPlanConfig::Point2 { x: 4, y: 5 },
                },
                ClientBuildPlan {
                    tile: (98, 99),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
            ])
        );
    }

    #[test]
    fn maybe_apply_relative_build_plans_replaces_existing_same_tile_plan() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--plan-place-relative",
            "1:0:0x0101:2;point2=4:5",
        ]))
        .unwrap();
        let mut applied = false;
        session.snapshot_input_mut().plans = Some(vec![
            ClientBuildPlan {
                tile: (100, 99),
                breaking: false,
                block_id: Some(0x0102),
                rotation: 1,
                config: ClientBuildPlanConfig::String("old".to_string()),
            },
            ClientBuildPlan {
                tile: (101, 99),
                breaking: false,
                block_id: Some(0x0103),
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
        ]);

        maybe_apply_relative_build_plans(
            &mut session,
            &args,
            &[ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 792.0,
                y: 792.0,
            }],
            &mut applied,
        );

        let plans = session.snapshot_input().plans.clone().unwrap_or_default();
        assert!(applied);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].tile, (101, 99));
        assert_eq!(plans[1].tile, (100, 99));
        assert_eq!(plans[1].block_id, Some(0x0101));
        assert_eq!(
            plans[1].config,
            ClientBuildPlanConfig::Point2 { x: 4, y: 5 }
        );
    }

    #[test]
    fn merge_build_plan_queue_tail_deduplicates_same_tile_with_tail_wins() {
        let existing = vec![
            ClientBuildPlan {
                tile: (1, 1),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (1, 1),
                breaking: false,
                block_id: Some(0x0102),
                rotation: 1,
                config: ClientBuildPlanConfig::String("dup-existing".to_string()),
            },
            ClientBuildPlan {
                tile: (2, 2),
                breaking: false,
                block_id: Some(0x0103),
                rotation: 2,
                config: ClientBuildPlanConfig::None,
            },
        ];
        let incoming = vec![
            ClientBuildPlan {
                tile: (3, 3),
                breaking: true,
                block_id: None,
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (2, 2),
                breaking: false,
                block_id: Some(0x0104),
                rotation: 3,
                config: ClientBuildPlanConfig::Bytes(vec![1, 2]),
            },
            ClientBuildPlan {
                tile: (3, 3),
                breaking: false,
                block_id: Some(0x0105),
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
        ];

        let merged = merge_build_plan_queue_tail(Some(existing.as_slice()), incoming.as_slice());

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].tile, (1, 1));
        assert_eq!(merged[0].block_id, Some(0x0102));
        assert_eq!(merged[1].tile, (2, 2));
        assert_eq!(merged[1].block_id, Some(0x0104));
        assert_eq!(merged[2].tile, (3, 3));
        assert_eq!(merged[2].block_id, Some(0x0105));
        assert!(!merged[2].breaking);
    }

    #[test]
    fn maybe_apply_auto_build_plans_selects_visible_empty_tile_near_player() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        session.snapshot_input_mut().selected_block_id = Some(0x0101);
        session.snapshot_input_mut().selected_rotation = 2;
        let args = parse_args(sample_args(&["--plan-place-near-player", "selected"])).unwrap();
        let mut applied = false;

        maybe_apply_auto_build_plans(
            &mut session,
            &args,
            &[ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 32.0,
                y: 32.0,
            }],
            &mut applied,
        );

        let input = session.snapshot_input();
        assert!(applied);
        assert!(input.building);
        assert_eq!(input.selected_block_id, Some(0x0101));
        assert_eq!(input.selected_rotation, 2);
        let plans = input.plans.as_ref().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].breaking, false);
        assert_eq!(plans[0].block_id, Some(0x0101));
        assert_eq!(plans[0].rotation, 2);
        assert_eq!(plans[0].config, ClientBuildPlanConfig::None);
        let world = session.loaded_world_state().unwrap();
        let graph = world.graph();
        let tile = graph
            .tile(plans[0].tile.0 as usize, plans[0].tile.1 as usize)
            .unwrap();
        assert_eq!(tile.block_id, 0);
        assert!(tile.building_center_index.is_none());
    }

    #[test]
    fn maybe_apply_auto_build_plans_prefers_core_conflict_tile_for_reject_path() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        let args = parse_args(sample_args(&[
            "--plan-break-near-player",
            "--plan-place-conflict-near-player",
            "0x0101:1",
        ]))
        .unwrap();
        let mut applied = false;

        maybe_apply_auto_build_plans(
            &mut session,
            &args,
            &[ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 32.0,
                y: 32.0,
            }],
            &mut applied,
        );

        let input = session.snapshot_input();
        assert!(applied);
        assert_eq!(
            input.plans,
            Some(vec![ClientBuildPlan {
                tile: (4, 4),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 1,
                config: ClientBuildPlanConfig::None,
            },])
        );
    }

    #[test]
    fn maybe_apply_auto_build_plans_applies_requested_configs() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        ingest_sample_world(&mut session);
        session.snapshot_input_mut().selected_block_id = Some(0x0103);
        session.snapshot_input_mut().selected_rotation = 2;
        let args = parse_args(sample_args(&[
            "--plan-place-conflict-near-player",
            "0x0101:1;string=router",
            "--plan-place-near-player",
            "selected;bytes=0102",
        ]))
        .unwrap();
        let mut applied = false;

        maybe_apply_auto_build_plans(
            &mut session,
            &args,
            &[ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 32.0,
                y: 32.0,
            }],
            &mut applied,
        );

        let plans = session.snapshot_input().plans.clone().unwrap_or_default();
        assert!(applied);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].tile, (4, 4));
        assert_eq!(plans[0].block_id, Some(0x0101));
        assert_eq!(plans[0].rotation, 1);
        assert_eq!(
            plans[0].config,
            ClientBuildPlanConfig::String("router".to_string())
        );
        assert_eq!(plans[1].block_id, Some(0x0103));
        assert_eq!(plans[1].rotation, 2);
        assert_eq!(plans[1].config, ClientBuildPlanConfig::Bytes(vec![1, 2]));
    }

    #[test]
    fn sync_runtime_build_selection_state_tracks_latest_non_breaking_plan() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[])).unwrap();
        {
            let input = session.snapshot_input_mut();
            input.building = false;
            input.selected_block_id = Some(0x0100);
            input.selected_rotation = 7;
            input.plans = Some(vec![
                ClientBuildPlan {
                    tile: (1, 1),
                    breaking: false,
                    block_id: Some(0x0101),
                    rotation: 1,
                    config: ClientBuildPlanConfig::None,
                },
                ClientBuildPlan {
                    tile: (2, 2),
                    breaking: true,
                    block_id: None,
                    rotation: 0,
                    config: ClientBuildPlanConfig::None,
                },
                ClientBuildPlan {
                    tile: (3, 3),
                    breaking: false,
                    block_id: Some(0x0103),
                    rotation: 3,
                    config: ClientBuildPlanConfig::None,
                },
            ]);
        }

        sync_runtime_build_selection_state(&mut session, &args);

        let input = session.snapshot_input();
        assert!(input.building);
        assert_eq!(input.selected_block_id, Some(0x0103));
        assert_eq!(input.selected_rotation, 3);
    }

    #[test]
    fn sync_runtime_build_selection_state_clears_building_when_queue_empty_without_override() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[])).unwrap();
        {
            let input = session.snapshot_input_mut();
            input.building = true;
            input.plans = Some(Vec::new());
        }

        sync_runtime_build_selection_state(&mut session, &args);

        assert!(!session.snapshot_input().building);
    }

    #[test]
    fn sync_runtime_build_selection_state_preserves_runtime_building_under_live_capture() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&["--intent-live-sampling"])).unwrap();
        {
            let input = session.snapshot_input_mut();
            input.building = true;
            input.plans = Some(Vec::new());
        }

        sync_runtime_build_selection_state(&mut session, &args);

        assert!(session.snapshot_input().building);
    }

    #[test]
    fn sync_runtime_build_selection_state_respects_snapshot_building_override() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&["--snapshot-no-building"])).unwrap();
        {
            let input = session.snapshot_input_mut();
            input.building = true;
            input.plans = Some(vec![ClientBuildPlan {
                tile: (4, 4),
                breaking: false,
                block_id: Some(0x0102),
                rotation: 2,
                config: ClientBuildPlanConfig::None,
            }]);
        }

        sync_runtime_build_selection_state(&mut session, &args);

        let input = session.snapshot_input();
        assert!(!input.building);
        assert_eq!(input.selected_block_id, Some(0x0102));
        assert_eq!(input.selected_rotation, 2);
    }

    #[test]
    fn movement_probe_steps_position_once_per_snapshot_interval() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--move-step-x",
            "2.0",
            "--move-step-y",
            "3.0",
        ]))
        .unwrap();
        let mut movement_probe = MovementProbeController::new(args.movement_probe.unwrap());

        let input = session.snapshot_input_mut();
        input.unit_id = Some(77);
        input.dead = false;
        input.position = Some((10.0, 20.0));
        input.pointer = None;

        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            Some(&mut movement_probe),
            None,
            500,
            1_000,
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.position, Some((12.0, 23.0)));
        assert_eq!(input.view_center, Some((12.0, 23.0)));
        assert_eq!(input.velocity, (2.0, 3.0));
        let expected_heading = 3.0f32.atan2(2.0).to_degrees();
        assert_eq!(input.rotation, expected_heading);
        assert_eq!(input.base_rotation, expected_heading);
        assert_eq!(input.pointer, Some((12.0, 23.0)));
        assert_eq!(movement_probe.last_step_at_ms(), Some(1_000));

        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            Some(&mut movement_probe),
            None,
            500,
            1_200,
        );
        assert_eq!(session.snapshot_input_mut().position, Some((12.0, 23.0)));

        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            Some(&mut movement_probe),
            None,
            500,
            1_500,
        );
        assert_eq!(session.snapshot_input_mut().position, Some((14.0, 26.0)));
        assert_eq!(session.snapshot_input_mut().pointer, Some((14.0, 26.0)));
        assert_eq!(movement_probe.last_step_at_ms(), Some(1_500));
    }

    #[test]
    fn live_intent_mapper_applies_intents_and_release_edges_without_movement_probe() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&[
            "--intent-delay-ms",
            "0",
            "--intent-spacing-ms",
            "100",
            "--intent-snapshot",
            "1:0:16:24:fire,boost:88,99",
            "--intent-snapshot",
            "0:0:32:48:chat:none",
        ]))
        .unwrap();
        let mut live_intent_mapper = LiveIntentMapperController::new(
            args.live_intent_schedule.clone(),
            args.live_intent_sampling_mode,
        );

        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            None,
            Some(&mut live_intent_mapper),
            500,
            0,
        );
        {
            let input = session.snapshot_input_mut();
            assert_eq!(input.velocity, (1.0, 0.0));
            assert_eq!(input.pointer, Some((16.0, 24.0)));
            assert_eq!(input.mining_tile, Some((88, 99)));
            assert_eq!(input.rotation, 0.0);
            assert_eq!(input.base_rotation, 0.0);
            assert!(input.shooting);
            assert!(input.boosting);
            assert!(!input.chatting);
        }

        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            None,
            Some(&mut live_intent_mapper),
            500,
            100,
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.velocity, (0.0, 0.0));
        assert_eq!(input.pointer, Some((32.0, 48.0)));
        assert_eq!(input.mining_tile, None);
        assert!(!input.shooting);
        assert!(!input.boosting);
        assert!(input.chatting);
    }

    #[test]
    fn apply_live_intents_to_snapshot_keeps_interact_out_of_snapshot_flags() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        {
            let input = session.snapshot_input_mut();
            input.shooting = true;
            input.boosting = true;
            input.chatting = true;
        }

        let state = LiveIntentState {
            move_axis: (0.0, 0.0),
            aim_axis: (18.0, 30.0),
            mining_tile: Some((4, 6)),
            building: true,
            active_actions: vec![BinaryAction::Interact],
            pressed_actions: vec![BinaryAction::Interact],
            released_actions: Vec::new(),
        };

        apply_live_intents_to_snapshot(&mut session, &state);

        let input = session.snapshot_input_mut();
        assert_eq!(input.pointer, Some((18.0, 30.0)));
        assert_eq!(input.mining_tile, Some((4, 6)));
        assert!(input.building);
        assert!(!input.shooting);
        assert!(!input.boosting);
        assert!(!input.chatting);
    }

    #[test]
    fn sample_runtime_intent_snapshot_reads_current_snapshot_flags() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        {
            let input = session.snapshot_input_mut();
            input.position = Some((5.0, 6.0));
            input.pointer = None;
            input.velocity = (1.5, -2.5);
            input.mining_tile = Some((9, 11));
            input.building = true;
            input.shooting = true;
            input.chatting = true;
        }

        let sampled = sample_runtime_intent_snapshot(&mut session);
        assert_eq!(sampled.move_axis, (1.5, -2.5));
        assert_eq!(sampled.aim_axis, (5.0, 6.0));
        assert_eq!(sampled.mining_tile, Some((9, 11)));
        assert!(sampled.building);
        assert_eq!(
            sampled.active_actions,
            vec![BinaryAction::Fire, BinaryAction::Chat]
        );
    }

    #[test]
    fn live_intent_mapper_samples_runtime_snapshot_without_schedule() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let args = parse_args(sample_args(&["--intent-live-sampling"])).unwrap();
        let mut live_intent_mapper = build_live_intent_mapper(&args)
            .expect("live sampling flag should enable runtime capture");

        {
            let input = session.snapshot_input_mut();
            input.position = Some((4.0, 8.0));
            input.pointer = Some((16.0, 24.0));
            input.velocity = (1.0, 0.0);
            input.building = true;
            input.shooting = true;
            input.boosting = true;
        }
        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            None,
            Some(&mut live_intent_mapper),
            500,
            0,
        );
        assert_eq!(
            live_intent_mapper.state().pressed_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert!(live_intent_mapper.state().building);
        assert!(live_intent_mapper.state().released_actions.is_empty());

        {
            let input = session.snapshot_input_mut();
            input.pointer = Some((32.0, 48.0));
            input.velocity = (0.0, 0.0);
            input.building = false;
            input.shooting = false;
            input.boosting = false;
        }
        maybe_apply_runtime_snapshot_overrides(
            &mut session,
            &args,
            None,
            Some(&mut live_intent_mapper),
            500,
            100,
        );

        let input = session.snapshot_input_mut();
        assert_eq!(input.pointer, Some((32.0, 48.0)));
        assert_eq!(input.velocity, (0.0, 0.0));
        assert!(!input.building);
        assert!(!input.shooting);
        assert!(!input.boosting);
        assert_eq!(
            live_intent_mapper.state().released_actions,
            vec![BinaryAction::Fire, BinaryAction::Boost]
        );
        assert!(!live_intent_mapper.state().building);
    }

    #[test]
    fn live_intent_schedule_override_yields_back_to_runtime_sampling_after_due_tick() {
        let args = parse_args(sample_args(&[
            "--intent-delay-ms",
            "0",
            "--intent-snapshot",
            "1:0:16:24:fire:none",
        ]))
        .unwrap();
        let mut live_intent_mapper = LiveIntentMapperController::new(
            args.live_intent_schedule.clone(),
            args.live_intent_sampling_mode,
        );
        let runtime_snapshot = InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (9.0, 9.0),
            mining_tile: None,
            building: false,
            active_actions: vec![],
        };

        assert!(live_intent_mapper.advance(&runtime_snapshot, 0));
        assert!(live_intent_mapper
            .state()
            .is_action_active(BinaryAction::Fire));
        assert!(live_intent_mapper.advance(&runtime_snapshot, 50));
        assert!(!live_intent_mapper
            .state()
            .is_action_active(BinaryAction::Fire));
        assert_eq!(live_intent_mapper.state().move_axis, (0.0, 0.0));
        assert_eq!(live_intent_mapper.state().aim_axis, (9.0, 9.0));
        assert_eq!(
            live_intent_mapper.state().released_actions,
            vec![BinaryAction::Fire]
        );
    }

    #[test]
    fn live_intent_mapper_controller_uses_configured_sampling_mode() {
        let args = parse_args(sample_args(&["--intent-live-sampling"])).unwrap();
        let mut live_intent_mapper = LiveIntentMapperController::new(
            args.live_intent_schedule.clone(),
            args.live_intent_sampling_mode,
        );
        let runtime_snapshot = InputSnapshot {
            move_axis: (0.0, 0.0),
            aim_axis: (0.0, 0.0),
            mining_tile: None,
            building: true,
            active_actions: vec![BinaryAction::Fire],
        };

        assert!(live_intent_mapper.advance(&runtime_snapshot, 0));
        assert_eq!(
            live_intent_mapper.state().pressed_actions,
            vec![BinaryAction::Fire]
        );
        assert!(live_intent_mapper.state().building);
        assert!(!live_intent_mapper.advance(&runtime_snapshot, 1));
        assert!(live_intent_mapper.state().pressed_actions.is_empty());
    }

    #[test]
    fn live_intent_schedule_preserves_transient_edges_when_multiple_due_snapshots_share_tick() {
        let schedule = vec![
            ScheduledIntentSnapshot {
                not_before_ms: 0,
                snapshot: InputSnapshot {
                    move_axis: (1.0, 0.0),
                    aim_axis: (16.0, 24.0),
                    mining_tile: None,
                    building: true,
                    active_actions: vec![BinaryAction::Fire],
                },
            },
            ScheduledIntentSnapshot {
                not_before_ms: 0,
                snapshot: InputSnapshot {
                    move_axis: (0.0, 0.0),
                    aim_axis: (32.0, 48.0),
                    mining_tile: Some((9, 10)),
                    building: false,
                    active_actions: vec![],
                },
            },
        ];
        let mut live_intent_mapper =
            LiveIntentMapperController::new(schedule, IntentSamplingMode::LiveSampling);
        let runtime_snapshot = InputSnapshot {
            move_axis: (9.0, 9.0),
            aim_axis: (99.0, 99.0),
            mining_tile: None,
            building: false,
            active_actions: vec![BinaryAction::Boost],
        };

        assert!(live_intent_mapper.advance(&runtime_snapshot, 0));
        assert_eq!(live_intent_mapper.state().move_axis, (0.0, 0.0));
        assert_eq!(live_intent_mapper.state().aim_axis, (32.0, 48.0));
        assert_eq!(live_intent_mapper.state().mining_tile, Some((9, 10)));
        assert_eq!(
            live_intent_mapper.state().pressed_actions,
            vec![BinaryAction::Fire]
        );
        assert_eq!(
            live_intent_mapper.state().released_actions,
            vec![BinaryAction::Fire]
        );
        assert!(!live_intent_mapper
            .state()
            .is_action_active(BinaryAction::Fire));
    }

    #[test]
    fn latest_runtime_view_center_prefers_latest_runtime_event_over_snapshot_state() {
        let events = vec![
            ClientSessionEvent::PlayerPositionUpdated { x: 10.0, y: 20.0 },
            ClientSessionEvent::CameraPositionUpdated { x: 30.0, y: 40.0 },
        ];

        let center = latest_runtime_view_center(&events, Some((1.0, 2.0)), Some((3.0, 4.0)));

        assert_eq!(center, Some((30.0, 40.0)));
    }

    #[test]
    fn latest_runtime_view_center_uses_newer_player_event_after_camera() {
        let events = vec![
            ClientSessionEvent::CameraPositionUpdated { x: 30.0, y: 40.0 },
            ClientSessionEvent::PlayerPositionUpdated { x: 10.0, y: 20.0 },
        ];

        let center = latest_runtime_view_center(&events, Some((1.0, 2.0)), Some((3.0, 4.0)));

        assert_eq!(center, Some((10.0, 20.0)));
    }

    #[test]
    fn latest_runtime_view_center_falls_back_to_snapshot_state() {
        let events = vec![ClientSessionEvent::SnapshotReceived(
            HighFrequencyRemoteMethod::StateSnapshot,
        )];

        assert_eq!(
            latest_runtime_view_center(&events, Some((5.0, 6.0)), Some((7.0, 8.0))),
            Some((5.0, 6.0))
        );
        assert_eq!(
            latest_runtime_view_center(&events, None, Some((7.0, 8.0))),
            Some((7.0, 8.0))
        );
        assert_eq!(latest_runtime_view_center(&events, None, None), None);
    }

    #[test]
    fn resolved_runtime_view_center_falls_back_to_loaded_player_position() {
        let events = vec![ClientSessionEvent::SnapshotReceived(
            HighFrequencyRemoteMethod::StateSnapshot,
        )];

        assert_eq!(
            resolved_runtime_view_center(&events, None, None, (9.0, 10.0)),
            Some((9.0, 10.0))
        );
    }

    #[test]
    fn should_render_ascii_on_events_accepts_runtime_view_events() {
        let world_ready = [ClientSessionEvent::WorldStreamReady {
            stream_id: 7,
            map_width: 8,
            map_height: 8,
            player_id: 7,
            ready_to_enter_world: true,
        }];
        let camera = [ClientSessionEvent::CameraPositionUpdated { x: 1.0, y: 2.0 }];
        let player_spawn = [ClientSessionEvent::PlayerSpawned {
            player_id: 7,
            x: 3.0,
            y: 4.0,
        }];
        let player_move = [ClientSessionEvent::PlayerPositionUpdated { x: 5.0, y: 6.0 }];
        let unrelated = [ClientSessionEvent::SnapshotReceived(
            HighFrequencyRemoteMethod::StateSnapshot,
        )];

        assert!(should_render_ascii_on_events(&world_ready));
        assert!(should_render_ascii_on_events(&camera));
        assert!(should_render_ascii_on_events(&player_spawn));
        assert!(should_render_ascii_on_events(&player_move));
        assert!(!should_render_ascii_on_events(&unrelated));
    }

    #[test]
    fn collect_authoritative_runtime_scene_object_ids_filters_runtime_overlay_objects() {
        let ids = collect_authoritative_runtime_scene_object_ids(&[
            RenderObject {
                id: "core:team-1".to_string(),
                layer: 1,
                x: 0.0,
                y: 0.0,
            },
            RenderObject {
                id: "block:runtime-construct:125:90:257".to_string(),
                layer: 16,
                x: 0.0,
                y: 0.0,
            },
            RenderObject {
                id: "terrain:runtime-deconstruct:125:90".to_string(),
                layer: 16,
                x: 0.0,
                y: 0.0,
            },
            RenderObject {
                id: "marker:runtime-health:125:90".to_string(),
                layer: 32,
                x: 0.0,
                y: 0.0,
            },
            RenderObject {
                id: "plan:runtime-place:0:125:90".to_string(),
                layer: 21,
                x: 0.0,
                y: 0.0,
            },
        ]);

        assert_eq!(
            ids,
            vec![
                "block:runtime-construct:125:90:257".to_string(),
                "terrain:runtime-deconstruct:125:90".to_string(),
                "marker:runtime-health:125:90".to_string(),
            ]
        );
    }

    #[test]
    fn first_connect_redirect_target_selects_first_redirect_event() {
        let events = vec![
            ClientSessionEvent::ServerMessage {
                message: "hello".to_string(),
            },
            ClientSessionEvent::ConnectRedirectRequested {
                ip: "127.0.0.2".to_string(),
                port: 7001,
            },
            ClientSessionEvent::ConnectRedirectRequested {
                ip: "127.0.0.3".to_string(),
                port: 7002,
            },
        ];

        assert_eq!(
            first_connect_redirect_target(&events),
            Some(("127.0.0.2".to_string(), 7001))
        );
    }

    #[test]
    fn first_server_restart_reconnect_delay_ms_selects_first_matching_kick() {
        let events = vec![
            ClientSessionEvent::Kicked {
                reason_text: Some("bye".to_string()),
                reason_ordinal: Some(7),
                duration_ms: Some(500),
            },
            ClientSessionEvent::Kicked {
                reason_text: None,
                reason_ordinal: Some(KICK_REASON_SERVER_RESTARTING_ORDINAL),
                duration_ms: Some(2_500),
            },
            ClientSessionEvent::Kicked {
                reason_text: None,
                reason_ordinal: Some(KICK_REASON_SERVER_RESTARTING_ORDINAL),
                duration_ms: Some(3_000),
            },
        ];

        assert_eq!(
            first_server_restart_reconnect_delay_ms(&events),
            Some(2_500)
        );
    }

    #[test]
    fn first_server_restart_reconnect_delay_ms_defaults_missing_duration_to_zero() {
        let events = vec![ClientSessionEvent::Kicked {
            reason_text: None,
            reason_ordinal: Some(KICK_REASON_SERVER_RESTARTING_ORDINAL),
            duration_ms: None,
        }];

        assert_eq!(first_server_restart_reconnect_delay_ms(&events), Some(0));
    }

    #[test]
    fn resolve_redirect_server_addr_accepts_ipv4_literal() {
        let resolved = resolve_redirect_server_addr("127.0.0.1", 6567);

        assert_eq!(resolved, Some("127.0.0.1:6567".parse().unwrap()));
    }

    #[test]
    fn resolve_redirect_server_addr_rejects_invalid_port() {
        assert_eq!(resolve_redirect_server_addr("127.0.0.1", -1), None);
        assert_eq!(resolve_redirect_server_addr("127.0.0.1", 70_000), None);
    }
}
