use crate::client_session::ClientSessionEvent;
use crate::session_state::UnitRefProjection;
use mdt_typeio::TypeIoObject;

pub fn format_final_kick_summary(
    kicked: bool,
    reason_text: Option<&str>,
    reason_ordinal: Option<i32>,
    duration_ms: Option<u64>,
) -> String {
    let (hint_category, hint_text) = summarize_kick_hint_from(reason_text, reason_ordinal);
    format!(
        "final_kick: kicked={} reason_text={reason_text:?} reason_ordinal={reason_ordinal:?} duration_ms={duration_ms:?} hint_category={} hint_text={hint_text:?}",
        kicked,
        hint_category.unwrap_or("none"),
    )
}

pub fn summarize_client_packet_events(events: &[ClientSessionEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            ClientSessionEvent::ConnectRedirectRequested { ip, port } => {
                Some(format!("connect_redirect: ip={ip:?} port={port}"))
            }
            ClientSessionEvent::PlayerSpawned { player_id, x, y } => Some(format!(
                "player_spawn: player_id={player_id} x_bits=0x{:08x} y_bits=0x{:08x}",
                x.to_bits(),
                y.to_bits()
            )),
            ClientSessionEvent::PlayerPositionUpdated { x, y } => Some(format!(
                "player_position: x_bits=0x{:08x} y_bits=0x{:08x}",
                x.to_bits(),
                y.to_bits()
            )),
            ClientSessionEvent::CameraPositionUpdated { x, y } => Some(format!(
                "camera_position: x_bits=0x{:08x} y_bits=0x{:08x}",
                x.to_bits(),
                y.to_bits()
            )),
            ClientSessionEvent::PlayerDisconnected {
                player_id,
                cleared_local_player_sync,
            } => Some(format!(
                "player_disconnect: player_id={player_id} cleared_local_player_sync={cleared_local_player_sync}"
            )),
            ClientSessionEvent::ServerMessage { message } => {
                Some(format!("server_message: message={message:?}"))
            }
            ClientSessionEvent::ChatMessage {
                message,
                unformatted,
                sender_entity_id,
            } => Some(format!(
                "chat_message: message={message:?} unformatted={unformatted:?} sender_entity_id={sender_entity_id:?}"
            )),
            ClientSessionEvent::ClientPacketReliable {
                packet_type,
                contents,
            } => Some(format_text_packet_summary(
                "reliable",
                packet_type,
                contents,
            )),
            ClientSessionEvent::ClientPacketUnreliable {
                packet_type,
                contents,
            } => Some(format_text_packet_summary(
                "unreliable",
                packet_type,
                contents,
            )),
            ClientSessionEvent::ClientBinaryPacketReliable {
                packet_type,
                contents,
            } => Some(format_binary_packet_summary(
                "reliable",
                packet_type,
                contents,
            )),
            ClientSessionEvent::ClientBinaryPacketUnreliable {
                packet_type,
                contents,
            } => Some(format_binary_packet_summary(
                "unreliable",
                packet_type,
                contents,
            )),
            ClientSessionEvent::ClientLogicDataReliable { channel, value } => {
                Some(format_logic_data_summary("reliable", channel, value))
            }
            ClientSessionEvent::ClientLogicDataUnreliable { channel, value } => {
                Some(format_logic_data_summary("unreliable", channel, value))
            }
            ClientSessionEvent::SoundRequested {
                sound_id,
                volume,
                pitch,
                pan,
            } => Some(format!(
                "sound: sound_id={sound_id:?} volume_bits=0x{:08x} pitch_bits=0x{:08x} pan_bits=0x{:08x}",
                volume.to_bits(),
                pitch.to_bits(),
                pan.to_bits()
            )),
            ClientSessionEvent::SoundAtRequested {
                sound_id,
                x,
                y,
                volume,
                pitch,
            } => Some(format!(
                "sound_at: sound_id={sound_id:?} x_bits=0x{:08x} y_bits=0x{:08x} volume_bits=0x{:08x} pitch_bits=0x{:08x}",
                x.to_bits(),
                y.to_bits(),
                volume.to_bits(),
                pitch.to_bits()
            )),
            ClientSessionEvent::CreateWeather {
                weather_id,
                intensity,
                duration,
                wind_x,
                wind_y,
            } => Some(format!(
                "create_weather: weather_id={weather_id:?} intensity_bits=0x{:08x} duration_bits=0x{:08x} wind_x_bits=0x{:08x} wind_y_bits=0x{:08x}",
                intensity.to_bits(),
                duration.to_bits(),
                wind_x.to_bits(),
                wind_y.to_bits()
            )),
            ClientSessionEvent::SpawnEffect {
                x,
                y,
                rotation,
                unit_type_id,
            } => Some(format!(
                "spawn_effect: x_bits=0x{:08x} y_bits=0x{:08x} rotation_bits=0x{:08x} unit_type_id={unit_type_id:?}",
                x.to_bits(),
                y.to_bits(),
                rotation.to_bits()
            )),
            ClientSessionEvent::LogicExplosionObserved {
                team_id,
                x,
                y,
                radius,
                damage,
                air,
                ground,
                pierce,
                effect,
            } => Some(format!(
                "logic_explosion: team_id={team_id} x_bits=0x{:08x} y_bits=0x{:08x} radius_bits=0x{:08x} damage_bits=0x{:08x} air={air} ground={ground} pierce={pierce} effect={effect}",
                x.to_bits(),
                y.to_bits(),
                radius.to_bits(),
                damage.to_bits()
            )),
            ClientSessionEvent::UnitSpawnObserved {
                unit_id,
                unit_class_id,
                payload_len,
                consumed_bytes,
                trailing_bytes,
            } => Some(format!(
                "unit_spawn: unit_id={unit_id} unit_class_id={unit_class_id} payload_len={payload_len} consumed_bytes={consumed_bytes} trailing_bytes={trailing_bytes}"
            )),
            ClientSessionEvent::UnitBlockSpawn { tile_pos } => {
                Some(format!("unit_block_spawn: tile_pos={tile_pos:?}"))
            }
            ClientSessionEvent::UnitTetherBlockSpawned { tile_pos, unit_id } => Some(format!(
                "unit_tether_block_spawned: tile_pos={tile_pos:?} unit_id={unit_id}"
            )),
            ClientSessionEvent::AutoDoorToggle { tile_pos, open } => Some(format!(
                "auto_door_toggle: tile_pos={tile_pos:?} open={open}"
            )),
            ClientSessionEvent::LandingPadLanded { tile_pos } => {
                Some(format!("landing_pad_landed: tile_pos={tile_pos:?}"))
            }
            ClientSessionEvent::AssemblerDroneSpawned { tile_pos, unit_id } => Some(format!(
                "assembler_drone_spawned: tile_pos={tile_pos:?} unit_id={unit_id}"
            )),
            ClientSessionEvent::AssemblerUnitSpawned { tile_pos } => {
                Some(format!("assembler_unit_spawned: tile_pos={tile_pos:?}"))
            }
            ClientSessionEvent::TraceInfoReceived {
                player_id,
                ip,
                uuid,
                locale,
                modded,
                mobile,
                times_joined,
                times_kicked,
                ips,
                names,
            } => Some(format!(
                "trace_info: player_id={player_id:?} ip={ip:?} uuid={uuid:?} locale={locale:?} modded={modded} mobile={mobile} times_joined={times_joined} times_kicked={times_kicked} ips={} names={}",
                ips.len(),
                names.len()
            )),
            ClientSessionEvent::DebugStatusReceived {
                reliable,
                value,
                last_client_snapshot,
                snapshots_sent,
            } => Some(format!(
                "debug_status: reliable={reliable} value={value} last_client_snapshot={last_client_snapshot} snapshots_sent={snapshots_sent}"
            )),
            ClientSessionEvent::RulesUpdatedRaw { json_data } => Some(format!(
                "set_rules: len={} json_data={json_data:?}",
                json_data.len()
            )),
            ClientSessionEvent::ObjectivesUpdatedRaw { json_data } => Some(format!(
                "set_objectives: len={} json_data={json_data:?}",
                json_data.len()
            )),
            ClientSessionEvent::SetRuleApplied { rule, json_data } => Some(format!(
                "set_rule: rule={rule:?} len={} json_data={json_data:?}",
                json_data.len()
            )),
            ClientSessionEvent::ObjectivesCleared => Some("clear_objectives".to_string()),
            ClientSessionEvent::ObjectiveCompleted { index } => {
                Some(format!("complete_objective: index={index}"))
            }
            ClientSessionEvent::Kicked {
                reason_text,
                reason_ordinal,
                duration_ms,
            } => {
                let (hint_category, hint_text) =
                    summarize_kick_hint_from(reason_text.as_deref(), *reason_ordinal);
                Some(format!(
                    "kick: reason_text={reason_text:?} reason_ordinal={reason_ordinal:?} duration_ms={duration_ms:?} hint_category={} hint_text={hint_text:?}",
                    hint_category.unwrap_or("none")
                ))
            }
            ClientSessionEvent::Ping {
                sent_at_ms,
                response_queued,
            } => Some(format!(
                "ping: sent_at_ms={sent_at_ms:?} response_queued={response_queued}"
            )),
            ClientSessionEvent::PingResponse {
                sent_at_ms,
                round_trip_ms,
            } => Some(format!(
                "ping_response: sent_at_ms={sent_at_ms:?} round_trip_ms={round_trip_ms:?}"
            )),
            ClientSessionEvent::StateSnapshotApplied { projection } => {
                Some(format_state_snapshot_applied_summary(projection))
            }
            ClientSessionEvent::DeferredPacketWhileLoading { packet_id, remote } => Some(format!(
                "deferred_packet_while_loading: packet_id={packet_id} method={:?} packet_class={:?}",
                remote.as_ref().map(|meta| meta.method.as_str()),
                remote.as_ref().map(|meta| meta.packet_class.as_str()),
            )),
            ClientSessionEvent::DroppedLowPriorityPacketWhileLoading { packet_id, remote } => {
                Some(format!(
                    "dropped_low_priority_packet_while_loading: packet_id={packet_id} method={:?} packet_class={:?}",
                    remote.as_ref().map(|meta| meta.method.as_str()),
                    remote.as_ref().map(|meta| meta.packet_class.as_str()),
                ))
            }
            ClientSessionEvent::IgnoredPacket { packet_id, remote } => Some(format!(
                "ignored_packet: packet_id={packet_id} method={:?} packet_class={:?}",
                remote.as_ref().map(|meta| meta.method.as_str()),
                remote.as_ref().map(|meta| meta.packet_class.as_str()),
            )),
            ClientSessionEvent::TileConfig {
                build_pos,
                config_kind,
                config_kind_name,
                parse_failed,
                business_applied,
                cleared_pending_local,
                was_rollback,
                pending_local_match,
            } => Some(format_tile_config_summary(
                *build_pos,
                *config_kind,
                config_kind_name.as_deref(),
                *parse_failed,
                *business_applied,
                *cleared_pending_local,
                *was_rollback,
                *pending_local_match,
            )),
            ClientSessionEvent::SetHudText { message } => {
                Some(format!("set_hud_text: message={message:?}"))
            }
            ClientSessionEvent::SetHudTextReliable { message } => {
                Some(format!("set_hud_text_reliable: message={message:?}"))
            }
            ClientSessionEvent::HideHudText => Some("hide_hud_text".to_string()),
            ClientSessionEvent::Announce { message } => {
                Some(format!("announce: message={message:?}"))
            }
            ClientSessionEvent::WorldLabel {
                reliable,
                label_id,
                message,
                duration,
                world_x,
                world_y,
            } => Some(format!(
                "world_label: reliable={reliable} label_id={label_id:?} message={message:?} duration_bits=0x{:08x} world_x_bits=0x{:08x} world_y_bits=0x{:08x}",
                duration.to_bits(),
                world_x.to_bits(),
                world_y.to_bits()
            )),
            ClientSessionEvent::RemoveWorldLabel { label_id } => {
                Some(format!("remove_world_label: label_id={label_id}"))
            }
            ClientSessionEvent::CreateMarker { marker_id, json_len } => {
                Some(format!("create_marker: marker_id={marker_id} json_len={json_len}"))
            }
            ClientSessionEvent::RemoveMarker { marker_id } => {
                Some(format!("remove_marker: marker_id={marker_id}"))
            }
            ClientSessionEvent::UpdateMarker {
                marker_id,
                control,
                control_name,
                p1_bits,
                p2_bits,
                p3_bits,
            } => Some(format!(
                "update_marker: marker_id={marker_id} control={control} control_name={control_name:?} p1_bits=0x{p1_bits:016x} p2_bits=0x{p2_bits:016x} p3_bits=0x{p3_bits:016x}"
            )),
            ClientSessionEvent::UpdateMarkerText {
                marker_id,
                control,
                control_name,
                fetch,
                text,
            } => Some(format!(
                "update_marker_text: marker_id={marker_id} control={control} control_name={control_name:?} fetch={fetch} text={text:?}"
            )),
            ClientSessionEvent::UpdateMarkerTexture {
                marker_id,
                texture_kind,
                texture_kind_name,
            } => Some(format!(
                "update_marker_texture: marker_id={marker_id} texture_kind={texture_kind} texture_kind_name={texture_kind_name:?}"
            )),
            ClientSessionEvent::MenuShown {
                menu_id,
                title,
                message,
                option_rows,
                first_row_len,
            } => Some(format!(
                "menu: menu_id={menu_id} title={title:?} message={message:?} rows={option_rows} first_row_len={first_row_len}"
            )),
            ClientSessionEvent::FollowUpMenuShown {
                menu_id,
                title,
                message,
                option_rows,
                first_row_len,
            } => Some(format!(
                "follow_up_menu: menu_id={menu_id} title={title:?} message={message:?} rows={option_rows} first_row_len={first_row_len}"
            )),
            ClientSessionEvent::HideFollowUpMenu { menu_id } => {
                Some(format!("hide_follow_up_menu: menu_id={menu_id}"))
            }
            ClientSessionEvent::CopyToClipboard { text } => {
                Some(format!("copy_to_clipboard: text={text:?}"))
            }
            ClientSessionEvent::OpenUri { uri } => Some(format!("open_uri: uri={uri:?}")),
            ClientSessionEvent::TextInput {
                text_input_id,
                title,
                message,
                text_length,
                default_text,
                numeric,
                allow_empty,
            } => Some(format!(
                "text_input: text_input_id={text_input_id} title={title:?} message={message:?} text_length={text_length} default_text={default_text:?} numeric={numeric} allow_empty={allow_empty}"
            )),
            ClientSessionEvent::SetItem {
                build_pos,
                item_id,
                amount,
            } => Some(format!(
                "set_item: build_pos={build_pos:?} item_id={item_id:?} amount={amount}"
            )),
            ClientSessionEvent::SetItems {
                build_pos,
                stack_count,
                first_item_id,
                first_amount,
            } => Some(format!(
                "set_items: build_pos={build_pos:?} count={stack_count} first_item_id={first_item_id:?} first_amount={first_amount:?}"
            )),
            ClientSessionEvent::SetLiquid {
                build_pos,
                liquid_id,
                amount,
            } => Some(format!(
                "set_liquid: build_pos={build_pos:?} liquid_id={liquid_id:?} amount_bits=0x{:08x}",
                amount.to_bits()
            )),
            ClientSessionEvent::SetLiquids {
                build_pos,
                stack_count,
                first_liquid_id,
                first_amount_bits,
            } => Some(format!(
                "set_liquids: build_pos={build_pos:?} count={stack_count} first_liquid_id={first_liquid_id:?} first_amount_bits={first_amount_bits:?}"
            )),
            ClientSessionEvent::SetFloor {
                tile_pos,
                floor_id,
                overlay_id,
            } => Some(format!(
                "set_floor: tile_pos={tile_pos:?} floor_id={floor_id:?} overlay_id={overlay_id:?}"
            )),
            ClientSessionEvent::SetOverlay {
                tile_pos,
                overlay_id,
            } => Some(format!(
                "set_overlay: tile_pos={tile_pos:?} overlay_id={overlay_id:?}"
            )),
            ClientSessionEvent::SetMapArea { x, y, w, h } => {
                Some(format!("set_map_area: x={x} y={y} w={w} h={h}"))
            }
            ClientSessionEvent::SetTeam { build_pos, team_id } => Some(format!(
                "set_team: build_pos={build_pos:?} team_id={team_id}"
            )),
            ClientSessionEvent::SetTeams {
                team_id,
                position_count,
                first_position,
            } => Some(format!(
                "set_teams: team_id={team_id} count={position_count} first_position={first_position:?}"
            )),
            ClientSessionEvent::SetTileBlocks {
                block_id,
                team_id,
                position_count,
                first_position,
            } => Some(format!(
                "set_tile_blocks: block_id={block_id:?} team_id={team_id} count={position_count} first_position={first_position:?}"
            )),
            ClientSessionEvent::SetTileFloors {
                block_id,
                position_count,
                first_position,
            } => Some(format!(
                "set_tile_floors: block_id={block_id:?} count={position_count} first_position={first_position:?}"
            )),
            ClientSessionEvent::SetTileItems {
                item_id,
                amount,
                position_count,
                first_position,
            } => Some(format!(
                "set_tile_items: item_id={item_id:?} amount={amount} count={position_count} first_position={first_position:?}"
            )),
            ClientSessionEvent::SetTileLiquids {
                liquid_id,
                amount_bits,
                position_count,
                first_position,
            } => Some(format!(
                "set_tile_liquids: liquid_id={liquid_id:?} amount_bits=0x{amount_bits:08x} count={position_count} first_position={first_position:?}"
            )),
            ClientSessionEvent::SetTileOverlays {
                block_id,
                position_count,
                first_position,
            } => Some(format!(
                "set_tile_overlays: block_id={block_id:?} count={position_count} first_position={first_position:?}"
            )),
            ClientSessionEvent::RemoveTile { tile_pos } => {
                Some(format!("remove_tile: tile_pos={tile_pos:?}"))
            }
            ClientSessionEvent::SetTile {
                tile_pos,
                block_id,
                team_id,
                rotation,
            } => Some(format!(
                "set_tile: tile_pos={tile_pos:?} block_id={block_id:?} team_id={team_id} rotation={rotation}"
            )),
            ClientSessionEvent::SyncVariable {
                build_pos,
                variable,
                value_kind,
                value_kind_name,
            } => Some(format!(
                "sync_variable: build_pos={build_pos:?} variable={variable} value_kind={value_kind} value_kind_name={value_kind_name:?}"
            )),
            ClientSessionEvent::InfoMessage { message } => {
                Some(format!("info_message: message={message:?}"))
            }
            ClientSessionEvent::InfoPopup {
                reliable,
                popup_id,
                message,
                duration,
                align,
                top,
                left,
                bottom,
                right,
            } => Some(format!(
                "info_popup: reliable={reliable} popup_id={popup_id:?} message={message:?} duration_bits=0x{:08x} align={align} top={top} left={left} bottom={bottom} right={right}",
                duration.to_bits()
            )),
            ClientSessionEvent::InfoToast { message, duration } => Some(format!(
                "info_toast: message={message:?} duration_bits=0x{:08x}",
                duration.to_bits()
            )),
            ClientSessionEvent::WarningToast { unicode, text } => {
                Some(format!("warning_toast: unicode={unicode} text={text:?}"))
            }
            ClientSessionEvent::SetFlag { flag, add } => {
                Some(format!("set_flag: flag={flag:?} add={add}"))
            }
            ClientSessionEvent::GameOver { winner_team_id } => {
                Some(format!("game_over: winner_team_id={winner_team_id}"))
            }
            ClientSessionEvent::UpdateGameOver { winner_team_id } => {
                Some(format!("update_game_over: winner_team_id={winner_team_id}"))
            }
            ClientSessionEvent::SectorCapture => Some("sector_capture".to_string()),
            ClientSessionEvent::Researched {
                content_type,
                content_id,
            } => Some(format!(
                "researched: content_type={content_type} content_id={content_id}"
            )),
            ClientSessionEvent::SetPlayerTeamEditor { team_id } => {
                Some(format!("set_player_team_editor: team_id={team_id}"))
            }
            ClientSessionEvent::MenuChoose { menu_id, option } => {
                Some(format!("menu_choose: menu_id={menu_id} option={option}"))
            }
            ClientSessionEvent::TextInputResult {
                text_input_id,
                text,
            } => Some(format!(
                "text_input_result: text_input_id={text_input_id} text={text:?}"
            )),
            ClientSessionEvent::RequestItem {
                build_pos,
                item_id,
                amount,
            } => Some(format!(
                "request_item: build_pos={build_pos:?} item_id={item_id:?} amount={amount}"
            )),
            ClientSessionEvent::RequestBuildPayload { build_pos } => {
                Some(format!("request_build_payload: build_pos={build_pos:?}"))
            }
            ClientSessionEvent::RequestDropPayload { x, y } => Some(format!(
                "request_drop_payload: x_bits=0x{:08x} y_bits=0x{:08x}",
                x.to_bits(),
                y.to_bits()
            )),
            ClientSessionEvent::RequestUnitPayload { target } => {
                Some(format!("request_unit_payload: target={target:?}"))
            }
            ClientSessionEvent::TransferInventory { build_pos } => {
                Some(format!("transfer_inventory: build_pos={build_pos:?}"))
            }
            ClientSessionEvent::ClearItems { build_pos } => {
                Some(format!("clear_items: build_pos={build_pos:?}"))
            }
            ClientSessionEvent::ClearLiquids { build_pos } => {
                Some(format!("clear_liquids: build_pos={build_pos:?}"))
            }
            ClientSessionEvent::RotateBlock {
                build_pos,
                direction,
            } => Some(format_rotate_block_summary(*build_pos, *direction)),
            ClientSessionEvent::DropItem { angle } => Some(format_drop_item_summary(*angle)),
            ClientSessionEvent::DeletePlans { positions } => {
                Some(format_delete_plans_summary(positions))
            }
            ClientSessionEvent::TileTap { tile_pos } => {
                Some(format!("tile_tap: tile_pos={tile_pos:?}"))
            }
            ClientSessionEvent::BuildingControlSelect { build_pos } => {
                Some(format!("building_control_select: build_pos={build_pos:?}"))
            }
            ClientSessionEvent::BuildDestroyed { build_pos } => {
                Some(format!("build_destroyed: build_pos={build_pos:?}"))
            }
            ClientSessionEvent::UnitDeath {
                unit_id,
                removed_entity_projection,
            } => Some(format!(
                "unit_death: unit_id={unit_id} removed_entity_projection={removed_entity_projection}"
            )),
            ClientSessionEvent::UnitDestroy {
                unit_id,
                removed_entity_projection,
            } => Some(format!(
                "unit_destroy: unit_id={unit_id} removed_entity_projection={removed_entity_projection}"
            )),
            ClientSessionEvent::UnitEnvDeath {
                unit,
                removed_entity_projection,
            } => Some(format!(
                "unit_env_death: unit={unit:?} removed_entity_projection={removed_entity_projection}"
            )),
            ClientSessionEvent::UnitSafeDeath {
                unit,
                removed_entity_projection,
            } => Some(format!(
                "unit_safe_death: unit={unit:?} removed_entity_projection={removed_entity_projection}"
            )),
            ClientSessionEvent::UnitCapDeath { unit } => {
                Some(format!("unit_cap_death: unit={unit:?}"))
            }
            ClientSessionEvent::UnitClear => Some("unit_clear".to_string()),
            ClientSessionEvent::UnitControl { target } => {
                Some(format!("unit_control: target={target:?}"))
            }
            ClientSessionEvent::UnitBuildingControlSelect { target, build_pos } => Some(
                format_unit_building_control_select_summary(*target, *build_pos),
            ),
            ClientSessionEvent::CommandBuilding { buildings, x, y } => {
                Some(format_command_building_summary(buildings, *x, *y))
            }
            ClientSessionEvent::CommandUnits {
                unit_ids,
                build_target,
                unit_target,
                x,
                y,
                queue_command,
                final_batch,
            } => Some(format_command_units_summary(
                unit_ids,
                *build_target,
                *unit_target,
                *x,
                *y,
                *queue_command,
                *final_batch,
            )),
            ClientSessionEvent::SetUnitCommand {
                unit_ids,
                command_id,
            } => Some(format_set_unit_command_summary(unit_ids, *command_id)),
            ClientSessionEvent::SetUnitStance {
                unit_ids,
                stance_id,
                enable,
            } => Some(format_set_unit_stance_summary(
                unit_ids, *stance_id, *enable,
            )),
            _ => None,
        })
        .collect()
}

fn summarize_kick_hint_from(
    reason_text: Option<&str>,
    reason_ordinal: Option<i32>,
) -> (Option<&'static str>, Option<&'static str>) {
    let normalized_reason = match reason_text {
        Some("banned") => Some("banned"),
        Some("clientOutdated") => Some("clientOutdated"),
        Some("recentKick") => Some("recentKick"),
        Some("nameInUse") => Some("nameInUse"),
        Some("idInUse") => Some("idInUse"),
        Some("nameEmpty") => Some("nameEmpty"),
        Some("serverOutdated") => Some("serverOutdated"),
        Some("customClient") => Some("customClient"),
        Some("typeMismatch") => Some("typeMismatch"),
        Some("whitelist") => Some("whitelist"),
        Some("playerLimit") => Some("playerLimit"),
        Some("serverRestarting") => Some("serverRestarting"),
        _ => reason_ordinal.and_then(summarize_kick_reason_name_from_ordinal),
    };

    match normalized_reason {
        Some("banned") => (
            Some("Banned"),
            Some(
                "server reports this identity or name is banned; use a different account or ask the server admin to review the ban.",
            ),
        ),
        Some("clientOutdated") => (
            Some("ClientOutdated"),
            Some("client build is outdated; upgrade this client to the server version."),
        ),
        Some("recentKick") => (
            Some("RecentKick"),
            Some(
                "server still remembers a recent kick; wait for the cooldown to expire before reconnecting.",
            ),
        ),
        Some("nameInUse") => (
            Some("NameInUse"),
            Some("player name is already in use; retry with a different --name value."),
        ),
        Some("idInUse") => (
            Some("IdInUse"),
            Some(
                "uuid or usid is already in use; wait for the old session to clear or regenerate the connect identity.",
            ),
        ),
        Some("nameEmpty") => (
            Some("NameEmpty"),
            Some(
                "player name is empty or invalid; set --name to a non-empty value accepted by the server.",
            ),
        ),
        Some("serverOutdated") => (
            Some("ServerOutdated"),
            Some(
                "server build is older than this client; use a matching server or older client build.",
            ),
        ),
        Some("customClient") => (
            Some("CustomClientRejected"),
            Some(
                "server rejected custom clients; connect to a server that allows custom clients.",
            ),
        ),
        Some("typeMismatch") => (
            Some("TypeMismatch"),
            Some("version type/protocol mismatch; align client/server version type and mod set."),
        ),
        Some("whitelist") => (
            Some("WhitelistRequired"),
            Some("server requires whitelist access; ask the server admin to whitelist this identity."),
        ),
        Some("playerLimit") => (
            Some("PlayerLimit"),
            Some("server is full; wait for an open slot or use an identity with reserved access."),
        ),
        Some("serverRestarting") => (
            Some("ServerRestarting"),
            Some("server is restarting; retry connection shortly."),
        ),
        _ => (None, None),
    }
}

fn summarize_kick_reason_name_from_ordinal(reason_ordinal: i32) -> Option<&'static str> {
    match reason_ordinal {
        3 => Some("banned"),
        1 => Some("clientOutdated"),
        2 => Some("serverOutdated"),
        5 => Some("recentKick"),
        6 => Some("nameInUse"),
        7 => Some("idInUse"),
        8 => Some("nameEmpty"),
        9 => Some("customClient"),
        12 => Some("typeMismatch"),
        13 => Some("whitelist"),
        14 => Some("playerLimit"),
        15 => Some("serverRestarting"),
        _ => None,
    }
}

fn format_text_packet_summary(transport: &str, packet_type: &str, contents: &str) -> String {
    let escaped = contents.escape_default().to_string();
    let preview = truncate_for_preview(&escaped, 96);
    format!(
        "client_packet: transport={transport} type={packet_type:?} len={} preview={preview:?}",
        contents.len()
    )
}

fn format_binary_packet_summary(transport: &str, packet_type: &str, contents: &[u8]) -> String {
    let prefix_len = contents.len().min(16);
    let hex_prefix = encode_hex_text(&contents[..prefix_len]);
    format!(
        "client_binary_packet: transport={transport} type={packet_type:?} len={} hex_prefix={hex_prefix}",
        contents.len()
    )
}

fn format_logic_data_summary(transport: &str, channel: &str, value: &TypeIoObject) -> String {
    let preview = truncate_for_preview(&format!("{value:?}"), 96);
    format!(
        "client_logic_data: transport={transport} channel={channel:?} kind={:?} preview={preview:?}",
        value.kind()
    )
}

fn format_tile_config_summary(
    build_pos: Option<i32>,
    config_kind: Option<u8>,
    config_kind_name: Option<&str>,
    parse_failed: bool,
    business_applied: bool,
    cleared_pending_local: bool,
    was_rollback: bool,
    pending_local_match: Option<bool>,
) -> String {
    format!(
        "tile_config: build_pos={build_pos:?} kind={config_kind:?} kind_name={config_kind_name:?} parse_failed={parse_failed} business_applied={business_applied} cleared_pending_local={cleared_pending_local} rollback={was_rollback} pending_local_match={pending_local_match:?}"
    )
}

fn format_state_snapshot_applied_summary(
    projection: &crate::client_session::StateSnapshotAppliedProjection,
) -> String {
    format!(
        "state_snapshot_applied: wave={} enemies={} tps={} gameplay_state={} transitions={} wave_advanced={} wave_from={:?} wave_to={:?} apply_count={} net_seconds_delta={} rollback={} time_regress_count={} wave_regress_count={} core_parse_failed={} core_parse_fail_count={} used_last_good_core_fallback={} core_teams={} core_items={} core_total={} core_changed={} core_changed_sample={}",
        projection.wave,
        projection.enemies,
        projection.tps,
        projection.gameplay_state_name(),
        projection.gameplay_state_transition_count,
        projection.wave_advanced,
        projection.wave_advance_from,
        projection.wave_advance_to,
        projection.apply_count,
        projection.net_seconds_delta,
        projection.net_seconds_rollback,
        projection.time_regress_count,
        projection.wave_regress_count,
        projection.core_parse_failed,
        projection.core_parse_fail_count,
        projection.used_last_good_core_fallback,
        projection.core_inventory_team_count,
        projection.core_inventory_item_entry_count,
        projection.core_inventory_total_amount,
        projection.core_inventory_changed_team_count,
        format_u8_sample(&projection.core_inventory_changed_team_sample),
    )
}

fn format_u8_sample(values: &[u8]) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn format_command_building_summary(buildings: &[i32], x: f32, y: f32) -> String {
    format!(
        "command_building: count={} first_build_pos={:?} x_bits=0x{:08x} y_bits=0x{:08x}",
        buildings.len(),
        buildings.first().copied(),
        x.to_bits(),
        y.to_bits()
    )
}

fn format_rotate_block_summary(build_pos: Option<i32>, direction: bool) -> String {
    format!("rotate_block: build_pos={build_pos:?} direction={direction}")
}

fn format_drop_item_summary(angle: f32) -> String {
    format!("drop_item: angle_bits=0x{:08x}", angle.to_bits())
}

fn format_delete_plans_summary(positions: &[i32]) -> String {
    format!(
        "delete_plans: count={} first_pos={:?}",
        positions.len(),
        positions.first().copied()
    )
}

fn format_unit_building_control_select_summary(
    target: Option<UnitRefProjection>,
    build_pos: Option<i32>,
) -> String {
    format!("unit_building_control_select: target={target:?} build_pos={build_pos:?}")
}

fn format_command_units_summary(
    unit_ids: &[i32],
    build_target: Option<i32>,
    unit_target: Option<UnitRefProjection>,
    x: f32,
    y: f32,
    queue_command: bool,
    final_batch: bool,
) -> String {
    format!(
        "command_units: count={} first_unit_id={:?} build_target={build_target:?} unit_target={unit_target:?} x_bits=0x{:08x} y_bits=0x{:08x} queue={queue_command} final_batch={final_batch}",
        unit_ids.len(),
        unit_ids.first().copied(),
        x.to_bits(),
        y.to_bits()
    )
}

fn format_set_unit_command_summary(unit_ids: &[i32], command_id: Option<u8>) -> String {
    format!(
        "set_unit_command: count={} first_unit_id={:?} command_id={command_id:?}",
        unit_ids.len(),
        unit_ids.first().copied(),
    )
}

fn format_set_unit_stance_summary(unit_ids: &[i32], stance_id: Option<u8>, enable: bool) -> String {
    format!(
        "set_unit_stance: count={} first_unit_id={:?} stance_id={stance_id:?} enable={enable}",
        unit_ids.len(),
        unit_ids.first().copied(),
    )
}

fn encode_hex_text(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(nibble_to_hex(byte >> 4));
        output.push(nibble_to_hex(byte & 0x0f));
    }
    output
}

fn nibble_to_hex(value: u8) -> char {
    match value & 0x0f {
        0..=9 => (b'0' + (value & 0x0f)) as char,
        10..=15 => (b'a' + ((value & 0x0f) - 10)) as char,
        _ => unreachable!(),
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
