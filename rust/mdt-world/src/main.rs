use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

const USAGE: &str = "usage: mdt-world <output-dir> [--input-root <dir>]";

struct CliArgs {
    output_dir: PathBuf,
    input_root: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_cli_args()?;
    let output_dir = &args.output_dir;
    create_dir_all_with_context(output_dir)?;

    let repo_root = repo_root_from_manifest_dir()?;
    let tests_resources = repo_root
        .join("tests")
        .join("src")
        .join("test")
        .join("resources");

    let connect_packet_hex = read_text_from_candidates(
        "connect-packet.hex",
        &connect_candidates(&args, &tests_resources),
    )?;
    write_output_with_context(output_dir, "connect-packet.hex", &connect_packet_hex)?;

    let snapshot_goldens = read_text_from_candidates(
        "snapshot-goldens.txt",
        &snapshot_candidates(&args, &tests_resources),
    )?;
    write_output_with_context(output_dir, "snapshot-goldens.txt", &snapshot_goldens)?;

    let world_stream_hex = read_text_from_candidates(
        "world-stream.hex",
        &world_stream_candidates(&args, &tests_resources, &repo_root),
    )?;
    write_output_with_context(output_dir, "world-stream.hex", &world_stream_hex)?;

    let connect_payload = decode_hex_text(&connect_packet_hex)?;
    let packet_serializer_text =
        mdt_protocol::generate_packet_serializer_goldens(&connect_payload)?;
    write_output_with_context(
        output_dir,
        "packet-serializer-goldens.txt",
        packet_serializer_text,
    )?;
    write_output_with_context(
        output_dir,
        "framework-message-goldens.txt",
        mdt_protocol::generate_framework_message_goldens()?,
    )?;
    write_output_with_context(
        output_dir,
        "typeio-goldens.txt",
        mdt_typeio::generate_typeio_goldens(),
    )?;

    let compressed = decode_hex_text(&world_stream_hex)?;
    write_output_with_context(
        output_dir,
        "world-stream-transport-goldens.txt",
        mdt_protocol::generate_world_stream_transport_goldens(&compressed)?,
    )?;

    let summary = mdt_world::parse_world_load_goldens(&compressed)?;
    let text = mdt_world::format_world_load_goldens(&summary);
    write_output_with_context(output_dir, "world-load-goldens.txt", text)?;

    let model = mdt_world::parse_world_model(&compressed)?;
    let model_text = mdt_world::format_world_model_goldens(&model);
    write_output_with_context(output_dir, "world-model-goldens.txt", model_text)?;

    let team_plan_bytes = mdt_world::generate_team_plan_sample_bytes();
    let team_plan_summary = mdt_world::parse_team_plan_goldens(&team_plan_bytes)?;
    let team_plan_text = mdt_world::format_team_plan_goldens(&team_plan_summary);
    write_output_with_context(output_dir, "team-plan-goldens.txt", team_plan_text)?;

    let static_fog_bytes = mdt_world::generate_static_fog_sample_bytes();
    let static_fog_summary = mdt_world::parse_static_fog_goldens(&static_fog_bytes)?;
    let static_fog_text = mdt_world::format_static_fog_goldens(&static_fog_summary);
    write_output_with_context(output_dir, "static-fog-goldens.txt", static_fog_text)?;

    let marker_bytes = mdt_world::generate_marker_sample_bytes();
    let marker_summary = mdt_world::parse_marker_goldens(&marker_bytes)?;
    let marker_text = mdt_world::format_marker_goldens(&marker_summary);
    write_output_with_context(output_dir, "marker-goldens.txt", marker_text)?;

    let payload_campaign_compound_summary =
        mdt_world::generate_payload_campaign_compound_goldens()?;
    let payload_campaign_compound_text =
        mdt_world::format_payload_campaign_compound_goldens(&payload_campaign_compound_summary);
    write_output_with_context(
        output_dir,
        "payload-campaign-compound-goldens.txt",
        payload_campaign_compound_text,
    )?;

    let world_graph_summary = mdt_world::parse_world_graph_goldens(&compressed)?;
    let world_graph_text = mdt_world::format_world_graph_goldens(&world_graph_summary);
    write_output_with_context(output_dir, "world-graph-goldens.txt", world_graph_text)?;

    let world_session_summary = mdt_world::parse_world_session_goldens(&compressed)?;
    let world_session_text = mdt_world::format_world_session_goldens(&world_session_summary);
    write_output_with_context(output_dir, "world-session-goldens.txt", world_session_text)?;

    let world_bootstrap_summary = mdt_world::parse_world_bootstrap_goldens(&compressed)?;
    let world_bootstrap_text = mdt_world::format_world_bootstrap_goldens(&world_bootstrap_summary);
    write_output_with_context(
        output_dir,
        "world-bootstrap-goldens.txt",
        world_bootstrap_text,
    )?;

    let world_enter_init_summary = mdt_world::parse_world_enter_init_goldens(&compressed)?;
    let world_enter_init_text =
        mdt_world::format_world_enter_init_goldens(&world_enter_init_summary);
    write_output_with_context(output_dir, "world-enter-init-goldens.txt", world_enter_init_text)?;

    let world_enter_init_state_summary =
        mdt_world::parse_world_enter_init_state_goldens(&compressed)?;
    let world_enter_init_state_text =
        mdt_world::format_world_enter_init_state_goldens(&world_enter_init_state_summary);
    write_output_with_context(
        output_dir,
        "world-enter-init-state-goldens.txt",
        world_enter_init_state_text,
    )?;

    let world_enter_component_summary =
        mdt_world::parse_world_enter_component_goldens(&compressed)?;
    let world_enter_component_text =
        mdt_world::format_world_enter_component_goldens(&world_enter_component_summary);
    write_output_with_context(
        output_dir,
        "world-enter-component-goldens.txt",
        world_enter_component_text,
    )?;

    let world_enter_surface_summary = mdt_world::parse_world_enter_surface_goldens(&compressed)?;
    let world_enter_surface_text =
        mdt_world::format_world_enter_surface_goldens(&world_enter_surface_summary);
    write_output_with_context(
        output_dir,
        "world-enter-surface-goldens.txt",
        world_enter_surface_text,
    )?;

    let world_enter_layout_summary = mdt_world::parse_world_enter_layout_goldens(&compressed)?;
    let world_enter_layout_text =
        mdt_world::format_world_enter_layout_goldens(&world_enter_layout_summary);
    write_output_with_context(
        output_dir,
        "world-enter-layout-goldens.txt",
        world_enter_layout_text,
    )?;

    let world_enter_page_summary = mdt_world::parse_world_enter_page_goldens(&compressed)?;
    let world_enter_page_text =
        mdt_world::format_world_enter_page_goldens(&world_enter_page_summary);
    write_output_with_context(output_dir, "world-enter-page-goldens.txt", world_enter_page_text)?;

    let world_enter_screen_summary = mdt_world::parse_world_enter_screen_goldens(&compressed)?;
    let world_enter_screen_text =
        mdt_world::format_world_enter_screen_goldens(&world_enter_screen_summary);
    write_output_with_context(
        output_dir,
        "world-enter-screen-goldens.txt",
        world_enter_screen_text,
    )?;

    let world_enter_transition_summary =
        mdt_world::parse_world_enter_transition_goldens(&compressed)?;
    let world_enter_transition_text =
        mdt_world::format_world_enter_transition_goldens(&world_enter_transition_summary);
    write_output_with_context(
        output_dir,
        "world-enter-transition-goldens.txt",
        world_enter_transition_text,
    )?;

    let world_enter_world_ready_summary =
        mdt_world::parse_world_enter_world_ready_goldens(&compressed)?;
    let world_enter_world_ready_text =
        mdt_world::format_world_enter_world_ready_goldens(&world_enter_world_ready_summary);
    write_output_with_context(
        output_dir,
        "world-enter-world-ready-goldens.txt",
        world_enter_world_ready_text,
    )?;

    let world_enter_play_summary = mdt_world::parse_world_enter_play_goldens(&compressed)?;
    let world_enter_play_text =
        mdt_world::format_world_enter_play_goldens(&world_enter_play_summary);
    write_output_with_context(output_dir, "world-enter-play-goldens.txt", world_enter_play_text)?;

    let world_enter_runtime_summary = mdt_world::parse_world_enter_runtime_goldens(&compressed)?;
    let world_enter_runtime_text =
        mdt_world::format_world_enter_runtime_goldens(&world_enter_runtime_summary);
    write_output_with_context(
        output_dir,
        "world-enter-runtime-goldens.txt",
        world_enter_runtime_text,
    )?;

    let world_enter_frame_summary = mdt_world::parse_world_enter_frame_goldens(&compressed)?;
    let world_enter_frame_text =
        mdt_world::format_world_enter_frame_goldens(&world_enter_frame_summary);
    write_output_with_context(output_dir, "world-enter-frame-goldens.txt", world_enter_frame_text)?;

    let world_enter_loop_summary = mdt_world::parse_world_enter_loop_goldens(&compressed)?;
    let world_enter_loop_text =
        mdt_world::format_world_enter_loop_goldens(&world_enter_loop_summary);
    write_output_with_context(output_dir, "world-enter-loop-goldens.txt", world_enter_loop_text)?;

    let world_enter_render_summary = mdt_world::parse_world_enter_render_goldens(&compressed)?;
    let world_enter_render_text =
        mdt_world::format_world_enter_render_goldens(&world_enter_render_summary);
    write_output_with_context(
        output_dir,
        "world-enter-render-goldens.txt",
        world_enter_render_text,
    )?;

    let world_enter_scene_frame_summary =
        mdt_world::parse_world_enter_scene_frame_goldens(&compressed)?;
    let world_enter_scene_frame_text =
        mdt_world::format_world_enter_scene_frame_goldens(&world_enter_scene_frame_summary);
    write_output_with_context(
        output_dir,
        "world-enter-scene-frame-goldens.txt",
        world_enter_scene_frame_text,
    )?;

    let world_enter_scene_present_summary =
        mdt_world::parse_world_enter_scene_present_goldens(&compressed)?;
    let world_enter_scene_present_text =
        mdt_world::format_world_enter_scene_present_goldens(&world_enter_scene_present_summary);
    write_output_with_context(
        output_dir,
        "world-enter-scene-present-goldens.txt",
        world_enter_scene_present_text,
    )?;

    let world_enter_world_shell_summary =
        mdt_world::parse_world_enter_world_shell_goldens(&compressed)?;
    let world_enter_world_shell_text =
        mdt_world::format_world_enter_world_shell_goldens(&world_enter_world_shell_summary);
    write_output_with_context(
        output_dir,
        "world-enter-world-shell-goldens.txt",
        world_enter_world_shell_text,
    )?;

    let world_enter_screen_activation_summary =
        mdt_world::parse_world_enter_screen_activation_goldens(&compressed)?;
    let world_enter_screen_activation_text =
        mdt_world::format_world_enter_screen_activation_goldens(
            &world_enter_screen_activation_summary,
        );
    write_with_context(
        output_dir.join("world-enter-screen-activation-goldens.txt"),
        world_enter_screen_activation_text,
    )?;

    let world_enter_session_activation_summary =
        mdt_world::parse_world_enter_session_activation_goldens(&compressed)?;
    let world_enter_session_activation_text =
        mdt_world::format_world_enter_session_activation_goldens(
            &world_enter_session_activation_summary,
        );
    write_with_context(
        output_dir.join("world-enter-session-activation-goldens.txt"),
        world_enter_session_activation_text,
    )?;

    let world_enter_connection_ready_summary =
        mdt_world::parse_world_enter_connection_ready_goldens(&compressed)?;
    let world_enter_connection_ready_text = mdt_world::format_world_enter_connection_ready_goldens(
        &world_enter_connection_ready_summary,
    );
    write_with_context(
        output_dir.join("world-enter-connection-ready-goldens.txt"),
        world_enter_connection_ready_text,
    )?;

    let world_enter_ready_proof_summary =
        mdt_world::parse_world_enter_ready_proof_goldens(&compressed)?;
    let world_enter_ready_proof_text =
        mdt_world::format_world_enter_ready_proof_goldens(&world_enter_ready_proof_summary);
    write_with_context(
        output_dir.join("world-enter-ready-proof-goldens.txt"),
        world_enter_ready_proof_text,
    )?;

    let world_enter_room_entry_proof_summary =
        mdt_world::parse_world_enter_room_entry_proof_goldens(&compressed)?;
    let world_enter_room_entry_proof_text = mdt_world::format_world_enter_room_entry_proof_goldens(
        &world_enter_room_entry_proof_summary,
    );
    write_with_context(
        output_dir.join("world-enter-room-entry-proof-goldens.txt"),
        world_enter_room_entry_proof_text,
    )?;

    let world_enter_world_loop_proof_summary =
        mdt_world::parse_world_enter_world_loop_proof_goldens(&compressed)?;
    let world_enter_world_loop_proof_text = mdt_world::format_world_enter_world_loop_proof_goldens(
        &world_enter_world_loop_proof_summary,
    );
    write_with_context(
        output_dir.join("world-enter-world-loop-proof-goldens.txt"),
        world_enter_world_loop_proof_text,
    )?;

    let world_enter_stable_session_proof_summary =
        mdt_world::parse_world_enter_stable_session_proof_goldens(&compressed)?;
    let world_enter_stable_session_proof_text =
        mdt_world::format_world_enter_stable_session_proof_goldens(
            &world_enter_stable_session_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-session-proof-goldens.txt"),
        world_enter_stable_session_proof_text,
    )?;

    let world_enter_stable_world_proof_summary =
        mdt_world::parse_world_enter_stable_world_proof_goldens(&compressed)?;
    let world_enter_stable_world_proof_text =
        mdt_world::format_world_enter_stable_world_proof_goldens(
            &world_enter_stable_world_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-world-proof-goldens.txt"),
        world_enter_stable_world_proof_text,
    )?;

    let world_enter_stable_bootstrap_proof_summary =
        mdt_world::parse_world_enter_stable_bootstrap_proof_goldens(&compressed)?;
    let world_enter_stable_bootstrap_proof_text =
        mdt_world::format_world_enter_stable_bootstrap_proof_goldens(
            &world_enter_stable_bootstrap_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-bootstrap-proof-goldens.txt"),
        world_enter_stable_bootstrap_proof_text,
    )?;

    let world_enter_stable_content_proof_summary =
        mdt_world::parse_world_enter_stable_content_proof_goldens(&compressed)?;
    let world_enter_stable_content_proof_text =
        mdt_world::format_world_enter_stable_content_proof_goldens(
            &world_enter_stable_content_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-content-proof-goldens.txt"),
        world_enter_stable_content_proof_text,
    )?;

    let world_enter_stable_entry_proof_summary =
        mdt_world::parse_world_enter_stable_entry_proof_goldens(&compressed)?;
    let world_enter_stable_entry_proof_text =
        mdt_world::format_world_enter_stable_entry_proof_goldens(
            &world_enter_stable_entry_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-entry-proof-goldens.txt"),
        world_enter_stable_entry_proof_text,
    )?;

    let world_enter_stable_stage_proof_summary =
        mdt_world::parse_world_enter_stable_stage_proof_goldens(&compressed)?;
    let world_enter_stable_stage_proof_text =
        mdt_world::format_world_enter_stable_stage_proof_goldens(
            &world_enter_stable_stage_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-stage-proof-goldens.txt"),
        world_enter_stable_stage_proof_text,
    )?;

    let world_enter_stable_envelope_proof_summary =
        mdt_world::parse_world_enter_stable_envelope_proof_goldens(&compressed)?;
    let world_enter_stable_envelope_proof_text =
        mdt_world::format_world_enter_stable_envelope_proof_goldens(
            &world_enter_stable_envelope_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-envelope-proof-goldens.txt"),
        world_enter_stable_envelope_proof_text,
    )?;

    let world_enter_stable_ready_proof_summary =
        mdt_world::parse_world_enter_stable_ready_proof_goldens(&compressed)?;
    let world_enter_stable_ready_proof_text =
        mdt_world::format_world_enter_stable_ready_proof_goldens(
            &world_enter_stable_ready_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-ready-proof-goldens.txt"),
        world_enter_stable_ready_proof_text,
    )?;

    let world_enter_stable_room_entry_proof_summary =
        mdt_world::parse_world_enter_stable_room_entry_proof_goldens(&compressed)?;
    let world_enter_stable_room_entry_proof_text =
        mdt_world::format_world_enter_stable_room_entry_proof_goldens(
            &world_enter_stable_room_entry_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-room-entry-proof-goldens.txt"),
        world_enter_stable_room_entry_proof_text,
    )?;

    let world_enter_stable_world_loop_proof_summary =
        mdt_world::parse_world_enter_stable_world_loop_proof_goldens(&compressed)?;
    let world_enter_stable_world_loop_proof_text =
        mdt_world::format_world_enter_stable_world_loop_proof_goldens(
            &world_enter_stable_world_loop_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-world-loop-proof-goldens.txt"),
        world_enter_stable_world_loop_proof_text,
    )?;

    let world_enter_stable_playable_session_proof_summary =
        mdt_world::parse_world_enter_stable_playable_session_proof_goldens(&compressed)?;
    let world_enter_stable_playable_session_proof_text =
        mdt_world::format_world_enter_stable_playable_session_proof_goldens(
            &world_enter_stable_playable_session_proof_summary,
        );
    write_with_context(
        output_dir.join("world-enter-stable-playable-session-proof-goldens.txt"),
        world_enter_stable_playable_session_proof_text,
    )?;

    let world_enter_connection_confirmed_summary =
        mdt_world::parse_world_enter_connection_confirmed_goldens(&compressed)?;
    let world_enter_connection_confirmed_text =
        mdt_world::format_world_enter_connection_confirmed_goldens(
            &world_enter_connection_confirmed_summary,
        );
    write_with_context(
        output_dir.join("world-enter-connection-confirmed-goldens.txt"),
        world_enter_connection_confirmed_text,
    )?;

    let world_enter_player_join_summary =
        mdt_world::parse_world_enter_player_join_goldens(&compressed)?;
    let world_enter_player_join_text =
        mdt_world::format_world_enter_player_join_goldens(&world_enter_player_join_summary);
    write_with_context(
        output_dir.join("world-enter-player-join-goldens.txt"),
        world_enter_player_join_text,
    )?;

    let world_enter_interaction_ready_summary =
        mdt_world::parse_world_enter_interaction_ready_goldens(&compressed)?;
    let world_enter_interaction_ready_text =
        mdt_world::format_world_enter_interaction_ready_goldens(
            &world_enter_interaction_ready_summary,
        );
    write_with_context(
        output_dir.join("world-enter-interaction-ready-goldens.txt"),
        world_enter_interaction_ready_text,
    )?;

    let world_enter_snapshot_ready_summary =
        mdt_world::parse_world_enter_snapshot_ready_goldens(&compressed)?;
    let world_enter_snapshot_ready_text =
        mdt_world::format_world_enter_snapshot_ready_goldens(&world_enter_snapshot_ready_summary);
    write_with_context(
        output_dir.join("world-enter-snapshot-ready-goldens.txt"),
        world_enter_snapshot_ready_text,
    )?;

    let world_enter_snapshot_live_summary =
        mdt_world::parse_world_enter_snapshot_live_goldens(&compressed)?;
    let world_enter_snapshot_live_text =
        mdt_world::format_world_enter_snapshot_live_goldens(&world_enter_snapshot_live_summary);
    write_with_context(
        output_dir.join("world-enter-snapshot-live-goldens.txt"),
        world_enter_snapshot_live_text,
    )?;

    let world_enter_snapshot_apply_summary =
        mdt_world::parse_world_enter_snapshot_apply_goldens(&compressed)?;
    let world_enter_snapshot_apply_text =
        mdt_world::format_world_enter_snapshot_apply_goldens(&world_enter_snapshot_apply_summary);
    write_with_context(
        output_dir.join("world-enter-snapshot-apply-goldens.txt"),
        world_enter_snapshot_apply_text,
    )?;

    let world_enter_world_sync_summary =
        mdt_world::parse_world_enter_world_sync_goldens(&compressed)?;
    let world_enter_world_sync_text =
        mdt_world::format_world_enter_world_sync_goldens(&world_enter_world_sync_summary);
    write_with_context(
        output_dir.join("world-enter-world-sync-goldens.txt"),
        world_enter_world_sync_text,
    )?;

    let world_enter_sync_state_summary =
        mdt_world::parse_world_enter_sync_state_goldens(&compressed)?;
    let world_enter_sync_state_text =
        mdt_world::format_world_enter_sync_state_goldens(&world_enter_sync_state_summary);
    write_with_context(
        output_dir.join("world-enter-sync-state-goldens.txt"),
        world_enter_sync_state_text,
    )?;

    let world_enter_sync_loop_summary =
        mdt_world::parse_world_enter_sync_loop_goldens(&compressed)?;
    let world_enter_sync_loop_text =
        mdt_world::format_world_enter_sync_loop_goldens(&world_enter_sync_loop_summary);
    write_with_context(
        output_dir.join("world-enter-sync-loop-goldens.txt"),
        world_enter_sync_loop_text,
    )?;

    let world_enter_client_snapshot_summary =
        mdt_world::parse_world_enter_client_snapshot_goldens(&compressed)?;
    let world_enter_client_snapshot_text =
        mdt_world::format_world_enter_client_snapshot_goldens(&world_enter_client_snapshot_summary);
    write_with_context(
        output_dir.join("world-enter-client-snapshot-goldens.txt"),
        world_enter_client_snapshot_text,
    )?;

    let world_enter_client_snapshot_apply_summary =
        mdt_world::parse_world_enter_client_snapshot_apply_goldens(&compressed)?;
    let world_enter_client_snapshot_apply_text =
        mdt_world::format_world_enter_client_snapshot_apply_goldens(
            &world_enter_client_snapshot_apply_summary,
        );
    write_with_context(
        output_dir.join("world-enter-client-snapshot-apply-goldens.txt"),
        world_enter_client_snapshot_apply_text,
    )?;

    let world_enter_client_reconcile_summary =
        mdt_world::parse_world_enter_client_reconcile_goldens(&compressed)?;
    let world_enter_client_reconcile_text = mdt_world::format_world_enter_client_reconcile_goldens(
        &world_enter_client_reconcile_summary,
    );
    write_with_context(
        output_dir.join("world-enter-client-reconcile-goldens.txt"),
        world_enter_client_reconcile_text,
    )?;

    let world_enter_multiplayer_runtime_summary =
        mdt_world::parse_world_enter_multiplayer_runtime_goldens(&compressed)?;
    let world_enter_multiplayer_runtime_text =
        mdt_world::format_world_enter_multiplayer_runtime_goldens(
            &world_enter_multiplayer_runtime_summary,
        );
    write_with_context(
        output_dir.join("world-enter-multiplayer-runtime-goldens.txt"),
        world_enter_multiplayer_runtime_text,
    )?;

    let world_enter_multiplayer_session_summary =
        mdt_world::parse_world_enter_multiplayer_session_goldens(&compressed)?;
    let world_enter_multiplayer_session_text =
        mdt_world::format_world_enter_multiplayer_session_goldens(
            &world_enter_multiplayer_session_summary,
        );
    write_with_context(
        output_dir.join("world-enter-multiplayer-session-goldens.txt"),
        world_enter_multiplayer_session_text,
    )?;

    let world_enter_multiplayer_shell_summary =
        mdt_world::parse_world_enter_multiplayer_shell_goldens(&compressed)?;
    let world_enter_multiplayer_shell_text =
        mdt_world::format_world_enter_multiplayer_shell_goldens(
            &world_enter_multiplayer_shell_summary,
        );
    write_with_context(
        output_dir.join("world-enter-multiplayer-shell-goldens.txt"),
        world_enter_multiplayer_shell_text,
    )?;

    let world_enter_playable_session_summary =
        mdt_world::parse_world_enter_playable_session_goldens(&compressed)?;
    let world_enter_playable_session_text = mdt_world::format_world_enter_playable_session_goldens(
        &world_enter_playable_session_summary,
    );
    write_with_context(
        output_dir.join("world-enter-playable-session-goldens.txt"),
        world_enter_playable_session_text,
    )?;
    Ok(())
}

fn parse_cli_args() -> Result<CliArgs, Box<dyn Error>> {
    parse_cli_args_from(env::args().skip(1))
}

fn parse_cli_args_from(args: impl Iterator<Item = String>) -> Result<CliArgs, Box<dyn Error>> {
    let mut output_dir = None;
    let mut input_root = None;
    let mut args = args;

    while let Some(arg) = args.next() {
        if arg == "--input-root" || arg == "-i" {
            let value = args.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("missing value for --input-root\n{USAGE}"),
                )
            })?;
            set_input_root_once(&mut input_root, PathBuf::from(value))?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--input-root=") {
            set_input_root_once(&mut input_root, PathBuf::from(value))?;
            continue;
        }

        if output_dir.is_none() {
            output_dir = Some(PathBuf::from(arg));
            continue;
        }

        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unexpected argument: {arg}\n{USAGE}"),
        )
        .into());
    }

    let output_dir = output_dir.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing required <output-dir>\n{USAGE}"),
        )
    })?;

    Ok(CliArgs {
        output_dir,
        input_root,
    })
}

fn set_input_root_once(
    input_root: &mut Option<PathBuf>,
    value: PathBuf,
) -> Result<(), Box<dyn Error>> {
    if input_root.replace(value.clone()).is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "duplicate --input-root provided; latest value was {}",
                value.display()
            ),
        )
        .into());
    }
    Ok(())
}

fn connect_candidates(args: &CliArgs, tests_resources: &Path) -> Vec<PathBuf> {
    with_optional_input_root(
        "connect-packet.hex",
        args.input_root.as_deref(),
        vec![tests_resources.join("connect-packet.hex")],
    )
}

fn snapshot_candidates(args: &CliArgs, tests_resources: &Path) -> Vec<PathBuf> {
    with_optional_input_root(
        "snapshot-goldens.txt",
        args.input_root.as_deref(),
        vec![tests_resources.join("snapshot-goldens.txt")],
    )
}

fn world_stream_candidates(
    args: &CliArgs,
    tests_resources: &Path,
    repo_root: &Path,
) -> Vec<PathBuf> {
    if let Some(root) = args.input_root.as_deref() {
        return vec![root.join("world-stream.hex")];
    }

    vec![
        tests_resources.join("world-stream.hex"),
        repo_root
            .join("rust")
            .join("fixtures")
            .join("world-streams")
            .join("archipelago-6567-world-stream.hex"),
        repo_root
            .join("fixtures")
            .join("world-streams")
            .join("archipelago-6567-world-stream.hex"),
    ]
}

fn with_optional_input_root(
    file_name: &str,
    input_root: Option<&Path>,
    mut candidates: Vec<PathBuf>,
) -> Vec<PathBuf> {
    if let Some(root) = input_root {
        candidates.insert(0, root.join(file_name));
    }
    candidates
}

fn create_dir_all_with_context(path: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to create output directory {}: {err}",
                path.display()
            ),
        )
    })?;
    Ok(())
}

fn write_output_with_context(
    output_dir: &Path,
    file_name: &str,
    contents: impl AsRef<[u8]>,
) -> Result<(), Box<dyn Error>> {
    write_with_context(output_dir.join(file_name), contents)
}

fn write_with_context(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    fs::write(path, contents).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to write {}: {err}", path.display()),
        )
    })?;
    Ok(())
}

fn decode_hex_text(text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let compact = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<Vec<char>>();
    if compact.len() % 2 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hex payload length must be even",
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(compact.len() / 2);
    for (index, chunk) in compact.chunks_exact(2).enumerate() {
        let mut pair = String::with_capacity(2);
        pair.push(chunk[0]);
        pair.push(chunk[1]);
        bytes.push(u8::from_str_radix(&pair, 16).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid hex at byte {}: {err}", index),
            )
        })?);
    }
    Ok(bytes)
}

fn repo_root_from_manifest_dir() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "failed to resolve repo root from CARGO_MANIFEST_DIR={}",
                    manifest_dir.display()
                ),
            )
            .into()
        })
}

fn read_text_from_candidates(
    label: &str,
    candidates: &[PathBuf],
) -> Result<String, Box<dyn Error>> {
    for candidate in candidates {
        if candidate.is_file() {
            return fs::read_to_string(candidate).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("failed to read {label} from {}: {err}", candidate.display()),
                )
                .into()
            });
        }
    }

    let checked = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("missing {label}; checked: {checked}"),
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::{
        connect_candidates, create_dir_all_with_context, decode_hex_text, parse_cli_args_from,
        read_text_from_candidates, repo_root_from_manifest_dir, set_input_root_once,
        snapshot_candidates, with_optional_input_root, world_stream_candidates,
        write_output_with_context, write_with_context, CliArgs, USAGE,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn decode_hex_text_rejects_odd_length() {
        let err = decode_hex_text("abc").unwrap_err();
        assert_eq!(err.to_string(), "hex payload length must be even");
    }

    #[test]
    fn decode_hex_text_rejects_invalid_nibble() {
        let err = decode_hex_text("0g").unwrap_err();
        assert!(
            err.to_string().starts_with("invalid hex at byte 0:"),
            "{err}"
        );
    }

    #[test]
    fn decode_hex_text_accepts_empty_input() {
        assert_eq!(decode_hex_text("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn decode_hex_text_strips_whitespace_and_accepts_uppercase_bytes() {
        let bytes = decode_hex_text("0A 0b\n0C\t0d").unwrap();
        assert_eq!(bytes, vec![10, 11, 12, 13]);
    }

    #[test]
    fn set_input_root_once_rejects_duplicate_assignment_and_overwrites_with_latest_value() {
        let mut input_root = None;

        set_input_root_once(&mut input_root, PathBuf::from("first")).unwrap();
        assert_eq!(input_root, Some(PathBuf::from("first")));

        let err = set_input_root_once(&mut input_root, PathBuf::from("second")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "duplicate --input-root provided; latest value was second"
        );
        assert_eq!(input_root, Some(PathBuf::from("second")));
    }

    #[test]
    fn parse_cli_args_reports_missing_input_root_value() {
        let err = parse_cli_args_from(vec!["--input-root".to_string()].into_iter())
            .err()
            .unwrap();

        assert_eq!(
            err.to_string(),
            format!("missing value for --input-root\n{USAGE}")
        );
    }

    #[test]
    fn parse_cli_args_accepts_short_input_root_flag() {
        let args = parse_cli_args_from(
            vec![
                "-i".to_string(),
                "custom-input".to_string(),
                "out".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.output_dir, PathBuf::from("out"));
        assert_eq!(args.input_root, Some(PathBuf::from("custom-input")));
    }

    #[test]
    fn parse_cli_args_accepts_equals_input_root_flag() {
        let args = parse_cli_args_from(
            vec!["--input-root=custom-input".to_string(), "out".to_string()].into_iter(),
        )
        .unwrap();

        assert_eq!(args.output_dir, PathBuf::from("out"));
        assert_eq!(args.input_root, Some(PathBuf::from("custom-input")));
    }

    #[test]
    fn parse_cli_args_accepts_input_root_after_output_dir() {
        let args = parse_cli_args_from(
            vec![
                "out".to_string(),
                "--input-root".to_string(),
                "custom-input".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.output_dir, PathBuf::from("out"));
        assert_eq!(args.input_root, Some(PathBuf::from("custom-input")));
    }

    #[test]
    fn parse_cli_args_rejects_unexpected_extra_argument_after_output_dir() {
        let err = parse_cli_args_from(vec!["out".to_string(), "extra".to_string()].into_iter())
            .err()
            .unwrap();

        assert_eq!(err.to_string(), format!("unexpected argument: extra\n{USAGE}"));
    }

    #[test]
    fn world_stream_candidates_with_input_root_only_checks_explicit_path() {
        let args = CliArgs {
            output_dir: PathBuf::from("out"),
            input_root: Some(PathBuf::from("custom-input")),
        };
        let candidates =
            world_stream_candidates(&args, Path::new("tests/resources"), Path::new("."));
        assert_eq!(
            candidates,
            vec![PathBuf::from("custom-input/world-stream.hex")]
        );

        let err = read_text_from_candidates("world-stream.hex", &candidates).unwrap_err();
        let checked = PathBuf::from("custom-input")
            .join("world-stream.hex")
            .display()
            .to_string();
        assert_eq!(
            err.to_string(),
            format!("missing world-stream.hex; checked: {checked}")
        );
    }

    #[test]
    fn world_stream_candidates_without_input_root_keeps_repo_fixtures() {
        let args = CliArgs {
            output_dir: PathBuf::from("out"),
            input_root: None,
        };
        let repo_root = Path::new("/repo");
        let candidates = world_stream_candidates(&args, Path::new("tests/resources"), repo_root);
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("tests/resources/world-stream.hex"),
                PathBuf::from(
                    "/repo/rust/fixtures/world-streams/archipelago-6567-world-stream.hex"
                ),
                PathBuf::from("/repo/fixtures/world-streams/archipelago-6567-world-stream.hex"),
            ]
        );
    }

    #[test]
    fn connect_and_snapshot_candidates_prepend_input_root_and_preserve_fixture_fallbacks() {
        let args = CliArgs {
            output_dir: PathBuf::from("out"),
            input_root: Some(PathBuf::from("custom-input")),
        };
        let tests_resources = Path::new("tests/resources");

        assert_eq!(
            with_optional_input_root(
                "fixture.hex",
                args.input_root.as_deref(),
                vec![tests_resources.join("fixture.hex")]
            ),
            vec![
                PathBuf::from("custom-input/fixture.hex"),
                PathBuf::from("tests/resources/fixture.hex"),
            ]
        );
        assert_eq!(
            connect_candidates(&args, tests_resources),
            vec![
                PathBuf::from("custom-input/connect-packet.hex"),
                PathBuf::from("tests/resources/connect-packet.hex"),
            ]
        );
        assert_eq!(
            snapshot_candidates(&args, tests_resources),
            vec![
                PathBuf::from("custom-input/snapshot-goldens.txt"),
                PathBuf::from("tests/resources/snapshot-goldens.txt"),
            ]
        );

        let no_root_args = CliArgs {
            output_dir: PathBuf::from("out"),
            input_root: None,
        };
        assert_eq!(
            connect_candidates(&no_root_args, tests_resources),
            vec![PathBuf::from("tests/resources/connect-packet.hex")]
        );
        assert_eq!(
            snapshot_candidates(&no_root_args, tests_resources),
            vec![PathBuf::from("tests/resources/snapshot-goldens.txt")]
        );
    }

    #[test]
    fn read_text_from_candidates_prefers_first_existing_candidate_and_reports_checked_paths() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mdt-world-read-text-from-candidates-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        let missing = temp_dir.join("missing.txt");
        let existing = temp_dir.join("existing.txt");
        fs::write(&existing, "hello\nworld").unwrap();

        let contents =
            read_text_from_candidates("sample.txt", &[missing.clone(), existing.clone()]).unwrap();
        assert_eq!(contents, "hello\nworld");

        let err = read_text_from_candidates("sample.txt", &[missing.clone()]).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("missing sample.txt; checked: {}", missing.display())
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn read_text_from_candidates_reports_not_found_with_no_candidates() {
        let err = read_text_from_candidates("sample.txt", &[]).unwrap_err();

        assert_eq!(err.to_string(), "missing sample.txt; checked: ");
    }

    #[test]
    fn repo_root_from_manifest_dir_returns_workspace_root() {
        let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();

        assert_eq!(repo_root_from_manifest_dir().unwrap(), expected);
    }

    #[test]
    fn create_dir_all_and_write_with_context_persist_output_bytes() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mdt-world-write-with-context-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let nested_dir = temp_dir.join("nested").join("out");
        let output_path = nested_dir.join("snapshot.txt");

        create_dir_all_with_context(&nested_dir).unwrap();
        write_with_context(&output_path, b"world-snapshot").unwrap();

        assert_eq!(fs::read(&output_path).unwrap(), b"world-snapshot");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_output_with_context_joins_output_dir_and_file_name() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mdt-world-write-output-with-context-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let nested_dir = temp_dir.join("nested").join("out");

        create_dir_all_with_context(&nested_dir).unwrap();
        write_output_with_context(&nested_dir, "snapshot.txt", b"world-snapshot").unwrap();

        assert_eq!(
            fs::read(nested_dir.join("snapshot.txt")).unwrap(),
            b"world-snapshot"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn create_dir_all_with_context_reports_existing_file_as_directory_error() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mdt-world-create-dir-all-with-context-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let file_path = temp_dir.join("occupied");
        fs::write(&file_path, b"occupied").unwrap();

        let err = create_dir_all_with_context(&file_path).unwrap_err();
        assert!(
            err.to_string()
                .starts_with("failed to create output directory "),
            "{err}"
        );
        assert!(err.to_string().contains("occupied"), "{err}");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
