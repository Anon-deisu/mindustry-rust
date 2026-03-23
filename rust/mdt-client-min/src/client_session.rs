use crate::bootstrap_flow::{
    apply_connect_packet, apply_world_bootstrap, ConnectPacketEnvelope, WorldStreamAssembler,
};
use crate::net_loop::{ingest_inbound_packet, NetLoopStats};
use crate::packet_registry::InboundSnapshotPacketRegistry;
use crate::session_state::{
    BuilderQueueEntryObservation, EffectBusinessContentKind, EffectBusinessPositionSource,
    EffectBusinessProjection, EffectDataSemantic, PayloadDroppedProjection,
    PickedBuildPayloadProjection, PickedUnitPayloadProjection, SessionState, TakeItemsProjection,
    TileConfigBusinessApply, TransferItemToProjection, TransferItemToUnitProjection,
    UnitEnteredPayloadProjection, UnitRefProjection,
};
use mdt_protocol::{
    decode_packet, encode_framework_message, encode_packet, FrameworkMessage, PacketCodecError,
    STREAM_BEGIN_PACKET_ID, STREAM_CHUNK_PACKET_ID,
};
use mdt_remote::{HighFrequencyRemoteMethod, RemoteManifest, RemoteManifestError};
use mdt_typeio::{
    read_object_prefix, write_int as write_typeio_int, write_object as write_typeio_object,
    TypeIoEffectPositionHint, TypeIoObject, TypeIoSemanticRef,
};
use mdt_world::{
    parse_building_sync_bytes, parse_entity_alpha_sync_bytes,
    parse_entity_building_tether_payload_sync_bytes,
    parse_entity_building_tether_payload_sync_bytes_with_content_header,
    parse_entity_fire_sync_bytes, parse_entity_mech_sync_bytes, parse_entity_missile_sync_bytes,
    parse_entity_payload_sync_bytes, parse_entity_payload_sync_bytes_with_content_header,
    parse_entity_player_sync_bytes, parse_entity_puddle_sync_bytes,
    parse_entity_weather_state_sync_bytes, parse_entity_world_label_sync_bytes, parse_world_bundle,
    ContentHeaderEntry, LoadedWorldBootstrap, LoadedWorldState, WorldBundle,
};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

// Java `Packets.KickReason.serverRestarting` ordinal.
pub const KICK_REASON_SERVER_RESTARTING_ORDINAL: i32 = 15;
const COMMAND_UNITS_DEFAULT_CHUNK_SIZE: usize = 200;
const KICK_REASON_NAMES: [&str; 16] = [
    "kick",
    "clientOutdated",
    "serverOutdated",
    "banned",
    "gameover",
    "recentKick",
    "nameInUse",
    "idInUse",
    "nameEmpty",
    "customClient",
    "serverClose",
    "vote",
    "typeMismatch",
    "whitelist",
    "playerLimit",
    "serverRestarting",
];
const MARKER_CONTROL_NAMES: [&str; 25] = [
    "remove",
    "world",
    "minimap",
    "autoscale",
    "pos",
    "endPos",
    "drawLayer",
    "color",
    "radius",
    "stroke",
    "outline",
    "rotation",
    "shape",
    "arc",
    "flushText",
    "fontSize",
    "textHeight",
    "textAlign",
    "lineAlign",
    "labelFlags",
    "texture",
    "textureSize",
    "posi",
    "uvi",
    "colori",
];
const BLOCK_CONTENT_TYPE: u8 = 1;
const ALPHA_SHAPE_ENTITY_CLASS_IDS: [u8; 17] = [
    0, 2, 3, 16, 18, 20, 21, 24, 29, 30, 31, 33, 40, 43, 44, 45, 46,
];
const MECH_SHAPE_ENTITY_CLASS_IDS: [u8; 4] = [4, 17, 19, 32];
const MISSILE_SHAPE_ENTITY_CLASS_IDS: [u8; 1] = [39];
const PAYLOAD_SHAPE_ENTITY_CLASS_IDS: [u8; 3] = [5, 23, 26];
const BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS: [u8; 1] = [36];
const FIRE_ENTITY_CLASS_IDS: [u8; 1] = [10];
const PUDDLE_ENTITY_CLASS_IDS: [u8; 1] = [13];
const WEATHER_STATE_ENTITY_CLASS_IDS: [u8; 1] = [14];
const WORLD_LABEL_ENTITY_CLASS_IDS: [u8; 1] = [35];

fn kick_reason_name_from_ordinal(reason_ordinal: i32) -> Option<&'static str> {
    usize::try_from(reason_ordinal)
        .ok()
        .and_then(|index| KICK_REASON_NAMES.get(index).copied())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickReasonHintCategory {
    Banned,
    ClientOutdated,
    CustomClientRejected,
    IdInUse,
    NameEmpty,
    NameInUse,
    PlayerLimit,
    RecentKick,
    ServerOutdated,
    WhitelistRequired,
    TypeMismatch,
    ServerRestarting,
}

fn kick_reason_hint_from(
    reason_text: Option<&str>,
    reason_ordinal: Option<i32>,
) -> Option<(KickReasonHintCategory, &'static str)> {
    let normalized = reason_text.or_else(|| reason_ordinal.and_then(kick_reason_name_from_ordinal));
    match normalized {
        Some("banned") => Some((
            KickReasonHintCategory::Banned,
            "server reports this identity or name is banned; use a different account or ask the server admin to review the ban.",
        )),
        Some("clientOutdated") => Some((
            KickReasonHintCategory::ClientOutdated,
            "client build is outdated; upgrade this client to the server version.",
        )),
        Some("recentKick") => Some((
            KickReasonHintCategory::RecentKick,
            "server still remembers a recent kick; wait for the cooldown to expire before reconnecting.",
        )),
        Some("nameInUse") => Some((
            KickReasonHintCategory::NameInUse,
            "player name is already in use; retry with a different --name value.",
        )),
        Some("idInUse") => Some((
            KickReasonHintCategory::IdInUse,
            "uuid or usid is already in use; wait for the old session to clear or regenerate the connect identity.",
        )),
        Some("nameEmpty") => Some((
            KickReasonHintCategory::NameEmpty,
            "player name is empty or invalid; set --name to a non-empty value accepted by the server.",
        )),
        Some("serverOutdated") => Some((
            KickReasonHintCategory::ServerOutdated,
            "server build is older than this client; use a matching server or older client build.",
        )),
        Some("customClient") => Some((
            KickReasonHintCategory::CustomClientRejected,
            "server rejected custom clients; connect to a server that allows custom clients.",
        )),
        Some("typeMismatch") => Some((
            KickReasonHintCategory::TypeMismatch,
            "version type/protocol mismatch; align client/server version type and mod set.",
        )),
        Some("whitelist") => Some((
            KickReasonHintCategory::WhitelistRequired,
            "server requires whitelist access; ask the server admin to whitelist this identity.",
        )),
        Some("playerLimit") => Some((
            KickReasonHintCategory::PlayerLimit,
            "server is full; wait for an open slot or use an identity with reserved access.",
        )),
        Some("serverRestarting") => Some((
            KickReasonHintCategory::ServerRestarting,
            "server is restarting; retry connection shortly.",
        )),
        _ => None,
    }
}

#[derive(Debug)]
pub struct ClientSession {
    locale: String,
    registry: InboundSnapshotPacketRegistry,
    known_remote_packets: BTreeMap<u8, IgnoredRemotePacketMeta>,
    known_remote_packet_priorities: BTreeMap<u8, DeferredInboundPriority>,
    client_snapshot_packet_id: u8,
    ping_packet_id: Option<u8>,
    ping_response_packet_id: Option<u8>,
    kick_string_packet_id: Option<u8>,
    kick_reason_packet_id: Option<u8>,
    connect_redirect_packet_id: Option<u8>,
    connect_confirm_packet_id: u8,
    world_data_begin_packet_id: u8,
    send_message_packet_id: Option<u8>,
    send_message_with_sender_packet_id: Option<u8>,
    send_chat_message_packet_id: Option<u8>,
    admin_request_packet_id: Option<u8>,
    request_debug_status_packet_id: Option<u8>,
    game_over_packet_id: Option<u8>,
    researched_packet_id: Option<u8>,
    sector_capture_packet_id: Option<u8>,
    set_flag_packet_id: Option<u8>,
    update_game_over_packet_id: Option<u8>,
    announce_packet_id: Option<u8>,
    copy_to_clipboard_packet_id: Option<u8>,
    follow_up_menu_packet_id: Option<u8>,
    hide_hud_text_packet_id: Option<u8>,
    hide_follow_up_menu_packet_id: Option<u8>,
    info_message_packet_id: Option<u8>,
    info_popup_packet_id: Option<u8>,
    info_popup_with_id_packet_id: Option<u8>,
    info_popup_reliable_packet_id: Option<u8>,
    info_popup_reliable_with_id_packet_id: Option<u8>,
    info_toast_packet_id: Option<u8>,
    label_packet_id: Option<u8>,
    label_reliable_packet_id: Option<u8>,
    label_with_id_packet_id: Option<u8>,
    label_reliable_with_id_packet_id: Option<u8>,
    menu_choose_packet_id: Option<u8>,
    menu_packet_id: Option<u8>,
    open_uri_packet_id: Option<u8>,
    create_marker_packet_id: Option<u8>,
    remove_marker_packet_id: Option<u8>,
    remove_world_label_packet_id: Option<u8>,
    update_marker_packet_id: Option<u8>,
    update_marker_text_packet_id: Option<u8>,
    update_marker_texture_packet_id: Option<u8>,
    set_item_packet_id: Option<u8>,
    set_items_packet_id: Option<u8>,
    set_hud_text_packet_id: Option<u8>,
    set_hud_text_reliable_packet_id: Option<u8>,
    set_liquid_packet_id: Option<u8>,
    set_liquids_packet_id: Option<u8>,
    set_tile_items_packet_id: Option<u8>,
    set_tile_liquids_packet_id: Option<u8>,
    text_input_packet_id: Option<u8>,
    text_input_allow_empty_packet_id: Option<u8>,
    text_input_result_packet_id: Option<u8>,
    set_player_team_editor_packet_id: Option<u8>,
    warning_toast_packet_id: Option<u8>,
    request_item_packet_id: Option<u8>,
    request_build_payload_packet_id: Option<u8>,
    request_drop_payload_packet_id: Option<u8>,
    request_unit_payload_packet_id: Option<u8>,
    payload_dropped_packet_id: Option<u8>,
    picked_build_payload_packet_id: Option<u8>,
    picked_unit_payload_packet_id: Option<u8>,
    unit_entered_payload_packet_id: Option<u8>,
    take_items_packet_id: Option<u8>,
    transfer_item_to_packet_id: Option<u8>,
    transfer_item_to_unit_packet_id: Option<u8>,
    unit_despawn_packet_id: Option<u8>,
    building_control_select_packet_id: Option<u8>,
    clear_items_packet_id: Option<u8>,
    clear_liquids_packet_id: Option<u8>,
    drop_item_packet_id: Option<u8>,
    rotate_block_packet_id: Option<u8>,
    transfer_inventory_packet_id: Option<u8>,
    tile_config_packet_id: Option<u8>,
    tile_tap_packet_id: Option<u8>,
    delete_plans_packet_id: Option<u8>,
    unit_clear_packet_id: Option<u8>,
    unit_control_packet_id: Option<u8>,
    unit_building_control_select_packet_id: Option<u8>,
    command_building_packet_id: Option<u8>,
    command_units_packet_id: Option<u8>,
    set_unit_command_packet_id: Option<u8>,
    set_unit_stance_packet_id: Option<u8>,
    begin_break_packet_id: Option<u8>,
    begin_place_packet_id: Option<u8>,
    remove_queue_block_packet_id: Option<u8>,
    construct_finish_packet_id: Option<u8>,
    deconstruct_finish_packet_id: Option<u8>,
    build_health_update_packet_id: Option<u8>,
    player_spawn_packet_id: Option<u8>,
    set_position_packet_id: Option<u8>,
    set_camera_position_packet_id: Option<u8>,
    sound_packet_id: Option<u8>,
    sound_at_packet_id: Option<u8>,
    effect_packet_id: Option<u8>,
    effect_with_data_packet_id: Option<u8>,
    effect_reliable_packet_id: Option<u8>,
    debug_status_client_packet_id: Option<u8>,
    debug_status_client_unreliable_packet_id: Option<u8>,
    trace_info_packet_id: Option<u8>,
    client_packet_reliable_packet_id: Option<u8>,
    client_packet_unreliable_packet_id: Option<u8>,
    client_binary_packet_reliable_packet_id: Option<u8>,
    client_binary_packet_unreliable_packet_id: Option<u8>,
    server_packet_reliable_packet_id: Option<u8>,
    server_packet_unreliable_packet_id: Option<u8>,
    server_binary_packet_reliable_packet_id: Option<u8>,
    server_binary_packet_unreliable_packet_id: Option<u8>,
    client_logic_data_reliable_packet_id: Option<u8>,
    client_logic_data_unreliable_packet_id: Option<u8>,
    set_rules_packet_id: Option<u8>,
    set_objectives_packet_id: Option<u8>,
    set_rule_packet_id: Option<u8>,
    clear_objectives_packet_id: Option<u8>,
    complete_objective_packet_id: Option<u8>,
    player_disconnect_packet_id: Option<u8>,
    pending_packets: VecDeque<PendingClientPacket>,
    deferred_inbound_packets: VecDeque<DeferredInboundPacket>,
    replayed_loading_events: VecDeque<ClientSessionEvent>,
    snapshot_input: ClientSnapshotInputState,
    timing: ClientSessionTiming,
    clock_ms: u64,
    last_inbound_at_ms: Option<u64>,
    last_ready_inbound_liveness_at_ms: Option<u64>,
    last_snapshot_at_ms: Option<u64>,
    last_keepalive_at_ms: Option<u64>,
    last_client_snapshot_at_ms: Option<u64>,
    last_remote_ping_at_ms: Option<u64>,
    last_remote_ping_rtt_ms: Option<u64>,
    kicked: bool,
    last_kick_reason_text: Option<String>,
    last_kick_reason_ordinal: Option<i32>,
    last_kick_duration_ms: Option<u64>,
    last_kick_hint_category: Option<KickReasonHintCategory>,
    last_kick_hint_text: Option<&'static str>,
    next_client_snapshot_id: i32,
    timed_out: bool,
    pending_world_stream: Option<WorldStreamAssembler>,
    loading_world_data: bool,
    loaded_world_bundle: Option<WorldBundle>,
    state: SessionState,
    stats: NetLoopStats,
    client_packet_handlers: ClientPacketHandlerRegistry,
    client_binary_packet_handlers: ClientBinaryPacketHandlerRegistry,
    client_logic_data_handlers: ClientLogicDataHandlerRegistry,
}

impl ClientSession {
    pub fn from_remote_manifest(
        manifest: &RemoteManifest,
        locale: impl Into<String>,
    ) -> Result<Self, ClientSessionError> {
        Self::from_remote_manifest_with_timing(manifest, locale, ClientSessionTiming::default())
    }

    pub fn from_remote_manifest_with_timing(
        manifest: &RemoteManifest,
        locale: impl Into<String>,
        timing: ClientSessionTiming,
    ) -> Result<Self, ClientSessionError> {
        let registry = InboundSnapshotPacketRegistry::from_remote_manifest(manifest)?;
        let client_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::ClientSnapshot.method_name())
            .ok_or(RemoteManifestError::MissingHighFrequencyPacket(
                HighFrequencyRemoteMethod::ClientSnapshot.method_name(),
            ))?
            .packet_id;
        let known_remote_packets = manifest
            .remote_packets
            .iter()
            .map(|entry| {
                (
                    entry.packet_id,
                    IgnoredRemotePacketMeta {
                        method: entry.method.clone(),
                        packet_class: entry.packet_class.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let known_remote_packet_priorities = manifest
            .remote_packets
            .iter()
            .map(|entry| {
                let priority = match entry.priority.as_str() {
                    "high" => DeferredInboundPriority::High,
                    "low" => DeferredInboundPriority::Low,
                    _ => DeferredInboundPriority::Normal,
                };
                (entry.packet_id, priority)
            })
            .collect::<BTreeMap<_, _>>();
        let ping_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "ping")
            .map(|entry| entry.packet_id);
        let ping_response_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "pingResponse")
            .map(|entry| entry.packet_id);
        let kick_string_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "kick"
                    && entry.params.len() == 1
                    && entry.params[0].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let kick_reason_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "kick"
                    && !entry.params.is_empty()
                    && entry.params[0].java_type.contains("KickReason")
            })
            .map(|entry| entry.packet_id);
        let connect_redirect_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connect" && entry.params.len() == 2)
            .map(|entry| entry.packet_id);
        let connect_confirm_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connectConfirm")
            .ok_or(ClientSessionError::MissingConnectConfirmPacket)?
            .packet_id;
        let world_data_begin_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "worldDataBegin")
            .ok_or(ClientSessionError::MissingWorldDataBeginPacket)?
            .packet_id;
        let player_spawn_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "playerSpawn")
            .map(|entry| entry.packet_id);
        let send_message_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .map(|entry| entry.packet_id);
        let send_message_with_sender_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 3)
            .map(|entry| entry.packet_id);
        let send_chat_message_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendChatMessage")
            .map(|entry| entry.packet_id);
        let admin_request_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "adminRequest"
                    && entry.params.len() == 4
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "Player"
                    && entry.params[2].java_type.contains("AdminAction")
                    && entry.params[3].java_type == "java.lang.Object"
            })
            .map(|entry| entry.packet_id);
        let request_debug_status_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "requestDebugStatus"
                    && entry.params.len() == 1
                    && entry.params[0].java_type == "Player"
            })
            .map(|entry| entry.packet_id);
        let game_over_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "gameOver")
            .map(|entry| entry.packet_id);
        let researched_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "researched")
            .map(|entry| entry.packet_id);
        let sector_capture_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sectorCapture")
            .map(|entry| entry.packet_id);
        let set_flag_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setFlag")
            .map(|entry| entry.packet_id);
        let update_game_over_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateGameOver")
            .map(|entry| entry.packet_id);
        let announce_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "announce")
            .map(|entry| entry.packet_id);
        let copy_to_clipboard_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "copyToClipboard")
            .map(|entry| entry.packet_id);
        let follow_up_menu_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "followUpMenu")
            .map(|entry| entry.packet_id);
        let hide_hud_text_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "hideHudText")
            .map(|entry| entry.packet_id);
        let hide_follow_up_menu_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "hideFollowUpMenu")
            .map(|entry| entry.packet_id);
        let info_message_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoMessage")
            .map(|entry| entry.packet_id);
        let info_popup_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopup" && entry.params.len() == 7)
            .map(|entry| entry.packet_id);
        let info_popup_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopup" && entry.params.len() == 8)
            .map(|entry| entry.packet_id);
        let info_popup_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopupReliable" && entry.params.len() == 7)
            .map(|entry| entry.packet_id);
        let info_popup_reliable_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopupReliable" && entry.params.len() == 8)
            .map(|entry| entry.packet_id);
        let info_toast_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoToast")
            .map(|entry| entry.packet_id);
        let label_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "label" && entry.params.len() == 4)
            .map(|entry| entry.packet_id);
        let label_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "labelReliable" && entry.params.len() == 4)
            .map(|entry| entry.packet_id);
        let label_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "label" && entry.params.len() == 5)
            .map(|entry| entry.packet_id);
        let label_reliable_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "labelReliable" && entry.params.len() == 5)
            .map(|entry| entry.packet_id);
        let menu_choose_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "menuChoose")
            .map(|entry| entry.packet_id);
        let menu_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "menu")
            .map(|entry| entry.packet_id);
        let open_uri_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "openURI")
            .map(|entry| entry.packet_id);
        let create_marker_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "createMarker")
            .map(|entry| entry.packet_id);
        let remove_marker_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeMarker")
            .map(|entry| entry.packet_id);
        let remove_world_label_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeWorldLabel")
            .map(|entry| entry.packet_id);
        let update_marker_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateMarker")
            .map(|entry| entry.packet_id);
        let update_marker_text_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateMarkerText")
            .map(|entry| entry.packet_id);
        let update_marker_texture_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateMarkerTexture")
            .map(|entry| entry.packet_id);
        let set_item_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setItem")
            .map(|entry| entry.packet_id);
        let set_items_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setItems")
            .map(|entry| entry.packet_id);
        let set_hud_text_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setHudText")
            .map(|entry| entry.packet_id);
        let set_hud_text_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setHudTextReliable")
            .map(|entry| entry.packet_id);
        let set_liquid_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setLiquid")
            .map(|entry| entry.packet_id);
        let set_liquids_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setLiquids")
            .map(|entry| entry.packet_id);
        let set_tile_items_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setTileItems")
            .map(|entry| entry.packet_id);
        let set_tile_liquids_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setTileLiquids")
            .map(|entry| entry.packet_id);
        let text_input_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInput" && entry.params.len() == 6)
            .map(|entry| entry.packet_id);
        let text_input_allow_empty_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInput" && entry.params.len() == 7)
            .map(|entry| entry.packet_id);
        let text_input_result_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInputResult")
            .map(|entry| entry.packet_id);
        let set_player_team_editor_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setPlayerTeamEditor")
            .map(|entry| entry.packet_id);
        let warning_toast_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "warningToast")
            .map(|entry| entry.packet_id);
        let request_item_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestItem")
            .map(|entry| entry.packet_id);
        let request_build_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestBuildPayload")
            .map(|entry| entry.packet_id);
        let request_drop_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestDropPayload")
            .map(|entry| entry.packet_id);
        let request_unit_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestUnitPayload")
            .map(|entry| entry.packet_id);
        let payload_dropped_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "payloadDropped")
            .map(|entry| entry.packet_id);
        let picked_build_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "pickedBuildPayload")
            .map(|entry| entry.packet_id);
        let picked_unit_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "pickedUnitPayload")
            .map(|entry| entry.packet_id);
        let unit_entered_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitEnteredPayload")
            .map(|entry| entry.packet_id);
        let take_items_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "takeItems")
            .map(|entry| entry.packet_id);
        let transfer_item_to_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferItemTo")
            .map(|entry| entry.packet_id);
        let transfer_item_to_unit_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferItemToUnit")
            .map(|entry| entry.packet_id);
        let unit_despawn_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitDespawn")
            .map(|entry| entry.packet_id);
        let building_control_select_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "buildingControlSelect")
            .map(|entry| entry.packet_id);
        let clear_items_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clearItems")
            .map(|entry| entry.packet_id);
        let clear_liquids_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clearLiquids")
            .map(|entry| entry.packet_id);
        let drop_item_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "dropItem")
            .map(|entry| entry.packet_id);
        let rotate_block_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "rotateBlock")
            .map(|entry| entry.packet_id);
        let transfer_inventory_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferInventory")
            .map(|entry| entry.packet_id);
        let tile_config_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .map(|entry| entry.packet_id);
        let tile_tap_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileTap")
            .map(|entry| entry.packet_id);
        let delete_plans_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "deletePlans")
            .map(|entry| entry.packet_id);
        let unit_clear_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitClear")
            .map(|entry| entry.packet_id);
        let unit_control_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitControl")
            .map(|entry| entry.packet_id);
        let unit_building_control_select_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitBuildingControlSelect")
            .map(|entry| entry.packet_id);
        let command_building_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandBuilding")
            .map(|entry| entry.packet_id);
        let command_units_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandUnits")
            .map(|entry| entry.packet_id);
        let set_unit_command_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setUnitCommand")
            .map(|entry| entry.packet_id);
        let set_unit_stance_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setUnitStance")
            .map(|entry| entry.packet_id);
        let begin_break_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "beginBreak")
            .map(|entry| entry.packet_id);
        let begin_place_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "beginPlace")
            .map(|entry| entry.packet_id);
        let remove_queue_block_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeQueueBlock")
            .map(|entry| entry.packet_id);
        let construct_finish_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "constructFinish")
            .map(|entry| entry.packet_id);
        let deconstruct_finish_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "deconstructFinish")
            .map(|entry| entry.packet_id);
        let build_health_update_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "buildHealthUpdate")
            .map(|entry| entry.packet_id);
        let set_position_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setPosition")
            .map(|entry| entry.packet_id);
        let set_camera_position_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setCameraPosition")
            .map(|entry| entry.packet_id);
        let sound_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sound" && entry.params.len() == 4)
            .map(|entry| entry.packet_id);
        let sound_at_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "soundAt" && entry.params.len() == 5)
            .map(|entry| entry.packet_id);
        let effect_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 5)
            .map(|entry| entry.packet_id);
        let effect_with_data_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .map(|entry| entry.packet_id);
        let effect_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effectReliable" && entry.params.len() == 5)
            .map(|entry| entry.packet_id);
        let debug_status_client_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "debugStatusClient"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "int"
                    && entry.params[1].java_type == "int"
                    && entry.params[2].java_type == "int"
            })
            .map(|entry| entry.packet_id);
        let debug_status_client_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "debugStatusClientUnreliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "int"
                    && entry.params[1].java_type == "int"
                    && entry.params[2].java_type == "int"
            })
            .map(|entry| entry.packet_id);
        let trace_info_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "traceInfo")
            .map(|entry| entry.packet_id);
        let client_packet_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientPacketReliable"
                    && entry.params.len() == 2
                    && entry.params[0].java_type == "java.lang.String"
                    && entry.params[1].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let client_packet_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientPacketUnreliable"
                    && entry.params.len() == 2
                    && entry.params[0].java_type == "java.lang.String"
                    && entry.params[1].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let client_binary_packet_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientBinaryPacketReliable"
                    && entry.params.len() == 2
                    && entry.params[0].java_type == "java.lang.String"
                    && entry.params[1].java_type == "byte[]"
            })
            .map(|entry| entry.packet_id);
        let client_binary_packet_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientBinaryPacketUnreliable"
                    && entry.params.len() == 2
                    && entry.params[0].java_type == "java.lang.String"
                    && entry.params[1].java_type == "byte[]"
            })
            .map(|entry| entry.packet_id);
        let server_packet_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "serverPacketReliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let server_packet_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "serverPacketUnreliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let server_binary_packet_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "serverBinaryPacketReliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "byte[]"
            })
            .map(|entry| entry.packet_id);
        let server_binary_packet_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "serverBinaryPacketUnreliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "byte[]"
            })
            .map(|entry| entry.packet_id);
        let client_logic_data_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientLogicDataReliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "java.lang.Object"
            })
            .map(|entry| entry.packet_id);
        let client_logic_data_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "clientLogicDataUnreliable"
                    && entry.params.len() == 3
                    && entry.params[0].java_type == "Player"
                    && entry.params[1].java_type == "java.lang.String"
                    && entry.params[2].java_type == "java.lang.Object"
            })
            .map(|entry| entry.packet_id);
        let set_rules_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRules")
            .map(|entry| entry.packet_id);
        let set_objectives_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setObjectives")
            .map(|entry| entry.packet_id);
        let set_rule_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "setRule"
                    && entry.params.len() == 2
                    && entry.params[0].java_type == "java.lang.String"
                    && entry.params[1].java_type == "java.lang.String"
            })
            .map(|entry| entry.packet_id);
        let clear_objectives_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clearObjectives")
            .map(|entry| entry.packet_id);
        let complete_objective_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "completeObjective"
                    && entry.params.len() == 1
                    && entry.params[0].java_type == "int"
            })
            .map(|entry| entry.packet_id);
        let player_disconnect_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "playerDisconnect")
            .map(|entry| entry.packet_id);
        Ok(Self {
            locale: locale.into(),
            registry,
            known_remote_packets,
            known_remote_packet_priorities,
            client_snapshot_packet_id,
            ping_packet_id,
            ping_response_packet_id,
            kick_string_packet_id,
            kick_reason_packet_id,
            connect_redirect_packet_id,
            connect_confirm_packet_id,
            world_data_begin_packet_id,
            send_message_packet_id,
            send_message_with_sender_packet_id,
            send_chat_message_packet_id,
            admin_request_packet_id,
            request_debug_status_packet_id,
            game_over_packet_id,
            researched_packet_id,
            sector_capture_packet_id,
            set_flag_packet_id,
            update_game_over_packet_id,
            announce_packet_id,
            copy_to_clipboard_packet_id,
            follow_up_menu_packet_id,
            hide_hud_text_packet_id,
            hide_follow_up_menu_packet_id,
            info_message_packet_id,
            info_popup_packet_id,
            info_popup_with_id_packet_id,
            info_popup_reliable_packet_id,
            info_popup_reliable_with_id_packet_id,
            info_toast_packet_id,
            label_packet_id,
            label_reliable_packet_id,
            label_with_id_packet_id,
            label_reliable_with_id_packet_id,
            menu_choose_packet_id,
            menu_packet_id,
            open_uri_packet_id,
            create_marker_packet_id,
            remove_marker_packet_id,
            remove_world_label_packet_id,
            update_marker_packet_id,
            update_marker_text_packet_id,
            update_marker_texture_packet_id,
            set_item_packet_id,
            set_items_packet_id,
            set_hud_text_packet_id,
            set_hud_text_reliable_packet_id,
            set_liquid_packet_id,
            set_liquids_packet_id,
            set_tile_items_packet_id,
            set_tile_liquids_packet_id,
            text_input_packet_id,
            text_input_allow_empty_packet_id,
            text_input_result_packet_id,
            set_player_team_editor_packet_id,
            warning_toast_packet_id,
            request_item_packet_id,
            request_build_payload_packet_id,
            request_drop_payload_packet_id,
            request_unit_payload_packet_id,
            payload_dropped_packet_id,
            picked_build_payload_packet_id,
            picked_unit_payload_packet_id,
            unit_entered_payload_packet_id,
            take_items_packet_id,
            transfer_item_to_packet_id,
            transfer_item_to_unit_packet_id,
            unit_despawn_packet_id,
            building_control_select_packet_id,
            clear_items_packet_id,
            clear_liquids_packet_id,
            drop_item_packet_id,
            rotate_block_packet_id,
            transfer_inventory_packet_id,
            tile_config_packet_id,
            tile_tap_packet_id,
            delete_plans_packet_id,
            unit_clear_packet_id,
            unit_control_packet_id,
            unit_building_control_select_packet_id,
            command_building_packet_id,
            command_units_packet_id,
            set_unit_command_packet_id,
            set_unit_stance_packet_id,
            begin_break_packet_id,
            begin_place_packet_id,
            remove_queue_block_packet_id,
            construct_finish_packet_id,
            deconstruct_finish_packet_id,
            build_health_update_packet_id,
            player_spawn_packet_id,
            set_position_packet_id,
            set_camera_position_packet_id,
            sound_packet_id,
            sound_at_packet_id,
            effect_packet_id,
            effect_with_data_packet_id,
            effect_reliable_packet_id,
            debug_status_client_packet_id,
            debug_status_client_unreliable_packet_id,
            trace_info_packet_id,
            client_packet_reliable_packet_id,
            client_packet_unreliable_packet_id,
            client_binary_packet_reliable_packet_id,
            client_binary_packet_unreliable_packet_id,
            server_packet_reliable_packet_id,
            server_packet_unreliable_packet_id,
            server_binary_packet_reliable_packet_id,
            server_binary_packet_unreliable_packet_id,
            client_logic_data_reliable_packet_id,
            client_logic_data_unreliable_packet_id,
            set_rules_packet_id,
            set_objectives_packet_id,
            set_rule_packet_id,
            clear_objectives_packet_id,
            complete_objective_packet_id,
            player_disconnect_packet_id,
            pending_packets: VecDeque::new(),
            deferred_inbound_packets: VecDeque::new(),
            replayed_loading_events: VecDeque::new(),
            snapshot_input: ClientSnapshotInputState::default(),
            timing,
            clock_ms: 0,
            last_inbound_at_ms: None,
            last_ready_inbound_liveness_at_ms: None,
            last_snapshot_at_ms: None,
            last_keepalive_at_ms: None,
            last_client_snapshot_at_ms: None,
            last_remote_ping_at_ms: None,
            last_remote_ping_rtt_ms: None,
            kicked: false,
            last_kick_reason_text: None,
            last_kick_reason_ordinal: None,
            last_kick_duration_ms: None,
            last_kick_hint_category: None,
            last_kick_hint_text: None,
            next_client_snapshot_id: 1,
            timed_out: false,
            pending_world_stream: None,
            loading_world_data: false,
            loaded_world_bundle: None,
            state: SessionState::default(),
            stats: NetLoopStats::default(),
            client_packet_handlers: ClientPacketHandlerRegistry::default(),
            client_binary_packet_handlers: ClientBinaryPacketHandlerRegistry::default(),
            client_logic_data_handlers: ClientLogicDataHandlerRegistry::default(),
        })
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn stats(&self) -> &NetLoopStats {
        &self.stats
    }

    pub fn snapshot_input(&self) -> &ClientSnapshotInputState {
        &self.snapshot_input
    }

    pub fn last_remote_ping_rtt_ms(&self) -> Option<u64> {
        self.last_remote_ping_rtt_ms
    }

    pub fn kicked(&self) -> bool {
        self.kicked
    }

    pub fn last_kick_reason_text(&self) -> Option<&str> {
        self.last_kick_reason_text.as_deref()
    }

    pub fn last_kick_reason_ordinal(&self) -> Option<i32> {
        self.last_kick_reason_ordinal
    }

    pub fn last_kick_duration_ms(&self) -> Option<u64> {
        self.last_kick_duration_ms
    }

    pub fn last_kick_hint_category(&self) -> Option<KickReasonHintCategory> {
        self.last_kick_hint_category
    }

    pub fn last_kick_hint_text(&self) -> Option<&'static str> {
        self.last_kick_hint_text
    }

    pub fn set_clock_ms(&mut self, now_ms: u64) {
        self.clock_ms = now_ms;
    }

    pub fn snapshot_input_mut(&mut self) -> &mut ClientSnapshotInputState {
        &mut self.snapshot_input
    }

    pub fn loaded_world_bundle(&self) -> Option<&WorldBundle> {
        self.loaded_world_bundle.as_ref()
    }

    pub fn loaded_world_state(&self) -> Option<LoadedWorldState<'_>> {
        self.loaded_world_bundle
            .as_ref()
            .map(WorldBundle::loaded_state)
    }

    pub fn take_replayed_loading_events(&mut self) -> Vec<ClientSessionEvent> {
        self.replayed_loading_events.drain(..).collect()
    }

    pub fn add_client_packet_handler<F>(&mut self, packet_type: impl Into<String>, handler: F)
    where
        F: FnMut(&str) + 'static,
    {
        self.client_packet_handlers.add(packet_type, handler);
    }

    pub fn add_client_binary_packet_handler<F>(
        &mut self,
        packet_type: impl Into<String>,
        handler: F,
    ) where
        F: FnMut(&[u8]) + 'static,
    {
        self.client_binary_packet_handlers.add(packet_type, handler);
    }

    pub fn add_client_logic_data_handler<F>(&mut self, channel: impl Into<String>, handler: F)
    where
        F: FnMut(ClientLogicDataTransport, &TypeIoObject) + 'static,
    {
        self.client_logic_data_handlers.add(channel, handler);
    }

    pub fn queue_client_packet(
        &mut self,
        packet_type: impl AsRef<str>,
        contents: impl AsRef<str>,
        transport: ClientPacketTransport,
    ) -> Result<(), ClientSessionError> {
        let packet_id = match transport {
            ClientPacketTransport::Tcp => self.client_packet_reliable_packet_id.ok_or(
                ClientSessionError::MissingRemotePacket("clientPacketReliable"),
            )?,
            ClientPacketTransport::Udp => self.client_packet_unreliable_packet_id.ok_or(
                ClientSessionError::MissingRemotePacket("clientPacketUnreliable"),
            )?,
        };
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload(packet_type.as_ref()));
        payload.extend_from_slice(&encode_typeio_string_payload(contents.as_ref()));
        self.queue_outbound_packet(packet_id, transport, payload)
    }

    pub fn queue_client_binary_packet(
        &mut self,
        packet_type: impl AsRef<str>,
        contents: &[u8],
        transport: ClientPacketTransport,
    ) -> Result<(), ClientSessionError> {
        let packet_id = match transport {
            ClientPacketTransport::Tcp => self.client_binary_packet_reliable_packet_id.ok_or(
                ClientSessionError::MissingRemotePacket("clientBinaryPacketReliable"),
            )?,
            ClientPacketTransport::Udp => self.client_binary_packet_unreliable_packet_id.ok_or(
                ClientSessionError::MissingRemotePacket("clientBinaryPacketUnreliable"),
            )?,
        };
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload(packet_type.as_ref()));
        payload.extend_from_slice(&encode_typeio_bytes_payload(contents));
        self.queue_outbound_packet(packet_id, transport, payload)
    }

    pub fn queue_client_logic_data(
        &mut self,
        channel: impl AsRef<str>,
        value: &TypeIoObject,
        transport: ClientLogicDataTransport,
    ) -> Result<(), ClientSessionError> {
        let (packet_id, packet_transport) = match transport {
            ClientLogicDataTransport::Reliable => (
                self.client_logic_data_reliable_packet_id.ok_or(
                    ClientSessionError::MissingRemotePacket("clientLogicDataReliable"),
                )?,
                ClientPacketTransport::Tcp,
            ),
            ClientLogicDataTransport::Unreliable => (
                self.client_logic_data_unreliable_packet_id.ok_or(
                    ClientSessionError::MissingRemotePacket("clientLogicDataUnreliable"),
                )?,
                ClientPacketTransport::Udp,
            ),
        };
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload(channel.as_ref()));
        write_typeio_object(&mut payload, value);
        self.queue_outbound_packet(packet_id, packet_transport, payload)
    }

    pub fn queue_send_chat_message(
        &mut self,
        message: impl Into<String>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .send_chat_message_packet_id
            .ok_or(ClientSessionError::MissingSendChatMessagePacket)?;
        let message = message.into();
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_typeio_string_payload(&message),
        )
    }

    pub fn queue_admin_request(
        &mut self,
        other_player_id: i32,
        action_ordinal: u8,
        params: &TypeIoObject,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .admin_request_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("adminRequest"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_admin_request_payload(other_player_id, action_ordinal, params),
        )
    }

    pub fn queue_request_debug_status(&mut self) -> Result<(), ClientSessionError> {
        let packet_id =
            self.request_debug_status_packet_id
                .ok_or(ClientSessionError::MissingRemotePacket(
                    "requestDebugStatus",
                ))?;
        self.queue_outbound_packet(packet_id, ClientPacketTransport::Tcp, Vec::new())
    }

    pub fn queue_menu_choose(
        &mut self,
        menu_id: i32,
        option: i32,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .menu_choose_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("menuChoose"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_menu_choose_payload(menu_id, option),
        )
    }

    pub fn queue_text_input_result(
        &mut self,
        text_input_id: i32,
        text: Option<&str>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .text_input_result_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("textInputResult"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_text_input_result_payload(text_input_id, text),
        )
    }

    pub fn queue_request_build_payload(
        &mut self,
        build_pos: Option<i32>,
    ) -> Result<(), ClientSessionError> {
        let packet_id =
            self.request_build_payload_packet_id
                .ok_or(ClientSessionError::MissingRemotePacket(
                    "requestBuildPayload",
                ))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_building_payload(build_pos),
        )
    }

    pub fn queue_request_item(
        &mut self,
        build_pos: Option<i32>,
        item_id: Option<i16>,
        amount: i32,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .request_item_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("requestItem"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_request_item_payload(build_pos, item_id, amount),
        )
    }

    pub fn queue_request_drop_payload(&mut self, x: f32, y: f32) -> Result<(), ClientSessionError> {
        let packet_id =
            self.request_drop_payload_packet_id
                .ok_or(ClientSessionError::MissingRemotePacket(
                    "requestDropPayload",
                ))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_two_f32_payload(x, y),
        )
    }

    pub fn queue_request_unit_payload(
        &mut self,
        target: ClientUnitRef,
    ) -> Result<(), ClientSessionError> {
        let packet_id =
            self.request_unit_payload_packet_id
                .ok_or(ClientSessionError::MissingRemotePacket(
                    "requestUnitPayload",
                ))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_unit_payload(target),
        )
    }

    pub fn queue_building_control_select(
        &mut self,
        build_pos: Option<i32>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self.building_control_select_packet_id.ok_or(
            ClientSessionError::MissingRemotePacket("buildingControlSelect"),
        )?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_building_payload(build_pos),
        )
    }

    pub fn queue_clear_items(&mut self, build_pos: Option<i32>) -> Result<(), ClientSessionError> {
        let packet_id = self
            .clear_items_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("clearItems"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Udp,
            encode_building_payload(build_pos),
        )
    }

    pub fn queue_clear_liquids(
        &mut self,
        build_pos: Option<i32>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .clear_liquids_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("clearLiquids"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Udp,
            encode_building_payload(build_pos),
        )
    }

    pub fn queue_drop_item(&mut self, angle: f32) -> Result<(), ClientSessionError> {
        let packet_id = self
            .drop_item_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("dropItem"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_single_f32_payload(angle),
        )
    }

    pub fn queue_rotate_block(
        &mut self,
        build_pos: Option<i32>,
        direction: bool,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .rotate_block_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("rotateBlock"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Udp,
            encode_building_bool_payload(build_pos, direction),
        )
    }

    pub fn queue_transfer_inventory(
        &mut self,
        build_pos: Option<i32>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .transfer_inventory_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("transferInventory"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_building_payload(build_pos),
        )
    }

    pub fn queue_tile_config(
        &mut self,
        build_pos: Option<i32>,
        value: TypeIoObject,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .tile_config_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("tileConfig"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_tile_config_payload(build_pos, &value),
        )?;
        if let Some(build_pos) = build_pos {
            self.state
                .tile_config_projection
                .record_local_intent(build_pos, value);
        }
        Ok(())
    }

    pub fn queue_tile_tap(&mut self, tile_pos: Option<i32>) -> Result<(), ClientSessionError> {
        let packet_id = self
            .tile_tap_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("tileTap"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Udp,
            encode_building_payload(tile_pos),
        )
    }

    pub fn queue_delete_plans(&mut self, positions: &[i32]) -> Result<(), ClientSessionError> {
        let packet_id = self
            .delete_plans_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("deletePlans"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Udp,
            encode_delete_plans_payload(positions),
        )
    }

    pub fn queue_unit_clear(&mut self) -> Result<(), ClientSessionError> {
        let packet_id = self
            .unit_clear_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("unitClear"))?;
        self.queue_outbound_packet(packet_id, ClientPacketTransport::Tcp, Vec::new())
    }

    pub fn queue_unit_control(&mut self, target: ClientUnitRef) -> Result<(), ClientSessionError> {
        let packet_id = self
            .unit_control_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("unitControl"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_unit_payload(target),
        )
    }

    pub fn queue_unit_building_control_select(
        &mut self,
        target: ClientUnitRef,
        build_pos: Option<i32>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self.unit_building_control_select_packet_id.ok_or(
            ClientSessionError::MissingRemotePacket("unitBuildingControlSelect"),
        )?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_unit_building_payload(target, build_pos),
        )
    }

    pub fn queue_command_building(
        &mut self,
        buildings: &[i32],
        x: f32,
        y: f32,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .command_building_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("commandBuilding"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_command_building_payload(buildings, x, y),
        )
    }

    pub fn queue_command_units(
        &mut self,
        unit_ids: &[i32],
        build_target: Option<i32>,
        unit_target: ClientUnitRef,
        pos_target: Option<(f32, f32)>,
        queue_command: bool,
        final_batch: bool,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .command_units_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("commandUnits"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_command_units_payload(
                unit_ids,
                build_target,
                unit_target,
                pos_target,
                queue_command,
                final_batch,
            ),
        )
    }

    pub fn queue_command_units_chunked(
        &mut self,
        unit_ids: &[i32],
        build_target: Option<i32>,
        unit_target: ClientUnitRef,
        pos_target: Option<(f32, f32)>,
        queue_command: bool,
    ) -> Result<usize, ClientSessionError> {
        self.queue_command_units_chunked_with_max_chunk(
            unit_ids,
            build_target,
            unit_target,
            pos_target,
            queue_command,
            COMMAND_UNITS_DEFAULT_CHUNK_SIZE,
        )
    }

    pub fn queue_command_units_chunked_with_max_chunk(
        &mut self,
        unit_ids: &[i32],
        build_target: Option<i32>,
        unit_target: ClientUnitRef,
        pos_target: Option<(f32, f32)>,
        queue_command: bool,
        max_chunk_size: usize,
    ) -> Result<usize, ClientSessionError> {
        if unit_ids.is_empty() {
            return Ok(0);
        }

        let chunk_size = max_chunk_size.max(1);
        let chunk_count = unit_ids.len().saturating_add(chunk_size - 1) / chunk_size;
        for (index, chunk) in unit_ids.chunks(chunk_size).enumerate() {
            let final_batch = index + 1 == chunk_count;
            self.queue_command_units(
                chunk,
                build_target,
                unit_target,
                pos_target,
                queue_command,
                final_batch,
            )?;
        }

        Ok(chunk_count)
    }

    pub fn queue_set_unit_command(
        &mut self,
        unit_ids: &[i32],
        command_id: Option<u8>,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .set_unit_command_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("setUnitCommand"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_set_unit_command_payload(unit_ids, command_id),
        )
    }

    pub fn queue_set_unit_stance(
        &mut self,
        unit_ids: &[i32],
        stance_id: Option<u8>,
        enable: bool,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .set_unit_stance_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("setUnitStance"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_set_unit_stance_payload(unit_ids, stance_id, enable),
        )
    }

    pub fn queue_begin_break(
        &mut self,
        builder: ClientUnitRef,
        team_id: u8,
        x: i32,
        y: i32,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .begin_break_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("beginBreak"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_begin_break_payload(builder, team_id, x, y),
        )
    }

    pub fn queue_begin_place(
        &mut self,
        builder: ClientUnitRef,
        block_id: Option<i16>,
        team_id: u8,
        x: i32,
        y: i32,
        rotation: i32,
        place_config: &TypeIoObject,
    ) -> Result<(), ClientSessionError> {
        let packet_id = self
            .begin_place_packet_id
            .ok_or(ClientSessionError::MissingRemotePacket("beginPlace"))?;
        self.queue_outbound_packet(
            packet_id,
            ClientPacketTransport::Tcp,
            encode_begin_place_payload(builder, block_id, team_id, x, y, rotation, place_config),
        )
    }

    pub fn prepare_connect_packet(
        &mut self,
        payload: &[u8],
    ) -> Result<ConnectPacketEnvelope, ClientSessionError> {
        let connect = ConnectPacketEnvelope::from_payload(payload)?;
        self.quiet_reset_for_reconnect();
        apply_connect_packet(&mut self.state, &connect);
        self.record_outbound_activity(self.clock_ms);
        Ok(connect)
    }

    pub fn prepare_connect_confirm_packet(
        &mut self,
    ) -> Result<Option<Vec<u8>>, ClientSessionError> {
        if !self.state.ready_to_enter_world || self.state.connect_confirm_sent {
            return Ok(None);
        }

        let bytes = encode_packet(self.connect_confirm_packet_id, &[], false)?;
        self.state.connect_confirm_sent = true;
        self.state.last_connect_confirm_at_ms = Some(self.clock_ms);
        self.last_snapshot_at_ms = Some(self.clock_ms);
        self.record_outbound_activity(self.clock_ms);
        Ok(Some(bytes))
    }

    pub fn advance_time(
        &mut self,
        now_ms: u64,
    ) -> Result<Vec<ClientSessionAction>, ClientSessionError> {
        self.clock_ms = now_ms;
        if self.timed_out || self.kicked {
            return Ok(Vec::new());
        }

        let timeout_anchor = if self.ready_for_interaction() {
            self.last_snapshot_at_ms
        } else {
            self.last_inbound_at_ms
        };
        let timeout_limit_ms = if self.ready_for_interaction() {
            self.timing.timeout_ms
        } else {
            self.timing.connect_timeout_ms
        };

        if let Some(last_activity) = timeout_anchor {
            let idle_ms = now_ms.saturating_sub(last_activity);
            if idle_ms >= timeout_limit_ms {
                self.timed_out = true;
                self.state.connection_timed_out = true;
                return Ok(vec![ClientSessionAction::TimedOut { idle_ms }]);
            }
        }

        let mut actions = Vec::new();

        if self.ready_for_interaction() {
            while let Some(packet) = self.pending_packets.pop_front() {
                self.record_outbound_activity(now_ms);
                actions.push(ClientSessionAction::SendPacket {
                    packet_id: packet.packet_id,
                    transport: packet.transport,
                    bytes: packet.bytes,
                });
            }
        }

        if self.should_send_keepalive(now_ms) {
            let message = FrameworkMessage::KeepAlive;
            let bytes = encode_framework_message(&message);
            self.last_keepalive_at_ms = Some(now_ms);
            self.state.last_keepalive_at_ms = Some(now_ms);
            self.state.sent_keepalive_count = self.state.sent_keepalive_count.saturating_add(1);
            self.record_outbound_activity(now_ms);
            actions.push(ClientSessionAction::SendFramework { message, bytes });
        }

        if self.ready_for_interaction() && self.should_send_remote_ping(now_ms) {
            if let Some(packet_id) = self.ping_packet_id {
                let payload = encode_ping_time_payload(now_ms);
                let bytes = encode_packet(packet_id, &payload, false)?;
                self.last_remote_ping_at_ms = Some(now_ms);
                self.record_outbound_activity(now_ms);
                actions.push(ClientSessionAction::SendPacket {
                    packet_id,
                    transport: ClientPacketTransport::Tcp,
                    bytes,
                });
            }
        }

        if self.should_send_client_snapshot(now_ms) {
            let snapshot_id = self.next_client_snapshot_id;
            self.next_client_snapshot_id = self.next_client_snapshot_id.saturating_add(1);
            let payload =
                encode_client_snapshot_payload(&self.state, &self.snapshot_input, snapshot_id);
            let bytes = encode_packet(self.client_snapshot_packet_id, &payload, false)?;
            self.last_client_snapshot_at_ms = Some(now_ms);
            self.state.last_client_snapshot_at_ms = Some(now_ms);
            self.state.sent_client_snapshot_count =
                self.state.sent_client_snapshot_count.saturating_add(1);
            self.state.last_sent_client_snapshot_id = Some(snapshot_id);
            self.record_outbound_activity(now_ms);
            actions.push(ClientSessionAction::SendPacket {
                packet_id: self.client_snapshot_packet_id,
                transport: ClientPacketTransport::Udp,
                bytes,
            });
        }

        Ok(actions)
    }

    pub fn ingest_packet_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<ClientSessionEvent, ClientSessionError> {
        let packet = decode_packet(bytes)?;
        if self.timed_out || self.kicked {
            return Ok(ClientSessionEvent::IgnoredPacket {
                packet_id: packet.packet_id,
                remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
            });
        }
        self.record_inbound_activity(self.clock_ms);
        self.record_ready_inbound_liveness_activity(self.clock_ms);

        if let Some(event) =
            self.maybe_defer_inbound_packet_while_loading(packet.packet_id, &packet.payload)
        {
            return Ok(event);
        }

        self.process_inbound_packet(bytes, packet.packet_id, &packet.payload)
    }

    fn process_inbound_packet(
        &mut self,
        raw_bytes: &[u8],
        packet_id: u8,
        payload: &[u8],
    ) -> Result<ClientSessionEvent, ClientSessionError> {
        let packet = InboundPacketRef {
            raw_bytes,
            packet_id,
            payload,
        };

        match packet.packet_id {
            STREAM_BEGIN_PACKET_ID => {
                let assembler = WorldStreamAssembler::from_stream_begin_packet(packet.raw_bytes)?;
                self.loading_world_data = true;
                self.state.bootstrap_stream_id = Some(assembler.stream_id);
                self.state.world_stream_expected_len = assembler.total_bytes;
                self.state.world_stream_received_len = 0;
                let event = ClientSessionEvent::WorldStreamStarted {
                    stream_id: assembler.stream_id,
                    total_bytes: assembler.total_bytes,
                };
                self.pending_world_stream = Some(assembler);
                Ok(event)
            }
            STREAM_CHUNK_PACKET_ID => {
                let assembler = self
                    .pending_world_stream
                    .as_mut()
                    .ok_or(ClientSessionError::MissingWorldStreamBegin)?;
                let complete = assembler.push_stream_chunk_packet(packet.raw_bytes)?;
                self.state.world_stream_received_len = assembler.compressed_world_stream().len();

                if complete {
                    let assembler = self
                        .pending_world_stream
                        .take()
                        .expect("checked Some above");
                    let stream_id = assembler.stream_id;
                    let compressed = assembler.finish()?;
                    let world_bundle = parse_world_bundle(&compressed)
                        .map_err(ClientSessionError::WorldBundleParse)?;
                    let bootstrap = world_bundle
                        .loaded_session()
                        .map_err(ClientSessionError::WorldBundleParse)?
                        .bootstrap(&self.locale);
                    apply_world_bootstrap(&mut self.state, stream_id, &bootstrap);
                    self.apply_world_baseline_from_bundle(&world_bundle);
                    self.apply_snapshot_input_from_bootstrap(&bootstrap);
                    self.loaded_world_bundle = Some(world_bundle);
                    self.mark_client_loaded()?;

                    Ok(ClientSessionEvent::WorldStreamReady {
                        stream_id,
                        map_width: bootstrap.map_width,
                        map_height: bootstrap.map_height,
                        player_id: bootstrap.player_id,
                        ready_to_enter_world: bootstrap.ready_to_enter_world,
                    })
                } else {
                    Ok(ClientSessionEvent::WorldStreamChunk {
                        stream_id: assembler.stream_id,
                        received_bytes: assembler.compressed_world_stream().len(),
                        total_bytes: assembler.total_bytes,
                    })
                }
            }
            packet_id if packet_id == self.world_data_begin_packet_id => {
                self.begin_world_data_reload();
                Ok(ClientSessionEvent::WorldDataBegin)
            }
            packet_id if Some(packet_id) == self.connect_redirect_packet_id => {
                if let Some((ip, port)) = decode_connect_redirect_payload(&packet.payload) {
                    self.state.received_connect_redirect_count =
                        self.state.received_connect_redirect_count.saturating_add(1);
                    self.state.last_connect_redirect_ip = Some(ip.clone());
                    self.state.last_connect_redirect_port = Some(port);
                    Ok(ClientSessionEvent::ConnectRedirectRequested { ip, port })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.player_spawn_packet_id => {
                if let Some((player_id, x, y)) =
                    self.try_apply_local_player_spawn_from_packet(&packet.payload)
                {
                    Ok(ClientSessionEvent::PlayerSpawned { player_id, x, y })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_flag_packet_id => {
                if let Some((flag, add)) = decode_set_flag_payload(&packet.payload) {
                    self.state.received_set_flag_count =
                        self.state.received_set_flag_count.saturating_add(1);
                    self.state.last_set_flag = flag.clone();
                    self.state.last_set_flag_add = Some(add);
                    Ok(ClientSessionEvent::SetFlag { flag, add })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.game_over_packet_id => {
                if let Some(winner_team_id) = decode_team_payload(&packet.payload) {
                    self.state.received_game_over_count =
                        self.state.received_game_over_count.saturating_add(1);
                    self.state.last_game_over_winner_team_id = Some(winner_team_id);
                    Ok(ClientSessionEvent::GameOver { winner_team_id })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.update_game_over_packet_id => {
                if let Some(winner_team_id) = decode_team_payload(&packet.payload) {
                    self.state.received_update_game_over_count =
                        self.state.received_update_game_over_count.saturating_add(1);
                    self.state.last_update_game_over_winner_team_id = Some(winner_team_id);
                    Ok(ClientSessionEvent::UpdateGameOver { winner_team_id })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.sector_capture_packet_id => {
                if packet.payload.is_empty() {
                    self.state.received_sector_capture_count =
                        self.state.received_sector_capture_count.saturating_add(1);
                    Ok(ClientSessionEvent::SectorCapture)
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.researched_packet_id => {
                if let Some((content_type, content_id)) = decode_content_payload(&packet.payload) {
                    self.state.received_researched_count =
                        self.state.received_researched_count.saturating_add(1);
                    self.state.last_researched_content_type = Some(content_type);
                    self.state.last_researched_content_id = Some(content_id);
                    Ok(ClientSessionEvent::Researched {
                        content_type,
                        content_id,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_hud_text_packet_id => {
                if let Some(message) = decode_optional_typeio_string_payload(&packet.payload) {
                    self.state.received_set_hud_text_count =
                        self.state.received_set_hud_text_count.saturating_add(1);
                    self.state.last_set_hud_text_message = message.clone();
                    Ok(ClientSessionEvent::SetHudText { message })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_hud_text_reliable_packet_id => {
                if let Some(message) = decode_optional_typeio_string_payload(&packet.payload) {
                    self.state.received_set_hud_text_reliable_count = self
                        .state
                        .received_set_hud_text_reliable_count
                        .saturating_add(1);
                    self.state.last_set_hud_text_reliable_message = message.clone();
                    Ok(ClientSessionEvent::SetHudTextReliable { message })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.hide_hud_text_packet_id => {
                if packet.payload.is_empty() {
                    self.state.received_hide_hud_text_count =
                        self.state.received_hide_hud_text_count.saturating_add(1);
                    Ok(ClientSessionEvent::HideHudText)
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.announce_packet_id => {
                if let Some(message) = decode_optional_typeio_string_payload(&packet.payload) {
                    self.state.received_announce_count =
                        self.state.received_announce_count.saturating_add(1);
                    self.state.last_announce_message = message.clone();
                    Ok(ClientSessionEvent::Announce { message })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.label_packet_id => {
                if let Some(summary) = decode_world_label_payload(&packet.payload, false) {
                    self.state.received_world_label_count =
                        self.state.received_world_label_count.saturating_add(1);
                    self.state.last_world_label_reliable = Some(false);
                    self.state.last_world_label_id = summary.label_id;
                    self.state.last_world_label_message = summary.message.clone();
                    self.state.last_world_label_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_world_label_world_x_bits = Some(summary.world_x.to_bits());
                    self.state.last_world_label_world_y_bits = Some(summary.world_y.to_bits());
                    Ok(ClientSessionEvent::WorldLabel {
                        reliable: false,
                        label_id: summary.label_id,
                        message: summary.message,
                        duration: summary.duration,
                        world_x: summary.world_x,
                        world_y: summary.world_y,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.label_with_id_packet_id => {
                if let Some(summary) = decode_world_label_payload(&packet.payload, true) {
                    self.state.received_world_label_count =
                        self.state.received_world_label_count.saturating_add(1);
                    self.state.last_world_label_reliable = Some(false);
                    self.state.last_world_label_id = summary.label_id;
                    self.state.last_world_label_message = summary.message.clone();
                    self.state.last_world_label_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_world_label_world_x_bits = Some(summary.world_x.to_bits());
                    self.state.last_world_label_world_y_bits = Some(summary.world_y.to_bits());
                    Ok(ClientSessionEvent::WorldLabel {
                        reliable: false,
                        label_id: summary.label_id,
                        message: summary.message,
                        duration: summary.duration,
                        world_x: summary.world_x,
                        world_y: summary.world_y,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.label_reliable_packet_id => {
                if let Some(summary) = decode_world_label_payload(&packet.payload, false) {
                    self.state.received_world_label_reliable_count = self
                        .state
                        .received_world_label_reliable_count
                        .saturating_add(1);
                    self.state.last_world_label_reliable = Some(true);
                    self.state.last_world_label_id = summary.label_id;
                    self.state.last_world_label_message = summary.message.clone();
                    self.state.last_world_label_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_world_label_world_x_bits = Some(summary.world_x.to_bits());
                    self.state.last_world_label_world_y_bits = Some(summary.world_y.to_bits());
                    Ok(ClientSessionEvent::WorldLabel {
                        reliable: true,
                        label_id: summary.label_id,
                        message: summary.message,
                        duration: summary.duration,
                        world_x: summary.world_x,
                        world_y: summary.world_y,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.label_reliable_with_id_packet_id => {
                if let Some(summary) = decode_world_label_payload(&packet.payload, true) {
                    self.state.received_world_label_reliable_count = self
                        .state
                        .received_world_label_reliable_count
                        .saturating_add(1);
                    self.state.last_world_label_reliable = Some(true);
                    self.state.last_world_label_id = summary.label_id;
                    self.state.last_world_label_message = summary.message.clone();
                    self.state.last_world_label_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_world_label_world_x_bits = Some(summary.world_x.to_bits());
                    self.state.last_world_label_world_y_bits = Some(summary.world_y.to_bits());
                    Ok(ClientSessionEvent::WorldLabel {
                        reliable: true,
                        label_id: summary.label_id,
                        message: summary.message,
                        duration: summary.duration,
                        world_x: summary.world_x,
                        world_y: summary.world_y,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.remove_world_label_packet_id => {
                if let Some(label_id) = decode_remove_world_label_payload(&packet.payload) {
                    self.state.received_remove_world_label_count = self
                        .state
                        .received_remove_world_label_count
                        .saturating_add(1);
                    self.state.last_remove_world_label_id = Some(label_id);
                    Ok(ClientSessionEvent::RemoveWorldLabel { label_id })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.create_marker_packet_id => {
                if let Some(summary) = decode_create_marker_payload(&packet.payload) {
                    self.state.received_create_marker_count = self
                        .state
                        .received_create_marker_count
                        .saturating_add(1);
                    self.state.last_marker_id = Some(summary.marker_id);
                    self.state.last_marker_json_len = Some(summary.json_len);
                    self.state.last_marker_control = None;
                    self.state.last_marker_control_name = None;
                    self.state.last_marker_p1_bits = None;
                    self.state.last_marker_p2_bits = None;
                    self.state.last_marker_p3_bits = None;
                    self.state.last_marker_fetch = None;
                    self.state.last_marker_text = None;
                    self.state.last_marker_texture_kind = None;
                    self.state.last_marker_texture_kind_name = None;
                    Ok(ClientSessionEvent::CreateMarker {
                        marker_id: summary.marker_id,
                        json_len: summary.json_len,
                    })
                } else {
                    self.state.failed_marker_decode_count = self
                        .state
                        .failed_marker_decode_count
                        .saturating_add(1);
                    self.state.last_failed_marker_method = Some("createMarker".to_string());
                    self.state.last_failed_marker_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.remove_marker_packet_id => {
                if let Some(marker_id) = decode_remove_marker_payload(&packet.payload) {
                    self.state.received_remove_marker_count = self
                        .state
                        .received_remove_marker_count
                        .saturating_add(1);
                    self.state.last_marker_id = Some(marker_id);
                    self.state.last_marker_json_len = None;
                    self.state.last_marker_control = None;
                    self.state.last_marker_control_name = None;
                    self.state.last_marker_p1_bits = None;
                    self.state.last_marker_p2_bits = None;
                    self.state.last_marker_p3_bits = None;
                    self.state.last_marker_fetch = None;
                    self.state.last_marker_text = None;
                    self.state.last_marker_texture_kind = None;
                    self.state.last_marker_texture_kind_name = None;
                    Ok(ClientSessionEvent::RemoveMarker { marker_id })
                } else {
                    self.state.failed_marker_decode_count = self
                        .state
                        .failed_marker_decode_count
                        .saturating_add(1);
                    self.state.last_failed_marker_method = Some("removeMarker".to_string());
                    self.state.last_failed_marker_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.update_marker_packet_id => {
                if let Some(summary) = decode_update_marker_payload(&packet.payload) {
                    self.state.received_update_marker_count = self
                        .state
                        .received_update_marker_count
                        .saturating_add(1);
                    self.state.last_marker_id = Some(summary.marker_id);
                    self.state.last_marker_json_len = None;
                    self.state.last_marker_control = Some(summary.control);
                    self.state.last_marker_control_name =
                        marker_control_name(summary.control).map(str::to_string);
                    self.state.last_marker_p1_bits = Some(summary.p1_bits);
                    self.state.last_marker_p2_bits = Some(summary.p2_bits);
                    self.state.last_marker_p3_bits = Some(summary.p3_bits);
                    self.state.last_marker_fetch = None;
                    self.state.last_marker_text = None;
                    self.state.last_marker_texture_kind = None;
                    self.state.last_marker_texture_kind_name = None;
                    Ok(ClientSessionEvent::UpdateMarker {
                        marker_id: summary.marker_id,
                        control: summary.control,
                        control_name: marker_control_name(summary.control).map(str::to_string),
                        p1_bits: summary.p1_bits,
                        p2_bits: summary.p2_bits,
                        p3_bits: summary.p3_bits,
                    })
                } else {
                    self.state.failed_marker_decode_count = self
                        .state
                        .failed_marker_decode_count
                        .saturating_add(1);
                    self.state.last_failed_marker_method = Some("updateMarker".to_string());
                    self.state.last_failed_marker_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.update_marker_text_packet_id => {
                if let Some(summary) = decode_update_marker_text_payload(&packet.payload) {
                    self.state.received_update_marker_text_count = self
                        .state
                        .received_update_marker_text_count
                        .saturating_add(1);
                    self.state.last_marker_id = Some(summary.marker_id);
                    self.state.last_marker_json_len = None;
                    self.state.last_marker_control = Some(summary.control);
                    self.state.last_marker_control_name =
                        marker_control_name(summary.control).map(str::to_string);
                    self.state.last_marker_p1_bits = None;
                    self.state.last_marker_p2_bits = None;
                    self.state.last_marker_p3_bits = None;
                    self.state.last_marker_fetch = Some(summary.fetch);
                    self.state.last_marker_text = summary.text.clone();
                    self.state.last_marker_texture_kind = None;
                    self.state.last_marker_texture_kind_name = None;
                    Ok(ClientSessionEvent::UpdateMarkerText {
                        marker_id: summary.marker_id,
                        control: summary.control,
                        control_name: marker_control_name(summary.control).map(str::to_string),
                        fetch: summary.fetch,
                        text: summary.text,
                    })
                } else {
                    self.state.failed_marker_decode_count = self
                        .state
                        .failed_marker_decode_count
                        .saturating_add(1);
                    self.state.last_failed_marker_method = Some("updateMarkerText".to_string());
                    self.state.last_failed_marker_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.update_marker_texture_packet_id => {
                if let Some(summary) = decode_update_marker_texture_payload(&packet.payload) {
                    self.state.received_update_marker_texture_count = self
                        .state
                        .received_update_marker_texture_count
                        .saturating_add(1);
                    self.state.last_marker_id = Some(summary.marker_id);
                    self.state.last_marker_json_len = None;
                    self.state.last_marker_control = None;
                    self.state.last_marker_control_name = None;
                    self.state.last_marker_p1_bits = None;
                    self.state.last_marker_p2_bits = None;
                    self.state.last_marker_p3_bits = None;
                    self.state.last_marker_fetch = None;
                    self.state.last_marker_text = None;
                    self.state.last_marker_texture_kind = Some(summary.texture_kind);
                    self.state.last_marker_texture_kind_name =
                        Some(summary.texture_kind_name.clone());
                    Ok(ClientSessionEvent::UpdateMarkerTexture {
                        marker_id: summary.marker_id,
                        texture_kind: summary.texture_kind,
                        texture_kind_name: summary.texture_kind_name,
                    })
                } else {
                    self.state.failed_marker_decode_count = self
                        .state
                        .failed_marker_decode_count
                        .saturating_add(1);
                    self.state.last_failed_marker_method =
                        Some("updateMarkerTexture".to_string());
                    self.state.last_failed_marker_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.menu_packet_id => {
                if let Some(summary) = decode_menu_dialog_payload(&packet.payload) {
                    self.state.received_menu_open_count =
                        self.state.received_menu_open_count.saturating_add(1);
                    self.state.last_menu_open_id = Some(summary.menu_id);
                    self.state.last_menu_open_title = summary.title.clone();
                    self.state.last_menu_open_message = summary.message.clone();
                    self.state.last_menu_open_option_rows = summary.option_rows;
                    self.state.last_menu_open_first_row_len = summary.first_row_len;
                    Ok(ClientSessionEvent::MenuShown {
                        menu_id: summary.menu_id,
                        title: summary.title,
                        message: summary.message,
                        option_rows: summary.option_rows,
                        first_row_len: summary.first_row_len,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.follow_up_menu_packet_id => {
                if let Some(summary) = decode_menu_dialog_payload(&packet.payload) {
                    self.state.received_follow_up_menu_open_count = self
                        .state
                        .received_follow_up_menu_open_count
                        .saturating_add(1);
                    self.state.last_follow_up_menu_open_id = Some(summary.menu_id);
                    self.state.last_follow_up_menu_open_title = summary.title.clone();
                    self.state.last_follow_up_menu_open_message = summary.message.clone();
                    self.state.last_follow_up_menu_open_option_rows = summary.option_rows;
                    self.state.last_follow_up_menu_open_first_row_len = summary.first_row_len;
                    Ok(ClientSessionEvent::FollowUpMenuShown {
                        menu_id: summary.menu_id,
                        title: summary.title,
                        message: summary.message,
                        option_rows: summary.option_rows,
                        first_row_len: summary.first_row_len,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.hide_follow_up_menu_packet_id => {
                if let Some(menu_id) = decode_hide_follow_up_menu_payload(&packet.payload) {
                    self.state.received_hide_follow_up_menu_count = self
                        .state
                        .received_hide_follow_up_menu_count
                        .saturating_add(1);
                    self.state.last_hide_follow_up_menu_id = Some(menu_id);
                    Ok(ClientSessionEvent::HideFollowUpMenu { menu_id })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.copy_to_clipboard_packet_id => {
                if let Some(text) = decode_optional_typeio_string_payload(&packet.payload) {
                    self.state.received_copy_to_clipboard_count = self
                        .state
                        .received_copy_to_clipboard_count
                        .saturating_add(1);
                    self.state.last_copy_to_clipboard_text = text.clone();
                    Ok(ClientSessionEvent::CopyToClipboard { text })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.open_uri_packet_id => {
                if let Some(uri) = decode_optional_typeio_string_payload(&packet.payload) {
                    self.state.received_open_uri_count =
                        self.state.received_open_uri_count.saturating_add(1);
                    self.state.last_open_uri = uri.clone();
                    Ok(ClientSessionEvent::OpenUri { uri })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.text_input_packet_id => {
                if let Some(summary) = decode_text_input_payload(&packet.payload, false) {
                    self.state.received_text_input_count =
                        self.state.received_text_input_count.saturating_add(1);
                    self.state.last_text_input_id = Some(summary.text_input_id);
                    self.state.last_text_input_title = summary.title.clone();
                    self.state.last_text_input_message = summary.message.clone();
                    self.state.last_text_input_length = Some(summary.text_length);
                    self.state.last_text_input_default_text = summary.default_text.clone();
                    self.state.last_text_input_numeric = Some(summary.numeric);
                    self.state.last_text_input_allow_empty = Some(summary.allow_empty);
                    Ok(ClientSessionEvent::TextInput {
                        text_input_id: summary.text_input_id,
                        title: summary.title,
                        message: summary.message,
                        text_length: summary.text_length,
                        default_text: summary.default_text,
                        numeric: summary.numeric,
                        allow_empty: summary.allow_empty,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.text_input_allow_empty_packet_id => {
                if let Some(summary) = decode_text_input_payload(&packet.payload, true) {
                    self.state.received_text_input_count =
                        self.state.received_text_input_count.saturating_add(1);
                    self.state.last_text_input_id = Some(summary.text_input_id);
                    self.state.last_text_input_title = summary.title.clone();
                    self.state.last_text_input_message = summary.message.clone();
                    self.state.last_text_input_length = Some(summary.text_length);
                    self.state.last_text_input_default_text = summary.default_text.clone();
                    self.state.last_text_input_numeric = Some(summary.numeric);
                    self.state.last_text_input_allow_empty = Some(summary.allow_empty);
                    Ok(ClientSessionEvent::TextInput {
                        text_input_id: summary.text_input_id,
                        title: summary.title,
                        message: summary.message,
                        text_length: summary.text_length,
                        default_text: summary.default_text,
                        numeric: summary.numeric,
                        allow_empty: summary.allow_empty,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_item_packet_id => {
                if let Some(summary) = decode_set_item_payload(&packet.payload) {
                    self.state.received_set_item_count =
                        self.state.received_set_item_count.saturating_add(1);
                    self.state.last_set_item_build_pos = summary.build_pos;
                    self.state.last_set_item_item_id = summary.item_id;
                    self.state.last_set_item_amount = Some(summary.amount);
                    Ok(ClientSessionEvent::SetItem {
                        build_pos: summary.build_pos,
                        item_id: summary.item_id,
                        amount: summary.amount,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_items_packet_id => {
                if let Some(summary) = decode_set_items_payload(&packet.payload) {
                    self.state.received_set_items_count =
                        self.state.received_set_items_count.saturating_add(1);
                    self.state.last_set_items_build_pos = summary.build_pos;
                    self.state.last_set_items_count = summary.stack_count;
                    self.state.last_set_items_first_item_id = summary.first_item_id;
                    self.state.last_set_items_first_amount = summary.first_amount;
                    Ok(ClientSessionEvent::SetItems {
                        build_pos: summary.build_pos,
                        stack_count: summary.stack_count,
                        first_item_id: summary.first_item_id,
                        first_amount: summary.first_amount,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_liquid_packet_id => {
                if let Some(summary) = decode_set_liquid_payload(&packet.payload) {
                    self.state.received_set_liquid_count =
                        self.state.received_set_liquid_count.saturating_add(1);
                    self.state.last_set_liquid_build_pos = summary.build_pos;
                    self.state.last_set_liquid_liquid_id = summary.liquid_id;
                    self.state.last_set_liquid_amount_bits = Some(summary.amount.to_bits());
                    Ok(ClientSessionEvent::SetLiquid {
                        build_pos: summary.build_pos,
                        liquid_id: summary.liquid_id,
                        amount: summary.amount,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_liquids_packet_id => {
                if let Some(summary) = decode_set_liquids_payload(&packet.payload) {
                    self.state.received_set_liquids_count =
                        self.state.received_set_liquids_count.saturating_add(1);
                    self.state.last_set_liquids_build_pos = summary.build_pos;
                    self.state.last_set_liquids_count = summary.stack_count;
                    self.state.last_set_liquids_first_liquid_id = summary.first_liquid_id;
                    self.state.last_set_liquids_first_amount_bits = summary.first_amount_bits;
                    Ok(ClientSessionEvent::SetLiquids {
                        build_pos: summary.build_pos,
                        stack_count: summary.stack_count,
                        first_liquid_id: summary.first_liquid_id,
                        first_amount_bits: summary.first_amount_bits,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_tile_items_packet_id => {
                if let Some(summary) = decode_set_tile_items_payload(&packet.payload) {
                    self.state.received_set_tile_items_count =
                        self.state.received_set_tile_items_count.saturating_add(1);
                    self.state.last_set_tile_items_item_id = summary.item_id;
                    self.state.last_set_tile_items_amount = Some(summary.amount);
                    self.state.last_set_tile_items_count = summary.position_count;
                    self.state.last_set_tile_items_first_position = summary.first_position;
                    Ok(ClientSessionEvent::SetTileItems {
                        item_id: summary.item_id,
                        amount: summary.amount,
                        position_count: summary.position_count,
                        first_position: summary.first_position,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_tile_liquids_packet_id => {
                if let Some(summary) = decode_set_tile_liquids_payload(&packet.payload) {
                    self.state.received_set_tile_liquids_count =
                        self.state.received_set_tile_liquids_count.saturating_add(1);
                    self.state.last_set_tile_liquids_liquid_id = summary.liquid_id;
                    self.state.last_set_tile_liquids_amount_bits = Some(summary.amount_bits);
                    self.state.last_set_tile_liquids_count = summary.position_count;
                    self.state.last_set_tile_liquids_first_position = summary.first_position;
                    Ok(ClientSessionEvent::SetTileLiquids {
                        liquid_id: summary.liquid_id,
                        amount_bits: summary.amount_bits,
                        position_count: summary.position_count,
                        first_position: summary.first_position,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.info_message_packet_id => {
                if let Some(message) = decode_optional_typeio_string_payload(&packet.payload) {
                    self.state.received_info_message_count =
                        self.state.received_info_message_count.saturating_add(1);
                    self.state.last_info_message = message.clone();
                    Ok(ClientSessionEvent::InfoMessage { message })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.info_popup_packet_id => {
                if let Some(summary) = decode_info_popup_payload(&packet.payload, false) {
                    self.state.received_info_popup_count =
                        self.state.received_info_popup_count.saturating_add(1);
                    self.state.last_info_popup_reliable = Some(false);
                    self.state.last_info_popup_id = summary.popup_id.clone();
                    self.state.last_info_popup_message = summary.message.clone();
                    self.state.last_info_popup_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_info_popup_align = Some(summary.align);
                    self.state.last_info_popup_top = Some(summary.top);
                    self.state.last_info_popup_left = Some(summary.left);
                    self.state.last_info_popup_bottom = Some(summary.bottom);
                    self.state.last_info_popup_right = Some(summary.right);
                    Ok(ClientSessionEvent::InfoPopup {
                        reliable: false,
                        popup_id: summary.popup_id,
                        message: summary.message,
                        duration: summary.duration,
                        align: summary.align,
                        top: summary.top,
                        left: summary.left,
                        bottom: summary.bottom,
                        right: summary.right,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.info_popup_with_id_packet_id => {
                if let Some(summary) = decode_info_popup_payload(&packet.payload, true) {
                    self.state.received_info_popup_count =
                        self.state.received_info_popup_count.saturating_add(1);
                    self.state.last_info_popup_reliable = Some(false);
                    self.state.last_info_popup_id = summary.popup_id.clone();
                    self.state.last_info_popup_message = summary.message.clone();
                    self.state.last_info_popup_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_info_popup_align = Some(summary.align);
                    self.state.last_info_popup_top = Some(summary.top);
                    self.state.last_info_popup_left = Some(summary.left);
                    self.state.last_info_popup_bottom = Some(summary.bottom);
                    self.state.last_info_popup_right = Some(summary.right);
                    Ok(ClientSessionEvent::InfoPopup {
                        reliable: false,
                        popup_id: summary.popup_id,
                        message: summary.message,
                        duration: summary.duration,
                        align: summary.align,
                        top: summary.top,
                        left: summary.left,
                        bottom: summary.bottom,
                        right: summary.right,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.info_popup_reliable_packet_id => {
                if let Some(summary) = decode_info_popup_payload(&packet.payload, false) {
                    self.state.received_info_popup_reliable_count = self
                        .state
                        .received_info_popup_reliable_count
                        .saturating_add(1);
                    self.state.last_info_popup_reliable = Some(true);
                    self.state.last_info_popup_id = summary.popup_id.clone();
                    self.state.last_info_popup_message = summary.message.clone();
                    self.state.last_info_popup_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_info_popup_align = Some(summary.align);
                    self.state.last_info_popup_top = Some(summary.top);
                    self.state.last_info_popup_left = Some(summary.left);
                    self.state.last_info_popup_bottom = Some(summary.bottom);
                    self.state.last_info_popup_right = Some(summary.right);
                    Ok(ClientSessionEvent::InfoPopup {
                        reliable: true,
                        popup_id: summary.popup_id,
                        message: summary.message,
                        duration: summary.duration,
                        align: summary.align,
                        top: summary.top,
                        left: summary.left,
                        bottom: summary.bottom,
                        right: summary.right,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.info_popup_reliable_with_id_packet_id => {
                if let Some(summary) = decode_info_popup_payload(&packet.payload, true) {
                    self.state.received_info_popup_reliable_count = self
                        .state
                        .received_info_popup_reliable_count
                        .saturating_add(1);
                    self.state.last_info_popup_reliable = Some(true);
                    self.state.last_info_popup_id = summary.popup_id.clone();
                    self.state.last_info_popup_message = summary.message.clone();
                    self.state.last_info_popup_duration_bits = Some(summary.duration.to_bits());
                    self.state.last_info_popup_align = Some(summary.align);
                    self.state.last_info_popup_top = Some(summary.top);
                    self.state.last_info_popup_left = Some(summary.left);
                    self.state.last_info_popup_bottom = Some(summary.bottom);
                    self.state.last_info_popup_right = Some(summary.right);
                    Ok(ClientSessionEvent::InfoPopup {
                        reliable: true,
                        popup_id: summary.popup_id,
                        message: summary.message,
                        duration: summary.duration,
                        align: summary.align,
                        top: summary.top,
                        left: summary.left,
                        bottom: summary.bottom,
                        right: summary.right,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.info_toast_packet_id => {
                if let Some(summary) = decode_info_toast_payload(&packet.payload) {
                    self.state.received_info_toast_count =
                        self.state.received_info_toast_count.saturating_add(1);
                    self.state.last_info_toast_message = summary.message.clone();
                    self.state.last_info_toast_duration_bits = Some(summary.duration.to_bits());
                    Ok(ClientSessionEvent::InfoToast {
                        message: summary.message,
                        duration: summary.duration,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.warning_toast_packet_id => {
                if let Some(summary) = decode_warning_toast_payload(&packet.payload) {
                    self.state.received_warning_toast_count =
                        self.state.received_warning_toast_count.saturating_add(1);
                    self.state.last_warning_toast_unicode = Some(summary.unicode);
                    self.state.last_warning_toast_text = summary.text.clone();
                    Ok(ClientSessionEvent::WarningToast {
                        unicode: summary.unicode,
                        text: summary.text,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_player_team_editor_packet_id => {
                if let Some(team_id) = decode_set_player_team_editor_payload(&packet.payload) {
                    self.state.received_set_player_team_editor_count = self
                        .state
                        .received_set_player_team_editor_count
                        .saturating_add(1);
                    self.state.last_set_player_team_editor_team_id = Some(team_id);
                    Ok(ClientSessionEvent::SetPlayerTeamEditor { team_id })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.menu_choose_packet_id => {
                if let Some(summary) = decode_menu_choose_payload(&packet.payload) {
                    self.state.received_menu_choose_count =
                        self.state.received_menu_choose_count.saturating_add(1);
                    self.state.last_menu_choose_menu_id = Some(summary.menu_id);
                    self.state.last_menu_choose_option = Some(summary.option);
                    Ok(ClientSessionEvent::MenuChoose {
                        menu_id: summary.menu_id,
                        option: summary.option,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.text_input_result_packet_id => {
                if let Some(summary) = decode_text_input_result_payload(&packet.payload) {
                    self.state.received_text_input_result_count = self
                        .state
                        .received_text_input_result_count
                        .saturating_add(1);
                    self.state.last_text_input_result_id = Some(summary.text_input_id);
                    self.state.last_text_input_result_text = summary.text.clone();
                    Ok(ClientSessionEvent::TextInputResult {
                        text_input_id: summary.text_input_id,
                        text: summary.text,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.request_item_packet_id => {
                if let Some(summary) = decode_request_item_inbound_payload(&packet.payload) {
                    self.state.received_request_item_count =
                        self.state.received_request_item_count.saturating_add(1);
                    self.state.last_request_item_build_pos = summary.build_pos;
                    self.state.last_request_item_item_id = summary.item_id;
                    self.state.last_request_item_amount = Some(summary.amount);
                    Ok(ClientSessionEvent::RequestItem {
                        build_pos: summary.build_pos,
                        item_id: summary.item_id,
                        amount: summary.amount,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.request_build_payload_packet_id => {
                if let Some(build_pos) = decode_request_build_payload_payload(&packet.payload) {
                    self.state.received_request_build_payload_count = self
                        .state
                        .received_request_build_payload_count
                        .saturating_add(1);
                    self.state.last_request_build_payload_build_pos = build_pos;
                    Ok(ClientSessionEvent::RequestBuildPayload { build_pos })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.request_unit_payload_packet_id => {
                if let Some(target) = decode_request_unit_payload_payload(&packet.payload) {
                    self.state.received_request_unit_payload_count = self
                        .state
                        .received_request_unit_payload_count
                        .saturating_add(1);
                    self.state.last_request_unit_payload_target = target;
                    Ok(ClientSessionEvent::RequestUnitPayload { target })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.transfer_inventory_packet_id => {
                if let Some(build_pos) = decode_transfer_inventory_payload(&packet.payload) {
                    self.state.received_transfer_inventory_count = self
                        .state
                        .received_transfer_inventory_count
                        .saturating_add(1);
                    self.state.last_transfer_inventory_build_pos = build_pos;
                    Ok(ClientSessionEvent::TransferInventory { build_pos })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.rotate_block_packet_id => {
                if let Some((build_pos, direction)) = decode_rotate_block_payload(&packet.payload) {
                    self.state.received_rotate_block_count =
                        self.state.received_rotate_block_count.saturating_add(1);
                    self.state.last_rotate_block_build_pos = build_pos;
                    self.state.last_rotate_block_direction = Some(direction);
                    Ok(ClientSessionEvent::RotateBlock {
                        build_pos,
                        direction,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.drop_item_packet_id => {
                if let Some(angle) = decode_drop_item_payload(&packet.payload) {
                    self.state.received_drop_item_count =
                        self.state.received_drop_item_count.saturating_add(1);
                    self.state.last_drop_item_angle_bits = Some(angle.to_bits());
                    Ok(ClientSessionEvent::DropItem { angle })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.delete_plans_packet_id => {
                if let Some(positions) = decode_delete_plans_payload(&packet.payload) {
                    self.state.received_delete_plans_count =
                        self.state.received_delete_plans_count.saturating_add(1);
                    self.state.last_delete_plans_count = positions.len();
                    self.state.last_delete_plans_first_pos = positions.first().copied();
                    Ok(ClientSessionEvent::DeletePlans { positions })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.building_control_select_packet_id => {
                if let Some(build_pos) = decode_building_control_select_payload(&packet.payload) {
                    self.state.received_building_control_select_count = self
                        .state
                        .received_building_control_select_count
                        .saturating_add(1);
                    self.state.last_building_control_select_build_pos = build_pos;
                    Ok(ClientSessionEvent::BuildingControlSelect { build_pos })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.unit_clear_packet_id => {
                if packet.payload.is_empty() {
                    self.state.received_unit_clear_count =
                        self.state.received_unit_clear_count.saturating_add(1);
                    Ok(ClientSessionEvent::UnitClear)
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.unit_control_packet_id => {
                if let Some(target) = decode_unit_control_payload(&packet.payload) {
                    self.state.received_unit_control_count =
                        self.state.received_unit_control_count.saturating_add(1);
                    self.state.last_unit_control_target = target;
                    Ok(ClientSessionEvent::UnitControl { target })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.unit_building_control_select_packet_id => {
                if let Some(summary) = decode_unit_building_control_select_payload(&packet.payload)
                {
                    self.state.received_unit_building_control_select_count = self
                        .state
                        .received_unit_building_control_select_count
                        .saturating_add(1);
                    self.state.last_unit_building_control_select_target = summary.target;
                    self.state.last_unit_building_control_select_build_pos = summary.build_pos;
                    Ok(ClientSessionEvent::UnitBuildingControlSelect {
                        target: summary.target,
                        build_pos: summary.build_pos,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.command_building_packet_id => {
                if let Some(summary) = decode_command_building_payload(&packet.payload) {
                    self.state.received_command_building_count =
                        self.state.received_command_building_count.saturating_add(1);
                    self.state.last_command_building_count = summary.buildings.len();
                    self.state.last_command_building_first_build_pos =
                        summary.buildings.first().copied();
                    self.state.last_command_building_x_bits = Some(summary.x.to_bits());
                    self.state.last_command_building_y_bits = Some(summary.y.to_bits());
                    Ok(ClientSessionEvent::CommandBuilding {
                        buildings: summary.buildings,
                        x: summary.x,
                        y: summary.y,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.command_units_packet_id => {
                if let Some(summary) = decode_command_units_payload(&packet.payload) {
                    self.state.received_command_units_count =
                        self.state.received_command_units_count.saturating_add(1);
                    self.state.last_command_units_count = summary.unit_ids.len();
                    self.state.last_command_units_first_unit_id = summary.unit_ids.first().copied();
                    self.state.last_command_units_build_target = summary.build_target;
                    self.state.last_command_units_unit_target = summary.unit_target;
                    self.state.last_command_units_x_bits = Some(summary.x.to_bits());
                    self.state.last_command_units_y_bits = Some(summary.y.to_bits());
                    self.state.last_command_units_queue = Some(summary.queue_command);
                    self.state.last_command_units_final_batch = Some(summary.final_batch);
                    Ok(ClientSessionEvent::CommandUnits {
                        unit_ids: summary.unit_ids,
                        build_target: summary.build_target,
                        unit_target: summary.unit_target,
                        x: summary.x,
                        y: summary.y,
                        queue_command: summary.queue_command,
                        final_batch: summary.final_batch,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_unit_command_packet_id => {
                if let Some(summary) = decode_set_unit_command_payload(&packet.payload) {
                    self.state.received_set_unit_command_count =
                        self.state.received_set_unit_command_count.saturating_add(1);
                    self.state.last_set_unit_command_count = summary.unit_ids.len();
                    self.state.last_set_unit_command_first_unit_id =
                        summary.unit_ids.first().copied();
                    self.state.last_set_unit_command_id = summary.command_id;
                    Ok(ClientSessionEvent::SetUnitCommand {
                        unit_ids: summary.unit_ids,
                        command_id: summary.command_id,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_unit_stance_packet_id => {
                if let Some(summary) = decode_set_unit_stance_payload(&packet.payload) {
                    self.state.received_set_unit_stance_count =
                        self.state.received_set_unit_stance_count.saturating_add(1);
                    self.state.last_set_unit_stance_count = summary.unit_ids.len();
                    self.state.last_set_unit_stance_first_unit_id =
                        summary.unit_ids.first().copied();
                    self.state.last_set_unit_stance_id = summary.stance_id;
                    self.state.last_set_unit_stance_enable = Some(summary.enable);
                    Ok(ClientSessionEvent::SetUnitStance {
                        unit_ids: summary.unit_ids,
                        stance_id: summary.stance_id,
                        enable: summary.enable,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.remove_queue_block_packet_id => {
                if let Some((x, y, breaking)) = decode_remove_queue_block_payload(&packet.payload) {
                    self.sync_builder_queue_projection_from_snapshot_input();
                    let removed_local_plan = self.remove_snapshot_plan(x, y);
                    self.state.received_remove_queue_block_count = self
                        .state
                        .received_remove_queue_block_count
                        .saturating_add(1);
                    self.state.last_remove_queue_block_x = Some(x);
                    self.state.last_remove_queue_block_y = Some(y);
                    self.state.last_remove_queue_block_breaking = Some(breaking);
                    self.state.last_remove_queue_block_removed_local_plan = removed_local_plan;
                    self.state.builder_queue_projection.mark_remove_queue_block(
                        x,
                        y,
                        breaking,
                        removed_local_plan,
                    );
                    self.sync_snapshot_building_from_builder_queue_projection();
                    Ok(ClientSessionEvent::RemoveQueueBlock {
                        x,
                        y,
                        breaking,
                        removed_local_plan,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.tile_config_packet_id => {
                if let Some(summary) = decode_tile_config_payload(&packet.payload) {
                    let TileConfigSummary {
                        build_pos,
                        config_kind,
                        config_kind_name,
                        config_consumed_len,
                        config_object,
                        parse_failed,
                        parse_error,
                    } = summary;
                    let business_apply = if !parse_failed {
                        if let (Some(build_pos), Some(config_object)) =
                            (build_pos, config_object.clone())
                        {
                            self.state
                                .building_table_projection
                                .apply_tile_config(build_pos, config_object.clone());
                            self.state
                                .tile_config_projection
                                .apply_authoritative_update(build_pos, config_object)
                        } else {
                            self.state
                                .tile_config_projection
                                .mark_packet_without_business_apply();
                            TileConfigBusinessApply::default()
                        }
                    } else {
                        self.state
                            .tile_config_projection
                            .mark_packet_without_business_apply();
                        TileConfigBusinessApply::default()
                    };
                    self.state.received_tile_config_count =
                        self.state.received_tile_config_count.saturating_add(1);
                    self.state.last_tile_config_build_pos = build_pos;
                    self.state.last_tile_config_kind = config_kind;
                    self.state.last_tile_config_kind_name = config_kind_name.clone();
                    self.state.last_tile_config_consumed_len = config_consumed_len;
                    self.state.last_tile_config_object = config_object.clone();
                    self.state.last_tile_config_parse_failed = parse_failed;
                    if parse_failed {
                        self.state.failed_tile_config_parse_count =
                            self.state.failed_tile_config_parse_count.saturating_add(1);
                    }
                    self.state.last_tile_config_parse_error = parse_error;
                    Ok(ClientSessionEvent::TileConfig {
                        build_pos,
                        config_kind,
                        config_kind_name,
                        parse_failed,
                        business_applied: business_apply.business_applied,
                        cleared_pending_local: business_apply.cleared_pending_local,
                        was_rollback: business_apply.was_rollback,
                        pending_local_match: business_apply.pending_local_match,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.begin_break_packet_id => {
                if let Some(summary) = decode_begin_break_payload(&packet.payload) {
                    self.sync_builder_queue_projection_from_snapshot_input();
                    self.state.received_begin_break_count =
                        self.state.received_begin_break_count.saturating_add(1);
                    self.state.last_begin_break_x = Some(summary.x);
                    self.state.last_begin_break_y = Some(summary.y);
                    self.state.last_begin_break_team_id = Some(summary.team_id);
                    self.state.builder_queue_projection.mark_begin_break(
                        summary.x,
                        summary.y,
                        summary.team_id,
                        summary.builder_kind,
                        summary.builder_value,
                    );
                    self.sync_snapshot_building_from_builder_queue_projection();
                    Ok(ClientSessionEvent::BeginBreak {
                        x: summary.x,
                        y: summary.y,
                        team_id: summary.team_id,
                        builder_kind: summary.builder_kind,
                        builder_value: summary.builder_value,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.begin_place_packet_id => {
                if let Some(summary) = decode_begin_place_payload(&packet.payload) {
                    self.sync_builder_queue_projection_from_snapshot_input();
                    self.state.received_begin_place_count =
                        self.state.received_begin_place_count.saturating_add(1);
                    self.state.last_begin_place_x = Some(summary.x);
                    self.state.last_begin_place_y = Some(summary.y);
                    self.state.last_begin_place_block_id = summary.block_id;
                    self.state.last_begin_place_rotation = Some(summary.rotation);
                    self.state.last_begin_place_team_id = Some(summary.team_id);
                    self.state.last_begin_place_config_kind = Some(summary.config_kind);
                    self.state.last_begin_place_config_kind_name =
                        Some(summary.config_kind_name.to_string());
                    self.state.last_begin_place_config_consumed_len =
                        Some(summary.config_consumed_len);
                    self.state.last_begin_place_config_object = Some(summary.config_object.clone());
                    self.state.builder_queue_projection.mark_begin_place(
                        summary.x,
                        summary.y,
                        summary.block_id,
                        u8::try_from(summary.rotation).unwrap_or_default(),
                        summary.team_id,
                        summary.builder_kind,
                        summary.builder_value,
                    );
                    self.sync_snapshot_building_from_builder_queue_projection();
                    Ok(ClientSessionEvent::BeginPlace {
                        x: summary.x,
                        y: summary.y,
                        block_id: summary.block_id,
                        rotation: summary.rotation,
                        team_id: summary.team_id,
                        config_kind: summary.config_kind,
                        config_kind_name: summary.config_kind_name,
                        builder_kind: summary.builder_kind,
                        builder_value: summary.builder_value,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.construct_finish_packet_id => {
                if let Some(summary) = decode_construct_finish_payload(&packet.payload) {
                    self.sync_builder_queue_projection_from_snapshot_input();
                    let ConstructFinishSummary {
                        tile_pos,
                        block_id,
                        builder_kind,
                        builder_value,
                        rotation,
                        team_id,
                        config_kind,
                        config_kind_name,
                        config_consumed_len,
                        config_object,
                    } = summary;
                    let removed_local_plan = self.remove_snapshot_plan_by_tile_pos(tile_pos);
                    let (tile_x, tile_y) = unpack_point2(tile_pos);
                    self.state.received_construct_finish_count =
                        self.state.received_construct_finish_count.saturating_add(1);
                    self.state.last_construct_finish_tile_pos = Some(tile_pos);
                    self.state.last_construct_finish_block_id = block_id;
                    self.state.last_construct_finish_config_kind = Some(config_kind);
                    self.state.last_construct_finish_config_kind_name =
                        Some(config_kind_name.to_string());
                    self.state.last_construct_finish_config_consumed_len =
                        Some(config_consumed_len);
                    self.state.last_construct_finish_config_object = Some(config_object);
                    self.state.tile_config_projection.seed_authoritative_state(
                        tile_pos,
                        self.state
                            .last_construct_finish_config_object
                            .clone()
                            .unwrap_or(TypeIoObject::Null),
                    );
                    self.state.building_table_projection.apply_construct_finish(
                        tile_pos,
                        block_id,
                        rotation,
                        team_id,
                        self.state
                            .last_construct_finish_config_object
                            .clone()
                            .unwrap_or(TypeIoObject::Null),
                    );
                    self.state.last_construct_finish_removed_local_plan = removed_local_plan;
                    self.state.builder_queue_projection.mark_construct_finish(
                        i32::from(tile_x),
                        i32::from(tile_y),
                        block_id,
                        rotation,
                        team_id,
                        builder_kind,
                        builder_value,
                        removed_local_plan,
                    );
                    self.sync_snapshot_building_from_builder_queue_projection();
                    Ok(ClientSessionEvent::ConstructFinish {
                        tile_pos,
                        block_id,
                        builder_kind,
                        builder_value,
                        rotation,
                        team_id,
                        config_kind,
                        removed_local_plan,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.deconstruct_finish_packet_id => {
                if let Some(summary) = decode_deconstruct_finish_payload(&packet.payload) {
                    self.sync_builder_queue_projection_from_snapshot_input();
                    let removed_local_plan =
                        self.remove_snapshot_plan_by_tile_pos(summary.tile_pos);
                    let (tile_x, tile_y) = unpack_point2(summary.tile_pos);
                    self.state.received_deconstruct_finish_count = self
                        .state
                        .received_deconstruct_finish_count
                        .saturating_add(1);
                    self.state.last_deconstruct_finish_tile_pos = Some(summary.tile_pos);
                    self.state.last_deconstruct_finish_block_id = summary.block_id;
                    self.state
                        .tile_config_projection
                        .remove_building_state(summary.tile_pos);
                    self.state
                        .building_table_projection
                        .apply_deconstruct_finish(summary.tile_pos, summary.block_id);
                    self.state.last_deconstruct_finish_removed_local_plan = removed_local_plan;
                    self.state.builder_queue_projection.mark_deconstruct_finish(
                        i32::from(tile_x),
                        i32::from(tile_y),
                        summary.block_id,
                        summary.builder_kind,
                        summary.builder_value,
                        removed_local_plan,
                    );
                    self.sync_snapshot_building_from_builder_queue_projection();
                    Ok(ClientSessionEvent::DeconstructFinish {
                        tile_pos: summary.tile_pos,
                        block_id: summary.block_id,
                        builder_kind: summary.builder_kind,
                        builder_value: summary.builder_value,
                        removed_local_plan,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.build_health_update_packet_id => {
                if let Some(summary) = try_read_build_health_update_payload(&packet.payload) {
                    self.state.received_build_health_update_count = self
                        .state
                        .received_build_health_update_count
                        .saturating_add(1);
                    self.state.received_build_health_update_pair_count = self
                        .state
                        .received_build_health_update_pair_count
                        .saturating_add(summary.pair_count as u64);
                    self.state.last_build_health_update_pair_count = summary.pair_count;
                    self.state.last_build_health_update_first_build_pos = summary.first_build_pos;
                    self.state.last_build_health_update_first_health_bits =
                        summary.first_health_bits;
                    for pair in &summary.pairs {
                        self.state
                            .building_table_projection
                            .apply_build_health(pair.build_pos, pair.health_bits);
                    }
                    Ok(ClientSessionEvent::BuildHealthUpdate {
                        pair_count: summary.pair_count,
                        first_build_pos: summary.first_build_pos,
                        first_health_bits: summary.first_health_bits,
                        pairs: summary.pairs,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.send_message_with_sender_packet_id => {
                if let Some(message) = try_read_chat_message_payload(&packet.payload) {
                    self.state.received_chat_message_count =
                        self.state.received_chat_message_count.saturating_add(1);
                    self.state.last_chat_message = Some(message.message.clone());
                    self.state.last_chat_unformatted = message.unformatted.clone();
                    self.state.last_chat_sender_entity_id = message.sender_entity_id;
                    Ok(ClientSessionEvent::ChatMessage {
                        message: message.message,
                        unformatted: message.unformatted,
                        sender_entity_id: message.sender_entity_id,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.send_message_packet_id => {
                if let Some(message) = try_read_typeio_string(&packet.payload) {
                    self.state.received_server_message_count =
                        self.state.received_server_message_count.saturating_add(1);
                    self.state.last_server_message = Some(message.clone());
                    Ok(ClientSessionEvent::ServerMessage { message })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id
                if Some(packet_id) == self.client_packet_reliable_packet_id
                    || Some(packet_id) == self.server_packet_reliable_packet_id =>
            {
                if let Some((packet_type, contents)) = decode_client_packet_payload(&packet.payload)
                {
                    self.state.received_client_packet_reliable_count = self
                        .state
                        .received_client_packet_reliable_count
                        .saturating_add(1);
                    self.state.last_client_packet_reliable_type = Some(packet_type.clone());
                    self.state.last_client_packet_reliable_contents = Some(contents.clone());
                    self.client_packet_handlers
                        .dispatch(&packet_type, &contents);
                    Ok(ClientSessionEvent::ClientPacketReliable {
                        packet_type,
                        contents,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id
                if Some(packet_id) == self.client_packet_unreliable_packet_id
                    || Some(packet_id) == self.server_packet_unreliable_packet_id =>
            {
                if let Some((packet_type, contents)) = decode_client_packet_payload(&packet.payload)
                {
                    self.state.received_client_packet_unreliable_count = self
                        .state
                        .received_client_packet_unreliable_count
                        .saturating_add(1);
                    self.state.last_client_packet_unreliable_type = Some(packet_type.clone());
                    self.state.last_client_packet_unreliable_contents = Some(contents.clone());
                    self.client_packet_handlers
                        .dispatch(&packet_type, &contents);
                    Ok(ClientSessionEvent::ClientPacketUnreliable {
                        packet_type,
                        contents,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id
                if Some(packet_id) == self.client_binary_packet_reliable_packet_id
                    || Some(packet_id) == self.server_binary_packet_reliable_packet_id =>
            {
                if let Some((packet_type, contents)) =
                    decode_client_binary_packet_payload(&packet.payload)
                {
                    self.state.received_client_binary_packet_reliable_count = self
                        .state
                        .received_client_binary_packet_reliable_count
                        .saturating_add(1);
                    self.state.last_client_binary_packet_reliable_type = Some(packet_type.clone());
                    self.state.last_client_binary_packet_reliable_contents = Some(contents.clone());
                    self.client_binary_packet_handlers
                        .dispatch(&packet_type, &contents);
                    Ok(ClientSessionEvent::ClientBinaryPacketReliable {
                        packet_type,
                        contents,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id
                if Some(packet_id) == self.client_binary_packet_unreliable_packet_id
                    || Some(packet_id) == self.server_binary_packet_unreliable_packet_id =>
            {
                if let Some((packet_type, contents)) =
                    decode_client_binary_packet_payload(&packet.payload)
                {
                    self.state.received_client_binary_packet_unreliable_count = self
                        .state
                        .received_client_binary_packet_unreliable_count
                        .saturating_add(1);
                    self.state.last_client_binary_packet_unreliable_type =
                        Some(packet_type.clone());
                    self.state.last_client_binary_packet_unreliable_contents =
                        Some(contents.clone());
                    self.client_binary_packet_handlers
                        .dispatch(&packet_type, &contents);
                    Ok(ClientSessionEvent::ClientBinaryPacketUnreliable {
                        packet_type,
                        contents,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.client_logic_data_reliable_packet_id => {
                if let Some((channel, value)) = decode_client_logic_data_payload(&packet.payload) {
                    self.state.received_client_logic_data_reliable_count = self
                        .state
                        .received_client_logic_data_reliable_count
                        .saturating_add(1);
                    self.state.last_client_logic_data_reliable_channel = Some(channel.clone());
                    self.state.last_client_logic_data_reliable_value = Some(value.clone());
                    self.client_logic_data_handlers.dispatch(
                        &channel,
                        ClientLogicDataTransport::Reliable,
                        &value,
                    );
                    Ok(ClientSessionEvent::ClientLogicDataReliable { channel, value })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.client_logic_data_unreliable_packet_id => {
                if let Some((channel, value)) = decode_client_logic_data_payload(&packet.payload) {
                    self.state.received_client_logic_data_unreliable_count = self
                        .state
                        .received_client_logic_data_unreliable_count
                        .saturating_add(1);
                    self.state.last_client_logic_data_unreliable_channel = Some(channel.clone());
                    self.state.last_client_logic_data_unreliable_value = Some(value.clone());
                    self.client_logic_data_handlers.dispatch(
                        &channel,
                        ClientLogicDataTransport::Unreliable,
                        &value,
                    );
                    Ok(ClientSessionEvent::ClientLogicDataUnreliable { channel, value })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_rules_packet_id => {
                if let Ok(json_data) = decode_length_prefixed_json_payload(&packet.payload) {
                    self.state.received_set_rules_count =
                        self.state.received_set_rules_count.saturating_add(1);
                    self.state.last_set_rules_json_data = Some(json_data.clone());
                    self.state.last_set_rules_parse_error = None;
                    self.state.last_set_rules_parse_error_payload_len = None;
                    self.state.rules_projection.apply_set_rules_json(&json_data);
                    Ok(ClientSessionEvent::RulesUpdatedRaw { json_data })
                } else {
                    let error = decode_length_prefixed_json_payload(&packet.payload)
                        .err()
                        .unwrap_or_else(|| "unknown setRules payload decode error".to_string());
                    self.state.failed_set_rules_parse_count =
                        self.state.failed_set_rules_parse_count.saturating_add(1);
                    self.state.last_set_rules_parse_error = Some(error);
                    self.state.last_set_rules_parse_error_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_objectives_packet_id => {
                if let Ok(json_data) = decode_length_prefixed_json_payload(&packet.payload) {
                    self.state.received_set_objectives_count =
                        self.state.received_set_objectives_count.saturating_add(1);
                    self.state.last_set_objectives_json_data = Some(json_data.clone());
                    self.state.last_set_objectives_parse_error = None;
                    self.state.last_set_objectives_parse_error_payload_len = None;
                    self.state
                        .objectives_projection
                        .replace_from_json(&json_data);
                    Ok(ClientSessionEvent::ObjectivesUpdatedRaw { json_data })
                } else {
                    let error = decode_length_prefixed_json_payload(&packet.payload)
                        .err()
                        .unwrap_or_else(|| {
                            "unknown setObjectives payload decode error".to_string()
                        });
                    self.state.failed_set_objectives_parse_count = self
                        .state
                        .failed_set_objectives_parse_count
                        .saturating_add(1);
                    self.state.last_set_objectives_parse_error = Some(error);
                    self.state.last_set_objectives_parse_error_payload_len =
                        Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_rule_packet_id => {
                if let Ok((rule, json_data)) = decode_set_rule_payload(&packet.payload) {
                    self.state.received_set_rule_count =
                        self.state.received_set_rule_count.saturating_add(1);
                    self.state.last_set_rule_name = Some(rule.clone());
                    self.state.last_set_rule_json_data = Some(json_data.clone());
                    self.state.last_set_rule_parse_error = None;
                    self.state.last_set_rule_parse_error_payload_len = None;
                    self.state
                        .rules_projection
                        .apply_set_rule_patch(&rule, &json_data);
                    Ok(ClientSessionEvent::SetRuleApplied { rule, json_data })
                } else {
                    let error = decode_set_rule_payload(&packet.payload)
                        .err()
                        .unwrap_or_else(|| "unknown setRule payload decode error".to_string());
                    self.state.failed_set_rule_parse_count =
                        self.state.failed_set_rule_parse_count.saturating_add(1);
                    self.state.last_set_rule_parse_error = Some(error);
                    self.state.last_set_rule_parse_error_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.clear_objectives_packet_id => {
                if packet.payload.is_empty() {
                    self.state.received_clear_objectives_count =
                        self.state.received_clear_objectives_count.saturating_add(1);
                    self.state.objectives_projection.clear();
                    Ok(ClientSessionEvent::ObjectivesCleared)
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.complete_objective_packet_id => {
                if let Some(index) = decode_complete_objective_payload(&packet.payload) {
                    self.state.received_complete_objective_count = self
                        .state
                        .received_complete_objective_count
                        .saturating_add(1);
                    self.state.last_complete_objective_index = Some(index);
                    self.state.objectives_projection.complete_by_index(index);
                    Ok(ClientSessionEvent::ObjectiveCompleted { index })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.set_position_packet_id => {
                let (x, y) = decode_set_position_payload(&packet.payload)?;
                self.state.world_player_x_bits = Some(x.to_bits());
                self.state.world_player_y_bits = Some(y.to_bits());
                self.snapshot_input.position = Some((x, y));
                self.snapshot_input.view_center = Some((x, y));
                Ok(ClientSessionEvent::PlayerPositionUpdated { x, y })
            }
            packet_id if Some(packet_id) == self.set_camera_position_packet_id => {
                let (x, y) = decode_set_position_payload(&packet.payload)?;
                self.state.received_set_camera_position_count = self
                    .state
                    .received_set_camera_position_count
                    .saturating_add(1);
                self.state.last_camera_x_bits = Some(x.to_bits());
                self.state.last_camera_y_bits = Some(y.to_bits());
                self.snapshot_input.view_center = Some((x, y));
                Ok(ClientSessionEvent::CameraPositionUpdated { x, y })
            }
            packet_id if Some(packet_id) == self.take_items_packet_id => {
                if let Some(projection) = decode_take_items_payload(&packet.payload) {
                    self.state.received_take_items_count =
                        self.state.received_take_items_count.saturating_add(1);
                    self.state.last_take_items = Some(projection.clone());
                    Ok(ClientSessionEvent::TakeItems { projection })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.transfer_item_to_packet_id => {
                if let Some(projection) = decode_transfer_item_to_payload(&packet.payload) {
                    self.state.received_transfer_item_to_count =
                        self.state.received_transfer_item_to_count.saturating_add(1);
                    self.state.last_transfer_item_to = Some(projection.clone());
                    Ok(ClientSessionEvent::TransferItemTo { projection })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.transfer_item_to_unit_packet_id => {
                if let Some(projection) = decode_transfer_item_to_unit_payload(&packet.payload) {
                    self.state.received_transfer_item_to_unit_count = self
                        .state
                        .received_transfer_item_to_unit_count
                        .saturating_add(1);
                    self.state.last_transfer_item_to_unit = Some(projection.clone());
                    Ok(ClientSessionEvent::TransferItemToUnit { projection })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.payload_dropped_packet_id => {
                if let Some(projection) = decode_payload_dropped_payload(&packet.payload) {
                    self.state.received_payload_dropped_count =
                        self.state.received_payload_dropped_count.saturating_add(1);
                    self.state.last_payload_dropped = Some(projection.clone());
                    Ok(ClientSessionEvent::PayloadDropped { projection })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.picked_build_payload_packet_id => {
                if let Some(projection) = decode_picked_build_payload(&packet.payload) {
                    self.state.received_picked_build_payload_count = self
                        .state
                        .received_picked_build_payload_count
                        .saturating_add(1);
                    self.state.last_picked_build_payload = Some(projection.clone());
                    Ok(ClientSessionEvent::PickedBuildPayload { projection })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.picked_unit_payload_packet_id => {
                if let Some(projection) = decode_picked_unit_payload(&packet.payload) {
                    self.state.received_picked_unit_payload_count = self
                        .state
                        .received_picked_unit_payload_count
                        .saturating_add(1);
                    self.state.last_picked_unit_payload = Some(projection.clone());
                    Ok(ClientSessionEvent::PickedUnitPayload { projection })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.unit_entered_payload_packet_id => {
                if let Some(projection) = decode_unit_entered_payload(&packet.payload) {
                    self.state.received_unit_entered_payload_count = self
                        .state
                        .received_unit_entered_payload_count
                        .saturating_add(1);
                    self.state.last_unit_entered_payload = Some(projection.clone());
                    let removed_entity_projection = projection
                        .unit
                        .filter(|unit| unit.kind == 2)
                        .is_some_and(|unit| {
                            self.state.record_entity_snapshot_tombstone(unit.value);
                            self.state.entity_table_projection.remove_entity(unit.value)
                        });
                    Ok(ClientSessionEvent::UnitEnteredPayload {
                        projection,
                        removed_entity_projection,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.unit_despawn_packet_id => {
                if let Some(unit) = decode_unit_despawn_payload(&packet.payload) {
                    self.state.received_unit_despawn_count =
                        self.state.received_unit_despawn_count.saturating_add(1);
                    self.state.last_unit_despawn = unit;
                    let removed_entity_projection =
                        unit.filter(|unit| unit.kind == 2).is_some_and(|unit| {
                            self.state.record_entity_snapshot_tombstone(unit.value);
                            self.state.entity_table_projection.remove_entity(unit.value)
                        });
                    Ok(ClientSessionEvent::UnitDespawned {
                        unit,
                        removed_entity_projection,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.sound_packet_id => {
                if let Some(sound) = decode_sound_payload(&packet.payload) {
                    self.state.received_sound_count =
                        self.state.received_sound_count.saturating_add(1);
                    self.state.last_sound_id = sound.sound_id;
                    self.state.last_sound_volume_bits = Some(sound.volume.to_bits());
                    self.state.last_sound_pitch_bits = Some(sound.pitch.to_bits());
                    self.state.last_sound_pan_bits = Some(sound.pan.to_bits());
                    self.state.last_sound_parse_error_payload_len = None;
                    Ok(ClientSessionEvent::SoundRequested {
                        sound_id: sound.sound_id,
                        volume: sound.volume,
                        pitch: sound.pitch,
                        pan: sound.pan,
                    })
                } else {
                    self.state.failed_sound_parse_count =
                        self.state.failed_sound_parse_count.saturating_add(1);
                    self.state.last_sound_parse_error_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.sound_at_packet_id => {
                if let Some(sound) = decode_sound_at_payload(&packet.payload) {
                    self.state.received_sound_at_count =
                        self.state.received_sound_at_count.saturating_add(1);
                    self.state.last_sound_at_id = sound.sound_id;
                    self.state.last_sound_at_x_bits = Some(sound.x.to_bits());
                    self.state.last_sound_at_y_bits = Some(sound.y.to_bits());
                    self.state.last_sound_at_volume_bits = Some(sound.volume.to_bits());
                    self.state.last_sound_at_pitch_bits = Some(sound.pitch.to_bits());
                    self.state.last_sound_at_parse_error_payload_len = None;
                    Ok(ClientSessionEvent::SoundAtRequested {
                        sound_id: sound.sound_id,
                        x: sound.x,
                        y: sound.y,
                        volume: sound.volume,
                        pitch: sound.pitch,
                    })
                } else {
                    self.state.failed_sound_at_parse_count =
                        self.state.failed_sound_at_parse_count.saturating_add(1);
                    self.state.last_sound_at_parse_error_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.effect_packet_id => {
                if let Some(effect) = decode_effect_payload(&packet.payload, false) {
                    self.state.received_effect_count =
                        self.state.received_effect_count.saturating_add(1);
                    self.state.last_effect_id = effect.effect_id;
                    self.state.last_effect_x_bits = Some(effect.x.to_bits());
                    self.state.last_effect_y_bits = Some(effect.y.to_bits());
                    self.state.last_effect_rotation_bits = Some(effect.rotation.to_bits());
                    self.state.last_effect_color_rgba = Some(effect.color_rgba);
                    self.state.last_effect_data_len = None;
                    self.state.last_effect_data_type_tag = None;
                    self.state.last_effect_data_kind = None;
                    self.state.last_effect_data_consumed_len = None;
                    self.state.last_effect_data_object = None;
                    self.state.last_effect_data_semantic = None;
                    self.state.last_effect_business_projection = None;
                    self.state.last_effect_business_path = None;
                    self.state.last_effect_data_parse_failed = false;
                    self.state.last_effect_data_parse_error = None;
                    Ok(ClientSessionEvent::EffectRequested {
                        effect_id: effect.effect_id,
                        x: effect.x,
                        y: effect.y,
                        rotation: effect.rotation,
                        color_rgba: effect.color_rgba,
                        data_object: None,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.effect_with_data_packet_id => {
                if let Some(effect) = decode_effect_payload(&packet.payload, true) {
                    self.state.received_effect_count =
                        self.state.received_effect_count.saturating_add(1);
                    self.state.last_effect_id = effect.effect_id;
                    self.state.last_effect_x_bits = Some(effect.x.to_bits());
                    self.state.last_effect_y_bits = Some(effect.y.to_bits());
                    self.state.last_effect_rotation_bits = Some(effect.rotation.to_bits());
                    self.state.last_effect_color_rgba = Some(effect.color_rgba);
                    self.state.last_effect_data_len = Some(effect.data_len);
                    self.state.last_effect_data_type_tag = effect.data_type_tag;
                    self.state.last_effect_data_kind =
                        effect.data_kind.map(|kind| kind.to_string());
                    self.state.last_effect_data_consumed_len = effect.data_consumed_len;
                    self.state.last_effect_data_object = effect.data_object.clone();
                    self.state.last_effect_data_semantic = derive_effect_data_semantic(
                        effect.data_object.as_ref(),
                        effect.data_type_tag,
                        effect.parse_failed,
                    );
                    let business_projection = derive_effect_business_projection(
                        &self.state,
                        &self.snapshot_input,
                        effect.data_object.as_ref(),
                    );
                    self.state.last_effect_business_path = business_projection.path;
                    self.state.last_effect_business_projection = business_projection.projection;
                    self.state.last_effect_data_parse_failed = effect.parse_failed;
                    if effect.parse_failed {
                        self.state.failed_effect_data_parse_count =
                            self.state.failed_effect_data_parse_count.saturating_add(1);
                    }
                    self.state.last_effect_data_parse_error = effect.parse_error.clone();
                    Ok(ClientSessionEvent::EffectRequested {
                        effect_id: effect.effect_id,
                        x: effect.x,
                        y: effect.y,
                        rotation: effect.rotation,
                        color_rgba: effect.color_rgba,
                        data_object: effect.data_object.clone(),
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.effect_reliable_packet_id => {
                if let Some(effect) = decode_effect_payload(&packet.payload, false) {
                    self.state.received_effect_reliable_count =
                        self.state.received_effect_reliable_count.saturating_add(1);
                    self.state.last_effect_reliable_id = effect.effect_id;
                    self.state.last_effect_reliable_x_bits = Some(effect.x.to_bits());
                    self.state.last_effect_reliable_y_bits = Some(effect.y.to_bits());
                    self.state.last_effect_reliable_rotation_bits = Some(effect.rotation.to_bits());
                    self.state.last_effect_reliable_color_rgba = Some(effect.color_rgba);
                    Ok(ClientSessionEvent::EffectReliableRequested {
                        effect_id: effect.effect_id,
                        x: effect.x,
                        y: effect.y,
                        rotation: effect.rotation,
                        color_rgba: effect.color_rgba,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.trace_info_packet_id => {
                if let Some(trace) = decode_trace_info_payload(&packet.payload) {
                    self.state.received_trace_info_count =
                        self.state.received_trace_info_count.saturating_add(1);
                    self.state.last_trace_info_player_id = trace.player_id;
                    self.state.last_trace_info_ip = trace.ip.clone();
                    self.state.last_trace_info_uuid = trace.uuid.clone();
                    self.state.last_trace_info_locale = trace.locale.clone();
                    self.state.last_trace_info_modded = Some(trace.modded);
                    self.state.last_trace_info_mobile = Some(trace.mobile);
                    self.state.last_trace_info_times_joined = Some(trace.times_joined);
                    self.state.last_trace_info_times_kicked = Some(trace.times_kicked);
                    self.state.last_trace_info_ips = Some(trace.ips.clone());
                    self.state.last_trace_info_names = Some(trace.names.clone());
                    self.state.last_trace_info_parse_error_payload_len = None;
                    Ok(ClientSessionEvent::TraceInfoReceived {
                        player_id: trace.player_id,
                        ip: trace.ip,
                        uuid: trace.uuid,
                        locale: trace.locale,
                        modded: trace.modded,
                        mobile: trace.mobile,
                        times_joined: trace.times_joined,
                        times_kicked: trace.times_kicked,
                        ips: trace.ips,
                        names: trace.names,
                    })
                } else {
                    self.state.failed_trace_info_parse_count =
                        self.state.failed_trace_info_parse_count.saturating_add(1);
                    self.state.last_trace_info_parse_error_payload_len = Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.debug_status_client_packet_id => {
                if let Some(status) = decode_debug_status_payload(&packet.payload) {
                    self.state.received_debug_status_client_count = self
                        .state
                        .received_debug_status_client_count
                        .saturating_add(1);
                    self.state.last_debug_status_reliable = Some(true);
                    self.state.last_debug_status_value = Some(status.value);
                    self.state.last_debug_status_last_client_snapshot =
                        Some(status.last_client_snapshot);
                    self.state.last_debug_status_snapshots_sent = Some(status.snapshots_sent);
                    self.state.last_debug_status_client_parse_error_payload_len = None;
                    Ok(ClientSessionEvent::DebugStatusReceived {
                        reliable: true,
                        value: status.value,
                        last_client_snapshot: status.last_client_snapshot,
                        snapshots_sent: status.snapshots_sent,
                    })
                } else {
                    self.state.failed_debug_status_client_parse_count = self
                        .state
                        .failed_debug_status_client_parse_count
                        .saturating_add(1);
                    self.state.last_debug_status_client_parse_error_payload_len =
                        Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.debug_status_client_unreliable_packet_id => {
                if let Some(status) = decode_debug_status_payload(&packet.payload) {
                    self.state.received_debug_status_client_unreliable_count = self
                        .state
                        .received_debug_status_client_unreliable_count
                        .saturating_add(1);
                    self.state.last_debug_status_reliable = Some(false);
                    self.state.last_debug_status_value = Some(status.value);
                    self.state.last_debug_status_last_client_snapshot =
                        Some(status.last_client_snapshot);
                    self.state.last_debug_status_snapshots_sent = Some(status.snapshots_sent);
                    self.state
                        .last_debug_status_client_unreliable_parse_error_payload_len = None;
                    Ok(ClientSessionEvent::DebugStatusReceived {
                        reliable: false,
                        value: status.value,
                        last_client_snapshot: status.last_client_snapshot,
                        snapshots_sent: status.snapshots_sent,
                    })
                } else {
                    self.state.failed_debug_status_client_unreliable_parse_count = self
                        .state
                        .failed_debug_status_client_unreliable_parse_count
                        .saturating_add(1);
                    self.state
                        .last_debug_status_client_unreliable_parse_error_payload_len =
                        Some(packet.payload.len());
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.player_disconnect_packet_id => {
                if let Some(player_id) = decode_player_disconnect_payload(&packet.payload) {
                    let cleared_local_player_sync = self.try_apply_player_disconnect(player_id);
                    Ok(ClientSessionEvent::PlayerDisconnected {
                        player_id,
                        cleared_local_player_sync,
                    })
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
            packet_id if Some(packet_id) == self.kick_string_packet_id => {
                let reason_text = try_read_typeio_string(&packet.payload);
                self.mark_kicked(reason_text.clone(), None, None);
                Ok(ClientSessionEvent::Kicked {
                    reason_text,
                    reason_ordinal: None,
                    duration_ms: None,
                })
            }
            packet_id if Some(packet_id) == self.kick_reason_packet_id => {
                let (reason_ordinal, duration_ms) = decode_kick_reason_payload(&packet.payload);
                let reason_text = reason_ordinal
                    .and_then(kick_reason_name_from_ordinal)
                    .map(str::to_string);
                self.mark_kicked(reason_text.clone(), reason_ordinal, duration_ms);
                Ok(ClientSessionEvent::Kicked {
                    reason_text,
                    reason_ordinal,
                    duration_ms,
                })
            }
            packet_id if Some(packet_id) == self.ping_packet_id => {
                let sent_at_ms = decode_ping_time_payload(&packet.payload);
                let response_queued = self.try_queue_ping_response(&packet.payload)?;
                Ok(ClientSessionEvent::Ping {
                    sent_at_ms,
                    response_queued,
                })
            }
            packet_id if Some(packet_id) == self.ping_response_packet_id => {
                let sent_at_ms = self.try_record_remote_ping_rtt(&packet.payload);
                Ok(ClientSessionEvent::PingResponse {
                    sent_at_ms,
                    round_trip_ms: self.last_remote_ping_rtt_ms,
                })
            }
            _ => {
                if let Some(snapshot) = ingest_inbound_packet(
                    &mut self.stats,
                    &mut self.state,
                    &self.registry,
                    packet.packet_id,
                    &packet.payload,
                ) {
                    if snapshot.method == HighFrequencyRemoteMethod::EntitySnapshot {
                        self.last_snapshot_at_ms = Some(self.clock_ms);
                        self.state.received_entity_snapshot_count =
                            self.state.received_entity_snapshot_count.saturating_add(1);
                        self.state.last_entity_snapshot_target_player_id = None;
                        self.state.last_entity_snapshot_used_projection_fallback = false;
                        self.state.last_entity_snapshot_local_player_sync_ambiguous = false;
                        self.state
                            .last_entity_snapshot_local_player_sync_match_count = 0;
                        match decode_entity_snapshot_envelope_header(snapshot.payload) {
                            Some(summary) => {
                                self.state.last_entity_snapshot_amount = Some(summary.amount);
                                self.state.last_entity_snapshot_body_len = Some(summary.body_len);
                                match validate_entity_snapshot_envelope(snapshot.payload, summary) {
                                    Ok(()) => {
                                        match parse_player_sync_rows_from_entity_snapshot(
                                            snapshot.payload,
                                        ) {
                                            Ok(player_rows) => {
                                                self.state.last_entity_snapshot_parse_error = None;
                                                self.state.prune_entity_snapshot_tombstones();
                                                self.state
                                                    .last_entity_snapshot_tombstone_skipped_ids_sample
                                                    .clear();
                                                self.apply_parseable_player_rows_from_entity_snapshot(
                                                    &player_rows,
                                                );
                                                let alpha_rows =
                                                    try_parse_alpha_sync_rows_from_entity_snapshot_prefix(
                                                        snapshot.payload,
                                                    );
                                                self.apply_parseable_alpha_rows_from_entity_snapshot(
                                                    &alpha_rows,
                                                );
                                                let mech_rows =
                                                    try_parse_mech_sync_rows_from_entity_snapshot_prefix(
                                                        snapshot.payload,
                                                    );
                                                self.apply_parseable_mech_rows_from_entity_snapshot(
                                                    &mech_rows,
                                                );
                                                let missile_rows =
                                                    try_parse_missile_sync_rows_from_entity_snapshot_prefix(
                                                        snapshot.payload,
                                                    );
                                                self.apply_parseable_missile_rows_from_entity_snapshot(
                                                    &missile_rows,
                                                );
                                                let (
                                                    payload_rows,
                                                    tether_payload_rows,
                                                    fire_rows,
                                                    puddle_rows,
                                                    weather_state_rows,
                                                    world_label_rows,
                                                ) = {
                                                    let content_header =
                                                        self.loaded_world_bundle().map(|bundle| {
                                                            bundle.content_header.as_slice()
                                                        });
                                                    (
                                                        try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                                                            snapshot.payload,
                                                            content_header,
                                                        ),
                                                        try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                                                            snapshot.payload,
                                                            content_header,
                                                        ),
                                                        try_parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(
                                                            snapshot.payload,
                                                            content_header,
                                                        ),
                                                        try_parse_puddle_sync_rows_from_entity_snapshot_prefix_with_content_header(
                                                            snapshot.payload,
                                                            content_header,
                                                        ),
                                                        try_parse_weather_state_sync_rows_from_entity_snapshot_prefix_with_content_header(
                                                            snapshot.payload,
                                                            content_header,
                                                        ),
                                                        try_parse_world_label_sync_rows_from_entity_snapshot_prefix_with_content_header(
                                                            snapshot.payload,
                                                            content_header,
                                                        ),
                                                    )
                                                };
                                                self.apply_parseable_payload_rows_from_entity_snapshot(
                                                    &payload_rows,
                                                );
                                                self.apply_parseable_building_tether_payload_rows_from_entity_snapshot(
                                                    &tether_payload_rows,
                                                );
                                                self.apply_parseable_fire_rows_from_entity_snapshot(
                                                    &fire_rows,
                                                );
                                                self.apply_parseable_puddle_rows_from_entity_snapshot(
                                                    &puddle_rows,
                                                );
                                                self.apply_parseable_weather_state_rows_from_entity_snapshot(
                                                    &weather_state_rows,
                                                );
                                                self.apply_parseable_world_label_rows_from_entity_snapshot(
                                                    &world_label_rows,
                                                );
                                                let sync_result = self
                                                    .try_apply_local_player_sync_from_entity_snapshot(
                                                        &player_rows,
                                                    );
                                                if sync_result.target_player_id.is_some() {
                                                    self.state
                                                        .entity_snapshot_with_local_target_count =
                                                        self.state
                                                            .entity_snapshot_with_local_target_count
                                                            .saturating_add(1);
                                                }
                                                self.state.last_entity_snapshot_target_player_id =
                                                    sync_result.target_player_id;
                                                self.state
                                                    .last_entity_snapshot_used_projection_fallback =
                                                    sync_result.used_projection_fallback;
                                                self.state
                                                    .last_entity_snapshot_local_player_sync_applied =
                                                    sync_result.applied;
                                                self.state
                                                    .last_entity_snapshot_local_player_sync_ambiguous =
                                                    sync_result.ambiguous;
                                                self.state
                                                    .last_entity_snapshot_local_player_sync_match_count =
                                                    sync_result.parseable_match_count;
                                                if sync_result.applied {
                                                    self.state
                                                        .applied_local_player_sync_from_entity_snapshot_count = self
                                                        .state
                                                        .applied_local_player_sync_from_entity_snapshot_count
                                                        .saturating_add(1);
                                                    if sync_result.used_projection_fallback {
                                                        self.state
                                                            .applied_local_player_sync_from_entity_snapshot_fallback_count = self
                                                            .state
                                                            .applied_local_player_sync_from_entity_snapshot_fallback_count
                                                            .saturating_add(1);
                                                    }
                                                } else if sync_result.target_player_id.is_some() {
                                                    if sync_result.ambiguous {
                                                        self.state
                                                            .ambiguous_local_player_sync_from_entity_snapshot_count = self
                                                            .state
                                                            .ambiguous_local_player_sync_from_entity_snapshot_count
                                                            .saturating_add(1);
                                                    }
                                                    self.state
                                                        .missed_local_player_sync_from_entity_snapshot_count = self
                                                        .state
                                                        .missed_local_player_sync_from_entity_snapshot_count
                                                        .saturating_add(1);
                                                }
                                            }
                                            Err(error) => {
                                                self.state.failed_entity_snapshot_parse_count =
                                                    self.state
                                                        .failed_entity_snapshot_parse_count
                                                        .saturating_add(1);
                                                self.state.last_entity_snapshot_parse_error =
                                                    Some(error.to_string());
                                                self.state
                                                    .last_entity_snapshot_local_player_sync_applied =
                                                    false;
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        self.state.failed_entity_snapshot_parse_count = self
                                            .state
                                            .failed_entity_snapshot_parse_count
                                            .saturating_add(1);
                                        self.state.last_entity_snapshot_parse_error = Some(error);
                                        self.state.last_entity_snapshot_local_player_sync_applied =
                                            false;
                                    }
                                }
                            }
                            None => {
                                self.state.failed_entity_snapshot_parse_count = self
                                    .state
                                    .failed_entity_snapshot_parse_count
                                    .saturating_add(1);
                                self.state.last_entity_snapshot_parse_error =
                                    Some("truncated_entity_snapshot_payload".to_string());
                                self.state.last_entity_snapshot_local_player_sync_applied = false;
                            }
                        }
                    } else if snapshot.method == HighFrequencyRemoteMethod::BlockSnapshot {
                        self.apply_block_snapshot_entries_from_loaded_world(snapshot.payload);
                    }
                    Ok(ClientSessionEvent::SnapshotReceived(snapshot.method))
                } else {
                    Ok(ClientSessionEvent::IgnoredPacket {
                        packet_id: packet.packet_id,
                        remote: self.known_remote_packets.get(&packet.packet_id).cloned(),
                    })
                }
            }
        }
    }

    fn maybe_defer_inbound_packet_while_loading(
        &mut self,
        packet_id: u8,
        payload: &[u8],
    ) -> Option<ClientSessionEvent> {
        if !self.loading_world_data || self.state.client_loaded {
            return None;
        }
        if packet_id == STREAM_BEGIN_PACKET_ID
            || packet_id == STREAM_CHUNK_PACKET_ID
            || packet_id == self.world_data_begin_packet_id
        {
            return None;
        }

        match self.known_remote_packet_priorities.get(&packet_id).copied() {
            Some(DeferredInboundPriority::Normal) => {
                self.deferred_inbound_packets
                    .push_back(DeferredInboundPacket {
                        packet_id,
                        payload: payload.to_vec(),
                    });
                self.state.deferred_inbound_packet_count =
                    self.state.deferred_inbound_packet_count.saturating_add(1);
                self.state.last_deferred_packet_id = Some(packet_id);
                self.state.last_deferred_packet_method = self
                    .known_remote_packets
                    .get(&packet_id)
                    .map(|meta| meta.method.clone());
                Some(ClientSessionEvent::DeferredPacketWhileLoading {
                    packet_id,
                    remote: self.known_remote_packets.get(&packet_id).cloned(),
                })
            }
            Some(DeferredInboundPriority::Low) => {
                self.state.dropped_loading_low_priority_packet_count = self
                    .state
                    .dropped_loading_low_priority_packet_count
                    .saturating_add(1);
                self.state.last_dropped_loading_packet_id = Some(packet_id);
                self.state.last_dropped_loading_packet_method = self
                    .known_remote_packets
                    .get(&packet_id)
                    .map(|meta| meta.method.clone());
                Some(ClientSessionEvent::IgnoredPacket {
                    packet_id,
                    remote: self.known_remote_packets.get(&packet_id).cloned(),
                })
            }
            Some(DeferredInboundPriority::High) | None => None,
        }
    }

    fn replay_deferred_loading_packets(&mut self) -> Result<(), ClientSessionError> {
        while let Some(packet) = self.deferred_inbound_packets.pop_front() {
            let event = self.process_inbound_packet(&[], packet.packet_id, &packet.payload)?;
            self.state.replayed_inbound_packet_count =
                self.state.replayed_inbound_packet_count.saturating_add(1);
            self.state.last_replayed_packet_id = Some(packet.packet_id);
            self.state.last_replayed_packet_method = self
                .known_remote_packets
                .get(&packet.packet_id)
                .map(|meta| meta.method.clone());
            self.replayed_loading_events.push_back(event);
        }
        Ok(())
    }

    fn mark_client_loaded(&mut self) -> Result<(), ClientSessionError> {
        if self.state.client_loaded {
            return Ok(());
        }
        self.state.client_loaded = true;
        self.loading_world_data = false;
        self.replay_deferred_loading_packets()
    }

    fn should_send_keepalive(&self, now_ms: u64) -> bool {
        match self.last_keepalive_at_ms {
            Some(last) => now_ms.saturating_sub(last) >= self.timing.keepalive_interval_ms,
            None => true,
        }
    }

    fn should_send_remote_ping(&self, now_ms: u64) -> bool {
        match self.last_remote_ping_at_ms {
            Some(last) => now_ms.saturating_sub(last) >= self.timing.keepalive_interval_ms,
            None => true,
        }
    }

    fn should_send_client_snapshot(&self, now_ms: u64) -> bool {
        if !self.state.ready_to_enter_world || !self.state.connect_confirm_sent {
            return false;
        }
        match self.last_client_snapshot_at_ms {
            Some(last) => now_ms.saturating_sub(last) >= self.timing.client_snapshot_interval_ms,
            None => true,
        }
    }

    fn record_inbound_activity(&mut self, now_ms: u64) {
        self.last_inbound_at_ms = Some(now_ms);
        self.state.last_inbound_at_ms = Some(now_ms);
        self.state.connection_timed_out = false;
    }

    fn record_ready_inbound_liveness_activity(&mut self, now_ms: u64) {
        if !self.ready_for_interaction() {
            return;
        }
        self.last_ready_inbound_liveness_at_ms = Some(now_ms);
        self.state.last_ready_inbound_liveness_anchor_at_ms = Some(now_ms);
        self.state.ready_inbound_liveness_anchor_count = self
            .state
            .ready_inbound_liveness_anchor_count
            .saturating_add(1);
    }

    fn record_outbound_activity(&mut self, now_ms: u64) {
        self.state.last_outbound_at_ms = Some(now_ms);
    }

    fn ready_for_interaction(&self) -> bool {
        self.state.ready_to_enter_world && self.state.connect_confirm_sent
    }

    fn mark_kicked(
        &mut self,
        reason_text: Option<String>,
        reason_ordinal: Option<i32>,
        duration_ms: Option<u64>,
    ) {
        self.kicked = true;
        self.pending_packets.clear();
        self.deferred_inbound_packets.clear();
        self.replayed_loading_events.clear();
        self.loading_world_data = false;
        self.state.client_loaded = false;
        let (hint_category, hint_text) =
            kick_reason_hint_from(reason_text.as_deref(), reason_ordinal)
                .map(|(category, text)| (Some(category), Some(text)))
                .unwrap_or((None, None));
        self.last_kick_reason_text = reason_text;
        self.last_kick_reason_ordinal = reason_ordinal;
        self.last_kick_duration_ms = duration_ms;
        self.last_kick_hint_category = hint_category;
        self.last_kick_hint_text = hint_text;
        self.timed_out = false;
        self.state.connection_timed_out = false;
    }

    fn try_queue_ping_response(&mut self, payload: &[u8]) -> Result<bool, ClientSessionError> {
        let Some(packet_id) = self.ping_response_packet_id else {
            return Ok(false);
        };
        let Some(time_ms) = decode_ping_time_payload(payload) else {
            return Ok(false);
        };
        let bytes = encode_packet(packet_id, &encode_ping_time_payload(time_ms), false)?;
        self.pending_packets.push_back(PendingClientPacket {
            packet_id,
            transport: ClientPacketTransport::Tcp,
            bytes,
        });
        Ok(true)
    }

    fn try_record_remote_ping_rtt(&mut self, payload: &[u8]) -> Option<u64> {
        let Some(sent_at_ms) = decode_ping_time_payload(payload) else {
            return None;
        };
        self.last_remote_ping_rtt_ms = Some(self.clock_ms.saturating_sub(sent_at_ms));
        Some(sent_at_ms)
    }

    fn remove_snapshot_plan(&mut self, x: i32, y: i32) -> bool {
        let Some(plans) = self.snapshot_input.plans.as_mut() else {
            return false;
        };
        let original_len = plans.len();
        plans.retain(|plan| plan.tile != (x, y));
        plans.len() != original_len
    }

    fn remove_snapshot_plan_by_tile_pos(&mut self, tile_pos: i32) -> bool {
        let (x, y) = unpack_point2(tile_pos);
        self.remove_snapshot_plan(i32::from(x), i32::from(y))
    }

    fn sync_builder_queue_projection_from_snapshot_input(&mut self) {
        let entries = self
            .snapshot_input
            .plans
            .as_deref()
            .into_iter()
            .flat_map(|plans| plans.iter())
            .map(|plan| BuilderQueueEntryObservation {
                x: plan.tile.0,
                y: plan.tile.1,
                breaking: plan.breaking,
                block_id: plan.block_id,
                rotation: plan.rotation,
            })
            .collect::<Vec<_>>();
        self.state
            .builder_queue_projection
            .sync_local_queue_entries(entries);
        self.sync_snapshot_building_from_builder_queue_projection();
    }

    fn sync_snapshot_building_from_builder_queue_projection(&mut self) {
        self.snapshot_input.building = !self
            .state
            .builder_queue_projection
            .active_by_tile
            .is_empty();
    }

    fn apply_snapshot_input_from_bootstrap(&mut self, bootstrap: &LoadedWorldBootstrap) {
        let unit_id = bootstrap_player_unit_id(bootstrap);
        let position = (
            sanitize_bootstrap_coord(bootstrap.player_x_bits),
            sanitize_bootstrap_coord(bootstrap.player_y_bits),
        );

        self.snapshot_input.unit_id = unit_id;
        self.snapshot_input.dead = bootstrap.player_unit_kind == 0;
        self.snapshot_input.position = Some(position);
        self.snapshot_input.view_center = Some(position);
        self.snapshot_input.selected_block_id = i16::try_from(bootstrap.selected_block_id)
            .ok()
            .filter(|id| *id >= 0);
        self.snapshot_input.selected_rotation =
            i32::try_from(bootstrap.selected_rotation).unwrap_or(0);
        if let Some(player_id) = self.state.world_player_id {
            self.state
                .entity_table_projection
                .upsert_bootstrap_local_player(
                    player_id,
                    bootstrap.player_unit_kind,
                    bootstrap.player_unit_value,
                    position.0.to_bits(),
                    position.1.to_bits(),
                    false,
                );
        }
    }

    fn apply_world_baseline_from_bundle(&mut self, world_bundle: &WorldBundle) {
        for center in &world_bundle.world.building_centers {
            let Ok(x) = i32::try_from(center.x) else {
                continue;
            };
            let Ok(y) = i32::try_from(center.y) else {
                continue;
            };
            let build_pos = pack_point2(x, y);
            let block_id = i16::from_be_bytes(center.block_id.to_be_bytes());
            let base = &center.building.base;
            self.state.building_table_projection.seed_world_baseline(
                build_pos,
                block_id,
                base.rotation,
                base.team_id,
                base.save_version,
                base.module_bitmask,
                base.time_scale_bits,
                base.time_scale_duration_bits,
                base.last_disabler_pos,
                base.legacy_consume_connected,
                base.health_bits,
                base.enabled,
                base.efficiency,
                base.optional_efficiency,
                base.visible_flags,
            );
        }
    }

    fn resolve_local_player_sync_target_id(&self) -> (Option<i32>, bool) {
        if let Some(player_id) = self.state.world_player_id {
            (Some(player_id), false)
        } else {
            let fallback = self.state.entity_table_projection.local_player_entity_id;
            (fallback, fallback.is_some())
        }
    }

    fn try_apply_local_player_sync_from_entity_snapshot(
        &mut self,
        player_rows: &[EntityPlayerSyncRow],
    ) -> LocalPlayerSyncApplicationResult {
        let (target_player_id, used_projection_fallback) =
            self.resolve_local_player_sync_target_id();
        let Some(player_id) = target_player_id else {
            return LocalPlayerSyncApplicationResult::notarget();
        };
        let parsed = try_parse_local_player_sync_from_entity_snapshot(player_rows, player_id);
        let Some(sync) = parsed.sync else {
            return LocalPlayerSyncApplicationResult {
                applied: false,
                target_player_id: Some(player_id),
                used_projection_fallback,
                ambiguous: parsed.ambiguous,
                parseable_match_count: parsed.parseable_match_count,
            };
        };
        if self
            .state
            .entity_snapshot_tombstone_blocks_upsert(player_id)
        {
            self.state.record_entity_snapshot_tombstone_skip(player_id);
            return LocalPlayerSyncApplicationResult {
                applied: false,
                target_player_id: Some(player_id),
                used_projection_fallback,
                ambiguous: false,
                parseable_match_count: parsed.parseable_match_count,
            };
        }
        let x = sync.x();
        let y = sync.y();

        if used_projection_fallback && self.state.world_player_id.is_none() {
            self.state.world_player_id = Some(player_id);
        }
        self.state.world_player_unit_kind = Some(sync.unit_kind);
        self.state.world_player_unit_value = Some(sync.unit_value);
        self.state.world_player_x_bits = Some(sync.x_bits);
        self.state.world_player_y_bits = Some(sync.y_bits);
        self.state.entity_table_projection.upsert_local_player(
            player_id,
            sync.unit_kind,
            sync.unit_value,
            sync.x_bits,
            sync.y_bits,
            false,
            self.state.received_entity_snapshot_count,
        );
        self.snapshot_input.unit_id = sync.snapshot_unit_id();
        self.snapshot_input.dead = sync.is_dead();
        self.snapshot_input.position = Some((x, y));
        self.snapshot_input.view_center = Some((x, y));
        LocalPlayerSyncApplicationResult {
            applied: true,
            target_player_id: Some(player_id),
            used_projection_fallback,
            ambiguous: false,
            parseable_match_count: parsed.parseable_match_count,
        }
    }

    fn apply_parseable_player_rows_from_entity_snapshot(
        &mut self,
        player_rows: &[EntityPlayerSyncRow],
    ) {
        let (local_player_id, _) = self.resolve_local_player_sync_target_id();
        for row in player_rows {
            if Some(row.entity_id) == local_player_id {
                continue;
            }
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_player_entity(
                row.entity_id,
                false,
                row.sync.unit_kind,
                row.sync.unit_value,
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_alpha_rows_from_entity_snapshot(
        &mut self,
        alpha_rows: &[EntityAlphaSyncRow],
    ) {
        for row in alpha_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                2,
                u32::try_from(row.entity_id).unwrap_or_default(),
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_mech_rows_from_entity_snapshot(&mut self, mech_rows: &[EntityMechSyncRow]) {
        for row in mech_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                2,
                u32::try_from(row.entity_id).unwrap_or_default(),
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_missile_rows_from_entity_snapshot(
        &mut self,
        missile_rows: &[EntityMissileSyncRow],
    ) {
        for row in missile_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                2,
                u32::try_from(row.entity_id).unwrap_or_default(),
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_payload_rows_from_entity_snapshot(
        &mut self,
        payload_rows: &[EntityPayloadSyncRow],
    ) {
        for row in payload_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                2,
                u32::try_from(row.entity_id).unwrap_or_default(),
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_building_tether_payload_rows_from_entity_snapshot(
        &mut self,
        tether_payload_rows: &[EntityBuildingTetherPayloadSyncRow],
    ) {
        for row in tether_payload_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                2,
                u32::try_from(row.entity_id).unwrap_or_default(),
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_fire_rows_from_entity_snapshot(&mut self, fire_rows: &[EntityFireSyncRow]) {
        for row in fire_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                0,
                0,
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_puddle_rows_from_entity_snapshot(
        &mut self,
        puddle_rows: &[EntityPuddleSyncRow],
    ) {
        for row in puddle_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                0,
                0,
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_weather_state_rows_from_entity_snapshot(
        &mut self,
        weather_state_rows: &[EntityWeatherStateSyncRow],
    ) {
        for row in weather_state_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                0,
                0,
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_parseable_world_label_rows_from_entity_snapshot(
        &mut self,
        world_label_rows: &[EntityWorldLabelSyncRow],
    ) {
        for row in world_label_rows {
            if self
                .state
                .entity_snapshot_tombstone_blocks_upsert(row.entity_id)
            {
                self.state
                    .record_entity_snapshot_tombstone_skip(row.entity_id);
                continue;
            }
            self.state.entity_table_projection.upsert_entity(
                row.entity_id,
                row.class_id,
                false,
                0,
                0,
                row.sync.x_bits,
                row.sync.y_bits,
                false,
                self.state.received_entity_snapshot_count,
            );
        }
    }

    fn apply_block_snapshot_entries_from_loaded_world(&mut self, payload: &[u8]) {
        if self.loaded_world_bundle().is_none() {
            return;
        }

        match self.collect_block_snapshot_entries_from_loaded_world(payload) {
            Ok(LoadedWorldBlockSnapshotEntryCollection::Complete(entries)) => {
                self.state
                    .last_loaded_world_block_snapshot_extra_entry_count = entries.len();
                self.state
                    .last_loaded_world_block_snapshot_extra_entry_parse_error = None;
                self.state
                    .applied_loaded_world_block_snapshot_extra_entry_count = self
                    .state
                    .applied_loaded_world_block_snapshot_extra_entry_count
                    .saturating_add(entries.len() as u64);
                self.apply_block_snapshot_entries_from_loaded_world_entries(entries);
            }
            Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error }) => {
                self.state
                    .last_loaded_world_block_snapshot_extra_entry_count = entries.len();
                self.state
                    .applied_loaded_world_block_snapshot_extra_entry_count = self
                    .state
                    .applied_loaded_world_block_snapshot_extra_entry_count
                    .saturating_add(entries.len() as u64);
                self.state
                    .failed_loaded_world_block_snapshot_extra_entry_parse_count = self
                    .state
                    .failed_loaded_world_block_snapshot_extra_entry_parse_count
                    .saturating_add(1);
                self.state
                    .last_loaded_world_block_snapshot_extra_entry_parse_error = Some(error);
                self.apply_block_snapshot_entries_from_loaded_world_entries(entries);
            }
            Err(error) => {
                self.state
                    .last_loaded_world_block_snapshot_extra_entry_count = 0;
                self.state
                    .failed_loaded_world_block_snapshot_extra_entry_parse_count = self
                    .state
                    .failed_loaded_world_block_snapshot_extra_entry_parse_count
                    .saturating_add(1);
                self.state
                    .last_loaded_world_block_snapshot_extra_entry_parse_error = Some(error);
            }
        }
    }

    fn collect_block_snapshot_entries_from_loaded_world(
        &self,
        payload: &[u8],
    ) -> Result<LoadedWorldBlockSnapshotEntryCollection, String> {
        let loaded_world = self
            .loaded_world_state()
            .ok_or_else(|| "loaded_world_state_unavailable".to_string())?;
        let mut cursor = 0usize;
        let amount = read_i16(payload, &mut cursor)
            .ok_or_else(|| "truncated_block_snapshot_payload".to_string())?;
        if amount < 0 {
            return Err(format!("negative_block_snapshot_amount:{amount}"));
        }
        let data = read_typeio_bytes_at(payload, &mut cursor)
            .ok_or_else(|| "truncated_block_snapshot_payload".to_string())?;
        if cursor != payload.len() {
            return Err(format!(
                "block_snapshot_payload_trailing_bytes:{cursor}/{}",
                payload.len()
            ));
        }

        let entry_count = usize::try_from(amount)
            .map_err(|_| format!("negative_block_snapshot_amount:{amount}"))?;
        let mut data_cursor = 0usize;
        let mut entries = Vec::with_capacity(entry_count);

        for index in 0..entry_count {
            let build_pos = match read_i32(&data, &mut data_cursor) {
                Some(value) => value,
                None => {
                    let error =
                        format!("loaded_world_block_snapshot_entry_{index}_truncated_header");
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            let block_id = match read_i16(&data, &mut data_cursor) {
                Some(value) => value,
                None => {
                    let error =
                        format!("loaded_world_block_snapshot_entry_{index}_truncated_header");
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            let (tile_x, tile_y) = unpack_point2(build_pos);
            let tile_x = match usize::try_from(tile_x) {
                Ok(value) => value,
                Err(_) => {
                    let error = format!(
                        "loaded_world_block_snapshot_entry_{index}_invalid_tile_x:{tile_x}"
                    );
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            let tile_y = match usize::try_from(tile_y) {
                Ok(value) => value,
                Err(_) => {
                    let error = format!(
                        "loaded_world_block_snapshot_entry_{index}_invalid_tile_y:{tile_y}"
                    );
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            let center = loaded_world
                .graph()
                .building_center_at(tile_x, tile_y)
                .ok_or_else(|| {
                    format!("loaded_world_block_snapshot_entry_{index}_missing_center:{build_pos}")
                });
            let center = match center {
                Ok(value) => value,
                Err(error) => {
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            if center.block_id != block_id as u16 {
                let error = format!(
                    "loaded_world_block_snapshot_entry_{index}_block_id_mismatch:{}/{}",
                    center.block_id, block_id
                );
                if entries.is_empty() {
                    return Err(error);
                }
                return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
            }
            let block_content_id = match usize::try_from(block_id) {
                Ok(value) => value,
                Err(_) => {
                    let error = format!(
                        "loaded_world_block_snapshot_entry_{index}_negative_block_id:{block_id}"
                    );
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            let block_name = loaded_world.content_name(BLOCK_CONTENT_TYPE, block_content_id);
            let parsed = parse_building_sync_bytes(
                loaded_world.content_headers(),
                block_name,
                center.building.revision,
                &data[data_cursor..],
            );
            let (building, consumed) = match parsed {
                Ok(value) => value,
                Err(error) => {
                    let error = format!("loaded_world_block_snapshot_entry_{index}_parse:{error}");
                    if entries.is_empty() {
                        return Err(error);
                    }
                    return Ok(LoadedWorldBlockSnapshotEntryCollection::Partial { entries, error });
                }
            };
            data_cursor = data_cursor.saturating_add(consumed);

            let entry = BlockSnapshotExtraEntrySummary {
                build_pos,
                block_id,
                health_bits: Some(building.base.health_bits),
                rotation: Some(building.base.rotation),
                team_id: Some(building.base.team_id),
                io_version: building.base.save_version,
                enabled: building.base.enabled,
                module_bitmask: building.base.module_bitmask,
                time_scale_bits: building.base.time_scale_bits,
                time_scale_duration_bits: building.base.time_scale_duration_bits,
                last_disabler_pos: building.base.last_disabler_pos,
                legacy_consume_connected: building.base.legacy_consume_connected,
                efficiency: building.base.efficiency,
                optional_efficiency: building.base.optional_efficiency,
                visible_flags: building.base.visible_flags,
            };
            entries.push(entry);
        }

        if data_cursor != data.len() {
            return Err(format!(
                "loaded_world_block_snapshot_extra_trailing_bytes:{data_cursor}/{}",
                data.len()
            ));
        }

        Ok(LoadedWorldBlockSnapshotEntryCollection::Complete(entries))
    }

    fn apply_block_snapshot_entries_from_loaded_world_entries(
        &mut self,
        entries: Vec<BlockSnapshotExtraEntrySummary>,
    ) {
        for entry in entries {
            self.state
                .building_table_projection
                .apply_block_snapshot_head(
                    entry.build_pos,
                    entry.block_id,
                    entry.rotation,
                    entry.team_id,
                    entry.io_version,
                    entry.module_bitmask,
                    entry.time_scale_bits,
                    entry.time_scale_duration_bits,
                    entry.last_disabler_pos,
                    entry.legacy_consume_connected,
                    entry.health_bits,
                    entry.enabled,
                    entry.efficiency,
                    entry.optional_efficiency,
                    entry.visible_flags,
                );
        }
    }

    fn try_apply_local_player_spawn_from_packet(
        &mut self,
        payload: &[u8],
    ) -> Option<(i32, f32, f32)> {
        let (tile_pos, player_id) = decode_player_spawn_payload(payload)?;
        if Some(player_id) != self.state.world_player_id {
            return None;
        }

        let (tile_x, tile_y) = unpack_point2(tile_pos);
        let x = tile_x as f32 * 8.0;
        let y = tile_y as f32 * 8.0;
        self.state.world_player_x_bits = Some(x.to_bits());
        self.state.world_player_y_bits = Some(y.to_bits());
        self.state
            .entity_table_projection
            .update_local_player_position(player_id, x.to_bits(), y.to_bits(), false);
        self.snapshot_input.unit_id = None;
        self.snapshot_input.dead = false;
        self.snapshot_input.position = Some((x, y));
        self.snapshot_input.view_center = Some((x, y));
        Some((player_id, x, y))
    }

    fn try_apply_player_disconnect(&mut self, player_id: i32) -> bool {
        self.state.record_entity_snapshot_tombstone(player_id);
        self.state.entity_table_projection.remove_entity(player_id);
        if Some(player_id) != self.state.world_player_id {
            return false;
        }

        self.state.world_player_unit_kind = None;
        self.state.world_player_unit_value = None;
        self.state.world_player_x_bits = None;
        self.state.world_player_y_bits = None;
        self.snapshot_input.unit_id = None;
        self.snapshot_input.dead = true;
        self.snapshot_input.position = None;
        self.snapshot_input.view_center = None;
        true
    }

    fn quiet_reset_for_reconnect(&mut self) {
        self.pending_packets.clear();
        self.deferred_inbound_packets.clear();
        self.replayed_loading_events.clear();
        self.pending_world_stream = None;
        self.loading_world_data = false;
        self.loaded_world_bundle = None;
        self.last_inbound_at_ms = None;
        self.last_ready_inbound_liveness_at_ms = None;
        self.last_snapshot_at_ms = None;
        self.last_keepalive_at_ms = None;
        self.last_client_snapshot_at_ms = None;
        self.last_remote_ping_at_ms = None;
        self.last_remote_ping_rtt_ms = None;
        self.kicked = false;
        self.last_kick_reason_text = None;
        self.last_kick_reason_ordinal = None;
        self.last_kick_duration_ms = None;
        self.last_kick_hint_category = None;
        self.last_kick_hint_text = None;
        self.next_client_snapshot_id = 1;
        self.timed_out = false;
        self.snapshot_input = ClientSnapshotInputState::default();
        self.state = SessionState::default();
        self.stats = NetLoopStats::default();
    }

    fn begin_world_data_reload(&mut self) {
        self.pending_world_stream = None;
        self.loaded_world_bundle = None;
        self.pending_packets.clear();
        self.deferred_inbound_packets.clear();
        self.replayed_loading_events.clear();
        self.loading_world_data = true;
        self.timed_out = false;
        self.state.connection_timed_out = false;
        self.state.client_loaded = false;
        self.state.connect_confirm_sent = false;
        self.state.last_connect_confirm_at_ms = None;
        self.state.last_ready_inbound_liveness_anchor_at_ms = None;
        self.state.ready_inbound_liveness_anchor_count = 0;
        self.state.bootstrap_stream_id = None;
        self.state.world_stream_expected_len = 0;
        self.state.world_stream_received_len = 0;
        self.state.world_stream_loaded = false;
        self.state.world_stream_compressed_len = 0;
        self.state.world_stream_inflated_len = 0;
        self.state.world_map_width = 0;
        self.state.world_map_height = 0;
        self.state.world_player_id = None;
        self.state.world_player_unit_kind = None;
        self.state.world_player_unit_value = None;
        self.state.world_player_x_bits = None;
        self.state.world_player_y_bits = None;
        self.state.last_camera_x_bits = None;
        self.state.last_camera_y_bits = None;
        self.state.world_display_title = None;
        self.state.ready_to_enter_world = false;
        self.state.deferred_inbound_packet_count = 0;
        self.state.replayed_inbound_packet_count = 0;
        self.state.dropped_loading_low_priority_packet_count = 0;
        self.state.dropped_loading_deferred_overflow_count = 0;
        self.state.last_deferred_packet_id = None;
        self.state.last_deferred_packet_method = None;
        self.state.last_replayed_packet_id = None;
        self.state.last_replayed_packet_method = None;
        self.state.last_dropped_loading_packet_id = None;
        self.state.last_dropped_loading_packet_method = None;
        self.state.last_dropped_loading_deferred_overflow_packet_id = None;
        self.state
            .last_dropped_loading_deferred_overflow_packet_method = None;
        self.state.last_client_snapshot_at_ms = None;
        self.state.last_sent_client_snapshot_id = None;
        self.state.received_snapshot_count = 0;
        self.state.last_snapshot_packet_id = None;
        self.state.last_snapshot_method = None;
        self.state.last_snapshot_payload_len = 0;
        self.state.applied_state_snapshot_count = 0;
        self.state.last_state_snapshot = None;
        self.state.last_state_snapshot_core_data = None;
        self.state.last_good_state_snapshot_core_data = None;
        self.state
            .last_state_snapshot_core_data_duplicate_team_count = 0;
        self.state
            .last_state_snapshot_core_data_duplicate_item_count = 0;
        self.state
            .state_snapshot_core_data_duplicate_team_count_total = 0;
        self.state
            .state_snapshot_core_data_duplicate_item_count_total = 0;
        self.state.authoritative_state_mirror = None;
        self.state.state_snapshot_authority_projection = None;
        self.state.state_snapshot_business_projection = None;
        self.state.failed_state_snapshot_core_data_parse_count = 0;
        self.state.last_state_snapshot_core_data_parse_error = None;
        self.state
            .last_state_snapshot_core_data_parse_error_payload_len = None;
        self.state.failed_state_snapshot_parse_count = 0;
        self.state.last_state_snapshot_parse_error = None;
        self.state.last_state_snapshot_parse_error_payload_len = None;
        self.state.seen_state_snapshot = false;
        self.state.seen_entity_snapshot = false;
        self.state.received_entity_snapshot_count = 0;
        self.state.last_entity_snapshot_amount = None;
        self.state.last_entity_snapshot_body_len = None;
        self.state.entity_snapshot_with_local_target_count = 0;
        self.state
            .missed_local_player_sync_from_entity_snapshot_count = 0;
        self.state
            .applied_local_player_sync_from_entity_snapshot_count = 0;
        self.state
            .applied_local_player_sync_from_entity_snapshot_fallback_count = 0;
        self.state
            .ambiguous_local_player_sync_from_entity_snapshot_count = 0;
        self.state.last_entity_snapshot_target_player_id = None;
        self.state.last_entity_snapshot_used_projection_fallback = false;
        self.state.last_entity_snapshot_local_player_sync_applied = false;
        self.state.last_entity_snapshot_local_player_sync_ambiguous = false;
        self.state
            .last_entity_snapshot_local_player_sync_match_count = 0;
        self.state.failed_entity_snapshot_parse_count = 0;
        self.state.last_entity_snapshot_parse_error = None;
        self.state.clear_entity_snapshot_tombstones();
        self.state.entity_snapshot_tombstone_skip_count = 0;
        self.state.seen_block_snapshot = false;
        self.state.received_block_snapshot_count = 0;
        self.state.last_block_snapshot_payload_len = None;
        self.state.applied_block_snapshot_count = 0;
        self.state.last_block_snapshot = None;
        self.state.block_snapshot_head_projection = None;
        self.state
            .applied_loaded_world_block_snapshot_extra_entry_count = 0;
        self.state
            .last_loaded_world_block_snapshot_extra_entry_count = 0;
        self.state
            .failed_loaded_world_block_snapshot_extra_entry_parse_count = 0;
        self.state
            .last_loaded_world_block_snapshot_extra_entry_parse_error = None;
        self.state.failed_block_snapshot_parse_count = 0;
        self.state.last_block_snapshot_parse_error = None;
        self.state.last_block_snapshot_parse_error_payload_len = None;
        self.state.seen_hidden_snapshot = false;
        self.state.received_hidden_snapshot_count = 0;
        self.state.last_hidden_snapshot_payload_len = None;
        self.state.applied_hidden_snapshot_count = 0;
        self.state.last_hidden_snapshot = None;
        self.state.hidden_snapshot_ids.clear();
        self.state.hidden_snapshot_delta_projection = None;
        self.state.hidden_lifecycle_remove_count = 0;
        self.state.last_hidden_lifecycle_removed_ids_sample.clear();
        self.state.failed_hidden_snapshot_parse_count = 0;
        self.state.last_hidden_snapshot_parse_error = None;
        self.state.last_hidden_snapshot_parse_error_payload_len = None;
        self.state.entity_table_projection.clear_for_world_reload();
        self.state.rules_projection = Default::default();
        self.state.objectives_projection = Default::default();
        self.state.last_effect_business_projection = None;
        self.state.last_effect_business_path = None;
        self.state.tile_config_projection.clear_for_world_reload();
        self.state
            .building_table_projection
            .clear_for_world_reload();
        self.state.builder_queue_projection.clear_for_world_reload();
        self.last_client_snapshot_at_ms = None;
        self.last_ready_inbound_liveness_at_ms = None;
        self.last_snapshot_at_ms = None;
        self.last_remote_ping_at_ms = None;
        self.last_remote_ping_rtt_ms = None;
        self.kicked = false;
        self.last_kick_reason_text = None;
        self.last_kick_reason_ordinal = None;
        self.last_kick_duration_ms = None;
        self.last_kick_hint_category = None;
        self.last_kick_hint_text = None;
        self.snapshot_input.unit_id = None;
        self.snapshot_input.dead = true;
        self.snapshot_input.position = None;
        self.snapshot_input.rotation = 0.0;
        self.snapshot_input.base_rotation = 0.0;
        self.snapshot_input.velocity = (0.0, 0.0);
        self.snapshot_input.building = false;
        self.snapshot_input.plans = None;
        self.snapshot_input.view_center = None;
    }

    fn queue_outbound_packet(
        &mut self,
        packet_id: u8,
        transport: ClientPacketTransport,
        payload: Vec<u8>,
    ) -> Result<(), ClientSessionError> {
        let bytes = encode_packet(packet_id, &payload, false)?;
        self.pending_packets.push_back(PendingClientPacket {
            packet_id,
            transport,
            bytes,
        });
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientSessionEvent {
    WorldStreamStarted {
        stream_id: i32,
        total_bytes: usize,
    },
    WorldStreamChunk {
        stream_id: i32,
        received_bytes: usize,
        total_bytes: usize,
    },
    WorldStreamReady {
        stream_id: i32,
        map_width: usize,
        map_height: usize,
        player_id: i32,
        ready_to_enter_world: bool,
    },
    WorldDataBegin,
    ConnectRedirectRequested {
        ip: String,
        port: i32,
    },
    PlayerSpawned {
        player_id: i32,
        x: f32,
        y: f32,
    },
    PlayerPositionUpdated {
        x: f32,
        y: f32,
    },
    CameraPositionUpdated {
        x: f32,
        y: f32,
    },
    SoundRequested {
        sound_id: Option<i16>,
        volume: f32,
        pitch: f32,
        pan: f32,
    },
    SoundAtRequested {
        sound_id: Option<i16>,
        x: f32,
        y: f32,
        volume: f32,
        pitch: f32,
    },
    TakeItems {
        projection: TakeItemsProjection,
    },
    TransferItemTo {
        projection: TransferItemToProjection,
    },
    TransferItemToUnit {
        projection: TransferItemToUnitProjection,
    },
    PayloadDropped {
        projection: PayloadDroppedProjection,
    },
    PickedBuildPayload {
        projection: PickedBuildPayloadProjection,
    },
    PickedUnitPayload {
        projection: PickedUnitPayloadProjection,
    },
    UnitEnteredPayload {
        projection: UnitEnteredPayloadProjection,
        removed_entity_projection: bool,
    },
    UnitDespawned {
        unit: Option<UnitRefProjection>,
        removed_entity_projection: bool,
    },
    EffectRequested {
        effect_id: Option<i16>,
        x: f32,
        y: f32,
        rotation: f32,
        color_rgba: u32,
        data_object: Option<TypeIoObject>,
    },
    EffectReliableRequested {
        effect_id: Option<i16>,
        x: f32,
        y: f32,
        rotation: f32,
        color_rgba: u32,
    },
    TraceInfoReceived {
        player_id: Option<i32>,
        ip: Option<String>,
        uuid: Option<String>,
        locale: Option<String>,
        modded: bool,
        mobile: bool,
        times_joined: i32,
        times_kicked: i32,
        ips: Vec<String>,
        names: Vec<String>,
    },
    DebugStatusReceived {
        reliable: bool,
        value: i32,
        last_client_snapshot: i32,
        snapshots_sent: i32,
    },
    RulesUpdatedRaw {
        json_data: String,
    },
    ObjectivesUpdatedRaw {
        json_data: String,
    },
    SetRuleApplied {
        rule: String,
        json_data: String,
    },
    ObjectivesCleared,
    ObjectiveCompleted {
        index: i32,
    },
    PlayerDisconnected {
        player_id: i32,
        cleared_local_player_sync: bool,
    },
    ServerMessage {
        message: String,
    },
    ChatMessage {
        message: String,
        unformatted: Option<String>,
        sender_entity_id: Option<i32>,
    },
    ClientPacketReliable {
        packet_type: String,
        contents: String,
    },
    ClientPacketUnreliable {
        packet_type: String,
        contents: String,
    },
    ClientBinaryPacketReliable {
        packet_type: String,
        contents: Vec<u8>,
    },
    ClientBinaryPacketUnreliable {
        packet_type: String,
        contents: Vec<u8>,
    },
    ClientLogicDataReliable {
        channel: String,
        value: TypeIoObject,
    },
    ClientLogicDataUnreliable {
        channel: String,
        value: TypeIoObject,
    },
    SetHudText {
        message: Option<String>,
    },
    SetHudTextReliable {
        message: Option<String>,
    },
    HideHudText,
    Announce {
        message: Option<String>,
    },
    SetFlag {
        flag: Option<String>,
        add: bool,
    },
    GameOver {
        winner_team_id: u8,
    },
    UpdateGameOver {
        winner_team_id: u8,
    },
    SectorCapture,
    Researched {
        content_type: u8,
        content_id: i16,
    },
    WorldLabel {
        reliable: bool,
        label_id: Option<i32>,
        message: Option<String>,
        duration: f32,
        world_x: f32,
        world_y: f32,
    },
    RemoveWorldLabel {
        label_id: i32,
    },
    CreateMarker {
        marker_id: i32,
        json_len: usize,
    },
    RemoveMarker {
        marker_id: i32,
    },
    UpdateMarker {
        marker_id: i32,
        control: u8,
        control_name: Option<String>,
        p1_bits: u64,
        p2_bits: u64,
        p3_bits: u64,
    },
    UpdateMarkerText {
        marker_id: i32,
        control: u8,
        control_name: Option<String>,
        fetch: bool,
        text: Option<String>,
    },
    UpdateMarkerTexture {
        marker_id: i32,
        texture_kind: u8,
        texture_kind_name: String,
    },
    MenuShown {
        menu_id: i32,
        title: Option<String>,
        message: Option<String>,
        option_rows: usize,
        first_row_len: usize,
    },
    FollowUpMenuShown {
        menu_id: i32,
        title: Option<String>,
        message: Option<String>,
        option_rows: usize,
        first_row_len: usize,
    },
    HideFollowUpMenu {
        menu_id: i32,
    },
    CopyToClipboard {
        text: Option<String>,
    },
    OpenUri {
        uri: Option<String>,
    },
    TextInput {
        text_input_id: i32,
        title: Option<String>,
        message: Option<String>,
        text_length: i32,
        default_text: Option<String>,
        numeric: bool,
        allow_empty: bool,
    },
    SetItem {
        build_pos: Option<i32>,
        item_id: Option<i16>,
        amount: i32,
    },
    SetItems {
        build_pos: Option<i32>,
        stack_count: usize,
        first_item_id: Option<i16>,
        first_amount: Option<i32>,
    },
    SetLiquid {
        build_pos: Option<i32>,
        liquid_id: Option<i16>,
        amount: f32,
    },
    SetLiquids {
        build_pos: Option<i32>,
        stack_count: usize,
        first_liquid_id: Option<i16>,
        first_amount_bits: Option<u32>,
    },
    SetTileItems {
        item_id: Option<i16>,
        amount: i32,
        position_count: usize,
        first_position: Option<i32>,
    },
    SetTileLiquids {
        liquid_id: Option<i16>,
        amount_bits: u32,
        position_count: usize,
        first_position: Option<i32>,
    },
    InfoMessage {
        message: Option<String>,
    },
    InfoPopup {
        reliable: bool,
        popup_id: Option<String>,
        message: Option<String>,
        duration: f32,
        align: i32,
        top: i32,
        left: i32,
        bottom: i32,
        right: i32,
    },
    InfoToast {
        message: Option<String>,
        duration: f32,
    },
    WarningToast {
        unicode: i32,
        text: Option<String>,
    },
    SetPlayerTeamEditor {
        team_id: u8,
    },
    MenuChoose {
        menu_id: i32,
        option: i32,
    },
    TextInputResult {
        text_input_id: i32,
        text: Option<String>,
    },
    RequestItem {
        build_pos: Option<i32>,
        item_id: Option<i16>,
        amount: i32,
    },
    RequestBuildPayload {
        build_pos: Option<i32>,
    },
    RequestUnitPayload {
        target: Option<UnitRefProjection>,
    },
    TransferInventory {
        build_pos: Option<i32>,
    },
    RotateBlock {
        build_pos: Option<i32>,
        direction: bool,
    },
    DropItem {
        angle: f32,
    },
    DeletePlans {
        positions: Vec<i32>,
    },
    BuildingControlSelect {
        build_pos: Option<i32>,
    },
    UnitClear,
    UnitControl {
        target: Option<UnitRefProjection>,
    },
    UnitBuildingControlSelect {
        target: Option<UnitRefProjection>,
        build_pos: Option<i32>,
    },
    CommandBuilding {
        buildings: Vec<i32>,
        x: f32,
        y: f32,
    },
    CommandUnits {
        unit_ids: Vec<i32>,
        build_target: Option<i32>,
        unit_target: Option<UnitRefProjection>,
        x: f32,
        y: f32,
        queue_command: bool,
        final_batch: bool,
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
        x: i32,
        y: i32,
        team_id: u8,
        builder_kind: u8,
        builder_value: i32,
    },
    BeginPlace {
        x: i32,
        y: i32,
        block_id: Option<i16>,
        rotation: i32,
        team_id: u8,
        config_kind: u8,
        config_kind_name: &'static str,
        builder_kind: u8,
        builder_value: i32,
    },
    RemoveQueueBlock {
        x: i32,
        y: i32,
        breaking: bool,
        removed_local_plan: bool,
    },
    TileConfig {
        build_pos: Option<i32>,
        config_kind: Option<u8>,
        config_kind_name: Option<String>,
        parse_failed: bool,
        business_applied: bool,
        cleared_pending_local: bool,
        was_rollback: bool,
        pending_local_match: Option<bool>,
    },
    ConstructFinish {
        tile_pos: i32,
        block_id: Option<i16>,
        builder_kind: u8,
        builder_value: i32,
        rotation: u8,
        team_id: u8,
        config_kind: u8,
        removed_local_plan: bool,
    },
    DeconstructFinish {
        tile_pos: i32,
        block_id: Option<i16>,
        builder_kind: u8,
        builder_value: i32,
        removed_local_plan: bool,
    },
    BuildHealthUpdate {
        pair_count: usize,
        first_build_pos: Option<i32>,
        first_health_bits: Option<u32>,
        pairs: Vec<BuildHealthPair>,
    },
    Kicked {
        reason_text: Option<String>,
        reason_ordinal: Option<i32>,
        duration_ms: Option<u64>,
    },
    Ping {
        sent_at_ms: Option<u64>,
        response_queued: bool,
    },
    PingResponse {
        sent_at_ms: Option<u64>,
        round_trip_ms: Option<u64>,
    },
    SnapshotReceived(HighFrequencyRemoteMethod),
    DeferredPacketWhileLoading {
        packet_id: u8,
        remote: Option<IgnoredRemotePacketMeta>,
    },
    IgnoredPacket {
        packet_id: u8,
        remote: Option<IgnoredRemotePacketMeta>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildHealthPair {
    pub build_pos: i32,
    pub health_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredRemotePacketMeta {
    pub method: String,
    pub packet_class: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPacketTransport {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientLogicDataTransport {
    Reliable,
    Unreliable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientSessionAction {
    SendPacket {
        packet_id: u8,
        transport: ClientPacketTransport,
        bytes: Vec<u8>,
    },
    SendFramework {
        message: FrameworkMessage,
        bytes: Vec<u8>,
    },
    TimedOut {
        idle_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientSnapshotInputState {
    pub unit_id: Option<i32>,
    pub dead: bool,
    pub position: Option<(f32, f32)>,
    pub pointer: Option<(f32, f32)>,
    pub rotation: f32,
    pub base_rotation: f32,
    pub velocity: (f32, f32),
    pub mining_tile: Option<(i32, i32)>,
    pub boosting: bool,
    pub shooting: bool,
    pub chatting: bool,
    pub building: bool,
    pub selected_block_id: Option<i16>,
    pub selected_rotation: i32,
    pub plans: Option<Vec<ClientBuildPlan>>,
    pub view_center: Option<(f32, f32)>,
    pub view_size: Option<(f32, f32)>,
}

impl Default for ClientSnapshotInputState {
    fn default() -> Self {
        Self {
            unit_id: None,
            dead: true,
            position: None,
            pointer: None,
            rotation: 0.0,
            base_rotation: 0.0,
            velocity: (0.0, 0.0),
            mining_tile: None,
            boosting: false,
            shooting: false,
            chatting: false,
            building: false,
            selected_block_id: None,
            selected_rotation: 0,
            plans: None,
            view_center: None,
            view_size: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientBuildPlan {
    pub tile: (i32, i32),
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: u8,
    pub config: ClientBuildPlanConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientBuildPlanConfig {
    None,
    Int(i32),
    Long(i64),
    FloatBits(u32),
    Bool(bool),
    IntSeq(Vec<i32>),
    Point2 { x: i32, y: i32 },
    Point2Array(Vec<(i32, i32)>),
    TechNodeRaw { content_type: u8, content_id: i16 },
    DoubleBits(u64),
    BuildingPos(i32),
    LAccess(i16),
    String(String),
    Bytes(Vec<u8>),
    LegacyUnitCommandNull(u8),
    BoolArray(Vec<bool>),
    UnitId(i32),
    Vec2Array(Vec<(u32, u32)>),
    Vec2 { x_bits: u32, y_bits: u32 },
    Team(u8),
    IntArray(Vec<i32>),
    ObjectArray(Vec<ClientBuildPlanConfig>),
    Content { content_type: u8, content_id: i16 },
    UnitCommand(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientUnitRef {
    None,
    Block(i32),
    Standard(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSessionTiming {
    pub keepalive_interval_ms: u64,
    pub client_snapshot_interval_ms: u64,
    pub connect_timeout_ms: u64,
    pub timeout_ms: u64,
}

impl Default for ClientSessionTiming {
    fn default() -> Self {
        Self {
            keepalive_interval_ms: 1_000,
            // Java syncs every 4 ticks, which is ~66.7ms at 60 TPS.
            client_snapshot_interval_ms: 67,
            // Java's connecting/data-load timeout is 30 minutes.
            connect_timeout_ms: 1_800_000,
            // Keep the single Rust idle watchdog no stricter than Java's 20s entity snapshot timeout.
            timeout_ms: 20_000,
        }
    }
}

#[derive(Debug)]
pub enum ClientSessionError {
    PacketCodec(PacketCodecError),
    RemoteManifest(RemoteManifestError),
    BootstrapFlow(crate::bootstrap_flow::BootstrapFlowError),
    MissingConnectConfirmPacket,
    MissingWorldDataBeginPacket,
    MissingSendChatMessagePacket,
    MissingRemotePacket(&'static str),
    MissingWorldStreamBegin,
    TruncatedRemotePayload {
        method: &'static str,
        expected_at_least: usize,
        actual: usize,
    },
    WorldBundleParse(String),
}

impl fmt::Display for ClientSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketCodec(error) => write!(f, "{error}"),
            Self::RemoteManifest(error) => write!(f, "{error}"),
            Self::BootstrapFlow(error) => write!(f, "{error}"),
            Self::MissingConnectConfirmPacket => {
                write!(f, "missing connectConfirm packet in remote manifest")
            }
            Self::MissingWorldDataBeginPacket => {
                write!(f, "missing worldDataBegin packet in remote manifest")
            }
            Self::MissingSendChatMessagePacket => {
                write!(f, "missing sendChatMessage packet in remote manifest")
            }
            Self::MissingRemotePacket(method) => {
                write!(f, "missing {method} packet in remote manifest")
            }
            Self::MissingWorldStreamBegin => {
                write!(f, "received stream chunk before stream begin")
            }
            Self::TruncatedRemotePayload {
                method,
                expected_at_least,
                actual,
            } => write!(
                f,
                "truncated remote payload for {method}: expected at least {expected_at_least} bytes, got {actual}"
            ),
            Self::WorldBundleParse(error) => write!(f, "failed to parse world bundle: {error}"),
        }
    }
}

impl std::error::Error for ClientSessionError {}

impl From<PacketCodecError> for ClientSessionError {
    fn from(value: PacketCodecError) -> Self {
        Self::PacketCodec(value)
    }
}

impl From<RemoteManifestError> for ClientSessionError {
    fn from(value: RemoteManifestError) -> Self {
        Self::RemoteManifest(value)
    }
}

impl From<crate::bootstrap_flow::BootstrapFlowError> for ClientSessionError {
    fn from(value: crate::bootstrap_flow::BootstrapFlowError) -> Self {
        Self::BootstrapFlow(value)
    }
}

fn encode_client_snapshot_payload(
    state: &SessionState,
    input: &ClientSnapshotInputState,
    snapshot_id: i32,
) -> Vec<u8> {
    const TILE_SIZE: f32 = 8.0;
    const DEFAULT_VIEW_WIDTH: f32 = 1920.0;
    const DEFAULT_VIEW_HEIGHT: f32 = 1080.0;

    let default_view_width = if state.world_map_width > 0 {
        state.world_map_width as f32 * TILE_SIZE
    } else {
        DEFAULT_VIEW_WIDTH
    };
    let default_view_height = if state.world_map_height > 0 {
        state.world_map_height as f32 * TILE_SIZE
    } else {
        DEFAULT_VIEW_HEIGHT
    };
    let (view_width, view_height) = input
        .view_size
        .unwrap_or((default_view_width, default_view_height));
    let (center_x, center_y) = input
        .view_center
        .unwrap_or((view_width * 0.5, view_height * 0.5));
    let (x, y) = input.position.unwrap_or((center_x, center_y));
    let (pointer_x, pointer_y) = input.pointer.unwrap_or((x, y));
    let (x_velocity, y_velocity) = input.velocity;
    let mining_tile = input
        .mining_tile
        .map(|(tile_x, tile_y)| pack_point2(tile_x, tile_y))
        .unwrap_or(-1);

    let mut payload = Vec::with_capacity(96);
    payload.extend_from_slice(&snapshot_id.to_be_bytes());
    payload.extend_from_slice(&input.unit_id.unwrap_or(-1).to_be_bytes());
    payload.push(u8::from(input.dead));
    write_f32(&mut payload, x);
    write_f32(&mut payload, y);
    write_f32(&mut payload, pointer_x);
    write_f32(&mut payload, pointer_y);
    write_f32(&mut payload, input.rotation);
    write_f32(&mut payload, input.base_rotation);
    write_f32(&mut payload, x_velocity);
    write_f32(&mut payload, y_velocity);
    payload.extend_from_slice(&mining_tile.to_be_bytes());
    payload.push(u8::from(input.boosting));
    payload.push(u8::from(input.shooting));
    payload.push(u8::from(input.chatting));
    payload.push(u8::from(input.building));
    payload.extend_from_slice(&input.selected_block_id.unwrap_or(-1).to_be_bytes());
    payload.extend_from_slice(&input.selected_rotation.to_be_bytes());
    write_client_build_plans_queue(&mut payload, input.plans.as_deref());
    write_f32(&mut payload, center_x);
    write_f32(&mut payload, center_y);
    write_f32(&mut payload, view_width);
    write_f32(&mut payload, view_height);
    payload
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_bits().to_be_bytes());
}

fn write_client_build_plans_queue(out: &mut Vec<u8>, plans: Option<&[ClientBuildPlan]>) {
    let Some(plans) = plans else {
        out.extend_from_slice(&(-1i32).to_be_bytes());
        return;
    };

    let used = max_client_build_plans(plans);
    out.extend_from_slice(&(used as i32).to_be_bytes());
    for plan in plans.iter().take(used) {
        write_client_build_plan(out, plan);
    }
}

fn max_client_build_plans(plans: &[ClientBuildPlan]) -> usize {
    let used = plans.len().min(20);
    let mut total_len = 0usize;

    for (index, plan) in plans.iter().take(used).enumerate() {
        total_len = total_len.saturating_add(client_build_plan_config_payload_len(&plan.config));
        if total_len > 500 {
            return index + 1;
        }
    }

    used
}

fn client_build_plan_config_payload_len(config: &ClientBuildPlanConfig) -> usize {
    match config {
        ClientBuildPlanConfig::String(text) => text.encode_utf16().count(),
        ClientBuildPlanConfig::Bytes(bytes) => bytes.len(),
        _ => 0,
    }
}

fn write_client_build_plan(out: &mut Vec<u8>, plan: &ClientBuildPlan) {
    out.push(u8::from(plan.breaking));
    out.extend_from_slice(&pack_point2(plan.tile.0, plan.tile.1).to_be_bytes());
    if plan.breaking {
        return;
    }

    out.extend_from_slice(&plan.block_id.unwrap_or(-1).to_be_bytes());
    out.push(plan.rotation);
    out.push(1);
    write_client_build_plan_config(out, &plan.config);
}

fn write_client_build_plan_config(out: &mut Vec<u8>, config: &ClientBuildPlanConfig) {
    write_typeio_object(out, &client_build_plan_config_to_typeio_object(config));
}

fn client_build_plan_config_to_typeio_object(config: &ClientBuildPlanConfig) -> TypeIoObject {
    match config {
        ClientBuildPlanConfig::None => TypeIoObject::Null,
        ClientBuildPlanConfig::Int(value) => TypeIoObject::Int(*value),
        ClientBuildPlanConfig::Long(value) => TypeIoObject::Long(*value),
        ClientBuildPlanConfig::FloatBits(bits) => TypeIoObject::Float(f32::from_bits(*bits)),
        ClientBuildPlanConfig::Bool(value) => TypeIoObject::Bool(*value),
        ClientBuildPlanConfig::IntSeq(values) => TypeIoObject::IntSeq(values.clone()),
        ClientBuildPlanConfig::Point2 { x, y } => TypeIoObject::Point2 { x: *x, y: *y },
        ClientBuildPlanConfig::Point2Array(points) => TypeIoObject::PackedPoint2Array(
            points
                .iter()
                .map(|(x, y)| pack_point2(*x, *y))
                .collect::<Vec<_>>(),
        ),
        ClientBuildPlanConfig::TechNodeRaw {
            content_type,
            content_id,
        } => TypeIoObject::TechNodeRaw {
            content_type: *content_type,
            content_id: *content_id,
        },
        ClientBuildPlanConfig::DoubleBits(bits) => TypeIoObject::Double(f64::from_bits(*bits)),
        ClientBuildPlanConfig::BuildingPos(value) => TypeIoObject::BuildingPos(*value),
        ClientBuildPlanConfig::LAccess(value) => TypeIoObject::LAccess(*value),
        ClientBuildPlanConfig::String(text) => TypeIoObject::String(Some(text.clone())),
        ClientBuildPlanConfig::Bytes(bytes) => TypeIoObject::Bytes(bytes.clone()),
        ClientBuildPlanConfig::LegacyUnitCommandNull(value) => {
            TypeIoObject::LegacyUnitCommandNull(*value)
        }
        ClientBuildPlanConfig::BoolArray(values) => TypeIoObject::BoolArray(values.clone()),
        ClientBuildPlanConfig::UnitId(value) => TypeIoObject::UnitId(*value),
        ClientBuildPlanConfig::Vec2Array(values) => TypeIoObject::Vec2Array(
            values
                .iter()
                .map(|(x_bits, y_bits)| (f32::from_bits(*x_bits), f32::from_bits(*y_bits)))
                .collect::<Vec<_>>(),
        ),
        ClientBuildPlanConfig::Vec2 { x_bits, y_bits } => TypeIoObject::Vec2 {
            x: f32::from_bits(*x_bits),
            y: f32::from_bits(*y_bits),
        },
        ClientBuildPlanConfig::Team(value) => TypeIoObject::Team(*value),
        ClientBuildPlanConfig::IntArray(values) => TypeIoObject::IntArray(values.clone()),
        ClientBuildPlanConfig::ObjectArray(values) => TypeIoObject::ObjectArray(
            values
                .iter()
                .map(client_build_plan_config_to_typeio_object)
                .collect::<Vec<_>>(),
        ),
        ClientBuildPlanConfig::Content {
            content_type,
            content_id,
        } => TypeIoObject::ContentRaw {
            content_type: *content_type,
            content_id: *content_id,
        },
        ClientBuildPlanConfig::UnitCommand(command_id) => TypeIoObject::UnitCommand(*command_id),
    }
}

fn pack_point2(x: i32, y: i32) -> i32 {
    ((x & 0xffff) << 16) | (y & 0xffff)
}

fn sanitize_bootstrap_coord(bits: u32) -> f32 {
    let value = f32::from_bits(bits);
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn bootstrap_player_unit_id(bootstrap: &LoadedWorldBootstrap) -> Option<i32> {
    if bootstrap.player_unit_kind != 2 || bootstrap.player_unit_value == 0 {
        None
    } else {
        i32::try_from(bootstrap.player_unit_value).ok()
    }
}

fn decode_set_position_payload(payload: &[u8]) -> Result<(f32, f32), ClientSessionError> {
    if payload.len() < 8 {
        return Err(ClientSessionError::TruncatedRemotePayload {
            method: "setPosition",
            expected_at_least: 8,
            actual: payload.len(),
        });
    }

    let x = f32::from_bits(u32::from_be_bytes(payload[0..4].try_into().unwrap()));
    let y = f32::from_bits(u32::from_be_bytes(payload[4..8].try_into().unwrap()));
    Ok((x, y))
}

fn decode_player_spawn_payload(payload: &[u8]) -> Option<(i32, i32)> {
    if payload.len() < 8 {
        return None;
    }
    let tile_pos = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let player_id = i32::from_be_bytes(payload[4..8].try_into().ok()?);
    Some((tile_pos, player_id))
}

fn decode_player_disconnect_payload(payload: &[u8]) -> Option<i32> {
    let mut cursor = 0usize;
    let player_id = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(player_id)
}

fn read_optional_build_pos(payload: &[u8], cursor: &mut usize) -> Option<Option<i32>> {
    let value = read_i32(payload, cursor)?;
    Some((value != -1).then_some(value))
}

fn read_optional_item_id(payload: &[u8], cursor: &mut usize) -> Option<Option<i16>> {
    let value = read_i16(payload, cursor)?;
    Some((value != -1).then_some(value))
}

fn read_optional_liquid_id(payload: &[u8], cursor: &mut usize) -> Option<Option<i16>> {
    let value = read_i16(payload, cursor)?;
    Some((value != -1).then_some(value))
}

fn read_optional_entity_id(payload: &[u8], cursor: &mut usize) -> Option<Option<i32>> {
    let value = read_i32(payload, cursor)?;
    Some((value != -1).then_some(value))
}

fn read_optional_unit_ref(payload: &[u8], cursor: &mut usize) -> Option<Option<UnitRefProjection>> {
    let kind = read_u8(payload, cursor)?;
    let value = read_i32(payload, cursor)?;
    match kind {
        0 => Some(None),
        1 | 2 => Some(Some(UnitRefProjection { kind, value })),
        _ => None,
    }
}

fn read_i32_array_with_i16_len(payload: &[u8], cursor: &mut usize) -> Option<Vec<i32>> {
    let len = read_i16(payload, cursor)?;
    if len < 0 {
        return None;
    }
    let len = usize::try_from(len).ok()?;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(read_i32(payload, cursor)?);
    }
    Some(values)
}

fn decode_with_optional_server_player_prefix<T>(
    payload: &[u8],
    min_body_len: usize,
    mut parse: impl FnMut(&[u8], &mut usize) -> Option<T>,
) -> Option<T> {
    if payload.len() >= min_body_len.saturating_add(4) {
        let mut cursor = 0usize;
        let _ = read_i32(payload, &mut cursor)?;
        if let Some(value) = parse(payload, &mut cursor) {
            if cursor == payload.len() {
                return Some(value);
            }
        }
    }

    if payload.len() < min_body_len {
        return None;
    }
    let mut cursor = 0usize;
    let value = parse(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(value)
}

fn decode_set_player_team_editor_payload(payload: &[u8]) -> Option<u8> {
    decode_with_optional_server_player_prefix(payload, 1, |payload, cursor| {
        read_u8(payload, cursor)
    })
}

fn decode_set_flag_payload(payload: &[u8]) -> Option<(Option<String>, bool)> {
    let mut cursor = 0usize;
    let flag = read_typeio_string_at(payload, &mut cursor)?;
    let add = read_u8(payload, &mut cursor)? != 0;
    (cursor == payload.len()).then_some((flag, add))
}

fn decode_team_payload(payload: &[u8]) -> Option<u8> {
    let mut cursor = 0usize;
    let team_id = read_u8(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(team_id)
}

fn decode_content_payload(payload: &[u8]) -> Option<(u8, i16)> {
    let mut cursor = 0usize;
    let content_type = read_u8(payload, &mut cursor)?;
    let content_id = read_i16(payload, &mut cursor)?;
    (cursor == payload.len()).then_some((content_type, content_id))
}

fn decode_optional_typeio_string_payload(payload: &[u8]) -> Option<Option<String>> {
    let mut cursor = 0usize;
    let value = read_typeio_string_at(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(value)
}

fn read_typeio_string_matrix_shape_at(
    payload: &[u8],
    cursor: &mut usize,
) -> Option<(usize, usize)> {
    let rows = read_u8(payload, cursor)? as usize;
    let mut first_row_len = 0usize;
    for row in 0..rows {
        let cols = read_u8(payload, cursor)? as usize;
        if row == 0 {
            first_row_len = cols;
        }
        for _ in 0..cols {
            let _ = read_typeio_string_at(payload, cursor)?;
        }
    }
    Some((rows, first_row_len))
}

fn decode_menu_dialog_payload(payload: &[u8]) -> Option<MenuDialogSummary> {
    let mut cursor = 0usize;
    let menu_id = read_i32(payload, &mut cursor)?;
    let title = read_typeio_string_at(payload, &mut cursor)?;
    let message = read_typeio_string_at(payload, &mut cursor)?;
    let (option_rows, first_row_len) = read_typeio_string_matrix_shape_at(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(MenuDialogSummary {
        menu_id,
        title,
        message,
        option_rows,
        first_row_len,
    })
}

fn decode_hide_follow_up_menu_payload(payload: &[u8]) -> Option<i32> {
    let mut cursor = 0usize;
    let menu_id = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(menu_id)
}

fn decode_text_input_payload(payload: &[u8], has_allow_empty: bool) -> Option<TextInputSummary> {
    let mut cursor = 0usize;
    let text_input_id = read_i32(payload, &mut cursor)?;
    let title = read_typeio_string_at(payload, &mut cursor)?;
    let message = read_typeio_string_at(payload, &mut cursor)?;
    let text_length = read_i32(payload, &mut cursor)?;
    let default_text = read_typeio_string_at(payload, &mut cursor)?;
    let numeric = read_u8(payload, &mut cursor)? != 0;
    let allow_empty = if has_allow_empty {
        read_u8(payload, &mut cursor)? != 0
    } else {
        false
    };
    (cursor == payload.len()).then_some(TextInputSummary {
        text_input_id,
        title,
        message,
        text_length,
        default_text,
        numeric,
        allow_empty,
    })
}

fn decode_set_item_payload(payload: &[u8]) -> Option<SetItemSummary> {
    let mut cursor = 0usize;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    let item_id = read_optional_item_id(payload, &mut cursor)?;
    let amount = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(SetItemSummary {
        build_pos,
        item_id,
        amount,
    })
}

fn decode_set_liquid_payload(payload: &[u8]) -> Option<SetLiquidSummary> {
    let mut cursor = 0usize;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    let liquid_id = read_optional_liquid_id(payload, &mut cursor)?;
    let amount = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(SetLiquidSummary {
        build_pos,
        liquid_id,
        amount,
    })
}

fn decode_set_items_payload(payload: &[u8]) -> Option<SetItemsSummary> {
    let mut cursor = 0usize;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    let stack_count = read_i16(payload, &mut cursor)?;
    if stack_count < 0 {
        return None;
    }
    let stack_count = usize::try_from(stack_count).ok()?;
    let mut first_item_id = None;
    let mut first_amount = None;
    for index in 0..stack_count {
        let item_id = read_optional_item_id(payload, &mut cursor)?;
        let amount = read_i32(payload, &mut cursor)?;
        if index == 0 {
            first_item_id = item_id;
            first_amount = Some(amount);
        }
    }
    (cursor == payload.len()).then_some(SetItemsSummary {
        build_pos,
        stack_count,
        first_item_id,
        first_amount,
    })
}

fn decode_set_liquids_payload(payload: &[u8]) -> Option<SetLiquidsSummary> {
    let mut cursor = 0usize;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    let stack_count = read_i16(payload, &mut cursor)?;
    if stack_count < 0 {
        return None;
    }
    let stack_count = usize::try_from(stack_count).ok()?;
    let mut first_liquid_id = None;
    let mut first_amount_bits = None;
    for index in 0..stack_count {
        let liquid_id = read_optional_liquid_id(payload, &mut cursor)?;
        let amount = read_f32(payload, &mut cursor)?;
        if index == 0 {
            first_liquid_id = liquid_id;
            first_amount_bits = Some(amount.to_bits());
        }
    }
    (cursor == payload.len()).then_some(SetLiquidsSummary {
        build_pos,
        stack_count,
        first_liquid_id,
        first_amount_bits,
    })
}

fn decode_set_tile_items_payload(payload: &[u8]) -> Option<SetTileItemsSummary> {
    let mut cursor = 0usize;
    let item_id = read_optional_item_id(payload, &mut cursor)?;
    let amount = read_i32(payload, &mut cursor)?;
    let positions = read_i32_array_with_i16_len(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(SetTileItemsSummary {
        item_id,
        amount,
        position_count: positions.len(),
        first_position: positions.first().copied(),
    })
}

fn decode_set_tile_liquids_payload(payload: &[u8]) -> Option<SetTileLiquidsSummary> {
    let mut cursor = 0usize;
    let liquid_id = read_optional_liquid_id(payload, &mut cursor)?;
    let amount = read_f32(payload, &mut cursor)?;
    let positions = read_i32_array_with_i16_len(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(SetTileLiquidsSummary {
        liquid_id,
        amount_bits: amount.to_bits(),
        position_count: positions.len(),
        first_position: positions.first().copied(),
    })
}

fn decode_world_label_payload(payload: &[u8], has_id: bool) -> Option<WorldLabelSummary> {
    let mut cursor = 0usize;
    let message = read_typeio_string_at(payload, &mut cursor)?;
    let label_id = if has_id {
        Some(read_i32(payload, &mut cursor)?)
    } else {
        None
    };
    let duration = read_f32(payload, &mut cursor)?;
    let world_x = read_f32(payload, &mut cursor)?;
    let world_y = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(WorldLabelSummary {
        message,
        label_id,
        duration,
        world_x,
        world_y,
    })
}

fn decode_remove_world_label_payload(payload: &[u8]) -> Option<i32> {
    let mut cursor = 0usize;
    let label_id = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(label_id)
}

fn decode_create_marker_payload(payload: &[u8]) -> Option<CreateMarkerSummary> {
    if payload.len() < 8 {
        return None;
    }
    let marker_id = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let json_len = decode_length_prefixed_json_payload(&payload[4..]).ok()?.len();
    Some(CreateMarkerSummary {
        marker_id,
        json_len,
    })
}

fn decode_remove_marker_payload(payload: &[u8]) -> Option<i32> {
    (payload.len() == 4).then_some(i32::from_be_bytes(payload.try_into().ok()?))
}

fn decode_update_marker_payload(payload: &[u8]) -> Option<UpdateMarkerSummary> {
    if payload.len() != 29 {
        return None;
    }
    Some(UpdateMarkerSummary {
        marker_id: i32::from_be_bytes(payload[0..4].try_into().ok()?),
        control: payload[4],
        p1_bits: u64::from_be_bytes(payload[5..13].try_into().ok()?),
        p2_bits: u64::from_be_bytes(payload[13..21].try_into().ok()?),
        p3_bits: u64::from_be_bytes(payload[21..29].try_into().ok()?),
    })
}

fn decode_update_marker_text_payload(payload: &[u8]) -> Option<UpdateMarkerTextSummary> {
    if payload.len() < 6 {
        return None;
    }
    Some(UpdateMarkerTextSummary {
        marker_id: i32::from_be_bytes(payload[0..4].try_into().ok()?),
        control: payload[4],
        fetch: payload[5] != 0,
        text: decode_optional_typeio_string_payload(&payload[6..])?,
    })
}

fn decode_update_marker_texture_payload(payload: &[u8]) -> Option<UpdateMarkerTextureSummary> {
    if payload.len() < 6 {
        return None;
    }
    let marker_id = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let texture_payload = &payload[4..];
    let (texture_object, consumed_len) = read_object_prefix(texture_payload).ok()?;
    if consumed_len != texture_payload.len() {
        return None;
    }
    Some(UpdateMarkerTextureSummary {
        marker_id,
        texture_kind: texture_payload[0],
        texture_kind_name: texture_object.kind().to_string(),
    })
}

fn decode_info_toast_payload(payload: &[u8]) -> Option<InfoToastSummary> {
    let mut cursor = 0usize;
    let message = read_typeio_string_at(payload, &mut cursor)?;
    let duration = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(InfoToastSummary { message, duration })
}

fn decode_warning_toast_payload(payload: &[u8]) -> Option<WarningToastSummary> {
    let mut cursor = 0usize;
    let unicode = read_i32(payload, &mut cursor)?;
    let text = read_typeio_string_at(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(WarningToastSummary { unicode, text })
}

fn decode_info_popup_payload(payload: &[u8], has_id: bool) -> Option<InfoPopupSummary> {
    let mut cursor = 0usize;
    let message = read_typeio_string_at(payload, &mut cursor)?;
    let popup_id = if has_id {
        read_typeio_string_at(payload, &mut cursor)?
    } else {
        None
    };
    let duration = read_f32(payload, &mut cursor)?;
    let align = read_i32(payload, &mut cursor)?;
    let top = read_i32(payload, &mut cursor)?;
    let left = read_i32(payload, &mut cursor)?;
    let bottom = read_i32(payload, &mut cursor)?;
    let right = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(InfoPopupSummary {
        popup_id,
        message,
        duration,
        align,
        top,
        left,
        bottom,
        right,
    })
}

fn decode_menu_choose_payload(payload: &[u8]) -> Option<MenuChooseSummary> {
    decode_with_optional_server_player_prefix(payload, 8, |payload, cursor| {
        let menu_id = read_i32(payload, cursor)?;
        let option = read_i32(payload, cursor)?;
        Some(MenuChooseSummary { menu_id, option })
    })
}

fn decode_text_input_result_payload(payload: &[u8]) -> Option<TextInputResultSummary> {
    decode_with_optional_server_player_prefix(payload, 5, |payload, cursor| {
        let text_input_id = read_i32(payload, cursor)?;
        let text = read_typeio_string_at(payload, cursor)?;
        Some(TextInputResultSummary {
            text_input_id,
            text,
        })
    })
}

fn decode_request_item_inbound_payload(payload: &[u8]) -> Option<RequestItemSummary> {
    decode_with_optional_server_player_prefix(payload, 10, |payload, cursor| {
        let build_pos = read_optional_build_pos(payload, cursor)?;
        let item_id = read_optional_item_id(payload, cursor)?;
        let amount = read_i32(payload, cursor)?;
        Some(RequestItemSummary {
            build_pos,
            item_id,
            amount,
        })
    })
}

fn decode_building_control_select_payload(payload: &[u8]) -> Option<Option<i32>> {
    decode_with_optional_server_player_prefix(payload, 4, |payload, cursor| {
        read_optional_build_pos(payload, cursor)
    })
}

fn decode_rotate_block_payload(payload: &[u8]) -> Option<(Option<i32>, bool)> {
    decode_with_optional_server_player_prefix(payload, 5, |payload, cursor| {
        let build_pos = read_optional_build_pos(payload, cursor)?;
        let direction = read_u8(payload, cursor)? != 0;
        Some((build_pos, direction))
    })
}

fn decode_request_build_payload_payload(payload: &[u8]) -> Option<Option<i32>> {
    decode_with_optional_server_player_prefix(payload, 4, |payload, cursor| {
        read_optional_build_pos(payload, cursor)
    })
}

fn decode_request_unit_payload_payload(payload: &[u8]) -> Option<Option<UnitRefProjection>> {
    decode_with_optional_server_player_prefix(payload, 5, |payload, cursor| {
        read_optional_unit_ref(payload, cursor)
    })
}

fn decode_transfer_inventory_payload(payload: &[u8]) -> Option<Option<i32>> {
    decode_with_optional_server_player_prefix(payload, 4, |payload, cursor| {
        read_optional_build_pos(payload, cursor)
    })
}

fn decode_drop_item_payload(payload: &[u8]) -> Option<f32> {
    let mut cursor = 0usize;
    let angle = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(angle)
}

fn decode_delete_plans_payload(payload: &[u8]) -> Option<Vec<i32>> {
    decode_with_optional_server_player_prefix(payload, 2, |payload, cursor| {
        read_i32_array_with_i16_len(payload, cursor)
    })
}

fn decode_unit_control_payload(payload: &[u8]) -> Option<Option<UnitRefProjection>> {
    decode_with_optional_server_player_prefix(payload, 5, |payload, cursor| {
        read_optional_unit_ref(payload, cursor)
    })
}

fn decode_unit_building_control_select_payload(
    payload: &[u8],
) -> Option<UnitBuildingControlSelectSummary> {
    let mut cursor = 0usize;
    let target = read_optional_unit_ref(payload, &mut cursor)?;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(UnitBuildingControlSelectSummary { target, build_pos })
}

fn decode_command_building_payload(payload: &[u8]) -> Option<CommandBuildingSummary> {
    let mut cursor = 0usize;
    let buildings = read_i32_array_with_i16_len(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(CommandBuildingSummary { buildings, x, y })
}

fn decode_command_units_payload(payload: &[u8]) -> Option<CommandUnitsSummary> {
    let mut cursor = 0usize;
    let unit_ids = read_i32_array_with_i16_len(payload, &mut cursor)?;
    let build_target = read_optional_build_pos(payload, &mut cursor)?;
    let unit_target = read_optional_unit_ref(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    let queue_command = read_u8(payload, &mut cursor)? != 0;
    let final_batch = read_u8(payload, &mut cursor)? != 0;
    (cursor == payload.len()).then_some(CommandUnitsSummary {
        unit_ids,
        build_target,
        unit_target,
        x,
        y,
        queue_command,
        final_batch,
    })
}

fn decode_set_unit_command_payload(payload: &[u8]) -> Option<SetUnitCommandSummary> {
    let mut cursor = 0usize;
    let unit_ids = read_i32_array_with_i16_len(payload, &mut cursor)?;
    let command_id = match read_u8(payload, &mut cursor)? {
        u8::MAX => None,
        value => Some(value),
    };
    (cursor == payload.len()).then_some(SetUnitCommandSummary {
        unit_ids,
        command_id,
    })
}

fn decode_set_unit_stance_payload(payload: &[u8]) -> Option<SetUnitStanceSummary> {
    let mut cursor = 0usize;
    let unit_ids = read_i32_array_with_i16_len(payload, &mut cursor)?;
    let stance_id = match read_u8(payload, &mut cursor)? {
        u8::MAX => None,
        value => Some(value),
    };
    let enable = read_u8(payload, &mut cursor)? != 0;
    (cursor == payload.len()).then_some(SetUnitStanceSummary {
        unit_ids,
        stance_id,
        enable,
    })
}

fn decode_take_items_payload(payload: &[u8]) -> Option<TakeItemsProjection> {
    let mut cursor = 0usize;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    let item_id = read_optional_item_id(payload, &mut cursor)?;
    let amount = read_i32(payload, &mut cursor)?;
    let to = read_optional_unit_ref(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(TakeItemsProjection {
        build_pos,
        item_id,
        amount,
        to,
    })
}

fn decode_transfer_item_to_payload(payload: &[u8]) -> Option<TransferItemToProjection> {
    let mut cursor = 0usize;
    let unit = read_optional_unit_ref(payload, &mut cursor)?;
    let item_id = read_optional_item_id(payload, &mut cursor)?;
    let amount = read_i32(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(TransferItemToProjection {
        unit,
        item_id,
        amount,
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
        build_pos,
    })
}

fn decode_transfer_item_to_unit_payload(payload: &[u8]) -> Option<TransferItemToUnitProjection> {
    let mut cursor = 0usize;
    let item_id = read_optional_item_id(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    let to_entity_id = read_optional_entity_id(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(TransferItemToUnitProjection {
        item_id,
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
        to_entity_id,
    })
}

fn decode_payload_dropped_payload(payload: &[u8]) -> Option<PayloadDroppedProjection> {
    let mut cursor = 0usize;
    let unit = read_optional_unit_ref(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(PayloadDroppedProjection {
        unit,
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
    })
}

fn decode_picked_build_payload(payload: &[u8]) -> Option<PickedBuildPayloadProjection> {
    let mut cursor = 0usize;
    let unit = read_optional_unit_ref(payload, &mut cursor)?;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    let on_ground = read_u8(payload, &mut cursor)? != 0;
    (cursor == payload.len()).then_some(PickedBuildPayloadProjection {
        unit,
        build_pos,
        on_ground,
    })
}

fn decode_picked_unit_payload(payload: &[u8]) -> Option<PickedUnitPayloadProjection> {
    let mut cursor = 0usize;
    let unit = read_optional_unit_ref(payload, &mut cursor)?;
    let target = read_optional_unit_ref(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(PickedUnitPayloadProjection { unit, target })
}

fn decode_unit_entered_payload(payload: &[u8]) -> Option<UnitEnteredPayloadProjection> {
    let mut cursor = 0usize;
    let unit = read_optional_unit_ref(payload, &mut cursor)?;
    let build_pos = read_optional_build_pos(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(UnitEnteredPayloadProjection { unit, build_pos })
}

fn decode_unit_despawn_payload(payload: &[u8]) -> Option<Option<UnitRefProjection>> {
    let mut cursor = 0usize;
    let unit = read_optional_unit_ref(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(unit)
}

fn decode_client_packet_payload(payload: &[u8]) -> Option<(String, String)> {
    let mut cursor = 0usize;
    let packet_type = read_typeio_string_at(payload, &mut cursor)??;
    let contents = read_typeio_string_at(payload, &mut cursor)??;
    (cursor == payload.len()).then_some((packet_type, contents))
}

fn decode_connect_redirect_payload(payload: &[u8]) -> Option<(String, i32)> {
    let mut cursor = 0usize;
    let ip = read_typeio_string_at(payload, &mut cursor)??;
    let port = read_i32(payload, &mut cursor)?;
    ((0..=u16::MAX as i32).contains(&port) && cursor == payload.len()).then_some((ip, port))
}

fn decode_client_binary_packet_payload(payload: &[u8]) -> Option<(String, Vec<u8>)> {
    let mut cursor = 0usize;
    let packet_type = read_typeio_string_at(payload, &mut cursor)??;
    let contents = read_typeio_bytes_at(payload, &mut cursor)?;
    (cursor == payload.len()).then_some((packet_type, contents))
}

fn decode_client_logic_data_payload(payload: &[u8]) -> Option<(String, TypeIoObject)> {
    let mut cursor = 0usize;
    let channel = read_typeio_string_at(payload, &mut cursor)??;
    let (value, consumed) = read_object_prefix(&payload[cursor..]).ok()?;
    cursor = cursor.saturating_add(consumed);
    (cursor == payload.len()).then_some((channel, value))
}

fn decode_set_rule_payload(payload: &[u8]) -> Result<(String, String), String> {
    let mut cursor = 0usize;
    let rule = read_typeio_string_required_at(payload, &mut cursor, "setRule rule")?;
    let json_data = read_typeio_string_required_at(payload, &mut cursor, "setRule json")?;
    if cursor != payload.len() {
        return Err(format!(
            "trailing bytes in setRule payload: consumed {cursor}, actual {}",
            payload.len()
        ));
    }
    Ok((rule, json_data))
}

fn decode_length_prefixed_json_payload(payload: &[u8]) -> Result<String, String> {
    let mut cursor = 0usize;
    let len = read_i32(payload, &mut cursor)
        .ok_or_else(|| "missing length prefix in JSON payload".to_string())?;
    if len < 0 {
        return Err(format!("negative JSON payload length: {len}"));
    }
    let len = usize::try_from(len).map_err(|_| format!("invalid JSON payload length: {len}"))?;
    let bytes = payload.get(cursor..cursor + len).ok_or_else(|| {
        format!(
            "truncated JSON payload body: expected {len} bytes, actual {}",
            payload.len().saturating_sub(cursor)
        )
    })?;
    cursor += len;
    if cursor != payload.len() {
        return Err(format!(
            "trailing bytes in JSON payload: consumed {cursor}, actual {}",
            payload.len()
        ));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|error| format!("invalid UTF-8 JSON payload: {error}"))
}

fn decode_complete_objective_payload(payload: &[u8]) -> Option<i32> {
    let mut cursor = 0usize;
    let index = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(index)
}

fn read_typeio_string_required_at(
    payload: &[u8],
    cursor: &mut usize,
    context: &str,
) -> Result<String, String> {
    match read_typeio_string_at(payload, cursor) {
        Some(Some(value)) => Ok(value),
        Some(None) => Err(format!("{context} string is null")),
        None => Err(format!("{context} string is truncated or invalid UTF-8")),
    }
}

fn decode_sound_payload(payload: &[u8]) -> Option<SoundSummary> {
    let mut cursor = 0usize;
    let raw_sound_id = read_i16(payload, &mut cursor)?;
    let volume = read_f32(payload, &mut cursor)?;
    let pitch = read_f32(payload, &mut cursor)?;
    let pan = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(SoundSummary {
        sound_id: (raw_sound_id != -1).then_some(raw_sound_id),
        volume,
        pitch,
        pan,
    })
}

fn decode_sound_at_payload(payload: &[u8]) -> Option<SoundAtSummary> {
    let mut cursor = 0usize;
    let raw_sound_id = read_i16(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    let volume = read_f32(payload, &mut cursor)?;
    let pitch = read_f32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(SoundAtSummary {
        sound_id: (raw_sound_id != -1).then_some(raw_sound_id),
        x,
        y,
        volume,
        pitch,
    })
}

fn decode_effect_payload(payload: &[u8], allow_trailing: bool) -> Option<EffectSummary> {
    let mut cursor = 0usize;
    let raw_effect_id = read_i16(payload, &mut cursor)?;
    let x = read_f32(payload, &mut cursor)?;
    let y = read_f32(payload, &mut cursor)?;
    let rotation = read_f32(payload, &mut cursor)?;
    let color_rgba = read_u32(payload, &mut cursor)?;
    let data_len = payload.len().saturating_sub(cursor);
    let data_type_tag = if data_len > 0 {
        Some(payload[cursor])
    } else {
        None
    };
    let (data_kind, data_consumed_len, data_object, parse_failed, parse_error) =
        if allow_trailing && data_len > 0 {
            match read_object_prefix(&payload[cursor..]) {
                Ok((object, consumed)) if consumed == data_len => (
                    Some(effect_data_kind_label(&object)),
                    Some(consumed),
                    Some(object),
                    false,
                    None,
                ),
                Ok((object, consumed)) => (
                    Some(effect_data_kind_label(&object)),
                    Some(consumed),
                    Some(object),
                    true,
                    Some(format!(
                        "trailing bytes after effect data object: consumed {consumed} of {data_len}"
                    )),
                ),
                Err(error) => (
                    None,
                    None,
                    None,
                    true,
                    Some(format!("failed to parse effect data object: {error}")),
                ),
            }
        } else {
            (None, None, None, false, None)
        };
    if !allow_trailing && cursor != payload.len() {
        return None;
    }
    Some(EffectSummary {
        effect_id: (raw_effect_id != -1).then_some(raw_effect_id),
        x,
        y,
        rotation,
        color_rgba,
        data_len,
        data_type_tag,
        data_kind,
        data_consumed_len,
        data_object,
        parse_failed,
        parse_error,
    })
}

fn effect_data_kind_label(object: &TypeIoObject) -> String {
    object.effect_summary().kind
}

fn derive_effect_data_semantic(
    object: Option<&TypeIoObject>,
    data_type_tag: Option<u8>,
    parse_failed: bool,
) -> Option<EffectDataSemantic> {
    let object = match object {
        Some(object) => object,
        None if parse_failed => return data_type_tag.map(EffectDataSemantic::OpaqueTypeTag),
        None => return None,
    };

    if let Some(semantic_ref) = object.semantic_ref() {
        let semantic = match semantic_ref {
            TypeIoSemanticRef::Content {
                content_type,
                content_id,
            } => EffectDataSemantic::ContentRaw {
                content_type,
                content_id,
            },
            TypeIoSemanticRef::TechNode {
                content_type,
                content_id,
            } => EffectDataSemantic::TechNodeRaw {
                content_type,
                content_id,
            },
            TypeIoSemanticRef::Unit { unit_id } => EffectDataSemantic::UnitId(unit_id),
            TypeIoSemanticRef::Building { build_pos } => EffectDataSemantic::BuildingPos(build_pos),
        };
        return Some(semantic);
    }

    match object {
        TypeIoObject::Null => Some(EffectDataSemantic::Null),
        TypeIoObject::Int(value) => Some(EffectDataSemantic::Int(*value)),
        TypeIoObject::Long(value) => Some(EffectDataSemantic::Long(*value)),
        TypeIoObject::Float(value) => Some(EffectDataSemantic::FloatBits(value.to_bits())),
        TypeIoObject::String(value) => Some(EffectDataSemantic::String(value.clone())),
        TypeIoObject::IntSeq(values) => Some(EffectDataSemantic::IntSeqLen(values.len())),
        TypeIoObject::Point2 { x, y } => Some(EffectDataSemantic::Point2 { x: *x, y: *y }),
        TypeIoObject::PackedPoint2Array(values) => {
            Some(EffectDataSemantic::PackedPoint2ArrayLen(values.len()))
        }
        TypeIoObject::Bool(value) => Some(EffectDataSemantic::Bool(*value)),
        TypeIoObject::Double(value) => Some(EffectDataSemantic::DoubleBits(value.to_bits())),
        TypeIoObject::LAccess(value) => Some(EffectDataSemantic::LAccess(*value)),
        TypeIoObject::Bytes(values) => Some(EffectDataSemantic::BytesLen(values.len())),
        TypeIoObject::LegacyUnitCommandNull(value) => {
            Some(EffectDataSemantic::LegacyUnitCommandNull(*value))
        }
        TypeIoObject::BoolArray(values) => Some(EffectDataSemantic::BoolArrayLen(values.len())),
        TypeIoObject::Vec2Array(values) => Some(EffectDataSemantic::Vec2ArrayLen(values.len())),
        TypeIoObject::Vec2 { x, y } => Some(EffectDataSemantic::Vec2 {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        }),
        TypeIoObject::Team(id) => Some(EffectDataSemantic::Team(*id)),
        TypeIoObject::IntArray(values) => Some(EffectDataSemantic::IntArrayLen(values.len())),
        TypeIoObject::ObjectArray(values) => Some(EffectDataSemantic::ObjectArrayLen(values.len())),
        TypeIoObject::UnitCommand(id) => Some(EffectDataSemantic::UnitCommand(*id)),
        TypeIoObject::ContentRaw { .. }
        | TypeIoObject::TechNodeRaw { .. }
        | TypeIoObject::BuildingPos(_)
        | TypeIoObject::UnitId(_) => None,
    }
}

struct EffectBusinessProjectionResult {
    projection: Option<EffectBusinessProjection>,
    path: Option<Vec<usize>>,
}

fn derive_effect_business_projection(
    state: &SessionState,
    snapshot_input: &ClientSnapshotInputState,
    object: Option<&TypeIoObject>,
) -> EffectBusinessProjectionResult {
    fn resolve_local_or_entity_unit_projection(
        state: &SessionState,
        snapshot_input: &ClientSnapshotInputState,
        unit_id: i32,
    ) -> Option<EffectBusinessProjection> {
        if let Some(entity) = state.entity_table_projection.by_entity_id.get(&unit_id) {
            return Some(EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::EntityUnitId,
                value: unit_id,
                x_bits: entity.x_bits,
                y_bits: entity.y_bits,
            });
        }
        if snapshot_input.unit_id == Some(unit_id) {
            let position = snapshot_input.position.or_else(|| {
                Some((
                    f32::from_bits(state.world_player_x_bits?),
                    f32::from_bits(state.world_player_y_bits?),
                ))
            })?;
            return Some(EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::LocalUnitId,
                value: unit_id,
                x_bits: position.0.to_bits(),
                y_bits: position.1.to_bits(),
            });
        }
        None
    }

    fn projection_from_semantic_ref(
        state: &SessionState,
        snapshot_input: &ClientSnapshotInputState,
        semantic_ref: TypeIoSemanticRef,
    ) -> Option<EffectBusinessProjection> {
        match semantic_ref {
            TypeIoSemanticRef::Content {
                content_type,
                content_id,
            } => Some(EffectBusinessProjection::ContentRef {
                kind: EffectBusinessContentKind::Content,
                content_type,
                content_id,
            }),
            TypeIoSemanticRef::TechNode {
                content_type,
                content_id,
            } => Some(EffectBusinessProjection::ContentRef {
                kind: EffectBusinessContentKind::TechNode,
                content_type,
                content_id,
            }),
            TypeIoSemanticRef::Building { build_pos } => {
                let (x, y) = world_coords_from_tile_pos(build_pos);
                Some(EffectBusinessProjection::ParentRef {
                    source: EffectBusinessPositionSource::BuildingPos,
                    value: build_pos,
                    x_bits: x.to_bits(),
                    y_bits: y.to_bits(),
                })
            }
            TypeIoSemanticRef::Unit { unit_id } => {
                resolve_local_or_entity_unit_projection(state, snapshot_input, unit_id)
            }
        }
    }

    fn projection_from_position_hint(
        position_hint: &TypeIoEffectPositionHint,
    ) -> EffectBusinessProjection {
        match position_hint {
            TypeIoEffectPositionHint::Point2 { x, y, .. } => {
                let (world_x, world_y) = point2_world_coords(*x, *y);
                EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Point2,
                    x_bits: world_x.to_bits(),
                    y_bits: world_y.to_bits(),
                }
            }
            TypeIoEffectPositionHint::PackedPoint2ArrayFirst { packed_point2, .. } => {
                let (tile_x, tile_y) = unpack_point2(*packed_point2);
                let (world_x, world_y) = point2_world_coords(i32::from(tile_x), i32::from(tile_y));
                EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Point2,
                    x_bits: world_x.to_bits(),
                    y_bits: world_y.to_bits(),
                }
            }
            TypeIoEffectPositionHint::Vec2 { x_bits, y_bits, .. }
            | TypeIoEffectPositionHint::Vec2ArrayFirst { x_bits, y_bits, .. } => {
                EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Vec2,
                    x_bits: *x_bits,
                    y_bits: *y_bits,
                }
            }
        }
    }

    fn projection_from_object(
        state: &SessionState,
        snapshot_input: &ClientSnapshotInputState,
        value: &TypeIoObject,
    ) -> Option<EffectBusinessProjection> {
        if let Some(semantic_ref) = value.semantic_ref() {
            if let Some(projection) =
                projection_from_semantic_ref(state, snapshot_input, semantic_ref)
            {
                return Some(projection);
            }
        }
        match value {
            TypeIoObject::Float(value) => {
                Some(EffectBusinessProjection::FloatValue(value.to_bits()))
            }
            TypeIoObject::Point2 { x, y } => {
                let (world_x, world_y) = point2_world_coords(*x, *y);
                Some(EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Point2,
                    x_bits: world_x.to_bits(),
                    y_bits: world_y.to_bits(),
                })
            }
            TypeIoObject::PackedPoint2Array(values) => values.first().map(|first| {
                let (tile_x, tile_y) = unpack_point2(*first);
                let (world_x, world_y) = point2_world_coords(i32::from(tile_x), i32::from(tile_y));
                EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Point2,
                    x_bits: world_x.to_bits(),
                    y_bits: world_y.to_bits(),
                }
            }),
            TypeIoObject::Vec2 { x, y } => Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Vec2,
                x_bits: x.to_bits(),
                y_bits: y.to_bits(),
            }),
            TypeIoObject::Vec2Array(values) => {
                values
                    .first()
                    .map(|(x, y)| EffectBusinessProjection::WorldPosition {
                        source: EffectBusinessPositionSource::Vec2,
                        x_bits: x.to_bits(),
                        y_bits: y.to_bits(),
                    })
            }
            _ => None,
        }
    }

    fn first_resolvable_dfs_projection(
        state: &SessionState,
        snapshot_input: &ClientSnapshotInputState,
        object: &TypeIoObject,
        max_depth: usize,
        max_nodes: usize,
    ) -> Option<(EffectBusinessProjection, Vec<usize>)> {
        object
            .find_first_dfs_bounded(max_depth, max_nodes, |value| {
                projection_from_object(state, snapshot_input, value).is_some()
            })
            .and_then(|matched| {
                projection_from_object(state, snapshot_input, matched.value)
                    .map(|projection| (projection, matched.path))
            })
    }

    fn normalize_effect_business_path(path: Vec<usize>) -> Option<Vec<usize>> {
        (!path.is_empty()).then_some(path)
    }

    fn pick_leftmost_depth_first_candidate(
        candidates: Vec<(EffectBusinessProjection, Vec<usize>)>,
    ) -> Option<(EffectBusinessProjection, Vec<usize>)> {
        candidates
            .into_iter()
            .min_by(|(_, left_path), (_, right_path)| left_path.cmp(right_path))
    }

    const EFFECT_BUSINESS_MAX_DEPTH: usize = 3;
    const EFFECT_BUSINESS_MAX_NODES: usize = 64;

    let Some(object) = object else {
        return EffectBusinessProjectionResult {
            projection: None,
            path: None,
        };
    };
    let summary = object.effect_summary();
    let unresolved_parent_hint = summary.first_parent_ref.as_ref().is_some_and(|matched| {
        matches!(matched.semantic_ref, TypeIoSemanticRef::Unit { unit_id } if resolve_local_or_entity_unit_projection(state, snapshot_input, unit_id).is_none())
    });

    let mut hint_candidates = Vec::new();
    if let Some(semantic_match) = summary.first_semantic_ref.as_ref() {
        if let Some(projection) =
            projection_from_semantic_ref(state, snapshot_input, semantic_match.semantic_ref)
        {
            hint_candidates.push((projection, semantic_match.path.clone()));
        }
    }
    if let Some(position_hint) = summary.first_position_hint.as_ref() {
        hint_candidates.push((
            projection_from_position_hint(position_hint),
            position_hint.path().to_vec(),
        ));
    }
    if let Some(float_match) = object.find_first_dfs_bounded(
        EFFECT_BUSINESS_MAX_DEPTH,
        EFFECT_BUSINESS_MAX_NODES,
        |value| matches!(value, TypeIoObject::Float(_)),
    ) {
        if let TypeIoObject::Float(value) = float_match.value {
            hint_candidates.push((
                EffectBusinessProjection::FloatValue(value.to_bits()),
                float_match.path,
            ));
        }
    }

    let matched = if unresolved_parent_hint {
        first_resolvable_dfs_projection(
            state,
            snapshot_input,
            object,
            EFFECT_BUSINESS_MAX_DEPTH,
            EFFECT_BUSINESS_MAX_NODES,
        )
    } else {
        pick_leftmost_depth_first_candidate(hint_candidates)
    };

    match matched {
        Some((projection, path)) => EffectBusinessProjectionResult {
            projection: Some(projection),
            path: normalize_effect_business_path(path),
        },
        None => EffectBusinessProjectionResult {
            projection: None,
            path: None,
        },
    }
}

fn world_coords_from_tile_pos(tile_pos: i32) -> (f32, f32) {
    let (tile_x, tile_y) = unpack_point2(tile_pos);
    point2_world_coords(i32::from(tile_x), i32::from(tile_y))
}

fn point2_world_coords(x: i32, y: i32) -> (f32, f32) {
    (x as f32 * 8.0, y as f32 * 8.0)
}

fn decode_debug_status_payload(payload: &[u8]) -> Option<DebugStatusSummary> {
    let mut cursor = 0usize;
    let value = read_i32(payload, &mut cursor)?;
    let last_client_snapshot = read_i32(payload, &mut cursor)?;
    let snapshots_sent = read_i32(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(DebugStatusSummary {
        value,
        last_client_snapshot,
        snapshots_sent,
    })
}

fn decode_trace_info_payload(payload: &[u8]) -> Option<TraceInfoSummary> {
    let mut cursor = 0usize;
    let raw_player_id = read_i32(payload, &mut cursor)?;
    let ip = read_typeio_string_at(payload, &mut cursor)?;
    let uuid = read_typeio_string_at(payload, &mut cursor)?;
    let locale = read_typeio_string_at(payload, &mut cursor)?;
    let modded = read_u8(payload, &mut cursor)? != 0;
    let mobile = read_u8(payload, &mut cursor)? != 0;
    let times_joined = read_i32(payload, &mut cursor)?;
    let times_kicked = read_i32(payload, &mut cursor)?;
    let ips = read_typeio_string_array_at(payload, &mut cursor)?;
    let names = read_typeio_string_array_at(payload, &mut cursor)?;
    (cursor == payload.len()).then_some(TraceInfoSummary {
        player_id: (raw_player_id != -1).then_some(raw_player_id),
        ip,
        uuid,
        locale,
        modded,
        mobile,
        times_joined,
        times_kicked,
        ips,
        names,
    })
}

fn decode_remove_queue_block_payload(payload: &[u8]) -> Option<(i32, i32, bool)> {
    if payload.len() < 9 {
        return None;
    }
    let x = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let y = i32::from_be_bytes(payload[4..8].try_into().ok()?);
    let breaking = payload[8] != 0;
    Some((x, y, breaking))
}

#[derive(Debug, Clone, PartialEq)]
struct CommandBuildingSummary {
    buildings: Vec<i32>,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct CommandUnitsSummary {
    unit_ids: Vec<i32>,
    build_target: Option<i32>,
    unit_target: Option<UnitRefProjection>,
    x: f32,
    y: f32,
    queue_command: bool,
    final_batch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetUnitCommandSummary {
    unit_ids: Vec<i32>,
    command_id: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestItemSummary {
    build_pos: Option<i32>,
    item_id: Option<i16>,
    amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuChooseSummary {
    menu_id: i32,
    option: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuDialogSummary {
    menu_id: i32,
    title: Option<String>,
    message: Option<String>,
    option_rows: usize,
    first_row_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetItemSummary {
    build_pos: Option<i32>,
    item_id: Option<i16>,
    amount: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct SetLiquidSummary {
    build_pos: Option<i32>,
    liquid_id: Option<i16>,
    amount: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetItemsSummary {
    build_pos: Option<i32>,
    stack_count: usize,
    first_item_id: Option<i16>,
    first_amount: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetLiquidsSummary {
    build_pos: Option<i32>,
    stack_count: usize,
    first_liquid_id: Option<i16>,
    first_amount_bits: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetTileItemsSummary {
    item_id: Option<i16>,
    amount: i32,
    position_count: usize,
    first_position: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetTileLiquidsSummary {
    liquid_id: Option<i16>,
    amount_bits: u32,
    position_count: usize,
    first_position: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
struct WorldLabelSummary {
    message: Option<String>,
    label_id: Option<i32>,
    duration: f32,
    world_x: f32,
    world_y: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CreateMarkerSummary {
    marker_id: i32,
    json_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateMarkerSummary {
    marker_id: i32,
    control: u8,
    p1_bits: u64,
    p2_bits: u64,
    p3_bits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateMarkerTextSummary {
    marker_id: i32,
    control: u8,
    fetch: bool,
    text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateMarkerTextureSummary {
    marker_id: i32,
    texture_kind: u8,
    texture_kind_name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct InfoToastSummary {
    message: Option<String>,
    duration: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WarningToastSummary {
    unicode: i32,
    text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct InfoPopupSummary {
    popup_id: Option<String>,
    message: Option<String>,
    duration: f32,
    align: i32,
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextInputResultSummary {
    text_input_id: i32,
    text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextInputSummary {
    text_input_id: i32,
    title: Option<String>,
    message: Option<String>,
    text_length: i32,
    default_text: Option<String>,
    numeric: bool,
    allow_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetUnitStanceSummary {
    unit_ids: Vec<i32>,
    stance_id: Option<u8>,
    enable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitBuildingControlSelectSummary {
    target: Option<UnitRefProjection>,
    build_pos: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
struct BeginPlaceSummary {
    x: i32,
    y: i32,
    block_id: Option<i16>,
    rotation: i32,
    team_id: u8,
    config_kind: u8,
    config_kind_name: &'static str,
    config_consumed_len: usize,
    config_object: TypeIoObject,
    builder_kind: u8,
    builder_value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BeginBreakSummary {
    x: i32,
    y: i32,
    team_id: u8,
    builder_kind: u8,
    builder_value: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct TileConfigSummary {
    build_pos: Option<i32>,
    config_kind: Option<u8>,
    config_kind_name: Option<String>,
    config_consumed_len: Option<usize>,
    config_object: Option<TypeIoObject>,
    parse_failed: bool,
    parse_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct ConstructFinishSummary {
    tile_pos: i32,
    block_id: Option<i16>,
    builder_kind: u8,
    builder_value: i32,
    rotation: u8,
    team_id: u8,
    config_kind: u8,
    config_kind_name: &'static str,
    config_consumed_len: usize,
    config_object: TypeIoObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeconstructFinishSummary {
    tile_pos: i32,
    block_id: Option<i16>,
    builder_kind: u8,
    builder_value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EntitySnapshotEnvelopeSummary {
    amount: u16,
    body_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct LocalPlayerSyncParseResult {
    sync: Option<mdt_world::EntityPlayerSyncSnapshot>,
    ambiguous: bool,
    parseable_match_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocalPlayerSyncApplicationResult {
    applied: bool,
    target_player_id: Option<i32>,
    used_projection_fallback: bool,
    ambiguous: bool,
    parseable_match_count: usize,
}

impl LocalPlayerSyncApplicationResult {
    const fn notarget() -> Self {
        Self {
            applied: false,
            target_player_id: None,
            used_projection_fallback: false,
            ambiguous: false,
            parseable_match_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EntityPlayerSyncRow {
    entity_id: i32,
    sync: mdt_world::EntityPlayerSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityAlphaSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityAlphaSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityMechSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityMechSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityMissileSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityMissileSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityPayloadSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityPayloadSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityBuildingTetherPayloadSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityBuildingTetherPayloadSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityFireSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityFireSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityPuddleSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityPuddleSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityWeatherStateSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityWeatherStateSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct EntityWorldLabelSyncRow {
    entity_id: i32,
    class_id: u8,
    sync: mdt_world::EntityWorldLabelSyncSnapshot,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EntityPlayerSyncRowsParseError {
    ParseableRowsExceedAmount { rows: usize, amount: u16 },
}

impl fmt::Display for EntityPlayerSyncRowsParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseableRowsExceedAmount { rows, amount } => {
                write!(
                    f,
                    "entity_snapshot_parseable_rows_exceed_amount:{rows}/{amount}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockSnapshotExtraEntrySummary {
    build_pos: i32,
    block_id: i16,
    health_bits: Option<u32>,
    rotation: Option<u8>,
    team_id: Option<u8>,
    io_version: Option<u8>,
    enabled: Option<bool>,
    module_bitmask: Option<u8>,
    time_scale_bits: Option<u32>,
    time_scale_duration_bits: Option<u32>,
    last_disabler_pos: Option<i32>,
    legacy_consume_connected: Option<bool>,
    efficiency: Option<u8>,
    optional_efficiency: Option<u8>,
    visible_flags: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoadedWorldBlockSnapshotEntryCollection {
    Complete(Vec<BlockSnapshotExtraEntrySummary>),
    Partial {
        entries: Vec<BlockSnapshotExtraEntrySummary>,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SoundSummary {
    sound_id: Option<i16>,
    volume: f32,
    pitch: f32,
    pan: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SoundAtSummary {
    sound_id: Option<i16>,
    x: f32,
    y: f32,
    volume: f32,
    pitch: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct EffectSummary {
    effect_id: Option<i16>,
    x: f32,
    y: f32,
    rotation: f32,
    color_rgba: u32,
    data_len: usize,
    data_type_tag: Option<u8>,
    data_kind: Option<String>,
    data_consumed_len: Option<usize>,
    data_object: Option<TypeIoObject>,
    parse_failed: bool,
    parse_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceInfoSummary {
    player_id: Option<i32>,
    ip: Option<String>,
    uuid: Option<String>,
    locale: Option<String>,
    modded: bool,
    mobile: bool,
    times_joined: i32,
    times_kicked: i32,
    ips: Vec<String>,
    names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DebugStatusSummary {
    value: i32,
    last_client_snapshot: i32,
    snapshots_sent: i32,
}

fn decode_begin_place_payload(payload: &[u8]) -> Option<BeginPlaceSummary> {
    if payload.len() < 21 {
        return None;
    }
    let builder_kind = payload[0];
    let builder_value = i32::from_be_bytes(payload[1..5].try_into().ok()?);
    let raw_block_id = i16::from_be_bytes(payload[5..7].try_into().ok()?);
    let team_id = payload[7];
    let x = i32::from_be_bytes(payload[8..12].try_into().ok()?);
    let y = i32::from_be_bytes(payload[12..16].try_into().ok()?);
    let rotation = i32::from_be_bytes(payload[16..20].try_into().ok()?);
    let config_payload = &payload[20..];
    let (config_object, config_consumed_len) = read_object_prefix(config_payload).ok()?;
    if config_consumed_len != config_payload.len() {
        return None;
    }
    let config_kind = config_payload[0];
    let config_kind_name = config_object.kind();
    Some(BeginPlaceSummary {
        x,
        y,
        block_id: (raw_block_id != -1).then_some(raw_block_id),
        rotation,
        team_id,
        config_kind,
        config_kind_name,
        config_consumed_len,
        config_object,
        builder_kind,
        builder_value,
    })
}

fn decode_begin_break_payload(payload: &[u8]) -> Option<BeginBreakSummary> {
    if payload.len() < 14 {
        return None;
    }
    let builder_kind = payload[0];
    let builder_value = i32::from_be_bytes(payload[1..5].try_into().ok()?);
    let team_id = payload[5];
    let x = i32::from_be_bytes(payload[6..10].try_into().ok()?);
    let y = i32::from_be_bytes(payload[10..14].try_into().ok()?);
    Some(BeginBreakSummary {
        x,
        y,
        team_id,
        builder_kind,
        builder_value,
    })
}

fn decode_tile_config_payload(payload: &[u8]) -> Option<TileConfigSummary> {
    if payload.len() < 4 {
        return None;
    }
    let raw_build_pos = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let build_pos = (raw_build_pos != -1).then_some(raw_build_pos);
    let config_payload = &payload[4..];
    let config_kind = config_payload.first().copied();
    if config_payload.is_empty() {
        return Some(TileConfigSummary {
            build_pos,
            config_kind: None,
            config_kind_name: None,
            config_consumed_len: None,
            config_object: None,
            parse_failed: true,
            parse_error: Some("missing tileConfig TypeIO payload".to_string()),
        });
    }

    match read_object_prefix(config_payload) {
        Ok((config_object, config_consumed_len)) => {
            let config_kind_name = Some(config_object.kind().to_string());
            if config_consumed_len == config_payload.len() {
                Some(TileConfigSummary {
                    build_pos,
                    config_kind,
                    config_kind_name,
                    config_consumed_len: Some(config_consumed_len),
                    config_object: Some(config_object),
                    parse_failed: false,
                    parse_error: None,
                })
            } else {
                Some(TileConfigSummary {
                    build_pos,
                    config_kind,
                    config_kind_name,
                    config_consumed_len: Some(config_consumed_len),
                    config_object: Some(config_object),
                    parse_failed: true,
                    parse_error: Some(format!(
                        "trailing bytes after TypeIO object: consumed {config_consumed_len} of {}",
                        config_payload.len()
                    )),
                })
            }
        }
        Err(error) => Some(TileConfigSummary {
            build_pos,
            config_kind,
            config_kind_name: config_kind.map(typeio_kind_name_for_type_id),
            config_consumed_len: None,
            config_object: None,
            parse_failed: true,
            parse_error: Some(error.to_string()),
        }),
    }
}

fn typeio_kind_name_for_type_id(type_id: u8) -> String {
    match type_id {
        0 => "null".to_string(),
        1 => "int".to_string(),
        2 => "long".to_string(),
        3 => "float".to_string(),
        4 => "string".to_string(),
        5 => "Content(raw)".to_string(),
        6 => "IntSeq".to_string(),
        7 => "Point2".to_string(),
        8 => "Point2[]".to_string(),
        9 => "TechNode(raw)".to_string(),
        10 => "bool".to_string(),
        11 => "double".to_string(),
        12 => "Building(raw)".to_string(),
        13 => "LAccess".to_string(),
        14 => "byte[]".to_string(),
        15 => "LegacyUnitCommandNull".to_string(),
        16 => "boolean[]".to_string(),
        17 => "Unit(raw)".to_string(),
        18 => "Vec2[]".to_string(),
        19 => "Vec2".to_string(),
        20 => "Team".to_string(),
        21 => "int[]".to_string(),
        22 => "object[]".to_string(),
        23 => "UnitCommand".to_string(),
        _ => format!("unsupported({type_id})"),
    }
}

fn marker_control_name(control: u8) -> Option<&'static str> {
    MARKER_CONTROL_NAMES.get(control as usize).copied()
}

fn decode_construct_finish_payload(payload: &[u8]) -> Option<ConstructFinishSummary> {
    if payload.len() < 14 {
        return None;
    }
    let tile_pos = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let raw_block_id = i16::from_be_bytes(payload[4..6].try_into().ok()?);
    let builder_kind = payload[6];
    let builder_value = i32::from_be_bytes(payload[7..11].try_into().ok()?);
    let rotation = payload[11];
    let team_id = payload[12];
    let config_payload = &payload[13..];
    let (config_object, config_consumed_len) = read_object_prefix(config_payload).ok()?;
    if config_consumed_len != config_payload.len() {
        return None;
    }
    let config_kind = config_payload[0];
    let config_kind_name = config_object.kind();
    Some(ConstructFinishSummary {
        tile_pos,
        block_id: (raw_block_id != -1).then_some(raw_block_id),
        builder_kind,
        builder_value,
        rotation,
        team_id,
        config_kind,
        config_kind_name,
        config_consumed_len,
        config_object,
    })
}

fn decode_deconstruct_finish_payload(payload: &[u8]) -> Option<DeconstructFinishSummary> {
    if payload.len() < 11 {
        return None;
    }
    let tile_pos = i32::from_be_bytes(payload[0..4].try_into().ok()?);
    let raw_block_id = i16::from_be_bytes(payload[4..6].try_into().ok()?);
    let builder_kind = payload[6];
    let builder_value = i32::from_be_bytes(payload[7..11].try_into().ok()?);
    Some(DeconstructFinishSummary {
        tile_pos,
        block_id: (raw_block_id != -1).then_some(raw_block_id),
        builder_kind,
        builder_value,
    })
}

fn decode_entity_snapshot_envelope_header(payload: &[u8]) -> Option<EntitySnapshotEnvelopeSummary> {
    if payload.len() < 4 {
        return None;
    }
    let amount = u16::from_be_bytes(payload[0..2].try_into().unwrap());
    let body_len = u16::from_be_bytes(payload[2..4].try_into().unwrap()) as usize;
    Some(EntitySnapshotEnvelopeSummary { amount, body_len })
}

fn validate_entity_snapshot_envelope(
    payload: &[u8],
    summary: EntitySnapshotEnvelopeSummary,
) -> Result<(), String> {
    let expected_total = 4usize.saturating_add(summary.body_len);
    if payload.len() < expected_total {
        return Err(format!(
            "entity_snapshot_body_len_out_of_range:{}/{}",
            summary.body_len,
            payload.len().saturating_sub(4)
        ));
    }
    if payload.len() != expected_total {
        return Err(format!(
            "entity_snapshot_trailing_bytes:{expected_total}/{}",
            payload.len()
        ));
    }
    Ok(())
}

fn unpack_point2(value: i32) -> (i16, i16) {
    let raw = value as u32;
    let x = ((raw >> 16) as u16) as i16;
    let y = (raw as u16) as i16;
    (x, y)
}

fn try_parse_local_player_sync_from_entity_snapshot(
    player_rows: &[EntityPlayerSyncRow],
    player_id: i32,
) -> LocalPlayerSyncParseResult {
    let mut matches = player_rows.iter().filter(|row| row.entity_id == player_id);
    let Some(first) = matches.next() else {
        return LocalPlayerSyncParseResult {
            sync: None,
            ambiguous: false,
            parseable_match_count: 0,
        };
    };
    let remaining_matches = matches.count();
    if remaining_matches > 0 {
        return LocalPlayerSyncParseResult {
            sync: None,
            ambiguous: true,
            parseable_match_count: remaining_matches.saturating_add(1),
        };
    }
    LocalPlayerSyncParseResult {
        sync: Some(first.sync.clone()),
        ambiguous: false,
        parseable_match_count: 1,
    }
}

#[cfg(test)]
fn try_parse_player_sync_rows_from_entity_snapshot(payload: &[u8]) -> Vec<EntityPlayerSyncRow> {
    parse_player_sync_rows_from_entity_snapshot(payload).unwrap_or_default()
}

fn try_parse_alpha_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityAlphaSyncRow> {
    parse_alpha_sync_rows_from_entity_snapshot_prefix(payload).unwrap_or_default()
}

fn try_parse_mech_sync_rows_from_entity_snapshot_prefix(payload: &[u8]) -> Vec<EntityMechSyncRow> {
    parse_mech_sync_rows_from_entity_snapshot_prefix(payload).unwrap_or_default()
}

fn try_parse_missile_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityMissileSyncRow> {
    parse_missile_sync_rows_from_entity_snapshot_prefix(payload).unwrap_or_default()
}

#[cfg(test)]
fn try_parse_payload_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityPayloadSyncRow> {
    try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, None)
}

fn try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Vec<EntityPayloadSyncRow> {
    parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, content_header)
        .unwrap_or_default()
}

#[cfg(test)]
fn try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityBuildingTetherPayloadSyncRow> {
    try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
        payload, None,
    )
}

fn try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Vec<EntityBuildingTetherPayloadSyncRow> {
    parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
        payload,
        content_header,
    )
    .unwrap_or_default()
}

#[cfg(test)]
fn try_parse_fire_sync_rows_from_entity_snapshot_prefix(payload: &[u8]) -> Vec<EntityFireSyncRow> {
    try_parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, None)
}

fn try_parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Vec<EntityFireSyncRow> {
    parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, content_header)
        .unwrap_or_default()
}

#[cfg(test)]
fn try_parse_puddle_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityPuddleSyncRow> {
    try_parse_puddle_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, None)
}

fn try_parse_puddle_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Vec<EntityPuddleSyncRow> {
    parse_puddle_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, content_header)
        .unwrap_or_default()
}

#[cfg(test)]
fn try_parse_weather_state_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityWeatherStateSyncRow> {
    try_parse_weather_state_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, None)
}

fn try_parse_weather_state_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Vec<EntityWeatherStateSyncRow> {
    parse_weather_state_sync_rows_from_entity_snapshot_prefix_with_content_header(
        payload,
        content_header,
    )
    .unwrap_or_default()
}

#[cfg(test)]
fn try_parse_world_label_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Vec<EntityWorldLabelSyncRow> {
    try_parse_world_label_sync_rows_from_entity_snapshot_prefix_with_content_header(payload, None)
}

fn try_parse_world_label_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Vec<EntityWorldLabelSyncRow> {
    parse_world_label_sync_rows_from_entity_snapshot_prefix_with_content_header(
        payload,
        content_header,
    )
    .unwrap_or_default()
}

fn parse_entity_payload_sync_bytes_with_optional_content_header(
    bytes: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<(mdt_world::EntityPayloadSyncSnapshot, usize), String> {
    match content_header {
        Some(content_header) => {
            parse_entity_payload_sync_bytes_with_content_header(content_header, bytes)
        }
        None => parse_entity_payload_sync_bytes(bytes),
    }
}

fn parse_entity_building_tether_payload_sync_bytes_with_optional_content_header(
    bytes: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<(mdt_world::EntityBuildingTetherPayloadSyncSnapshot, usize), String> {
    match content_header {
        Some(content_header) => {
            parse_entity_building_tether_payload_sync_bytes_with_content_header(
                content_header,
                bytes,
            )
        }
        None => parse_entity_building_tether_payload_sync_bytes(bytes),
    }
}

fn parse_player_sync_rows_from_entity_snapshot(
    payload: &[u8],
) -> Result<Vec<EntityPlayerSyncRow>, EntityPlayerSyncRowsParseError> {
    const PLAYER_ENTITY_CLASS_ID: u8 = 12;

    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }
    let max_rows = usize::from(amount);

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };
    let Some(last_start) = body.len().checked_sub(5) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut last_end = 0usize;
    for start in 0..=last_start {
        if start < last_end {
            continue;
        }
        let entity_id = i32::from_be_bytes(body[start..start + 4].try_into().unwrap());
        if body[start + 4] != PLAYER_ENTITY_CLASS_ID {
            continue;
        }
        if let Ok((sync, consumed)) = parse_entity_player_sync_bytes(&body[start + 5..]) {
            let end = start.saturating_add(5).saturating_add(consumed);
            rows.push(EntityPlayerSyncRow {
                entity_id,
                sync,
                start,
                end,
            });
            if rows.len() > max_rows {
                return Err(EntityPlayerSyncRowsParseError::ParseableRowsExceedAmount {
                    rows: rows.len(),
                    amount,
                });
            }
            last_end = end;
        }
    }
    Ok(rows)
}

fn parse_alpha_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Result<Vec<EntityAlphaSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityAlphaSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_mech_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Result<Vec<EntityMechSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityMechSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_missile_sync_rows_from_entity_snapshot_prefix(
    payload: &[u8],
) -> Result<Vec<EntityMissileSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityMissileSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<Vec<EntityPayloadSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) =
                    parse_entity_payload_sync_bytes_with_optional_content_header(
                        &body[cursor + 5..],
                        content_header,
                    )
                    .map_err(|error| format!("entity_snapshot_known_prefix_payload:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityPayloadSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<Vec<EntityBuildingTetherPayloadSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_payload_sync_bytes_with_optional_content_header(
                    &body[cursor + 5..],
                    content_header,
                )
                .map_err(|error| format!("entity_snapshot_known_prefix_payload:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) =
                    parse_entity_building_tether_payload_sync_bytes_with_optional_content_header(
                        &body[cursor + 5..],
                        content_header,
                    )
                    .map_err(|error| {
                        format!("entity_snapshot_known_prefix_tether_payload:{error}")
                    })?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityBuildingTetherPayloadSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<Vec<EntityFireSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_payload_sync_bytes_with_optional_content_header(
                    &body[cursor + 5..],
                    content_header,
                )
                .map_err(|error| format!("entity_snapshot_known_prefix_payload:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) =
                    parse_entity_building_tether_payload_sync_bytes_with_optional_content_header(
                        &body[cursor + 5..],
                        content_header,
                    )
                    .map_err(|error| {
                        format!("entity_snapshot_known_prefix_tether_payload:{error}")
                    })?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if FIRE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_fire_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_fire:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityFireSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_puddle_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<Vec<EntityPuddleSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_payload_sync_bytes_with_optional_content_header(
                    &body[cursor + 5..],
                    content_header,
                )
                .map_err(|error| format!("entity_snapshot_known_prefix_payload:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) =
                    parse_entity_building_tether_payload_sync_bytes_with_optional_content_header(
                        &body[cursor + 5..],
                        content_header,
                    )
                    .map_err(|error| {
                        format!("entity_snapshot_known_prefix_tether_payload:{error}")
                    })?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if FIRE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_fire_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_fire:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PUDDLE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_puddle_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_puddle:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityPuddleSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_weather_state_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<Vec<EntityWeatherStateSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_payload_sync_bytes_with_optional_content_header(
                    &body[cursor + 5..],
                    content_header,
                )
                .map_err(|error| format!("entity_snapshot_known_prefix_payload:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) =
                    parse_entity_building_tether_payload_sync_bytes_with_optional_content_header(
                        &body[cursor + 5..],
                        content_header,
                    )
                    .map_err(|error| {
                        format!("entity_snapshot_known_prefix_tether_payload:{error}")
                    })?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if FIRE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_fire_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_fire:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PUDDLE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_puddle_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_puddle:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if WEATHER_STATE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_weather_state_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| {
                    format!("entity_snapshot_known_prefix_weather_state:{error}")
                })?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityWeatherStateSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn parse_world_label_sync_rows_from_entity_snapshot_prefix_with_content_header(
    payload: &[u8],
    content_header: Option<&[ContentHeaderEntry]>,
) -> Result<Vec<EntityWorldLabelSyncRow>, String> {
    if payload.len() < 4 {
        return Ok(Vec::new());
    }

    let amount = u16::from_be_bytes([payload[0], payload[1]]);
    if amount == 0 {
        return Ok(Vec::new());
    }

    let Some(body_len_bytes) = payload.get(2..4) else {
        return Ok(Vec::new());
    };
    let body_len = u16::from_be_bytes(body_len_bytes.try_into().unwrap()) as usize;
    let Some(body) = payload.get(4..4 + body_len) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    let mut cursor = 0usize;
    let max_rows = usize::from(amount);
    while cursor.saturating_add(5) <= body.len() && rows.len() < max_rows {
        let entity_id = i32::from_be_bytes(body[cursor..cursor + 4].try_into().unwrap());
        let class_id = body[cursor + 4];
        match class_id {
            12 => {
                let (_, consumed) = parse_entity_player_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_player:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_alpha_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_alpha:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_mech_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_mech:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_missile_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_missile:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_payload_sync_bytes_with_optional_content_header(
                    &body[cursor + 5..],
                    content_header,
                )
                .map_err(|error| format!("entity_snapshot_known_prefix_payload:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) =
                    parse_entity_building_tether_payload_sync_bytes_with_optional_content_header(
                        &body[cursor + 5..],
                        content_header,
                    )
                    .map_err(|error| {
                        format!("entity_snapshot_known_prefix_tether_payload:{error}")
                    })?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if FIRE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_fire_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_fire:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if PUDDLE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_puddle_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_puddle:{error}"))?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if WEATHER_STATE_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (_, consumed) = parse_entity_weather_state_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| {
                        format!("entity_snapshot_known_prefix_weather_state:{error}")
                    })?;
                cursor = cursor.saturating_add(5).saturating_add(consumed);
            }
            _ if WORLD_LABEL_ENTITY_CLASS_IDS.contains(&class_id) => {
                let (sync, consumed) = parse_entity_world_label_sync_bytes(&body[cursor + 5..])
                    .map_err(|error| format!("entity_snapshot_known_prefix_world_label:{error}"))?;
                let end = cursor.saturating_add(5).saturating_add(consumed);
                rows.push(EntityWorldLabelSyncRow {
                    entity_id,
                    class_id,
                    sync,
                    start: cursor,
                    end,
                });
                cursor = end;
            }
            _ => break,
        }
    }

    Ok(rows)
}

fn try_read_typeio_string(payload: &[u8]) -> Option<String> {
    let mut cursor = 0usize;
    read_typeio_string_at(payload, &mut cursor)?
}

fn read_typeio_string_at(payload: &[u8], cursor: &mut usize) -> Option<Option<String>> {
    let exists = read_u8(payload, cursor)?;
    if exists == 0 {
        return Some(None);
    }

    let len = read_u16(payload, cursor)? as usize;
    let bytes = payload.get(*cursor..*cursor + len)?;
    *cursor += len;
    Some(Some(String::from_utf8(bytes.to_vec()).ok()?))
}

fn read_typeio_bytes_at(payload: &[u8], cursor: &mut usize) -> Option<Vec<u8>> {
    let len = read_u16(payload, cursor)? as usize;
    let bytes = payload.get(*cursor..*cursor + len)?;
    *cursor += len;
    Some(bytes.to_vec())
}

fn read_typeio_string_array_at(payload: &[u8], cursor: &mut usize) -> Option<Vec<String>> {
    let len = read_u8(payload, cursor)? as usize;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(read_typeio_string_at(payload, cursor)??);
    }
    Some(values)
}

fn try_read_chat_message_payload(payload: &[u8]) -> Option<ChatMessagePayload> {
    let mut cursor = 0usize;
    let message = read_typeio_string_at(payload, &mut cursor)??;
    let unformatted = read_typeio_string_at(payload, &mut cursor)?;
    let sender_entity_id = match read_i32(payload, &mut cursor)? {
        -1 => None,
        id => Some(id),
    };

    Some(ChatMessagePayload {
        message,
        unformatted,
        sender_entity_id,
    })
}

fn try_read_build_health_update_payload(payload: &[u8]) -> Option<BuildHealthUpdateSummary> {
    let mut cursor = 0usize;
    let len = read_i32(payload, &mut cursor)?;
    if len < 0 {
        return None;
    }
    let len = usize::try_from(len).ok()?;
    if len % 2 != 0 {
        return None;
    }

    let pair_count = len / 2;
    let mut pairs = Vec::with_capacity(pair_count);
    let mut first_build_pos = None;
    let mut first_health_bits = None;
    for _ in 0..pair_count {
        let build_pos = read_i32(payload, &mut cursor)?;
        let health_bits = read_u32(payload, &mut cursor)?;
        if first_build_pos.is_none() {
            first_build_pos = Some(build_pos);
            first_health_bits = Some(health_bits);
        }
        pairs.push(BuildHealthPair {
            build_pos,
            health_bits,
        });
    }

    if cursor != payload.len() {
        return None;
    }

    Some(BuildHealthUpdateSummary {
        pair_count,
        first_build_pos,
        first_health_bits,
        pairs,
    })
}

fn read_u8(payload: &[u8], cursor: &mut usize) -> Option<u8> {
    let value = *payload.get(*cursor)?;
    *cursor += 1;
    Some(value)
}

fn read_u16(payload: &[u8], cursor: &mut usize) -> Option<u16> {
    let bytes: [u8; 2] = payload.get(*cursor..*cursor + 2)?.try_into().ok()?;
    *cursor += 2;
    Some(u16::from_be_bytes(bytes))
}

fn read_i16(payload: &[u8], cursor: &mut usize) -> Option<i16> {
    let bytes: [u8; 2] = payload.get(*cursor..*cursor + 2)?.try_into().ok()?;
    *cursor += 2;
    Some(i16::from_be_bytes(bytes))
}

fn read_i32(payload: &[u8], cursor: &mut usize) -> Option<i32> {
    let bytes: [u8; 4] = payload.get(*cursor..*cursor + 4)?.try_into().ok()?;
    *cursor += 4;
    Some(i32::from_be_bytes(bytes))
}

fn read_i64(payload: &[u8], cursor: &mut usize) -> Option<i64> {
    let bytes: [u8; 8] = payload.get(*cursor..*cursor + 8)?.try_into().ok()?;
    *cursor += 8;
    Some(i64::from_be_bytes(bytes))
}

fn read_u32(payload: &[u8], cursor: &mut usize) -> Option<u32> {
    let bytes: [u8; 4] = payload.get(*cursor..*cursor + 4)?.try_into().ok()?;
    *cursor += 4;
    Some(u32::from_be_bytes(bytes))
}

fn read_f32(payload: &[u8], cursor: &mut usize) -> Option<f32> {
    Some(f32::from_bits(read_u32(payload, cursor)?))
}

fn decode_ping_time_payload(payload: &[u8]) -> Option<u64> {
    let mut cursor = 0;
    let sent_at_ms = read_i64(payload, &mut cursor)?;
    if sent_at_ms < 0 || cursor != payload.len() {
        return None;
    }
    Some(sent_at_ms as u64)
}

fn encode_ping_time_payload(time_ms: u64) -> Vec<u8> {
    i64::try_from(time_ms)
        .unwrap_or(i64::MAX)
        .to_be_bytes()
        .to_vec()
}

fn encode_admin_request_payload(
    other_player_id: i32,
    action_ordinal: u8,
    params: &TypeIoObject,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&other_player_id.to_be_bytes());
    payload.push(action_ordinal);
    write_typeio_object(&mut payload, params);
    payload
}

#[cfg(test)]
#[cfg(test)]
fn encode_debug_status_payload(
    value: i32,
    last_client_snapshot: i32,
    snapshots_sent: i32,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&value.to_be_bytes());
    payload.extend_from_slice(&last_client_snapshot.to_be_bytes());
    payload.extend_from_slice(&snapshots_sent.to_be_bytes());
    payload
}

fn encode_building_payload(build_pos: Option<i32>) -> Vec<u8> {
    build_pos.unwrap_or(-1).to_be_bytes().to_vec()
}

fn encode_menu_choose_payload(menu_id: i32, option: i32) -> Vec<u8> {
    [menu_id.to_be_bytes(), option.to_be_bytes()].concat()
}

fn encode_text_input_result_payload(text_input_id: i32, text: Option<&str>) -> Vec<u8> {
    let mut payload = text_input_id.to_be_bytes().to_vec();
    payload.extend_from_slice(&encode_optional_typeio_string_payload(text));
    payload
}

fn encode_request_item_payload(
    build_pos: Option<i32>,
    item_id: Option<i16>,
    amount: i32,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(10);
    payload.extend_from_slice(&encode_building_payload(build_pos));
    payload.extend_from_slice(&item_id.unwrap_or(-1).to_be_bytes());
    payload.extend_from_slice(&amount.to_be_bytes());
    payload
}

#[cfg(test)]
fn encode_player_prefixed_payload(player_id: i32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&player_id.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
fn encode_team_payload(team_id: u8) -> Vec<u8> {
    vec![team_id]
}

#[cfg(test)]
fn encode_content_payload(content_type: u8, content_id: i16) -> Vec<u8> {
    let mut payload = Vec::with_capacity(3);
    payload.push(content_type);
    payload.extend_from_slice(&content_id.to_be_bytes());
    payload
}

fn encode_unit_payload(target: ClientUnitRef) -> Vec<u8> {
    let mut payload = Vec::with_capacity(5);
    match target {
        ClientUnitRef::None => {
            payload.push(0);
            payload.extend_from_slice(&0i32.to_be_bytes());
        }
        ClientUnitRef::Block(tile_pos) => {
            payload.push(1);
            payload.extend_from_slice(&tile_pos.to_be_bytes());
        }
        ClientUnitRef::Standard(unit_id) => {
            payload.push(2);
            payload.extend_from_slice(&unit_id.to_be_bytes());
        }
    }
    payload
}

fn encode_begin_break_payload(builder: ClientUnitRef, team_id: u8, x: i32, y: i32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(14);
    payload.extend_from_slice(&encode_unit_payload(builder));
    payload.push(team_id);
    payload.extend_from_slice(&x.to_be_bytes());
    payload.extend_from_slice(&y.to_be_bytes());
    payload
}

fn encode_begin_place_payload(
    builder: ClientUnitRef,
    block_id: Option<i16>,
    team_id: u8,
    x: i32,
    y: i32,
    rotation: i32,
    place_config: &TypeIoObject,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&encode_unit_payload(builder));
    payload.extend_from_slice(&block_id.unwrap_or(-1).to_be_bytes());
    payload.push(team_id);
    payload.extend_from_slice(&x.to_be_bytes());
    payload.extend_from_slice(&y.to_be_bytes());
    payload.extend_from_slice(&rotation.to_be_bytes());
    write_typeio_object(&mut payload, place_config);
    payload
}

fn encode_building_bool_payload(build_pos: Option<i32>, value: bool) -> Vec<u8> {
    let mut payload = encode_building_payload(build_pos);
    payload.push(u8::from(value));
    payload
}

fn encode_delete_plans_payload(positions: &[i32]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + positions.len() * 4);
    payload.extend_from_slice(&(positions.len() as i16).to_be_bytes());
    for pos in positions {
        payload.extend_from_slice(&pos.to_be_bytes());
    }
    payload
}

fn encode_unit_building_payload(target: ClientUnitRef, build_pos: Option<i32>) -> Vec<u8> {
    let mut payload = Vec::with_capacity(9);
    payload.extend_from_slice(&encode_unit_payload(target));
    payload.extend_from_slice(&encode_building_payload(build_pos));
    payload
}

fn encode_command_building_payload(buildings: &[i32], x: f32, y: f32) -> Vec<u8> {
    let mut payload = encode_delete_plans_payload(buildings);
    payload.extend_from_slice(&encode_two_f32_payload(x, y));
    payload
}

fn encode_command_units_payload(
    unit_ids: &[i32],
    build_target: Option<i32>,
    unit_target: ClientUnitRef,
    pos_target: Option<(f32, f32)>,
    queue_command: bool,
    final_batch: bool,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(21 + unit_ids.len() * 4);
    payload.extend_from_slice(&encode_delete_plans_payload(unit_ids));
    payload.extend_from_slice(&encode_building_payload(build_target));
    payload.extend_from_slice(&encode_unit_payload(unit_target));
    let (x, y) = pos_target.unwrap_or((0.0, 0.0));
    payload.extend_from_slice(&encode_two_f32_payload(x, y));
    payload.push(u8::from(queue_command));
    payload.push(u8::from(final_batch));
    payload
}

fn encode_set_unit_command_payload(unit_ids: &[i32], command_id: Option<u8>) -> Vec<u8> {
    let mut payload = encode_delete_plans_payload(unit_ids);
    payload.push(command_id.unwrap_or(u8::MAX));
    payload
}

fn encode_set_unit_stance_payload(
    unit_ids: &[i32],
    stance_id: Option<u8>,
    enable: bool,
) -> Vec<u8> {
    let mut payload = encode_delete_plans_payload(unit_ids);
    payload.push(stance_id.unwrap_or(u8::MAX));
    payload.push(u8::from(enable));
    payload
}

#[cfg(test)]
fn encode_optional_item_payload(item_id: Option<i16>) -> Vec<u8> {
    item_id.unwrap_or(-1).to_be_bytes().to_vec()
}

#[cfg(test)]
fn encode_optional_entity_payload(entity_id: Option<i32>) -> Vec<u8> {
    entity_id.unwrap_or(-1).to_be_bytes().to_vec()
}

#[cfg(test)]
fn encode_take_items_payload(
    build_pos: Option<i32>,
    item_id: Option<i16>,
    amount: i32,
    to: ClientUnitRef,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(15);
    payload.extend_from_slice(&encode_building_payload(build_pos));
    payload.extend_from_slice(&encode_optional_item_payload(item_id));
    payload.extend_from_slice(&amount.to_be_bytes());
    payload.extend_from_slice(&encode_unit_payload(to));
    payload
}

#[cfg(test)]
fn encode_transfer_item_to_payload(
    unit: ClientUnitRef,
    item_id: Option<i16>,
    amount: i32,
    x: f32,
    y: f32,
    build_pos: Option<i32>,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(19);
    payload.extend_from_slice(&encode_unit_payload(unit));
    payload.extend_from_slice(&encode_optional_item_payload(item_id));
    payload.extend_from_slice(&amount.to_be_bytes());
    payload.extend_from_slice(&encode_two_f32_payload(x, y));
    payload.extend_from_slice(&encode_building_payload(build_pos));
    payload
}

#[cfg(test)]
fn encode_transfer_item_to_unit_payload(
    item_id: Option<i16>,
    x: f32,
    y: f32,
    to_entity_id: Option<i32>,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(14);
    payload.extend_from_slice(&encode_optional_item_payload(item_id));
    payload.extend_from_slice(&encode_two_f32_payload(x, y));
    payload.extend_from_slice(&encode_optional_entity_payload(to_entity_id));
    payload
}

#[cfg(test)]
fn encode_payload_dropped_payload(unit: ClientUnitRef, x: f32, y: f32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(13);
    payload.extend_from_slice(&encode_unit_payload(unit));
    payload.extend_from_slice(&encode_two_f32_payload(x, y));
    payload
}

#[cfg(test)]
fn encode_picked_build_payload(
    unit: ClientUnitRef,
    build_pos: Option<i32>,
    on_ground: bool,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(10);
    payload.extend_from_slice(&encode_unit_payload(unit));
    payload.extend_from_slice(&encode_building_payload(build_pos));
    payload.push(u8::from(on_ground));
    payload
}

#[cfg(test)]
fn encode_picked_unit_payload(unit: ClientUnitRef, target: ClientUnitRef) -> Vec<u8> {
    let mut payload = Vec::with_capacity(10);
    payload.extend_from_slice(&encode_unit_payload(unit));
    payload.extend_from_slice(&encode_unit_payload(target));
    payload
}

#[cfg(test)]
fn encode_unit_entered_payload(unit: ClientUnitRef, build_pos: Option<i32>) -> Vec<u8> {
    let mut payload = Vec::with_capacity(9);
    payload.extend_from_slice(&encode_unit_payload(unit));
    payload.extend_from_slice(&encode_building_payload(build_pos));
    payload
}

fn encode_tile_config_payload(build_pos: Option<i32>, value: &TypeIoObject) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16);
    write_typeio_int(&mut payload, build_pos.unwrap_or(-1));
    write_typeio_object(&mut payload, value);
    payload
}

fn encode_single_f32_payload(value: f32) -> Vec<u8> {
    value.to_bits().to_be_bytes().to_vec()
}

fn encode_two_f32_payload(x: f32, y: f32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8);
    write_f32(&mut payload, x);
    write_f32(&mut payload, y);
    payload
}

fn decode_kick_reason_payload(payload: &[u8]) -> (Option<i32>, Option<u64>) {
    decode_kick_reason_payload_with_i32(payload)
        .or_else(|| decode_kick_reason_payload_with_u8(payload))
        .unwrap_or((None, None))
}

fn decode_kick_reason_payload_with_i32(payload: &[u8]) -> Option<(Option<i32>, Option<u64>)> {
    let mut cursor = 0usize;
    let reason = read_i32(payload, &mut cursor)?;
    let duration_ms = if cursor == payload.len() {
        None
    } else {
        let duration = read_i64(payload, &mut cursor)?;
        (duration >= 0).then_some(duration as u64)
    };
    if cursor != payload.len() {
        return None;
    }
    Some((Some(reason), duration_ms))
}

fn decode_kick_reason_payload_with_u8(payload: &[u8]) -> Option<(Option<i32>, Option<u64>)> {
    let mut cursor = 0usize;
    let reason = i32::from(read_u8(payload, &mut cursor)?);
    let duration_ms = if cursor == payload.len() {
        None
    } else {
        let duration = read_i64(payload, &mut cursor)?;
        (duration >= 0).then_some(duration as u64)
    };
    if cursor != payload.len() {
        return None;
    }
    Some((Some(reason), duration_ms))
}

#[cfg(test)]
fn encode_client_packet_payload(packet_type: &str, contents: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&encode_typeio_string_payload(packet_type));
    payload.extend_from_slice(&encode_typeio_string_payload(contents));
    payload
}

#[cfg(test)]
fn encode_client_binary_packet_payload(packet_type: &str, contents: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&encode_typeio_string_payload(packet_type));
    payload.extend_from_slice(&encode_typeio_bytes_payload(contents));
    payload
}

#[cfg(test)]
fn encode_client_logic_data_payload(channel: &str, value: &TypeIoObject) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&encode_typeio_string_payload(channel));
    write_typeio_object(&mut payload, value);
    payload
}

fn encode_typeio_string_payload(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut payload = Vec::with_capacity(1 + 2 + bytes.len());
    payload.push(1);
    payload.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    payload.extend_from_slice(bytes);
    payload
}

fn encode_typeio_bytes_payload(bytes: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + bytes.len());
    payload.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    payload.extend_from_slice(bytes);
    payload
}

#[cfg(test)]
fn encode_length_prefixed_utf8_payload(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut payload = Vec::with_capacity(4 + bytes.len());
    payload.extend_from_slice(&(bytes.len() as i32).to_be_bytes());
    payload.extend_from_slice(bytes);
    payload
}

#[cfg(test)]
fn encode_sound_payload(sound_id: i16, volume: f32, pitch: f32, pan: f32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(14);
    payload.extend_from_slice(&sound_id.to_be_bytes());
    write_f32(&mut payload, volume);
    write_f32(&mut payload, pitch);
    write_f32(&mut payload, pan);
    payload
}

#[cfg(test)]
fn encode_sound_at_payload(sound_id: i16, x: f32, y: f32, volume: f32, pitch: f32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(18);
    payload.extend_from_slice(&sound_id.to_be_bytes());
    write_f32(&mut payload, x);
    write_f32(&mut payload, y);
    write_f32(&mut payload, volume);
    write_f32(&mut payload, pitch);
    payload
}

#[cfg(test)]
fn encode_effect_payload(
    effect_id: i16,
    x: f32,
    y: f32,
    rotation: f32,
    color_rgba: u32,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(18);
    payload.extend_from_slice(&effect_id.to_be_bytes());
    write_f32(&mut payload, x);
    write_f32(&mut payload, y);
    write_f32(&mut payload, rotation);
    payload.extend_from_slice(&color_rgba.to_be_bytes());
    payload
}

fn encode_optional_typeio_string_payload(text: Option<&str>) -> Vec<u8> {
    match text {
        Some(text) => encode_typeio_string_payload(text),
        None => vec![0],
    }
}

#[cfg(test)]
fn encode_info_popup_payload(
    message: Option<&str>,
    popup_id: Option<&str>,
    duration: f32,
    align: i32,
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
) -> Vec<u8> {
    let mut payload = encode_optional_typeio_string_payload(message);
    if let Some(popup_id) = popup_id {
        payload.extend_from_slice(&encode_optional_typeio_string_payload(Some(popup_id)));
    }
    payload.extend_from_slice(&duration.to_bits().to_be_bytes());
    payload.extend_from_slice(&align.to_be_bytes());
    payload.extend_from_slice(&top.to_be_bytes());
    payload.extend_from_slice(&left.to_be_bytes());
    payload.extend_from_slice(&bottom.to_be_bytes());
    payload.extend_from_slice(&right.to_be_bytes());
    payload
}

#[cfg(test)]
fn encode_typeio_string_matrix_payload(rows: &[&[&str]]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(rows.len() as u8);
    for row in rows {
        payload.push(row.len() as u8);
        for value in *row {
            payload.extend_from_slice(&encode_typeio_string_payload(value));
        }
    }
    payload
}

#[cfg(test)]
fn encode_typeio_string_array_payload(values: &[&str]) -> Vec<u8> {
    let mut payload =
        Vec::with_capacity(1 + values.iter().map(|value| value.len() + 3).sum::<usize>());
    payload.push(values.len() as u8);
    for value in values {
        payload.extend_from_slice(&encode_typeio_string_payload(value));
    }
    payload
}

#[cfg(test)]
fn encode_trace_info_payload(
    player_id: i32,
    ip: Option<&str>,
    uuid: Option<&str>,
    locale: Option<&str>,
    modded: bool,
    mobile: bool,
    times_joined: i32,
    times_kicked: i32,
    ips: &[&str],
    names: &[&str],
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&player_id.to_be_bytes());
    payload.extend_from_slice(&encode_optional_typeio_string_payload(ip));
    payload.extend_from_slice(&encode_optional_typeio_string_payload(uuid));
    payload.extend_from_slice(&encode_optional_typeio_string_payload(locale));
    payload.push(u8::from(modded));
    payload.push(u8::from(mobile));
    payload.extend_from_slice(&times_joined.to_be_bytes());
    payload.extend_from_slice(&times_kicked.to_be_bytes());
    payload.extend_from_slice(&encode_typeio_string_array_payload(ips));
    payload.extend_from_slice(&encode_typeio_string_array_payload(names));
    payload
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingClientPacket {
    packet_id: u8,
    transport: ClientPacketTransport,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredInboundPriority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeferredInboundPacket {
    packet_id: u8,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct InboundPacketRef<'a> {
    raw_bytes: &'a [u8],
    packet_id: u8,
    payload: &'a [u8],
}

type ClientPacketHandler = Box<dyn FnMut(&str)>;
type ClientBinaryPacketHandler = Box<dyn FnMut(&[u8])>;
type ClientLogicDataHandler = Box<dyn FnMut(ClientLogicDataTransport, &TypeIoObject)>;

#[derive(Default)]
struct ClientPacketHandlerRegistry {
    handlers: BTreeMap<String, Vec<ClientPacketHandler>>,
}

impl ClientPacketHandlerRegistry {
    fn add<F>(&mut self, packet_type: impl Into<String>, handler: F)
    where
        F: FnMut(&str) + 'static,
    {
        self.handlers
            .entry(packet_type.into())
            .or_default()
            .push(Box::new(handler));
    }

    fn dispatch(&mut self, packet_type: &str, contents: &str) {
        let Some(handlers) = self.handlers.get_mut(packet_type) else {
            return;
        };
        for handler in handlers {
            handler(contents);
        }
    }
}

impl fmt::Debug for ClientPacketHandlerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = self
            .handlers
            .iter()
            .map(|(packet_type, handlers)| (packet_type, handlers.len()))
            .collect::<Vec<_>>();
        f.debug_struct("ClientPacketHandlerRegistry")
            .field("handler_counts", &counts)
            .finish()
    }
}

#[derive(Default)]
struct ClientBinaryPacketHandlerRegistry {
    handlers: BTreeMap<String, Vec<ClientBinaryPacketHandler>>,
}

impl ClientBinaryPacketHandlerRegistry {
    fn add<F>(&mut self, packet_type: impl Into<String>, handler: F)
    where
        F: FnMut(&[u8]) + 'static,
    {
        self.handlers
            .entry(packet_type.into())
            .or_default()
            .push(Box::new(handler));
    }

    fn dispatch(&mut self, packet_type: &str, contents: &[u8]) {
        let Some(handlers) = self.handlers.get_mut(packet_type) else {
            return;
        };
        for handler in handlers {
            handler(contents);
        }
    }
}

impl fmt::Debug for ClientBinaryPacketHandlerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = self
            .handlers
            .iter()
            .map(|(packet_type, handlers)| (packet_type, handlers.len()))
            .collect::<Vec<_>>();
        f.debug_struct("ClientBinaryPacketHandlerRegistry")
            .field("handler_counts", &counts)
            .finish()
    }
}

#[derive(Default)]
struct ClientLogicDataHandlerRegistry {
    handlers: BTreeMap<String, Vec<ClientLogicDataHandler>>,
}

impl ClientLogicDataHandlerRegistry {
    fn add<F>(&mut self, channel: impl Into<String>, handler: F)
    where
        F: FnMut(ClientLogicDataTransport, &TypeIoObject) + 'static,
    {
        self.handlers
            .entry(channel.into())
            .or_default()
            .push(Box::new(handler));
    }

    fn dispatch(
        &mut self,
        channel: &str,
        transport: ClientLogicDataTransport,
        value: &TypeIoObject,
    ) {
        let Some(handlers) = self.handlers.get_mut(channel) else {
            return;
        };
        for handler in handlers {
            handler(transport, value);
        }
    }
}

impl fmt::Debug for ClientLogicDataHandlerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = self
            .handlers
            .iter()
            .map(|(channel, handlers)| (channel, handlers.len()))
            .collect::<Vec<_>>();
        f.debug_struct("ClientLogicDataHandlerRegistry")
            .field("handler_counts", &counts)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatMessagePayload {
    message: String,
    unformatted: Option<String>,
    sender_entity_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildHealthUpdateSummary {
    pair_count: usize,
    first_build_pos: Option<i32>,
    first_health_bits: Option<u32>,
    pairs: Vec<BuildHealthPair>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap_flow::encode_world_stream_packets;
    use crate::session_state::AppliedStateSnapshot;
    use mdt_protocol::{decode_framework_message, decode_packet, encode_packet, FrameworkMessage};
    use mdt_remote::read_remote_manifest;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

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

    fn sample_snapshot_packet(key: &str) -> Vec<u8> {
        let text = include_str!("../../../tests/src/test/resources/snapshot-goldens.txt");
        let hex = text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("missing snapshot golden key: {key}"));
        decode_hex_text(hex)
    }

    fn synthetic_mech_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&15.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(2);
        bytes.push(0);
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_missile_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&240.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(2);
        bytes.push(0);
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&12.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&39i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_payload_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(2);
        bytes.push(0);
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_payload_sync_bytes_with_build_payload(
        block_id: i16,
        conveyor_chunk: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(1);
        bytes.push(1);
        bytes.extend_from_slice(&block_id.to_be_bytes());
        bytes.push(conveyor_chunk[0]);
        bytes.extend_from_slice(&conveyor_chunk[1..]);
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_payload_campaign_unit_payload_body() -> Vec<u8> {
        decode_hex_text(
            "00030042f600000900000000003f80000000000000000000004316000000ffffffff0200000000000000000000000000000000000000000000000000000000000000000000000000000000000100230100000000000000000000000000000000",
        )
    }

    fn synthetic_oct_revision_one_unit_payload_body() -> Vec<u8> {
        let mut unit_body = Vec::new();
        unit_body.extend_from_slice(&1i16.to_be_bytes());
        unit_body.extend_from_slice(&12.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        unit_body.push(0);
        unit_body.extend_from_slice(&7i32.to_be_bytes());
        unit_body.extend_from_slice(&3.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&5.0f64.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        unit_body.push(1);
        unit_body.push(0);
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.extend_from_slice(&180.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&10.0f32.to_bits().to_be_bytes());
        unit_body.push(0);
        unit_body.extend_from_slice(&0i16.to_be_bytes());
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.push(2);
        unit_body.extend_from_slice(&26i16.to_be_bytes());
        unit_body.extend_from_slice(&64.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&96.0f32.to_bits().to_be_bytes());
        unit_body
    }

    fn synthetic_quad_revision_six_unit_payload_body() -> Vec<u8> {
        let mut unit_body = Vec::new();
        unit_body.extend_from_slice(&6i16.to_be_bytes());
        unit_body.push(0);
        unit_body.extend_from_slice(&27.0f32.to_bits().to_be_bytes());
        unit_body.push(0);
        unit_body.extend_from_slice(&11i32.to_be_bytes());
        unit_body.extend_from_slice(&0.75f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&9.0f64.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&140.0f32.to_bits().to_be_bytes());
        unit_body.push(0);
        unit_body.extend_from_slice(&12345i32.to_be_bytes());
        unit_body.push(0);
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.extend_from_slice(&45.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&20.0f32.to_bits().to_be_bytes());
        unit_body.push(1);
        unit_body.extend_from_slice(&1i16.to_be_bytes());
        unit_body.extend_from_slice(&30i32.to_be_bytes());
        unit_body.extend_from_slice(&0i32.to_be_bytes());
        unit_body.push(3);
        unit_body.extend_from_slice(&23i16.to_be_bytes());
        unit_body.push(1);
        unit_body.extend_from_slice(&2.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&(-3.0f32).to_bits().to_be_bytes());
        unit_body.extend_from_slice(&128.0f32.to_bits().to_be_bytes());
        unit_body.extend_from_slice(&256.0f32.to_bits().to_be_bytes());
        unit_body
    }

    fn java_unit_payload_golden_body(sample_name: &str) -> (u8, Vec<u8>) {
        let class_id_key = format!("unitPayload.{sample_name}.classId");
        let body_hex_key = format!("unitPayload.{sample_name}.bodyHex");
        let mut class_id = None;
        let mut body_hex = None;

        for line in
            include_str!("../../../tests/src/test/resources/unit-payload-goldens.txt").lines()
        {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                _ if key == class_id_key => {
                    class_id = Some(u8::from_str_radix(value.trim(), 16).unwrap());
                }
                _ if key == body_hex_key => {
                    body_hex = Some(value.trim().to_string());
                }
                _ => {}
            }
        }

        (
            class_id.unwrap_or_else(|| panic!("missing classId for {sample_name}")),
            decode_hex_text(
                body_hex
                    .unwrap_or_else(|| panic!("missing bodyHex for {sample_name}"))
                    .as_str(),
            ),
        )
    }

    fn real_java_payload_entity_rows() -> Vec<(i32, u8, Vec<u8>)> {
        [
            (777, 5u8, "alpha"),
            (778, 23u8, "quad"),
            (779, 26u8, "oct"),
            (780, 5u8, "mega"),
            (781, 23u8, "quell-missile"),
            (782, 26u8, "flare"),
            (783, 5u8, "mono"),
            (784, 23u8, "poly"),
            (785, 26u8, "mace"),
            (786, 5u8, "stell"),
            (787, 23u8, "elude"),
            (788, 26u8, "latum"),
            (789, 5u8, "spiroct"),
            (790, 23u8, "vanquish"),
        ]
        .into_iter()
        .map(|(entity_id, outer_class_id, sample_name)| {
            let (inner_class_id, unit_body) = java_unit_payload_golden_body(sample_name);
            (
                entity_id,
                outer_class_id,
                build_entity_snapshot_row(
                    entity_id,
                    outer_class_id,
                    &synthetic_payload_sync_bytes_with_unit_payload(inner_class_id, &unit_body),
                ),
            )
        })
        .collect()
    }

    fn real_java_building_tether_payload_entity_row() -> (i32, u8, Vec<u8>) {
        let (inner_class_id, unit_body) = java_unit_payload_golden_body("manifold");
        (
            888,
            36,
            build_entity_snapshot_row(
                888,
                36,
                &synthetic_building_tether_payload_sync_bytes_with_unit_payload(
                    inner_class_id,
                    &unit_body,
                ),
            ),
        )
    }

    fn synthetic_payload_sync_bytes_with_unit_payload(
        unit_class_id: u8,
        unit_body: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(1);
        bytes.push(0);
        bytes.push(unit_class_id);
        bytes.extend_from_slice(unit_body);
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_building_tether_payload_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&12345i32.to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(2);
        bytes.push(0);
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_building_tether_payload_sync_bytes_with_build_payload(
        block_id: i16,
        conveyor_chunk: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&12345i32.to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(1);
        bytes.push(1);
        bytes.extend_from_slice(&block_id.to_be_bytes());
        bytes.push(conveyor_chunk[0]);
        bytes.extend_from_slice(&conveyor_chunk[1..]);
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_building_tether_payload_sync_bytes_with_unit_payload(
        unit_class_id: u8,
        unit_body: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&123.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&12345i32.to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&7i32.to_be_bytes());
        bytes.extend_from_slice(&1.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0f64.to_bits().to_be_bytes());
        bytes.extend_from_slice(&150.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(1);
        bytes.push(0);
        bytes.push(unit_class_id);
        bytes.extend_from_slice(unit_body);
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&90.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.0f32.to_bits().to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0i16.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&35i16.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_fire_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&240.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&12345i32.to_be_bytes());
        bytes.extend_from_slice(&12.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_puddle_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&6.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&3i16.to_be_bytes());
        bytes.extend_from_slice(&12345i32.to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_weather_state_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1.25f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.75f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&600.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0.5f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&2i16.to_be_bytes());
        bytes.extend_from_slice(&3.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&(-4.0f32).to_bits().to_be_bytes());
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn synthetic_world_label_sync_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(3);
        bytes.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        bytes.push(1);
        let text = b"hello world";
        bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(text);
        bytes.extend_from_slice(&40.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&60.0f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&120.0f32.to_bits().to_be_bytes());
        bytes
    }

    fn build_entity_snapshot_row(entity_id: i32, class_id: u8, sync_bytes: &[u8]) -> Vec<u8> {
        let mut row = Vec::with_capacity(5 + sync_bytes.len());
        row.extend_from_slice(&entity_id.to_be_bytes());
        row.push(class_id);
        row.extend_from_slice(sync_bytes);
        row
    }

    fn build_entity_snapshot_payload(rows: &[Vec<u8>]) -> Vec<u8> {
        let body_len: usize = rows.iter().map(Vec::len).sum();
        let mut payload = Vec::with_capacity(4 + body_len);
        payload.extend_from_slice(&u16::try_from(rows.len()).unwrap().to_be_bytes());
        payload.extend_from_slice(&u16::try_from(body_len).unwrap().to_be_bytes());
        for row in rows {
            payload.extend_from_slice(row);
        }
        payload
    }

    fn real_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/remote/remote-manifest-v1.json")
    }

    #[test]
    fn drives_connect_world_stream_and_snapshot_chain() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        assert!(session.state().connect_packet_sent);
        assert_eq!(session.state().connect_payload_len, connect.payload.len());

        let begin_event = session.ingest_packet_bytes(&begin_packet).unwrap();
        assert_eq!(
            begin_event,
            ClientSessionEvent::WorldStreamStarted {
                stream_id: 7,
                total_bytes: compressed_world_stream.len(),
            }
        );
        assert_eq!(
            session.state().world_stream_expected_len,
            compressed_world_stream.len()
        );

        for chunk_packet in &chunk_packets[..chunk_packets.len() - 1] {
            let event = session.ingest_packet_bytes(chunk_packet).unwrap();
            match event {
                ClientSessionEvent::WorldStreamChunk {
                    stream_id,
                    received_bytes,
                    total_bytes,
                } => {
                    assert_eq!(stream_id, 7);
                    assert!(received_bytes < total_bytes);
                }
                other => panic!("expected chunk progress event, got {other:?}"),
            }
        }

        let ready_event = session
            .ingest_packet_bytes(chunk_packets.last().unwrap())
            .unwrap();
        assert_eq!(
            ready_event,
            ClientSessionEvent::WorldStreamReady {
                stream_id: 7,
                map_width: 8,
                map_height: 8,
                player_id: 7,
                ready_to_enter_world: true,
            }
        );
        assert!(session.state().world_stream_loaded);
        assert_eq!(session.state().world_map_width, 8);
        assert_eq!(session.state().world_map_height, 8);
        assert_eq!(
            session.state().world_display_title.as_deref(),
            Some("Golden Deterministic")
        );
        assert_eq!(
            session
                .loaded_world_bundle()
                .map(|bundle| bundle.world.width),
            Some(8)
        );
        assert_eq!(
            session
                .loaded_world_state()
                .map(|state| state.player().team_id),
            Some(1)
        );
        assert_eq!(
            session.state().building_table_projection.by_build_pos.len(),
            6
        );
        assert_eq!(
            session.state().building_table_projection.last_update,
            Some(crate::session_state::BuildingProjectionUpdateKind::WorldBaseline)
        );

        let snapshot_packet = encode_packet(122, &[1, 2, 3, 4], false).unwrap();
        let snapshot_event = session.ingest_packet_bytes(&snapshot_packet).unwrap();
        assert_eq!(
            snapshot_event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::StateSnapshot)
        );
        assert_eq!(session.stats().packets_seen, 1);
        assert_eq!(session.stats().snapshot_packets_seen, 1);
        assert!(session.state().seen_state_snapshot);
    }

    #[test]
    fn world_stream_ready_seeds_building_table_projection_from_world_bundle() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let bundle = session.loaded_world_bundle().unwrap();
        assert_eq!(
            session.state().building_table_projection.by_build_pos.len(),
            bundle.world.building_centers.len()
        );

        let first_center = &bundle.world.building_centers[0];
        let first_build_pos = pack_point2(first_center.x as i32, first_center.y as i32);
        let projection = session
            .state()
            .building_table_projection
            .by_build_pos
            .get(&first_build_pos)
            .unwrap();

        assert_eq!(
            projection.block_id,
            Some(i16::from_be_bytes(first_center.block_id.to_be_bytes()))
        );
        assert_eq!(
            projection.rotation,
            Some(first_center.building.base.rotation)
        );
        assert_eq!(projection.team_id, Some(first_center.building.base.team_id));
        assert_eq!(
            projection.io_version,
            first_center.building.base.save_version
        );
        assert_eq!(
            projection.health_bits,
            Some(first_center.building.base.health_bits)
        );
        assert_eq!(projection.enabled, first_center.building.base.enabled);
        assert_eq!(
            projection.last_update,
            crate::session_state::BuildingProjectionUpdateKind::WorldBaseline
        );
    }

    #[test]
    fn schedules_keepalive_and_client_snapshot_after_world_ready() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 1000,
            client_snapshot_interval_ms: 1000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let connect = session
            .prepare_connect_packet(&sample_connect_payload())
            .unwrap();
        assert!(!connect.encoded_packet.is_empty());

        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        assert!(session.state().ready_to_enter_world);
        let expected_unit_id =
            i32::try_from(session.state().world_player_unit_value.unwrap()).unwrap();
        let expected_x_bits = session.state().world_player_x_bits.unwrap();
        let expected_y_bits = session.state().world_player_y_bits.unwrap();
        let connect_confirm = session.prepare_connect_confirm_packet().unwrap();
        let connect_confirm = connect_confirm.expect("world-ready session should confirm connect");
        let connect_confirm_packet = decode_packet(&connect_confirm).unwrap();
        assert_eq!(connect_confirm_packet.packet_id, 29);
        assert!(connect_confirm_packet.payload.is_empty());
        assert!(session.state().connect_confirm_sent);
        let expected_ping_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "ping")
            .unwrap()
            .packet_id;

        let first_actions = session.advance_time(1_000).unwrap();
        assert_eq!(first_actions.len(), 3);
        assert_eq!(
            first_actions[0],
            ClientSessionAction::SendFramework {
                message: FrameworkMessage::KeepAlive,
                bytes: vec![0xfe, 2],
            }
        );
        match &first_actions[1] {
            ClientSessionAction::SendPacket {
                packet_id,
                transport,
                bytes,
            } => {
                assert_eq!(*packet_id, expected_ping_packet_id);
                assert_eq!(*transport, ClientPacketTransport::Tcp);
                let decoded = decode_packet(bytes).unwrap();
                assert_eq!(decoded.packet_id, expected_ping_packet_id);
                assert_eq!(decoded.payload, 1_000i64.to_be_bytes());
            }
            other => panic!("expected ping send action, got {other:?}"),
        }
        match &first_actions[2] {
            ClientSessionAction::SendPacket {
                packet_id,
                transport,
                bytes,
            } => {
                assert_eq!(*packet_id, 24);
                assert_eq!(*transport, ClientPacketTransport::Udp);
                let decoded = decode_packet(bytes).unwrap();
                assert_eq!(decoded.packet_id, 24);
                assert_eq!(&decoded.payload[0..4], &1i32.to_be_bytes());
                assert_eq!(&decoded.payload[4..8], &expected_unit_id.to_be_bytes());
                assert_eq!(decoded.payload[8], 0);
                assert_eq!(&decoded.payload[9..13], &expected_x_bits.to_be_bytes());
                assert_eq!(&decoded.payload[13..17], &expected_y_bits.to_be_bytes());
            }
            other => panic!("expected snapshot send action, got {other:?}"),
        }
        assert_eq!(session.state().sent_keepalive_count, 1);
        assert_eq!(session.state().sent_client_snapshot_count, 1);
        assert_eq!(session.state().last_sent_client_snapshot_id, Some(1));

        let second_actions = session.advance_time(2_000).unwrap();
        assert_eq!(second_actions.len(), 3);
        assert_eq!(
            decode_framework_message(match &second_actions[0] {
                ClientSessionAction::SendFramework { bytes, .. } => bytes,
                other => panic!("expected framework action, got {other:?}"),
            })
            .unwrap(),
            FrameworkMessage::KeepAlive
        );
        match &second_actions[1] {
            ClientSessionAction::SendPacket {
                packet_id,
                transport,
                bytes,
            } => {
                assert_eq!(*packet_id, expected_ping_packet_id);
                assert_eq!(*transport, ClientPacketTransport::Tcp);
                let decoded = decode_packet(bytes).unwrap();
                assert_eq!(decoded.payload, 2_000i64.to_be_bytes());
            }
            other => panic!("expected ping send action, got {other:?}"),
        }
        match &second_actions[2] {
            ClientSessionAction::SendPacket {
                transport, bytes, ..
            } => {
                assert_eq!(*transport, ClientPacketTransport::Udp);
                let decoded = decode_packet(bytes).unwrap();
                assert_eq!(&decoded.payload[0..4], &2i32.to_be_bytes());
            }
            other => panic!("expected snapshot send action, got {other:?}"),
        }
    }

    #[test]
    fn multi_tick_movement_and_ack_packets_prune_local_plans() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 500,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let input = session.snapshot_input_mut();
        input.unit_id = Some(42);
        input.dead = false;
        input.position = Some((10.0, 20.0));
        input.pointer = Some((10.0, 20.0));
        input.plans = Some(vec![
            ClientBuildPlan {
                tile: (100, 99),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (5, 6),
                breaking: true,
                block_id: None,
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (8, 9),
                breaking: true,
                block_id: None,
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
        ]);

        let snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::ClientSnapshot.method_name())
            .unwrap()
            .packet_id;
        let set_position_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setPosition")
            .unwrap()
            .packet_id;
        let remove_queue_block_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeQueueBlock")
            .unwrap()
            .packet_id;
        let construct_finish_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "constructFinish")
            .unwrap()
            .packet_id;
        let deconstruct_finish_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "deconstructFinish")
            .unwrap()
            .packet_id;

        let snapshot_payload_at = |actions: &[ClientSessionAction], tick_ms: u64| -> Vec<u8> {
            actions
                .iter()
                .find_map(|action| match action {
                    ClientSessionAction::SendPacket {
                        packet_id,
                        transport,
                        bytes,
                    } if *packet_id == snapshot_packet_id
                        && *transport == ClientPacketTransport::Udp =>
                    {
                        Some(decode_packet(bytes).unwrap().payload)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing snapshot payload at tick={tick_ms}"))
        };

        let tick1 = session.advance_time(500).unwrap();
        let payload1 = snapshot_payload_at(&tick1, 500);
        assert_eq!(&payload1[0..4], &1i32.to_be_bytes());
        assert_eq!(&payload1[9..13], &10.0f32.to_bits().to_be_bytes());
        assert_eq!(&payload1[13..17], &20.0f32.to_bits().to_be_bytes());
        assert_eq!(&payload1[55..59], &3i32.to_be_bytes());

        let mut set_pos_payload = Vec::new();
        set_pos_payload.extend_from_slice(&64.0f32.to_bits().to_be_bytes());
        set_pos_payload.extend_from_slice(&96.0f32.to_bits().to_be_bytes());
        let set_pos_packet =
            encode_packet(set_position_packet_id, &set_pos_payload, false).unwrap();
        let set_pos_event = session.ingest_packet_bytes(&set_pos_packet).unwrap();
        assert_eq!(
            set_pos_event,
            ClientSessionEvent::PlayerPositionUpdated { x: 64.0, y: 96.0 }
        );

        let mut remove_payload = Vec::new();
        remove_payload.extend_from_slice(&5i32.to_be_bytes());
        remove_payload.extend_from_slice(&6i32.to_be_bytes());
        remove_payload.push(1);
        let remove_packet =
            encode_packet(remove_queue_block_packet_id, &remove_payload, false).unwrap();
        let remove_event = session.ingest_packet_bytes(&remove_packet).unwrap();
        assert_eq!(
            remove_event,
            ClientSessionEvent::RemoveQueueBlock {
                x: 5,
                y: 6,
                breaking: true,
                removed_local_plan: true,
            }
        );
        assert_eq!(
            session.snapshot_input().plans.as_ref().map(Vec::len),
            Some(2)
        );

        let tick2 = session.advance_time(1_000).unwrap();
        let payload2 = snapshot_payload_at(&tick2, 1_000);
        assert_eq!(&payload2[0..4], &2i32.to_be_bytes());
        assert_eq!(&payload2[9..13], &64.0f32.to_bits().to_be_bytes());
        assert_eq!(&payload2[13..17], &96.0f32.to_bits().to_be_bytes());
        assert_eq!(&payload2[55..59], &2i32.to_be_bytes());

        let mut construct_payload = Vec::new();
        construct_payload.extend_from_slice(&pack_point2(100, 99).to_be_bytes());
        construct_payload.extend_from_slice(&0x0101i16.to_be_bytes());
        construct_payload.push(2);
        construct_payload.extend_from_slice(&42i32.to_be_bytes());
        construct_payload.push(0);
        construct_payload.push(1);
        construct_payload.push(0);
        let construct_packet =
            encode_packet(construct_finish_packet_id, &construct_payload, false).unwrap();
        let construct_event = session.ingest_packet_bytes(&construct_packet).unwrap();
        assert_eq!(
            construct_event,
            ClientSessionEvent::ConstructFinish {
                tile_pos: pack_point2(100, 99),
                block_id: Some(0x0101),
                builder_kind: 2,
                builder_value: 42,
                rotation: 0,
                team_id: 1,
                config_kind: 0,
                removed_local_plan: true,
            }
        );
        assert_eq!(
            session.snapshot_input().plans.as_ref().map(Vec::len),
            Some(1)
        );

        let tick3 = session.advance_time(1_500).unwrap();
        let payload3 = snapshot_payload_at(&tick3, 1_500);
        assert_eq!(&payload3[0..4], &3i32.to_be_bytes());
        assert_eq!(&payload3[55..59], &1i32.to_be_bytes());

        let mut deconstruct_payload = Vec::new();
        deconstruct_payload.extend_from_slice(&pack_point2(8, 9).to_be_bytes());
        deconstruct_payload.extend_from_slice(&0x0101i16.to_be_bytes());
        deconstruct_payload.push(2);
        deconstruct_payload.extend_from_slice(&7i32.to_be_bytes());
        let deconstruct_packet =
            encode_packet(deconstruct_finish_packet_id, &deconstruct_payload, false).unwrap();
        let deconstruct_event = session.ingest_packet_bytes(&deconstruct_packet).unwrap();
        assert_eq!(
            deconstruct_event,
            ClientSessionEvent::DeconstructFinish {
                tile_pos: pack_point2(8, 9),
                block_id: Some(0x0101),
                builder_kind: 2,
                builder_value: 7,
                removed_local_plan: true,
            }
        );
        assert_eq!(session.snapshot_input().plans, Some(Vec::new()));

        let tick4 = session.advance_time(2_000).unwrap();
        let payload4 = snapshot_payload_at(&tick4, 2_000);
        assert_eq!(&payload4[0..4], &4i32.to_be_bytes());
        assert_eq!(&payload4[55..59], &0i32.to_be_bytes());
    }

    #[test]
    fn encodes_overridden_client_snapshot_input_fields() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 1_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let input = session.snapshot_input_mut();
        input.unit_id = Some(42);
        input.dead = false;
        input.position = Some((10.0, 20.0));
        input.pointer = Some((30.0, 40.0));
        input.rotation = 50.0;
        input.base_rotation = 60.0;
        input.velocity = (1.5, -2.5);
        input.mining_tile = Some((7, 11));
        input.boosting = true;
        input.shooting = true;
        input.chatting = true;
        input.building = true;
        input.selected_block_id = Some(0x0101);
        input.selected_rotation = 3;
        input.view_center = Some((70.0, 80.0));
        input.view_size = Some((90.0, 100.0));

        let expected_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::ClientSnapshot.method_name())
            .unwrap()
            .packet_id;
        let actions = session.advance_time(1_000).unwrap();
        let snapshot_bytes = actions
            .iter()
            .find_map(|action| match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport,
                    bytes,
                } if *packet_id == expected_snapshot_packet_id
                    && *transport == ClientPacketTransport::Udp =>
                {
                    Some(bytes)
                }
                _ => None,
            })
            .expect("expected snapshot send action");
        let decoded = decode_packet(snapshot_bytes).unwrap();

        assert_eq!(&decoded.payload[4..8], &42i32.to_be_bytes());
        assert_eq!(decoded.payload[8], 0);
        assert_eq!(&decoded.payload[9..13], &10.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[13..17], &20.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[17..21], &30.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[21..25], &40.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[25..29], &50.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[29..33], &60.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[33..37], &1.5f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[37..41], &(-2.5f32).to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[41..45], &pack_point2(7, 11).to_be_bytes());
        assert_eq!(decoded.payload[45], 1);
        assert_eq!(decoded.payload[46], 1);
        assert_eq!(decoded.payload[47], 1);
        assert_eq!(decoded.payload[48], 1);
        assert_eq!(&decoded.payload[49..51], &0x0101i16.to_be_bytes());
        assert_eq!(&decoded.payload[51..55], &3i32.to_be_bytes());
        assert_eq!(&decoded.payload[59..63], &70.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[63..67], &80.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[67..71], &90.0f32.to_bits().to_be_bytes());
        assert_eq!(&decoded.payload[71..75], &100.0f32.to_bits().to_be_bytes());
    }

    #[test]
    fn encodes_client_snapshot_build_plan_queue() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 1_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let plans = vec![
            ClientBuildPlan {
                tile: (1, 2),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 1,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (5, 6),
                breaking: true,
                block_id: None,
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
        ];
        let input = session.snapshot_input_mut();
        input.unit_id = Some(42);
        input.dead = false;
        input.position = Some((10.0, 20.0));
        input.selected_block_id = Some(0x0101);
        input.selected_rotation = 1;
        input.plans = Some(plans.clone());

        let expected_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::ClientSnapshot.method_name())
            .unwrap()
            .packet_id;
        let actions = session.advance_time(1_000).unwrap();
        let snapshot_bytes = actions
            .iter()
            .find_map(|action| match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport,
                    bytes,
                } if *packet_id == expected_snapshot_packet_id
                    && *transport == ClientPacketTransport::Udp =>
                {
                    Some(bytes)
                }
                _ => None,
            })
            .expect("expected snapshot send action");
        let decoded = decode_packet(snapshot_bytes).unwrap();

        let mut expected_plans = Vec::new();
        write_client_build_plans_queue(&mut expected_plans, Some(plans.as_slice()));
        assert_eq!(&decoded.payload[49..51], &0x0101i16.to_be_bytes());
        assert_eq!(&decoded.payload[51..55], &1i32.to_be_bytes());
        assert_eq!(&decoded.payload[41..45], &(-1i32).to_be_bytes());
        assert_eq!(
            &decoded.payload[55..55 + expected_plans.len()],
            expected_plans.as_slice()
        );
    }

    #[test]
    fn caps_client_snapshot_build_plan_queue_to_java_network_limit() {
        let plans = (0..25)
            .map(|index| ClientBuildPlan {
                tile: (index, index + 1),
                breaking: false,
                block_id: Some(0x0101),
                rotation: (index % 4) as u8,
                config: ClientBuildPlanConfig::None,
            })
            .collect::<Vec<_>>();
        let mut encoded = Vec::new();

        write_client_build_plans_queue(&mut encoded, Some(plans.as_slice()));

        assert_eq!(&encoded[..4], &20i32.to_be_bytes());

        let mut expected = Vec::new();
        for plan in plans.iter().take(20) {
            write_client_build_plan(&mut expected, plan);
        }
        assert_eq!(&encoded[4..], expected.as_slice());
    }

    #[test]
    fn encodes_client_snapshot_build_plan_string_and_bytes_configs() {
        let string_plan = ClientBuildPlan {
            tile: (1, 2),
            breaking: false,
            block_id: Some(0x0101),
            rotation: 1,
            config: ClientBuildPlanConfig::String("router".to_string()),
        };
        let bytes_plan = ClientBuildPlan {
            tile: (3, 4),
            breaking: false,
            block_id: Some(0x0102),
            rotation: 2,
            config: ClientBuildPlanConfig::Bytes(vec![1, 2, 3, 4]),
        };
        let mut string_encoded = Vec::new();
        let mut bytes_encoded = Vec::new();

        write_client_build_plan(&mut string_encoded, &string_plan);
        write_client_build_plan(&mut bytes_encoded, &bytes_plan);

        assert_eq!(string_encoded[8], 1);
        assert_eq!(string_encoded[9], 4);
        assert_eq!(
            &string_encoded[10..],
            encode_typeio_string_payload("router")
        );

        assert_eq!(bytes_encoded[8], 1);
        assert_eq!(bytes_encoded[9], 14);
        assert_eq!(&bytes_encoded[10..14], &4i32.to_be_bytes());
        assert_eq!(&bytes_encoded[14..], &[1, 2, 3, 4]);
    }

    #[test]
    fn encodes_client_snapshot_build_plan_additional_config_variants() {
        let plans = vec![
            (ClientBuildPlanConfig::Int(7), TypeIoObject::Int(7), "int"),
            (
                ClientBuildPlanConfig::Long(0x0102_0304_0506_0708),
                TypeIoObject::Long(0x0102_0304_0506_0708),
                "long",
            ),
            (
                ClientBuildPlanConfig::FloatBits(12.5f32.to_bits()),
                TypeIoObject::Float(12.5),
                "float",
            ),
            (
                ClientBuildPlanConfig::Bool(true),
                TypeIoObject::Bool(true),
                "bool",
            ),
            (
                ClientBuildPlanConfig::IntSeq(vec![1, 2, 3]),
                TypeIoObject::IntSeq(vec![1, 2, 3]),
                "int-seq",
            ),
            (
                ClientBuildPlanConfig::Content {
                    content_type: 1,
                    content_id: 0x0101,
                },
                TypeIoObject::ContentRaw {
                    content_type: 1,
                    content_id: 0x0101,
                },
                "content",
            ),
            (
                ClientBuildPlanConfig::TechNodeRaw {
                    content_type: 1,
                    content_id: 0x0102,
                },
                TypeIoObject::TechNodeRaw {
                    content_type: 1,
                    content_id: 0x0102,
                },
                "tech-node-raw",
            ),
            (
                ClientBuildPlanConfig::Point2Array(vec![(1, 2), (3, 4)]),
                TypeIoObject::PackedPoint2Array(vec![pack_point2(1, 2), pack_point2(3, 4)]),
                "point2-array",
            ),
            (
                ClientBuildPlanConfig::DoubleBits(12.5f64.to_bits()),
                TypeIoObject::Double(12.5),
                "double",
            ),
            (
                ClientBuildPlanConfig::BuildingPos(0x0001_0002),
                TypeIoObject::BuildingPos(0x0001_0002),
                "building-pos",
            ),
            (
                ClientBuildPlanConfig::LAccess(5),
                TypeIoObject::LAccess(5),
                "laccess",
            ),
            (
                ClientBuildPlanConfig::LegacyUnitCommandNull(0xab),
                TypeIoObject::LegacyUnitCommandNull(0xab),
                "legacy-unit-command-null",
            ),
            (
                ClientBuildPlanConfig::BoolArray(vec![true, false, true]),
                TypeIoObject::BoolArray(vec![true, false, true]),
                "bool-array",
            ),
            (
                ClientBuildPlanConfig::UnitId(0x0102_0304),
                TypeIoObject::UnitId(0x0102_0304),
                "unit-id",
            ),
            (
                ClientBuildPlanConfig::Vec2Array(vec![
                    (1.5f32.to_bits(), (-2.5f32).to_bits()),
                    (3.25f32.to_bits(), (-4.75f32).to_bits()),
                ]),
                TypeIoObject::Vec2Array(vec![(1.5, -2.5), (3.25, -4.75)]),
                "vec2-array",
            ),
            (
                ClientBuildPlanConfig::Vec2 {
                    x_bits: (-2.25f32).to_bits(),
                    y_bits: 1.5f32.to_bits(),
                },
                TypeIoObject::Vec2 { x: -2.25, y: 1.5 },
                "vec2",
            ),
            (
                ClientBuildPlanConfig::Team(7),
                TypeIoObject::Team(7),
                "team",
            ),
            (
                ClientBuildPlanConfig::IntArray(vec![1, -2, 3]),
                TypeIoObject::IntArray(vec![1, -2, 3]),
                "int-array",
            ),
            (
                ClientBuildPlanConfig::ObjectArray(vec![
                    ClientBuildPlanConfig::Int(7),
                    ClientBuildPlanConfig::Long(9),
                    ClientBuildPlanConfig::FloatBits(1.25f32.to_bits()),
                    ClientBuildPlanConfig::String("router".to_string()),
                    ClientBuildPlanConfig::Bool(true),
                    ClientBuildPlanConfig::Point2 { x: 3, y: 4 },
                    ClientBuildPlanConfig::Vec2 {
                        x_bits: 1.5f32.to_bits(),
                        y_bits: (-2.5f32).to_bits(),
                    },
                    ClientBuildPlanConfig::Team(2),
                    ClientBuildPlanConfig::None,
                ]),
                TypeIoObject::ObjectArray(vec![
                    TypeIoObject::Int(7),
                    TypeIoObject::Long(9),
                    TypeIoObject::Float(1.25),
                    TypeIoObject::String(Some("router".to_string())),
                    TypeIoObject::Bool(true),
                    TypeIoObject::Point2 { x: 3, y: 4 },
                    TypeIoObject::Vec2 { x: 1.5, y: -2.5 },
                    TypeIoObject::Team(2),
                    TypeIoObject::Null,
                ]),
                "object-array",
            ),
            (
                ClientBuildPlanConfig::UnitCommand(42),
                TypeIoObject::UnitCommand(42),
                "unit-command",
            ),
        ];

        for (config, expected_object, label) in plans {
            let plan = ClientBuildPlan {
                tile: (1, 2),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 1,
                config,
            };
            let mut encoded = Vec::new();
            write_client_build_plan(&mut encoded, &plan);
            let mut expected_config_payload = Vec::new();
            write_typeio_object(&mut expected_config_payload, &expected_object);

            assert_eq!(encoded[8], 1, "{label} missing config marker");
            assert_eq!(&encoded[9..], expected_config_payload.as_slice(), "{label}");
        }
    }

    #[test]
    fn caps_client_snapshot_build_plan_queue_to_java_config_payload_budget() {
        let plans = vec![
            ClientBuildPlan {
                tile: (1, 2),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 0,
                config: ClientBuildPlanConfig::String("a".repeat(250)),
            },
            ClientBuildPlan {
                tile: (3, 4),
                breaking: false,
                block_id: Some(0x0102),
                rotation: 1,
                config: ClientBuildPlanConfig::Bytes(vec![7; 250]),
            },
            ClientBuildPlan {
                tile: (5, 6),
                breaking: false,
                block_id: Some(0x0103),
                rotation: 2,
                config: ClientBuildPlanConfig::String("z".to_string()),
            },
            ClientBuildPlan {
                tile: (7, 8),
                breaking: false,
                block_id: Some(0x0104),
                rotation: 3,
                config: ClientBuildPlanConfig::String("trimmed".to_string()),
            },
        ];
        let mut encoded = Vec::new();

        write_client_build_plans_queue(&mut encoded, Some(plans.as_slice()));

        assert_eq!(max_client_build_plans(&plans), 3);
        assert_eq!(&encoded[..4], &3i32.to_be_bytes());

        let mut expected = Vec::new();
        for plan in plans.iter().take(3) {
            write_client_build_plan(&mut expected, plan);
        }
        assert_eq!(&encoded[4..], expected.as_slice());
    }

    #[test]
    fn initializes_snapshot_input_from_world_bootstrap() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let expected_unit_id =
            i32::try_from(session.state().world_player_unit_value.unwrap()).unwrap();
        let expected_position = (
            sanitize_bootstrap_coord(session.state().world_player_x_bits.unwrap()),
            sanitize_bootstrap_coord(session.state().world_player_y_bits.unwrap()),
        );

        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, Some(expected_unit_id));
        assert!(!input.dead);
        assert_eq!(input.position, Some(expected_position));
        assert_eq!(input.view_center, Some(expected_position));
        assert_eq!(input.selected_block_id, None);
        assert_eq!(input.selected_rotation, 0);
    }

    #[test]
    fn entity_snapshot_packet_refreshes_local_player_unit_id() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        let local_player_id = session.state().world_player_id.unwrap();

        let input = session.snapshot_input_mut();
        input.unit_id = Some(999);
        input.position = Some((999.0, 999.0));
        input.view_center = Some((999.0, 999.0));

        let entity_snapshot_wire =
            encode_packet(44, &sample_snapshot_packet("entitySnapshot.packet"), false).unwrap();
        let event = session.ingest_packet_bytes(&entity_snapshot_wire).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(session.state().world_player_unit_kind, Some(2));
        assert_eq!(session.state().world_player_unit_value, Some(100));
        assert_eq!(session.state().world_player_x_bits, Some(0.0f32.to_bits()));
        assert_eq!(session.state().world_player_y_bits, Some(0.0f32.to_bits()));
        assert_eq!(session.state().received_entity_snapshot_count, 1);
        assert_eq!(session.state().last_entity_snapshot_amount, Some(2));
        assert_eq!(session.state().last_entity_snapshot_body_len, Some(155));
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 1);
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_count,
            1
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_fallback_count,
            0
        );
        assert_eq!(
            session.state().last_entity_snapshot_target_player_id,
            Some(local_player_id)
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_used_projection_fallback
        );
        assert!(
            session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
        assert_eq!(session.state().failed_entity_snapshot_parse_count, 0);
        assert_eq!(session.state().last_entity_snapshot_parse_error, None);
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .local_player_entity_id,
            Some(local_player_id)
        );
        assert_eq!(session.state().entity_table_projection.hidden_count, 0);
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .applied_local_player_count,
            1
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&local_player_id),
            Some(&crate::session_state::EntityProjection {
                class_id: crate::session_state::EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, Some(100));
        assert!(!input.dead);
        assert_eq!(input.position, Some((0.0, 0.0)));
        assert_eq!(input.view_center, Some((0.0, 0.0)));
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_player_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        let local_player_id = session.state().world_player_id.unwrap();

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        assert_eq!(rows.len(), 1);
        let player_row = &sample_body[rows[0].start..rows[0].end];
        let mut other_player_row = player_row.to_vec();
        other_player_row[..4].copy_from_slice(&99i32.to_be_bytes());

        let mut payload = Vec::new();
        payload.extend_from_slice(&2u16.to_be_bytes());
        payload.extend_from_slice(
            &u16::try_from(player_row.len() + other_player_row.len())
                .unwrap()
                .to_be_bytes(),
        );
        payload.extend_from_slice(player_row);
        payload.extend_from_slice(&other_player_row);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(session.state().last_entity_snapshot_amount, Some(2));
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&local_player_id),
            Some(&crate::session_state::EntityProjection {
                class_id: crate::session_state::EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&99),
            Some(&crate::session_state::EntityProjection {
                class_id: crate::session_state::EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_alpha_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let payload = sample_snapshot_packet("entitySnapshot.packet");
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![100]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&100),
            Some(&crate::session_state::EntityProjection {
                class_id: 0,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn alpha_shape_entity_snapshot_prefix_parser_accepts_same_revision_family_class_ids() {
        for class_id in [30u8, 45u8, 24u8, 2u8] {
            let mut payload = sample_snapshot_packet("entitySnapshot.packet");
            let body_len = u16::from_be_bytes([payload[2], payload[3]]) as usize;
            let body = &mut payload[4..4 + body_len];
            body[57 + 4] = class_id;

            let rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&payload);

            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].entity_id, 100);
            assert_eq!(rows[0].class_id, class_id);
            assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
            assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
        }
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_mech_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let payload = build_entity_snapshot_payload(&[player_row, alpha_row, mech_row]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_mech_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![321]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&321),
            Some(&crate::session_state::EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 321,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn mech_shape_entity_snapshot_prefix_parser_accepts_same_revision_family_class_ids() {
        let mech_bytes = synthetic_mech_sync_bytes();
        for class_id in [4u8, 17u8, 19u8, 32u8] {
            let payload = build_entity_snapshot_payload(&[build_entity_snapshot_row(
                321,
                class_id,
                &mech_bytes,
            )]);

            let rows = try_parse_mech_sync_rows_from_entity_snapshot_prefix(&payload);

            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].entity_id, 321);
            assert_eq!(rows[0].class_id, class_id);
            assert_eq!(rows[0].sync.base_rotation_bits, 15.0f32.to_bits());
            assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
            assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
        }
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_missile_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload =
            build_entity_snapshot_payload(&[player_row, alpha_row, mech_row, missile_row]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_missile_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![654]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&654),
            Some(&crate::session_state::EntityProjection {
                class_id: 39,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 654,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn missile_shape_entity_snapshot_prefix_parser_accepts_missile_class_id() {
        let payload = build_entity_snapshot_payload(&[build_entity_snapshot_row(
            654,
            39,
            &synthetic_missile_sync_bytes(),
        )]);

        let rows = try_parse_missile_sync_rows_from_entity_snapshot_prefix(&payload);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id, 654);
        assert_eq!(rows[0].class_id, 39);
        assert_eq!(rows[0].sync.lifetime_bits, 240.0f32.to_bits());
        assert_eq!(rows[0].sync.time_bits, 12.5f32.to_bits());
        assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
        assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_payload_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload_row = build_entity_snapshot_row(777, 5, &synthetic_payload_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            player_row,
            alpha_row,
            mech_row,
            missile_row,
            payload_row,
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_payload_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![777]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&777),
            Some(&crate::session_state::EntityProjection {
                class_id: 5,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 777,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn entity_snapshot_packet_applies_real_java_payload_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let payload_rows = real_java_payload_entity_rows();
        let (tether_entity_id, tether_class_id, tether_row) =
            real_java_building_tether_payload_entity_row();
        let mut rows = vec![player_row, alpha_row];
        rows.extend(payload_rows.iter().map(|(_, _, row)| row.clone()));
        rows.push(tether_row);
        let payload = build_entity_snapshot_payload(&rows);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                Some(
                    parse_world_bundle(&compressed_world_stream)
                        .unwrap()
                        .content_header
                        .as_slice()
                ),
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            payload_rows
                .iter()
                .map(|(entity_id, _, _)| *entity_id)
                .collect::<Vec<_>>()
        );

        for (entity_id, class_id) in payload_rows
            .iter()
            .map(|(entity_id, class_id, _)| (*entity_id, *class_id))
            .chain(std::iter::once((tether_entity_id, tether_class_id)))
        {
            assert_eq!(
                session
                    .state()
                    .entity_table_projection
                    .by_entity_id
                    .get(&entity_id),
                Some(&crate::session_state::EntityProjection {
                    class_id,
                    hidden: false,
                    is_local_player: false,
                    unit_kind: 2,
                    unit_value: entity_id as u32,
                    x_bits: 40.0f32.to_bits(),
                    y_bits: 60.0f32.to_bits(),
                    last_seen_entity_snapshot_count: 1,
                }),
                "entity_id={entity_id}"
            );
        }
    }

    #[test]
    fn payload_shape_entity_snapshot_prefix_parser_accepts_same_revision_family_class_ids() {
        let payload_bytes = synthetic_payload_sync_bytes();
        for class_id in [5u8, 23u8, 26u8] {
            let payload = build_entity_snapshot_payload(&[build_entity_snapshot_row(
                777,
                class_id,
                &payload_bytes,
            )]);

            let rows = try_parse_payload_sync_rows_from_entity_snapshot_prefix(&payload);

            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].entity_id, 777);
            assert_eq!(rows[0].class_id, class_id);
            assert_eq!(rows[0].sync.payload_count, 0);
            assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
            assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
        }
    }

    #[test]
    fn later_entity_snapshot_prefix_parsers_skip_build_payload_rows_when_content_header_is_loaded()
    {
        let world_bundle = parse_world_bundle(&sample_world_stream_bytes()).unwrap();
        let conveyor = world_bundle.world.building_center_at(1, 2).unwrap();
        let conveyor_chunk = conveyor.chunk_bytes.clone();
        let block_id = i16::from_be_bytes(conveyor.block_id.to_be_bytes());
        let content_header = Some(world_bundle.content_header.as_slice());
        let payload_row = build_entity_snapshot_row(
            777,
            5,
            &synthetic_payload_sync_bytes_with_build_payload(block_id, &conveyor_chunk),
        );
        let tether_row = build_entity_snapshot_row(
            888,
            36,
            &synthetic_building_tether_payload_sync_bytes_with_build_payload(
                block_id,
                &conveyor_chunk,
            ),
        );
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let payload = build_entity_snapshot_payload(&[payload_row, tether_row, fire_row]);

        assert_eq!(
            try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![777]
        );
        assert_eq!(
            try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![888]
        );
        assert_eq!(
            try_parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![901]
        );
    }

    #[test]
    fn later_entity_snapshot_prefix_parsers_skip_unit_payload_rows_when_content_header_is_loaded() {
        let world_bundle = parse_world_bundle(&sample_world_stream_bytes()).unwrap();
        let alpha_unit_body = synthetic_payload_campaign_unit_payload_body();
        let oct_unit_body = synthetic_oct_revision_one_unit_payload_body();
        let quad_unit_body = synthetic_quad_revision_six_unit_payload_body();
        let content_header = Some(world_bundle.content_header.as_slice());
        let alpha_payload_row = build_entity_snapshot_row(
            777,
            5,
            &synthetic_payload_sync_bytes_with_unit_payload(0, &alpha_unit_body),
        );
        let quad_payload_row = build_entity_snapshot_row(
            778,
            23,
            &synthetic_payload_sync_bytes_with_unit_payload(23, &quad_unit_body),
        );
        let oct_payload_row = build_entity_snapshot_row(
            779,
            26,
            &synthetic_payload_sync_bytes_with_unit_payload(26, &oct_unit_body),
        );
        let tether_row = build_entity_snapshot_row(
            888,
            36,
            &synthetic_building_tether_payload_sync_bytes_with_unit_payload(23, &quad_unit_body),
        );
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            alpha_payload_row,
            quad_payload_row,
            oct_payload_row,
            tether_row,
            fire_row,
        ]);

        assert_eq!(
            try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![777, 778, 779]
        );
        assert_eq!(
            try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![888]
        );
        assert_eq!(
            try_parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![901]
        );
    }

    #[test]
    fn later_entity_snapshot_prefix_parsers_skip_real_java_unit_payload_rows_when_content_header_is_loaded(
    ) {
        let world_bundle = parse_world_bundle(&sample_world_stream_bytes()).unwrap();
        let content_header = Some(world_bundle.content_header.as_slice());
        let payload_rows = real_java_payload_entity_rows();
        let (tether_entity_id, _, tether_row) = real_java_building_tether_payload_entity_row();
        let mut rows = payload_rows
            .iter()
            .map(|(_, _, row)| row.clone())
            .collect::<Vec<_>>();
        rows.push(tether_row);
        rows.push(build_entity_snapshot_row(
            901,
            10,
            &synthetic_fire_sync_bytes(),
        ));
        let payload = build_entity_snapshot_payload(&rows);

        assert_eq!(
            try_parse_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            payload_rows
                .iter()
                .map(|(entity_id, _, _)| *entity_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![tether_entity_id]
        );
        assert_eq!(
            try_parse_fire_sync_rows_from_entity_snapshot_prefix_with_content_header(
                &payload,
                content_header,
            )
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
            vec![901]
        );
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_building_tether_payload_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload_row = build_entity_snapshot_row(777, 5, &synthetic_payload_sync_bytes());
        let tether_payload_row =
            build_entity_snapshot_row(888, 36, &synthetic_building_tether_payload_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            player_row,
            alpha_row,
            mech_row,
            missile_row,
            payload_row,
            tether_payload_row,
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![888]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&888),
            Some(&crate::session_state::EntityProjection {
                class_id: 36,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 888,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn entity_snapshot_packet_applies_real_java_building_tether_payload_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let (manifold_class_id, manifold_unit_body) = java_unit_payload_golden_body("manifold");
        let tether_payload_row = build_entity_snapshot_row(
            888,
            36,
            &synthetic_building_tether_payload_sync_bytes_with_unit_payload(
                manifold_class_id,
                &manifold_unit_body,
            ),
        );
        let payload = build_entity_snapshot_payload(&[player_row, alpha_row, tether_payload_row]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&888),
            Some(&crate::session_state::EntityProjection {
                class_id: 36,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 888,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn building_tether_payload_shape_entity_snapshot_prefix_parser_accepts_class_id_36() {
        let payload = build_entity_snapshot_payload(&[build_entity_snapshot_row(
            888,
            36,
            &synthetic_building_tether_payload_sync_bytes(),
        )]);

        let rows =
            try_parse_building_tether_payload_sync_rows_from_entity_snapshot_prefix(&payload);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id, 888);
        assert_eq!(rows[0].class_id, 36);
        assert_eq!(rows[0].sync.building_pos, 12345);
        assert_eq!(rows[0].sync.payload_count, 0);
        assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
        assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_fire_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload_row = build_entity_snapshot_row(777, 5, &synthetic_payload_sync_bytes());
        let tether_payload_row =
            build_entity_snapshot_row(888, 36, &synthetic_building_tether_payload_sync_bytes());
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            player_row,
            alpha_row,
            mech_row,
            missile_row,
            payload_row,
            tether_payload_row,
            fire_row,
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_fire_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![901]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&901),
            Some(&crate::session_state::EntityProjection {
                class_id: 10,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn fire_shape_entity_snapshot_prefix_parser_accepts_class_id_10() {
        let payload = build_entity_snapshot_payload(&[build_entity_snapshot_row(
            901,
            10,
            &synthetic_fire_sync_bytes(),
        )]);

        let rows = try_parse_fire_sync_rows_from_entity_snapshot_prefix(&payload);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id, 901);
        assert_eq!(rows[0].class_id, 10);
        assert_eq!(rows[0].sync.lifetime_bits, 240.0f32.to_bits());
        assert_eq!(rows[0].sync.tile_pos, 12345);
        assert_eq!(rows[0].sync.time_bits, 12.5f32.to_bits());
        assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
        assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_puddle_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload_row = build_entity_snapshot_row(777, 5, &synthetic_payload_sync_bytes());
        let tether_payload_row =
            build_entity_snapshot_row(888, 36, &synthetic_building_tether_payload_sync_bytes());
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let puddle_row = build_entity_snapshot_row(902, 13, &synthetic_puddle_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            player_row,
            alpha_row,
            mech_row,
            missile_row,
            payload_row,
            tether_payload_row,
            fire_row,
            puddle_row,
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_puddle_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![902]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&902),
            Some(&crate::session_state::EntityProjection {
                class_id: 13,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn puddle_shape_entity_snapshot_prefix_parser_accepts_class_id_13() {
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let puddle_row = build_entity_snapshot_row(902, 13, &synthetic_puddle_sync_bytes());
        let payload = build_entity_snapshot_payload(&[fire_row, puddle_row]);

        let rows = try_parse_puddle_sync_rows_from_entity_snapshot_prefix(&payload);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id, 902);
        assert_eq!(rows[0].class_id, 13);
        assert_eq!(rows[0].sync.amount_bits, 6.5f32.to_bits());
        assert_eq!(rows[0].sync.liquid_id, 3);
        assert_eq!(rows[0].sync.tile_pos, 12345);
        assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
        assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_weather_state_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload_row = build_entity_snapshot_row(777, 5, &synthetic_payload_sync_bytes());
        let tether_payload_row =
            build_entity_snapshot_row(888, 36, &synthetic_building_tether_payload_sync_bytes());
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let puddle_row = build_entity_snapshot_row(902, 13, &synthetic_puddle_sync_bytes());
        let weather_state_row =
            build_entity_snapshot_row(903, 14, &synthetic_weather_state_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            player_row,
            alpha_row,
            mech_row,
            missile_row,
            payload_row,
            tether_payload_row,
            fire_row,
            puddle_row,
            weather_state_row,
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_weather_state_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![903]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&903),
            Some(&crate::session_state::EntityProjection {
                class_id: 14,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn weather_state_shape_entity_snapshot_prefix_parser_accepts_class_id_14() {
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let puddle_row = build_entity_snapshot_row(902, 13, &synthetic_puddle_sync_bytes());
        let weather_state_row =
            build_entity_snapshot_row(903, 14, &synthetic_weather_state_sync_bytes());
        let payload = build_entity_snapshot_payload(&[fire_row, puddle_row, weather_state_row]);

        let rows = try_parse_weather_state_sync_rows_from_entity_snapshot_prefix(&payload);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id, 903);
        assert_eq!(rows[0].class_id, 14);
        assert_eq!(rows[0].sync.effect_timer_bits, 1.25f32.to_bits());
        assert_eq!(rows[0].sync.intensity_bits, 0.75f32.to_bits());
        assert_eq!(rows[0].sync.life_bits, 600.0f32.to_bits());
        assert_eq!(rows[0].sync.opacity_bits, 0.5f32.to_bits());
        assert_eq!(rows[0].sync.weather_id, 2);
        assert_eq!(rows[0].sync.wind_x_bits, 3.0f32.to_bits());
        assert_eq!(rows[0].sync.wind_y_bits, (-4.0f32).to_bits());
        assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
        assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
    }

    #[test]
    fn entity_snapshot_packet_applies_parseable_world_label_rows_to_entity_table() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let player_rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        let alpha_rows = try_parse_alpha_sync_rows_from_entity_snapshot_prefix(&sample_payload);
        assert_eq!(player_rows.len(), 1);
        assert_eq!(alpha_rows.len(), 1);
        let player_row = sample_body[player_rows[0].start..player_rows[0].end].to_vec();
        let alpha_row = sample_body[alpha_rows[0].start..alpha_rows[0].end].to_vec();
        let mech_row = build_entity_snapshot_row(321, 4, &synthetic_mech_sync_bytes());
        let missile_row = build_entity_snapshot_row(654, 39, &synthetic_missile_sync_bytes());
        let payload_row = build_entity_snapshot_row(777, 5, &synthetic_payload_sync_bytes());
        let tether_payload_row =
            build_entity_snapshot_row(888, 36, &synthetic_building_tether_payload_sync_bytes());
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let puddle_row = build_entity_snapshot_row(902, 13, &synthetic_puddle_sync_bytes());
        let weather_state_row =
            build_entity_snapshot_row(903, 14, &synthetic_weather_state_sync_bytes());
        let world_label_row =
            build_entity_snapshot_row(904, 35, &synthetic_world_label_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            player_row,
            alpha_row,
            mech_row,
            missile_row,
            payload_row,
            tether_payload_row,
            fire_row,
            puddle_row,
            weather_state_row,
            world_label_row,
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            try_parse_world_label_sync_rows_from_entity_snapshot_prefix(&payload)
                .iter()
                .map(|row| row.entity_id)
                .collect::<Vec<_>>(),
            vec![904]
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&904),
            Some(&crate::session_state::EntityProjection {
                class_id: 35,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 40.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            })
        );
    }

    #[test]
    fn world_label_shape_entity_snapshot_prefix_parser_accepts_class_id_35() {
        let fire_row = build_entity_snapshot_row(901, 10, &synthetic_fire_sync_bytes());
        let puddle_row = build_entity_snapshot_row(902, 13, &synthetic_puddle_sync_bytes());
        let weather_state_row =
            build_entity_snapshot_row(903, 14, &synthetic_weather_state_sync_bytes());
        let world_label_row =
            build_entity_snapshot_row(904, 35, &synthetic_world_label_sync_bytes());
        let payload = build_entity_snapshot_payload(&[
            fire_row,
            puddle_row,
            weather_state_row,
            world_label_row,
        ]);

        let rows = try_parse_world_label_sync_rows_from_entity_snapshot_prefix(&payload);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_id, 904);
        assert_eq!(rows[0].class_id, 35);
        assert_eq!(rows[0].sync.flags, 3);
        assert_eq!(rows[0].sync.font_size_bits, 1.5f32.to_bits());
        assert_eq!(rows[0].sync.text.as_deref(), Some("hello world"));
        assert_eq!(rows[0].sync.x_bits, 40.0f32.to_bits());
        assert_eq!(rows[0].sync.y_bits, 60.0f32.to_bits());
        assert_eq!(rows[0].sync.z_bits, 120.0f32.to_bits());
    }

    #[test]
    fn entity_snapshot_rejects_parseable_player_rows_exceeding_declared_amount() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        assert_eq!(rows.len(), 1);
        let player_row = &sample_body[rows[0].start..rows[0].end];
        let mut other_player_row = player_row.to_vec();
        other_player_row[..4].copy_from_slice(&99i32.to_be_bytes());

        let mut payload = Vec::new();
        payload.extend_from_slice(&1u16.to_be_bytes());
        payload.extend_from_slice(
            &u16::try_from(player_row.len() + other_player_row.len())
                .unwrap()
                .to_be_bytes(),
        );
        payload.extend_from_slice(player_row);
        payload.extend_from_slice(&other_player_row);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(session.state().last_entity_snapshot_amount, Some(1));
        assert_eq!(
            session.state().last_entity_snapshot_body_len,
            Some(player_row.len() + other_player_row.len())
        );
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 0);
        assert_eq!(session.state().last_entity_snapshot_target_player_id, None);
        assert_eq!(
            session
                .state()
                .last_entity_snapshot_local_player_sync_match_count,
            0
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
        assert_eq!(session.state().failed_entity_snapshot_parse_count, 1);
        assert_eq!(
            session.state().last_entity_snapshot_parse_error.as_deref(),
            Some("entity_snapshot_parseable_rows_exceed_amount:2/1")
        );
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));
    }

    #[test]
    fn unit_despawn_tombstone_blocks_immediate_entity_snapshot_revival_and_expires_next_snapshot() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        assert_eq!(rows.len(), 1);
        let player_row = &sample_body[rows[0].start..rows[0].end];
        let mut other_player_row = player_row.to_vec();
        other_player_row[..4].copy_from_slice(&99i32.to_be_bytes());

        let mut entity_payload = Vec::new();
        entity_payload.extend_from_slice(&2u16.to_be_bytes());
        entity_payload.extend_from_slice(
            &u16::try_from(player_row.len() + other_player_row.len())
                .unwrap()
                .to_be_bytes(),
        );
        entity_payload.extend_from_slice(player_row);
        entity_payload.extend_from_slice(&other_player_row);

        let entity_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let entity_packet = encode_packet(entity_packet_id, &entity_payload, false).unwrap();
        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));

        let unit_despawn_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitDespawn")
            .unwrap()
            .packet_id;
        let unit_despawn_packet = encode_packet(
            unit_despawn_packet_id,
            &encode_unit_payload(ClientUnitRef::Standard(99)),
            false,
        )
        .unwrap();

        let despawn_event = session.ingest_packet_bytes(&unit_despawn_packet).unwrap();
        assert_eq!(
            despawn_event,
            ClientSessionEvent::UnitDespawned {
                unit: Some(UnitRefProjection { kind: 2, value: 99 }),
                removed_entity_projection: true,
            }
        );
        assert!(session.state().entity_snapshot_tombstones.contains_key(&99));
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));

        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));
        assert_eq!(session.state().entity_snapshot_tombstone_skip_count, 1);
        assert_eq!(
            session
                .state()
                .last_entity_snapshot_tombstone_skipped_ids_sample,
            vec![99]
        );

        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));
        assert!(session
            .state()
            .last_entity_snapshot_tombstone_skipped_ids_sample
            .is_empty());
    }

    #[test]
    fn hidden_snapshot_does_not_block_non_local_entity_snapshot_revival() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let rows = try_parse_player_sync_rows_from_entity_snapshot(&sample_payload);
        assert_eq!(rows.len(), 1);
        let player_row = &sample_body[rows[0].start..rows[0].end];
        let mut other_player_row = player_row.to_vec();
        other_player_row[..4].copy_from_slice(&99i32.to_be_bytes());

        let mut entity_payload = Vec::new();
        entity_payload.extend_from_slice(&2u16.to_be_bytes());
        entity_payload.extend_from_slice(
            &u16::try_from(player_row.len() + other_player_row.len())
                .unwrap()
                .to_be_bytes(),
        );
        entity_payload.extend_from_slice(player_row);
        entity_payload.extend_from_slice(&other_player_row);

        let entity_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let entity_packet = encode_packet(entity_packet_id, &entity_payload, false).unwrap();
        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));

        let hidden_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::HiddenSnapshot.method_name())
            .unwrap()
            .packet_id;
        let mut hidden_payload = Vec::new();
        hidden_payload.extend_from_slice(&1i32.to_be_bytes());
        hidden_payload.extend_from_slice(&99i32.to_be_bytes());
        let hidden_packet = encode_packet(hidden_packet_id, &hidden_payload, false).unwrap();
        session.ingest_packet_bytes(&hidden_packet).unwrap();

        assert!(session.state().hidden_snapshot_ids.contains(&99));
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));

        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&99)
                .map(|entity| entity.hidden),
            Some(false)
        );

        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&99));
        assert!(session.state().entity_snapshot_tombstones.is_empty());
        assert_eq!(session.state().entity_snapshot_tombstone_skip_count, 0);
    }

    fn pack_build_pos_for_block_snapshot_test(x: usize, y: usize) -> i32 {
        ((((x as i16) as u16 as u32) << 16) | ((y as i16) as u16 as u32)) as i32
    }

    fn loaded_world_ready_session_for_block_snapshot_test(
    ) -> (mdt_remote::RemoteManifest, ClientSession) {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        (manifest, session)
    }

    fn build_loaded_world_block_snapshot_payload(
        session: &ClientSession,
        take: usize,
    ) -> (Vec<u8>, Vec<(i32, i16, u32)>) {
        let world = &session.loaded_world_bundle().unwrap().world;
        assert!(world.building_centers.len() >= take);
        let mut data = Vec::new();
        let mut entries = Vec::new();

        for center in world.building_centers.iter().take(take) {
            let build_pos = pack_build_pos_for_block_snapshot_test(center.x, center.y);
            data.extend_from_slice(&build_pos.to_be_bytes());
            data.extend_from_slice(&(center.block_id as i16).to_be_bytes());
            data.extend_from_slice(&center.chunk_bytes[1..]);
            entries.push((
                build_pos,
                center.block_id as i16,
                center.building.base.health_bits,
            ));
        }

        let mut payload = Vec::new();
        payload.extend_from_slice(&(take as i16).to_be_bytes());
        payload.extend_from_slice(&u16::try_from(data.len()).unwrap().to_be_bytes());
        payload.extend_from_slice(&data);
        (payload, entries)
    }

    #[test]
    fn block_snapshot_packet_applies_additional_loaded_world_entries_to_building_table() {
        let (manifest, mut session) = loaded_world_ready_session_for_block_snapshot_test();
        let (payload, entries) = build_loaded_world_block_snapshot_payload(&session, 2);
        let (first_build_pos, first_block_id, _) = entries[0];
        let (second_build_pos, second_block_id, second_health_bits) = entries[1];

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::BlockSnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::BlockSnapshot)
        );
        assert_eq!(
            session
                .state()
                .last_block_snapshot
                .as_ref()
                .map(|value| value.amount),
            Some(2)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&first_build_pos)
                .and_then(|building| building.block_id),
            Some(first_block_id)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&second_build_pos)
                .and_then(|building| building.block_id),
            Some(second_block_id)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&second_build_pos)
                .and_then(|building| building.health_bits),
            Some(second_health_bits)
        );
        assert_eq!(
            session
                .state()
                .applied_loaded_world_block_snapshot_extra_entry_count,
            2
        );
        assert_eq!(
            session
                .state()
                .last_loaded_world_block_snapshot_extra_entry_count,
            2
        );
        assert_eq!(
            session
                .state()
                .failed_loaded_world_block_snapshot_extra_entry_parse_count,
            0
        );
        assert_eq!(
            session
                .state()
                .last_loaded_world_block_snapshot_extra_entry_parse_error,
            None
        );
    }

    #[test]
    fn block_snapshot_packet_rejects_loaded_world_extra_entries_with_trailing_bytes() {
        let (manifest, mut session) = loaded_world_ready_session_for_block_snapshot_test();
        let (mut payload, entries) = build_loaded_world_block_snapshot_payload(&session, 2);
        let (first_build_pos, first_block_id, _) = entries[0];
        let (second_build_pos, _, _) = entries[1];
        let second_before = session
            .state()
            .building_table_projection
            .by_build_pos
            .get(&second_build_pos)
            .cloned();
        let original_data_len = u16::from_be_bytes([payload[2], payload[3]]);
        payload.push(0x7f);
        payload[2..4].copy_from_slice(&original_data_len.saturating_add(1).to_be_bytes());

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::BlockSnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::BlockSnapshot)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&first_build_pos)
                .and_then(|building| building.block_id),
            Some(first_block_id)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&second_build_pos)
                .cloned(),
            second_before
        );
        assert_eq!(
            session
                .state()
                .applied_loaded_world_block_snapshot_extra_entry_count,
            0
        );
        assert_eq!(
            session
                .state()
                .last_loaded_world_block_snapshot_extra_entry_count,
            0
        );
        assert_eq!(
            session
                .state()
                .failed_loaded_world_block_snapshot_extra_entry_parse_count,
            1
        );
        assert!(session
            .state()
            .last_loaded_world_block_snapshot_extra_entry_parse_error
            .as_deref()
            .unwrap()
            .starts_with("loaded_world_block_snapshot_extra_trailing_bytes:"));
    }

    #[test]
    fn loaded_world_block_snapshot_keeps_applied_prefix_before_later_entry_parse_failure() {
        let (manifest, mut session) = loaded_world_ready_session_for_block_snapshot_test();
        let (mut payload, entries) = build_loaded_world_block_snapshot_payload(&session, 2);
        let (first_build_pos, first_block_id, _) = entries[0];
        let (second_build_pos, second_block_id, _) = entries[1];
        let (first_entry_len, second_center_block_id) = {
            let world = &session.loaded_world_bundle().unwrap().world;
            (
                6 + world.building_centers[0]
                    .chunk_bytes
                    .len()
                    .saturating_sub(1),
                world.building_centers[1].block_id,
            )
        };
        let first_health_bits = 0x4000_0000u32;
        payload[10..14].copy_from_slice(&first_health_bits.to_be_bytes());
        let second_block_id_offset = 4 + first_entry_len + 4;
        payload[second_block_id_offset..second_block_id_offset + 2]
            .copy_from_slice(&second_block_id.wrapping_add(1).to_be_bytes());
        let second_before = session
            .state()
            .building_table_projection
            .by_build_pos
            .get(&second_build_pos)
            .cloned();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::BlockSnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::BlockSnapshot)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&first_build_pos)
                .and_then(|building| building.block_id),
            Some(first_block_id)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&first_build_pos)
                .and_then(|building| building.health_bits),
            Some(first_health_bits)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&second_build_pos)
                .cloned(),
            second_before
        );
        assert_eq!(
            session
                .state()
                .applied_loaded_world_block_snapshot_extra_entry_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_loaded_world_block_snapshot_extra_entry_count,
            1
        );
        assert_eq!(
            session
                .state()
                .failed_loaded_world_block_snapshot_extra_entry_parse_count,
            1
        );
        let expected_error = format!(
            "loaded_world_block_snapshot_entry_1_block_id_mismatch:{}/{}",
            second_center_block_id,
            second_block_id.wrapping_add(1)
        );
        assert_eq!(
            session
                .state()
                .last_loaded_world_block_snapshot_extra_entry_parse_error
                .as_deref(),
            Some(expected_error.as_str())
        );
    }

    #[test]
    fn state_snapshot_packet_applies_header_fields_and_core_data() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::StateSnapshot)
        );
        assert!(session.state().seen_state_snapshot);
        assert_eq!(session.state().applied_state_snapshot_count, 1);
        assert_eq!(
            session.state().last_state_snapshot,
            Some(AppliedStateSnapshot {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 0,
                paused: false,
                game_over: false,
                time_data: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                core_data: sample_snapshot_packet("stateSnapshot.coreData"),
            })
        );
    }

    #[test]
    fn malformed_state_snapshot_keeps_existing_applied_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let valid_packet = encode_packet(
            packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&valid_packet).unwrap();
        let expected = session.state().last_state_snapshot.clone();

        let truncated_payload = sample_snapshot_packet("stateSnapshot.packet")[..20].to_vec();
        let truncated_packet = encode_packet(packet_id, &truncated_payload, false).unwrap();

        let event = session.ingest_packet_bytes(&truncated_packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::StateSnapshot)
        );
        assert!(session.state().seen_state_snapshot);
        assert_eq!(
            session.state().last_snapshot_payload_len,
            truncated_payload.len()
        );
        assert_eq!(session.state().applied_state_snapshot_count, 1);
        assert_eq!(session.state().last_state_snapshot, expected);
    }

    #[test]
    fn malformed_entity_snapshot_keeps_existing_local_player_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let expected_state = (
            session.state().world_player_unit_kind,
            session.state().world_player_unit_value,
            session.state().world_player_x_bits,
            session.state().world_player_y_bits,
        );
        let input = session.snapshot_input_mut();
        input.unit_id = Some(321);
        input.dead = false;
        input.position = Some((12.0, 34.0));
        input.view_center = Some((12.0, 34.0));

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let truncated_payload = sample_snapshot_packet("entitySnapshot.packet")[..12].to_vec();
        let packet = encode_packet(packet_id, &truncated_payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            (
                session.state().world_player_unit_kind,
                session.state().world_player_unit_value,
                session.state().world_player_x_bits,
                session.state().world_player_y_bits,
            ),
            expected_state
        );
        assert_eq!(session.state().received_entity_snapshot_count, 1);
        assert_eq!(session.state().last_entity_snapshot_amount, Some(2));
        assert_eq!(session.state().last_entity_snapshot_body_len, Some(155));
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 0);
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_fallback_count,
            0
        );
        assert_eq!(session.state().last_entity_snapshot_target_player_id, None);
        assert!(
            !session
                .state()
                .last_entity_snapshot_used_projection_fallback
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
        assert_eq!(session.state().failed_entity_snapshot_parse_count, 1);
        assert_eq!(
            session.state().last_entity_snapshot_parse_error.as_deref(),
            Some("entity_snapshot_body_len_out_of_range:155/8")
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, Some(321));
        assert!(!input.dead);
        assert_eq!(input.position, Some((12.0, 34.0)));
        assert_eq!(input.view_center, Some((12.0, 34.0)));
    }

    #[test]
    fn entity_snapshot_without_matching_local_player_still_tracks_envelope_observability() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        session.state.world_player_id = Some(999_999);

        let entity_snapshot_wire =
            encode_packet(44, &sample_snapshot_packet("entitySnapshot.packet"), false).unwrap();
        let event = session.ingest_packet_bytes(&entity_snapshot_wire).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(session.state().received_entity_snapshot_count, 1);
        assert_eq!(session.state().last_entity_snapshot_amount, Some(2));
        assert_eq!(session.state().last_entity_snapshot_body_len, Some(155));
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 1);
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            1
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_fallback_count,
            0
        );
        assert_eq!(
            session.state().last_entity_snapshot_target_player_id,
            Some(999_999)
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_used_projection_fallback
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
        assert_eq!(session.state().failed_entity_snapshot_parse_count, 0);
        assert_eq!(session.state().last_entity_snapshot_parse_error, None);
    }

    #[test]
    fn entity_snapshot_can_sync_local_player_via_projection_fallback_when_world_player_id_missing()
    {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let local_player_id = session.state().world_player_id.unwrap();
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .local_player_entity_id,
            Some(local_player_id)
        );
        session.state.world_player_id = None;
        session.state.world_player_unit_kind = None;
        session.state.world_player_unit_value = None;
        session.state.world_player_x_bits = None;
        session.state.world_player_y_bits = None;
        let input = session.snapshot_input_mut();
        input.unit_id = Some(999);
        input.dead = true;
        input.position = Some((999.0, 999.0));
        input.view_center = Some((999.0, 999.0));

        let entity_snapshot_wire =
            encode_packet(44, &sample_snapshot_packet("entitySnapshot.packet"), false).unwrap();
        let event = session.ingest_packet_bytes(&entity_snapshot_wire).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(session.state().world_player_id, Some(local_player_id));
        assert_eq!(session.state().world_player_unit_kind, Some(2));
        assert_eq!(session.state().world_player_unit_value, Some(100));
        assert_eq!(session.state().world_player_x_bits, Some(0.0f32.to_bits()));
        assert_eq!(session.state().world_player_y_bits, Some(0.0f32.to_bits()));
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 1);
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_count,
            1
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_fallback_count,
            1
        );
        assert_eq!(
            session.state().last_entity_snapshot_target_player_id,
            Some(local_player_id)
        );
        assert!(
            session
                .state()
                .last_entity_snapshot_used_projection_fallback
        );
        assert!(
            session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
    }

    #[test]
    fn entity_snapshot_with_short_body_keeps_local_player_state_without_panicking() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let local_player_id = session.state().world_player_id.unwrap();
        let expected_state = (
            session.state().world_player_unit_kind,
            session.state().world_player_unit_value,
            session.state().world_player_x_bits,
            session.state().world_player_y_bits,
        );
        let input = session.snapshot_input_mut();
        input.unit_id = Some(321);
        input.dead = false;
        input.position = Some((12.0, 34.0));
        input.view_center = Some((12.0, 34.0));

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u16.to_be_bytes());
        payload.extend_from_slice(&4u16.to_be_bytes());
        payload.extend_from_slice(&local_player_id.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(
            (
                session.state().world_player_unit_kind,
                session.state().world_player_unit_value,
                session.state().world_player_x_bits,
                session.state().world_player_y_bits,
            ),
            expected_state
        );
        assert_eq!(session.state().received_entity_snapshot_count, 1);
        assert_eq!(session.state().last_entity_snapshot_amount, Some(1));
        assert_eq!(session.state().last_entity_snapshot_body_len, Some(4));
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 1);
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            1
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(session.state().failed_entity_snapshot_parse_count, 0);
        assert_eq!(session.state().last_entity_snapshot_parse_error, None);
        assert!(
            !session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, Some(321));
        assert!(!input.dead);
        assert_eq!(input.position, Some((12.0, 34.0)));
        assert_eq!(input.view_center, Some((12.0, 34.0)));
    }

    #[test]
    fn entity_snapshot_rejects_ambiguous_local_player_sync_matches() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let local_player_id = session.state().world_player_id.unwrap();
        session.state.world_player_unit_kind = Some(9);
        session.state.world_player_unit_value = Some(999);
        session.state.world_player_x_bits = Some(12.0f32.to_bits());
        session.state.world_player_y_bits = Some(34.0f32.to_bits());
        let input = session.snapshot_input_mut();
        input.unit_id = Some(555);
        input.dead = false;
        input.position = Some((12.0, 34.0));
        input.view_center = Some((12.0, 34.0));

        let sample_payload = sample_snapshot_packet("entitySnapshot.packet");
        let sample_body_len = u16::from_be_bytes([sample_payload[2], sample_payload[3]]) as usize;
        let sample_body = &sample_payload[4..4 + sample_body_len];
        let mut payload = Vec::new();
        payload.extend_from_slice(&4u16.to_be_bytes());
        payload.extend_from_slice(&u16::try_from(sample_body.len() * 2).unwrap().to_be_bytes());
        payload.extend_from_slice(sample_body);
        payload.extend_from_slice(sample_body);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::EntitySnapshot)
        );
        assert_eq!(session.state().received_entity_snapshot_count, 1);
        assert_eq!(session.state().last_entity_snapshot_amount, Some(4));
        assert_eq!(
            session.state().last_entity_snapshot_body_len,
            Some(sample_body.len() * 2)
        );
        assert_eq!(session.state().entity_snapshot_with_local_target_count, 1);
        assert_eq!(
            session
                .state()
                .missed_local_player_sync_from_entity_snapshot_count,
            1
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_count,
            0
        );
        assert_eq!(
            session
                .state()
                .applied_local_player_sync_from_entity_snapshot_fallback_count,
            0
        );
        assert_eq!(
            session
                .state()
                .ambiguous_local_player_sync_from_entity_snapshot_count,
            1
        );
        assert_eq!(
            session.state().last_entity_snapshot_target_player_id,
            Some(local_player_id)
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_used_projection_fallback
        );
        assert!(
            !session
                .state()
                .last_entity_snapshot_local_player_sync_applied
        );
        assert!(
            session
                .state()
                .last_entity_snapshot_local_player_sync_ambiguous
        );
        assert_eq!(
            session
                .state()
                .last_entity_snapshot_local_player_sync_match_count,
            2
        );
        assert_eq!(session.state().failed_entity_snapshot_parse_count, 0);
        assert_eq!(session.state().last_entity_snapshot_parse_error, None);
        assert_eq!(session.state().world_player_unit_kind, Some(9));
        assert_eq!(session.state().world_player_unit_value, Some(999));
        assert_eq!(session.state().world_player_x_bits, Some(12.0f32.to_bits()));
        assert_eq!(session.state().world_player_y_bits, Some(34.0f32.to_bits()));
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, Some(555));
        assert!(!input.dead);
        assert_eq!(input.position, Some((12.0, 34.0)));
        assert_eq!(input.view_center, Some((12.0, 34.0)));
    }

    #[test]
    fn hidden_snapshot_applies_to_bootstrap_local_player_entity_row() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let local_player_id = session.state().world_player_id.unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&local_player_id));

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::HiddenSnapshot.method_name())
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&1i32.to_be_bytes());
        payload.extend_from_slice(&local_player_id.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::HiddenSnapshot)
        );
        assert_eq!(session.state().entity_table_projection.hidden_count, 1);
        assert_eq!(
            session.state().entity_table_projection.hidden_apply_count,
            1
        );
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .get(&local_player_id)
            .is_some_and(|entity| entity.hidden));
    }

    #[test]
    fn ready_session_ignores_state_snapshot_for_snapshot_timeout_after_entity_snapshot() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
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
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());

        let entity_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let entity_packet = encode_packet(
            entity_packet_id,
            &sample_snapshot_packet("entitySnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&entity_packet).unwrap();

        session.advance_time(1_000).unwrap();

        let state_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let state_packet = encode_packet(
            state_packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&state_packet).unwrap();

        let actions = session.advance_time(1_201).unwrap();

        assert_eq!(
            actions,
            vec![ClientSessionAction::TimedOut { idle_ms: 1_201 }]
        );
        assert!(session.state().connection_timed_out);
    }

    #[test]
    fn ready_session_state_snapshot_does_not_extend_timeout_before_first_entity_snapshot() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
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
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());

        session.advance_time(1_000).unwrap();

        let state_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let state_packet = encode_packet(
            state_packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&state_packet).unwrap();

        let actions = session.advance_time(1_201).unwrap();

        assert_eq!(
            actions,
            vec![ClientSessionAction::TimedOut { idle_ms: 1_201 }]
        );
        assert!(session.state().connection_timed_out);
    }

    #[test]
    fn player_spawn_packet_updates_local_player_spawn_position() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        let local_player_id = session.state().world_player_id.unwrap();

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "playerSpawn")
            .unwrap()
            .packet_id;
        let tile_pos = ((4i16 as u16 as u32) << 16 | 4i16 as u16 as u32) as i32;
        let mut payload = Vec::new();
        payload.extend_from_slice(&tile_pos.to_be_bytes());
        payload.extend_from_slice(&7i32.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::PlayerSpawned {
                player_id: 7,
                x: 32.0,
                y: 32.0
            }
        );
        assert_eq!(session.state().world_player_x_bits, Some(32.0f32.to_bits()));
        assert_eq!(session.state().world_player_y_bits, Some(32.0f32.to_bits()));
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, None);
        assert!(!input.dead);
        assert_eq!(input.position, Some((32.0, 32.0)));
        assert_eq!(input.view_center, Some((32.0, 32.0)));
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .local_player_entity_id,
            Some(local_player_id)
        );
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .by_entity_id
                .get(&local_player_id)
                .map(|entity| (entity.x_bits, entity.y_bits)),
            Some((32.0f32.to_bits(), 32.0f32.to_bits()))
        );
    }

    #[test]
    fn send_message_packet_emits_server_message_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_typeio_string_payload("[accent]hello"),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ServerMessage {
                message: "[accent]hello".to_string()
            }
        );
        assert_eq!(session.state().received_server_message_count, 1);
        assert_eq!(
            session.state().last_server_message.as_deref(),
            Some("[accent]hello")
        );
    }

    #[test]
    fn connect_redirect_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connect" && entry.params.len() == 2)
            .unwrap()
            .packet_id;
        let mut payload = encode_typeio_string_payload("127.0.0.1");
        payload.extend_from_slice(&6567i32.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ConnectRedirectRequested {
                ip: "127.0.0.1".to_string(),
                port: 6567,
            }
        );
        assert_eq!(session.state().received_connect_redirect_count, 1);
        assert_eq!(
            session.state().last_connect_redirect_ip.as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(session.state().last_connect_redirect_port, Some(6567));
    }

    #[test]
    fn connect_redirect_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "connect" && entry.params.len() == 2)
            .unwrap()
            .packet_id;
        let packet =
            encode_packet(packet_id, &encode_typeio_string_payload("127.0.0.1"), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "connect".to_string(),
                    packet_class: "mindustry.gen.ConnectCallPacket".to_string(),
                }),
            }
        );
        assert_eq!(session.state().received_connect_redirect_count, 0);
        assert_eq!(session.state().last_connect_redirect_ip, None);
        assert_eq!(session.state().last_connect_redirect_port, None);
    }

    #[test]
    fn send_message_with_sender_packet_emits_chat_message_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 3)
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("[accent]bot[]: hello"));
        payload.extend_from_slice(&encode_typeio_string_payload("hello"));
        payload.extend_from_slice(&42i32.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ChatMessage {
                message: "[accent]bot[]: hello".to_string(),
                unformatted: Some("hello".to_string()),
                sender_entity_id: Some(42),
            }
        );
        assert_eq!(session.state().received_chat_message_count, 1);
        assert_eq!(
            session.state().last_chat_message.as_deref(),
            Some("[accent]bot[]: hello")
        );
        assert_eq!(
            session.state().last_chat_unformatted.as_deref(),
            Some("hello")
        );
        assert_eq!(session.state().last_chat_sender_entity_id, Some(42));
    }

    #[test]
    fn client_packet_reliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketReliable")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("mod-channel"));
        payload.extend_from_slice(&encode_typeio_string_payload("{\"ok\":1}"));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientPacketReliable {
                packet_type: "mod-channel".to_string(),
                contents: "{\"ok\":1}".to_string(),
            }
        );
        assert_eq!(session.state().received_client_packet_reliable_count, 1);
        assert_eq!(
            session.state().last_client_packet_reliable_type.as_deref(),
            Some("mod-channel")
        );
        assert_eq!(
            session
                .state()
                .last_client_packet_reliable_contents
                .as_deref(),
            Some("{\"ok\":1}")
        );
    }

    #[test]
    fn client_packet_unreliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketUnreliable")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("mod-channel-u"));
        payload.extend_from_slice(&encode_typeio_string_payload("{\"seq\":2}"));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientPacketUnreliable {
                packet_type: "mod-channel-u".to_string(),
                contents: "{\"seq\":2}".to_string(),
            }
        );
        assert_eq!(session.state().received_client_packet_unreliable_count, 1);
        assert_eq!(
            session
                .state()
                .last_client_packet_unreliable_type
                .as_deref(),
            Some("mod-channel-u")
        );
        assert_eq!(
            session
                .state()
                .last_client_packet_unreliable_contents
                .as_deref(),
            Some("{\"seq\":2}")
        );
    }

    #[test]
    fn client_binary_packet_reliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketReliable")
            .unwrap()
            .packet_id;
        let contents = vec![0xde, 0xad, 0xbe, 0xef];
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("bin-r"));
        payload.extend_from_slice(&encode_typeio_bytes_payload(&contents));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type: "bin-r".to_string(),
                contents: contents.clone(),
            }
        );
        assert_eq!(
            session.state().received_client_binary_packet_reliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_reliable_type
                .as_deref(),
            Some("bin-r")
        );
        assert_eq!(
            session.state().last_client_binary_packet_reliable_contents,
            Some(contents)
        );
    }

    #[test]
    fn client_binary_packet_unreliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketUnreliable")
            .unwrap()
            .packet_id;
        let contents = vec![1, 2, 3, 4, 5];
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("bin-u"));
        payload.extend_from_slice(&encode_typeio_bytes_payload(&contents));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type: "bin-u".to_string(),
                contents: contents.clone(),
            }
        );
        assert_eq!(
            session
                .state()
                .received_client_binary_packet_unreliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_unreliable_type
                .as_deref(),
            Some("bin-u")
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_unreliable_contents,
            Some(contents)
        );
    }

    #[test]
    fn server_packet_reliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverPacketReliable")
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_client_packet_payload("server.text.r", "payload-r"),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientPacketReliable {
                packet_type: "server.text.r".to_string(),
                contents: "payload-r".to_string(),
            }
        );
        assert_eq!(session.state().received_client_packet_reliable_count, 1);
        assert_eq!(
            session.state().last_client_packet_reliable_type.as_deref(),
            Some("server.text.r")
        );
        assert_eq!(
            session
                .state()
                .last_client_packet_reliable_contents
                .as_deref(),
            Some("payload-r")
        );
    }

    #[test]
    fn server_packet_unreliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverPacketUnreliable")
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_client_packet_payload("server.text.u", "payload-u"),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientPacketUnreliable {
                packet_type: "server.text.u".to_string(),
                contents: "payload-u".to_string(),
            }
        );
        assert_eq!(session.state().received_client_packet_unreliable_count, 1);
        assert_eq!(
            session
                .state()
                .last_client_packet_unreliable_type
                .as_deref(),
            Some("server.text.u")
        );
        assert_eq!(
            session
                .state()
                .last_client_packet_unreliable_contents
                .as_deref(),
            Some("payload-u")
        );
    }

    #[test]
    fn server_binary_packet_reliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverBinaryPacketReliable")
            .unwrap()
            .packet_id;
        let contents = vec![0x10, 0x20, 0x30];
        let packet = encode_packet(
            packet_id,
            &encode_client_binary_packet_payload("server.bin.r", &contents),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type: "server.bin.r".to_string(),
                contents: contents.clone(),
            }
        );
        assert_eq!(
            session.state().received_client_binary_packet_reliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_reliable_type
                .as_deref(),
            Some("server.bin.r")
        );
        assert_eq!(
            session.state().last_client_binary_packet_reliable_contents,
            Some(contents)
        );
    }

    #[test]
    fn server_binary_packet_unreliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverBinaryPacketUnreliable")
            .unwrap()
            .packet_id;
        let contents = vec![0xaa, 0xbb, 0xcc, 0xdd];
        let packet = encode_packet(
            packet_id,
            &encode_client_binary_packet_payload("server.bin.u", &contents),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type: "server.bin.u".to_string(),
                contents: contents.clone(),
            }
        );
        assert_eq!(
            session
                .state()
                .received_client_binary_packet_unreliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_unreliable_type
                .as_deref(),
            Some("server.bin.u")
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_unreliable_contents,
            Some(contents)
        );
    }

    #[test]
    fn client_logic_data_reliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataReliable")
            .unwrap()
            .packet_id;
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        let packet = encode_packet(
            packet_id,
            &encode_client_logic_data_payload("logic.reliable", &value),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientLogicDataReliable {
                channel: "logic.reliable".to_string(),
                value: value.clone(),
            }
        );
        assert_eq!(session.state().received_client_logic_data_reliable_count, 1);
        assert_eq!(
            session
                .state()
                .last_client_logic_data_reliable_channel
                .as_deref(),
            Some("logic.reliable")
        );
        assert_eq!(
            session.state().last_client_logic_data_reliable_value,
            Some(value)
        );
    }

    #[test]
    fn client_logic_data_unreliable_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataUnreliable")
            .unwrap()
            .packet_id;
        let value = TypeIoObject::UnitCommand(42);
        let packet = encode_packet(
            packet_id,
            &encode_client_logic_data_payload("logic.unreliable", &value),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientLogicDataUnreliable {
                channel: "logic.unreliable".to_string(),
                value,
            }
        );
        assert_eq!(
            session.state().received_client_logic_data_unreliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_logic_data_unreliable_channel
                .as_deref(),
            Some("logic.unreliable")
        );
        assert_eq!(
            session.state().last_client_logic_data_unreliable_value,
            Some(TypeIoObject::UnitCommand(42))
        );
    }

    #[test]
    fn registered_client_logic_data_handlers_dispatch_for_reliable_and_unreliable_packets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataReliable")
            .unwrap()
            .packet_id;
        let unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataUnreliable")
            .unwrap()
            .packet_id;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let first_seen = Rc::clone(&seen);
        session.add_client_logic_data_handler("logic-channel", move |transport, value| {
            first_seen.borrow_mut().push((transport, value.clone()));
        });

        let reliable_packet = encode_packet(
            reliable_packet_id,
            &encode_client_logic_data_payload("logic-channel", &TypeIoObject::Int(7)),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&reliable_packet).unwrap();

        let ignored_packet = encode_packet(
            unreliable_packet_id,
            &encode_client_logic_data_payload("other-channel", &TypeIoObject::Bool(true)),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&ignored_packet).unwrap();

        let unreliable_packet = encode_packet(
            unreliable_packet_id,
            &encode_client_logic_data_payload("logic-channel", &TypeIoObject::String(None)),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&unreliable_packet).unwrap();

        assert_eq!(
            seen.borrow().as_slice(),
            [
                (ClientLogicDataTransport::Reliable, TypeIoObject::Int(7)),
                (
                    ClientLogicDataTransport::Unreliable,
                    TypeIoObject::String(None)
                ),
            ]
        );
    }

    #[test]
    fn registered_client_packet_handlers_dispatch_for_reliable_and_unreliable_packets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketReliable")
            .unwrap()
            .packet_id;
        let unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketUnreliable")
            .unwrap()
            .packet_id;
        let server_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverPacketReliable")
            .unwrap()
            .packet_id;
        let server_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverPacketUnreliable")
            .unwrap()
            .packet_id;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let first_seen = Rc::clone(&seen);
        session.add_client_packet_handler("mod-channel", move |contents| {
            first_seen.borrow_mut().push(format!("first:{contents}"));
        });
        let second_seen = Rc::clone(&seen);
        session.add_client_packet_handler("mod-channel", move |contents| {
            second_seen.borrow_mut().push(format!("second:{contents}"));
        });

        let reliable_packet = encode_packet(
            reliable_packet_id,
            &encode_client_packet_payload("mod-channel", "alpha"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&reliable_packet).unwrap();

        let ignored_packet = encode_packet(
            unreliable_packet_id,
            &encode_client_packet_payload("other-channel", "skip"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&ignored_packet).unwrap();

        let unreliable_packet = encode_packet(
            unreliable_packet_id,
            &encode_client_packet_payload("mod-channel", "beta"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&unreliable_packet).unwrap();

        assert_eq!(
            seen.borrow().as_slice(),
            ["first:alpha", "second:alpha", "first:beta", "second:beta"]
        );

        let server_reliable_packet = encode_packet(
            server_reliable_packet_id,
            &encode_client_packet_payload("mod-channel", "gamma"),
            false,
        )
        .unwrap();
        session
            .ingest_packet_bytes(&server_reliable_packet)
            .unwrap();

        let server_unreliable_packet = encode_packet(
            server_unreliable_packet_id,
            &encode_client_packet_payload("mod-channel", "delta"),
            false,
        )
        .unwrap();
        session
            .ingest_packet_bytes(&server_unreliable_packet)
            .unwrap();

        assert_eq!(
            seen.borrow().as_slice(),
            [
                "first:alpha",
                "second:alpha",
                "first:beta",
                "second:beta",
                "first:gamma",
                "second:gamma",
                "first:delta",
                "second:delta",
            ]
        );
    }

    #[test]
    fn registered_client_binary_packet_handlers_dispatch_for_reliable_and_unreliable_packets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketReliable")
            .unwrap()
            .packet_id;
        let unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketUnreliable")
            .unwrap()
            .packet_id;
        let server_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverBinaryPacketReliable")
            .unwrap()
            .packet_id;
        let server_unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "serverBinaryPacketUnreliable")
            .unwrap()
            .packet_id;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let first_seen = Rc::clone(&seen);
        session.add_client_binary_packet_handler("mod-bin", move |contents| {
            first_seen.borrow_mut().push(contents.to_vec());
        });

        let reliable_packet = encode_packet(
            reliable_packet_id,
            &encode_client_binary_packet_payload("mod-bin", &[1, 2, 3]),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&reliable_packet).unwrap();

        let ignored_packet = encode_packet(
            unreliable_packet_id,
            &encode_client_binary_packet_payload("other-bin", &[7, 7]),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&ignored_packet).unwrap();

        let unreliable_packet = encode_packet(
            unreliable_packet_id,
            &encode_client_binary_packet_payload("mod-bin", &[9, 8]),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&unreliable_packet).unwrap();

        assert_eq!(seen.borrow().as_slice(), [vec![1, 2, 3], vec![9, 8]]);

        let server_reliable_packet = encode_packet(
            server_reliable_packet_id,
            &encode_client_binary_packet_payload("mod-bin", &[4, 5, 6]),
            false,
        )
        .unwrap();
        session
            .ingest_packet_bytes(&server_reliable_packet)
            .unwrap();

        let server_unreliable_packet = encode_packet(
            server_unreliable_packet_id,
            &encode_client_binary_packet_payload("mod-bin", &[7, 6, 5, 4]),
            false,
        )
        .unwrap();
        session
            .ingest_packet_bytes(&server_unreliable_packet)
            .unwrap();

        assert_eq!(
            seen.borrow().as_slice(),
            [vec![1, 2, 3], vec![9, 8], vec![4, 5, 6], vec![7, 6, 5, 4]]
        );
    }

    #[test]
    fn set_rule_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRule")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("wave"));
        payload.extend_from_slice(&encode_typeio_string_payload("{\"spacing\":60}"));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SetRuleApplied {
                rule: "wave".to_string(),
                json_data: "{\"spacing\":60}".to_string(),
            }
        );
        assert_eq!(session.state().received_set_rule_count, 1);
        assert_eq!(session.state().failed_set_rule_parse_count, 0);
        assert_eq!(session.state().last_set_rule_parse_error, None);
        assert_eq!(session.state().last_set_rule_parse_error_payload_len, None);
        assert_eq!(session.state().last_set_rule_name.as_deref(), Some("wave"));
        assert_eq!(
            session.state().last_set_rule_json_data.as_deref(),
            Some("{\"spacing\":60}")
        );
        assert_eq!(
            session
                .state()
                .rules_projection
                .applied_set_rule_patch_count,
            1
        );
        assert_eq!(
            session
                .state()
                .rules_projection
                .unknown_set_rule_patch_count,
            1
        );
        assert_eq!(
            session
                .state()
                .rules_projection
                .last_unknown_set_rule_patch_name
                .as_deref(),
            Some("wave")
        );
    }

    #[test]
    fn set_rule_packet_applies_known_rules_projection_fields() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRule")
            .unwrap()
            .packet_id;

        let mut pvp_payload = Vec::new();
        pvp_payload.extend_from_slice(&encode_typeio_string_payload("pvp"));
        pvp_payload.extend_from_slice(&encode_typeio_string_payload("true"));
        let pvp_packet = encode_packet(packet_id, &pvp_payload, false).unwrap();
        let pvp_event = session.ingest_packet_bytes(&pvp_packet).unwrap();

        assert_eq!(
            pvp_event,
            ClientSessionEvent::SetRuleApplied {
                rule: "pvp".to_string(),
                json_data: "true".to_string(),
            }
        );
        assert_eq!(session.state().rules_projection.pvp, Some(true));
        assert_eq!(session.state().rules_projection.default_team_id, None);

        let mut default_team_payload = Vec::new();
        default_team_payload.extend_from_slice(&encode_typeio_string_payload("defaultTeam"));
        default_team_payload.extend_from_slice(&encode_typeio_string_payload("1"));
        let default_team_packet = encode_packet(packet_id, &default_team_payload, false).unwrap();
        let default_team_event = session.ingest_packet_bytes(&default_team_packet).unwrap();

        assert_eq!(
            default_team_event,
            ClientSessionEvent::SetRuleApplied {
                rule: "defaultTeam".to_string(),
                json_data: "1".to_string(),
            }
        );
        assert_eq!(session.state().received_set_rule_count, 2);
        assert_eq!(session.state().failed_set_rule_parse_count, 0);
        assert_eq!(
            session.state().last_set_rule_name.as_deref(),
            Some("defaultTeam")
        );
        assert_eq!(
            session.state().last_set_rule_json_data.as_deref(),
            Some("1")
        );
        assert_eq!(session.state().rules_projection.pvp, Some(true));
        assert_eq!(session.state().rules_projection.default_team_id, Some(1));
        assert_eq!(
            session
                .state()
                .rules_projection
                .applied_set_rule_patch_count,
            2
        );
        assert_eq!(
            session
                .state()
                .rules_projection
                .unknown_set_rule_patch_count,
            0
        );
        assert_eq!(
            session
                .state()
                .rules_projection
                .ignored_set_rule_patch_count,
            0
        );
    }

    #[test]
    fn set_rule_packet_with_trailing_bytes_is_ignored_and_records_parse_fail() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRule")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("waves"));
        payload.extend_from_slice(&encode_typeio_string_payload("true"));
        payload.push(0xff);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_set_rule_count, 0);
        assert_eq!(session.state().failed_set_rule_parse_count, 1);
        assert!(session.state().last_set_rule_parse_error.is_some());
        assert_eq!(
            session.state().last_set_rule_parse_error_payload_len,
            Some(payload.len())
        );
        assert_eq!(session.state().last_set_rule_name, None);
        assert_eq!(session.state().last_set_rule_json_data, None);
    }

    #[test]
    fn set_rules_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRules")
            .unwrap()
            .packet_id;
        let payload = encode_length_prefixed_utf8_payload(
            "{\"waveSpacing\":120.0,\"waves\":true,\"pvp\":true,\"canGameOver\":false,\"coreCapture\":true,\"winWave\":21,\"defaultTeam\":1,\"waveTeam\":2}",
        );
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::RulesUpdatedRaw {
                json_data: "{\"waveSpacing\":120.0,\"waves\":true,\"pvp\":true,\"canGameOver\":false,\"coreCapture\":true,\"winWave\":21,\"defaultTeam\":1,\"waveTeam\":2}".to_string(),
            }
        );
        assert_eq!(session.state().received_set_rules_count, 1);
        assert_eq!(session.state().failed_set_rules_parse_count, 0);
        assert_eq!(session.state().last_set_rules_parse_error, None);
        assert_eq!(session.state().last_set_rules_parse_error_payload_len, None);
        assert_eq!(
            session.state().last_set_rules_json_data.as_deref(),
            Some("{\"waveSpacing\":120.0,\"waves\":true,\"pvp\":true,\"canGameOver\":false,\"coreCapture\":true,\"winWave\":21,\"defaultTeam\":1,\"waveTeam\":2}")
        );
        assert_eq!(session.state().rules_projection.waves, Some(true));
        assert_eq!(session.state().rules_projection.wave_spacing, Some(120.0));
        assert_eq!(session.state().rules_projection.pvp, Some(true));
        assert_eq!(session.state().rules_projection.can_game_over, Some(false));
        assert_eq!(session.state().rules_projection.core_capture, Some(true));
        assert_eq!(session.state().rules_projection.win_wave, Some(21));
        assert_eq!(session.state().rules_projection.default_team_id, Some(1));
        assert_eq!(session.state().rules_projection.wave_team_id, Some(2));
        assert_eq!(
            session
                .state()
                .rules_projection
                .replaced_from_set_rules_count,
            1
        );
    }

    #[test]
    fn set_objectives_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setObjectives")
            .unwrap()
            .packet_id;
        let payload =
            encode_length_prefixed_utf8_payload("[{\"type\":\"Timer\",\"duration\":90.0}]");
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ObjectivesUpdatedRaw {
                json_data: "[{\"type\":\"Timer\",\"duration\":90.0}]".to_string(),
            }
        );
        assert_eq!(session.state().received_set_objectives_count, 1);
        assert_eq!(session.state().failed_set_objectives_parse_count, 0);
        assert_eq!(session.state().last_set_objectives_parse_error, None);
        assert_eq!(
            session.state().last_set_objectives_parse_error_payload_len,
            None
        );
        assert_eq!(
            session.state().last_set_objectives_json_data.as_deref(),
            Some("[{\"type\":\"Timer\",\"duration\":90.0}]")
        );
        assert_eq!(session.state().objectives_projection.objectives.len(), 1);
        assert_eq!(
            session.state().objectives_projection.objectives[0]
                .objective_type
                .as_deref(),
            Some("Timer")
        );
        assert_eq!(
            session
                .state()
                .objectives_projection
                .replaced_from_set_objectives_count,
            1
        );
    }

    #[test]
    fn set_rules_packet_with_truncated_json_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRules")
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &5i32.to_be_bytes(), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_set_rules_count, 0);
        assert_eq!(session.state().failed_set_rules_parse_count, 1);
        assert!(session.state().last_set_rules_parse_error.is_some());
        assert_eq!(
            session.state().last_set_rules_parse_error_payload_len,
            Some(4)
        );
        assert_eq!(session.state().last_set_rules_json_data, None);
    }

    #[test]
    fn set_objectives_packet_with_truncated_json_is_ignored_and_records_parse_fail() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setObjectives")
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &5i32.to_be_bytes(), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_set_objectives_count, 0);
        assert_eq!(session.state().failed_set_objectives_parse_count, 1);
        assert!(session.state().last_set_objectives_parse_error.is_some());
        assert_eq!(
            session.state().last_set_objectives_parse_error_payload_len,
            Some(4)
        );
        assert_eq!(session.state().last_set_objectives_json_data, None);
    }

    #[test]
    fn client_packet_reliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketReliable")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("golden.reliable"));
        payload.extend_from_slice(&encode_typeio_string_payload("payload-reliable"));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientPacketReliable {
                packet_type: "golden.reliable".to_string(),
                contents: "payload-reliable".to_string(),
            }
        );
        assert_eq!(session.state().received_client_packet_reliable_count, 1);
        assert_eq!(
            session.state().last_client_packet_reliable_type.as_deref(),
            Some("golden.reliable")
        );
        assert_eq!(
            session
                .state()
                .last_client_packet_reliable_contents
                .as_deref(),
            Some("payload-reliable")
        );
    }

    #[test]
    fn client_packet_unreliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketUnreliable")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("golden.unreliable"));
        payload.extend_from_slice(&encode_typeio_string_payload("payload-unreliable"));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientPacketUnreliable {
                packet_type: "golden.unreliable".to_string(),
                contents: "payload-unreliable".to_string(),
            }
        );
        assert_eq!(session.state().received_client_packet_unreliable_count, 1);
        assert_eq!(
            session
                .state()
                .last_client_packet_unreliable_type
                .as_deref(),
            Some("golden.unreliable")
        );
        assert_eq!(
            session
                .state()
                .last_client_packet_unreliable_contents
                .as_deref(),
            Some("payload-unreliable")
        );
    }

    #[test]
    fn client_binary_packet_reliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketReliable")
            .unwrap()
            .packet_id;
        let contents = vec![0x00, 0x11, 0x22, 0x33, 0xaa, 0xff];
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("golden.binary.reliable"));
        payload.extend_from_slice(&encode_typeio_bytes_payload(&contents));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type: "golden.binary.reliable".to_string(),
                contents: contents.clone(),
            }
        );
        assert_eq!(
            session.state().received_client_binary_packet_reliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_reliable_type
                .as_deref(),
            Some("golden.binary.reliable")
        );
        assert_eq!(
            session.state().last_client_binary_packet_reliable_contents,
            Some(contents)
        );
    }

    #[test]
    fn client_binary_packet_unreliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketUnreliable")
            .unwrap()
            .packet_id;
        let contents = vec![0x55, 0x66, 0x77, 0x00, 0x7f];
        let mut payload = Vec::new();
        payload.extend_from_slice(&encode_typeio_string_payload("golden.binary.unreliable"));
        payload.extend_from_slice(&encode_typeio_bytes_payload(&contents));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type: "golden.binary.unreliable".to_string(),
                contents: contents.clone(),
            }
        );
        assert_eq!(
            session
                .state()
                .received_client_binary_packet_unreliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_unreliable_type
                .as_deref(),
            Some("golden.binary.unreliable")
        );
        assert_eq!(
            session
                .state()
                .last_client_binary_packet_unreliable_contents,
            Some(contents)
        );
    }

    #[test]
    fn client_logic_data_reliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataReliable")
            .unwrap()
            .packet_id;
        let value = TypeIoObject::String(Some("logic-value".to_string()));
        let packet = encode_packet(
            packet_id,
            &encode_client_logic_data_payload("golden.logic.reliable", &value),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientLogicDataReliable {
                channel: "golden.logic.reliable".to_string(),
                value: value.clone(),
            }
        );
        assert_eq!(session.state().received_client_logic_data_reliable_count, 1);
        assert_eq!(
            session
                .state()
                .last_client_logic_data_reliable_channel
                .as_deref(),
            Some("golden.logic.reliable")
        );
        assert_eq!(
            session.state().last_client_logic_data_reliable_value,
            Some(value)
        );
    }

    #[test]
    fn client_logic_data_unreliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataUnreliable")
            .unwrap()
            .packet_id;
        let value = TypeIoObject::BoolArray(vec![true, false, true]);
        let packet = encode_packet(
            packet_id,
            &encode_client_logic_data_payload("golden.logic.unreliable", &value),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ClientLogicDataUnreliable {
                channel: "golden.logic.unreliable".to_string(),
                value: value.clone(),
            }
        );
        assert_eq!(
            session.state().received_client_logic_data_unreliable_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_client_logic_data_unreliable_channel
                .as_deref(),
            Some("golden.logic.unreliable")
        );
        assert_eq!(
            session.state().last_client_logic_data_unreliable_value,
            Some(value)
        );
    }

    #[test]
    fn clear_objectives_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clearObjectives")
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &[], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(event, ClientSessionEvent::ObjectivesCleared);
        assert_eq!(session.state().received_clear_objectives_count, 1);
        assert_eq!(session.state().objectives_projection.cleared_count, 1);
        assert!(session.state().objectives_projection.objectives.is_empty());
    }

    #[test]
    fn complete_objective_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "completeObjective")
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &3i32.to_be_bytes(), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(event, ClientSessionEvent::ObjectiveCompleted { index: 3 });
        assert_eq!(session.state().received_complete_objective_count, 1);
        assert_eq!(session.state().last_complete_objective_index, Some(3));
        assert_eq!(
            session
                .state()
                .objectives_projection
                .complete_by_index_count,
            1
        );
        assert_eq!(
            session
                .state()
                .objectives_projection
                .complete_out_of_range_count,
            1
        );
        assert_eq!(
            session.state().objectives_projection.last_completed_index,
            Some(3)
        );
    }

    #[test]
    fn objectives_projection_tracks_replace_complete_and_clear_transitions() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_objectives_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setObjectives")
            .unwrap()
            .packet_id;
        let complete_objective_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "completeObjective")
            .unwrap()
            .packet_id;
        let clear_objectives_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clearObjectives")
            .unwrap()
            .packet_id;

        let set_payload =
            encode_length_prefixed_utf8_payload("[{\"type\":\"Timer\"},{\"type\":\"Destroy\"}]");
        let set_packet = encode_packet(set_objectives_packet_id, &set_payload, false).unwrap();
        session.ingest_packet_bytes(&set_packet).unwrap();

        assert_eq!(session.state().objectives_projection.objectives.len(), 2);
        assert!(!session.state().objectives_projection.objectives[0].completed);
        assert!(!session.state().objectives_projection.objectives[1].completed);

        let complete_packet =
            encode_packet(complete_objective_packet_id, &1i32.to_be_bytes(), false).unwrap();
        session.ingest_packet_bytes(&complete_packet).unwrap();

        assert!(!session.state().objectives_projection.objectives[0].completed);
        assert!(session.state().objectives_projection.objectives[1].completed);
        assert_eq!(
            session
                .state()
                .objectives_projection
                .complete_by_index_count,
            1
        );
        assert_eq!(
            session
                .state()
                .objectives_projection
                .complete_out_of_range_count,
            0
        );

        let clear_packet = encode_packet(clear_objectives_packet_id, &[], false).unwrap();
        session.ingest_packet_bytes(&clear_packet).unwrap();

        assert!(session.state().objectives_projection.objectives.is_empty());
        assert_eq!(session.state().objectives_projection.cleared_count, 1);
        assert_eq!(session.state().received_set_objectives_count, 1);
        assert_eq!(session.state().received_complete_objective_count, 1);
        assert_eq!(session.state().received_clear_objectives_count, 1);
    }

    #[test]
    fn remove_queue_block_packet_emits_event_and_prunes_local_plan() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.snapshot_input_mut().plans = Some(vec![
            ClientBuildPlan {
                tile: (1, 2),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (5, 6),
                breaking: true,
                block_id: None,
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeQueueBlock")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&5i32.to_be_bytes());
        payload.extend_from_slice(&6i32.to_be_bytes());
        payload.push(1);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::RemoveQueueBlock {
                x: 5,
                y: 6,
                breaking: true,
                removed_local_plan: true,
            }
        );
        assert_eq!(session.state().received_remove_queue_block_count, 1);
        assert_eq!(session.state().last_remove_queue_block_x, Some(5));
        assert_eq!(session.state().last_remove_queue_block_y, Some(6));
        assert_eq!(session.state().last_remove_queue_block_breaking, Some(true));
        assert!(session.state().last_remove_queue_block_removed_local_plan);
        assert_eq!(
            session
                .snapshot_input_mut()
                .plans
                .as_ref()
                .map(|plans| plans.len()),
            Some(1)
        );
        assert_eq!(
            session.snapshot_input_mut().plans.as_ref().unwrap()[0].tile,
            (1, 2)
        );
        assert_eq!(session.state().builder_queue_projection.queued_count, 1);
        assert_eq!(session.state().builder_queue_projection.inflight_count, 0);
        assert_eq!(session.state().builder_queue_projection.removed_count, 1);
        assert_eq!(
            session
                .state()
                .builder_queue_projection
                .orphan_authoritative_count,
            0
        );
        assert_eq!(
            session.state().builder_queue_projection.last_stage,
            Some(crate::session_state::BuilderPlanStage::Removed)
        );
        assert_eq!(session.state().builder_queue_projection.last_x, Some(5));
        assert_eq!(session.state().builder_queue_projection.last_y, Some(6));
        assert_eq!(
            session.state().builder_queue_projection.last_breaking,
            Some(true)
        );
        assert!(
            session
                .state()
                .builder_queue_projection
                .last_removed_local_plan
        );
        assert!(session.snapshot_input_mut().building);
    }

    #[test]
    fn remove_queue_block_packet_prunes_all_local_plans_for_same_tile() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.snapshot_input_mut().plans = Some(vec![
            ClientBuildPlan {
                tile: (5, 6),
                breaking: false,
                block_id: Some(0x0101),
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (5, 6),
                breaking: true,
                block_id: None,
                rotation: 0,
                config: ClientBuildPlanConfig::None,
            },
            ClientBuildPlan {
                tile: (1, 2),
                breaking: false,
                block_id: Some(0x0102),
                rotation: 1,
                config: ClientBuildPlanConfig::None,
            },
        ]);

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeQueueBlock")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&5i32.to_be_bytes());
        payload.extend_from_slice(&6i32.to_be_bytes());
        payload.push(1);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::RemoveQueueBlock {
                x: 5,
                y: 6,
                breaking: true,
                removed_local_plan: true,
            }
        );
        assert_eq!(
            session.snapshot_input_mut().plans,
            Some(vec![ClientBuildPlan {
                tile: (1, 2),
                breaking: false,
                block_id: Some(0x0102),
                rotation: 1,
                config: ClientBuildPlanConfig::None,
            }])
        );
    }

    #[test]
    fn remove_queue_block_packet_tracks_orphan_authoritative_without_local_plan() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeQueueBlock")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&8i32.to_be_bytes());
        payload.extend_from_slice(&9i32.to_be_bytes());
        payload.push(0);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::RemoveQueueBlock {
                x: 8,
                y: 9,
                breaking: false,
                removed_local_plan: false,
            }
        );
        assert_eq!(session.state().builder_queue_projection.queued_count, 0);
        assert_eq!(session.state().builder_queue_projection.inflight_count, 0);
        assert_eq!(session.state().builder_queue_projection.removed_count, 1);
        assert_eq!(
            session
                .state()
                .builder_queue_projection
                .orphan_authoritative_count,
            1
        );
        assert!(
            session
                .state()
                .builder_queue_projection
                .last_orphan_authoritative
        );
        assert!(!session.snapshot_input_mut().building);
    }

    #[test]
    fn building_control_select_packet_emits_observability_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "buildingControlSelect")
            .unwrap()
            .packet_id;
        let build_pos = Some(pack_point2(12, 34));
        let packet = encode_packet(
            packet_id,
            &encode_player_prefixed_payload(42, &encode_building_payload(build_pos)),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::BuildingControlSelect { build_pos }
        );
        assert_eq!(session.state().received_building_control_select_count, 1);
        assert_eq!(
            session.state().last_building_control_select_build_pos,
            build_pos
        );
    }

    #[test]
    fn unit_clear_and_unit_control_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let unit_clear_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitClear")
            .unwrap()
            .packet_id;
        let unit_control_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitControl")
            .unwrap()
            .packet_id;

        let clear_event = session
            .ingest_packet_bytes(&encode_packet(unit_clear_packet_id, &[], false).unwrap())
            .unwrap();
        assert_eq!(clear_event, ClientSessionEvent::UnitClear);
        assert_eq!(session.state().received_unit_clear_count, 1);

        let target = Some(UnitRefProjection {
            kind: 2,
            value: 222,
        });
        let control_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    unit_control_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &encode_unit_payload(ClientUnitRef::Standard(222)),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(control_event, ClientSessionEvent::UnitControl { target });
        assert_eq!(session.state().received_unit_control_count, 1);
        assert_eq!(session.state().last_unit_control_target, target);
    }

    #[test]
    fn unit_building_control_select_packet_emits_observability_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitBuildingControlSelect")
            .unwrap()
            .packet_id;
        let target = Some(UnitRefProjection {
            kind: 2,
            value: 222,
        });
        let build_pos = Some(pack_point2(12, 34));
        let packet = encode_packet(
            packet_id,
            &encode_unit_building_payload(ClientUnitRef::Standard(222), build_pos),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::UnitBuildingControlSelect { target, build_pos }
        );
        assert_eq!(
            session.state().received_unit_building_control_select_count,
            1
        );
        assert_eq!(
            session.state().last_unit_building_control_select_target,
            target
        );
        assert_eq!(
            session.state().last_unit_building_control_select_build_pos,
            build_pos
        );
    }

    #[test]
    fn command_building_packet_emits_observability_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandBuilding")
            .unwrap()
            .packet_id;
        let buildings = vec![pack_point2(5, 6), pack_point2(-7, 8)];
        let packet = encode_packet(
            packet_id,
            &encode_command_building_payload(&buildings, 12.5, -4.0),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::CommandBuilding {
                buildings: buildings.clone(),
                x: 12.5,
                y: -4.0,
            }
        );
        assert_eq!(session.state().received_command_building_count, 1);
        assert_eq!(session.state().last_command_building_count, 2);
        assert_eq!(
            session.state().last_command_building_first_build_pos,
            buildings.first().copied()
        );
        assert_eq!(
            session.state().last_command_building_x_bits,
            Some(12.5f32.to_bits())
        );
        assert_eq!(
            session.state().last_command_building_y_bits,
            Some((-4.0f32).to_bits())
        );
    }

    #[test]
    fn command_units_packet_emits_observability_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandUnits")
            .unwrap()
            .packet_id;
        let unit_ids = vec![333, 444];
        let build_target = Some(pack_point2(9, 10));
        let unit_target = Some(UnitRefProjection {
            kind: 1,
            value: pack_point2(7, 8),
        });
        let packet = encode_packet(
            packet_id,
            &encode_command_units_payload(
                &unit_ids,
                build_target,
                ClientUnitRef::Block(pack_point2(7, 8)),
                Some((12.5, -4.0)),
                true,
                false,
            ),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::CommandUnits {
                unit_ids: unit_ids.clone(),
                build_target,
                unit_target,
                x: 12.5,
                y: -4.0,
                queue_command: true,
                final_batch: false,
            }
        );
        assert_eq!(session.state().received_command_units_count, 1);
        assert_eq!(session.state().last_command_units_count, 2);
        assert_eq!(session.state().last_command_units_first_unit_id, Some(333));
        assert_eq!(
            session.state().last_command_units_build_target,
            build_target
        );
        assert_eq!(session.state().last_command_units_unit_target, unit_target);
        assert_eq!(
            session.state().last_command_units_x_bits,
            Some(12.5f32.to_bits())
        );
        assert_eq!(
            session.state().last_command_units_y_bits,
            Some((-4.0f32).to_bits())
        );
        assert_eq!(session.state().last_command_units_queue, Some(true));
        assert_eq!(session.state().last_command_units_final_batch, Some(false));
    }

    #[test]
    fn set_unit_command_and_stance_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_unit_command_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setUnitCommand")
            .unwrap()
            .packet_id;
        let set_unit_stance_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setUnitStance")
            .unwrap()
            .packet_id;
        let unit_ids = vec![555, 666];

        let command_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    set_unit_command_packet_id,
                    &encode_set_unit_command_payload(&unit_ids, Some(12)),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            command_event,
            ClientSessionEvent::SetUnitCommand {
                unit_ids: unit_ids.clone(),
                command_id: Some(12),
            }
        );
        assert_eq!(session.state().received_set_unit_command_count, 1);
        assert_eq!(session.state().last_set_unit_command_count, 2);
        assert_eq!(
            session.state().last_set_unit_command_first_unit_id,
            Some(555)
        );
        assert_eq!(session.state().last_set_unit_command_id, Some(12));

        let stance_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    set_unit_stance_packet_id,
                    &encode_set_unit_stance_payload(&unit_ids, Some(7), false),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            stance_event,
            ClientSessionEvent::SetUnitStance {
                unit_ids,
                stance_id: Some(7),
                enable: false,
            }
        );
        assert_eq!(session.state().received_set_unit_stance_count, 1);
        assert_eq!(session.state().last_set_unit_stance_count, 2);
        assert_eq!(
            session.state().last_set_unit_stance_first_unit_id,
            Some(555)
        );
        assert_eq!(session.state().last_set_unit_stance_id, Some(7));
        assert_eq!(session.state().last_set_unit_stance_enable, Some(false));
    }

    #[test]
    fn request_and_inventory_control_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let request_item_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestItem")
            .unwrap()
            .packet_id;
        let request_build_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestBuildPayload")
            .unwrap()
            .packet_id;
        let request_unit_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestUnitPayload")
            .unwrap()
            .packet_id;
        let transfer_inventory_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferInventory")
            .unwrap()
            .packet_id;
        let rotate_block_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "rotateBlock")
            .unwrap()
            .packet_id;
        let drop_item_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "dropItem")
            .unwrap()
            .packet_id;
        let delete_plans_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "deletePlans")
            .unwrap()
            .packet_id;

        let request_item_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    request_item_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &encode_request_item_payload(Some(pack_point2(9, 1)), Some(7), 15),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            request_item_event,
            ClientSessionEvent::RequestItem {
                build_pos: Some(pack_point2(9, 1)),
                item_id: Some(7),
                amount: 15,
            }
        );
        assert_eq!(session.state().received_request_item_count, 1);
        assert_eq!(
            session.state().last_request_item_build_pos,
            Some(pack_point2(9, 1))
        );
        assert_eq!(session.state().last_request_item_item_id, Some(7));
        assert_eq!(session.state().last_request_item_amount, Some(15));

        let request_build_pos = Some(pack_point2(3, 4));
        let request_build_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    request_build_payload_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &encode_building_payload(request_build_pos),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            request_build_event,
            ClientSessionEvent::RequestBuildPayload {
                build_pos: request_build_pos,
            }
        );
        assert_eq!(session.state().received_request_build_payload_count, 1);
        assert_eq!(
            session.state().last_request_build_payload_build_pos,
            request_build_pos
        );

        let request_unit_target = Some(UnitRefProjection {
            kind: 1,
            value: pack_point2(5, 6),
        });
        let request_unit_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    request_unit_payload_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &encode_unit_payload(ClientUnitRef::Block(pack_point2(5, 6))),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            request_unit_event,
            ClientSessionEvent::RequestUnitPayload {
                target: request_unit_target,
            }
        );
        assert_eq!(session.state().received_request_unit_payload_count, 1);
        assert_eq!(
            session.state().last_request_unit_payload_target,
            request_unit_target
        );

        let transfer_inventory_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    transfer_inventory_packet_id,
                    &encode_player_prefixed_payload(42, &encode_building_payload(None)),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            transfer_inventory_event,
            ClientSessionEvent::TransferInventory { build_pos: None }
        );
        assert_eq!(session.state().received_transfer_inventory_count, 1);
        assert_eq!(session.state().last_transfer_inventory_build_pos, None);

        let rotate_build_pos = Some(pack_point2(7, 8));
        let rotate_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    rotate_block_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &encode_building_bool_payload(rotate_build_pos, true),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            rotate_event,
            ClientSessionEvent::RotateBlock {
                build_pos: rotate_build_pos,
                direction: true,
            }
        );
        assert_eq!(session.state().received_rotate_block_count, 1);
        assert_eq!(
            session.state().last_rotate_block_build_pos,
            rotate_build_pos
        );
        assert_eq!(session.state().last_rotate_block_direction, Some(true));

        let drop_angle = 135.0f32;
        let drop_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    drop_item_packet_id,
                    &encode_single_f32_payload(drop_angle),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            drop_event,
            ClientSessionEvent::DropItem { angle: drop_angle }
        );
        assert_eq!(session.state().received_drop_item_count, 1);
        assert_eq!(
            session.state().last_drop_item_angle_bits,
            Some(drop_angle.to_bits())
        );

        let delete_positions = vec![pack_point2(1, 2), pack_point2(-3, 4)];
        let delete_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    delete_plans_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &encode_delete_plans_payload(&delete_positions),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            delete_event,
            ClientSessionEvent::DeletePlans {
                positions: delete_positions.clone(),
            }
        );
        assert_eq!(session.state().received_delete_plans_count, 1);
        assert_eq!(session.state().last_delete_plans_count, 2);
        assert_eq!(
            session.state().last_delete_plans_first_pos,
            delete_positions.first().copied()
        );
    }

    #[test]
    fn team_menu_and_text_input_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_player_team_editor_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setPlayerTeamEditor")
            .unwrap()
            .packet_id;
        let menu_choose_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "menuChoose")
            .unwrap()
            .packet_id;
        let text_input_result_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInputResult")
            .unwrap()
            .packet_id;

        let team_editor_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    set_player_team_editor_packet_id,
                    &encode_player_prefixed_payload(42, &[7]),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            team_editor_event,
            ClientSessionEvent::SetPlayerTeamEditor { team_id: 7 }
        );
        assert_eq!(session.state().received_set_player_team_editor_count, 1);
        assert_eq!(session.state().last_set_player_team_editor_team_id, Some(7));

        let menu_choose_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    menu_choose_packet_id,
                    &encode_player_prefixed_payload(
                        42,
                        &[12i32.to_be_bytes(), (-1i32).to_be_bytes()].concat(),
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            menu_choose_event,
            ClientSessionEvent::MenuChoose {
                menu_id: 12,
                option: -1,
            }
        );
        assert_eq!(session.state().received_menu_choose_count, 1);
        assert_eq!(session.state().last_menu_choose_menu_id, Some(12));
        assert_eq!(session.state().last_menu_choose_option, Some(-1));

        let mut text_input_payload = 9i32.to_be_bytes().to_vec();
        text_input_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(Some("router")));
        let text_input_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    text_input_result_packet_id,
                    &encode_player_prefixed_payload(42, &text_input_payload),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            text_input_event,
            ClientSessionEvent::TextInputResult {
                text_input_id: 9,
                text: Some("router".to_string()),
            }
        );
        assert_eq!(session.state().received_text_input_result_count, 1);
        assert_eq!(session.state().last_text_input_result_id, Some(9));
        assert_eq!(
            session.state().last_text_input_result_text.as_deref(),
            Some("router")
        );
    }

    #[test]
    fn clipboard_uri_and_text_input_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let copy_to_clipboard_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "copyToClipboard")
            .unwrap()
            .packet_id;
        let open_uri_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "openURI")
            .unwrap()
            .packet_id;
        let text_input_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInput" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let text_input_allow_empty_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInput" && entry.params.len() == 7)
            .unwrap()
            .packet_id;

        let copy_to_clipboard_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    copy_to_clipboard_packet_id,
                    &encode_optional_typeio_string_payload(Some("copied")),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            copy_to_clipboard_event,
            ClientSessionEvent::CopyToClipboard {
                text: Some("copied".to_string()),
            }
        );
        assert_eq!(session.state().received_copy_to_clipboard_count, 1);
        assert_eq!(
            session.state().last_copy_to_clipboard_text.as_deref(),
            Some("copied")
        );

        let open_uri_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    open_uri_packet_id,
                    &encode_optional_typeio_string_payload(Some("https://example.com")),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            open_uri_event,
            ClientSessionEvent::OpenUri {
                uri: Some("https://example.com".to_string()),
            }
        );
        assert_eq!(session.state().received_open_uri_count, 1);
        assert_eq!(
            session.state().last_open_uri.as_deref(),
            Some("https://example.com")
        );

        let mut text_input_payload = 9i32.to_be_bytes().to_vec();
        text_input_payload.extend_from_slice(&encode_optional_typeio_string_payload(Some("Title")));
        text_input_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(Some("Message")));
        text_input_payload.extend_from_slice(&64i32.to_be_bytes());
        text_input_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(Some("router")));
        text_input_payload.push(1);
        let text_input_event = session
            .ingest_packet_bytes(
                &encode_packet(text_input_packet_id, &text_input_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            text_input_event,
            ClientSessionEvent::TextInput {
                text_input_id: 9,
                title: Some("Title".to_string()),
                message: Some("Message".to_string()),
                text_length: 64,
                default_text: Some("router".to_string()),
                numeric: true,
                allow_empty: false,
            }
        );
        assert_eq!(session.state().received_text_input_count, 1);
        assert_eq!(session.state().last_text_input_id, Some(9));
        assert_eq!(
            session.state().last_text_input_title.as_deref(),
            Some("Title")
        );
        assert_eq!(
            session.state().last_text_input_message.as_deref(),
            Some("Message")
        );
        assert_eq!(session.state().last_text_input_length, Some(64));
        assert_eq!(
            session.state().last_text_input_default_text.as_deref(),
            Some("router")
        );
        assert_eq!(session.state().last_text_input_numeric, Some(true));
        assert_eq!(session.state().last_text_input_allow_empty, Some(false));

        let mut text_input_allow_empty_payload = 10i32.to_be_bytes().to_vec();
        text_input_allow_empty_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(Some("Digits")));
        text_input_allow_empty_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(None));
        text_input_allow_empty_payload.extend_from_slice(&16i32.to_be_bytes());
        text_input_allow_empty_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(Some("123")));
        text_input_allow_empty_payload.push(1);
        text_input_allow_empty_payload.push(1);
        let text_input_allow_empty_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    text_input_allow_empty_packet_id,
                    &text_input_allow_empty_payload,
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            text_input_allow_empty_event,
            ClientSessionEvent::TextInput {
                text_input_id: 10,
                title: Some("Digits".to_string()),
                message: None,
                text_length: 16,
                default_text: Some("123".to_string()),
                numeric: true,
                allow_empty: true,
            }
        );
        assert_eq!(session.state().received_text_input_count, 2);
        assert_eq!(session.state().last_text_input_id, Some(10));
        assert_eq!(
            session.state().last_text_input_title.as_deref(),
            Some("Digits")
        );
        assert_eq!(session.state().last_text_input_message, None);
        assert_eq!(session.state().last_text_input_length, Some(16));
        assert_eq!(
            session.state().last_text_input_default_text.as_deref(),
            Some("123")
        );
        assert_eq!(session.state().last_text_input_numeric, Some(true));
        assert_eq!(session.state().last_text_input_allow_empty, Some(true));
    }

    #[test]
    fn game_state_and_research_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "en_US").unwrap();
        let set_flag_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setFlag")
            .unwrap()
            .packet_id;
        let game_over_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "gameOver")
            .unwrap()
            .packet_id;
        let update_game_over_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateGameOver")
            .unwrap()
            .packet_id;
        let sector_capture_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sectorCapture")
            .unwrap()
            .packet_id;
        let researched_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "researched")
            .unwrap()
            .packet_id;

        let mut set_flag_payload = encode_typeio_string_payload("wave-start");
        set_flag_payload.push(1);
        let set_flag_event = session
            .ingest_packet_bytes(
                &encode_packet(set_flag_packet_id, &set_flag_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_flag_event,
            ClientSessionEvent::SetFlag {
                flag: Some("wave-start".to_string()),
                add: true,
            }
        );
        assert_eq!(session.state().received_set_flag_count, 1);
        assert_eq!(session.state().last_set_flag.as_deref(), Some("wave-start"));
        assert_eq!(session.state().last_set_flag_add, Some(true));

        let game_over_event = session
            .ingest_packet_bytes(
                &encode_packet(game_over_packet_id, &encode_team_payload(3), false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            game_over_event,
            ClientSessionEvent::GameOver { winner_team_id: 3 }
        );
        assert_eq!(session.state().received_game_over_count, 1);
        assert_eq!(session.state().last_game_over_winner_team_id, Some(3));

        let update_game_over_event = session
            .ingest_packet_bytes(
                &encode_packet(update_game_over_packet_id, &encode_team_payload(5), false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            update_game_over_event,
            ClientSessionEvent::UpdateGameOver { winner_team_id: 5 }
        );
        assert_eq!(session.state().received_update_game_over_count, 1);
        assert_eq!(
            session.state().last_update_game_over_winner_team_id,
            Some(5)
        );

        let sector_capture_event = session
            .ingest_packet_bytes(&encode_packet(sector_capture_packet_id, &[], false).unwrap())
            .unwrap();
        assert_eq!(sector_capture_event, ClientSessionEvent::SectorCapture);
        assert_eq!(session.state().received_sector_capture_count, 1);

        let researched_event = session
            .ingest_packet_bytes(
                &encode_packet(researched_packet_id, &encode_content_payload(2, 123), false)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            researched_event,
            ClientSessionEvent::Researched {
                content_type: 2,
                content_id: 123,
            }
        );
        assert_eq!(session.state().received_researched_count, 1);
        assert_eq!(session.state().last_researched_content_type, Some(2));
        assert_eq!(session.state().last_researched_content_id, Some(123));
    }

    #[test]
    fn info_popup_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let info_popup_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopup" && entry.params.len() == 7)
            .unwrap()
            .packet_id;
        let info_popup_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopup" && entry.params.len() == 8)
            .unwrap()
            .packet_id;
        let info_popup_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopupReliable" && entry.params.len() == 7)
            .unwrap()
            .packet_id;
        let info_popup_reliable_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoPopupReliable" && entry.params.len() == 8)
            .unwrap()
            .packet_id;

        let info_popup_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    info_popup_packet_id,
                    &encode_info_popup_payload(Some("popup-u"), None, 1.5, 2, 3, 4, 5, 6),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            info_popup_event,
            ClientSessionEvent::InfoPopup {
                reliable: false,
                popup_id: None,
                message: Some("popup-u".to_string()),
                duration: 1.5,
                align: 2,
                top: 3,
                left: 4,
                bottom: 5,
                right: 6,
            }
        );

        let info_popup_with_id_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    info_popup_with_id_packet_id,
                    &encode_info_popup_payload(
                        Some("popup-id-u"),
                        Some("id-u"),
                        2.0,
                        7,
                        8,
                        9,
                        10,
                        11,
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            info_popup_with_id_event,
            ClientSessionEvent::InfoPopup {
                reliable: false,
                popup_id: Some("id-u".to_string()),
                message: Some("popup-id-u".to_string()),
                duration: 2.0,
                align: 7,
                top: 8,
                left: 9,
                bottom: 10,
                right: 11,
            }
        );

        let info_popup_reliable_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    info_popup_reliable_packet_id,
                    &encode_info_popup_payload(Some("popup-r"), None, 2.5, 12, 13, 14, 15, 16),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            info_popup_reliable_event,
            ClientSessionEvent::InfoPopup {
                reliable: true,
                popup_id: None,
                message: Some("popup-r".to_string()),
                duration: 2.5,
                align: 12,
                top: 13,
                left: 14,
                bottom: 15,
                right: 16,
            }
        );

        let info_popup_reliable_with_id_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    info_popup_reliable_with_id_packet_id,
                    &encode_info_popup_payload(
                        Some("popup-id-r"),
                        Some("id-r"),
                        3.0,
                        17,
                        18,
                        19,
                        20,
                        21,
                    ),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            info_popup_reliable_with_id_event,
            ClientSessionEvent::InfoPopup {
                reliable: true,
                popup_id: Some("id-r".to_string()),
                message: Some("popup-id-r".to_string()),
                duration: 3.0,
                align: 17,
                top: 18,
                left: 19,
                bottom: 20,
                right: 21,
            }
        );

        assert_eq!(session.state().received_info_popup_count, 2);
        assert_eq!(session.state().received_info_popup_reliable_count, 2);
        assert_eq!(session.state().last_info_popup_reliable, Some(true));
        assert_eq!(session.state().last_info_popup_id.as_deref(), Some("id-r"));
        assert_eq!(
            session.state().last_info_popup_message.as_deref(),
            Some("popup-id-r")
        );
        assert_eq!(
            session.state().last_info_popup_duration_bits,
            Some(3.0f32.to_bits())
        );
        assert_eq!(session.state().last_info_popup_align, Some(17));
        assert_eq!(session.state().last_info_popup_top, Some(18));
        assert_eq!(session.state().last_info_popup_left, Some(19));
        assert_eq!(session.state().last_info_popup_bottom, Some(20));
        assert_eq!(session.state().last_info_popup_right, Some(21));
    }

    #[test]
    fn hud_and_ui_notice_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_hud_text_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setHudText")
            .unwrap()
            .packet_id;
        let set_hud_text_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setHudTextReliable")
            .unwrap()
            .packet_id;
        let hide_hud_text_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "hideHudText")
            .unwrap()
            .packet_id;
        let announce_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "announce")
            .unwrap()
            .packet_id;
        let info_message_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoMessage")
            .unwrap()
            .packet_id;
        let info_toast_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "infoToast")
            .unwrap()
            .packet_id;
        let warning_toast_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "warningToast")
            .unwrap()
            .packet_id;

        let set_hud_text_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    set_hud_text_packet_id,
                    &encode_optional_typeio_string_payload(Some("hud-u")),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_hud_text_event,
            ClientSessionEvent::SetHudText {
                message: Some("hud-u".to_string()),
            }
        );
        assert_eq!(session.state().received_set_hud_text_count, 1);
        assert_eq!(
            session.state().last_set_hud_text_message.as_deref(),
            Some("hud-u")
        );

        let set_hud_text_reliable_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    set_hud_text_reliable_packet_id,
                    &encode_optional_typeio_string_payload(Some("hud-r")),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_hud_text_reliable_event,
            ClientSessionEvent::SetHudTextReliable {
                message: Some("hud-r".to_string()),
            }
        );
        assert_eq!(session.state().received_set_hud_text_reliable_count, 1);
        assert_eq!(
            session
                .state()
                .last_set_hud_text_reliable_message
                .as_deref(),
            Some("hud-r")
        );

        let hide_hud_text_event = session
            .ingest_packet_bytes(&encode_packet(hide_hud_text_packet_id, &[], false).unwrap())
            .unwrap();
        assert_eq!(hide_hud_text_event, ClientSessionEvent::HideHudText);
        assert_eq!(session.state().received_hide_hud_text_count, 1);

        let announce_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    announce_packet_id,
                    &encode_optional_typeio_string_payload(Some("incoming")),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            announce_event,
            ClientSessionEvent::Announce {
                message: Some("incoming".to_string()),
            }
        );
        assert_eq!(session.state().received_announce_count, 1);
        assert_eq!(
            session.state().last_announce_message.as_deref(),
            Some("incoming")
        );

        let info_message_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    info_message_packet_id,
                    &encode_optional_typeio_string_payload(Some("alert")),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            info_message_event,
            ClientSessionEvent::InfoMessage {
                message: Some("alert".to_string()),
            }
        );
        assert_eq!(session.state().received_info_message_count, 1);
        assert_eq!(session.state().last_info_message.as_deref(), Some("alert"));

        let mut info_toast_payload = encode_optional_typeio_string_payload(Some("toast"));
        info_toast_payload.extend_from_slice(&1.5f32.to_be_bytes());
        let info_toast_event = session
            .ingest_packet_bytes(
                &encode_packet(info_toast_packet_id, &info_toast_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            info_toast_event,
            ClientSessionEvent::InfoToast {
                message: Some("toast".to_string()),
                duration: 1.5,
            }
        );
        assert_eq!(session.state().received_info_toast_count, 1);
        assert_eq!(
            session.state().last_info_toast_message.as_deref(),
            Some("toast")
        );
        assert_eq!(
            session.state().last_info_toast_duration_bits,
            Some(1.5f32.to_bits())
        );

        let mut warning_toast_payload = 0xe813i32.to_be_bytes().to_vec();
        warning_toast_payload
            .extend_from_slice(&encode_optional_typeio_string_payload(Some("warn")));
        let warning_toast_event = session
            .ingest_packet_bytes(
                &encode_packet(warning_toast_packet_id, &warning_toast_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            warning_toast_event,
            ClientSessionEvent::WarningToast {
                unicode: 0xe813,
                text: Some("warn".to_string()),
            }
        );
        assert_eq!(session.state().received_warning_toast_count, 1);
        assert_eq!(session.state().last_warning_toast_unicode, Some(0xe813));
        assert_eq!(
            session.state().last_warning_toast_text.as_deref(),
            Some("warn")
        );
    }

    #[test]
    fn menu_lifecycle_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let menu_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "menu")
            .unwrap()
            .packet_id;
        let follow_up_menu_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "followUpMenu")
            .unwrap()
            .packet_id;
        let hide_follow_up_menu_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "hideFollowUpMenu")
            .unwrap()
            .packet_id;

        let menu_options: [&[&str]; 2] = [&["alpha", "beta"], &["gamma"]];
        let mut menu_payload = 12i32.to_be_bytes().to_vec();
        menu_payload.extend_from_slice(&encode_optional_typeio_string_payload(Some("title")));
        menu_payload.extend_from_slice(&encode_optional_typeio_string_payload(Some("body")));
        menu_payload.extend_from_slice(&encode_typeio_string_matrix_payload(&menu_options));
        let menu_event = session
            .ingest_packet_bytes(&encode_packet(menu_packet_id, &menu_payload, false).unwrap())
            .unwrap();
        assert_eq!(
            menu_event,
            ClientSessionEvent::MenuShown {
                menu_id: 12,
                title: Some("title".to_string()),
                message: Some("body".to_string()),
                option_rows: 2,
                first_row_len: 2,
            }
        );
        assert_eq!(session.state().received_menu_open_count, 1);
        assert_eq!(session.state().last_menu_open_id, Some(12));
        assert_eq!(
            session.state().last_menu_open_title.as_deref(),
            Some("title")
        );
        assert_eq!(
            session.state().last_menu_open_message.as_deref(),
            Some("body")
        );
        assert_eq!(session.state().last_menu_open_option_rows, 2);
        assert_eq!(session.state().last_menu_open_first_row_len, 2);

        let follow_up_options: [&[&str]; 1] = [&["delta"]];
        let mut follow_up_payload = 21i32.to_be_bytes().to_vec();
        follow_up_payload.extend_from_slice(&encode_optional_typeio_string_payload(Some("next")));
        follow_up_payload.extend_from_slice(&encode_optional_typeio_string_payload(Some("step")));
        follow_up_payload
            .extend_from_slice(&encode_typeio_string_matrix_payload(&follow_up_options));
        let follow_up_event = session
            .ingest_packet_bytes(
                &encode_packet(follow_up_menu_packet_id, &follow_up_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            follow_up_event,
            ClientSessionEvent::FollowUpMenuShown {
                menu_id: 21,
                title: Some("next".to_string()),
                message: Some("step".to_string()),
                option_rows: 1,
                first_row_len: 1,
            }
        );
        assert_eq!(session.state().received_follow_up_menu_open_count, 1);
        assert_eq!(session.state().last_follow_up_menu_open_id, Some(21));
        assert_eq!(
            session.state().last_follow_up_menu_open_title.as_deref(),
            Some("next")
        );
        assert_eq!(
            session.state().last_follow_up_menu_open_message.as_deref(),
            Some("step")
        );
        assert_eq!(session.state().last_follow_up_menu_open_option_rows, 1);
        assert_eq!(session.state().last_follow_up_menu_open_first_row_len, 1);

        let hide_event = session
            .ingest_packet_bytes(
                &encode_packet(hide_follow_up_menu_packet_id, &21i32.to_be_bytes(), false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            hide_event,
            ClientSessionEvent::HideFollowUpMenu { menu_id: 21 }
        );
        assert_eq!(session.state().received_hide_follow_up_menu_count, 1);
        assert_eq!(session.state().last_hide_follow_up_menu_id, Some(21));
    }

    #[test]
    fn item_and_liquid_mirror_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_item_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setItem")
            .unwrap()
            .packet_id;
        let set_liquid_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setLiquid")
            .unwrap()
            .packet_id;

        let mut set_item_payload = encode_building_payload(Some(pack_point2(2, 3)));
        set_item_payload.extend_from_slice(&7i16.to_be_bytes());
        set_item_payload.extend_from_slice(&25i32.to_be_bytes());
        let set_item_event = session
            .ingest_packet_bytes(
                &encode_packet(set_item_packet_id, &set_item_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_item_event,
            ClientSessionEvent::SetItem {
                build_pos: Some(pack_point2(2, 3)),
                item_id: Some(7),
                amount: 25,
            }
        );
        assert_eq!(session.state().received_set_item_count, 1);
        assert_eq!(
            session.state().last_set_item_build_pos,
            Some(pack_point2(2, 3))
        );
        assert_eq!(session.state().last_set_item_item_id, Some(7));
        assert_eq!(session.state().last_set_item_amount, Some(25));

        let mut set_liquid_payload = encode_building_payload(Some(pack_point2(4, 5)));
        set_liquid_payload.extend_from_slice(&3i16.to_be_bytes());
        set_liquid_payload.extend_from_slice(&2.5f32.to_be_bytes());
        let set_liquid_event = session
            .ingest_packet_bytes(
                &encode_packet(set_liquid_packet_id, &set_liquid_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_liquid_event,
            ClientSessionEvent::SetLiquid {
                build_pos: Some(pack_point2(4, 5)),
                liquid_id: Some(3),
                amount: 2.5,
            }
        );
        assert_eq!(session.state().received_set_liquid_count, 1);
        assert_eq!(
            session.state().last_set_liquid_build_pos,
            Some(pack_point2(4, 5))
        );
        assert_eq!(session.state().last_set_liquid_liquid_id, Some(3));
        assert_eq!(
            session.state().last_set_liquid_amount_bits,
            Some(2.5f32.to_bits())
        );
    }

    #[test]
    fn items_and_liquids_mirror_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_items_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setItems")
            .unwrap()
            .packet_id;
        let set_liquids_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setLiquids")
            .unwrap()
            .packet_id;

        let mut set_items_payload = encode_building_payload(Some(pack_point2(6, 7)));
        set_items_payload.extend_from_slice(&2i16.to_be_bytes());
        set_items_payload.extend_from_slice(&9i16.to_be_bytes());
        set_items_payload.extend_from_slice(&11i32.to_be_bytes());
        set_items_payload.extend_from_slice(&4i16.to_be_bytes());
        set_items_payload.extend_from_slice(&3i32.to_be_bytes());
        let set_items_event = session
            .ingest_packet_bytes(
                &encode_packet(set_items_packet_id, &set_items_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_items_event,
            ClientSessionEvent::SetItems {
                build_pos: Some(pack_point2(6, 7)),
                stack_count: 2,
                first_item_id: Some(9),
                first_amount: Some(11),
            }
        );
        assert_eq!(session.state().received_set_items_count, 1);
        assert_eq!(
            session.state().last_set_items_build_pos,
            Some(pack_point2(6, 7))
        );
        assert_eq!(session.state().last_set_items_count, 2);
        assert_eq!(session.state().last_set_items_first_item_id, Some(9));
        assert_eq!(session.state().last_set_items_first_amount, Some(11));

        let mut set_liquids_payload = encode_building_payload(Some(pack_point2(8, 9)));
        set_liquids_payload.extend_from_slice(&2i16.to_be_bytes());
        set_liquids_payload.extend_from_slice(&5i16.to_be_bytes());
        set_liquids_payload.extend_from_slice(&1.25f32.to_be_bytes());
        set_liquids_payload.extend_from_slice(&7i16.to_be_bytes());
        set_liquids_payload.extend_from_slice(&3.5f32.to_be_bytes());
        let set_liquids_event = session
            .ingest_packet_bytes(
                &encode_packet(set_liquids_packet_id, &set_liquids_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_liquids_event,
            ClientSessionEvent::SetLiquids {
                build_pos: Some(pack_point2(8, 9)),
                stack_count: 2,
                first_liquid_id: Some(5),
                first_amount_bits: Some(1.25f32.to_bits()),
            }
        );
        assert_eq!(session.state().received_set_liquids_count, 1);
        assert_eq!(
            session.state().last_set_liquids_build_pos,
            Some(pack_point2(8, 9))
        );
        assert_eq!(session.state().last_set_liquids_count, 2);
        assert_eq!(session.state().last_set_liquids_first_liquid_id, Some(5));
        assert_eq!(
            session.state().last_set_liquids_first_amount_bits,
            Some(1.25f32.to_bits())
        );
    }

    #[test]
    fn tile_item_and_liquid_mirror_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let set_tile_items_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setTileItems")
            .unwrap()
            .packet_id;
        let set_tile_liquids_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setTileLiquids")
            .unwrap()
            .packet_id;

        let mut set_tile_items_payload = Vec::new();
        set_tile_items_payload.extend_from_slice(&6i16.to_be_bytes());
        set_tile_items_payload.extend_from_slice(&13i32.to_be_bytes());
        set_tile_items_payload.extend_from_slice(&2i16.to_be_bytes());
        set_tile_items_payload.extend_from_slice(&pack_point2(1, 2).to_be_bytes());
        set_tile_items_payload.extend_from_slice(&pack_point2(3, 4).to_be_bytes());
        let set_tile_items_event = session
            .ingest_packet_bytes(
                &encode_packet(set_tile_items_packet_id, &set_tile_items_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_tile_items_event,
            ClientSessionEvent::SetTileItems {
                item_id: Some(6),
                amount: 13,
                position_count: 2,
                first_position: Some(pack_point2(1, 2)),
            }
        );
        assert_eq!(session.state().received_set_tile_items_count, 1);
        assert_eq!(session.state().last_set_tile_items_item_id, Some(6));
        assert_eq!(session.state().last_set_tile_items_amount, Some(13));
        assert_eq!(session.state().last_set_tile_items_count, 2);
        assert_eq!(
            session.state().last_set_tile_items_first_position,
            Some(pack_point2(1, 2))
        );

        let mut set_tile_liquids_payload = Vec::new();
        set_tile_liquids_payload.extend_from_slice(&4i16.to_be_bytes());
        set_tile_liquids_payload.extend_from_slice(&0.75f32.to_be_bytes());
        set_tile_liquids_payload.extend_from_slice(&2i16.to_be_bytes());
        set_tile_liquids_payload.extend_from_slice(&pack_point2(5, 6).to_be_bytes());
        set_tile_liquids_payload.extend_from_slice(&pack_point2(7, 8).to_be_bytes());
        let set_tile_liquids_event = session
            .ingest_packet_bytes(
                &encode_packet(set_tile_liquids_packet_id, &set_tile_liquids_payload, false)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            set_tile_liquids_event,
            ClientSessionEvent::SetTileLiquids {
                liquid_id: Some(4),
                amount_bits: 0.75f32.to_bits(),
                position_count: 2,
                first_position: Some(pack_point2(5, 6)),
            }
        );
        assert_eq!(session.state().received_set_tile_liquids_count, 1);
        assert_eq!(session.state().last_set_tile_liquids_liquid_id, Some(4));
        assert_eq!(
            session.state().last_set_tile_liquids_amount_bits,
            Some(0.75f32.to_bits())
        );
        assert_eq!(session.state().last_set_tile_liquids_count, 2);
        assert_eq!(
            session.state().last_set_tile_liquids_first_position,
            Some(pack_point2(5, 6))
        );
    }

    #[test]
    fn world_label_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let label_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "label" && entry.params.len() == 4)
            .unwrap()
            .packet_id;
        let label_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "label" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let label_reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "labelReliable" && entry.params.len() == 4)
            .unwrap()
            .packet_id;
        let label_reliable_with_id_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "labelReliable" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let remove_world_label_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeWorldLabel")
            .unwrap()
            .packet_id;

        let mut label_payload = encode_optional_typeio_string_payload(Some("u-label"));
        label_payload.extend_from_slice(&2.0f32.to_be_bytes());
        label_payload.extend_from_slice(&32.5f32.to_be_bytes());
        label_payload.extend_from_slice(&48.0f32.to_be_bytes());
        let label_event = session
            .ingest_packet_bytes(&encode_packet(label_packet_id, &label_payload, false).unwrap())
            .unwrap();
        assert_eq!(
            label_event,
            ClientSessionEvent::WorldLabel {
                reliable: false,
                label_id: None,
                message: Some("u-label".to_string()),
                duration: 2.0,
                world_x: 32.5,
                world_y: 48.0,
            }
        );

        let mut label_with_id_payload = encode_optional_typeio_string_payload(Some("u-id"));
        label_with_id_payload.extend_from_slice(&77i32.to_be_bytes());
        label_with_id_payload.extend_from_slice(&3.0f32.to_be_bytes());
        label_with_id_payload.extend_from_slice(&12.0f32.to_be_bytes());
        label_with_id_payload.extend_from_slice(&24.0f32.to_be_bytes());
        session
            .ingest_packet_bytes(
                &encode_packet(label_with_id_packet_id, &label_with_id_payload, false).unwrap(),
            )
            .unwrap();

        let mut label_reliable_payload = encode_optional_typeio_string_payload(Some("r-label"));
        label_reliable_payload.extend_from_slice(&4.0f32.to_be_bytes());
        label_reliable_payload.extend_from_slice(&16.0f32.to_be_bytes());
        label_reliable_payload.extend_from_slice(&8.0f32.to_be_bytes());
        session
            .ingest_packet_bytes(
                &encode_packet(label_reliable_packet_id, &label_reliable_payload, false).unwrap(),
            )
            .unwrap();

        let mut label_reliable_with_id_payload =
            encode_optional_typeio_string_payload(Some("r-id"));
        label_reliable_with_id_payload.extend_from_slice(&99i32.to_be_bytes());
        label_reliable_with_id_payload.extend_from_slice(&5.0f32.to_be_bytes());
        label_reliable_with_id_payload.extend_from_slice(&6.0f32.to_be_bytes());
        label_reliable_with_id_payload.extend_from_slice(&7.0f32.to_be_bytes());
        let reliable_with_id_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    label_reliable_with_id_packet_id,
                    &label_reliable_with_id_payload,
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            reliable_with_id_event,
            ClientSessionEvent::WorldLabel {
                reliable: true,
                label_id: Some(99),
                message: Some("r-id".to_string()),
                duration: 5.0,
                world_x: 6.0,
                world_y: 7.0,
            }
        );

        let remove_event = session
            .ingest_packet_bytes(
                &encode_packet(remove_world_label_packet_id, &99i32.to_be_bytes(), false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            remove_event,
            ClientSessionEvent::RemoveWorldLabel { label_id: 99 }
        );

        assert_eq!(session.state().received_world_label_count, 2);
        assert_eq!(session.state().received_world_label_reliable_count, 2);
        assert_eq!(session.state().last_world_label_reliable, Some(true));
        assert_eq!(session.state().last_world_label_id, Some(99));
        assert_eq!(
            session.state().last_world_label_message.as_deref(),
            Some("r-id")
        );
        assert_eq!(
            session.state().last_world_label_duration_bits,
            Some(5.0f32.to_bits())
        );
        assert_eq!(
            session.state().last_world_label_world_x_bits,
            Some(6.0f32.to_bits())
        );
        assert_eq!(
            session.state().last_world_label_world_y_bits,
            Some(7.0f32.to_bits())
        );
        assert_eq!(session.state().received_remove_world_label_count, 1);
        assert_eq!(session.state().last_remove_world_label_id, Some(99));
    }

    #[test]
    fn marker_packets_emit_observability_events() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let create_marker_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "createMarker")
            .unwrap()
            .packet_id;
        let remove_marker_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "removeMarker")
            .unwrap()
            .packet_id;
        let update_marker_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateMarker")
            .unwrap()
            .packet_id;
        let update_marker_text_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateMarkerText")
            .unwrap()
            .packet_id;
        let update_marker_texture_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "updateMarkerTexture")
            .unwrap()
            .packet_id;

        let marker_json = r#"{"type":"shape"}"#;
        let mut create_marker_payload = Vec::new();
        write_typeio_int(&mut create_marker_payload, 77);
        write_typeio_int(&mut create_marker_payload, marker_json.len() as i32);
        create_marker_payload.extend_from_slice(marker_json.as_bytes());
        let create_event = session
            .ingest_packet_bytes(
                &encode_packet(create_marker_packet_id, &create_marker_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            create_event,
            ClientSessionEvent::CreateMarker {
                marker_id: 77,
                json_len: marker_json.len(),
            }
        );

        let mut update_marker_payload = Vec::new();
        update_marker_payload.extend_from_slice(&77i32.to_be_bytes());
        update_marker_payload.push(4);
        update_marker_payload.extend_from_slice(&12.5f64.to_bits().to_be_bytes());
        update_marker_payload.extend_from_slice(&(-3.0f64).to_bits().to_be_bytes());
        update_marker_payload.extend_from_slice(&2.25f64.to_bits().to_be_bytes());
        let update_event = session
            .ingest_packet_bytes(
                &encode_packet(update_marker_packet_id, &update_marker_payload, false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            update_event,
            ClientSessionEvent::UpdateMarker {
                marker_id: 77,
                control: 4,
                control_name: Some("pos".to_string()),
                p1_bits: 12.5f64.to_bits(),
                p2_bits: (-3.0f64).to_bits(),
                p3_bits: 2.25f64.to_bits(),
            }
        );

        let mut update_marker_text_payload = Vec::new();
        update_marker_text_payload.extend_from_slice(&77i32.to_be_bytes());
        update_marker_text_payload.push(14);
        update_marker_text_payload.push(1);
        mdt_typeio::write_string(&mut update_marker_text_payload, Some("logic-text"));
        let update_text_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    update_marker_text_packet_id,
                    &update_marker_text_payload,
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            update_text_event,
            ClientSessionEvent::UpdateMarkerText {
                marker_id: 77,
                control: 14,
                control_name: Some("flushText".to_string()),
                fetch: true,
                text: Some("logic-text".to_string()),
            }
        );

        let mut update_marker_texture_payload = Vec::new();
        update_marker_texture_payload.extend_from_slice(&77i32.to_be_bytes());
        write_typeio_object(
            &mut update_marker_texture_payload,
            &TypeIoObject::String(Some("atlas-region".to_string())),
        );
        let update_texture_event = session
            .ingest_packet_bytes(
                &encode_packet(
                    update_marker_texture_packet_id,
                    &update_marker_texture_payload,
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            update_texture_event,
            ClientSessionEvent::UpdateMarkerTexture {
                marker_id: 77,
                texture_kind: 4,
                texture_kind_name: "string".to_string(),
            }
        );

        let remove_event = session
            .ingest_packet_bytes(
                &encode_packet(remove_marker_packet_id, &77i32.to_be_bytes(), false).unwrap(),
            )
            .unwrap();
        assert_eq!(
            remove_event,
            ClientSessionEvent::RemoveMarker { marker_id: 77 }
        );

        assert_eq!(session.state().received_create_marker_count, 1);
        assert_eq!(session.state().received_remove_marker_count, 1);
        assert_eq!(session.state().received_update_marker_count, 1);
        assert_eq!(session.state().received_update_marker_text_count, 1);
        assert_eq!(session.state().received_update_marker_texture_count, 1);
        assert_eq!(session.state().failed_marker_decode_count, 0);
        assert_eq!(session.state().last_marker_id, Some(77));
        assert_eq!(session.state().last_marker_json_len, None);
        assert_eq!(session.state().last_marker_control, None);
        assert_eq!(session.state().last_marker_text, None);
        assert_eq!(session.state().last_marker_texture_kind, None);
        assert_eq!(session.state().last_marker_texture_kind_name, None);
    }

    #[test]
    fn tile_config_packet_emits_event_and_tracks_full_typeio_object() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .unwrap()
            .packet_id;
        let expected_config = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        let payload = encode_tile_config_payload(Some(888), &expected_config);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TileConfig {
                build_pos: Some(888),
                config_kind: Some(22),
                config_kind_name: Some("object[]".to_string()),
                parse_failed: false,
                business_applied: true,
                cleared_pending_local: false,
                was_rollback: false,
                pending_local_match: None,
            }
        );
        assert_eq!(session.state().received_tile_config_count, 1);
        assert_eq!(session.state().last_tile_config_build_pos, Some(888));
        assert_eq!(session.state().last_tile_config_kind, Some(22));
        assert_eq!(
            session.state().last_tile_config_kind_name.as_deref(),
            Some("object[]")
        );
        assert_eq!(
            session.state().last_tile_config_consumed_len,
            Some(payload.len() - 4)
        );
        assert_eq!(
            session.state().last_tile_config_object,
            Some(expected_config.clone())
        );
        assert!(!session.state().last_tile_config_parse_failed);
        assert_eq!(session.state().failed_tile_config_parse_count, 0);
        assert_eq!(session.state().last_tile_config_parse_error, None);
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .applied_authoritative_count,
            1
        );
        assert_eq!(session.state().tile_config_projection.rollback_count, 0);
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .last_business_build_pos,
            Some(888)
        );
        assert_eq!(
            session.state().tile_config_projection.last_business_value,
            Some(expected_config.clone())
        );
        assert!(session.state().tile_config_projection.last_business_applied);
        assert!(
            !session
                .state()
                .tile_config_projection
                .last_cleared_pending_local
        );
        assert!(!session.state().tile_config_projection.last_was_rollback);
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .last_pending_local_match,
            None
        );
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .authoritative_by_build_pos
                .get(&888),
            Some(&expected_config)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&888)
                .and_then(|building| building.config.as_ref()),
            Some(&expected_config)
        );
        assert_eq!(
            session.state().building_table_projection.last_update,
            Some(crate::session_state::BuildingProjectionUpdateKind::TileConfig)
        );
    }

    #[test]
    fn queue_tile_config_records_pending_local_intent_for_authoritative_reconcile() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let value = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
        ]);

        session.queue_tile_config(Some(888), value.clone()).unwrap();

        assert_eq!(session.state().tile_config_projection.queued_local_count, 1);
        assert_eq!(
            session.state().tile_config_projection.last_queued_build_pos,
            Some(888)
        );
        assert_eq!(
            session.state().tile_config_projection.last_queued_value,
            Some(value.clone())
        );
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .pending_local_by_build_pos
                .get(&888),
            Some(&value)
        );
    }

    #[test]
    fn tile_config_packet_matching_pending_local_intent_applies_without_rollback() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .unwrap()
            .packet_id;
        let expected_config = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        session
            .queue_tile_config(Some(888), expected_config.clone())
            .unwrap();

        let payload = encode_tile_config_payload(Some(888), &expected_config);
        let packet = encode_packet(packet_id, &payload, false).unwrap();
        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TileConfig {
                build_pos: Some(888),
                config_kind: Some(22),
                config_kind_name: Some("object[]".to_string()),
                parse_failed: false,
                business_applied: true,
                cleared_pending_local: true,
                was_rollback: false,
                pending_local_match: Some(true),
            }
        );

        assert!(session
            .state()
            .tile_config_projection
            .pending_local_by_build_pos
            .is_empty());
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .authoritative_by_build_pos
                .get(&888),
            Some(&expected_config)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&888)
                .and_then(|building| building.config.as_ref()),
            Some(&expected_config)
        );
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .applied_authoritative_count,
            1
        );
        assert_eq!(session.state().tile_config_projection.rollback_count, 0);
        assert!(
            session
                .state()
                .tile_config_projection
                .last_cleared_pending_local
        );
        assert!(!session.state().tile_config_projection.last_was_rollback);
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .last_pending_local_match,
            Some(true)
        );
    }

    #[test]
    fn tile_config_packet_with_mismatched_pending_intent_marks_rollback() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .unwrap()
            .packet_id;
        let local_value = TypeIoObject::Int(7);
        let authoritative_value = TypeIoObject::Int(9);
        session.queue_tile_config(Some(888), local_value).unwrap();

        let payload = encode_tile_config_payload(Some(888), &authoritative_value);
        let packet = encode_packet(packet_id, &payload, false).unwrap();
        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TileConfig {
                build_pos: Some(888),
                config_kind: Some(1),
                config_kind_name: Some("int".to_string()),
                parse_failed: false,
                business_applied: true,
                cleared_pending_local: true,
                was_rollback: true,
                pending_local_match: Some(false),
            }
        );

        assert!(session
            .state()
            .tile_config_projection
            .pending_local_by_build_pos
            .is_empty());
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .authoritative_by_build_pos
                .get(&888),
            Some(&authoritative_value)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&888)
                .and_then(|building| building.config.as_ref()),
            Some(&authoritative_value)
        );
        assert_eq!(session.state().tile_config_projection.rollback_count, 1);
        assert!(session.state().tile_config_projection.last_was_rollback);
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .last_pending_local_match,
            Some(false)
        );
    }

    #[test]
    fn tile_config_packet_with_unsupported_type_keeps_kind_and_parse_fail_observability() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&444i32.to_be_bytes());
        payload.push(99);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TileConfig {
                build_pos: Some(444),
                config_kind: Some(99),
                config_kind_name: Some("unsupported(99)".to_string()),
                parse_failed: true,
                business_applied: false,
                cleared_pending_local: false,
                was_rollback: false,
                pending_local_match: None,
            }
        );
        assert_eq!(session.state().received_tile_config_count, 1);
        assert_eq!(session.state().last_tile_config_build_pos, Some(444));
        assert_eq!(session.state().last_tile_config_kind, Some(99));
        assert_eq!(
            session.state().last_tile_config_kind_name.as_deref(),
            Some("unsupported(99)")
        );
        assert_eq!(session.state().last_tile_config_consumed_len, None);
        assert_eq!(session.state().last_tile_config_object, None);
        assert!(session.state().last_tile_config_parse_failed);
        assert_eq!(session.state().failed_tile_config_parse_count, 1);
        assert!(session
            .state()
            .last_tile_config_parse_error
            .as_deref()
            .unwrap_or_default()
            .contains("unsupported TypeIO object type id 99"));
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .applied_authoritative_count,
            0
        );
        assert!(session
            .state()
            .tile_config_projection
            .authoritative_by_build_pos
            .is_empty());
        assert!(!session.state().tile_config_projection.last_business_applied);
    }

    #[test]
    fn tile_config_packet_with_trailing_bytes_marks_parse_failed_without_packet_drop() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session
            .queue_tile_config(Some(777), TypeIoObject::Int(7))
            .unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .unwrap()
            .packet_id;
        let mut payload = encode_tile_config_payload(Some(777), &TypeIoObject::Int(7));
        payload.push(0xff);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TileConfig {
                build_pos: Some(777),
                config_kind: Some(1),
                config_kind_name: Some("int".to_string()),
                parse_failed: true,
                business_applied: false,
                cleared_pending_local: false,
                was_rollback: false,
                pending_local_match: None,
            }
        );
        assert_eq!(session.state().received_tile_config_count, 1);
        assert_eq!(session.state().last_tile_config_kind, Some(1));
        assert_eq!(
            session.state().last_tile_config_kind_name.as_deref(),
            Some("int")
        );
        assert_eq!(session.state().last_tile_config_consumed_len, Some(5));
        assert_eq!(
            session.state().last_tile_config_object,
            Some(TypeIoObject::Int(7))
        );
        assert!(session.state().last_tile_config_parse_failed);
        assert_eq!(session.state().failed_tile_config_parse_count, 1);
        assert!(session
            .state()
            .last_tile_config_parse_error
            .as_deref()
            .unwrap_or_default()
            .contains("trailing bytes after TypeIO object"));
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .applied_authoritative_count,
            0
        );
        assert_eq!(session.state().tile_config_projection.rollback_count, 0);
        assert!(!session.state().tile_config_projection.last_business_applied);
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .pending_local_by_build_pos
                .get(&777),
            Some(&TypeIoObject::Int(7))
        );
        assert!(session
            .state()
            .tile_config_projection
            .authoritative_by_build_pos
            .is_empty());
    }

    #[test]
    fn begin_place_packet_emits_summary_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "beginPlace")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(1);
        payload.extend_from_slice(&100i32.to_be_bytes());
        payload.extend_from_slice(&99i32.to_be_bytes());
        payload.extend_from_slice(&3i32.to_be_bytes());
        write_typeio_object(&mut payload, &TypeIoObject::Int(7));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::BeginPlace {
                x: 100,
                y: 99,
                block_id: Some(0x0101),
                rotation: 3,
                team_id: 1,
                config_kind: 1,
                config_kind_name: "int",
                builder_kind: 2,
                builder_value: 42,
            }
        );
        assert_eq!(session.state().received_begin_place_count, 1);
        assert_eq!(session.state().last_begin_place_x, Some(100));
        assert_eq!(session.state().last_begin_place_y, Some(99));
        assert_eq!(session.state().last_begin_place_block_id, Some(0x0101));
        assert_eq!(session.state().last_begin_place_rotation, Some(3));
        assert_eq!(session.state().last_begin_place_team_id, Some(1));
        assert_eq!(session.state().last_begin_place_config_kind, Some(1));
        assert_eq!(
            session.state().last_begin_place_config_kind_name.as_deref(),
            Some("int")
        );
        assert_eq!(
            session.state().last_begin_place_config_consumed_len,
            Some(5)
        );
        assert_eq!(
            session.state().last_begin_place_config_object,
            Some(TypeIoObject::Int(7))
        );
        assert_eq!(session.state().builder_queue_projection.queued_count, 0);
        assert_eq!(session.state().builder_queue_projection.inflight_count, 1);
        assert_eq!(
            session.state().builder_queue_projection.last_stage,
            Some(crate::session_state::BuilderPlanStage::InFlight)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_block_id,
            Some(0x0101)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_rotation,
            Some(3)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_team_id,
            Some(1)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_builder_kind,
            Some(2)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_builder_value,
            Some(42)
        );
        assert!(session.snapshot_input_mut().building);
    }

    #[test]
    fn begin_place_packet_with_truncated_config_object_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "beginPlace")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(1);
        payload.extend_from_slice(&100i32.to_be_bytes());
        payload.extend_from_slice(&99i32.to_be_bytes());
        payload.extend_from_slice(&3i32.to_be_bytes());
        payload.push(1);
        payload.extend_from_slice(&7i32.to_be_bytes()[..2]);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_begin_place_count, 0);
        assert_eq!(session.state().last_begin_place_x, None);
        assert_eq!(session.state().last_begin_place_config_kind, None);
        assert_eq!(session.state().last_begin_place_config_object, None);
    }

    #[test]
    fn begin_place_packet_with_trailing_config_bytes_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "beginPlace")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(1);
        payload.extend_from_slice(&100i32.to_be_bytes());
        payload.extend_from_slice(&99i32.to_be_bytes());
        payload.extend_from_slice(&3i32.to_be_bytes());
        write_typeio_object(&mut payload, &TypeIoObject::Int(7));
        payload.push(0xff);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_begin_place_count, 0);
        assert_eq!(session.state().last_begin_place_x, None);
        assert_eq!(session.state().last_begin_place_config_kind, None);
        assert_eq!(session.state().last_begin_place_config_object, None);
    }

    #[test]
    fn begin_break_packet_emits_summary_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "beginBreak")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.push(1);
        payload.extend_from_slice(&100i32.to_be_bytes());
        payload.extend_from_slice(&99i32.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::BeginBreak {
                x: 100,
                y: 99,
                team_id: 1,
                builder_kind: 2,
                builder_value: 42,
            }
        );
        assert_eq!(session.state().received_begin_break_count, 1);
        assert_eq!(session.state().last_begin_break_x, Some(100));
        assert_eq!(session.state().last_begin_break_y, Some(99));
        assert_eq!(session.state().last_begin_break_team_id, Some(1));
        assert_eq!(session.state().builder_queue_projection.queued_count, 0);
        assert_eq!(session.state().builder_queue_projection.inflight_count, 1);
        assert_eq!(
            session.state().builder_queue_projection.last_stage,
            Some(crate::session_state::BuilderPlanStage::InFlight)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_breaking,
            Some(true)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_team_id,
            Some(1)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_builder_kind,
            Some(2)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_builder_value,
            Some(42)
        );
        assert!(session.snapshot_input_mut().building);
    }

    #[test]
    fn construct_finish_packet_emits_summary_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.snapshot_input_mut().plans = Some(vec![ClientBuildPlan {
            tile: (100, 99),
            breaking: false,
            block_id: Some(0x0101),
            rotation: 0,
            config: ClientBuildPlanConfig::None,
        }]);
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "constructFinish")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&pack_point2(100, 99).to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.push(3);
        payload.push(1);
        payload.push(0);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ConstructFinish {
                tile_pos: pack_point2(100, 99),
                block_id: Some(0x0101),
                builder_kind: 2,
                builder_value: 42,
                rotation: 3,
                team_id: 1,
                config_kind: 0,
                removed_local_plan: true,
            }
        );
        assert_eq!(session.state().received_construct_finish_count, 1);
        assert_eq!(
            session.state().last_construct_finish_tile_pos,
            Some(pack_point2(100, 99))
        );
        assert_eq!(session.state().last_construct_finish_block_id, Some(0x0101));
        assert_eq!(session.state().last_construct_finish_config_kind, Some(0));
        assert_eq!(
            session
                .state()
                .last_construct_finish_config_kind_name
                .as_deref(),
            Some("null")
        );
        assert_eq!(
            session.state().last_construct_finish_config_consumed_len,
            Some(1)
        );
        assert_eq!(
            session.state().last_construct_finish_config_object,
            Some(TypeIoObject::Null)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&pack_point2(100, 99))
                .and_then(|building| building.block_id),
            Some(0x0101)
        );
        assert!(session.state().last_construct_finish_removed_local_plan);
        assert_eq!(session.snapshot_input_mut().plans, Some(Vec::new()));
        assert_eq!(session.state().builder_queue_projection.queued_count, 0);
        assert_eq!(session.state().builder_queue_projection.inflight_count, 0);
        assert_eq!(session.state().builder_queue_projection.finished_count, 1);
        assert_eq!(
            session
                .state()
                .builder_queue_projection
                .orphan_authoritative_count,
            0
        );
        assert_eq!(
            session.state().builder_queue_projection.last_stage,
            Some(crate::session_state::BuilderPlanStage::Finished)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_breaking,
            Some(false)
        );
        assert!(
            session
                .state()
                .builder_queue_projection
                .last_removed_local_plan
        );
        assert!(!session.snapshot_input_mut().building);
    }

    #[test]
    fn construct_finish_packet_prunes_same_tile_break_plan() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.snapshot_input_mut().plans = Some(vec![ClientBuildPlan {
            tile: (100, 99),
            breaking: true,
            block_id: None,
            rotation: 0,
            config: ClientBuildPlanConfig::None,
        }]);
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "constructFinish")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&pack_point2(100, 99).to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.push(3);
        payload.push(1);
        payload.push(0);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ConstructFinish {
                tile_pos: pack_point2(100, 99),
                block_id: Some(0x0101),
                builder_kind: 2,
                builder_value: 42,
                rotation: 3,
                team_id: 1,
                config_kind: 0,
                removed_local_plan: true,
            }
        );
        assert_eq!(session.snapshot_input_mut().plans, Some(Vec::new()));
        assert!(session.state().last_construct_finish_removed_local_plan);
    }

    #[test]
    fn construct_finish_packet_tracks_full_typeio_config_object() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "constructFinish")
            .unwrap()
            .packet_id;
        let expected_config = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        let mut payload = Vec::new();
        payload.extend_from_slice(&pack_point2(120, 75).to_be_bytes());
        payload.extend_from_slice(&0x0102i16.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice(&7i32.to_be_bytes());
        payload.push(1);
        payload.push(1);
        write_typeio_object(&mut payload, &expected_config);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::ConstructFinish {
                tile_pos: pack_point2(120, 75),
                block_id: Some(0x0102),
                builder_kind: 2,
                builder_value: 7,
                rotation: 1,
                team_id: 1,
                config_kind: 22,
                removed_local_plan: false,
            }
        );
        assert_eq!(session.state().received_construct_finish_count, 1);
        assert_eq!(session.state().last_construct_finish_config_kind, Some(22));
        assert_eq!(
            session
                .state()
                .last_construct_finish_config_kind_name
                .as_deref(),
            Some("object[]")
        );
        assert_eq!(
            session.state().last_construct_finish_config_consumed_len,
            Some(payload.len() - 13)
        );
        assert_eq!(
            session.state().last_construct_finish_config_object,
            Some(expected_config)
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&pack_point2(120, 75))
                .and_then(|building| building.config.as_ref()),
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::Int(7),
                TypeIoObject::String(Some("router".to_string())),
                TypeIoObject::Bool(true),
            ]))
        );
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .authoritative_by_build_pos
                .get(&pack_point2(120, 75)),
            Some(&TypeIoObject::ObjectArray(vec![
                TypeIoObject::Int(7),
                TypeIoObject::String(Some("router".to_string())),
                TypeIoObject::Bool(true),
            ]))
        );
        assert_eq!(session.state().builder_queue_projection.finished_count, 1);
        assert_eq!(
            session
                .state()
                .builder_queue_projection
                .orphan_authoritative_count,
            1
        );
        assert!(
            session
                .state()
                .builder_queue_projection
                .last_orphan_authoritative
        );
    }

    #[test]
    fn construct_finish_packet_with_incomplete_typeio_config_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "constructFinish")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&pack_point2(100, 99).to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        payload.push(3);
        payload.push(1);
        payload.push(4);
        payload.push(1);
        payload.extend_from_slice(&5u16.to_be_bytes());
        payload.extend_from_slice(b"ab");
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_construct_finish_count, 0);
        assert_eq!(session.state().last_construct_finish_config_kind, None);
        assert_eq!(session.state().last_construct_finish_config_object, None);
    }

    #[test]
    fn deconstruct_finish_packet_emits_summary_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session
            .state
            .tile_config_projection
            .seed_authoritative_state(pack_point2(100, 99), TypeIoObject::Int(7));
        session
            .state
            .building_table_projection
            .apply_construct_finish(pack_point2(100, 99), Some(0x0101), 0, 1, TypeIoObject::Null);
        session.snapshot_input_mut().plans = Some(vec![ClientBuildPlan {
            tile: (100, 99),
            breaking: true,
            block_id: None,
            rotation: 0,
            config: ClientBuildPlanConfig::None,
        }]);
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "deconstructFinish")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&pack_point2(100, 99).to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::DeconstructFinish {
                tile_pos: pack_point2(100, 99),
                block_id: Some(0x0101),
                builder_kind: 2,
                builder_value: 42,
                removed_local_plan: true,
            }
        );
        assert_eq!(session.state().received_deconstruct_finish_count, 1);
        assert_eq!(
            session.state().last_deconstruct_finish_tile_pos,
            Some(pack_point2(100, 99))
        );
        assert_eq!(
            session.state().last_deconstruct_finish_block_id,
            Some(0x0101)
        );
        assert!(session.state().last_deconstruct_finish_removed_local_plan);
        assert_eq!(session.snapshot_input_mut().plans, Some(Vec::new()));
        assert_eq!(session.state().builder_queue_projection.queued_count, 0);
        assert_eq!(session.state().builder_queue_projection.inflight_count, 0);
        assert_eq!(session.state().builder_queue_projection.finished_count, 1);
        assert_eq!(
            session.state().builder_queue_projection.last_breaking,
            Some(true)
        );
        assert_eq!(
            session.state().builder_queue_projection.last_stage,
            Some(crate::session_state::BuilderPlanStage::Finished)
        );
        assert!(session
            .state()
            .building_table_projection
            .by_build_pos
            .is_empty());
        assert!(session
            .state()
            .tile_config_projection
            .authoritative_by_build_pos
            .is_empty());
        assert_eq!(
            session.state().building_table_projection.last_update,
            Some(crate::session_state::BuildingProjectionUpdateKind::DeconstructFinish)
        );
        assert!(!session.snapshot_input_mut().building);
    }

    #[test]
    fn deconstruct_finish_packet_prunes_same_tile_place_plan() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.snapshot_input_mut().plans = Some(vec![ClientBuildPlan {
            tile: (100, 99),
            breaking: false,
            block_id: Some(0x0101),
            rotation: 0,
            config: ClientBuildPlanConfig::None,
        }]);
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "deconstructFinish")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&pack_point2(100, 99).to_be_bytes());
        payload.extend_from_slice(&0x0101i16.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice(&42i32.to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::DeconstructFinish {
                tile_pos: pack_point2(100, 99),
                block_id: Some(0x0101),
                builder_kind: 2,
                builder_value: 42,
                removed_local_plan: true,
            }
        );
        assert_eq!(session.snapshot_input_mut().plans, Some(Vec::new()));
        assert!(session.state().last_deconstruct_finish_removed_local_plan);
    }

    #[test]
    fn build_health_update_packet_emits_summary_event() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "buildHealthUpdate")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&4i32.to_be_bytes());
        payload.extend_from_slice(&123i32.to_be_bytes());
        payload.extend_from_slice(&(1.25f32.to_bits() as i32).to_be_bytes());
        payload.extend_from_slice(&456i32.to_be_bytes());
        payload.extend_from_slice(&(0.5f32.to_bits() as i32).to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::BuildHealthUpdate {
                pair_count: 2,
                first_build_pos: Some(123),
                first_health_bits: Some(1.25f32.to_bits()),
                pairs: vec![
                    BuildHealthPair {
                        build_pos: 123,
                        health_bits: 1.25f32.to_bits(),
                    },
                    BuildHealthPair {
                        build_pos: 456,
                        health_bits: 0.5f32.to_bits(),
                    },
                ],
            }
        );
        assert_eq!(session.state().received_build_health_update_count, 1);
        assert_eq!(session.state().received_build_health_update_pair_count, 2);
        assert_eq!(session.state().last_build_health_update_pair_count, 2);
        assert_eq!(
            session.state().last_build_health_update_first_build_pos,
            Some(123)
        );
        assert_eq!(
            session.state().last_build_health_update_first_health_bits,
            Some(1.25f32.to_bits())
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .build_health_apply_count,
            2
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&123)
                .and_then(|building| building.health_bits),
            Some(1.25f32.to_bits())
        );
        assert_eq!(
            session
                .state()
                .building_table_projection
                .by_build_pos
                .get(&456)
                .and_then(|building| building.health_bits),
            Some(0.5f32.to_bits())
        );
        assert_eq!(
            session.state().building_table_projection.last_update,
            Some(crate::session_state::BuildingProjectionUpdateKind::BuildHealthUpdate)
        );
    }

    #[test]
    fn malformed_build_health_update_packet_stays_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "buildHealthUpdate")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&3i32.to_be_bytes());
        payload.extend_from_slice(&123i32.to_be_bytes());
        payload.extend_from_slice(&(1.25f32.to_bits() as i32).to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "buildHealthUpdate".to_string(),
                    packet_class: "mindustry.gen.BuildHealthUpdateCallPacket".to_string(),
                }),
            }
        );
        assert_eq!(session.state().received_build_health_update_count, 0);
        assert_eq!(session.state().received_build_health_update_pair_count, 0);
    }

    #[test]
    fn queued_send_chat_message_waits_for_world_ready_and_uses_tcp_transport() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();

        session
            .queue_send_chat_message("/sync".to_string())
            .unwrap();
        let early_actions = session.advance_time(500).unwrap();
        assert!(!early_actions.iter().any(|action| {
            matches!(
                action,
                ClientSessionAction::SendPacket {
                    transport: ClientPacketTransport::Tcp,
                    ..
                }
            )
        }));

        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        session.prepare_connect_confirm_packet().unwrap();

        let actions = session.advance_time(1_000).unwrap();
        assert_eq!(actions.len(), 3);
        match &actions[0] {
            ClientSessionAction::SendPacket {
                packet_id,
                transport,
                bytes,
            } => {
                assert_eq!(*transport, ClientPacketTransport::Tcp);
                let expected_packet_id = manifest
                    .remote_packets
                    .iter()
                    .find(|entry| entry.method == "sendChatMessage")
                    .unwrap()
                    .packet_id;
                assert_eq!(*packet_id, expected_packet_id);
                let decoded = decode_packet(bytes).unwrap();
                assert_eq!(decoded.packet_id, expected_packet_id);
                assert_eq!(decoded.payload, encode_typeio_string_payload("/sync"));
            }
            other => panic!("expected queued chat packet, got {other:?}"),
        }
        assert!(matches!(
            &actions[1],
            ClientSessionAction::SendPacket {
                transport: ClientPacketTransport::Tcp,
                ..
            }
        ));
        assert!(matches!(
            &actions[2],
            ClientSessionAction::SendPacket {
                transport: ClientPacketTransport::Udp,
                ..
            }
        ));
    }

    #[test]
    fn queued_menu_choose_and_text_input_result_use_expected_payloads() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let menu_choose_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "menuChoose")
            .unwrap()
            .packet_id;
        let text_input_result_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "textInputResult")
            .unwrap()
            .packet_id;

        session.queue_menu_choose(12, -1).unwrap();
        session.queue_text_input_result(9, Some("router")).unwrap();
        session.queue_text_input_result(10, None).unwrap();

        let actions = session.advance_time(1).unwrap();
        let expected = vec![
            (
                menu_choose_packet_id,
                ClientPacketTransport::Tcp,
                encode_menu_choose_payload(12, -1),
            ),
            (
                text_input_result_packet_id,
                ClientPacketTransport::Tcp,
                encode_text_input_result_payload(9, Some("router")),
            ),
            (
                text_input_result_packet_id,
                ClientPacketTransport::Tcp,
                encode_text_input_result_payload(10, None),
            ),
        ];
        let relevant_actions = actions
            .iter()
            .filter(|action| {
                matches!(
                    action,
                    ClientSessionAction::SendPacket { packet_id, .. }
                        if *packet_id == menu_choose_packet_id
                            || *packet_id == text_input_result_packet_id
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(relevant_actions.len(), 3);

        for (action, (expected_packet_id, expected_transport, expected_payload)) in
            relevant_actions.into_iter().zip(expected.into_iter())
        {
            match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport,
                    bytes,
                } => {
                    assert_eq!(*packet_id, expected_packet_id);
                    assert_eq!(*transport, expected_transport);
                    let decoded = decode_packet(bytes).unwrap();
                    assert_eq!(decoded.packet_id, expected_packet_id);
                    assert_eq!(decoded.payload, expected_payload);
                }
                other => panic!("expected queued menu/text packet action, got {other:?}"),
            }
        }
    }

    #[test]
    fn queued_custom_and_logic_packets_use_expected_ids_transports_and_payloads() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let client_packet_reliable_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketReliable")
            .unwrap()
            .packet_id;
        let client_packet_unreliable_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientPacketUnreliable")
            .unwrap()
            .packet_id;
        let client_binary_packet_reliable_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketReliable")
            .unwrap()
            .packet_id;
        let client_binary_packet_unreliable_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientBinaryPacketUnreliable")
            .unwrap()
            .packet_id;
        let client_logic_data_reliable_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataReliable")
            .unwrap()
            .packet_id;
        let client_logic_data_unreliable_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "clientLogicDataUnreliable")
            .unwrap()
            .packet_id;

        session
            .queue_client_packet("mod.text", "alpha", ClientPacketTransport::Tcp)
            .unwrap();
        session
            .queue_client_packet("mod.text", "beta", ClientPacketTransport::Udp)
            .unwrap();
        session
            .queue_client_binary_packet("mod.bin", &[1, 2, 3], ClientPacketTransport::Tcp)
            .unwrap();
        session
            .queue_client_binary_packet("mod.bin", &[4, 5], ClientPacketTransport::Udp)
            .unwrap();
        session
            .queue_client_logic_data(
                "logic.r",
                &TypeIoObject::Int(7),
                ClientLogicDataTransport::Reliable,
            )
            .unwrap();
        session
            .queue_client_logic_data(
                "logic.u",
                &TypeIoObject::Bool(true),
                ClientLogicDataTransport::Unreliable,
            )
            .unwrap();

        let actions = session.advance_time(1).unwrap();
        let expected = vec![
            (
                client_packet_reliable_id,
                ClientPacketTransport::Tcp,
                encode_client_packet_payload("mod.text", "alpha"),
            ),
            (
                client_packet_unreliable_id,
                ClientPacketTransport::Udp,
                encode_client_packet_payload("mod.text", "beta"),
            ),
            (
                client_binary_packet_reliable_id,
                ClientPacketTransport::Tcp,
                encode_client_binary_packet_payload("mod.bin", &[1, 2, 3]),
            ),
            (
                client_binary_packet_unreliable_id,
                ClientPacketTransport::Udp,
                encode_client_binary_packet_payload("mod.bin", &[4, 5]),
            ),
            (
                client_logic_data_reliable_id,
                ClientPacketTransport::Tcp,
                encode_client_logic_data_payload("logic.r", &TypeIoObject::Int(7)),
            ),
            (
                client_logic_data_unreliable_id,
                ClientPacketTransport::Udp,
                encode_client_logic_data_payload("logic.u", &TypeIoObject::Bool(true)),
            ),
        ];
        let relevant_actions = actions
            .iter()
            .filter(|action| {
                matches!(
                    action,
                    ClientSessionAction::SendPacket { packet_id, .. }
                        if *packet_id == client_packet_reliable_id
                            || *packet_id == client_packet_unreliable_id
                            || *packet_id == client_binary_packet_reliable_id
                            || *packet_id == client_binary_packet_unreliable_id
                            || *packet_id == client_logic_data_reliable_id
                            || *packet_id == client_logic_data_unreliable_id
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(relevant_actions.len(), 6);

        for (action, (expected_packet_id, expected_transport, expected_payload)) in
            relevant_actions.into_iter().zip(expected.into_iter())
        {
            match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport,
                    bytes,
                } => {
                    assert_eq!(*packet_id, expected_packet_id);
                    assert_eq!(*transport, expected_transport);
                    let decoded = decode_packet(bytes).unwrap();
                    assert_eq!(decoded.packet_id, expected_packet_id);
                    assert_eq!(decoded.payload, expected_payload);
                }
                other => panic!("expected queued custom packet action, got {other:?}"),
            }
        }
    }

    #[test]
    fn queued_admin_request_and_request_debug_status_use_expected_payloads() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let admin_request_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "adminRequest")
            .unwrap()
            .packet_id;
        let request_debug_status_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "requestDebugStatus")
            .unwrap()
            .packet_id;
        let params = TypeIoObject::String(Some("golden-params".to_string()));
        session.queue_admin_request(456, 4, &params).unwrap();
        session.queue_request_debug_status().unwrap();

        let actions = session.advance_time(1).unwrap();
        let expected = vec![
            (
                admin_request_packet_id,
                ClientPacketTransport::Tcp,
                encode_admin_request_payload(456, 4, &params),
            ),
            (
                request_debug_status_packet_id,
                ClientPacketTransport::Tcp,
                Vec::new(),
            ),
        ];
        let relevant_actions = actions
            .iter()
            .filter(|action| {
                matches!(
                    action,
                    ClientSessionAction::SendPacket { packet_id, .. }
                        if *packet_id == admin_request_packet_id
                            || *packet_id == request_debug_status_packet_id
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(relevant_actions.len(), 2);

        for (action, (expected_packet_id, expected_transport, expected_payload)) in
            relevant_actions.into_iter().zip(expected.into_iter())
        {
            match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport,
                    bytes,
                } => {
                    assert_eq!(*packet_id, expected_packet_id);
                    assert_eq!(*transport, expected_transport);
                    let decoded = decode_packet(bytes).unwrap();
                    assert_eq!(decoded.packet_id, expected_packet_id);
                    assert_eq!(decoded.payload, expected_payload);
                }
                other => panic!("expected queued admin/debug packet action, got {other:?}"),
            }
        }
    }

    #[test]
    fn queued_gameplay_remote_actions_wait_for_world_ready_and_preserve_transport_order() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();

        session.queue_request_item(Some(222), Some(9), 15).unwrap();
        session
            .queue_request_unit_payload(ClientUnitRef::Standard(444))
            .unwrap();
        session.queue_clear_items(Some(333)).unwrap();
        session.queue_clear_liquids(Some(334)).unwrap();
        session.queue_building_control_select(Some(555)).unwrap();
        session.queue_transfer_inventory(Some(321)).unwrap();
        session.queue_request_build_payload(Some(654)).unwrap();
        session.queue_request_drop_payload(12.5, 48.0).unwrap();
        session.queue_rotate_block(Some(777), true).unwrap();
        session.queue_drop_item(135.0).unwrap();
        let tile_config = TypeIoObject::ObjectArray(vec![
            TypeIoObject::Int(7),
            TypeIoObject::String(Some("router".to_string())),
            TypeIoObject::Bool(true),
        ]);
        session
            .queue_tile_config(Some(888), tile_config.clone())
            .unwrap();
        session.queue_tile_tap(Some(999)).unwrap();
        session
            .queue_delete_plans(&[pack_point2(1, 2), pack_point2(-3, 4)])
            .unwrap();
        session.queue_unit_clear().unwrap();
        session
            .queue_unit_control(ClientUnitRef::Block(111))
            .unwrap();
        session
            .queue_unit_building_control_select(ClientUnitRef::Standard(222), Some(333))
            .unwrap();
        session
            .queue_command_building(&[pack_point2(5, 6), pack_point2(-7, 8)], 12.5, -4.0)
            .unwrap();
        session
            .queue_command_units(
                &[111, 222],
                Some(pack_point2(9, 10)),
                ClientUnitRef::Standard(333),
                Some((48.0, 96.0)),
                true,
                false,
            )
            .unwrap();
        session
            .queue_set_unit_command(&[333, 444], Some(12))
            .unwrap();
        session
            .queue_set_unit_stance(&[555, 666], Some(7), false)
            .unwrap();
        session
            .queue_begin_break(ClientUnitRef::Standard(777), 8, -11, 22)
            .unwrap();
        session
            .queue_begin_place(
                ClientUnitRef::Block(888),
                Some(999),
                3,
                44,
                -55,
                2,
                &TypeIoObject::Point2 { x: 7, y: -8 },
            )
            .unwrap();

        let early_actions = session.advance_time(500).unwrap();
        assert!(!early_actions
            .iter()
            .any(|action| matches!(action, ClientSessionAction::SendPacket { .. })));

        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }
        session.prepare_connect_confirm_packet().unwrap();

        let actions = session.advance_time(1_000).unwrap();

        let expected = [
            (
                "requestItem",
                ClientPacketTransport::Tcp,
                encode_request_item_payload(Some(222), Some(9), 15),
            ),
            (
                "requestUnitPayload",
                ClientPacketTransport::Tcp,
                encode_unit_payload(ClientUnitRef::Standard(444)),
            ),
            (
                "clearItems",
                ClientPacketTransport::Udp,
                encode_building_payload(Some(333)),
            ),
            (
                "clearLiquids",
                ClientPacketTransport::Udp,
                encode_building_payload(Some(334)),
            ),
            (
                "buildingControlSelect",
                ClientPacketTransport::Tcp,
                encode_building_payload(Some(555)),
            ),
            (
                "transferInventory",
                ClientPacketTransport::Tcp,
                encode_building_payload(Some(321)),
            ),
            (
                "requestBuildPayload",
                ClientPacketTransport::Tcp,
                encode_building_payload(Some(654)),
            ),
            (
                "requestDropPayload",
                ClientPacketTransport::Tcp,
                encode_two_f32_payload(12.5, 48.0),
            ),
            (
                "rotateBlock",
                ClientPacketTransport::Udp,
                encode_building_bool_payload(Some(777), true),
            ),
            (
                "dropItem",
                ClientPacketTransport::Tcp,
                encode_single_f32_payload(135.0),
            ),
            (
                "tileConfig",
                ClientPacketTransport::Tcp,
                encode_tile_config_payload(Some(888), &tile_config),
            ),
            (
                "tileTap",
                ClientPacketTransport::Udp,
                encode_building_payload(Some(999)),
            ),
            (
                "deletePlans",
                ClientPacketTransport::Udp,
                encode_delete_plans_payload(&[pack_point2(1, 2), pack_point2(-3, 4)]),
            ),
            ("unitClear", ClientPacketTransport::Tcp, Vec::new()),
            (
                "unitControl",
                ClientPacketTransport::Tcp,
                encode_unit_payload(ClientUnitRef::Block(111)),
            ),
            (
                "unitBuildingControlSelect",
                ClientPacketTransport::Tcp,
                encode_unit_building_payload(ClientUnitRef::Standard(222), Some(333)),
            ),
            (
                "commandBuilding",
                ClientPacketTransport::Tcp,
                encode_command_building_payload(
                    &[pack_point2(5, 6), pack_point2(-7, 8)],
                    12.5,
                    -4.0,
                ),
            ),
            (
                "commandUnits",
                ClientPacketTransport::Tcp,
                encode_command_units_payload(
                    &[111, 222],
                    Some(pack_point2(9, 10)),
                    ClientUnitRef::Standard(333),
                    Some((48.0, 96.0)),
                    true,
                    false,
                ),
            ),
            (
                "setUnitCommand",
                ClientPacketTransport::Tcp,
                encode_set_unit_command_payload(&[333, 444], Some(12)),
            ),
            (
                "setUnitStance",
                ClientPacketTransport::Tcp,
                encode_set_unit_stance_payload(&[555, 666], Some(7), false),
            ),
            (
                "beginBreak",
                ClientPacketTransport::Tcp,
                encode_begin_break_payload(ClientUnitRef::Standard(777), 8, -11, 22),
            ),
            (
                "beginPlace",
                ClientPacketTransport::Tcp,
                encode_begin_place_payload(
                    ClientUnitRef::Block(888),
                    Some(999),
                    3,
                    44,
                    -55,
                    2,
                    &TypeIoObject::Point2 { x: 7, y: -8 },
                ),
            ),
        ];
        assert!(actions.len() >= expected.len() + 2);

        for (action, (method, expected_transport, expected_payload)) in
            actions.iter().zip(expected.iter())
        {
            let expected_packet_id = manifest
                .remote_packets
                .iter()
                .find(|entry| entry.method == *method)
                .unwrap()
                .packet_id;
            match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport,
                    bytes,
                } => {
                    assert_eq!(*packet_id, expected_packet_id);
                    assert_eq!(*transport, *expected_transport);
                    let decoded = decode_packet(bytes).unwrap();
                    assert_eq!(decoded.packet_id, expected_packet_id);
                    assert_eq!(decoded.payload, *expected_payload);
                }
                other => panic!("expected queued gameplay packet, got {other:?}"),
            }
        }
        assert!(matches!(
            &actions[expected.len()],
            ClientSessionAction::SendPacket {
                transport: ClientPacketTransport::Tcp,
                ..
            }
        ));
        assert!(matches!(
            &actions[expected.len() + 1],
            ClientSessionAction::SendPacket {
                transport: ClientPacketTransport::Udp,
                ..
            }
        ));
    }

    #[test]
    fn queued_command_units_chunked_uses_java_like_default_chunk_and_final_batch() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let unit_ids = (1..=401).collect::<Vec<i32>>();
        let build_target = Some(pack_point2(9, 10));
        let unit_target = ClientUnitRef::Standard(333);
        let pos_target = Some((48.0, 96.0));
        let queued = session
            .queue_command_units_chunked(&unit_ids, build_target, unit_target, pos_target, true)
            .unwrap();
        assert_eq!(queued, 3);

        let command_units_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandUnits")
            .unwrap()
            .packet_id;

        let actions = session.advance_time(1).unwrap();
        let command_actions = actions
            .iter()
            .filter_map(|action| match action {
                ClientSessionAction::SendPacket {
                    packet_id,
                    transport: ClientPacketTransport::Tcp,
                    bytes,
                } if *packet_id == command_units_packet_id => Some(bytes),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(command_actions.len(), 3);

        let expected_lengths = [200usize, 200usize, 1usize];
        let expected_final_batch = [false, false, true];
        let expected_first_ids = [1i32, 201i32, 401i32];
        for (index, bytes) in command_actions.iter().enumerate() {
            let decoded = decode_packet(bytes).unwrap();
            let summary = decode_command_units_payload(&decoded.payload).unwrap();
            assert_eq!(summary.unit_ids.len(), expected_lengths[index]);
            assert_eq!(summary.unit_ids.first().copied(), Some(expected_first_ids[index]));
            assert_eq!(summary.build_target, build_target);
            assert_eq!(
                summary.unit_target,
                Some(UnitRefProjection {
                    kind: 2,
                    value: 333,
                })
            );
            assert_eq!(summary.x, 48.0);
            assert_eq!(summary.y, 96.0);
            assert!(summary.queue_command);
            assert_eq!(summary.final_batch, expected_final_batch[index]);
        }
    }

    #[test]
    fn queued_command_units_chunked_with_empty_units_is_noop() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
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
        session.prepare_connect_confirm_packet().unwrap();

        let queued = session
            .queue_command_units_chunked(
                &[],
                Some(pack_point2(9, 10)),
                ClientUnitRef::Standard(333),
                Some((48.0, 96.0)),
                true,
            )
            .unwrap();
        assert_eq!(queued, 0);

        let command_units_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "commandUnits")
            .unwrap()
            .packet_id;
        let actions = session.advance_time(1).unwrap();
        assert!(!actions.iter().any(|action| {
            matches!(
                action,
                ClientSessionAction::SendPacket { packet_id, .. } if *packet_id == command_units_packet_id
            )
        }));
    }

    #[test]
    fn tile_config_payload_encodes_building_and_nested_typeio_object() {
        let payload = encode_tile_config_payload(
            Some(0x0102_0304),
            &TypeIoObject::ObjectArray(vec![
                TypeIoObject::Int(7),
                TypeIoObject::String(Some("ok".to_string())),
                TypeIoObject::Bool(false),
            ]),
        );
        assert_eq!(
            payload,
            vec![
                0x01, 0x02, 0x03, 0x04, 22, 0, 0, 0, 3, 1, 0, 0, 0, 7, 4, 1, 0, 2, b'o', b'k', 10,
                0,
            ]
        );
    }

    #[test]
    fn delete_plans_payload_encodes_short_len_and_positions() {
        let payload = encode_delete_plans_payload(&[0x0102_0304, -1]);
        assert_eq!(
            payload,
            vec![0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn unit_building_payload_encodes_unit_then_building() {
        let payload = encode_unit_building_payload(ClientUnitRef::Block(123), Some(456));
        assert_eq!(payload, vec![1, 0, 0, 0, 123, 0, 0, 1, 200]);
    }

    #[test]
    fn command_building_payload_encodes_ints_then_vec2() {
        let payload = encode_command_building_payload(&[0x0102_0304, -1], 12.5, -4.0);
        assert_eq!(
            payload,
            vec![
                0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff, 0x41, 0x48, 0x00, 0x00,
                0xc0, 0x80, 0x00, 0x00,
            ]
        );
    }

    #[test]
    fn command_units_payload_encodes_java_wire_shape() {
        let payload = encode_command_units_payload(
            &[0x0102_0304, -1],
            Some(0x0a0b_0c0d),
            ClientUnitRef::Block(0x0102_0304),
            Some((12.5, -4.0)),
            true,
            false,
        );
        assert_eq!(
            payload,
            vec![
                0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff, 0x0a, 0x0b, 0x0c, 0x0d,
                0x01, 0x01, 0x02, 0x03, 0x04, 0x41, 0x48, 0x00, 0x00, 0xc0, 0x80, 0x00, 0x00, 0x01,
                0x00,
            ]
        );
    }

    #[test]
    fn set_unit_command_payload_encodes_java_wire_shape() {
        let payload = encode_set_unit_command_payload(&[0x0102_0304, -1], Some(12));
        assert_eq!(
            payload,
            vec![0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff, 0x0c,]
        );
    }

    #[test]
    fn set_unit_stance_payload_encodes_java_wire_shape() {
        let payload = encode_set_unit_stance_payload(&[0x0102_0304, -1], Some(7), false);
        assert_eq!(
            payload,
            vec![0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff, 0x07, 0x00,]
        );
    }

    #[test]
    fn set_position_packet_updates_snapshot_input_position() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setPosition")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&123.5f32.to_bits().to_be_bytes());
        payload.extend_from_slice(&456.25f32.to_bits().to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::PlayerPositionUpdated {
                x: 123.5,
                y: 456.25
            }
        );
        assert_eq!(
            session.state().world_player_x_bits,
            Some(123.5f32.to_bits())
        );
        assert_eq!(
            session.state().world_player_y_bits,
            Some(456.25f32.to_bits())
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.position, Some((123.5, 456.25)));
        assert_eq!(input.view_center, Some((123.5, 456.25)));
    }

    #[test]
    fn set_camera_position_packet_updates_view_center_without_moving_player_position() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.snapshot_input_mut().position = Some((10.0, 20.0));
        session.snapshot_input_mut().view_center = Some((10.0, 20.0));
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setCameraPosition")
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&123.5f32.to_bits().to_be_bytes());
        payload.extend_from_slice(&456.25f32.to_bits().to_be_bytes());
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::CameraPositionUpdated {
                x: 123.5,
                y: 456.25,
            }
        );
        assert_eq!(session.state().received_set_camera_position_count, 1);
        assert_eq!(session.state().last_camera_x_bits, Some(123.5f32.to_bits()));
        assert_eq!(
            session.state().last_camera_y_bits,
            Some(456.25f32.to_bits())
        );
        let input = session.snapshot_input_mut();
        assert_eq!(input.position, Some((10.0, 20.0)));
        assert_eq!(input.view_center, Some((123.5, 456.25)));
    }

    #[test]
    fn take_items_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "takeItems")
            .unwrap()
            .packet_id;
        let projection = TakeItemsProjection {
            build_pos: Some(pack_point2(7, 11)),
            item_id: Some(9),
            amount: 13,
            to: Some(UnitRefProjection { kind: 2, value: 77 }),
        };
        let packet = encode_packet(
            packet_id,
            &encode_take_items_payload(
                projection.build_pos,
                projection.item_id,
                projection.amount,
                ClientUnitRef::Standard(77),
            ),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TakeItems {
                projection: projection.clone()
            }
        );
        assert_eq!(session.state().received_take_items_count, 1);
        assert_eq!(session.state().last_take_items, Some(projection));
    }

    #[test]
    fn transfer_item_packets_emit_events_and_update_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let transfer_item_to_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferItemTo")
            .unwrap()
            .packet_id;
        let transfer_item_to_projection = TransferItemToProjection {
            unit: Some(UnitRefProjection {
                kind: 1,
                value: pack_point2(5, 6),
            }),
            item_id: Some(4),
            amount: 22,
            x_bits: 12.5f32.to_bits(),
            y_bits: (-4.0f32).to_bits(),
            build_pos: Some(pack_point2(9, 10)),
        };
        let transfer_item_to_packet = encode_packet(
            transfer_item_to_packet_id,
            &encode_transfer_item_to_payload(
                ClientUnitRef::Block(pack_point2(5, 6)),
                transfer_item_to_projection.item_id,
                transfer_item_to_projection.amount,
                f32::from_bits(transfer_item_to_projection.x_bits),
                f32::from_bits(transfer_item_to_projection.y_bits),
                transfer_item_to_projection.build_pos,
            ),
            false,
        )
        .unwrap();

        let transfer_item_to_event = session
            .ingest_packet_bytes(&transfer_item_to_packet)
            .unwrap();

        assert_eq!(
            transfer_item_to_event,
            ClientSessionEvent::TransferItemTo {
                projection: transfer_item_to_projection.clone(),
            }
        );
        assert_eq!(session.state().received_transfer_item_to_count, 1);
        assert_eq!(
            session.state().last_transfer_item_to,
            Some(transfer_item_to_projection)
        );

        let transfer_item_to_unit_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferItemToUnit")
            .unwrap()
            .packet_id;
        let transfer_item_to_unit_projection = TransferItemToUnitProjection {
            item_id: Some(15),
            x_bits: 3.25f32.to_bits(),
            y_bits: 4.5f32.to_bits(),
            to_entity_id: Some(1234),
        };
        let transfer_item_to_unit_packet = encode_packet(
            transfer_item_to_unit_packet_id,
            &encode_transfer_item_to_unit_payload(
                transfer_item_to_unit_projection.item_id,
                f32::from_bits(transfer_item_to_unit_projection.x_bits),
                f32::from_bits(transfer_item_to_unit_projection.y_bits),
                transfer_item_to_unit_projection.to_entity_id,
            ),
            false,
        )
        .unwrap();

        let transfer_item_to_unit_event = session
            .ingest_packet_bytes(&transfer_item_to_unit_packet)
            .unwrap();

        assert_eq!(
            transfer_item_to_unit_event,
            ClientSessionEvent::TransferItemToUnit {
                projection: transfer_item_to_unit_projection.clone(),
            }
        );
        assert_eq!(session.state().received_transfer_item_to_unit_count, 1);
        assert_eq!(
            session.state().last_transfer_item_to_unit,
            Some(transfer_item_to_unit_projection)
        );
    }

    #[test]
    fn payload_packets_emit_events_and_update_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();

        let payload_dropped_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "payloadDropped")
            .unwrap()
            .packet_id;
        let payload_dropped_projection = PayloadDroppedProjection {
            unit: Some(UnitRefProjection { kind: 2, value: 41 }),
            x_bits: 8.0f32.to_bits(),
            y_bits: 16.0f32.to_bits(),
        };
        let payload_dropped_packet = encode_packet(
            payload_dropped_packet_id,
            &encode_payload_dropped_payload(ClientUnitRef::Standard(41), 8.0, 16.0),
            false,
        )
        .unwrap();
        let payload_dropped_event = session
            .ingest_packet_bytes(&payload_dropped_packet)
            .unwrap();
        assert_eq!(
            payload_dropped_event,
            ClientSessionEvent::PayloadDropped {
                projection: payload_dropped_projection.clone(),
            }
        );
        assert_eq!(session.state().received_payload_dropped_count, 1);
        assert_eq!(
            session.state().last_payload_dropped,
            Some(payload_dropped_projection)
        );

        let picked_build_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "pickedBuildPayload")
            .unwrap()
            .packet_id;
        let picked_build_payload_projection = PickedBuildPayloadProjection {
            unit: Some(UnitRefProjection {
                kind: 1,
                value: pack_point2(2, 3),
            }),
            build_pos: Some(pack_point2(9, 12)),
            on_ground: true,
        };
        let picked_build_payload_packet = encode_packet(
            picked_build_payload_packet_id,
            &encode_picked_build_payload(
                ClientUnitRef::Block(pack_point2(2, 3)),
                picked_build_payload_projection.build_pos,
                true,
            ),
            false,
        )
        .unwrap();
        let picked_build_payload_event = session
            .ingest_packet_bytes(&picked_build_payload_packet)
            .unwrap();
        assert_eq!(
            picked_build_payload_event,
            ClientSessionEvent::PickedBuildPayload {
                projection: picked_build_payload_projection.clone(),
            }
        );
        assert_eq!(session.state().received_picked_build_payload_count, 1);
        assert_eq!(
            session.state().last_picked_build_payload,
            Some(picked_build_payload_projection)
        );

        let picked_unit_payload_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "pickedUnitPayload")
            .unwrap()
            .packet_id;
        let picked_unit_payload_projection = PickedUnitPayloadProjection {
            unit: Some(UnitRefProjection { kind: 2, value: 5 }),
            target: Some(UnitRefProjection { kind: 2, value: 6 }),
        };
        let picked_unit_payload_packet = encode_packet(
            picked_unit_payload_packet_id,
            &encode_picked_unit_payload(ClientUnitRef::Standard(5), ClientUnitRef::Standard(6)),
            false,
        )
        .unwrap();
        let picked_unit_payload_event = session
            .ingest_packet_bytes(&picked_unit_payload_packet)
            .unwrap();
        assert_eq!(
            picked_unit_payload_event,
            ClientSessionEvent::PickedUnitPayload {
                projection: picked_unit_payload_projection.clone(),
            }
        );
        assert_eq!(session.state().received_picked_unit_payload_count, 1);
        assert_eq!(
            session.state().last_picked_unit_payload,
            Some(picked_unit_payload_projection)
        );
    }

    #[test]
    fn unit_despawn_packet_removes_entity_projection_for_standard_unit() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.state.entity_table_projection.by_entity_id.insert(
            77,
            crate::session_state::EntityProjection {
                class_id: 5,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 77,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitDespawn")
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_unit_payload(ClientUnitRef::Standard(77)),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::UnitDespawned {
                unit: Some(UnitRefProjection { kind: 2, value: 77 }),
                removed_entity_projection: true,
            }
        );
        assert_eq!(session.state().received_unit_despawn_count, 1);
        assert_eq!(
            session.state().last_unit_despawn,
            Some(UnitRefProjection { kind: 2, value: 77 })
        );
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&77));
        assert!(session.state().entity_snapshot_tombstones.contains_key(&77));
    }

    #[test]
    fn unit_entered_payload_packet_removes_entity_projection_for_standard_unit() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        session.state.entity_table_projection.by_entity_id.insert(
            77,
            crate::session_state::EntityProjection {
                class_id: 5,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 77,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        let projection = UnitEnteredPayloadProjection {
            unit: Some(UnitRefProjection { kind: 2, value: 77 }),
            build_pos: Some(0x0003_0004),
        };
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "unitEnteredPayload")
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_unit_entered_payload(ClientUnitRef::Standard(77), projection.build_pos),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::UnitEnteredPayload {
                projection: projection.clone(),
                removed_entity_projection: true,
            }
        );
        assert_eq!(session.state().received_unit_entered_payload_count, 1);
        assert_eq!(
            session.state().last_unit_entered_payload,
            Some(projection.clone())
        );
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&77));
        assert!(session.state().entity_snapshot_tombstones.contains_key(&77));
    }

    #[test]
    fn transfer_item_to_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "transferItemTo")
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &[2, 0, 0, 0, 7, 0, 4, 0], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_transfer_item_to_count, 0);
        assert_eq!(session.state().last_transfer_item_to, None);
    }

    #[test]
    fn sound_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sound" && entry.params.len() == 4)
            .unwrap()
            .packet_id;
        let packet =
            encode_packet(packet_id, &encode_sound_payload(7, 0.5, 1.25, -0.75), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SoundRequested {
                sound_id: Some(7),
                volume: 0.5,
                pitch: 1.25,
                pan: -0.75,
            }
        );
        assert_eq!(session.state().received_sound_count, 1);
        assert_eq!(session.state().last_sound_id, Some(7));
        assert_eq!(
            session.state().last_sound_volume_bits,
            Some(0.5f32.to_bits())
        );
        assert_eq!(
            session.state().last_sound_pitch_bits,
            Some(1.25f32.to_bits())
        );
        assert_eq!(
            session.state().last_sound_pan_bits,
            Some((-0.75f32).to_bits())
        );
    }

    #[test]
    fn sound_at_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "soundAt" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_sound_at_payload(11, 64.0, 96.0, 0.8, 1.1),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::SoundAtRequested {
                sound_id: Some(11),
                x: 64.0,
                y: 96.0,
                volume: 0.8,
                pitch: 1.1,
            }
        );
        assert_eq!(session.state().received_sound_at_count, 1);
        assert_eq!(session.state().last_sound_at_id, Some(11));
        assert_eq!(
            session.state().last_sound_at_x_bits,
            Some(64.0f32.to_bits())
        );
        assert_eq!(
            session.state().last_sound_at_y_bits,
            Some(96.0f32.to_bits())
        );
        assert_eq!(
            session.state().last_sound_at_volume_bits,
            Some(0.8f32.to_bits())
        );
        assert_eq!(
            session.state().last_sound_at_pitch_bits,
            Some(1.1f32.to_bits())
        );
    }

    #[test]
    fn sound_at_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "soundAt" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_sound_at_payload(11, 64.0, 96.0, 0.8, 1.1)[..17],
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_sound_at_count, 0);
        assert_eq!(session.state().last_sound_at_id, None);
        assert_eq!(session.state().failed_sound_at_parse_count, 1);
        assert_eq!(
            session.state().last_sound_at_parse_error_payload_len,
            Some(17)
        );
    }

    #[test]
    fn sound_packet_with_truncated_payload_is_ignored_and_tracks_parse_failure() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sound" && entry.params.len() == 4)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_sound_payload(7, 0.5, 1.25, -0.75)[..9],
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_sound_count, 0);
        assert_eq!(session.state().last_sound_id, None);
        assert_eq!(session.state().failed_sound_parse_count, 1);
        assert_eq!(session.state().last_sound_parse_error_payload_len, Some(9));
    }

    #[test]
    fn effect_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: None,
            }
        );
        assert_eq!(session.state().received_effect_count, 1);
        assert_eq!(session.state().last_effect_id, Some(13));
        assert_eq!(session.state().last_effect_x_bits, Some(32.5f32.to_bits()));
        assert_eq!(session.state().last_effect_y_bits, Some(48.0f32.to_bits()));
        assert_eq!(
            session.state().last_effect_rotation_bits,
            Some(90.0f32.to_bits())
        );
        assert_eq!(session.state().last_effect_color_rgba, Some(0x11223344));
        assert_eq!(session.state().last_effect_data_len, None);
        assert_eq!(session.state().last_effect_data_type_tag, None);
        assert_eq!(session.state().last_effect_data_kind, None);
        assert_eq!(session.state().last_effect_data_consumed_len, None);
        assert_eq!(session.state().last_effect_data_object, None);
        assert_eq!(session.state().last_effect_data_semantic, None);
        assert!(!session.state().last_effect_data_parse_failed);
        assert_eq!(session.state().failed_effect_data_parse_count, 0);
        assert_eq!(session.state().last_effect_data_parse_error, None);
    }

    #[test]
    fn effect_reliable_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effectReliable" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_effect_payload(21, -5.0, 6.5, 180.0, 0xaabbccdd),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectReliableRequested {
                effect_id: Some(21),
                x: -5.0,
                y: 6.5,
                rotation: 180.0,
                color_rgba: 0xaabbccdd,
            }
        );
        assert_eq!(session.state().received_effect_reliable_count, 1);
        assert_eq!(session.state().last_effect_reliable_id, Some(21));
        assert_eq!(
            session.state().last_effect_reliable_x_bits,
            Some((-5.0f32).to_bits())
        );
        assert_eq!(
            session.state().last_effect_reliable_y_bits,
            Some(6.5f32.to_bits())
        );
        assert_eq!(
            session.state().last_effect_reliable_rotation_bits,
            Some(180.0f32.to_bits())
        );
        assert_eq!(
            session.state().last_effect_reliable_color_rgba,
            Some(0xaabbccdd)
        );
    }

    #[test]
    fn effect_packet_with_object_data_payload_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        payload.push(4);
        payload.extend_from_slice(&encode_typeio_string_payload("spark"));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: Some(TypeIoObject::String(Some("spark".to_string()))),
            }
        );
        assert_eq!(session.state().received_effect_count, 1);
        assert_eq!(session.state().last_effect_id, Some(13));
        assert_eq!(session.state().last_effect_x_bits, Some(32.5f32.to_bits()));
        assert_eq!(session.state().last_effect_y_bits, Some(48.0f32.to_bits()));
        assert_eq!(
            session.state().last_effect_rotation_bits,
            Some(90.0f32.to_bits())
        );
        assert_eq!(session.state().last_effect_color_rgba, Some(0x11223344));
        assert_eq!(session.state().last_effect_data_len, Some(9));
        assert_eq!(session.state().last_effect_data_type_tag, Some(4));
        assert_eq!(
            session.state().last_effect_data_kind.as_deref(),
            Some("string")
        );
        assert_eq!(session.state().last_effect_data_consumed_len, Some(9));
        assert_eq!(
            session.state().last_effect_data_object,
            Some(TypeIoObject::String(Some("spark".to_string())))
        );
        assert_eq!(
            session.state().last_effect_data_semantic,
            Some(EffectDataSemantic::String(Some("spark".to_string())))
        );
        assert_eq!(session.state().last_effect_business_projection, None);
        assert!(!session.state().last_effect_data_parse_failed);
        assert_eq!(session.state().failed_effect_data_parse_count, 0);
        assert_eq!(session.state().last_effect_data_parse_error, None);
    }

    #[test]
    fn effect_packet_with_int_data_payload_projects_numeric_semantics() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(&mut payload, &TypeIoObject::Int(7));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: Some(TypeIoObject::Int(7)),
            }
        );
        assert_eq!(session.state().last_effect_data_type_tag, Some(1));
        assert_eq!(
            session.state().last_effect_data_kind.as_deref(),
            Some("int")
        );
        assert_eq!(
            session.state().last_effect_data_object,
            Some(TypeIoObject::Int(7))
        );
        assert_eq!(
            session.state().last_effect_data_semantic,
            Some(EffectDataSemantic::Int(7))
        );
        assert_eq!(session.state().last_effect_business_projection, None);
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_point2_data_payload_projects_spatial_semantics() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(&mut payload, &TypeIoObject::Point2 { x: 3, y: 4 });
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: Some(TypeIoObject::Point2 { x: 3, y: 4 }),
            }
        );
        assert_eq!(session.state().last_effect_data_type_tag, Some(7));
        assert_eq!(
            session.state().last_effect_data_kind.as_deref(),
            Some("Point2")
        );
        assert_eq!(
            session.state().last_effect_data_object,
            Some(TypeIoObject::Point2 { x: 3, y: 4 })
        );
        assert_eq!(
            session.state().last_effect_data_semantic,
            Some(EffectDataSemantic::Point2 { x: 3, y: 4 })
        );
        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Point2,
                x_bits: 24.0f32.to_bits(),
                y_bits: 32.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, None);
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_business_projection_data_payloads_updates_runtime_apply_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;

        let cases = vec![
            (
                TypeIoObject::ContentRaw {
                    content_type: 2,
                    content_id: 0x0123,
                },
                Some(EffectBusinessProjection::ContentRef {
                    kind: EffectBusinessContentKind::Content,
                    content_type: 2,
                    content_id: 0x0123,
                }),
                None,
            ),
            (
                TypeIoObject::TechNodeRaw {
                    content_type: 4,
                    content_id: 0x0102,
                },
                Some(EffectBusinessProjection::ContentRef {
                    kind: EffectBusinessContentKind::TechNode,
                    content_type: 4,
                    content_id: 0x0102,
                }),
                None,
            ),
            (
                TypeIoObject::Float(12.5),
                Some(EffectBusinessProjection::FloatValue(12.5f32.to_bits())),
                None,
            ),
            (
                TypeIoObject::BuildingPos(pack_point2(7, 11)),
                Some(EffectBusinessProjection::ParentRef {
                    source: EffectBusinessPositionSource::BuildingPos,
                    value: pack_point2(7, 11),
                    x_bits: 56.0f32.to_bits(),
                    y_bits: 88.0f32.to_bits(),
                }),
                None,
            ),
            (
                TypeIoObject::Vec2 { x: 12.5, y: -3.0 },
                Some(EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Vec2,
                    x_bits: 12.5f32.to_bits(),
                    y_bits: (-3.0f32).to_bits(),
                }),
                None,
            ),
            (
                TypeIoObject::PackedPoint2Array(vec![pack_point2(9, 6), pack_point2(1, 2)]),
                Some(EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Point2,
                    x_bits: 72.0f32.to_bits(),
                    y_bits: 48.0f32.to_bits(),
                }),
                Some(vec![0]),
            ),
            (
                TypeIoObject::Vec2Array(vec![(5.5, -7.25), (9.0, 11.0)]),
                Some(EffectBusinessProjection::WorldPosition {
                    source: EffectBusinessPositionSource::Vec2,
                    x_bits: 5.5f32.to_bits(),
                    y_bits: (-7.25f32).to_bits(),
                }),
                Some(vec![0]),
            ),
        ];

        for (object, projection, path) in cases {
            let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
            let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
            write_typeio_object(&mut payload, &object);
            let packet = encode_packet(packet_id, &payload, false).unwrap();

            session.ingest_packet_bytes(&packet).unwrap();

            assert_eq!(session.state().last_effect_data_object, Some(object));
            assert_eq!(session.state().last_effect_business_projection, projection);
            assert_eq!(session.state().last_effect_business_path, path);
            assert!(!session.state().last_effect_data_parse_failed);
        }
    }

    #[test]
    fn effect_packet_with_local_unit_id_data_payload_projects_runtime_apply_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let input = session.snapshot_input_mut();
        input.unit_id = Some(77);
        input.position = Some((64.0, 72.0));
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(&mut payload, &TypeIoObject::UnitId(77));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::LocalUnitId,
                value: 77,
                x_bits: 64.0f32.to_bits(),
                y_bits: 72.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, None);
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_entity_unit_id_data_payload_projects_runtime_apply_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        session.state.entity_table_projection.by_entity_id.insert(
            1234,
            crate::session_state::EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 99,
                x_bits: 128.0f32.to_bits(),
                y_bits: 256.0f32.to_bits(),
                last_seen_entity_snapshot_count: 7,
            },
        );
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(&mut payload, &TypeIoObject::UnitId(1234));
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::EntityUnitId,
                value: 1234,
                x_bits: 128.0f32.to_bits(),
                y_bits: 256.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, None);
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_object_array_data_payload_projects_nested_runtime_apply_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(
            &mut payload,
            &TypeIoObject::ObjectArray(vec![
                TypeIoObject::Int(7),
                TypeIoObject::Point2 { x: 10, y: 20 },
                TypeIoObject::Bool(true),
            ]),
        );
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_data_kind.as_deref(),
            Some("object[len=3]{0=int,1=Point2,2=bool}")
        );
        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Point2,
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, Some(vec![1]));
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_nested_object_array_data_payload_reports_structured_kind_and_path() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(
            &mut payload,
            &TypeIoObject::ObjectArray(vec![
                TypeIoObject::Int(7),
                TypeIoObject::ObjectArray(vec![
                    TypeIoObject::Point2 { x: 10, y: 20 },
                    TypeIoObject::Bool(true),
                ]),
                TypeIoObject::Bool(false),
            ]),
        );
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_data_kind.as_deref(),
            Some("object[len=3]{0=int,1=object[len=2]{0=Point2,1=bool},2=bool}")
        );
        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Point2,
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, Some(vec![1, 0]));
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_depth_three_object_array_data_payload_projects_runtime_apply_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(
            &mut payload,
            &TypeIoObject::ObjectArray(vec![TypeIoObject::ObjectArray(vec![
                TypeIoObject::ObjectArray(vec![TypeIoObject::Point2 { x: 10, y: 20 }]),
            ])]),
        );
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Point2,
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
            })
        );
        assert_eq!(
            session.state().last_effect_business_path,
            Some(vec![0, 0, 0])
        );
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_unresolved_unit_id_falls_through_to_later_point2_projection() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(
            &mut payload,
            &TypeIoObject::ObjectArray(vec![
                TypeIoObject::UnitId(9_999),
                TypeIoObject::Point2 { x: 10, y: 20 },
                TypeIoObject::Bool(false),
            ]),
        );
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Point2,
                x_bits: 80.0f32.to_bits(),
                y_bits: 160.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, Some(vec![1]));
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_object_array_nested_unit_id_projects_parent_ref() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        session.state.entity_table_projection.by_entity_id.insert(
            4321,
            crate::session_state::EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 88,
                x_bits: 96.0f32.to_bits(),
                y_bits: 104.0f32.to_bits(),
                last_seen_entity_snapshot_count: 3,
            },
        );
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(
            &mut payload,
            &TypeIoObject::ObjectArray(vec![
                TypeIoObject::Int(7),
                TypeIoObject::UnitId(4321),
                TypeIoObject::Bool(true),
            ]),
        );
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::ParentRef {
                source: EffectBusinessPositionSource::EntityUnitId,
                value: 4321,
                x_bits: 96.0f32.to_bits(),
                y_bits: 104.0f32.to_bits(),
            })
        );
        assert_eq!(session.state().last_effect_business_path, Some(vec![1]));
        assert!(!session.state().last_effect_data_parse_failed);
    }

    #[test]
    fn effect_packet_with_extended_data_types_projects_semantics() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;

        let cases = vec![
            (
                TypeIoObject::TechNodeRaw {
                    content_type: 1,
                    content_id: 0x0101,
                },
                EffectDataSemantic::TechNodeRaw {
                    content_type: 1,
                    content_id: 0x0101,
                },
            ),
            (
                TypeIoObject::Double(12.5),
                EffectDataSemantic::DoubleBits(12.5f64.to_bits()),
            ),
            (
                TypeIoObject::BuildingPos(0x0001_0002),
                EffectDataSemantic::BuildingPos(0x0001_0002),
            ),
            (
                TypeIoObject::LegacyUnitCommandNull(0xab),
                EffectDataSemantic::LegacyUnitCommandNull(0xab),
            ),
            (
                TypeIoObject::BoolArray(vec![true, false, true]),
                EffectDataSemantic::BoolArrayLen(3),
            ),
            (
                TypeIoObject::UnitId(0x0102_0304),
                EffectDataSemantic::UnitId(0x0102_0304),
            ),
            (
                TypeIoObject::Vec2Array(vec![(1.0, 2.0), (3.0, 4.0)]),
                EffectDataSemantic::Vec2ArrayLen(2),
            ),
        ];

        for (object, semantic) in cases {
            let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
            let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
            write_typeio_object(&mut payload, &object);
            let packet = encode_packet(packet_id, &payload, false).unwrap();

            let event = session.ingest_packet_bytes(&packet).unwrap();

            assert_eq!(
                event,
                ClientSessionEvent::EffectRequested {
                    effect_id: Some(13),
                    x: 32.5,
                    y: 48.0,
                    rotation: 90.0,
                    color_rgba: 0x11223344,
                    data_object: Some(object.clone()),
                }
            );
            assert_eq!(
                session.state().last_effect_data_type_tag,
                payload.get(18).copied()
            );
            assert_eq!(
                session.state().last_effect_data_kind.as_deref(),
                Some(object.kind())
            );
            assert_eq!(session.state().last_effect_data_object, Some(object));
            assert_eq!(session.state().last_effect_data_semantic, Some(semantic));
            assert!(!session.state().last_effect_data_parse_failed);
        }
    }

    #[test]
    fn effect_packet_with_unsupported_data_type_marks_parse_failed_without_packet_drop() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        payload.push(0x7f);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: None,
            }
        );
        assert_eq!(session.state().received_effect_count, 1);
        assert_eq!(session.state().last_effect_data_len, Some(1));
        assert_eq!(session.state().last_effect_data_type_tag, Some(0x7f));
        assert_eq!(session.state().last_effect_data_kind, None);
        assert_eq!(session.state().last_effect_data_consumed_len, None);
        assert_eq!(session.state().last_effect_data_object, None);
        assert_eq!(
            session.state().last_effect_data_semantic,
            Some(EffectDataSemantic::OpaqueTypeTag(0x7f))
        );
        assert!(session.state().last_effect_data_parse_failed);
        assert_eq!(session.state().failed_effect_data_parse_count, 1);
        assert!(session
            .state()
            .last_effect_data_parse_error
            .as_deref()
            .unwrap_or_default()
            .contains("failed to parse effect data object"));
    }

    #[test]
    fn effect_packet_with_trailing_data_bytes_marks_parse_failed_without_packet_drop() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        payload.push(4);
        payload.extend_from_slice(&encode_typeio_string_payload("spark"));
        payload.push(0xaa);
        let packet = encode_packet(packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::EffectRequested {
                effect_id: Some(13),
                x: 32.5,
                y: 48.0,
                rotation: 90.0,
                color_rgba: 0x11223344,
                data_object: Some(TypeIoObject::String(Some("spark".to_string()))),
            }
        );
        assert_eq!(session.state().received_effect_count, 1);
        assert_eq!(session.state().last_effect_data_len, Some(10));
        assert_eq!(session.state().last_effect_data_type_tag, Some(4));
        assert_eq!(
            session.state().last_effect_data_kind.as_deref(),
            Some("string")
        );
        assert_eq!(session.state().last_effect_data_consumed_len, Some(9));
        assert_eq!(
            session.state().last_effect_data_object,
            Some(TypeIoObject::String(Some("spark".to_string())))
        );
        assert_eq!(
            session.state().last_effect_data_semantic,
            Some(EffectDataSemantic::String(Some("spark".to_string())))
        );
        assert!(session.state().last_effect_data_parse_failed);
        assert_eq!(session.state().failed_effect_data_parse_count, 1);
        assert!(session
            .state()
            .last_effect_data_parse_error
            .as_deref()
            .unwrap_or_default()
            .contains("trailing bytes after effect data object"));
    }

    #[test]
    fn effect_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 5)
            .unwrap()
            .packet_id;
        let payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        let packet = encode_packet(packet_id, &payload[..17], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_effect_count, 0);
        assert_eq!(session.state().last_effect_id, None);
    }

    #[test]
    fn trace_info_packet_emits_event_and_updates_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "traceInfo")
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_trace_info_payload(
                123456,
                Some("127.0.0.1"),
                Some("uuid-golden"),
                Some("en_US"),
                true,
                false,
                7,
                2,
                &["10.0.0.1", "10.0.0.2"],
                &["alice", "bob"],
            ),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::TraceInfoReceived {
                player_id: Some(123456),
                ip: Some("127.0.0.1".to_string()),
                uuid: Some("uuid-golden".to_string()),
                locale: Some("en_US".to_string()),
                modded: true,
                mobile: false,
                times_joined: 7,
                times_kicked: 2,
                ips: vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()],
                names: vec!["alice".to_string(), "bob".to_string()],
            }
        );
        assert_eq!(session.state().received_trace_info_count, 1);
        assert_eq!(session.state().last_trace_info_player_id, Some(123456));
        assert_eq!(
            session.state().last_trace_info_ip.as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(
            session.state().last_trace_info_uuid.as_deref(),
            Some("uuid-golden")
        );
        assert_eq!(
            session.state().last_trace_info_locale.as_deref(),
            Some("en_US")
        );
        assert_eq!(session.state().last_trace_info_modded, Some(true));
        assert_eq!(session.state().last_trace_info_mobile, Some(false));
        assert_eq!(session.state().last_trace_info_times_joined, Some(7));
        assert_eq!(session.state().last_trace_info_times_kicked, Some(2));
        assert_eq!(
            session.state().last_trace_info_ips,
            Some(vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()])
        );
        assert_eq!(
            session.state().last_trace_info_names,
            Some(vec!["alice".to_string(), "bob".to_string()])
        );
    }

    #[test]
    fn trace_info_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "traceInfo")
            .unwrap()
            .packet_id;
        let payload = encode_trace_info_payload(
            123456,
            Some("127.0.0.1"),
            Some("uuid-golden"),
            Some("en_US"),
            true,
            false,
            7,
            2,
            &["10.0.0.1"],
            &["alice"],
        );
        let packet = encode_packet(packet_id, &payload[..payload.len() - 1], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_trace_info_count, 0);
        assert_eq!(session.state().last_trace_info_player_id, None);
        assert_eq!(session.state().failed_trace_info_parse_count, 1);
        assert_eq!(
            session.state().last_trace_info_parse_error_payload_len,
            Some(payload.len() - 1)
        );
    }

    #[test]
    fn debug_status_packets_emit_events_and_update_state() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let reliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "debugStatusClient")
            .unwrap()
            .packet_id;
        let unreliable_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "debugStatusClientUnreliable")
            .unwrap()
            .packet_id;

        let reliable_packet = encode_packet(
            reliable_packet_id,
            &encode_debug_status_payload(7, 101, 303),
            false,
        )
        .unwrap();
        let reliable_event = session.ingest_packet_bytes(&reliable_packet).unwrap();
        assert_eq!(
            reliable_event,
            ClientSessionEvent::DebugStatusReceived {
                reliable: true,
                value: 7,
                last_client_snapshot: 101,
                snapshots_sent: 303,
            }
        );
        assert_eq!(session.state().received_debug_status_client_count, 1);
        assert_eq!(
            session
                .state()
                .received_debug_status_client_unreliable_count,
            0
        );
        assert_eq!(session.state().last_debug_status_reliable, Some(true));
        assert_eq!(session.state().last_debug_status_value, Some(7));
        assert_eq!(
            session.state().last_debug_status_last_client_snapshot,
            Some(101)
        );
        assert_eq!(session.state().last_debug_status_snapshots_sent, Some(303));

        let unreliable_packet = encode_packet(
            unreliable_packet_id,
            &encode_debug_status_payload(12, 202, 404),
            false,
        )
        .unwrap();
        let unreliable_event = session.ingest_packet_bytes(&unreliable_packet).unwrap();
        assert_eq!(
            unreliable_event,
            ClientSessionEvent::DebugStatusReceived {
                reliable: false,
                value: 12,
                last_client_snapshot: 202,
                snapshots_sent: 404,
            }
        );
        assert_eq!(session.state().received_debug_status_client_count, 1);
        assert_eq!(
            session
                .state()
                .received_debug_status_client_unreliable_count,
            1
        );
        assert_eq!(session.state().last_debug_status_reliable, Some(false));
        assert_eq!(session.state().last_debug_status_value, Some(12));
        assert_eq!(
            session.state().last_debug_status_last_client_snapshot,
            Some(202)
        );
        assert_eq!(session.state().last_debug_status_snapshots_sent, Some(404));
    }

    #[test]
    fn debug_status_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "debugStatusClient")
            .unwrap()
            .packet_id;
        let payload = encode_debug_status_payload(7, 101, 303);
        let packet = encode_packet(packet_id, &payload[..payload.len() - 1], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(session.state().received_debug_status_client_count, 0);
        assert_eq!(
            session
                .state()
                .received_debug_status_client_unreliable_count,
            0
        );
        assert_eq!(session.state().last_debug_status_reliable, None);
        assert_eq!(session.state().last_debug_status_value, None);
        assert_eq!(session.state().last_debug_status_last_client_snapshot, None);
        assert_eq!(session.state().last_debug_status_snapshots_sent, None);
        assert_eq!(session.state().failed_debug_status_client_parse_count, 1);
        assert_eq!(
            session
                .state()
                .last_debug_status_client_parse_error_payload_len,
            Some(payload.len() - 1)
        );
    }

    #[test]
    fn debug_status_unreliable_packet_with_truncated_payload_is_ignored() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "debugStatusClientUnreliable")
            .unwrap()
            .packet_id;
        let payload = encode_debug_status_payload(12, 202, 404);
        let packet = encode_packet(packet_id, &payload[..payload.len() - 1], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert!(matches!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id: ignored_id,
                ..
            } if ignored_id == packet_id
        ));
        assert_eq!(
            session
                .state()
                .received_debug_status_client_unreliable_count,
            0
        );
        assert_eq!(
            session
                .state()
                .failed_debug_status_client_unreliable_parse_count,
            1
        );
        assert_eq!(
            session
                .state()
                .last_debug_status_client_unreliable_parse_error_payload_len,
            Some(payload.len() - 1)
        );
    }

    #[test]
    fn player_disconnect_packet_emits_event_and_clears_local_player_sync() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let local_player_id = session.state().world_player_id.unwrap();
        let entity_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::EntitySnapshot.method_name())
            .unwrap()
            .packet_id;
        let entity_packet = encode_packet(
            entity_packet_id,
            &sample_snapshot_packet("entitySnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&entity_packet).unwrap();
        assert!(session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&local_player_id));
        assert!(session.state().world_player_unit_value.is_some());
        assert!(session.state().world_player_x_bits.is_some());
        assert!(session.state().world_player_y_bits.is_some());
        assert!(session.snapshot_input().position.is_some());

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "playerDisconnect")
            .unwrap()
            .packet_id;
        let packet = encode_packet(packet_id, &local_player_id.to_be_bytes(), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::PlayerDisconnected {
                player_id: local_player_id,
                cleared_local_player_sync: true,
            }
        );
        assert_eq!(session.state().world_player_unit_kind, None);
        assert_eq!(session.state().world_player_unit_value, None);
        assert_eq!(session.state().world_player_x_bits, None);
        assert_eq!(session.state().world_player_y_bits, None);
        let input = session.snapshot_input();
        assert_eq!(input.unit_id, None);
        assert!(input.dead);
        assert_eq!(input.position, None);
        assert_eq!(input.view_center, None);
        assert_eq!(
            session
                .state()
                .entity_table_projection
                .local_player_entity_id,
            None
        );
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&local_player_id));
        assert!(session
            .state()
            .entity_snapshot_tombstones
            .contains_key(&local_player_id));
    }

    #[test]
    fn player_disconnect_packet_for_other_player_keeps_local_sync() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in chunk_packets {
            session.ingest_packet_bytes(&chunk).unwrap();
        }

        let local_player_id = session.state().world_player_id.unwrap();
        session.state.entity_table_projection.by_entity_id.insert(
            local_player_id + 1000,
            crate::session_state::EntityProjection {
                class_id: 33,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "playerDisconnect")
            .unwrap()
            .packet_id;
        let other_player_id = local_player_id + 1000;
        let packet = encode_packet(packet_id, &other_player_id.to_be_bytes(), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::PlayerDisconnected {
                player_id: other_player_id,
                cleared_local_player_sync: false,
            }
        );
        assert!(session.state().world_player_unit_value.is_some());
        assert!(session.state().world_player_x_bits.is_some());
        assert!(session.state().world_player_y_bits.is_some());
        assert!(session.snapshot_input().position.is_some());
        assert!(!session
            .state()
            .entity_table_projection
            .by_entity_id
            .contains_key(&other_player_id));
    }

    #[test]
    fn reports_timeout_when_inbound_packets_stall() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 1000,
            client_snapshot_interval_ms: 1000,
            connect_timeout_ms: 1200,
            timeout_ms: 1200,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, _) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();

        let actions = session.advance_time(1_201).unwrap();
        assert_eq!(
            actions,
            vec![ClientSessionAction::TimedOut { idle_ms: 1_201 }]
        );
        assert!(session.state().connection_timed_out);
    }

    #[test]
    fn ready_session_uses_snapshot_timeout_after_connect_confirm() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
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
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        assert!(session.state().connect_confirm_sent);

        let actions = session.advance_time(1_201).unwrap();

        assert_eq!(
            actions,
            vec![ClientSessionAction::TimedOut { idle_ms: 1_201 }]
        );
        assert!(session.state().connection_timed_out);
    }

    #[test]
    fn ready_state_inbound_liveness_anchor_counts_without_extending_snapshot_timeout() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 60_000,
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
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        assert!(session.state().connect_confirm_sent);
        assert_eq!(session.state().ready_inbound_liveness_anchor_count, 0);
        assert_eq!(
            session.state().last_ready_inbound_liveness_anchor_at_ms,
            None
        );

        session.advance_time(1_000).unwrap();
        let state_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let state_packet = encode_packet(
            state_packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();
        let event = session.ingest_packet_bytes(&state_packet).unwrap();
        assert_eq!(
            event,
            ClientSessionEvent::SnapshotReceived(HighFrequencyRemoteMethod::StateSnapshot)
        );
        assert_eq!(session.state().ready_inbound_liveness_anchor_count, 1);
        assert_eq!(
            session.state().last_ready_inbound_liveness_anchor_at_ms,
            Some(1_000)
        );

        let actions = session.advance_time(1_201).unwrap();
        assert_eq!(
            actions,
            vec![ClientSessionAction::TimedOut { idle_ms: 1_201 }]
        );
        assert!(session.state().connection_timed_out);
    }

    #[test]
    fn timed_out_session_ignores_later_inbound_packets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 1_000,
            client_snapshot_interval_ms: 1_000,
            connect_timeout_ms: 1_200,
            timeout_ms: 1_200,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, _) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        assert_eq!(
            session.advance_time(1_201).unwrap(),
            vec![ClientSessionAction::TimedOut { idle_ms: 1_201 }]
        );

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_typeio_string_payload("[accent]late"),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "sendMessage".to_string(),
                    packet_class: "mindustry.gen.SendMessageCallPacket".to_string(),
                }),
            }
        );
        assert!(session.state().connection_timed_out);
        assert_eq!(session.state().received_server_message_count, 0);
        assert_eq!(session.state().last_server_message, None);
    }

    #[test]
    fn default_timing_tracks_java_snapshot_and_timeout_baseline() {
        let timing = ClientSessionTiming::default();

        assert_eq!(timing.keepalive_interval_ms, 1_000);
        assert_eq!(timing.client_snapshot_interval_ms, 67);
        assert_eq!(timing.connect_timeout_ms, 1_800_000);
        assert_eq!(timing.timeout_ms, 20_000);
    }

    #[test]
    fn counts_ignored_packets_once() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, _) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let ignored_packet = encode_packet(
            packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        let event = session.ingest_packet_bytes(&ignored_packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "stateSnapshot".to_string(),
                    packet_class: "mindustry.gen.StateSnapshotCallPacket".to_string(),
                }),
            }
        );
        assert_eq!(session.stats().packets_seen, 0);
        assert_eq!(session.stats().snapshot_packets_seen, 0);
        assert_eq!(session.state().received_snapshot_count, 0);
    }

    #[test]
    fn kick_string_packet_marks_session_kicked_and_stops_actions() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let kick_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "kick"
                    && entry.params.len() == 1
                    && entry.params[0].java_type == "java.lang.String"
            })
            .unwrap()
            .packet_id;
        let packet =
            encode_packet(kick_packet_id, &encode_typeio_string_payload("bye"), false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::Kicked {
                reason_text: Some("bye".to_string()),
                reason_ordinal: None,
                duration_ms: None,
            }
        );
        assert!(session.kicked());
        assert_eq!(session.last_kick_reason_text(), Some("bye"));
        assert_eq!(session.last_kick_reason_ordinal(), None);
        assert_eq!(session.last_kick_duration_ms(), None);
        assert_eq!(session.last_kick_hint_category(), None);
        assert_eq!(session.last_kick_hint_text(), None);
        let actions = session.advance_time(1_000).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn kick_reason_packet_marks_session_kicked_and_decodes_payload() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let kick_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "kick"
                    && !entry.params.is_empty()
                    && entry.params[0].java_type.contains("KickReason")
            })
            .unwrap()
            .packet_id;
        let mut payload = Vec::new();
        payload.extend_from_slice(&7i32.to_be_bytes());
        payload.extend_from_slice(&30_000i64.to_be_bytes());
        let packet = encode_packet(kick_packet_id, &payload, false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::Kicked {
                reason_text: Some("idInUse".to_string()),
                reason_ordinal: Some(7),
                duration_ms: Some(30_000),
            }
        );
        assert!(session.kicked());
        assert_eq!(session.last_kick_reason_text(), Some("idInUse"));
        assert_eq!(session.last_kick_reason_ordinal(), Some(7));
        assert_eq!(session.last_kick_duration_ms(), Some(30_000));
        assert_eq!(
            session.last_kick_hint_category(),
            Some(KickReasonHintCategory::IdInUse)
        );
        assert_eq!(
            session.last_kick_hint_text(),
            Some(
                "uuid or usid is already in use; wait for the old session to clear or regenerate the connect identity.",
            )
        );
        let actions = session.advance_time(1_000).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn kick_reason_packet_decodes_u8_server_restarting_ordinal() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let kick_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "kick"
                    && !entry.params.is_empty()
                    && entry.params[0].java_type.contains("KickReason")
            })
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            kick_packet_id,
            &[KICK_REASON_SERVER_RESTARTING_ORDINAL as u8],
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::Kicked {
                reason_text: Some("serverRestarting".to_string()),
                reason_ordinal: Some(KICK_REASON_SERVER_RESTARTING_ORDINAL),
                duration_ms: None,
            }
        );
        assert!(session.kicked());
        assert_eq!(session.last_kick_reason_text(), Some("serverRestarting"));
        assert_eq!(
            session.last_kick_reason_ordinal(),
            Some(KICK_REASON_SERVER_RESTARTING_ORDINAL)
        );
        assert_eq!(
            session.last_kick_hint_category(),
            Some(KickReasonHintCategory::ServerRestarting)
        );
        assert_eq!(
            session.last_kick_hint_text(),
            Some("server is restarting; retry connection shortly.")
        );
    }

    #[test]
    fn kick_reason_name_mapping_covers_java_handshake_taxonomy_ordinals() {
        assert_eq!(kick_reason_name_from_ordinal(1), Some("clientOutdated"));
        assert_eq!(kick_reason_name_from_ordinal(2), Some("serverOutdated"));
        assert_eq!(kick_reason_name_from_ordinal(9), Some("customClient"));
        assert_eq!(kick_reason_name_from_ordinal(12), Some("typeMismatch"));
        assert_eq!(kick_reason_name_from_ordinal(KICK_REASON_SERVER_RESTARTING_ORDINAL), Some("serverRestarting"));
        assert_eq!(kick_reason_name_from_ordinal(-1), None);
        assert_eq!(kick_reason_name_from_ordinal(99), None);
    }

    #[test]
    fn kick_reason_hint_mapping_covers_high_signal_taxonomy() {
        assert_eq!(
            kick_reason_hint_from(Some("banned"), None),
            Some((
                KickReasonHintCategory::Banned,
                "server reports this identity or name is banned; use a different account or ask the server admin to review the ban.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("clientOutdated"), None),
            Some((
                KickReasonHintCategory::ClientOutdated,
                "client build is outdated; upgrade this client to the server version.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("recentKick"), None),
            Some((
                KickReasonHintCategory::RecentKick,
                "server still remembers a recent kick; wait for the cooldown to expire before reconnecting.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("nameInUse"), None),
            Some((
                KickReasonHintCategory::NameInUse,
                "player name is already in use; retry with a different --name value.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("idInUse"), None),
            Some((
                KickReasonHintCategory::IdInUse,
                "uuid or usid is already in use; wait for the old session to clear or regenerate the connect identity.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("nameEmpty"), None),
            Some((
                KickReasonHintCategory::NameEmpty,
                "player name is empty or invalid; set --name to a non-empty value accepted by the server.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("serverOutdated"), None),
            Some((
                KickReasonHintCategory::ServerOutdated,
                "server build is older than this client; use a matching server or older client build.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("customClient"), None),
            Some((
                KickReasonHintCategory::CustomClientRejected,
                "server rejected custom clients; connect to a server that allows custom clients.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("typeMismatch"), None),
            Some((
                KickReasonHintCategory::TypeMismatch,
                "version type/protocol mismatch; align client/server version type and mod set.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("whitelist"), None),
            Some((
                KickReasonHintCategory::WhitelistRequired,
                "server requires whitelist access; ask the server admin to whitelist this identity.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(Some("playerLimit"), None),
            Some((
                KickReasonHintCategory::PlayerLimit,
                "server is full; wait for an open slot or use an identity with reserved access.",
            ))
        );
        assert_eq!(
            kick_reason_hint_from(None, Some(KICK_REASON_SERVER_RESTARTING_ORDINAL)),
            Some((
                KickReasonHintCategory::ServerRestarting,
                "server is restarting; retry connection shortly.",
            ))
        );
        assert_eq!(kick_reason_hint_from(Some("gameover"), Some(4)), None);
    }

    #[test]
    fn kicked_session_ignores_later_inbound_packets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let kick_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| {
                entry.method == "kick"
                    && entry.params.len() == 1
                    && entry.params[0].java_type == "java.lang.String"
            })
            .unwrap()
            .packet_id;
        let kick_packet =
            encode_packet(kick_packet_id, &encode_typeio_string_payload("bye"), false).unwrap();
        session.ingest_packet_bytes(&kick_packet).unwrap();

        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_typeio_string_payload("[accent]late"),
            false,
        )
        .unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "sendMessage".to_string(),
                    packet_class: "mindustry.gen.SendMessageCallPacket".to_string(),
                }),
            }
        );
        assert!(session.kicked());
        assert_eq!(session.state().received_server_message_count, 0);
        assert_eq!(session.state().last_server_message, None);
    }

    #[test]
    fn handles_remote_ping_and_ping_response_packets() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let timing = ClientSessionTiming {
            keepalive_interval_ms: 60_000,
            client_snapshot_interval_ms: 60_000,
            connect_timeout_ms: 10_000,
            timeout_ms: 10_000,
        };
        let mut session =
            ClientSession::from_remote_manifest_with_timing(&manifest, "fr", timing).unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }
        session.prepare_connect_confirm_packet().unwrap();
        session.advance_time(1_000).unwrap();

        let ping_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "ping")
            .unwrap()
            .packet_id;
        let ping_response_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "pingResponse")
            .unwrap()
            .packet_id;

        let ping_packet = encode_packet(ping_packet_id, &1_234i64.to_be_bytes(), false).unwrap();
        let ping_event = session.ingest_packet_bytes(&ping_packet).unwrap();
        assert_eq!(
            ping_event,
            ClientSessionEvent::Ping {
                sent_at_ms: Some(1_234),
                response_queued: true,
            }
        );

        let actions = session.advance_time(1_100).unwrap();
        let ping_response = actions.iter().find_map(|action| match action {
            ClientSessionAction::SendPacket {
                packet_id,
                transport,
                bytes,
            } if *packet_id == ping_response_packet_id
                && *transport == ClientPacketTransport::Tcp =>
            {
                Some(bytes)
            }
            _ => None,
        });
        let decoded_ping_response =
            decode_packet(ping_response.expect("expected queued pingResponse packet")).unwrap();
        assert_eq!(decoded_ping_response.packet_id, ping_response_packet_id);
        assert_eq!(decoded_ping_response.payload, 1_234i64.to_be_bytes());

        let ping_response_packet =
            encode_packet(ping_response_packet_id, &900i64.to_be_bytes(), false).unwrap();
        let ping_response_event = session.ingest_packet_bytes(&ping_response_packet).unwrap();
        assert_eq!(
            ping_response_event,
            ClientSessionEvent::PingResponse {
                sent_at_ms: Some(900),
                round_trip_ms: Some(200),
            }
        );
        assert_eq!(session.last_remote_ping_rtt_ms(), Some(200));
    }

    #[test]
    fn invalid_ping_payload_still_emits_ping_event_without_response() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let ping_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "ping")
            .unwrap()
            .packet_id;
        let packet = encode_packet(ping_packet_id, &[0x01, 0x02], false).unwrap();

        let event = session.ingest_packet_bytes(&packet).unwrap();

        assert_eq!(
            event,
            ClientSessionEvent::Ping {
                sent_at_ms: None,
                response_queued: false,
            }
        );
        assert!(session
            .advance_time(1)
            .unwrap()
            .into_iter()
            .all(|action| !matches!(
                action,
                ClientSessionAction::SendPacket { packet_id, .. }
                    if Some(packet_id) == session.ping_response_packet_id
            )));
    }

    #[test]
    fn normal_priority_packets_defer_until_client_loaded_and_replay_before_connect_confirm() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_typeio_string_payload("[accent]queued"),
            false,
        )
        .unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        let deferred = session.ingest_packet_bytes(&packet).unwrap();
        assert_eq!(
            deferred,
            ClientSessionEvent::DeferredPacketWhileLoading {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "sendMessage".to_string(),
                    packet_class: "mindustry.gen.SendMessageCallPacket".to_string(),
                }),
            }
        );
        assert_eq!(session.state().received_server_message_count, 0);
        assert_eq!(session.state().deferred_inbound_packet_count, 1);
        assert!(!session.state().client_loaded);

        let mut saw_world_ready = false;
        for chunk in &chunk_packets {
            if matches!(
                session.ingest_packet_bytes(chunk).unwrap(),
                ClientSessionEvent::WorldStreamReady { .. }
            ) {
                saw_world_ready = true;
            }
        }

        assert!(saw_world_ready);
        assert!(session.state().client_loaded);
        assert!(!session.state().connect_confirm_sent);
        assert_eq!(session.state().replayed_inbound_packet_count, 1);
        assert_eq!(
            session.state().last_replayed_packet_method.as_deref(),
            Some("sendMessage")
        );
        assert_eq!(
            session.take_replayed_loading_events(),
            vec![ClientSessionEvent::ServerMessage {
                message: "[accent]queued".to_string()
            }]
        );
        assert_eq!(session.state().received_server_message_count, 1);
        assert_eq!(
            session.state().last_server_message.as_deref(),
            Some("[accent]queued")
        );
    }

    #[test]
    fn normal_priority_packets_continue_to_queue_past_previous_cap_while_loading() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &encode_typeio_string_payload("[accent]queued"),
            false,
        )
        .unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        const QUEUED_PACKET_COUNT: usize = 300;

        for _ in 0..QUEUED_PACKET_COUNT {
            let deferred = session.ingest_packet_bytes(&packet).unwrap();
            assert!(matches!(
                deferred,
                ClientSessionEvent::DeferredPacketWhileLoading { .. }
            ));
        }
        assert_eq!(
            session.state().deferred_inbound_packet_count,
            QUEUED_PACKET_COUNT as u64
        );
        assert_eq!(session.state().dropped_loading_deferred_overflow_count, 0);
        assert_eq!(
            session
                .state()
                .last_dropped_loading_deferred_overflow_packet_id,
            None
        );
        assert_eq!(
            session
                .state()
                .last_dropped_loading_deferred_overflow_packet_method,
            None
        );

        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }

        assert!(session.state().client_loaded);
        assert_eq!(
            session.state().replayed_inbound_packet_count,
            QUEUED_PACKET_COUNT as u64
        );
        assert_eq!(
            session.take_replayed_loading_events().len(),
            QUEUED_PACKET_COUNT
        );
        assert_eq!(
            session.state().received_server_message_count,
            QUEUED_PACKET_COUNT as u64
        );
    }

    #[test]
    fn low_priority_packets_are_ignored_while_loading_and_not_replayed() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let packet = encode_packet(
            packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        let event = session.ingest_packet_bytes(&packet).unwrap();
        assert_eq!(
            event,
            ClientSessionEvent::IgnoredPacket {
                packet_id,
                remote: Some(IgnoredRemotePacketMeta {
                    method: "stateSnapshot".to_string(),
                    packet_class: "mindustry.gen.StateSnapshotCallPacket".to_string(),
                }),
            }
        );
        assert_eq!(session.state().received_snapshot_count, 0);
        assert!(!session.state().seen_state_snapshot);
        assert_eq!(session.state().dropped_loading_low_priority_packet_count, 1);
        assert_eq!(
            session
                .state()
                .last_dropped_loading_packet_method
                .as_deref(),
            Some("stateSnapshot")
        );

        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }

        assert!(session.state().client_loaded);
        assert!(session.take_replayed_loading_events().is_empty());
        assert_eq!(session.state().received_snapshot_count, 0);
        assert!(!session.state().seen_state_snapshot);
    }

    #[test]
    fn world_data_begin_clears_deferred_loading_queue() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let send_message_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "sendMessage" && entry.params.len() == 1)
            .unwrap()
            .packet_id;
        let send_message_packet = encode_packet(
            send_message_packet_id,
            &encode_typeio_string_payload("[accent]stale"),
            false,
        )
        .unwrap();
        let world_data_begin_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "worldDataBegin")
            .unwrap()
            .packet_id;
        let world_data_begin = encode_packet(world_data_begin_packet_id, &[], false).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        let deferred = session.ingest_packet_bytes(&send_message_packet).unwrap();
        assert!(matches!(
            deferred,
            ClientSessionEvent::DeferredPacketWhileLoading { .. }
        ));
        assert_eq!(session.state().deferred_inbound_packet_count, 1);

        let reset = session.ingest_packet_bytes(&world_data_begin).unwrap();
        assert_eq!(reset, ClientSessionEvent::WorldDataBegin);
        assert!(!session.state().client_loaded);
        assert_eq!(session.state().deferred_inbound_packet_count, 0);
        assert!(session.take_replayed_loading_events().is_empty());

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }

        assert!(session.state().client_loaded);
        assert!(session.take_replayed_loading_events().is_empty());
        assert_eq!(session.state().received_server_message_count, 0);
        assert_eq!(session.state().last_server_message, None);
    }

    #[test]
    fn world_data_begin_resets_ready_state_and_allows_second_connect_confirm() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }

        assert!(session.state().world_stream_loaded);
        assert!(session.state().ready_to_enter_world);
        assert!(session.state().client_loaded);
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        assert!(session.state().connect_confirm_sent);
        assert!(session.loaded_world_bundle().is_some());
        let state_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let block_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::BlockSnapshot.method_name())
            .unwrap()
            .packet_id;
        let hidden_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::HiddenSnapshot.method_name())
            .unwrap()
            .packet_id;
        let state_snapshot = encode_packet(
            state_snapshot_packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&state_snapshot).unwrap();
        let block_snapshot = encode_packet(
            block_snapshot_packet_id,
            &sample_snapshot_packet("blockSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&block_snapshot).unwrap();
        let hidden_snapshot = encode_packet(
            hidden_snapshot_packet_id,
            &sample_snapshot_packet("hiddenSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&hidden_snapshot).unwrap();
        assert_eq!(session.state().received_snapshot_count, 3);
        assert!(session.state().last_state_snapshot.is_some());
        assert!(session.state().last_state_snapshot_core_data.is_some());
        assert!(session
            .state()
            .state_snapshot_authority_projection
            .is_some());
        assert!(session.state().last_block_snapshot.is_some());
        assert!(session.state().last_hidden_snapshot.is_some());
        let tile_config_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "tileConfig")
            .unwrap()
            .packet_id;
        session
            .queue_tile_config(Some(321), TypeIoObject::Int(7))
            .unwrap();
        let tile_config_packet = encode_packet(
            tile_config_packet_id,
            &encode_tile_config_payload(Some(321), &TypeIoObject::Int(7)),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&tile_config_packet).unwrap();
        session
            .queue_tile_config(Some(999), TypeIoObject::Int(3))
            .unwrap();
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .authoritative_by_build_pos
                .get(&321),
            Some(&TypeIoObject::Int(7))
        );
        assert!(!session
            .state()
            .building_table_projection
            .by_build_pos
            .is_empty());
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .pending_local_by_build_pos
                .get(&999),
            Some(&TypeIoObject::Int(3))
        );
        let effect_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "effect" && entry.params.len() == 6)
            .unwrap()
            .packet_id;
        let mut effect_payload = encode_effect_payload(13, 32.5, 48.0, 90.0, 0x11223344);
        write_typeio_object(&mut effect_payload, &TypeIoObject::Point2 { x: 3, y: 4 });
        let effect_packet = encode_packet(effect_packet_id, &effect_payload, false).unwrap();
        session.ingest_packet_bytes(&effect_packet).unwrap();
        assert_eq!(
            session.state().last_effect_business_projection,
            Some(EffectBusinessProjection::WorldPosition {
                source: EffectBusinessPositionSource::Point2,
                x_bits: 24.0f32.to_bits(),
                y_bits: 32.0f32.to_bits(),
            })
        );
        let set_rules_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setRules")
            .unwrap()
            .packet_id;
        let set_rules_json = br#"{"waves":true,"waveSpacing":120.0}"#;
        let mut set_rules_payload = Vec::new();
        set_rules_payload.extend_from_slice(&(set_rules_json.len() as i32).to_be_bytes());
        set_rules_payload.extend_from_slice(set_rules_json);
        let set_rules_packet =
            encode_packet(set_rules_packet_id, &set_rules_payload, false).unwrap();
        session.ingest_packet_bytes(&set_rules_packet).unwrap();
        assert_eq!(session.state().rules_projection.waves, Some(true));
        assert_eq!(session.state().rules_projection.wave_spacing, Some(120.0));
        let set_objectives_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "setObjectives")
            .unwrap()
            .packet_id;
        let set_objectives_json = br#"[{"type":"Research","content":"router","completed":false}]"#;
        let mut set_objectives_payload = Vec::new();
        set_objectives_payload.extend_from_slice(&(set_objectives_json.len() as i32).to_be_bytes());
        set_objectives_payload.extend_from_slice(set_objectives_json);
        let set_objectives_packet =
            encode_packet(set_objectives_packet_id, &set_objectives_payload, false).unwrap();
        session.ingest_packet_bytes(&set_objectives_packet).unwrap();
        assert_eq!(session.state().objectives_projection.objectives.len(), 1);
        session.state.builder_queue_projection.finished_count = 4;
        session.state.builder_queue_projection.removed_count = 2;
        session
            .state
            .builder_queue_projection
            .orphan_authoritative_count = 1;
        session.state.builder_queue_projection.last_stage =
            Some(crate::session_state::BuilderPlanStage::Finished);
        session.state.builder_queue_projection.last_x = Some(100);
        session.state.builder_queue_projection.last_y = Some(99);
        session.state.builder_queue_projection.last_breaking = Some(false);
        session.state.entity_table_projection.upsert_local_player(
            777,
            2,
            123,
            4.0f32.to_bits(),
            8.0f32.to_bits(),
            true,
            1,
        );

        let world_data_begin_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "worldDataBegin")
            .unwrap()
            .packet_id;
        let world_data_begin = encode_packet(world_data_begin_packet_id, &[], false).unwrap();

        let event = session.ingest_packet_bytes(&world_data_begin).unwrap();
        assert_eq!(event, ClientSessionEvent::WorldDataBegin);
        assert!(!session.state().world_stream_loaded);
        assert!(!session.state().ready_to_enter_world);
        assert!(!session.state().client_loaded);
        assert!(!session.state().connect_confirm_sent);
        assert_eq!(session.state().last_connect_confirm_at_ms, None);
        assert_eq!(session.state().last_client_snapshot_at_ms, None);
        assert_eq!(session.state().received_snapshot_count, 0);
        assert_eq!(session.state().last_snapshot_packet_id, None);
        assert_eq!(session.state().last_snapshot_method, None);
        assert_eq!(session.state().last_snapshot_payload_len, 0);
        assert_eq!(session.state().last_state_snapshot, None);
        assert_eq!(session.state().last_state_snapshot_core_data, None);
        assert_eq!(session.state().last_good_state_snapshot_core_data, None);
        assert_eq!(session.state().authoritative_state_mirror, None);
        assert_eq!(session.state().state_snapshot_authority_projection, None);
        assert_eq!(session.state().state_snapshot_business_projection, None);
        assert_eq!(session.state().world_player_unit_value, None);
        assert_eq!(session.state().world_player_x_bits, None);
        assert_eq!(session.state().world_player_y_bits, None);
        assert_eq!(session.state().last_block_snapshot, None);
        assert_eq!(session.state().last_hidden_snapshot, None);
        assert_eq!(session.state().hidden_snapshot_delta_projection, None);
        assert!(session.state().hidden_snapshot_ids.is_empty());
        assert_eq!(
            session.state().rules_projection,
            crate::rules_objectives_semantics::RulesProjection::default()
        );
        assert_eq!(
            session.state().objectives_projection,
            crate::rules_objectives_semantics::ObjectivesProjection::default()
        );
        assert_eq!(session.state().last_effect_business_projection, None);
        assert!(session.loaded_world_bundle().is_none());
        let input = session.snapshot_input_mut();
        assert_eq!(input.unit_id, None);
        assert!(input.dead);
        assert_eq!(input.position, None);
        assert!(!input.building);
        assert_eq!(input.plans, None);
        assert_eq!(input.view_center, None);
        assert!(session
            .state()
            .tile_config_projection
            .authoritative_by_build_pos
            .is_empty());
        assert!(session
            .state()
            .tile_config_projection
            .pending_local_by_build_pos
            .is_empty());
        assert_eq!(
            session.state().tile_config_projection.last_queued_build_pos,
            None
        );
        assert_eq!(
            session
                .state()
                .tile_config_projection
                .last_business_build_pos,
            None
        );
        assert!(!session.state().tile_config_projection.last_business_applied);
        assert_eq!(
            session.state().building_table_projection,
            crate::session_state::BuildingTableProjection::default()
        );
        assert_eq!(
            session.state().builder_queue_projection,
            crate::session_state::BuilderQueueProjection::default()
        );
        assert_eq!(
            session.state().entity_table_projection,
            crate::session_state::EntityTableProjection::default()
        );

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }

        assert!(session.state().world_stream_loaded);
        assert!(session.state().ready_to_enter_world);
        assert!(session.state().client_loaded);
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        assert!(session.state().connect_confirm_sent);
    }

    #[test]
    fn world_data_begin_clears_state_snapshot_authority_projection() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }

        let state_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let state_snapshot = encode_packet(
            state_snapshot_packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();
        session.ingest_packet_bytes(&state_snapshot).unwrap();
        assert!(session.state().authoritative_state_mirror.is_some());
        assert!(session
            .state()
            .state_snapshot_authority_projection
            .is_some());

        let world_data_begin_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == "worldDataBegin")
            .unwrap()
            .packet_id;
        let world_data_begin = encode_packet(world_data_begin_packet_id, &[], false).unwrap();

        session.ingest_packet_bytes(&world_data_begin).unwrap();

        assert_eq!(session.state().authoritative_state_mirror, None);
        assert_eq!(session.state().state_snapshot_authority_projection, None);
    }

    #[test]
    fn prepare_connect_packet_quiet_resets_session_before_reconnect() {
        let manifest = read_remote_manifest(real_manifest_path()).unwrap();
        let mut session = ClientSession::from_remote_manifest(&manifest, "fr").unwrap();
        let connect_payload = sample_connect_payload();
        let compressed_world_stream = sample_world_stream_bytes();
        let (begin_packet, chunk_packets) =
            encode_world_stream_packets(&compressed_world_stream, 7, 1024).unwrap();
        let state_snapshot_packet_id = manifest
            .remote_packets
            .iter()
            .find(|entry| entry.method == HighFrequencyRemoteMethod::StateSnapshot.method_name())
            .unwrap()
            .packet_id;
        let state_snapshot = encode_packet(
            state_snapshot_packet_id,
            &sample_snapshot_packet("stateSnapshot.packet"),
            false,
        )
        .unwrap();

        session.prepare_connect_packet(&connect_payload).unwrap();
        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
        session.ingest_packet_bytes(&state_snapshot).unwrap();
        assert!(session.state().connect_confirm_sent);
        assert!(session.state().world_stream_loaded);
        assert!(session.state().authoritative_state_mirror.is_some());
        session.state.connection_timed_out = true;
        session.kicked = true;
        session.timed_out = true;
        session.last_inbound_at_ms = Some(123);
        session.last_keepalive_at_ms = Some(456);
        session.next_client_snapshot_id = 42;
        session
            .queue_tile_config(Some(321), TypeIoObject::Int(7))
            .unwrap();
        assert!(!session.pending_packets.is_empty());

        let reconnect = session.prepare_connect_packet(&connect_payload).unwrap();
        assert_eq!(reconnect.payload, connect_payload);
        assert!(session.state().connect_packet_sent);
        assert_eq!(session.state().connect_payload_len, connect_payload.len());
        assert_eq!(session.state().connect_packet_len, reconnect.encoded_packet.len());
        assert!(!session.state().world_stream_loaded);
        assert!(!session.state().ready_to_enter_world);
        assert!(!session.state().client_loaded);
        assert!(!session.state().connect_confirm_sent);
        assert!(!session.state().connection_timed_out);
        assert_eq!(session.state().authoritative_state_mirror, None);
        assert_eq!(session.state().state_snapshot_authority_projection, None);
        assert!(session.pending_packets.is_empty());
        assert!(session.deferred_inbound_packets.is_empty());
        assert!(session.replayed_loading_events.is_empty());
        assert_eq!(session.last_inbound_at_ms, None);
        assert_eq!(session.last_keepalive_at_ms, None);
        assert_eq!(session.next_client_snapshot_id, 1);
        assert!(!session.kicked);
        assert!(!session.timed_out);
        assert!(!session.loading_world_data);
        assert!(session.loaded_world_bundle().is_none());

        session.ingest_packet_bytes(&begin_packet).unwrap();
        for chunk in &chunk_packets {
            session.ingest_packet_bytes(chunk).unwrap();
        }
        assert!(session.state().world_stream_loaded);
        assert!(session.prepare_connect_confirm_packet().unwrap().is_some());
    }
}
