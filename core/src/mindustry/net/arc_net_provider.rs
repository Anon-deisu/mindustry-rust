use super::{
    net::{ConnectFilter, DoneCallback, HostCallback, NetProvider, ProviderEvent},
    net_connection::NetConnection,
    packet::PacketCodec,
    packets::{
        packet_ids, AdminRequestCallPacket, AnnounceCallPacket, AssemblerDroneSpawnedCallPacket,
        AssemblerUnitSpawnedCallPacket, AutoDoorToggleCallPacket, BeginBreakCallPacket,
        BeginPlaceCallPacket, BlockSnapshotCallPacket, BuildDestroyedCallPacket,
        BuildHealthUpdateCallPacket, BuildingControlSelectCallPacket, ClearItemsCallPacket,
        ClearLiquidsCallPacket, ClearObjectivesCallPacket, ClientBinaryPacketReliableCallPacket,
        ClientBinaryPacketUnreliableCallPacket, ClientLogicDataReliableCallPacket,
        ClientLogicDataUnreliableCallPacket, ClientPacketReliableCallPacket,
        ClientPacketUnreliableCallPacket, ClientPlanSnapshotCallPacket,
        ClientPlanSnapshotReceivedCallPacket, ClientSnapshotCallPacket, CommandBuildingCallPacket,
        CommandUnitsCallPacket, CompleteObjectiveCallPacket, ConnectCallPacket,
        ConnectConfirmCallPacket, ConnectPacket, ConstructFinishCallPacket,
        CopyToClipboardCallPacket, CreateBulletCallPacket, CreateMarkerCallPacket,
        CreateWeatherCallPacket, DebugStatusClientCallPacket,
        DebugStatusClientUnreliableCallPacket, DeconstructFinishCallPacket, DeletePlansCallPacket,
        DestroyPayloadCallPacket, DropItemCallPacket, EffectCallPacket, EffectCallPacket2,
        EffectReliableCallPacket, EntitySnapshotCallPacket, FollowUpMenuCallPacket,
        GameOverCallPacket, HiddenSnapshotCallPacket, HideFollowUpMenuCallPacket,
        HideHudTextCallPacket, InfoMessageCallPacket, InfoPopupCallPacket, InfoPopupCallPacket2,
        InfoPopupReliableCallPacket, InfoPopupReliableCallPacket2, InfoToastCallPacket,
        KickCallPacket, KickCallPacket2, LabelCallPacket, LabelCallPacket2,
        LabelReliableCallPacket, LabelReliableCallPacket2, LandingPadLandedCallPacket,
        LogicExplosionCallPacket, MenuCallPacket, MenuChooseCallPacket, OpenUriCallPacket,
        PayloadDroppedCallPacket, PickedBuildPayloadCallPacket, PickedUnitPayloadCallPacket,
        PingCallPacket, PingLocationCallPacket, PingResponseCallPacket, PlayerDisconnectCallPacket,
        PlayerSpawnCallPacket, RemoveMarkerCallPacket, RemoveQueueBlockCallPacket,
        RemoveTileCallPacket, RemoveWorldLabelCallPacket, RequestBlockSnapshotCallPacket,
        RequestBuildPayloadCallPacket, RequestDebugStatusCallPacket, RequestDropPayloadCallPacket,
        RequestItemCallPacket, RequestUnitPayloadCallPacket, ResearchedCallPacket,
        RotateBlockCallPacket, SectorCaptureCallPacket, SendChatMessageCallPacket,
        SendMessageCallPacket, SendMessageCallPacket2, ServerBinaryPacketReliableCallPacket,
        ServerBinaryPacketUnreliableCallPacket, ServerPacketReliableCallPacket,
        ServerPacketUnreliableCallPacket, SetCameraPositionCallPacket, SetFlagCallPacket,
        SetFloorCallPacket, SetHudTextCallPacket, SetHudTextReliableCallPacket, SetItemCallPacket,
        SetItemsCallPacket, SetLiquidCallPacket, SetLiquidsCallPacket, SetMapAreaCallPacket,
        SetObjectivesCallPacket, SetOverlayCallPacket, SetPlayerTeamEditorCallPacket,
        SetPositionCallPacket, SetRuleCallPacket, SetRulesCallPacket, SetTeamCallPacket,
        SetTeamsCallPacket, SetTileBlocksCallPacket, SetTileCallPacket, SetTileFloorsCallPacket,
        SetTileItemsCallPacket, SetTileLiquidsCallPacket, SetTileOverlaysCallPacket,
        SetUnitCommandCallPacket, SetUnitStanceCallPacket, SoundAtCallPacket, SoundCallPacket,
        SpawnEffectCallPacket, StateSnapshotCallPacket, StreamBegin, StreamChunk,
        SyncVariableCallPacket, TakeItemsCallPacket, TextInputCallPacket, TextInputCallPacket2,
        TextInputResultCallPacket, TileConfigCallPacket, TileTapCallPacket, TraceInfoCallPacket,
        TransferInventoryCallPacket, TransferItemEffectCallPacket, TransferItemToCallPacket,
        TransferItemToUnitCallPacket, UnitBlockSpawnCallPacket,
        UnitBuildingControlSelectCallPacket, UnitCapDeathCallPacket, UnitClearCallPacket,
        UnitControlCallPacket, UnitDeathCallPacket, UnitDespawnCallPacket, UnitDestroyCallPacket,
        UnitEnteredPayloadCallPacket, UnitEnvDeathCallPacket, UnitSafeDeathCallPacket,
        UnitSpawnCallPacket, UnitTetherBlockSpawnedCallPacket, UpdateGameOverCallPacket,
        UpdateMarkerCallPacket, UpdateMarkerTextCallPacket, UpdateMarkerTextureCallPacket,
        WarningToastCallPacket, WorldDataBeginCallPacket,
    },
    Host, PacketKind,
};
use crate::mindustry::core::content_loader::ContentLoader;
use crate::mindustry::game::Gamemode;
use crate::mindustry::net::network_io::ServerData;
use crate::mindustry::vars::{DEFAULT_PORT, MULTICAST_GROUP, MULTICAST_PORT};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc, Mutex, MutexGuard, OnceLock,
};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameworkMessage {
    Ping { id: i32, is_reply: bool },
    DiscoverHost,
    KeepAlive,
    RegisterUdp { connection_id: i32 },
    RegisterTcp { connection_id: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketEnvelope {
    Framework(FrameworkMessage),
    Packet {
        id: u8,
        length: u16,
        compression: u8,
        payload: Vec<u8>,
    },
    Raw(Vec<u8>),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PacketSerializer;

impl PacketSerializer {
    pub const FRAMEWORK_ID: u8 = 0xfe;
    pub const COMPRESSION_NONE: u8 = 0;
    pub const COMPRESSION_LZ4: u8 = 1;
    pub const COMPRESS_THRESHOLD: usize = 36;

    fn default_content_loader() -> &'static ContentLoader {
        static DEFAULT_CONTENT_LOADER: OnceLock<ContentLoader> = OnceLock::new();
        DEFAULT_CONTENT_LOADER.get_or_init(ContentLoader::create_base_content_or_panic)
    }

    pub fn write_framework(message: &FrameworkMessage, out: &mut Vec<u8>) {
        match message {
            FrameworkMessage::Ping { id, is_reply } => {
                out.push(0);
                out.extend_from_slice(&id.to_be_bytes());
                out.push(if *is_reply { 1 } else { 0 });
            }
            FrameworkMessage::DiscoverHost => out.push(1),
            FrameworkMessage::KeepAlive => out.push(2),
            FrameworkMessage::RegisterUdp { connection_id } => {
                out.push(3);
                out.extend_from_slice(&connection_id.to_be_bytes());
            }
            FrameworkMessage::RegisterTcp { connection_id } => {
                out.push(4);
                out.extend_from_slice(&connection_id.to_be_bytes());
            }
        }
    }

    pub fn read_framework(bytes: &[u8]) -> Result<FrameworkMessage, SerializerError> {
        let mut cursor = Cursor::new(bytes);
        let id = cursor.u8()?;
        match id {
            0 => Ok(FrameworkMessage::Ping {
                id: cursor.i32()?,
                is_reply: cursor.u8()? == 1,
            }),
            1 => Ok(FrameworkMessage::DiscoverHost),
            2 => Ok(FrameworkMessage::KeepAlive),
            3 => Ok(FrameworkMessage::RegisterUdp {
                connection_id: cursor.i32()?,
            }),
            4 => Ok(FrameworkMessage::RegisterTcp {
                connection_id: cursor.i32()?,
            }),
            _ => Err(SerializerError::UnknownFrameworkMessage(id)),
        }
    }

    pub fn write_envelope(envelope: &PacketEnvelope) -> Vec<u8> {
        match envelope {
            PacketEnvelope::Raw(bytes) => bytes.clone(),
            PacketEnvelope::Framework(message) => {
                let mut out = vec![Self::FRAMEWORK_ID];
                Self::write_framework(message, &mut out);
                out
            }
            PacketEnvelope::Packet {
                id,
                length: _,
                compression,
                payload,
            } => {
                let mut out = vec![*id];
                out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
                out.push(*compression);
                if *compression == Self::COMPRESSION_LZ4 {
                    let compressed = lz4_flex::block::compress(payload);
                    out.extend_from_slice(&compressed);
                } else {
                    out.extend_from_slice(payload);
                }
                out
            }
        }
    }

    pub fn read_envelope(bytes: &[u8]) -> Result<PacketEnvelope, SerializerError> {
        let mut cursor = Cursor::new(bytes);
        let id = cursor.u8()?;
        if id == Self::FRAMEWORK_ID {
            return Ok(PacketEnvelope::Framework(Self::read_framework(
                cursor.remaining(),
            )?));
        }
        let length = cursor.u16()?;
        let compression = cursor.u8()?;
        let payload = if compression == Self::COMPRESSION_NONE {
            cursor.take(length as usize)?.to_vec()
        } else if compression == Self::COMPRESSION_LZ4 {
            lz4_flex::block::decompress(cursor.remaining(), length as usize)
                .map_err(|err| SerializerError::Compression(err.to_string()))?
        } else {
            return Err(SerializerError::UnknownCompression(compression));
        };
        Ok(PacketEnvelope::Packet {
            id,
            length,
            compression,
            payload,
        })
    }

    pub fn write_packet_kind(packet: &PacketKind) -> Result<Vec<u8>, SerializerError> {
        let envelope = Self::packet_kind_to_envelope(packet)?;
        Ok(Self::write_envelope(&envelope))
    }

    pub fn write_packet_kind_with_loader(
        packet: &PacketKind,
        loader: &ContentLoader,
    ) -> Result<Vec<u8>, SerializerError> {
        let envelope = Self::packet_kind_to_envelope_with_loader(packet, loader)?;
        Ok(Self::write_envelope(&envelope))
    }

    pub fn read_packet_kind(bytes: &[u8]) -> Result<PacketKind, SerializerError> {
        let envelope = Self::read_envelope(bytes)?;
        Self::packet_kind_from_envelope(&envelope)
    }

    pub fn read_packet_kind_with_loader(
        bytes: &[u8],
        loader: &ContentLoader,
    ) -> Result<PacketKind, SerializerError> {
        let envelope = Self::read_envelope(bytes)?;
        Self::packet_kind_from_envelope_with_loader(&envelope, loader)
    }

    pub fn packet_kind_to_envelope(packet: &PacketKind) -> Result<PacketEnvelope, SerializerError> {
        Self::packet_kind_to_envelope_with_loader(packet, Self::default_content_loader())
    }

    fn packet_kind_to_envelope_without_loader(
        packet: &PacketKind,
    ) -> Result<PacketEnvelope, SerializerError> {
        if matches!(
            packet,
            PacketKind::ClientPlanSnapshotCallPacket(_)
                | PacketKind::ClientPlanSnapshotReceivedCallPacket(_)
                | PacketKind::ClientSnapshotCallPacket(_)
                | PacketKind::BeginPlaceCallPacket(_)
                | PacketKind::ConstructFinishCallPacket(_)
                | PacketKind::DeconstructFinishCallPacket(_)
                | PacketKind::SetFloorCallPacket(_)
                | PacketKind::SetItemCallPacket(_)
                | PacketKind::SetItemsCallPacket(_)
                | PacketKind::SetLiquidCallPacket(_)
                | PacketKind::SetLiquidsCallPacket(_)
                | PacketKind::SetOverlayCallPacket(_)
                | PacketKind::SetTileCallPacket(_)
                | PacketKind::SetTileBlocksCallPacket(_)
                | PacketKind::SetTileFloorsCallPacket(_)
                | PacketKind::SetTileItemsCallPacket(_)
                | PacketKind::SetTileLiquidsCallPacket(_)
                | PacketKind::SetTileOverlaysCallPacket(_)
                | PacketKind::SetUnitCommandCallPacket(_)
                | PacketKind::SetUnitStanceCallPacket(_)
                | PacketKind::SpawnEffectCallPacket(_)
                | PacketKind::TakeItemsCallPacket(_)
                | PacketKind::TransferItemEffectCallPacket(_)
                | PacketKind::TransferItemToCallPacket(_)
                | PacketKind::TransferItemToUnitCallPacket(_)
        ) {
            return Err(SerializerError::RequiresContentLoader);
        }

        let mut payload = Vec::new();
        let id = match packet {
            PacketKind::Connect(_) | PacketKind::Disconnect(_) => {
                return Err(SerializerError::ExpectedPacketEnvelope)
            }
            PacketKind::StreamBegin(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::STREAM_BEGIN
            }
            PacketKind::StreamChunk(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::STREAM_CHUNK
            }
            PacketKind::ConnectPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CONNECT_PACKET
            }
            PacketKind::AdminRequestCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::ADMIN_REQUEST_CALL_PACKET
            }
            PacketKind::AnnounceCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::ANNOUNCE_CALL_PACKET
            }
            PacketKind::AssemblerDroneSpawnedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::ASSEMBLER_DRONE_SPAWNED_CALL_PACKET
            }
            PacketKind::AssemblerUnitSpawnedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::ASSEMBLER_UNIT_SPAWNED_CALL_PACKET
            }
            PacketKind::AutoDoorToggleCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::AUTO_DOOR_TOGGLE_CALL_PACKET
            }
            PacketKind::BeginBreakCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::BEGIN_BREAK_CALL_PACKET
            }
            PacketKind::BlockSnapshotCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::BLOCK_SNAPSHOT_CALL_PACKET
            }
            PacketKind::BuildDestroyedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::BUILD_DESTROYED_CALL_PACKET
            }
            PacketKind::BuildHealthUpdateCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::BUILD_HEALTH_UPDATE_CALL_PACKET
            }
            PacketKind::BuildingControlSelectCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::BUILDING_CONTROL_SELECT_CALL_PACKET
            }
            PacketKind::ClearItemsCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLEAR_ITEMS_CALL_PACKET
            }
            PacketKind::ClearLiquidsCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLEAR_LIQUIDS_CALL_PACKET
            }
            PacketKind::ClearObjectivesCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLEAR_OBJECTIVES_CALL_PACKET
            }
            PacketKind::ClientBinaryPacketReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLIENT_BINARY_PACKET_RELIABLE_CALL_PACKET
            }
            PacketKind::ClientBinaryPacketUnreliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLIENT_BINARY_PACKET_UNRELIABLE_CALL_PACKET
            }
            PacketKind::ClientLogicDataReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLIENT_LOGIC_DATA_RELIABLE_CALL_PACKET
            }
            PacketKind::ClientLogicDataUnreliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLIENT_LOGIC_DATA_UNRELIABLE_CALL_PACKET
            }
            PacketKind::ClientPacketReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLIENT_PACKET_RELIABLE_CALL_PACKET
            }
            PacketKind::ClientPacketUnreliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CLIENT_PACKET_UNRELIABLE_CALL_PACKET
            }
            PacketKind::CommandBuildingCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::COMMAND_BUILDING_CALL_PACKET
            }
            PacketKind::CommandUnitsCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::COMMAND_UNITS_CALL_PACKET
            }
            PacketKind::CompleteObjectiveCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::COMPLETE_OBJECTIVE_CALL_PACKET
            }
            PacketKind::ConnectCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CONNECT_CALL_PACKET
            }
            PacketKind::ConnectConfirmCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CONNECT_CONFIRM_CALL_PACKET
            }
            PacketKind::CopyToClipboardCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::COPY_TO_CLIPBOARD_CALL_PACKET
            }
            PacketKind::CreateBulletCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CREATE_BULLET_CALL_PACKET
            }
            PacketKind::CreateMarkerCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CREATE_MARKER_CALL_PACKET
            }
            PacketKind::CreateWeatherCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::CREATE_WEATHER_CALL_PACKET
            }
            PacketKind::DebugStatusClientCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::DEBUG_STATUS_CLIENT_CALL_PACKET
            }
            PacketKind::DebugStatusClientUnreliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::DEBUG_STATUS_CLIENT_UNRELIABLE_CALL_PACKET
            }
            PacketKind::DeletePlansCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::DELETE_PLANS_CALL_PACKET
            }
            PacketKind::DestroyPayloadCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::DESTROY_PAYLOAD_CALL_PACKET
            }
            PacketKind::DropItemCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::DROP_ITEM_CALL_PACKET
            }
            PacketKind::EffectCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::EFFECT_CALL_PACKET
            }
            PacketKind::EffectCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::EFFECT_CALL_PACKET2
            }
            PacketKind::EffectReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::EFFECT_RELIABLE_CALL_PACKET
            }
            PacketKind::EntitySnapshotCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::ENTITY_SNAPSHOT_CALL_PACKET
            }
            PacketKind::FollowUpMenuCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::FOLLOW_UP_MENU_CALL_PACKET
            }
            PacketKind::GameOverCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::GAME_OVER_CALL_PACKET
            }
            PacketKind::HiddenSnapshotCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::HIDDEN_SNAPSHOT_CALL_PACKET
            }
            PacketKind::HideFollowUpMenuCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::HIDE_FOLLOW_UP_MENU_CALL_PACKET
            }
            PacketKind::HideHudTextCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::HIDE_HUD_TEXT_CALL_PACKET
            }
            PacketKind::InfoMessageCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::INFO_MESSAGE_CALL_PACKET
            }
            PacketKind::InfoPopupCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::INFO_POPUP_CALL_PACKET
            }
            PacketKind::InfoPopupCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::INFO_POPUP_CALL_PACKET2
            }
            PacketKind::InfoPopupReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::INFO_POPUP_RELIABLE_CALL_PACKET
            }
            PacketKind::InfoPopupReliableCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::INFO_POPUP_RELIABLE_CALL_PACKET2
            }
            PacketKind::InfoToastCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::INFO_TOAST_CALL_PACKET
            }
            PacketKind::KickCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::KICK_CALL_PACKET
            }
            PacketKind::KickCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::KICK_CALL_PACKET2
            }
            PacketKind::LabelCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::LABEL_CALL_PACKET
            }
            PacketKind::LabelCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::LABEL_CALL_PACKET2
            }
            PacketKind::LabelReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::LABEL_RELIABLE_CALL_PACKET
            }
            PacketKind::LabelReliableCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::LABEL_RELIABLE_CALL_PACKET2
            }
            PacketKind::LandingPadLandedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::LANDING_PAD_LANDED_CALL_PACKET
            }
            PacketKind::LogicExplosionCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::LOGIC_EXPLOSION_CALL_PACKET
            }
            PacketKind::MenuCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::MENU_CALL_PACKET
            }
            PacketKind::MenuChooseCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::MENU_CHOOSE_CALL_PACKET
            }
            PacketKind::OpenUriCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::OPEN_URI_CALL_PACKET
            }
            PacketKind::PayloadDroppedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PAYLOAD_DROPPED_CALL_PACKET
            }
            PacketKind::PickedBuildPayloadCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PICKED_BUILD_PAYLOAD_CALL_PACKET
            }
            PacketKind::PickedUnitPayloadCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PICKED_UNIT_PAYLOAD_CALL_PACKET
            }
            PacketKind::PingCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PING_CALL_PACKET
            }
            PacketKind::PingLocationCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::PING_LOCATION_CALL_PACKET
            }
            PacketKind::PingResponseCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PING_RESPONSE_CALL_PACKET
            }
            PacketKind::PlayerDisconnectCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PLAYER_DISCONNECT_CALL_PACKET
            }
            PacketKind::PlayerSpawnCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::PLAYER_SPAWN_CALL_PACKET
            }
            PacketKind::RemoveMarkerCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::REMOVE_MARKER_CALL_PACKET
            }
            PacketKind::RemoveQueueBlockCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::REMOVE_QUEUE_BLOCK_CALL_PACKET
            }
            PacketKind::RemoveTileCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::REMOVE_TILE_CALL_PACKET
            }
            PacketKind::RemoveWorldLabelCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::REMOVE_WORLD_LABEL_CALL_PACKET
            }
            PacketKind::RequestBlockSnapshotCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::REQUEST_BLOCK_SNAPSHOT_CALL_PACKET
            }
            PacketKind::RequestBuildPayloadCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::REQUEST_BUILD_PAYLOAD_CALL_PACKET
            }
            PacketKind::RequestDebugStatusCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::REQUEST_DEBUG_STATUS_CALL_PACKET
            }
            PacketKind::RequestDropPayloadCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::REQUEST_DROP_PAYLOAD_CALL_PACKET
            }
            PacketKind::RequestUnitPayloadCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::REQUEST_UNIT_PAYLOAD_CALL_PACKET
            }
            PacketKind::RotateBlockCallPacket(packet) => {
                packet.write_client_payload(&mut payload)?;
                packet_ids::ROTATE_BLOCK_CALL_PACKET
            }
            PacketKind::SectorCaptureCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SECTOR_CAPTURE_CALL_PACKET
            }
            PacketKind::SendChatMessageCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SEND_CHAT_MESSAGE_CALL_PACKET
            }
            PacketKind::SendMessageCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SEND_MESSAGE_CALL_PACKET
            }
            PacketKind::SendMessageCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SEND_MESSAGE_CALL_PACKET2
            }
            PacketKind::ServerBinaryPacketReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SERVER_BINARY_PACKET_RELIABLE_CALL_PACKET
            }
            PacketKind::ServerBinaryPacketUnreliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SERVER_BINARY_PACKET_UNRELIABLE_CALL_PACKET
            }
            PacketKind::ServerPacketReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SERVER_PACKET_RELIABLE_CALL_PACKET
            }
            PacketKind::ServerPacketUnreliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SERVER_PACKET_UNRELIABLE_CALL_PACKET
            }
            PacketKind::SetCameraPositionCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_CAMERA_POSITION_CALL_PACKET
            }
            PacketKind::SetFlagCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_FLAG_CALL_PACKET
            }
            PacketKind::SetHudTextCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_HUD_TEXT_CALL_PACKET
            }
            PacketKind::SetHudTextReliableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_HUD_TEXT_RELIABLE_CALL_PACKET
            }
            PacketKind::SetMapAreaCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_MAP_AREA_CALL_PACKET
            }
            PacketKind::SetObjectivesCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_OBJECTIVES_CALL_PACKET
            }
            PacketKind::SetPlayerTeamEditorCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_PLAYER_TEAM_EDITOR_CALL_PACKET
            }
            PacketKind::SetPositionCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_POSITION_CALL_PACKET
            }
            PacketKind::SetRuleCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_RULE_CALL_PACKET
            }
            PacketKind::SetRulesCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_RULES_CALL_PACKET
            }
            PacketKind::SetTeamCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_TEAM_CALL_PACKET
            }
            PacketKind::SetTeamsCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SET_TEAMS_CALL_PACKET
            }
            PacketKind::SoundCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SOUND_CALL_PACKET
            }
            PacketKind::SoundAtCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SOUND_AT_CALL_PACKET
            }
            PacketKind::StateSnapshotCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::STATE_SNAPSHOT_CALL_PACKET
            }
            PacketKind::SyncVariableCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::SYNC_VARIABLE_CALL_PACKET
            }
            PacketKind::TextInputCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TEXT_INPUT_CALL_PACKET
            }
            PacketKind::TextInputCallPacket2(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TEXT_INPUT_CALL_PACKET2
            }
            PacketKind::TextInputResultCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TEXT_INPUT_RESULT_CALL_PACKET
            }
            PacketKind::TileConfigCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TILE_CONFIG_CALL_PACKET
            }
            PacketKind::TileTapCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TILE_TAP_CALL_PACKET
            }
            PacketKind::TraceInfoCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TRACE_INFO_CALL_PACKET
            }
            PacketKind::TransferInventoryCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::TRANSFER_INVENTORY_CALL_PACKET
            }
            PacketKind::UnitBlockSpawnCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_BLOCK_SPAWN_CALL_PACKET
            }
            PacketKind::UnitBuildingControlSelectCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_BUILDING_CONTROL_SELECT_CALL_PACKET
            }
            PacketKind::UnitCapDeathCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_CAP_DEATH_CALL_PACKET
            }
            PacketKind::UnitClearCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_CLEAR_CALL_PACKET
            }
            PacketKind::UnitControlCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_CONTROL_CALL_PACKET
            }
            PacketKind::UnitDeathCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_DEATH_CALL_PACKET
            }
            PacketKind::UnitDespawnCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_DESPAWN_CALL_PACKET
            }
            PacketKind::UnitDestroyCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_DESTROY_CALL_PACKET
            }
            PacketKind::UnitEnteredPayloadCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_ENTERED_PAYLOAD_CALL_PACKET
            }
            PacketKind::UnitEnvDeathCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_ENV_DEATH_CALL_PACKET
            }
            PacketKind::UnitSafeDeathCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_SAFE_DEATH_CALL_PACKET
            }
            PacketKind::UnitSpawnCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_SPAWN_CALL_PACKET
            }
            PacketKind::UnitTetherBlockSpawnedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_TETHER_BLOCK_SPAWNED_CALL_PACKET
            }
            PacketKind::UpdateGameOverCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_GAME_OVER_CALL_PACKET
            }
            PacketKind::UpdateMarkerCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_MARKER_CALL_PACKET
            }
            PacketKind::UpdateMarkerTextCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_MARKER_TEXT_CALL_PACKET
            }
            PacketKind::UpdateMarkerTextureCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_MARKER_TEXTURE_CALL_PACKET
            }
            PacketKind::WarningToastCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::WARNING_TOAST_CALL_PACKET
            }
            PacketKind::WorldDataBeginCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::WORLD_DATA_BEGIN_CALL_PACKET
            }
            PacketKind::Streamable(stream) => {
                payload.extend_from_slice(&stream.stream);
                packet_ids::WORLD_STREAM
            }
            PacketKind::ClientPlanSnapshotCallPacket(_)
            | PacketKind::ClientPlanSnapshotReceivedCallPacket(_)
            | PacketKind::ClientSnapshotCallPacket(_)
            | PacketKind::BeginPlaceCallPacket(_)
            | PacketKind::ConstructFinishCallPacket(_)
            | PacketKind::DeconstructFinishCallPacket(_)
            | PacketKind::RequestItemCallPacket(_)
            | PacketKind::ResearchedCallPacket(_)
            | PacketKind::SetFloorCallPacket(_)
            | PacketKind::SetItemCallPacket(_)
            | PacketKind::SetItemsCallPacket(_)
            | PacketKind::SetLiquidCallPacket(_)
            | PacketKind::SetLiquidsCallPacket(_)
            | PacketKind::SetOverlayCallPacket(_)
            | PacketKind::SetTileCallPacket(_)
            | PacketKind::SetTileBlocksCallPacket(_)
            | PacketKind::SetTileFloorsCallPacket(_)
            | PacketKind::SetTileItemsCallPacket(_)
            | PacketKind::SetTileLiquidsCallPacket(_)
            | PacketKind::SetTileOverlaysCallPacket(_)
            | PacketKind::SetUnitCommandCallPacket(_)
            | PacketKind::SetUnitStanceCallPacket(_)
            | PacketKind::SpawnEffectCallPacket(_)
            | PacketKind::TakeItemsCallPacket(_) => {
                return Err(SerializerError::RequiresContentLoader);
            }
            PacketKind::TransferItemEffectCallPacket(_)
            | PacketKind::TransferItemToCallPacket(_)
            | PacketKind::TransferItemToUnitCallPacket(_) => {
                return Err(SerializerError::RequiresContentLoader);
            }
            PacketKind::Other { id, .. } => return Err(SerializerError::UnsupportedPacketId(*id)),
        };

        Self::packet_payload_to_envelope(packet, id, payload)
    }

    pub fn packet_kind_to_envelope_with_loader(
        packet: &PacketKind,
        loader: &ContentLoader,
    ) -> Result<PacketEnvelope, SerializerError> {
        let mut payload = Vec::new();
        let id = match packet {
            PacketKind::Connect(_) | PacketKind::Disconnect(_) => {
                return Err(SerializerError::ExpectedPacketEnvelope)
            }
            PacketKind::BeginPlaceCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::BEGIN_PLACE_CALL_PACKET
            }
            PacketKind::ClientPlanSnapshotCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::CLIENT_PLAN_SNAPSHOT_CALL_PACKET
            }
            PacketKind::ClientPlanSnapshotReceivedCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::CLIENT_PLAN_SNAPSHOT_RECEIVED_CALL_PACKET
            }
            PacketKind::ClientSnapshotCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::CLIENT_SNAPSHOT_CALL_PACKET
            }
            PacketKind::ConstructFinishCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::CONSTRUCT_FINISH_CALL_PACKET
            }
            PacketKind::DeconstructFinishCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::DECONSTRUCT_FINISH_CALL_PACKET
            }
            PacketKind::SetFloorCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_FLOOR_CALL_PACKET
            }
            PacketKind::SetItemCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_ITEM_CALL_PACKET
            }
            PacketKind::SetItemsCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_ITEMS_CALL_PACKET
            }
            PacketKind::SetLiquidCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_LIQUID_CALL_PACKET
            }
            PacketKind::SetLiquidsCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_LIQUIDS_CALL_PACKET
            }
            PacketKind::SetOverlayCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_OVERLAY_CALL_PACKET
            }
            PacketKind::SetTileCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_TILE_CALL_PACKET
            }
            PacketKind::SetTileBlocksCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_TILE_BLOCKS_CALL_PACKET
            }
            PacketKind::SetTileFloorsCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_TILE_FLOORS_CALL_PACKET
            }
            PacketKind::SetTileItemsCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_TILE_ITEMS_CALL_PACKET
            }
            PacketKind::SetTileLiquidsCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_TILE_LIQUIDS_CALL_PACKET
            }
            PacketKind::SetTileOverlaysCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SET_TILE_OVERLAYS_CALL_PACKET
            }
            PacketKind::SetUnitCommandCallPacket(packet) => {
                packet.write_client_payload_with_loader(&mut payload, loader)?;
                packet_ids::SET_UNIT_COMMAND_CALL_PACKET
            }
            PacketKind::SetUnitStanceCallPacket(packet) => {
                packet.write_client_payload_with_loader(&mut payload, loader)?;
                packet_ids::SET_UNIT_STANCE_CALL_PACKET
            }
            PacketKind::SpawnEffectCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::SPAWN_EFFECT_CALL_PACKET
            }
            PacketKind::TakeItemsCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::TAKE_ITEMS_CALL_PACKET
            }
            PacketKind::RequestItemCallPacket(packet) => {
                packet.write_client_payload_with_loader(&mut payload, loader)?;
                packet_ids::REQUEST_ITEM_CALL_PACKET
            }
            PacketKind::ResearchedCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::RESEARCHED_CALL_PACKET
            }
            PacketKind::TransferItemEffectCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::TRANSFER_ITEM_EFFECT_CALL_PACKET
            }
            PacketKind::TransferItemToCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::TRANSFER_ITEM_TO_CALL_PACKET
            }
            PacketKind::TransferItemToUnitCallPacket(packet) => {
                packet.write_to_with_loader(&mut payload, loader)?;
                packet_ids::TRANSFER_ITEM_TO_UNIT_CALL_PACKET
            }
            PacketKind::UnitDespawnCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_DESPAWN_CALL_PACKET
            }
            PacketKind::UnitDestroyCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_DESTROY_CALL_PACKET
            }
            PacketKind::UnitEnteredPayloadCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_ENTERED_PAYLOAD_CALL_PACKET
            }
            PacketKind::UnitEnvDeathCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_ENV_DEATH_CALL_PACKET
            }
            PacketKind::UnitSafeDeathCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_SAFE_DEATH_CALL_PACKET
            }
            PacketKind::UnitSpawnCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_SPAWN_CALL_PACKET
            }
            PacketKind::UnitTetherBlockSpawnedCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UNIT_TETHER_BLOCK_SPAWNED_CALL_PACKET
            }
            PacketKind::UpdateGameOverCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_GAME_OVER_CALL_PACKET
            }
            PacketKind::UpdateMarkerCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_MARKER_CALL_PACKET
            }
            PacketKind::UpdateMarkerTextCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_MARKER_TEXT_CALL_PACKET
            }
            PacketKind::UpdateMarkerTextureCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::UPDATE_MARKER_TEXTURE_CALL_PACKET
            }
            PacketKind::WarningToastCallPacket(packet) => {
                packet.write_to(&mut payload)?;
                packet_ids::WARNING_TOAST_CALL_PACKET
            }
            _ => return Self::packet_kind_to_envelope_without_loader(packet),
        };

        Self::packet_payload_to_envelope(packet, id, payload)
    }

    fn packet_payload_to_envelope(
        packet: &PacketKind,
        id: u8,
        payload: Vec<u8>,
    ) -> Result<PacketEnvelope, SerializerError> {
        Ok(PacketEnvelope::Packet {
            id,
            length: payload
                .len()
                .try_into()
                .map_err(|_| SerializerError::PayloadTooLarge(payload.len()))?,
            compression: if payload.len() < Self::COMPRESS_THRESHOLD
                || matches!(packet, PacketKind::StreamChunk(_))
            {
                Self::COMPRESSION_NONE
            } else {
                Self::COMPRESSION_LZ4
            },
            payload,
        })
    }

    pub fn packet_kind_from_envelope(
        envelope: &PacketEnvelope,
    ) -> Result<PacketKind, SerializerError> {
        Self::packet_kind_from_envelope_with_loader(envelope, Self::default_content_loader())
    }

    fn packet_kind_from_envelope_without_loader(
        envelope: &PacketEnvelope,
    ) -> Result<PacketKind, SerializerError> {
        if let PacketEnvelope::Packet { id, .. } = envelope {
            if matches!(
                *id,
                packet_ids::CLIENT_PLAN_SNAPSHOT_CALL_PACKET
                    | packet_ids::CLIENT_PLAN_SNAPSHOT_RECEIVED_CALL_PACKET
                    | packet_ids::CLIENT_SNAPSHOT_CALL_PACKET
                    | packet_ids::BEGIN_PLACE_CALL_PACKET
                    | packet_ids::CONSTRUCT_FINISH_CALL_PACKET
                    | packet_ids::DECONSTRUCT_FINISH_CALL_PACKET
                    | packet_ids::REQUEST_ITEM_CALL_PACKET
                    | packet_ids::RESEARCHED_CALL_PACKET
                    | packet_ids::SET_FLOOR_CALL_PACKET
                    | packet_ids::SET_ITEM_CALL_PACKET
                    | packet_ids::SET_ITEMS_CALL_PACKET
                    | packet_ids::SET_LIQUID_CALL_PACKET
                    | packet_ids::SET_LIQUIDS_CALL_PACKET
                    | packet_ids::SET_OVERLAY_CALL_PACKET
                    | packet_ids::SET_TILE_CALL_PACKET
                    | packet_ids::SET_TILE_BLOCKS_CALL_PACKET
                    | packet_ids::SET_TILE_FLOORS_CALL_PACKET
                    | packet_ids::SET_TILE_ITEMS_CALL_PACKET
                    | packet_ids::SET_TILE_LIQUIDS_CALL_PACKET
                    | packet_ids::SET_TILE_OVERLAYS_CALL_PACKET
                    | packet_ids::SET_UNIT_COMMAND_CALL_PACKET
                    | packet_ids::SET_UNIT_STANCE_CALL_PACKET
                    | packet_ids::SPAWN_EFFECT_CALL_PACKET
                    | packet_ids::TAKE_ITEMS_CALL_PACKET
            ) {
                return Err(SerializerError::RequiresContentLoader);
            }
        }

        match envelope {
            PacketEnvelope::Packet { id, payload, .. } => {
                let mut cursor = payload.as_slice();
                match *id {
                    packet_ids::STREAM_BEGIN => Ok(PacketKind::StreamBegin(
                        StreamBegin::read_from(&mut cursor)?,
                    )),
                    packet_ids::STREAM_CHUNK => Ok(PacketKind::StreamChunk(
                        StreamChunk::read_from(&mut cursor)?,
                    )),
                    packet_ids::WORLD_STREAM => Ok(PacketKind::Streamable(
                        crate::mindustry::net::streamable::Streamable {
                            stream: payload.clone(),
                        },
                    )),
                    packet_ids::CONNECT_PACKET => Ok(PacketKind::ConnectPacket(
                        ConnectPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::ADMIN_REQUEST_CALL_PACKET => {
                        Ok(PacketKind::AdminRequestCallPacket(
                            AdminRequestCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::ANNOUNCE_CALL_PACKET => Ok(PacketKind::AnnounceCallPacket(
                        AnnounceCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::ASSEMBLER_DRONE_SPAWNED_CALL_PACKET => {
                        Ok(PacketKind::AssemblerDroneSpawnedCallPacket(
                            AssemblerDroneSpawnedCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::ASSEMBLER_UNIT_SPAWNED_CALL_PACKET => {
                        Ok(PacketKind::AssemblerUnitSpawnedCallPacket(
                            AssemblerUnitSpawnedCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::AUTO_DOOR_TOGGLE_CALL_PACKET => {
                        Ok(PacketKind::AutoDoorToggleCallPacket(
                            AutoDoorToggleCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::BEGIN_BREAK_CALL_PACKET => Ok(PacketKind::BeginBreakCallPacket(
                        BeginBreakCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::BLOCK_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::BlockSnapshotCallPacket(
                            BlockSnapshotCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::BUILD_DESTROYED_CALL_PACKET => {
                        Ok(PacketKind::BuildDestroyedCallPacket(
                            BuildDestroyedCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::BUILD_HEALTH_UPDATE_CALL_PACKET => {
                        Ok(PacketKind::BuildHealthUpdateCallPacket(
                            BuildHealthUpdateCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::BUILDING_CONTROL_SELECT_CALL_PACKET => {
                        Ok(PacketKind::BuildingControlSelectCallPacket(
                            BuildingControlSelectCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLEAR_ITEMS_CALL_PACKET => Ok(PacketKind::ClearItemsCallPacket(
                        ClearItemsCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::CLEAR_LIQUIDS_CALL_PACKET => {
                        Ok(PacketKind::ClearLiquidsCallPacket(
                            ClearLiquidsCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLEAR_OBJECTIVES_CALL_PACKET => {
                        Ok(PacketKind::ClearObjectivesCallPacket(
                            ClearObjectivesCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLIENT_BINARY_PACKET_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ClientBinaryPacketReliableCallPacket(
                            ClientBinaryPacketReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLIENT_BINARY_PACKET_UNRELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ClientBinaryPacketUnreliableCallPacket(
                            ClientBinaryPacketUnreliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLIENT_LOGIC_DATA_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ClientLogicDataReliableCallPacket(
                            ClientLogicDataReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLIENT_LOGIC_DATA_UNRELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ClientLogicDataUnreliableCallPacket(
                            ClientLogicDataUnreliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLIENT_PACKET_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ClientPacketReliableCallPacket(
                            ClientPacketReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CLIENT_PACKET_UNRELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ClientPacketUnreliableCallPacket(
                            ClientPacketUnreliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::COMMAND_BUILDING_CALL_PACKET => {
                        Ok(PacketKind::CommandBuildingCallPacket(
                            CommandBuildingCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::COMMAND_UNITS_CALL_PACKET => {
                        Ok(PacketKind::CommandUnitsCallPacket(
                            CommandUnitsCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::COMPLETE_OBJECTIVE_CALL_PACKET => {
                        Ok(PacketKind::CompleteObjectiveCallPacket(
                            CompleteObjectiveCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CONNECT_CALL_PACKET => Ok(PacketKind::ConnectCallPacket(
                        ConnectCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::CONNECT_CONFIRM_CALL_PACKET => {
                        Ok(PacketKind::ConnectConfirmCallPacket(
                            ConnectConfirmCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::COPY_TO_CLIPBOARD_CALL_PACKET => {
                        Ok(PacketKind::CopyToClipboardCallPacket(
                            CopyToClipboardCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CREATE_BULLET_CALL_PACKET => {
                        Ok(PacketKind::CreateBulletCallPacket(
                            CreateBulletCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CREATE_MARKER_CALL_PACKET => {
                        Ok(PacketKind::CreateMarkerCallPacket(
                            CreateMarkerCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::CREATE_WEATHER_CALL_PACKET => {
                        Ok(PacketKind::CreateWeatherCallPacket(
                            CreateWeatherCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::DEBUG_STATUS_CLIENT_CALL_PACKET => {
                        Ok(PacketKind::DebugStatusClientCallPacket(
                            DebugStatusClientCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::DEBUG_STATUS_CLIENT_UNRELIABLE_CALL_PACKET => {
                        Ok(PacketKind::DebugStatusClientUnreliableCallPacket(
                            DebugStatusClientUnreliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::DELETE_PLANS_CALL_PACKET => Ok(PacketKind::DeletePlansCallPacket(
                        DeletePlansCallPacket::read_from_client_payload(&mut cursor)?,
                    )),
                    packet_ids::DESTROY_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::DestroyPayloadCallPacket(
                            DestroyPayloadCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::DROP_ITEM_CALL_PACKET => Ok(PacketKind::DropItemCallPacket(
                        DropItemCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::EFFECT_CALL_PACKET => Ok(PacketKind::EffectCallPacket(
                        EffectCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::EFFECT_CALL_PACKET2 => Ok(PacketKind::EffectCallPacket2(
                        EffectCallPacket2::read_from(&mut cursor)?,
                    )),
                    packet_ids::EFFECT_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::EffectReliableCallPacket(
                            EffectReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::ENTITY_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::EntitySnapshotCallPacket(
                            EntitySnapshotCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::FOLLOW_UP_MENU_CALL_PACKET => {
                        Ok(PacketKind::FollowUpMenuCallPacket(
                            FollowUpMenuCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::GAME_OVER_CALL_PACKET => Ok(PacketKind::GameOverCallPacket(
                        GameOverCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::HIDDEN_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::HiddenSnapshotCallPacket(
                            HiddenSnapshotCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::HIDE_FOLLOW_UP_MENU_CALL_PACKET => {
                        Ok(PacketKind::HideFollowUpMenuCallPacket(
                            HideFollowUpMenuCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::HIDE_HUD_TEXT_CALL_PACKET => Ok(PacketKind::HideHudTextCallPacket(
                        HideHudTextCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::INFO_MESSAGE_CALL_PACKET => Ok(PacketKind::InfoMessageCallPacket(
                        InfoMessageCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::INFO_POPUP_CALL_PACKET => Ok(PacketKind::InfoPopupCallPacket(
                        InfoPopupCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::INFO_POPUP_CALL_PACKET2 => Ok(PacketKind::InfoPopupCallPacket2(
                        InfoPopupCallPacket2::read_from(&mut cursor)?,
                    )),
                    packet_ids::INFO_POPUP_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::InfoPopupReliableCallPacket(
                            InfoPopupReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::INFO_POPUP_RELIABLE_CALL_PACKET2 => {
                        Ok(PacketKind::InfoPopupReliableCallPacket2(
                            InfoPopupReliableCallPacket2::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::INFO_TOAST_CALL_PACKET => Ok(PacketKind::InfoToastCallPacket(
                        InfoToastCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::KICK_CALL_PACKET => Ok(PacketKind::KickCallPacket(
                        KickCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::KICK_CALL_PACKET2 => Ok(PacketKind::KickCallPacket2(
                        KickCallPacket2::read_from(&mut cursor)?,
                    )),
                    packet_ids::LABEL_CALL_PACKET => Ok(PacketKind::LabelCallPacket(
                        LabelCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::LABEL_CALL_PACKET2 => Ok(PacketKind::LabelCallPacket2(
                        LabelCallPacket2::read_from(&mut cursor)?,
                    )),
                    packet_ids::LABEL_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::LabelReliableCallPacket(
                            LabelReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::LABEL_RELIABLE_CALL_PACKET2 => {
                        Ok(PacketKind::LabelReliableCallPacket2(
                            LabelReliableCallPacket2::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::LANDING_PAD_LANDED_CALL_PACKET => {
                        Ok(PacketKind::LandingPadLandedCallPacket(
                            LandingPadLandedCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::LOGIC_EXPLOSION_CALL_PACKET => {
                        Ok(PacketKind::LogicExplosionCallPacket(
                            LogicExplosionCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::MENU_CALL_PACKET => Ok(PacketKind::MenuCallPacket(
                        MenuCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::MENU_CHOOSE_CALL_PACKET => Ok(PacketKind::MenuChooseCallPacket(
                        MenuChooseCallPacket::read_from_client_payload(&mut cursor)?,
                    )),
                    packet_ids::OPEN_URI_CALL_PACKET => Ok(PacketKind::OpenUriCallPacket(
                        OpenUriCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::PAYLOAD_DROPPED_CALL_PACKET => {
                        Ok(PacketKind::PayloadDroppedCallPacket(
                            PayloadDroppedCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::PICKED_BUILD_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::PickedBuildPayloadCallPacket(
                            PickedBuildPayloadCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::PICKED_UNIT_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::PickedUnitPayloadCallPacket(
                            PickedUnitPayloadCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::PING_CALL_PACKET => Ok(PacketKind::PingCallPacket(
                        PingCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::PING_LOCATION_CALL_PACKET => {
                        Ok(PacketKind::PingLocationCallPacket(
                            PingLocationCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::PING_RESPONSE_CALL_PACKET => {
                        Ok(PacketKind::PingResponseCallPacket(
                            PingResponseCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::PLAYER_DISCONNECT_CALL_PACKET => {
                        Ok(PacketKind::PlayerDisconnectCallPacket(
                            PlayerDisconnectCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::PLAYER_SPAWN_CALL_PACKET => Ok(PacketKind::PlayerSpawnCallPacket(
                        PlayerSpawnCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::REMOVE_MARKER_CALL_PACKET => {
                        Ok(PacketKind::RemoveMarkerCallPacket(
                            RemoveMarkerCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::REMOVE_QUEUE_BLOCK_CALL_PACKET => {
                        Ok(PacketKind::RemoveQueueBlockCallPacket(
                            RemoveQueueBlockCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::REMOVE_TILE_CALL_PACKET => Ok(PacketKind::RemoveTileCallPacket(
                        RemoveTileCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::REMOVE_WORLD_LABEL_CALL_PACKET => {
                        Ok(PacketKind::RemoveWorldLabelCallPacket(
                            RemoveWorldLabelCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::REQUEST_BLOCK_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::RequestBlockSnapshotCallPacket(
                            RequestBlockSnapshotCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::REQUEST_BUILD_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::RequestBuildPayloadCallPacket(
                            RequestBuildPayloadCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::REQUEST_DEBUG_STATUS_CALL_PACKET => {
                        Ok(PacketKind::RequestDebugStatusCallPacket(
                            RequestDebugStatusCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::REQUEST_DROP_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::RequestDropPayloadCallPacket(
                            RequestDropPayloadCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::REQUEST_UNIT_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::RequestUnitPayloadCallPacket(
                            RequestUnitPayloadCallPacket::read_from_client_payload(&mut cursor)?,
                        ))
                    }
                    packet_ids::ROTATE_BLOCK_CALL_PACKET => Ok(PacketKind::RotateBlockCallPacket(
                        RotateBlockCallPacket::read_from_client_payload(&mut cursor)?,
                    )),
                    packet_ids::SECTOR_CAPTURE_CALL_PACKET => {
                        Ok(PacketKind::SectorCaptureCallPacket(
                            SectorCaptureCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SEND_CHAT_MESSAGE_CALL_PACKET => {
                        Ok(PacketKind::SendChatMessageCallPacket(
                            SendChatMessageCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SEND_MESSAGE_CALL_PACKET => Ok(PacketKind::SendMessageCallPacket(
                        SendMessageCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SEND_MESSAGE_CALL_PACKET2 => {
                        Ok(PacketKind::SendMessageCallPacket2(
                            SendMessageCallPacket2::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SERVER_BINARY_PACKET_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ServerBinaryPacketReliableCallPacket(
                            ServerBinaryPacketReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SERVER_BINARY_PACKET_UNRELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ServerBinaryPacketUnreliableCallPacket(
                            ServerBinaryPacketUnreliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SERVER_PACKET_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ServerPacketReliableCallPacket(
                            ServerPacketReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SERVER_PACKET_UNRELIABLE_CALL_PACKET => {
                        Ok(PacketKind::ServerPacketUnreliableCallPacket(
                            ServerPacketUnreliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SET_CAMERA_POSITION_CALL_PACKET => {
                        Ok(PacketKind::SetCameraPositionCallPacket(
                            SetCameraPositionCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SET_FLAG_CALL_PACKET => Ok(PacketKind::SetFlagCallPacket(
                        SetFlagCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_HUD_TEXT_CALL_PACKET => Ok(PacketKind::SetHudTextCallPacket(
                        SetHudTextCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_HUD_TEXT_RELIABLE_CALL_PACKET => {
                        Ok(PacketKind::SetHudTextReliableCallPacket(
                            SetHudTextReliableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SET_OBJECTIVES_CALL_PACKET => {
                        Ok(PacketKind::SetObjectivesCallPacket(
                            SetObjectivesCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SET_PLAYER_TEAM_EDITOR_CALL_PACKET => {
                        Ok(PacketKind::SetPlayerTeamEditorCallPacket(
                            SetPlayerTeamEditorCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SET_POSITION_CALL_PACKET => Ok(PacketKind::SetPositionCallPacket(
                        SetPositionCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_MAP_AREA_CALL_PACKET => Ok(PacketKind::SetMapAreaCallPacket(
                        SetMapAreaCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_RULE_CALL_PACKET => Ok(PacketKind::SetRuleCallPacket(
                        SetRuleCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_RULES_CALL_PACKET => Ok(PacketKind::SetRulesCallPacket(
                        SetRulesCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_TEAM_CALL_PACKET => Ok(PacketKind::SetTeamCallPacket(
                        SetTeamCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SET_TEAMS_CALL_PACKET => Ok(PacketKind::SetTeamsCallPacket(
                        SetTeamsCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SOUND_CALL_PACKET => Ok(PacketKind::SoundCallPacket(
                        SoundCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::SOUND_AT_CALL_PACKET => Ok(PacketKind::SoundAtCallPacket(
                        SoundAtCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::STATE_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::StateSnapshotCallPacket(
                            StateSnapshotCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::SYNC_VARIABLE_CALL_PACKET => {
                        Ok(PacketKind::SyncVariableCallPacket(
                            SyncVariableCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::TEXT_INPUT_CALL_PACKET => Ok(PacketKind::TextInputCallPacket(
                        TextInputCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::TEXT_INPUT_CALL_PACKET2 => Ok(PacketKind::TextInputCallPacket2(
                        TextInputCallPacket2::read_from(&mut cursor)?,
                    )),
                    packet_ids::TEXT_INPUT_RESULT_CALL_PACKET => {
                        Ok(PacketKind::TextInputResultCallPacket(
                            TextInputResultCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::TILE_CONFIG_CALL_PACKET => Ok(PacketKind::TileConfigCallPacket(
                        TileConfigCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::TILE_TAP_CALL_PACKET => Ok(PacketKind::TileTapCallPacket(
                        TileTapCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::TRACE_INFO_CALL_PACKET => Ok(PacketKind::TraceInfoCallPacket(
                        TraceInfoCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::TRANSFER_INVENTORY_CALL_PACKET => {
                        Ok(PacketKind::TransferInventoryCallPacket(
                            TransferInventoryCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_BLOCK_SPAWN_CALL_PACKET => {
                        Ok(PacketKind::UnitBlockSpawnCallPacket(
                            UnitBlockSpawnCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_BUILDING_CONTROL_SELECT_CALL_PACKET => {
                        Ok(PacketKind::UnitBuildingControlSelectCallPacket(
                            UnitBuildingControlSelectCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_CAP_DEATH_CALL_PACKET => {
                        Ok(PacketKind::UnitCapDeathCallPacket(
                            UnitCapDeathCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_CLEAR_CALL_PACKET => Ok(PacketKind::UnitClearCallPacket(
                        UnitClearCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::UNIT_CONTROL_CALL_PACKET => Ok(PacketKind::UnitControlCallPacket(
                        UnitControlCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::UNIT_DEATH_CALL_PACKET => Ok(PacketKind::UnitDeathCallPacket(
                        UnitDeathCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::UNIT_DESPAWN_CALL_PACKET => Ok(PacketKind::UnitDespawnCallPacket(
                        UnitDespawnCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::UNIT_DESTROY_CALL_PACKET => Ok(PacketKind::UnitDestroyCallPacket(
                        UnitDestroyCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::UNIT_ENTERED_PAYLOAD_CALL_PACKET => {
                        Ok(PacketKind::UnitEnteredPayloadCallPacket(
                            UnitEnteredPayloadCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_ENV_DEATH_CALL_PACKET => {
                        Ok(PacketKind::UnitEnvDeathCallPacket(
                            UnitEnvDeathCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_SAFE_DEATH_CALL_PACKET => {
                        Ok(PacketKind::UnitSafeDeathCallPacket(
                            UnitSafeDeathCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UNIT_SPAWN_CALL_PACKET => Ok(PacketKind::UnitSpawnCallPacket(
                        UnitSpawnCallPacket::read_from(&mut cursor)?,
                    )),
                    packet_ids::UNIT_TETHER_BLOCK_SPAWNED_CALL_PACKET => {
                        Ok(PacketKind::UnitTetherBlockSpawnedCallPacket(
                            UnitTetherBlockSpawnedCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UPDATE_GAME_OVER_CALL_PACKET => {
                        Ok(PacketKind::UpdateGameOverCallPacket(
                            UpdateGameOverCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UPDATE_MARKER_CALL_PACKET => {
                        Ok(PacketKind::UpdateMarkerCallPacket(
                            UpdateMarkerCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UPDATE_MARKER_TEXT_CALL_PACKET => {
                        Ok(PacketKind::UpdateMarkerTextCallPacket(
                            UpdateMarkerTextCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::UPDATE_MARKER_TEXTURE_CALL_PACKET => {
                        Ok(PacketKind::UpdateMarkerTextureCallPacket(
                            UpdateMarkerTextureCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::WARNING_TOAST_CALL_PACKET => {
                        Ok(PacketKind::WarningToastCallPacket(
                            WarningToastCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    packet_ids::WORLD_DATA_BEGIN_CALL_PACKET => {
                        Ok(PacketKind::WorldDataBeginCallPacket(
                            WorldDataBeginCallPacket::read_from(&mut cursor)?,
                        ))
                    }
                    other => Err(SerializerError::UnsupportedPacketId(other)),
                }
            }
            PacketEnvelope::Framework(_) => Err(SerializerError::ExpectedPacketEnvelope),
            PacketEnvelope::Raw(_) => Err(SerializerError::ExpectedPacketEnvelope),
        }
    }

    pub fn packet_kind_from_envelope_with_loader(
        envelope: &PacketEnvelope,
        loader: &ContentLoader,
    ) -> Result<PacketKind, SerializerError> {
        match envelope {
            PacketEnvelope::Packet { id, payload, .. } => {
                let mut cursor = payload.as_slice();
                match *id {
                    packet_ids::CLIENT_PLAN_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::ClientPlanSnapshotCallPacket(
                            ClientPlanSnapshotCallPacket::read_from_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    packet_ids::CLIENT_PLAN_SNAPSHOT_RECEIVED_CALL_PACKET => {
                        Ok(PacketKind::ClientPlanSnapshotReceivedCallPacket(
                            ClientPlanSnapshotReceivedCallPacket::read_from_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    packet_ids::CLIENT_SNAPSHOT_CALL_PACKET => {
                        Ok(PacketKind::ClientSnapshotCallPacket(
                            ClientSnapshotCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::BEGIN_PLACE_CALL_PACKET => Ok(PacketKind::BeginPlaceCallPacket(
                        BeginPlaceCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::CONSTRUCT_FINISH_CALL_PACKET => {
                        Ok(PacketKind::ConstructFinishCallPacket(
                            ConstructFinishCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::DECONSTRUCT_FINISH_CALL_PACKET => {
                        Ok(PacketKind::DeconstructFinishCallPacket(
                            DeconstructFinishCallPacket::read_from_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    packet_ids::SET_FLOOR_CALL_PACKET => Ok(PacketKind::SetFloorCallPacket(
                        SetFloorCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_ITEM_CALL_PACKET => Ok(PacketKind::SetItemCallPacket(
                        SetItemCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_ITEMS_CALL_PACKET => Ok(PacketKind::SetItemsCallPacket(
                        SetItemsCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_LIQUID_CALL_PACKET => Ok(PacketKind::SetLiquidCallPacket(
                        SetLiquidCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_LIQUIDS_CALL_PACKET => Ok(PacketKind::SetLiquidsCallPacket(
                        SetLiquidsCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_OVERLAY_CALL_PACKET => Ok(PacketKind::SetOverlayCallPacket(
                        SetOverlayCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_TILE_CALL_PACKET => Ok(PacketKind::SetTileCallPacket(
                        SetTileCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::SET_TILE_BLOCKS_CALL_PACKET => {
                        Ok(PacketKind::SetTileBlocksCallPacket(
                            SetTileBlocksCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::SET_TILE_FLOORS_CALL_PACKET => {
                        Ok(PacketKind::SetTileFloorsCallPacket(
                            SetTileFloorsCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::SET_TILE_ITEMS_CALL_PACKET => {
                        Ok(PacketKind::SetTileItemsCallPacket(
                            SetTileItemsCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::SET_TILE_LIQUIDS_CALL_PACKET => {
                        Ok(PacketKind::SetTileLiquidsCallPacket(
                            SetTileLiquidsCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::SET_TILE_OVERLAYS_CALL_PACKET => {
                        Ok(PacketKind::SetTileOverlaysCallPacket(
                            SetTileOverlaysCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::SET_UNIT_COMMAND_CALL_PACKET => {
                        Ok(PacketKind::SetUnitCommandCallPacket(
                            SetUnitCommandCallPacket::read_from_client_payload_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    packet_ids::SET_UNIT_STANCE_CALL_PACKET => {
                        Ok(PacketKind::SetUnitStanceCallPacket(
                            SetUnitStanceCallPacket::read_from_client_payload_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    packet_ids::SPAWN_EFFECT_CALL_PACKET => Ok(PacketKind::SpawnEffectCallPacket(
                        SpawnEffectCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::TAKE_ITEMS_CALL_PACKET => Ok(PacketKind::TakeItemsCallPacket(
                        TakeItemsCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::REQUEST_ITEM_CALL_PACKET => Ok(PacketKind::RequestItemCallPacket(
                        RequestItemCallPacket::read_from_client_payload_with_loader(
                            &mut cursor,
                            loader,
                        )?,
                    )),
                    packet_ids::RESEARCHED_CALL_PACKET => Ok(PacketKind::ResearchedCallPacket(
                        ResearchedCallPacket::read_from_with_loader(&mut cursor, loader)?,
                    )),
                    packet_ids::TRANSFER_ITEM_EFFECT_CALL_PACKET => {
                        Ok(PacketKind::TransferItemEffectCallPacket(
                            TransferItemEffectCallPacket::read_from_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    packet_ids::TRANSFER_ITEM_TO_CALL_PACKET => {
                        Ok(PacketKind::TransferItemToCallPacket(
                            TransferItemToCallPacket::read_from_with_loader(&mut cursor, loader)?,
                        ))
                    }
                    packet_ids::TRANSFER_ITEM_TO_UNIT_CALL_PACKET => {
                        Ok(PacketKind::TransferItemToUnitCallPacket(
                            TransferItemToUnitCallPacket::read_from_with_loader(
                                &mut cursor,
                                loader,
                            )?,
                        ))
                    }
                    _ => Self::packet_kind_from_envelope_without_loader(envelope),
                }
            }
            PacketEnvelope::Framework(_) => Err(SerializerError::ExpectedPacketEnvelope),
            PacketEnvelope::Raw(_) => Err(SerializerError::ExpectedPacketEnvelope),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SerializerError {
    #[error("buffer underflow while reading packet serializer bytes")]
    Underflow,
    #[error("unknown framework message id {0}")]
    UnknownFrameworkMessage(u8),
    #[error("unsupported packet id {0}")]
    UnsupportedPacketId(u8),
    #[error("unknown packet compression id {0}")]
    UnknownCompression(u8),
    #[error("packet compression error: {0}")]
    Compression(String),
    #[error("packet payload too large for Java unsigned short length: {0}")]
    PayloadTooLarge(usize),
    #[error("expected normal packet envelope")]
    ExpectedPacketEnvelope,
    #[error("packet codec requires ContentLoader")]
    RequiresContentLoader,
    #[error("packet codec IO error: {0}")]
    Io(String),
}

impl From<std::io::Error> for SerializerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.pos..]
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], SerializerError> {
        if self.pos + len > self.bytes.len() {
            return Err(SerializerError::Underflow);
        }
        let out = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, SerializerError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SerializerError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32, SerializerError> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ArcTransport;

impl ArcTransport {
    pub const DISCOVER_HOST_BYTES: [u8; 2] = [PacketSerializer::FRAMEWORK_ID, 1];

    pub fn discover_host_probe() -> Vec<u8> {
        PacketSerializer::write_envelope(&PacketEnvelope::Framework(FrameworkMessage::DiscoverHost))
    }

    pub fn wrap_tcp_frame(payload: &[u8]) -> Result<Vec<u8>, SerializerError> {
        if payload.len() > u16::MAX as usize {
            return Err(SerializerError::PayloadTooLarge(payload.len()));
        }

        let mut out = Vec::with_capacity(payload.len() + 2);
        out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        out.extend_from_slice(payload);
        Ok(out)
    }

    pub fn unwrap_tcp_frame(bytes: &[u8]) -> Result<(&[u8], usize), SerializerError> {
        let mut cursor = Cursor::new(bytes);
        let len = cursor.u16()? as usize;
        let payload = cursor.take(len)?;
        Ok((payload, 2 + len))
    }

    pub fn wrap_tcp_packet(packet: &PacketKind) -> Result<Vec<u8>, SerializerError> {
        Self::wrap_tcp_frame(&PacketSerializer::write_packet_kind(packet)?)
    }

    pub fn unwrap_tcp_packet(bytes: &[u8]) -> Result<(PacketKind, usize), SerializerError> {
        let (payload, used) = Self::unwrap_tcp_frame(bytes)?;
        Ok((PacketSerializer::read_packet_kind(payload)?, used))
    }

    pub fn ping_host(address: &str, port: u16, timeout: Duration) -> io::Result<Host> {
        let target = (address, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "could not resolve host"))?;

        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(timeout))?;

        let probe = Self::discover_host_probe();
        let started = Instant::now();
        socket.send_to(&probe, target)?;

        let mut buffer = [0u8; 512];
        let (len, src) = socket.recv_from(&mut buffer)?;
        let elapsed = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
        let mut host =
            super::network_io::read_server_data(elapsed, src.ip().to_string(), &buffer[..len])
                .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))?;
        host.port = port as i32;
        Ok(host)
    }
}

#[derive(Debug, Clone)]
struct SharedArcConnection {
    info: Arc<Mutex<NetConnection>>,
    tcp: Arc<Mutex<TcpStream>>,
    udp_addr: Option<SocketAddr>,
}

impl SharedArcConnection {
    fn new(info: NetConnection, tcp: Arc<Mutex<TcpStream>>, udp_addr: Option<SocketAddr>) -> Self {
        Self {
            info: Arc::new(Mutex::new(info)),
            tcp,
            udp_addr,
        }
    }

    fn snapshot(&self) -> NetConnection {
        lock_unpoison(&self.info).clone()
    }

    fn is_connected(&self) -> bool {
        let info = lock_unpoison(&self.info);
        !info.has_disconnected && !info.kicked
    }

    fn mark_connected(&self, address: impl Into<String>) {
        let mut info = lock_unpoison(&self.info);
        info.address = address.into();
        info.has_connected = true;
        info.has_disconnected = false;
    }

    fn mark_disconnected(&self) {
        lock_unpoison(&self.info).has_disconnected = true;
    }

    fn close(&self) {
        self.mark_disconnected();
        let _ = lock_unpoison(&self.tcp).shutdown(Shutdown::Both);
    }

    fn send(
        &self,
        udp: &UdpSocket,
        packet: &PacketKind,
        reliable: bool,
        content_loader: &Arc<Mutex<Option<ContentLoader>>>,
    ) -> io::Result<()> {
        if !self.is_connected() {
            return Err(io::Error::new(
                ErrorKind::NotConnected,
                "connection is closed",
            ));
        }

        let result = if reliable {
            write_tcp_packet(&self.tcp, packet, content_loader)
        } else if let Some(target) = self.udp_addr {
            let bytes = write_packet_bytes(packet, content_loader)?;
            udp.send_to(&bytes, target).map(|_| ())
        } else {
            Err(io::Error::new(
                ErrorKind::AddrNotAvailable,
                "connection has no registered UDP address",
            ))
        };

        if result
            .as_ref()
            .is_err_and(|err| is_connection_send_error(err))
        {
            self.close();
        }
        result
    }
}

#[derive(Debug)]
struct ClientState {
    tcp: Arc<Mutex<TcpStream>>,
    udp: Arc<UdpSocket>,
    shutdown: Arc<AtomicBool>,
}

#[derive(Debug)]
struct ServerState {
    shutdown: Arc<AtomicBool>,
    udp: Arc<UdpSocket>,
    pending: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    connections: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
}

pub struct ArcNetProvider {
    client: Option<ClientState>,
    server: Option<ServerState>,
    events: Arc<Mutex<VecDeque<ProviderEvent>>>,
    connect_filter: Option<ConnectFilter>,
    discovery_data: Arc<Mutex<Option<ServerData>>>,
    content_loader: Arc<Mutex<Option<ContentLoader>>>,
    next_connection_id: Arc<AtomicI32>,
    connect_timeout: Duration,
}

impl std::fmt::Debug for ArcNetProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcNetProvider")
            .field("has_client", &self.client.is_some())
            .field("has_server", &self.server.is_some())
            .field("connect_timeout", &self.connect_timeout)
            .finish()
    }
}

impl Default for ArcNetProvider {
    fn default() -> Self {
        Self {
            client: None,
            server: None,
            events: Arc::new(Mutex::new(VecDeque::new())),
            connect_filter: None,
            discovery_data: Arc::new(Mutex::new(None)),
            content_loader: Arc::new(Mutex::new(None)),
            next_connection_id: Arc::new(AtomicI32::new(1)),
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl ArcNetProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_server_data(data: ServerData) -> Self {
        let provider = Self::default();
        *lock_unpoison(&provider.discovery_data) = Some(data);
        provider
    }

    pub fn set_server_data(&self, data: ServerData) {
        *lock_unpoison(&self.discovery_data) = Some(data);
    }

    pub fn set_content_loader(&self, loader: ContentLoader) {
        *lock_unpoison(&self.content_loader) = Some(loader);
    }

    pub fn clear_content_loader(&self) {
        *lock_unpoison(&self.content_loader) = None;
    }

    pub fn set_connect_timeout(&mut self, timeout: Duration) {
        self.connect_timeout = timeout;
    }

    pub fn drain_events(&self) -> Vec<ProviderEvent> {
        drain_events(&self.events)
    }

    fn send_client_packet(&mut self, object: &PacketKind, reliable: bool) -> io::Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| io::Error::new(ErrorKind::NotConnected, "client is not connected"))?;

        if reliable {
            write_tcp_packet(&client.tcp, object, &self.content_loader)
        } else {
            let bytes = write_packet_bytes(object, &self.content_loader)?;
            client.udp.send(&bytes)?;
            Ok(())
        }
    }

    fn send_server_packet(
        &mut self,
        except_connection_id: Option<i32>,
        object: &PacketKind,
        reliable: bool,
    ) -> io::Result<()> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| io::Error::new(ErrorKind::NotConnected, "server is not hosted"))?;

        let connections: Vec<_> = lock_unpoison(&server.connections)
            .iter()
            .map(|(id, connection)| (*id, connection.clone()))
            .collect();

        let mut first_error = None;
        let mut disconnected = Vec::new();

        for (id, connection) in connections {
            if except_connection_id == Some(id) {
                continue;
            }

            if let Err(err) = connection.send(&server.udp, object, reliable, &self.content_loader) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
                if !connection.is_connected() {
                    disconnected.push(id);
                }
            }
        }

        if !disconnected.is_empty() {
            let mut active = lock_unpoison(&server.connections);
            for id in disconnected {
                if let Some(connection) = active.remove(&id) {
                    connection.mark_disconnected();
                    push_event(
                        &self.events,
                        ProviderEvent::ServerDisconnected {
                            connection_id: id,
                            reason: "error".to_string(),
                        },
                    );
                }
            }
        }

        if let Some(err) = first_error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

impl NetProvider for ArcNetProvider {
    fn connect_client(
        &mut self,
        ip: &str,
        port: u16,
        success: Box<dyn Fn() + Send + 'static>,
    ) -> io::Result<()> {
        self.disconnect_client();

        let target = (ip, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "could not resolve host"))?;

        let mut tcp = TcpStream::connect_timeout(&target, self.connect_timeout)?;
        tcp.set_nodelay(true)?;
        tcp.set_read_timeout(Some(self.connect_timeout))?;
        tcp.set_write_timeout(Some(self.connect_timeout))?;

        let connection_id = match read_tcp_envelope(&mut tcp)? {
            PacketEnvelope::Framework(FrameworkMessage::RegisterTcp { connection_id }) => {
                connection_id
            }
            other => {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("expected RegisterTCP, got {other:?}"),
                ));
            }
        };

        let udp = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
        udp.connect(target)?;
        udp.set_read_timeout(Some(Duration::from_millis(250)))?;

        let registering = Arc::new(AtomicBool::new(true));
        let udp_retry = udp.try_clone()?;
        let retrying = registering.clone();
        thread::spawn(move || {
            let envelope =
                PacketEnvelope::Framework(FrameworkMessage::RegisterUdp { connection_id });
            let bytes = PacketSerializer::write_envelope(&envelope);
            while retrying.load(Ordering::Relaxed) {
                let _ = udp_retry.send(&bytes);
                thread::sleep(Duration::from_millis(100));
            }
        });

        let ack = read_tcp_envelope(&mut tcp);
        registering.store(false, Ordering::Relaxed);
        match ack? {
            PacketEnvelope::Framework(FrameworkMessage::RegisterUdp { .. }) => {}
            other => {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("expected RegisterUDP ack, got {other:?}"),
                ));
            }
        }

        tcp.set_read_timeout(Some(Duration::from_millis(250)))?;
        let read_tcp = tcp.try_clone()?;
        let tcp = Arc::new(Mutex::new(tcp));
        let shutdown = Arc::new(AtomicBool::new(false));
        self.client = Some(ClientState {
            tcp: tcp.clone(),
            udp: udp.clone(),
            shutdown: shutdown.clone(),
        });

        push_event(
            &self.events,
            ProviderEvent::ClientConnected {
                address_tcp: target.to_string(),
            },
        );
        success();

        spawn_client_tcp_reader(
            read_tcp,
            self.events.clone(),
            self.content_loader.clone(),
            shutdown.clone(),
        );
        spawn_client_udp_reader(
            udp,
            self.events.clone(),
            self.content_loader.clone(),
            shutdown,
        );

        Ok(())
    }

    fn send_client(&mut self, object: &PacketKind, reliable: bool) -> io::Result<()> {
        self.send_client_packet(object, reliable)
    }

    fn disconnect_client(&mut self) {
        if let Some(client) = self.client.take() {
            client.shutdown.store(true, Ordering::Relaxed);
            let _ = lock_unpoison(&client.tcp).shutdown(Shutdown::Both);
            push_event(
                &self.events,
                ProviderEvent::ClientDisconnected {
                    reason: "closed".to_string(),
                },
            );
        }
    }

    fn discover_servers(&self, callback: HostCallback, done: DoneCallback) {
        thread::spawn(move || {
            let socket = match UdpSocket::bind("0.0.0.0:0") {
                Ok(socket) => socket,
                Err(_) => {
                    done();
                    return;
                }
            };
            let _ = socket.set_broadcast(true);
            let _ = socket.set_read_timeout(Some(Duration::from_secs(3)));
            let probe = ArcTransport::discover_host_probe();
            let _ = socket.send_to(&probe, ("255.255.255.255", DEFAULT_PORT));
            let _ = socket.send_to(&probe, (MULTICAST_GROUP, MULTICAST_PORT));

            let started = Instant::now();
            let mut found = HashSet::new();
            let mut buffer = [0u8; 512];
            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((len, src)) => {
                        if !found.insert(src.ip()) {
                            continue;
                        }
                        let elapsed = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
                        if let Ok(host) = super::network_io::read_server_data(
                            elapsed,
                            src.ip().to_string(),
                            &buffer[..len],
                        ) {
                            callback(host);
                        }
                    }
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        break;
                    }
                    Err(_) => break,
                }
            }
            done();
        });
    }

    fn ping_host(&self, address: &str, port: u16, timeout: Duration) -> io::Result<Host> {
        ArcTransport::ping_host(address, port, timeout)
    }

    fn host_server(&mut self, port: u16) -> io::Result<()> {
        self.close_server();

        let listener = TcpListener::bind(("0.0.0.0", port))?;
        listener.set_nonblocking(true)?;
        let udp = Arc::new(UdpSocket::bind(("0.0.0.0", port))?);
        udp.set_read_timeout(Some(Duration::from_millis(250)))?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let connections = Arc::new(Mutex::new(HashMap::new()));

        self.server = Some(ServerState {
            shutdown: shutdown.clone(),
            udp: udp.clone(),
            pending: pending.clone(),
            connections: connections.clone(),
        });

        spawn_server_accept_loop(
            listener,
            pending.clone(),
            connections.clone(),
            self.events.clone(),
            self.content_loader.clone(),
            self.connect_filter.clone(),
            self.next_connection_id.clone(),
            shutdown.clone(),
        );
        spawn_server_udp_loop(
            udp,
            pending,
            connections,
            self.events.clone(),
            self.content_loader.clone(),
            self.discovery_data.clone(),
            shutdown,
            port,
        );

        Ok(())
    }

    fn get_connections(&self) -> Vec<NetConnection> {
        self.server
            .as_ref()
            .map(|server| {
                lock_unpoison(&server.connections)
                    .values()
                    .map(SharedArcConnection::snapshot)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn close_server(&mut self) {
        if let Some(server) = self.server.take() {
            server.shutdown.store(true, Ordering::Relaxed);

            for connection in lock_unpoison(&server.connections).values() {
                connection.close();
            }
            lock_unpoison(&server.pending).clear();
            lock_unpoison(&server.connections).clear();
        }
    }

    fn send_server(&mut self, object: &PacketKind, reliable: bool) -> io::Result<()> {
        self.send_server_packet(None, object, reliable)
    }

    fn send_server_to(
        &mut self,
        connection_id: i32,
        object: &PacketKind,
        reliable: bool,
    ) -> io::Result<()> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| io::Error::new(ErrorKind::NotConnected, "server is not hosted"))?;

        let connection = lock_unpoison(&server.connections)
            .get(&connection_id)
            .cloned()
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::NotFound,
                    format!("connection {connection_id} not found"),
                )
            })?;

        let result = connection.send(&server.udp, object, reliable, &self.content_loader);
        if result.is_err() && !connection.is_connected() {
            if let Some(connection) = lock_unpoison(&server.connections).remove(&connection_id) {
                connection.mark_disconnected();
                push_event(
                    &self.events,
                    ProviderEvent::ServerDisconnected {
                        connection_id,
                        reason: "error".to_string(),
                    },
                );
            }
        }
        result
    }

    fn send_server_except(
        &mut self,
        except_connection_id: i32,
        object: &PacketKind,
        reliable: bool,
    ) -> io::Result<()> {
        self.send_server_packet(Some(except_connection_id), object, reliable)
    }

    fn drain_events(&mut self) -> Vec<ProviderEvent> {
        drain_events(&self.events)
    }

    fn dispose(&mut self) {
        self.disconnect_client();
        self.close_server();
    }

    fn set_connect_filter(&mut self, connect_filter: Option<ConnectFilter>) {
        self.connect_filter = connect_filter;
    }

    fn get_connect_filter(&self) -> Option<ConnectFilter> {
        self.connect_filter.clone()
    }
}

fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn push_event(events: &Arc<Mutex<VecDeque<ProviderEvent>>>, event: ProviderEvent) {
    lock_unpoison(events).push_back(event);
}

fn drain_events(events: &Arc<Mutex<VecDeque<ProviderEvent>>>) -> Vec<ProviderEvent> {
    lock_unpoison(events).drain(..).collect()
}

fn is_connection_send_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::NotConnected
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::BrokenPipe
            | ErrorKind::TimedOut
            | ErrorKind::WriteZero
            | ErrorKind::AddrNotAvailable
    )
}

fn serializer_io_error(err: SerializerError) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, err)
}

fn write_packet_bytes(
    packet: &PacketKind,
    content_loader: &Arc<Mutex<Option<ContentLoader>>>,
) -> io::Result<Vec<u8>> {
    let loader = lock_unpoison(content_loader);
    if let Some(loader) = loader.as_ref() {
        PacketSerializer::write_packet_kind_with_loader(packet, loader).map_err(serializer_io_error)
    } else {
        PacketSerializer::write_packet_kind(packet).map_err(serializer_io_error)
    }
}

fn read_tcp_envelope(stream: &mut TcpStream) -> io::Result<PacketEnvelope> {
    let mut len = [0u8; 2];
    stream.read_exact(&mut len)?;
    let len = u16::from_be_bytes(len) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    PacketSerializer::read_envelope(&payload).map_err(serializer_io_error)
}

fn write_tcp_envelope(tcp: &Arc<Mutex<TcpStream>>, envelope: &PacketEnvelope) -> io::Result<()> {
    let payload = PacketSerializer::write_envelope(envelope);
    let frame = ArcTransport::wrap_tcp_frame(&payload).map_err(serializer_io_error)?;
    let mut stream = lock_unpoison(tcp);
    stream.write_all(&frame)?;
    stream.flush()
}

fn write_tcp_packet(
    tcp: &Arc<Mutex<TcpStream>>,
    packet: &PacketKind,
    content_loader: &Arc<Mutex<Option<ContentLoader>>>,
) -> io::Result<()> {
    let payload = write_packet_bytes(packet, content_loader)?;
    let frame = ArcTransport::wrap_tcp_frame(&payload).map_err(serializer_io_error)?;
    let mut stream = lock_unpoison(tcp);
    stream.write_all(&frame)?;
    stream.flush()
}

fn parse_packet_envelope(
    envelope: PacketEnvelope,
    content_loader: &Arc<Mutex<Option<ContentLoader>>>,
) -> Result<Option<PacketKind>, SerializerError> {
    match envelope {
        PacketEnvelope::Framework(_) | PacketEnvelope::Raw(_) => Ok(None),
        packet @ PacketEnvelope::Packet { .. } => lock_unpoison(content_loader)
            .as_ref()
            .map(|loader| PacketSerializer::packet_kind_from_envelope_with_loader(&packet, loader))
            .unwrap_or_else(|| PacketSerializer::packet_kind_from_envelope(&packet))
            .map(Some),
    }
}

fn spawn_client_tcp_reader(
    mut stream: TcpStream,
    events: Arc<Mutex<VecDeque<ProviderEvent>>>,
    content_loader: Arc<Mutex<Option<ContentLoader>>>,
    shutdown: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match read_tcp_envelope(&mut stream) {
                Ok(envelope) => match parse_packet_envelope(envelope, &content_loader) {
                    Ok(Some(packet)) => push_event(&events, ProviderEvent::ClientPacket(packet)),
                    Ok(None) => {}
                    Err(_) => {}
                },
                Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    continue;
                }
                Err(_) => {
                    if !shutdown.load(Ordering::Relaxed) {
                        push_event(
                            &events,
                            ProviderEvent::ClientDisconnected {
                                reason: "closed".to_string(),
                            },
                        );
                    }
                    break;
                }
            }
        }
    });
}

fn spawn_client_udp_reader(
    udp: Arc<UdpSocket>,
    events: Arc<Mutex<VecDeque<ProviderEvent>>>,
    content_loader: Arc<Mutex<Option<ContentLoader>>>,
    shutdown: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut buffer = [0u8; 65535];
        while !shutdown.load(Ordering::Relaxed) {
            match udp.recv(&mut buffer) {
                Ok(len) => match PacketSerializer::read_envelope(&buffer[..len])
                    .and_then(|envelope| parse_packet_envelope(envelope, &content_loader))
                {
                    Ok(Some(packet)) => push_event(&events, ProviderEvent::ClientPacket(packet)),
                    Ok(None) => {}
                    Err(_) => {}
                },
                Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    continue;
                }
                Err(_) => break,
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_server_accept_loop(
    listener: TcpListener,
    pending: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    connections: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    events: Arc<Mutex<VecDeque<ProviderEvent>>>,
    content_loader: Arc<Mutex<Option<ContentLoader>>>,
    connect_filter: Option<ConnectFilter>,
    next_connection_id: Arc<AtomicI32>,
    shutdown: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, addr)) => {
                    let address = addr.ip().to_string();
                    if let Some(filter) = &connect_filter {
                        if !filter(&address) {
                            let _ = stream.shutdown(Shutdown::Both);
                            continue;
                        }
                    }

                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                    let writer = match stream.try_clone() {
                        Ok(stream) => Arc::new(Mutex::new(stream)),
                        Err(_) => continue,
                    };

                    let connection_id = next_connection_id.fetch_add(1, Ordering::Relaxed);
                    let mut info = NetConnection::new(address.clone());
                    info.has_begun_connecting = true;

                    let connection = SharedArcConnection::new(info, writer.clone(), None);
                    lock_unpoison(&pending).insert(connection_id, connection);

                    let register =
                        PacketEnvelope::Framework(FrameworkMessage::RegisterTcp { connection_id });
                    if write_tcp_envelope(&writer, &register).is_err() {
                        lock_unpoison(&pending).remove(&connection_id);
                        continue;
                    }

                    spawn_server_tcp_reader(
                        connection_id,
                        stream,
                        pending.clone(),
                        connections.clone(),
                        events.clone(),
                        content_loader.clone(),
                        shutdown.clone(),
                    );
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_server_tcp_reader(
    connection_id: i32,
    mut stream: TcpStream,
    pending: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    connections: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    events: Arc<Mutex<VecDeque<ProviderEvent>>>,
    content_loader: Arc<Mutex<Option<ContentLoader>>>,
    shutdown: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match read_tcp_envelope(&mut stream) {
                Ok(envelope) => match parse_packet_envelope(envelope, &content_loader) {
                    Ok(Some(packet)) => push_event(
                        &events,
                        ProviderEvent::ServerPacket {
                            connection_id,
                            packet,
                        },
                    ),
                    Ok(None) => {}
                    Err(_) => {}
                },
                Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    continue;
                }
                Err(_) => break,
            }
        }

        if let Some(connection) = lock_unpoison(&pending).remove(&connection_id) {
            connection.mark_disconnected();
        }
        if let Some(connection) = lock_unpoison(&connections).remove(&connection_id) {
            connection.mark_disconnected();
            push_event(
                &events,
                ProviderEvent::ServerDisconnected {
                    connection_id,
                    reason: "closed".to_string(),
                },
            );
        }
    });
}

fn spawn_server_udp_loop(
    udp: Arc<UdpSocket>,
    pending: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    connections: Arc<Mutex<HashMap<i32, SharedArcConnection>>>,
    events: Arc<Mutex<VecDeque<ProviderEvent>>>,
    content_loader: Arc<Mutex<Option<ContentLoader>>>,
    discovery_data: Arc<Mutex<Option<ServerData>>>,
    shutdown: Arc<AtomicBool>,
    port: u16,
) {
    thread::spawn(move || {
        let mut buffer = [0u8; 65535];
        while !shutdown.load(Ordering::Relaxed) {
            let (len, src) = match udp.recv_from(&mut buffer) {
                Ok(result) => result,
                Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    continue;
                }
                Err(_) => break,
            };

            let envelope = match PacketSerializer::read_envelope(&buffer[..len]) {
                Ok(envelope) => envelope,
                Err(_) => continue,
            };

            match envelope {
                PacketEnvelope::Framework(FrameworkMessage::RegisterUdp { connection_id }) => {
                    let mut connection = match lock_unpoison(&pending).remove(&connection_id) {
                        Some(connection) => connection,
                        None => continue,
                    };
                    connection.udp_addr = Some(src);
                    connection.mark_connected(src.ip().to_string());

                    let tcp = connection.tcp.clone();
                    lock_unpoison(&connections).insert(connection_id, connection.clone());
                    let register =
                        PacketEnvelope::Framework(FrameworkMessage::RegisterUdp { connection_id });
                    let _ = write_tcp_envelope(&tcp, &register);
                    push_event(
                        &events,
                        ProviderEvent::ServerConnected {
                            connection_id,
                            address: connection.snapshot().address,
                        },
                    );
                }
                PacketEnvelope::Framework(FrameworkMessage::DiscoverHost) => {
                    let data = lock_unpoison(&discovery_data)
                        .clone()
                        .unwrap_or_else(|| default_server_data(port));
                    let bytes = super::network_io::write_server_data(&data);
                    let _ = udp.send_to(&bytes, src);
                }
                PacketEnvelope::Framework(_) => {}
                packet @ PacketEnvelope::Packet { .. } => {
                    let connection_id =
                        lock_unpoison(&connections)
                            .iter()
                            .find_map(|(id, connection)| {
                                (connection.udp_addr == Some(src)).then_some(*id)
                            });
                    if let Some(connection_id) = connection_id {
                        if let Ok(Some(packet)) = parse_packet_envelope(packet, &content_loader) {
                            push_event(
                                &events,
                                ProviderEvent::ServerPacket {
                                    connection_id,
                                    packet,
                                },
                            );
                        }
                    }
                }
                PacketEnvelope::Raw(_) => {}
            }
        }
    });
}

fn default_server_data(port: u16) -> ServerData {
    ServerData {
        name: "Mindustry Rust".to_string(),
        map: String::new(),
        players: 0,
        wave: 0,
        version: 157,
        version_type: "release".to_string(),
        mode: Gamemode::Survival,
        player_limit: 0,
        description: String::new(),
        mode_name: None,
        port,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::{net::UdpSocket, thread, time::Duration};

    use crate::mindustry::{
        core::{content_loader::ContentLoader, ClientConnectConfig, NetClient, NetServer},
        ctype::ContentType,
        game::Gamemode,
        io::{BuildPlanWire, BuildingRef, ContentRef, EntityRef, TeamId, TypeValue, UnitRef},
        net::packets::{
            ClientBinaryPacketCallPacket, ClientPacketCallPacket, ClientPlanSnapshotCallPacket,
            ClientPlanSnapshotReceivedCallPacket, ClientSnapshotCallPacket,
            ConstructFinishCallPacket, DeconstructFinishCallPacket, DeletePlansCallPacket,
            DestroyPayloadCallPacket,
        },
        net::{
            network_io::ServerData, write_minimal_world_data, write_server_data, Net, SentPacket,
        },
    };

    #[test]
    fn discover_host_probe_matches_java_framework_bytes() {
        assert_eq!(ArcTransport::discover_host_probe(), vec![0xfe, 1]);
    }

    #[test]
    fn tcp_frame_wrap_and_unwrap_roundtrips_packet_bytes() {
        let packet = PacketKind::PingCallPacket(PingCallPacket { time: 123456789 });

        let framed = ArcTransport::wrap_tcp_packet(&packet).unwrap();
        let (decoded, used) = ArcTransport::unwrap_tcp_packet(&framed).unwrap();
        assert_eq!(used, framed.len());
        assert_eq!(decoded, packet);
    }

    #[test]
    fn ping_host_reads_java_server_data_payload_over_udp() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let port = server.local_addr().unwrap().port();

        let responder = thread::spawn(move || {
            let mut buffer = [0u8; 512];
            let (len, peer) = server.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..len], ArcTransport::DISCOVER_HOST_BYTES);

            let bytes = write_server_data(&ServerData {
                name: "Server".into(),
                map: "Map".into(),
                players: 5,
                wave: 12,
                version: 158,
                version_type: "release".into(),
                mode: Gamemode::Attack,
                player_limit: 16,
                description: "desc".into(),
                mode_name: Some("custom".into()),
                port,
            });
            server.send_to(&bytes, peer).unwrap();
        });

        let host = ArcTransport::ping_host("127.0.0.1", port, Duration::from_secs(2)).unwrap();
        responder.join().unwrap();

        assert_eq!(host.name, "Server");
        assert_eq!(host.mapname, "Map");
        assert_eq!(host.players, 5);
        assert_eq!(host.wave, 12);
        assert_eq!(host.version, 158);
        assert_eq!(host.version_type, "release");
        assert_eq!(host.mode, Gamemode::Attack);
        assert_eq!(host.player_limit, 16);
        assert_eq!(host.description, "desc");
        assert_eq!(host.mode_name.as_deref(), Some("custom"));
        assert_eq!(host.port, i32::from(port));
        assert!(host.ping >= 0);
    }

    #[test]
    fn arc_net_provider_registers_tcp_udp_and_exchanges_packets() {
        let port = free_local_port();
        let mut server = ArcNetProvider::new();
        server.host_server(port).unwrap();

        let mut client = ArcNetProvider::new();
        client
            .connect_client("127.0.0.1", port, Box::new(|| {}))
            .unwrap();

        let server_connected = wait_for_event(&server, |event| {
            matches!(event, ProviderEvent::ServerConnected { .. })
        });
        let connection_id = match server_connected {
            ProviderEvent::ServerConnected { connection_id, .. } => connection_id,
            other => panic!("unexpected event: {other:?}"),
        };

        assert!(matches!(
            wait_for_event(&client, |event| matches!(
                event,
                ProviderEvent::ClientConnected { .. }
            )),
            ProviderEvent::ClientConnected { .. }
        ));

        let client_to_server = PacketKind::PingCallPacket(PingCallPacket { time: 99 });
        server.drain_events();
        client.send_client(&client_to_server, true).unwrap();
        assert_eq!(
            wait_for_server_packet(&server, connection_id),
            client_to_server
        );

        let client_to_server_udp = PacketKind::PingCallPacket(PingCallPacket { time: 100 });
        client.send_client(&client_to_server_udp, false).unwrap();
        assert_eq!(
            wait_for_server_packet(&server, connection_id),
            client_to_server_udp
        );

        let server_to_client =
            PacketKind::PingResponseCallPacket(PingResponseCallPacket { time: 101 });
        client.drain_events();
        server.send_server(&server_to_client, true).unwrap();
        assert_eq!(wait_for_client_packet(&client), server_to_client);

        let server_to_client_udp =
            PacketKind::PingResponseCallPacket(PingResponseCallPacket { time: 102 });
        server.send_server(&server_to_client_udp, false).unwrap();
        assert_eq!(wait_for_client_packet(&client), server_to_client_udp);

        client.disconnect_client();
        server.close_server();
    }

    #[test]
    fn arc_net_provider_continues_broadcast_after_one_connection_fails() {
        let port = free_local_port();
        let mut server = ArcNetProvider::new();
        server.host_server(port).unwrap();

        let mut first_client = ArcNetProvider::new();
        first_client
            .connect_client("127.0.0.1", port, Box::new(|| {}))
            .unwrap();

        let first_connected = wait_for_event(&server, |event| {
            matches!(event, ProviderEvent::ServerConnected { .. })
        });
        let first_connection_id = match first_connected {
            ProviderEvent::ServerConnected { connection_id, .. } => connection_id,
            other => panic!("unexpected event: {other:?}"),
        };

        assert!(matches!(
            wait_for_event(&first_client, |event| matches!(
                event,
                ProviderEvent::ClientConnected { .. }
            )),
            ProviderEvent::ClientConnected { .. }
        ));

        let mut second_client = ArcNetProvider::new();
        second_client
            .connect_client("127.0.0.1", port, Box::new(|| {}))
            .unwrap();

        let second_connected = wait_for_event(&server, |event| {
            matches!(event, ProviderEvent::ServerConnected { .. })
        });
        let _second_connection_id = match second_connected {
            ProviderEvent::ServerConnected { connection_id, .. } => connection_id,
            other => panic!("unexpected event: {other:?}"),
        };

        assert!(matches!(
            wait_for_event(&second_client, |event| matches!(
                event,
                ProviderEvent::ClientConnected { .. }
            )),
            ProviderEvent::ClientConnected { .. }
        ));

        server.drain_events();
        first_client.drain_events();
        second_client.drain_events();

        {
            let state = server.server.as_ref().expect("server state missing");
            let connections = lock_unpoison(&state.connections);
            let connection = connections
                .get(&first_connection_id)
                .expect("missing first connection");
            connection.mark_disconnected();
        }

        let packet = PacketKind::PingResponseCallPacket(PingResponseCallPacket { time: 303 });
        assert!(server.send_server(&packet, true).is_err());
        assert_eq!(wait_for_client_packet(&second_client), packet);

        let disconnected = wait_for_event(&server, |event| {
            matches!(
                event,
                ProviderEvent::ServerDisconnected {
                    connection_id,
                    ..
                } if *connection_id == first_connection_id
            )
        });
        match disconnected {
            ProviderEvent::ServerDisconnected { connection_id, .. } => {
                assert_eq!(connection_id, first_connection_id);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert_eq!(server.get_connections().len(), 1);
        assert!(server
            .get_connections()
            .iter()
            .any(|connection| connection.is_connected()));

        first_client.disconnect_client();
        second_client.disconnect_client();
        server.close_server();
    }

    #[test]
    fn hosted_arc_net_provider_responds_to_discovery_probe() {
        let port = free_local_port();
        let mut server = ArcNetProvider::with_server_data(ServerData {
            name: "Rust Server".into(),
            map: "Rust Map".into(),
            players: 3,
            wave: 9,
            version: 158,
            version_type: "release".into(),
            mode: Gamemode::Survival,
            player_limit: 12,
            description: "provider discovery".into(),
            mode_name: None,
            port,
        });

        server.host_server(port).unwrap();
        let host = ArcTransport::ping_host("127.0.0.1", port, Duration::from_secs(2)).unwrap();
        server.close_server();

        assert_eq!(host.name, "Rust Server");
        assert_eq!(host.mapname, "Rust Map");
        assert_eq!(host.players, 3);
        assert_eq!(host.wave, 9);
        assert_eq!(host.version, 158);
        assert_eq!(host.description, "provider discovery");
        assert_eq!(host.port, i32::from(port));
    }

    #[test]
    fn arc_net_provider_uses_content_loader_for_live_packet_decode() {
        let loader = ContentLoader::create_base_content().unwrap();
        let port = free_local_port();
        let mut server = ArcNetProvider::new();
        server.set_content_loader(loader.clone());
        server.host_server(port).unwrap();

        let mut client = ArcNetProvider::new();
        client.set_content_loader(loader);
        client
            .connect_client("127.0.0.1", port, Box::new(|| {}))
            .unwrap();

        let server_connected = wait_for_event(&server, |event| {
            matches!(event, ProviderEvent::ServerConnected { .. })
        });
        let connection_id = match server_connected {
            ProviderEvent::ServerConnected { connection_id, .. } => connection_id,
            other => panic!("unexpected event: {other:?}"),
        };

        let packet = PacketKind::ClientPlanSnapshotCallPacket(ClientPlanSnapshotCallPacket {
            group_id: 91,
            plans: Some(vec![BuildPlanWire::new_place_config(
                4,
                5,
                1,
                "duo",
                TypeValue::Content(ContentRef::new(ContentType::Item, 0)),
            )]),
        });

        server.drain_events();
        client.send_client(&packet, true).unwrap();
        assert_eq!(wait_for_server_packet(&server, connection_id), packet);

        client.disconnect_client();
        server.close_server();
    }

    #[test]
    fn arc_net_provider_smoke_java_join_flow_roundtrips_connect_packet_world_stream_and_confirm() {
        let port = free_local_port();
        let server = NetServer::new(Net::new(Box::new(ArcNetProvider::new())));
        server.open(port).unwrap();

        let client = NetClient::with_net(Net::new(Box::new(ArcNetProvider::new())));
        let connect_config = ClientConnectConfig {
            name: "java-smoke".into(),
            usid: "java-smoke-usid".into(),
            color: 0x336699,
            ..ClientConnectConfig::default()
        };
        let expected_connect_packet = connect_config.to_connect_packet();
        client.set_connect_config(Some(connect_config));
        client.begin_connecting();
        {
            let mut net = client.net_mut();
            net.connect("127.0.0.1", port, Box::new(|| {})).unwrap();
        }

        wait_until(
            "server accepts v157.4 ConnectPacket and queues world data",
            || {
                client.update();
                server.update();
                let state = server.state();
                let state = state.lock().unwrap();
                state.connect_packets_accepted == 1
                    && !state.pending_world_data_connections.is_empty()
            },
        );

        let connection_id = {
            let state = server.state();
            let state = state.lock().unwrap();
            assert_eq!(state.connect_packets_rejected, 0);
            assert_eq!(state.pending_world_data_connections.len(), 1);
            let handshake = state.last_handshake.as_ref().unwrap();
            assert_eq!(handshake.version, expected_connect_packet.version);
            assert_eq!(handshake.version_type, expected_connect_packet.version_type);
            assert_eq!(handshake.mods, expected_connect_packet.mods);
            assert_eq!(handshake.name, expected_connect_packet.name);
            assert_eq!(handshake.locale, expected_connect_packet.locale);
            assert_eq!(handshake.usid, expected_connect_packet.usid);
            assert_eq!(handshake.mobile, expected_connect_packet.mobile);
            assert_eq!(handshake.color, expected_connect_packet.color);
            let connection_id = state.pending_world_data_connections[0];
            let connection = state.connection_states.get(&connection_id).unwrap();
            assert_eq!(connection.name, "java-smoke");
            assert_eq!(connection.locale, "en_US");
            assert!(connection.has_begun_connecting);
            assert!(!connection.has_connected);
            assert!(!connection.player_added);
            connection_id
        };

        let world_data = write_minimal_world_data(connection_id).unwrap();
        assert_eq!(
            server
                .send_pending_world_data(|id| write_minimal_world_data(id).unwrap())
                .unwrap(),
            1
        );

        {
            let state = server.state();
            let state = state.lock().unwrap();
            let connection = state.connection_states.get(&connection_id).unwrap();
            assert_eq!(state.last_world_data_connection_id, Some(connection_id));
            assert_eq!(state.last_world_data_bytes, Some(world_data.len()));
            assert_eq!(state.world_streams_sent, 1);
            assert_eq!(connection.sent.len(), 2);
            assert!(matches!(connection.sent[0].0, SentPacket::StreamBegin(_)));
            assert!(matches!(connection.sent[1].0, SentPacket::StreamChunk(_)));
        }

        wait_until(
            "client reassembles world stream and server receives ConnectConfirm",
            || {
                client.update();
                server.update();
                let state = server.state();
                let state = state.lock().unwrap();
                state.last_connect_confirm_connection_id == Some(connection_id)
            },
        );

        {
            let state = client.state();
            let state = state.lock().unwrap();
            assert_eq!(
                state.last_sent_connect_packet.as_ref(),
                Some(&expected_connect_packet)
            );
            assert!(state.connect_packet_sent);
            assert_eq!(
                state
                    .last_world_stream
                    .as_ref()
                    .map(|stream| stream.stream.as_slice()),
                Some(world_data.as_slice())
            );
            assert!(state.connect_confirm_sent);
            assert!(state.last_connect_confirm_error.is_none());
            assert!(state.connected);
        }

        {
            let state = server.state();
            let state = state.lock().unwrap();
            assert!(state.pending_world_data_connections.is_empty());
            assert_eq!(
                state.last_connect_confirm_connection_id,
                Some(connection_id)
            );
            let connection = state.connection_states.get(&connection_id).unwrap();
            assert!(connection.has_begun_connecting);
            assert!(connection.has_connected);
            assert!(connection.player_added);
        }

        client.disconnect_no_reset();
        server.close();
    }

    #[test]
    fn arc_net_provider_updates_connection_snapshots_after_disconnect() {
        let port = free_local_port();
        let mut server = ArcNetProvider::new();
        server.host_server(port).unwrap();

        let mut client = ArcNetProvider::new();
        client
            .connect_client("127.0.0.1", port, Box::new(|| {}))
            .unwrap();

        let connected = wait_for_event(&server, |event| {
            matches!(event, ProviderEvent::ServerConnected { .. })
        });
        let connection_id = match connected {
            ProviderEvent::ServerConnected { connection_id, .. } => connection_id,
            other => panic!("unexpected event: {other:?}"),
        };

        let connections = server.get_connections();
        assert_eq!(connections.len(), 1);
        assert!(connections[0].is_connected());

        client.disconnect_client();
        assert!(matches!(
            wait_for_event(&server, |event| matches!(
                event,
                ProviderEvent::ServerDisconnected { connection_id: id, .. } if *id == connection_id
            )),
            ProviderEvent::ServerDisconnected { .. }
        ));
        assert!(server.get_connections().is_empty());

        server.close_server();
    }

    fn free_local_port() -> u16 {
        for _ in 0..32 {
            let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = tcp.local_addr().unwrap().port();
            if UdpSocket::bind(("127.0.0.1", port)).is_ok() {
                return port;
            }
        }
        panic!("could not reserve a local TCP/UDP port pair");
    }

    fn wait_for_event(
        provider: &ArcNetProvider,
        mut predicate: impl FnMut(&ProviderEvent) -> bool,
    ) -> ProviderEvent {
        for _ in 0..150 {
            for event in provider.drain_events() {
                if predicate(&event) {
                    return event;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for provider event");
    }

    fn wait_for_server_packet(
        provider: &ArcNetProvider,
        expected_connection_id: i32,
    ) -> PacketKind {
        match wait_for_event(provider, |event| {
            matches!(
                event,
                ProviderEvent::ServerPacket {
                    connection_id,
                    ..
                } if *connection_id == expected_connection_id
            )
        }) {
            ProviderEvent::ServerPacket { packet, .. } => packet,
            other => panic!("unexpected event: {other:?}"),
        }
    }

    fn wait_for_client_packet(provider: &ArcNetProvider) -> PacketKind {
        match wait_for_event(provider, |event| {
            matches!(event, ProviderEvent::ClientPacket(_))
        }) {
            ProviderEvent::ClientPacket(packet) => packet,
            other => panic!("unexpected event: {other:?}"),
        }
    }

    fn wait_until(description: &str, mut predicate: impl FnMut() -> bool) {
        for _ in 0..150 {
            if predicate() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for {description}");
    }

    #[test]
    fn framework_ping_matches_java_layout() {
        let msg = FrameworkMessage::Ping {
            id: 42,
            is_reply: true,
        };
        let bytes = PacketSerializer::write_envelope(&PacketEnvelope::Framework(msg.clone()));
        assert_eq!(bytes, vec![0xfe, 0, 0, 0, 0, 42, 1]);
        assert_eq!(
            PacketSerializer::read_envelope(&bytes).unwrap(),
            PacketEnvelope::Framework(msg)
        );
    }

    #[test]
    fn packet_envelope_matches_java_header_layout() {
        let env = PacketEnvelope::Packet {
            id: 4,
            length: 0,
            compression: 0,
            payload: vec![9, 8],
        };
        let bytes = PacketSerializer::write_envelope(&env);
        assert_eq!(bytes, vec![4, 0, 2, 0, 9, 8]);
        assert_eq!(
            PacketSerializer::read_envelope(&bytes).unwrap(),
            PacketEnvelope::Packet {
                id: 4,
                length: 2,
                compression: 0,
                payload: vec![9, 8]
            }
        );
    }

    #[test]
    fn uncompressed_packet_envelope_consumes_declared_length_only() {
        let bytes = vec![4, 0, 2, 0, 9, 8, 7, 6];
        assert_eq!(
            PacketSerializer::read_envelope(&bytes).unwrap(),
            PacketEnvelope::Packet {
                id: 4,
                length: 2,
                compression: 0,
                payload: vec![9, 8]
            }
        );
    }

    #[test]
    fn uncompressed_packet_envelope_rejects_short_payloads() {
        let bytes = vec![4, 0, 3, 0, 9, 8];
        assert_eq!(
            PacketSerializer::read_envelope(&bytes).unwrap_err(),
            SerializerError::Underflow
        );
    }

    #[test]
    fn packet_kind_stream_begin_roundtrips_through_java_envelope() {
        let packet = PacketKind::StreamBegin(StreamBegin {
            id: 9,
            total: 12,
            packet_type: packet_ids::WORLD_STREAM,
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes, vec![0, 0, 9, 0, 0, 0, 0, 9, 0, 0, 0, 12, 2]);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
    }

    #[test]
    fn packet_kind_stream_chunk_roundtrips_through_java_envelope() {
        let packet = PacketKind::StreamChunk(StreamChunk {
            id: 7,
            data: vec![4, 5, 6],
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes, vec![1, 0, 9, 0, 0, 0, 0, 7, 0, 3, 4, 5, 6]);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
    }

    #[test]
    fn packet_kind_connect_packet_uses_manifest_id_without_changing_codec() {
        let packet = ConnectPacket {
            version: 158,
            version_type: "official".into(),
            mods: Vec::new(),
            name: "p".into(),
            locale: "en_US".into(),
            uuid: base64::engine::general_purpose::STANDARD.encode([1, 2, 3, 4, 5, 6, 7, 8]),
            usid: "usid".into(),
            mobile: false,
            color: 7,
            uuid_crc32: None,
        };
        let kind = PacketKind::ConnectPacket(packet);
        let bytes = PacketSerializer::write_packet_kind(&kind).unwrap();
        assert_eq!(bytes[0], packet_ids::CONNECT_PACKET);
        let envelope = PacketSerializer::read_envelope(&bytes).unwrap();
        match envelope {
            PacketEnvelope::Packet {
                id,
                compression,
                payload,
                ..
            } => {
                assert_eq!(id, packet_ids::CONNECT_PACKET);
                assert_eq!(compression, PacketSerializer::COMPRESSION_LZ4);
                assert!(!payload.is_empty());
            }
            other => panic!("unexpected envelope: {other:?}"),
        }
    }

    #[test]
    fn lz4_packet_envelope_uses_declared_uncompressed_length() {
        let payload: Vec<u8> = (0..96).map(|i| (i % 7) as u8).collect();
        let envelope = PacketEnvelope::Packet {
            id: packet_ids::WORLD_STREAM,
            length: payload.len() as u16,
            compression: PacketSerializer::COMPRESSION_LZ4,
            payload: payload.clone(),
        };
        let bytes = PacketSerializer::write_envelope(&envelope);
        assert_eq!(bytes[0], packet_ids::WORLD_STREAM);
        assert_eq!(
            u16::from_be_bytes([bytes[1], bytes[2]]) as usize,
            payload.len()
        );
        assert_eq!(bytes[3], PacketSerializer::COMPRESSION_LZ4);
        assert_ne!(&bytes[4..], payload.as_slice());

        assert_eq!(
            PacketSerializer::read_envelope(&bytes).unwrap(),
            PacketEnvelope::Packet {
                id: packet_ids::WORLD_STREAM,
                length: payload.len() as u16,
                compression: PacketSerializer::COMPRESSION_LZ4,
                payload,
            }
        );
    }

    #[test]
    fn packet_kind_world_stream_is_compressed_above_java_threshold() {
        let stream = crate::mindustry::net::streamable::Streamable {
            stream: vec![3; PacketSerializer::COMPRESS_THRESHOLD + 8],
        };
        let kind = PacketKind::Streamable(stream.clone());
        let bytes = PacketSerializer::write_packet_kind(&kind).unwrap();
        assert_eq!(bytes[0], packet_ids::WORLD_STREAM);
        assert_eq!(bytes[3], PacketSerializer::COMPRESSION_LZ4);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            PacketKind::Streamable(stream)
        );
    }

    #[test]
    fn packet_kind_stream_chunk_remains_uncompressed_above_threshold() {
        let packet = PacketKind::StreamChunk(StreamChunk {
            id: 99,
            data: vec![5; PacketSerializer::COMPRESS_THRESHOLD + 20],
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes[0], packet_ids::STREAM_CHUNK);
        assert_eq!(bytes[3], PacketSerializer::COMPRESSION_NONE);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
    }

    #[test]
    fn client_plan_snapshot_packets_roundtrip_with_content_loader() {
        let loader = ContentLoader::create_base_content().unwrap();
        let plans = vec![BuildPlanWire::new_place_config(
            10,
            20,
            1,
            "duo",
            TypeValue::Content(ContentRef::new(ContentType::Item, 0)),
        )];
        let packet = PacketKind::ClientPlanSnapshotCallPacket(ClientPlanSnapshotCallPacket {
            group_id: 77,
            plans: Some(plans.clone()),
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes[0], packet_ids::CLIENT_PLAN_SNAPSHOT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            packet
        );
        assert_eq!(
            PacketSerializer::write_packet_kind_with_loader(&packet, &loader).unwrap(),
            bytes
        );

        let received = PacketKind::ClientPlanSnapshotReceivedCallPacket(
            ClientPlanSnapshotReceivedCallPacket {
                player_id: 123,
                group_id: 78,
                plans: Some(plans),
            },
        );
        let bytes = PacketSerializer::write_packet_kind_with_loader(&received, &loader).unwrap();
        assert_eq!(
            bytes[0],
            packet_ids::CLIENT_PLAN_SNAPSHOT_RECEIVED_CALL_PACKET
        );
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            received
        );
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            received
        );
    }

    #[test]
    fn client_snapshot_packet_roundtrips_with_content_loader() {
        let loader = ContentLoader::create_base_content().unwrap();
        let packet = PacketKind::ClientSnapshotCallPacket(ClientSnapshotCallPacket {
            snapshot_id: 11,
            unit_id: 22,
            dead: false,
            x: 1.0,
            y: 2.0,
            pointer_x: 3.0,
            pointer_y: 4.0,
            rotation: 5.0,
            base_rotation: 6.0,
            x_velocity: 7.0,
            y_velocity: 8.0,
            mining: Some(crate::mindustry::world::point2_pack(9, 10)),
            boosting: true,
            shooting: false,
            chatting: true,
            building: false,
            selected_block: Some("duo".into()),
            selected_rotation: 1,
            plans: Some(vec![BuildPlanWire::new_place(12, 13, 2, "router")]),
            view_x: 14.0,
            view_y: 15.0,
            view_width: 16.0,
            view_height: 17.0,
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes[0], packet_ids::CLIENT_SNAPSHOT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            packet
        );
        assert_eq!(
            PacketSerializer::write_packet_kind_with_loader(&packet, &loader).unwrap(),
            bytes
        );
    }

    #[test]
    fn construct_finish_packet_roundtrips_with_content_loader() {
        let loader = ContentLoader::create_base_content().unwrap();
        let packet = PacketKind::ConstructFinishCallPacket(ConstructFinishCallPacket {
            tile: Some(crate::mindustry::world::point2_pack(1, 2)),
            block: Some("router".into()),
            builder: UnitRef::Block {
                tile_pos: crate::mindustry::world::point2_pack(3, 4),
            },
            rotation: 1,
            team: TeamId(6),
            config: TypeValue::Content(ContentRef::new(ContentType::Item, 0)),
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes[0], packet_ids::CONSTRUCT_FINISH_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            packet
        );
        assert_eq!(
            PacketSerializer::write_packet_kind_with_loader(&packet, &loader).unwrap(),
            bytes
        );
    }

    #[test]
    fn deconstruct_finish_packet_roundtrips_with_content_loader() {
        let loader = ContentLoader::create_base_content().unwrap();
        let packet = PacketKind::DeconstructFinishCallPacket(DeconstructFinishCallPacket {
            tile: Some(crate::mindustry::world::point2_pack(7, 8)),
            block: Some("router".into()),
            builder: UnitRef::Unit { id: 1234 },
        });
        let bytes = PacketSerializer::write_packet_kind(&packet).unwrap();
        assert_eq!(bytes[0], packet_ids::DECONSTRUCT_FINISH_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), packet);
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            packet
        );
        assert_eq!(
            PacketSerializer::write_packet_kind_with_loader(&packet, &loader).unwrap(),
            bytes
        );
    }

    #[test]
    fn generated_call_packets_roundtrip_through_java_envelope() {
        let announce = PacketKind::AnnounceCallPacket(AnnounceCallPacket {
            message: "server announcement".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&announce).unwrap();
        assert_eq!(bytes[0], packet_ids::ANNOUNCE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            announce
        );

        let clear = PacketKind::ClearObjectivesCallPacket(ClearObjectivesCallPacket);
        let bytes = PacketSerializer::write_packet_kind(&clear).unwrap();
        assert_eq!(bytes[0], packet_ids::CLEAR_OBJECTIVES_CALL_PACKET);
        assert_eq!(bytes[1], 0);
        assert_eq!(bytes[2], 0);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), clear);

        let binary = PacketKind::ClientBinaryPacketReliableCallPacket(
            ClientBinaryPacketReliableCallPacket(ClientBinaryPacketCallPacket {
                packet_type: "mod".into(),
                contents: vec![9; 64],
            }),
        );
        let bytes = PacketSerializer::write_packet_kind(&binary).unwrap();
        assert_eq!(
            bytes[0],
            packet_ids::CLIENT_BINARY_PACKET_RELIABLE_CALL_PACKET
        );
        assert_eq!(bytes[3], PacketSerializer::COMPRESSION_LZ4);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), binary);

        let string_packet = PacketKind::ClientPacketUnreliableCallPacket(
            ClientPacketUnreliableCallPacket(ClientPacketCallPacket {
                packet_type: "chat".into(),
                contents: "hello".into(),
            }),
        );
        let bytes = PacketSerializer::write_packet_kind(&string_packet).unwrap();
        assert_eq!(bytes[0], packet_ids::CLIENT_PACKET_UNRELIABLE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            string_packet
        );

        let complete =
            PacketKind::CompleteObjectiveCallPacket(CompleteObjectiveCallPacket { index: 3 });
        let bytes = PacketSerializer::write_packet_kind(&complete).unwrap();
        assert_eq!(bytes[0], packet_ids::COMPLETE_OBJECTIVE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            complete
        );

        let connect = PacketKind::ConnectCallPacket(ConnectCallPacket {
            ip: "localhost".into(),
            port: 6567,
        });
        let bytes = PacketSerializer::write_packet_kind(&connect).unwrap();
        assert_eq!(bytes[0], packet_ids::CONNECT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), connect);

        let confirm = PacketKind::ConnectConfirmCallPacket(ConnectConfirmCallPacket);
        let bytes = PacketSerializer::write_packet_kind(&confirm).unwrap();
        assert_eq!(
            bytes,
            vec![packet_ids::CONNECT_CONFIRM_CALL_PACKET, 0, 0, 0]
        );
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), confirm);

        let clipboard = PacketKind::CopyToClipboardCallPacket(CopyToClipboardCallPacket {
            text: "copied text".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&clipboard).unwrap();
        assert_eq!(bytes[0], packet_ids::COPY_TO_CLIPBOARD_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            clipboard
        );

        let bullet = PacketKind::CreateBulletCallPacket(CreateBulletCallPacket {
            bullet_type_id: 12,
            team: TeamId(6),
            x: 10.0,
            y: -20.5,
            angle: 90.0,
            damage: 35.25,
            velocity_scl: 1.5,
            lifetime_scl: 0.75,
        });
        let bytes = PacketSerializer::write_packet_kind(&bullet).unwrap();
        assert_eq!(bytes[0], packet_ids::CREATE_BULLET_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), bullet);

        let marker = PacketKind::CreateMarkerCallPacket(CreateMarkerCallPacket {
            id: 99,
            marker_json: r#"{"type":"Point","x":4,"y":5}"#.into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&marker).unwrap();
        assert_eq!(bytes[0], packet_ids::CREATE_MARKER_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), marker);

        let weather = PacketKind::CreateWeatherCallPacket(CreateWeatherCallPacket {
            weather_id: Some(1),
            intensity: 0.8,
            duration: 120.0,
            wind_x: -0.25,
            wind_y: 0.5,
        });
        let bytes = PacketSerializer::write_packet_kind(&weather).unwrap();
        assert_eq!(bytes[0], packet_ids::CREATE_WEATHER_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), weather);

        let effect = PacketKind::EffectCallPacket(EffectCallPacket {
            effect_id: 0x1234,
            x: 1.25,
            y: -2.5,
            rotation: 90.0,
            color: crate::mindustry::io::type_io::RgbaColor::new(0x11223344),
        });
        let bytes = PacketSerializer::write_packet_kind(&effect).unwrap();
        assert_eq!(bytes[0], packet_ids::EFFECT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), effect);

        let effect_with_data = PacketKind::EffectCallPacket2(EffectCallPacket2 {
            effect: EffectCallPacket {
                effect_id: 7,
                x: 3.0,
                y: 4.0,
                rotation: 5.0,
                color: crate::mindustry::io::type_io::RgbaColor::new(-1),
            },
            data: TypeValue::Int(42),
        });
        let bytes = PacketSerializer::write_packet_kind(&effect_with_data).unwrap();
        assert_eq!(bytes[0], packet_ids::EFFECT_CALL_PACKET2);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            effect_with_data
        );

        let reliable_effect =
            PacketKind::EffectReliableCallPacket(EffectReliableCallPacket(EffectCallPacket {
                effect_id: 8,
                x: -1.0,
                y: -2.0,
                rotation: 180.0,
                color: crate::mindustry::io::type_io::RgbaColor::new(0),
            }));
        let bytes = PacketSerializer::write_packet_kind(&reliable_effect).unwrap();
        assert_eq!(bytes[0], packet_ids::EFFECT_RELIABLE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            reliable_effect
        );

        let drop_item = PacketKind::DropItemCallPacket(DropItemCallPacket { angle: -45.5 });
        let bytes = PacketSerializer::write_packet_kind(&drop_item).unwrap();
        assert_eq!(bytes[0], packet_ids::DROP_ITEM_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            drop_item
        );

        let delete_plans = PacketKind::DeletePlansCallPacket(DeletePlansCallPacket {
            player_id: Some(321),
            positions: vec![
                crate::mindustry::world::point2_pack(1, 2),
                crate::mindustry::world::point2_pack(3, 4),
            ],
        });
        let bytes = PacketSerializer::write_packet_kind(&delete_plans).unwrap();
        assert_eq!(bytes[0], packet_ids::DELETE_PLANS_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            PacketKind::DeletePlansCallPacket(DeletePlansCallPacket {
                player_id: None,
                positions: vec![
                    crate::mindustry::world::point2_pack(1, 2),
                    crate::mindustry::world::point2_pack(3, 4),
                ]
            })
        );

        let destroy_payload = PacketKind::DestroyPayloadCallPacket(DestroyPayloadCallPacket {
            build_pos: Some(crate::mindustry::world::point2_pack(5, 6)),
        });
        let bytes = PacketSerializer::write_packet_kind(&destroy_payload).unwrap();
        assert_eq!(bytes[0], packet_ids::DESTROY_PAYLOAD_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            destroy_payload
        );

        let entity_snapshot = PacketKind::EntitySnapshotCallPacket(EntitySnapshotCallPacket {
            amount: 2,
            data: vec![9, 8, 7],
        });
        let bytes = PacketSerializer::write_packet_kind(&entity_snapshot).unwrap();
        assert_eq!(bytes[0], packet_ids::ENTITY_SNAPSHOT_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            entity_snapshot
        );

        let follow_up = PacketKind::FollowUpMenuCallPacket(FollowUpMenuCallPacket {
            menu_id: 77,
            title: Some("title".into()),
            message: None,
            options: vec![vec![Some("A".into()), Some("B".into())]],
        });
        let bytes = PacketSerializer::write_packet_kind(&follow_up).unwrap();
        assert_eq!(bytes[0], packet_ids::FOLLOW_UP_MENU_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            follow_up
        );

        let game_over = PacketKind::GameOverCallPacket(GameOverCallPacket { winner: TeamId(6) });
        let bytes = PacketSerializer::write_packet_kind(&game_over).unwrap();
        assert_eq!(bytes[0], packet_ids::GAME_OVER_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            game_over
        );

        let hidden_snapshot = PacketKind::HiddenSnapshotCallPacket(HiddenSnapshotCallPacket {
            ids: vec![3, -4, 5],
        });
        let bytes = PacketSerializer::write_packet_kind(&hidden_snapshot).unwrap();
        assert_eq!(bytes[0], packet_ids::HIDDEN_SNAPSHOT_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            hidden_snapshot
        );

        let debug = PacketKind::DebugStatusClientCallPacket(DebugStatusClientCallPacket {
            value: 1,
            last_client_snapshot: 2,
            snapshots_sent: 3,
        });
        let bytes = PacketSerializer::write_packet_kind(&debug).unwrap();
        assert_eq!(bytes[0], packet_ids::DEBUG_STATUS_CLIENT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), debug);

        let popup = PacketKind::InfoPopupCallPacket(InfoPopupCallPacket {
            message: Some("popup".into()),
            duration: 2.5,
            align: 1,
            top: 2,
            left: 3,
            bottom: 4,
            right: 5,
        });
        let bytes = PacketSerializer::write_packet_kind(&popup).unwrap();
        assert_eq!(bytes[0], packet_ids::INFO_POPUP_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), popup);

        let popup_with_id = PacketKind::InfoPopupCallPacket2(InfoPopupCallPacket2 {
            message: None,
            id: Some("objective".into()),
            duration: 3.0,
            align: 6,
            top: 7,
            left: 8,
            bottom: 9,
            right: 10,
        });
        let bytes = PacketSerializer::write_packet_kind(&popup_with_id).unwrap();
        assert_eq!(bytes[0], packet_ids::INFO_POPUP_CALL_PACKET2);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            popup_with_id
        );

        let reliable_popup = PacketKind::InfoPopupReliableCallPacket(InfoPopupReliableCallPacket(
            InfoPopupCallPacket {
                message: Some("reliable popup".into()),
                duration: 1.0,
                align: 11,
                top: 12,
                left: 13,
                bottom: 14,
                right: 15,
            },
        ));
        let bytes = PacketSerializer::write_packet_kind(&reliable_popup).unwrap();
        assert_eq!(bytes[0], packet_ids::INFO_POPUP_RELIABLE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            reliable_popup
        );

        let reliable_popup_with_id = PacketKind::InfoPopupReliableCallPacket2(
            InfoPopupReliableCallPacket2(InfoPopupCallPacket2 {
                message: Some("reliable popup id".into()),
                id: Some("id".into()),
                duration: 1.25,
                align: 16,
                top: 17,
                left: 18,
                bottom: 19,
                right: 20,
            }),
        );
        let bytes = PacketSerializer::write_packet_kind(&reliable_popup_with_id).unwrap();
        assert_eq!(bytes[0], packet_ids::INFO_POPUP_RELIABLE_CALL_PACKET2);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            reliable_popup_with_id
        );

        let kick_string = PacketKind::KickCallPacket(KickCallPacket {
            reason: "custom".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&kick_string).unwrap();
        assert_eq!(bytes[0], packet_ids::KICK_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            kick_string
        );

        let kick_reason = PacketKind::KickCallPacket2(KickCallPacket2 {
            reason: crate::mindustry::net::KickReason::ServerClose,
        });
        let bytes = PacketSerializer::write_packet_kind(&kick_reason).unwrap();
        assert_eq!(bytes[0], packet_ids::KICK_CALL_PACKET2);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            kick_reason
        );

        let label = PacketKind::LabelCallPacket(LabelCallPacket {
            message: Some("label".into()),
            duration: 1.5,
            world_x: 10.0,
            world_y: -2.0,
        });
        let bytes = PacketSerializer::write_packet_kind(&label).unwrap();
        assert_eq!(bytes[0], packet_ids::LABEL_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), label);

        let label_with_id = PacketKind::LabelCallPacket2(LabelCallPacket2 {
            message: None,
            id: 42,
            duration: 2.0,
            world_x: 4.0,
            world_y: 5.0,
        });
        let bytes = PacketSerializer::write_packet_kind(&label_with_id).unwrap();
        assert_eq!(bytes[0], packet_ids::LABEL_CALL_PACKET2);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            label_with_id
        );

        let reliable_label =
            PacketKind::LabelReliableCallPacket(LabelReliableCallPacket(LabelCallPacket {
                message: Some("reliable label".into()),
                duration: 3.0,
                world_x: 6.0,
                world_y: 7.0,
            }));
        let bytes = PacketSerializer::write_packet_kind(&reliable_label).unwrap();
        assert_eq!(bytes[0], packet_ids::LABEL_RELIABLE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            reliable_label
        );

        let reliable_label_with_id =
            PacketKind::LabelReliableCallPacket2(LabelReliableCallPacket2(LabelCallPacket2 {
                message: Some("reliable label id".into()),
                id: 43,
                duration: 4.0,
                world_x: 8.0,
                world_y: 9.0,
            }));
        let bytes = PacketSerializer::write_packet_kind(&reliable_label_with_id).unwrap();
        assert_eq!(bytes[0], packet_ids::LABEL_RELIABLE_CALL_PACKET2);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            reliable_label_with_id
        );

        let landing = PacketKind::LandingPadLandedCallPacket(LandingPadLandedCallPacket {
            tile: Some(crate::mindustry::world::point2_pack(1, 2)),
        });
        let bytes = PacketSerializer::write_packet_kind(&landing).unwrap();
        assert_eq!(bytes[0], packet_ids::LANDING_PAD_LANDED_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), landing);

        let logic = PacketKind::LogicExplosionCallPacket(LogicExplosionCallPacket {
            team: TeamId(3),
            x: 1.0,
            y: 2.0,
            radius: 3.0,
            damage: 4.0,
            air: true,
            ground: false,
            pierce: true,
            effect: false,
        });
        let bytes = PacketSerializer::write_packet_kind(&logic).unwrap();
        assert_eq!(bytes[0], packet_ids::LOGIC_EXPLOSION_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), logic);

        let menu = PacketKind::MenuCallPacket(MenuCallPacket {
            menu_id: 5,
            title: "title".into(),
            message: "message".into(),
            options: vec![vec![Some("A".into()), None]],
        });
        let bytes = PacketSerializer::write_packet_kind(&menu).unwrap();
        assert_eq!(bytes[0], packet_ids::MENU_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), menu);

        let menu_choose = PacketKind::MenuChooseCallPacket(MenuChooseCallPacket {
            player_id: Some(123),
            menu_id: 5,
            option: 1,
        });
        let bytes = PacketSerializer::write_packet_kind(&menu_choose).unwrap();
        assert_eq!(bytes[0], packet_ids::MENU_CHOOSE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            PacketKind::MenuChooseCallPacket(MenuChooseCallPacket {
                player_id: None,
                menu_id: 5,
                option: 1,
            })
        );

        let uri = PacketKind::OpenUriCallPacket(OpenUriCallPacket {
            uri: "https://example.invalid".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&uri).unwrap();
        assert_eq!(bytes[0], packet_ids::OPEN_URI_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), uri);

        let payload_drop = PacketKind::PayloadDroppedCallPacket(PayloadDroppedCallPacket {
            unit: UnitRef::Unit { id: 77 },
            x: 4.0,
            y: 5.0,
        });
        let bytes = PacketSerializer::write_packet_kind(&payload_drop).unwrap();
        assert_eq!(bytes[0], packet_ids::PAYLOAD_DROPPED_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            payload_drop
        );

        let picked_build = PacketKind::PickedBuildPayloadCallPacket(PickedBuildPayloadCallPacket {
            unit: UnitRef::Unit { id: 78 },
            build_pos: Some(crate::mindustry::world::point2_pack(3, 4)),
            on_ground: true,
        });
        let bytes = PacketSerializer::write_packet_kind(&picked_build).unwrap();
        assert_eq!(bytes[0], packet_ids::PICKED_BUILD_PAYLOAD_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            picked_build
        );

        let picked_unit = PacketKind::PickedUnitPayloadCallPacket(PickedUnitPayloadCallPacket {
            unit: UnitRef::Unit { id: 79 },
            target: UnitRef::Unit { id: 80 },
        });
        let bytes = PacketSerializer::write_packet_kind(&picked_unit).unwrap();
        assert_eq!(bytes[0], packet_ids::PICKED_UNIT_PAYLOAD_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            picked_unit
        );

        let ping = PacketKind::PingCallPacket(PingCallPacket { time: 123456789 });
        let bytes = PacketSerializer::write_packet_kind(&ping).unwrap();
        assert_eq!(bytes[0], packet_ids::PING_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), ping);

        let ping_location = PacketKind::PingLocationCallPacket(PingLocationCallPacket {
            player_id: Some(7),
            x: 9.0,
            y: 10.0,
            text: "look".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&ping_location).unwrap();
        assert_eq!(bytes[0], packet_ids::PING_LOCATION_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            PacketKind::PingLocationCallPacket(PingLocationCallPacket {
                player_id: None,
                x: 9.0,
                y: 10.0,
                text: "look".into(),
            })
        );

        let camera =
            PacketKind::SetCameraPositionCallPacket(SetCameraPositionCallPacket { x: 1.0, y: 2.0 });
        let bytes = PacketSerializer::write_packet_kind(&camera).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_CAMERA_POSITION_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), camera);

        let flag = PacketKind::SetFlagCallPacket(SetFlagCallPacket {
            flag: "objective".into(),
            add: true,
        });
        let bytes = PacketSerializer::write_packet_kind(&flag).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_FLAG_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), flag);

        let hud = PacketKind::SetHudTextCallPacket(SetHudTextCallPacket {
            message: "hud".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&hud).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_HUD_TEXT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), hud);

        let reliable_hud = PacketKind::SetHudTextReliableCallPacket(SetHudTextReliableCallPacket(
            SetHudTextCallPacket {
                message: "hud reliable".into(),
            },
        ));
        let bytes = PacketSerializer::write_packet_kind(&reliable_hud).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_HUD_TEXT_RELIABLE_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            reliable_hud
        );

        let area = PacketKind::SetMapAreaCallPacket(SetMapAreaCallPacket {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        });
        let bytes = PacketSerializer::write_packet_kind(&area).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_MAP_AREA_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), area);

        let rule = PacketKind::SetRuleCallPacket(SetRuleCallPacket {
            rule: "unitCap".into(),
            json_data: "{\"value\":42}".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&rule).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_RULE_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), rule);

        let prompt = PacketKind::TextInputCallPacket(TextInputCallPacket {
            text_input_id: 1,
            title: "title".into(),
            message: "message".into(),
            text_length: 64,
            default_text: "default".into(),
            numeric: false,
        });
        let bytes = PacketSerializer::write_packet_kind(&prompt).unwrap();
        assert_eq!(bytes[0], packet_ids::TEXT_INPUT_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), prompt);

        let prompt2 = PacketKind::TextInputCallPacket2(TextInputCallPacket2 {
            prompt: TextInputCallPacket {
                text_input_id: 2,
                title: "title2".into(),
                message: "message2".into(),
                text_length: 32,
                default_text: String::new(),
                numeric: true,
            },
            allow_empty: true,
        });
        let bytes = PacketSerializer::write_packet_kind(&prompt2).unwrap();
        assert_eq!(bytes[0], packet_ids::TEXT_INPUT_CALL_PACKET2);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), prompt2);
    }

    #[test]
    fn generated_loader_call_packets_roundtrip_with_and_without_content_loader() {
        let loader = ContentLoader::create_base_content().unwrap();

        let sound = PacketKind::SoundCallPacket(SoundCallPacket {
            sound_id: 7,
            volume: 0.75,
            pitch: 1.25,
            pan: -0.5,
        });
        let bytes = PacketSerializer::write_packet_kind(&sound).unwrap();
        assert_eq!(bytes[0], packet_ids::SOUND_CALL_PACKET);
        assert_eq!(PacketSerializer::read_packet_kind(&bytes).unwrap(), sound);

        let snapshot = PacketKind::StateSnapshotCallPacket(StateSnapshotCallPacket {
            wave_time: 12.5,
            wave: 3,
            enemies: 45,
            paused: true,
            game_over: false,
            time_data: 123,
            tps: 60,
            rand0: 1,
            rand1: -2,
            core_data: vec![9, 8, 7],
        });
        let bytes = PacketSerializer::write_packet_kind(&snapshot).unwrap();
        assert_eq!(bytes[0], packet_ids::STATE_SNAPSHOT_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            snapshot
        );

        let tile_floors = PacketKind::SetTileFloorsCallPacket(SetTileFloorsCallPacket {
            block: Some("duo".into()),
            positions: vec![
                crate::mindustry::world::point2_pack(1, 2),
                crate::mindustry::world::point2_pack(3, 4),
            ],
        });
        let bytes = PacketSerializer::write_packet_kind(&tile_floors).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_TILE_FLOORS_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            tile_floors
        );
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            tile_floors
        );
        assert_eq!(
            PacketSerializer::write_packet_kind_with_loader(&tile_floors, &loader).unwrap(),
            bytes
        );

        let unit_command = PacketKind::SetUnitCommandCallPacket(SetUnitCommandCallPacket {
            player: EntityRef::null(),
            unit_ids: vec![11, 12, 13],
            command: "move".into(),
        });
        let bytes = PacketSerializer::write_packet_kind(&unit_command).unwrap();
        assert_eq!(bytes[0], packet_ids::SET_UNIT_COMMAND_CALL_PACKET);
        assert_eq!(
            PacketSerializer::read_packet_kind(&bytes).unwrap(),
            unit_command
        );
        assert_eq!(
            PacketSerializer::read_packet_kind_with_loader(&bytes, &loader).unwrap(),
            unit_command
        );
        assert_eq!(
            PacketSerializer::write_packet_kind_with_loader(&unit_command, &loader).unwrap(),
            bytes
        );

        let building_ref = BuildingRef::new(crate::mindustry::world::point2_pack(5, 6));
        assert_eq!(
            building_ref.tile_pos,
            Some(crate::mindustry::world::point2_pack(5, 6))
        );
    }
}
