use crate::session_state::{
    AppliedBlockSnapshotEnvelope, AppliedHiddenSnapshotIds, AppliedStateSnapshot,
    AppliedStateSnapshotCoreData, AppliedStateSnapshotCoreDataItem,
    AppliedStateSnapshotCoreDataTeam, BlockSnapshotHeadProjection, GameplayStateProjection,
    SessionState, StateSnapshotAuthorityProjection, StateSnapshotBusinessProjection,
};
use crate::state_snapshot_semantics::{
    derive_state_snapshot_core_inventory_transition, StateSnapshotCoreInventoryPrevious,
};
use mdt_remote::HighFrequencyRemoteMethod;
use mdt_world::parse_building_base_snapshot_bytes;
use std::collections::BTreeSet;
use std::fmt;

const HIDDEN_SNAPSHOT_SAMPLE_LIMIT: usize = 4;
const CORE_INVENTORY_CHANGED_TEAM_SAMPLE_LIMIT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub tick: u64,
}

impl Snapshot {
    pub const fn new(tick: u64) -> Self {
        Self { tick }
    }
}

pub fn ingest_snapshot(state: &mut SessionState, snapshot: Snapshot) {
    state.last_applied_tick = state.last_applied_tick.max(snapshot.tick);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboundSnapshot<'a> {
    pub method: HighFrequencyRemoteMethod,
    pub packet_id: u8,
    pub payload: &'a [u8],
}

impl<'a> InboundSnapshot<'a> {
    pub const fn new(method: HighFrequencyRemoteMethod, packet_id: u8, payload: &'a [u8]) -> Self {
        Self {
            method,
            packet_id,
            payload,
        }
    }
}

pub fn ingest_inbound_snapshot(state: &mut SessionState, snapshot: InboundSnapshot<'_>) {
    state.received_snapshot_count = state.received_snapshot_count.saturating_add(1);
    state.last_snapshot_packet_id = Some(snapshot.packet_id);
    state.last_snapshot_method = Some(snapshot.method);
    state.last_snapshot_payload_len = snapshot.payload.len();

    match snapshot.method {
        HighFrequencyRemoteMethod::ClientSnapshot => {}
        HighFrequencyRemoteMethod::StateSnapshot => {
            state.seen_state_snapshot = true;
            match try_parse_state_snapshot(snapshot.payload) {
                Ok(parsed) => {
                    let parsed_core_data = try_parse_state_snapshot_core_data(&parsed.core_data);
                    let parsed_core_data_len = parsed.core_data.len();
                    state.apply_state_snapshot_runtime(
                        &parsed,
                        parsed_core_data.as_ref().ok(),
                        parsed_core_data.is_err(),
                    );
                    let authority_projection = derive_state_snapshot_authority_projection(
                        state.state_snapshot_authority_projection.as_ref(),
                        &parsed,
                        parsed_core_data.as_ref().ok(),
                        parsed_core_data.is_err(),
                    );
                    let business_projection = derive_state_snapshot_business_projection(
                        state.state_snapshot_business_projection.as_ref(),
                        &parsed,
                        parsed_core_data.as_ref().ok(),
                    );
                    state.applied_state_snapshot_count =
                        state.applied_state_snapshot_count.saturating_add(1);
                    state.last_state_snapshot = Some(parsed);
                    state.state_snapshot_authority_projection = Some(authority_projection);
                    state.state_snapshot_business_projection = Some(business_projection);
                    match parsed_core_data {
                        Ok(core_data) => {
                            let core_inventory = derive_state_snapshot_core_inventory_transition(
                                None,
                                Some(&core_data),
                            );
                            state.last_state_snapshot_core_data = Some(core_data.clone());
                            state.last_good_state_snapshot_core_data = Some(core_data);
                            state.last_state_snapshot_core_data_duplicate_team_count =
                                core_inventory.inventory.duplicate_team_count;
                            state.last_state_snapshot_core_data_duplicate_item_count =
                                core_inventory.inventory.duplicate_item_count;
                            state.state_snapshot_core_data_duplicate_team_count_total = state
                                .state_snapshot_core_data_duplicate_team_count_total
                                .saturating_add(
                                    core_inventory.inventory.duplicate_team_count as u64,
                                );
                            state.state_snapshot_core_data_duplicate_item_count_total = state
                                .state_snapshot_core_data_duplicate_item_count_total
                                .saturating_add(
                                    core_inventory.inventory.duplicate_item_count as u64,
                                );
                            state.last_state_snapshot_core_data_parse_error = None;
                            state.last_state_snapshot_core_data_parse_error_payload_len = None;
                        }
                        Err(error) => {
                            state.last_state_snapshot_core_data = None;
                            state.last_state_snapshot_core_data_duplicate_team_count = 0;
                            state.last_state_snapshot_core_data_duplicate_item_count = 0;
                            state.failed_state_snapshot_core_data_parse_count = state
                                .failed_state_snapshot_core_data_parse_count
                                .saturating_add(1);
                            state.last_state_snapshot_core_data_parse_error =
                                Some(error.to_string());
                            state.last_state_snapshot_core_data_parse_error_payload_len =
                                Some(parsed_core_data_len);
                        }
                    }
                }
                Err(error) => {
                    state.failed_state_snapshot_parse_count =
                        state.failed_state_snapshot_parse_count.saturating_add(1);
                    state.last_state_snapshot_parse_error = Some(error.to_string());
                    state.last_state_snapshot_parse_error_payload_len =
                        Some(snapshot.payload.len());
                }
            }
        }
        HighFrequencyRemoteMethod::EntitySnapshot => state.seen_entity_snapshot = true,
        HighFrequencyRemoteMethod::BlockSnapshot => {
            state.seen_block_snapshot = true;
            state.received_block_snapshot_count =
                state.received_block_snapshot_count.saturating_add(1);
            state.last_block_snapshot_payload_len = Some(snapshot.payload.len());
            match try_parse_block_snapshot_envelope(snapshot.payload) {
                Ok(parsed) => {
                    state.applied_block_snapshot_count =
                        state.applied_block_snapshot_count.saturating_add(1);
                    let head_projection = parsed.first_build_pos.zip(parsed.first_block_id).map(
                        |(build_pos, block_id)| BlockSnapshotHeadProjection {
                            build_pos,
                            block_id,
                            health_bits: parsed.first_health_bits,
                            rotation: parsed.first_rotation,
                            team_id: parsed.first_team_id,
                            io_version: parsed.first_io_version,
                            enabled: parsed.first_enabled,
                            module_bitmask: parsed.first_module_bitmask,
                            time_scale_bits: parsed.first_time_scale_bits,
                            time_scale_duration_bits: parsed.first_time_scale_duration_bits,
                            last_disabler_pos: parsed.first_last_disabler_pos,
                            legacy_consume_connected: parsed.first_legacy_consume_connected,
                            efficiency: parsed.first_efficiency,
                            optional_efficiency: parsed.first_optional_efficiency,
                            visible_flags: parsed.first_visible_flags,
                        },
                    );
                    if !state.suppress_block_snapshot_head_table_apply {
                        if let Some(head) = head_projection.as_ref() {
                            state.building_table_projection.apply_block_snapshot_head(
                                head.build_pos,
                                head.block_id,
                                None,
                                head.rotation,
                                head.team_id,
                                head.io_version,
                                head.module_bitmask,
                                head.time_scale_bits,
                                head.time_scale_duration_bits,
                                head.last_disabler_pos,
                                head.legacy_consume_connected,
                                None,
                                head.health_bits,
                                head.enabled,
                                head.efficiency,
                                head.optional_efficiency,
                                head.visible_flags,
                                None,
                                None,
                                None,
                            );
                            state.refresh_runtime_typed_building_from_tables(head.build_pos);
                        }
                    }
                    state.block_snapshot_head_projection = head_projection;
                    state.last_block_snapshot = Some(parsed);
                    state.last_block_snapshot_parse_error = None;
                    state.last_block_snapshot_parse_error_payload_len = None;
                }
                Err(error) => {
                    state.failed_block_snapshot_parse_count =
                        state.failed_block_snapshot_parse_count.saturating_add(1);
                    state.block_snapshot_head_projection = None;
                    state.last_block_snapshot_parse_error = Some(error.to_string());
                    state.last_block_snapshot_parse_error_payload_len =
                        Some(snapshot.payload.len());
                }
            }
        }
        HighFrequencyRemoteMethod::HiddenSnapshot => {
            state.seen_hidden_snapshot = true;
            state.received_hidden_snapshot_count =
                state.received_hidden_snapshot_count.saturating_add(1);
            state.last_hidden_snapshot_payload_len = Some(snapshot.payload.len());
            match try_parse_hidden_snapshot_ids(snapshot.payload) {
                Ok(parsed) => {
                    let trigger_hidden_ids = parsed.ids.iter().copied().collect::<BTreeSet<_>>();
                    state.apply_hidden_snapshot(parsed.applied, trigger_hidden_ids);
                    state.last_hidden_snapshot_parse_error = None;
                    state.last_hidden_snapshot_parse_error_payload_len = None;
                }
                Err(error) => {
                    state.failed_hidden_snapshot_parse_count =
                        state.failed_hidden_snapshot_parse_count.saturating_add(1);
                    state.last_hidden_snapshot_parse_error = Some(error.to_string());
                    state.last_hidden_snapshot_parse_error_payload_len =
                        Some(snapshot.payload.len());
                }
            }
        }
    }
}

fn derive_state_snapshot_authority_projection(
    previous: Option<&StateSnapshotAuthorityProjection>,
    snapshot: &AppliedStateSnapshot,
    core_data: Option<&AppliedStateSnapshotCoreData>,
    core_data_parse_failed: bool,
) -> StateSnapshotAuthorityProjection {
    let previous_wave = previous
        .map(|projection| projection.wave)
        .unwrap_or_default();
    let previous_time_data = previous
        .map(|projection| projection.time_data)
        .unwrap_or_default();
    let last_wave_advanced = snapshot.wave > previous_wave;
    let wave_advance_count = previous
        .map(|projection| projection.wave_advance_count)
        .unwrap_or_default()
        .saturating_add(u64::from(last_wave_advanced));
    let last_net_seconds_rollback = snapshot.time_data < previous_time_data;
    let net_seconds_delta_i64 = i64::from(snapshot.time_data) - i64::from(previous_time_data);
    let net_seconds_delta =
        net_seconds_delta_i64.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let gameplay_state = if snapshot.game_over {
        GameplayStateProjection::GameOver
    } else if snapshot.paused {
        GameplayStateProjection::Paused
    } else {
        GameplayStateProjection::Playing
    };
    let core_inventory = derive_state_snapshot_core_inventory_transition(
        previous.map(|previous| StateSnapshotCoreInventoryPrevious {
            inventory_by_team: &previous.core_inventory_by_team,
            item_entry_count: previous.core_inventory_item_entry_count,
            total_amount: previous.core_inventory_total_amount,
            nonzero_item_count: previous.core_inventory_nonzero_item_count,
        }),
        core_data,
    );
    let core_inventory_changed_team_sample = core_inventory
        .changed_team_ids
        .iter()
        .take(CORE_INVENTORY_CHANGED_TEAM_SAMPLE_LIMIT)
        .copied()
        .collect();

    StateSnapshotAuthorityProjection {
        wave_time_bits: snapshot.wave_time_bits,
        wave: snapshot.wave,
        enemies: snapshot.enemies,
        paused: snapshot.paused,
        game_over: snapshot.game_over,
        time_data: snapshot.time_data,
        tps: snapshot.tps,
        rand0: snapshot.rand0,
        rand1: snapshot.rand1,
        gameplay_state,
        last_wave_advanced,
        wave_advance_count,
        state_snapshot_apply_count: previous
            .map(|projection| projection.state_snapshot_apply_count)
            .unwrap_or_default()
            .saturating_add(1),
        last_net_seconds_rollback,
        net_seconds_delta,
        state_snapshot_wave_regress_count: previous
            .map(|projection| projection.state_snapshot_wave_regress_count)
            .unwrap_or_default()
            .saturating_add(u64::from(snapshot.wave < previous_wave)),
        core_inventory_team_count: core_inventory.inventory.inventory_by_team.len(),
        core_inventory_item_entry_count: core_inventory.inventory.item_entry_count,
        core_inventory_total_amount: core_inventory.inventory.total_amount,
        core_inventory_nonzero_item_count: core_inventory.inventory.nonzero_item_count,
        core_inventory_changed_team_count: core_inventory.changed_team_ids.len(),
        core_inventory_changed_team_sample,
        core_inventory_by_team: core_inventory.inventory.inventory_by_team,
        last_core_sync_ok: core_inventory.synced,
        core_parse_fail_count: previous
            .map(|projection| projection.core_parse_fail_count)
            .unwrap_or_default()
            .saturating_add(u64::from(core_data_parse_failed)),
    }
}

fn derive_state_snapshot_business_projection(
    previous: Option<&StateSnapshotBusinessProjection>,
    snapshot: &AppliedStateSnapshot,
    core_data: Option<&AppliedStateSnapshotCoreData>,
) -> StateSnapshotBusinessProjection {
    let previous_wave = previous
        .map(|projection| projection.wave)
        .unwrap_or_default();
    let previous_time_data = previous
        .map(|projection| projection.time_data)
        .unwrap_or_default();
    let last_wave_advanced = snapshot.wave > previous_wave;
    let last_wave_advance_from = last_wave_advanced.then_some(previous_wave);
    let last_wave_advance_to = last_wave_advanced.then_some(snapshot.wave);
    let wave_advance_count = previous
        .map(|projection| projection.wave_advance_count)
        .unwrap_or_default()
        .saturating_add(u64::from(last_wave_advanced));
    let last_net_seconds_rollback = snapshot.time_data < previous_time_data;
    let net_seconds_delta_i64 = i64::from(snapshot.time_data) - i64::from(previous_time_data);
    let net_seconds_delta =
        net_seconds_delta_i64.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let gameplay_state = if snapshot.game_over {
        GameplayStateProjection::GameOver
    } else if snapshot.paused {
        GameplayStateProjection::Paused
    } else {
        GameplayStateProjection::Playing
    };
    let gameplay_state_transition_count = previous
        .map(|projection| projection.gameplay_state_transition_count)
        .unwrap_or_default()
        .saturating_add(u64::from(
            previous
                .map(|projection| projection.gameplay_state != gameplay_state)
                .unwrap_or(false),
        ));
    let core_inventory = derive_state_snapshot_core_inventory_transition(
        previous.map(|previous| StateSnapshotCoreInventoryPrevious {
            inventory_by_team: &previous.core_inventory_by_team,
            item_entry_count: previous.core_inventory_item_entry_count,
            total_amount: previous.core_inventory_total_amount,
            nonzero_item_count: previous.core_inventory_nonzero_item_count,
        }),
        core_data,
    );
    let core_inventory_changed_team_sample = core_inventory
        .changed_team_ids
        .iter()
        .take(CORE_INVENTORY_CHANGED_TEAM_SAMPLE_LIMIT)
        .copied()
        .collect();

    StateSnapshotBusinessProjection {
        wave_time_bits: snapshot.wave_time_bits,
        wave: snapshot.wave,
        enemies: snapshot.enemies,
        paused: snapshot.paused,
        game_over: snapshot.game_over,
        time_data: snapshot.time_data,
        tps: snapshot.tps,
        rand0: snapshot.rand0,
        rand1: snapshot.rand1,
        gameplay_state,
        gameplay_state_transition_count,
        last_wave_advanced,
        last_wave_advance_from,
        last_wave_advance_to,
        wave_advance_count,
        net_seconds_applied_count: previous
            .map(|projection| projection.net_seconds_applied_count)
            .unwrap_or_default()
            .saturating_add(1),
        last_net_seconds_rollback,
        net_seconds_delta,
        state_snapshot_apply_count: previous
            .map(|projection| projection.state_snapshot_apply_count)
            .unwrap_or_default()
            .saturating_add(1),
        state_snapshot_time_regress_count: previous
            .map(|projection| projection.state_snapshot_time_regress_count)
            .unwrap_or_default()
            .saturating_add(u64::from(last_net_seconds_rollback)),
        state_snapshot_wave_regress_count: previous
            .map(|projection| projection.state_snapshot_wave_regress_count)
            .unwrap_or_default()
            .saturating_add(u64::from(snapshot.wave < previous_wave)),
        core_inventory_synced: core_inventory.synced,
        core_inventory_team_count: core_inventory.inventory.inventory_by_team.len(),
        core_inventory_item_entry_count: core_inventory.inventory.item_entry_count,
        core_inventory_total_amount: core_inventory.inventory.total_amount,
        core_inventory_nonzero_item_count: core_inventory.inventory.nonzero_item_count,
        core_inventory_changed_team_count: core_inventory.changed_team_ids.len(),
        core_inventory_changed_team_sample,
        core_inventory_by_team: core_inventory.inventory.inventory_by_team,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateSnapshotParseError {
    Truncated,
    TrailingBytes { consumed: usize, total: usize },
}

impl fmt::Display for StateSnapshotParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated_state_snapshot_payload"),
            Self::TrailingBytes { consumed, total } => {
                write!(f, "state_snapshot_trailing_bytes:{consumed}/{total}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateSnapshotCoreDataParseError {
    Truncated,
    TrailingBytes { consumed: usize, total: usize },
}

impl fmt::Display for StateSnapshotCoreDataParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated_state_snapshot_core_data"),
            Self::TrailingBytes { consumed, total } => {
                write!(
                    f,
                    "state_snapshot_core_data_trailing_bytes:{consumed}/{total}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockSnapshotParseError {
    Truncated,
    NegativeAmount(i16),
    TruncatedFirstEntryHeader { data_len: usize },
    TruncatedFirstEntryPrefix { data_len: usize },
    InvalidFirstEntryBase(String),
    TrailingBytes { consumed: usize, total: usize },
}

impl fmt::Display for BlockSnapshotParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated_block_snapshot_payload"),
            Self::NegativeAmount(amount) => write!(f, "negative_block_snapshot_amount:{amount}"),
            Self::TruncatedFirstEntryHeader { data_len } => {
                write!(f, "truncated_block_snapshot_first_entry_header:{data_len}")
            }
            Self::TruncatedFirstEntryPrefix { data_len } => {
                write!(f, "truncated_block_snapshot_first_entry_prefix:{data_len}")
            }
            Self::InvalidFirstEntryBase(error) => {
                write!(f, "invalid_block_snapshot_first_entry_base:{error}")
            }
            Self::TrailingBytes { consumed, total } => {
                write!(f, "block_snapshot_trailing_bytes:{consumed}/{total}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HiddenSnapshotParseError {
    Truncated,
    NegativeCount(i32),
    TrailingBytes { consumed: usize, total: usize },
}

impl fmt::Display for HiddenSnapshotParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated_hidden_snapshot_payload"),
            Self::NegativeCount(count) => write!(f, "negative_hidden_snapshot_count:{count}"),
            Self::TrailingBytes { consumed, total } => {
                write!(f, "hidden_snapshot_trailing_bytes:{consumed}/{total}")
            }
        }
    }
}

fn try_parse_state_snapshot(
    payload: &[u8],
) -> Result<AppliedStateSnapshot, StateSnapshotParseError> {
    let mut cursor = 0;
    let snapshot = AppliedStateSnapshot {
        wave_time_bits: read_u32(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        wave: read_i32(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        enemies: read_i32(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        paused: read_bool(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        game_over: read_bool(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        time_data: read_i32(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        tps: read_u8(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        rand0: read_i64(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        rand1: read_i64(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
        core_data: read_bytes(payload, &mut cursor).ok_or(StateSnapshotParseError::Truncated)?,
    };
    if cursor != payload.len() {
        return Err(StateSnapshotParseError::TrailingBytes {
            consumed: cursor,
            total: payload.len(),
        });
    }
    Ok(snapshot)
}

fn try_parse_state_snapshot_core_data(
    payload: &[u8],
) -> Result<AppliedStateSnapshotCoreData, StateSnapshotCoreDataParseError> {
    const TEAM_HEADER_LEN: usize = 3;
    const ITEM_ENTRY_LEN: usize = 6;

    let mut cursor = 0;
    let team_count =
        read_u8(payload, &mut cursor).ok_or(StateSnapshotCoreDataParseError::Truncated)?;
    if !count_fits_remaining_bytes(
        team_count as usize,
        payload.len().saturating_sub(cursor),
        TEAM_HEADER_LEN,
    ) {
        return Err(StateSnapshotCoreDataParseError::Truncated);
    }
    let mut teams = Vec::with_capacity(team_count as usize);
    for _ in 0..team_count {
        let team_id =
            read_u8(payload, &mut cursor).ok_or(StateSnapshotCoreDataParseError::Truncated)?;
        let item_count =
            read_u16(payload, &mut cursor).ok_or(StateSnapshotCoreDataParseError::Truncated)?;
        if !count_fits_remaining_bytes(
            item_count as usize,
            payload.len().saturating_sub(cursor),
            ITEM_ENTRY_LEN,
        ) {
            return Err(StateSnapshotCoreDataParseError::Truncated);
        }
        let mut items = Vec::with_capacity(item_count as usize);
        for _ in 0..item_count {
            let item_id =
                read_u16(payload, &mut cursor).ok_or(StateSnapshotCoreDataParseError::Truncated)?;
            let amount =
                read_i32(payload, &mut cursor).ok_or(StateSnapshotCoreDataParseError::Truncated)?;
            items.push(AppliedStateSnapshotCoreDataItem { item_id, amount });
        }
        teams.push(AppliedStateSnapshotCoreDataTeam { team_id, items });
    }
    if cursor != payload.len() {
        return Err(StateSnapshotCoreDataParseError::TrailingBytes {
            consumed: cursor,
            total: payload.len(),
        });
    }
    Ok(AppliedStateSnapshotCoreData { team_count, teams })
}

fn try_parse_block_snapshot_envelope(
    payload: &[u8],
) -> Result<AppliedBlockSnapshotEnvelope, BlockSnapshotParseError> {
    const FIRST_ENTRY_HEADER_LEN: usize = 6;
    const FIRST_ENTRY_FIXED_PREFIX_LEN: usize = 15;

    let mut cursor = 0;
    let amount = read_i16(payload, &mut cursor).ok_or(BlockSnapshotParseError::Truncated)?;
    if amount < 0 {
        return Err(BlockSnapshotParseError::NegativeAmount(amount));
    }
    let data = read_bytes(payload, &mut cursor).ok_or(BlockSnapshotParseError::Truncated)?;
    if cursor != payload.len() {
        return Err(BlockSnapshotParseError::TrailingBytes {
            consumed: cursor,
            total: payload.len(),
        });
    }
    let (
        first_build_pos,
        first_block_id,
        first_health_bits,
        first_rotation,
        first_team_id,
        first_io_version,
        first_enabled,
        first_module_bitmask,
        first_time_scale_bits,
        first_time_scale_duration_bits,
        first_last_disabler_pos,
        first_legacy_consume_connected,
        first_efficiency,
        first_optional_efficiency,
        first_visible_flags,
    ) = if amount > 0 {
        if data.len() < FIRST_ENTRY_HEADER_LEN {
            return Err(BlockSnapshotParseError::TruncatedFirstEntryHeader {
                data_len: data.len(),
            });
        }
        if data.len() < FIRST_ENTRY_FIXED_PREFIX_LEN {
            return Err(BlockSnapshotParseError::TruncatedFirstEntryPrefix {
                data_len: data.len(),
            });
        }
        let mut data_cursor = 0;
        let first_build_pos = read_i32(&data, &mut data_cursor);
        let first_block_id = read_i16(&data, &mut data_cursor);
        let (
            first_health_bits,
            first_rotation,
            first_team_id,
            first_io_version,
            first_enabled,
            first_module_bitmask,
            first_time_scale_bits,
            first_time_scale_duration_bits,
            first_last_disabler_pos,
            first_legacy_consume_connected,
            first_efficiency,
            first_optional_efficiency,
            first_visible_flags,
        ) = match parse_building_base_snapshot_bytes(&data[data_cursor..]) {
            Ok((base, _consumed)) => (
                Some(base.health_bits),
                Some(base.rotation),
                Some(base.team_id),
                base.save_version,
                base.enabled,
                base.module_bitmask,
                base.time_scale_bits,
                base.time_scale_duration_bits,
                base.last_disabler_pos,
                base.legacy_consume_connected,
                base.efficiency,
                base.optional_efficiency,
                base.visible_flags,
            ),
            Err(error) => return Err(BlockSnapshotParseError::InvalidFirstEntryBase(error)),
        };

        (
            first_build_pos,
            first_block_id,
            first_health_bits,
            first_rotation,
            first_team_id,
            first_io_version,
            first_enabled,
            first_module_bitmask,
            first_time_scale_bits,
            first_time_scale_duration_bits,
            first_last_disabler_pos,
            first_legacy_consume_connected,
            first_efficiency,
            first_optional_efficiency,
            first_visible_flags,
        )
    } else {
        (
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None,
        )
    };
    Ok(AppliedBlockSnapshotEnvelope {
        amount,
        data_len: data.len(),
        first_build_pos,
        first_block_id,
        first_health_bits,
        first_rotation,
        first_team_id,
        first_io_version,
        first_enabled,
        first_module_bitmask,
        first_time_scale_bits,
        first_time_scale_duration_bits,
        first_last_disabler_pos,
        first_legacy_consume_connected,
        first_efficiency,
        first_optional_efficiency,
        first_visible_flags,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHiddenSnapshotIds {
    applied: AppliedHiddenSnapshotIds,
    ids: Vec<i32>,
}

fn try_parse_hidden_snapshot_ids(
    payload: &[u8],
) -> Result<ParsedHiddenSnapshotIds, HiddenSnapshotParseError> {
    const HIDDEN_ID_LEN: usize = 4;

    let mut cursor = 0;
    let count = read_i32(payload, &mut cursor).ok_or(HiddenSnapshotParseError::Truncated)?;
    if count < 0 {
        return Err(HiddenSnapshotParseError::NegativeCount(count));
    }
    let count_usize = usize::try_from(count).map_err(|_| HiddenSnapshotParseError::Truncated)?;
    if !count_fits_remaining_bytes(
        count_usize,
        payload.len().saturating_sub(cursor),
        HIDDEN_ID_LEN,
    ) {
        return Err(HiddenSnapshotParseError::Truncated);
    }
    let mut first_id = None;
    let mut ids = Vec::with_capacity(count_usize);
    let mut sample_ids = Vec::with_capacity(count_usize.min(HIDDEN_SNAPSHOT_SAMPLE_LIMIT));
    for index in 0..count_usize {
        let id = read_i32(payload, &mut cursor).ok_or(HiddenSnapshotParseError::Truncated)?;
        if index == 0 {
            first_id = Some(id);
        }
        ids.push(id);
        if sample_ids.len() < HIDDEN_SNAPSHOT_SAMPLE_LIMIT {
            sample_ids.push(id);
        }
    }
    if cursor != payload.len() {
        return Err(HiddenSnapshotParseError::TrailingBytes {
            consumed: cursor,
            total: payload.len(),
        });
    }
    Ok(ParsedHiddenSnapshotIds {
        applied: AppliedHiddenSnapshotIds {
            count,
            first_id,
            sample_ids,
        },
        ids,
    })
}

fn count_fits_remaining_bytes(count: usize, remaining_bytes: usize, entry_size: usize) -> bool {
    entry_size != 0 && count <= remaining_bytes / entry_size
}

fn read_u8(payload: &[u8], cursor: &mut usize) -> Option<u8> {
    let value = *payload.get(*cursor)?;
    *cursor += 1;
    Some(value)
}

fn read_bool(payload: &[u8], cursor: &mut usize) -> Option<bool> {
    Some(read_u8(payload, cursor)? != 0)
}

fn read_u16(payload: &[u8], cursor: &mut usize) -> Option<u16> {
    let bytes: [u8; 2] = payload.get(*cursor..*cursor + 2)?.try_into().ok()?;
    *cursor += 2;
    Some(u16::from_be_bytes(bytes))
}

fn read_i32(payload: &[u8], cursor: &mut usize) -> Option<i32> {
    let bytes: [u8; 4] = payload.get(*cursor..*cursor + 4)?.try_into().ok()?;
    *cursor += 4;
    Some(i32::from_be_bytes(bytes))
}

fn read_i16(payload: &[u8], cursor: &mut usize) -> Option<i16> {
    let bytes: [u8; 2] = payload.get(*cursor..*cursor + 2)?.try_into().ok()?;
    *cursor += 2;
    Some(i16::from_be_bytes(bytes))
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

fn read_bytes(payload: &[u8], cursor: &mut usize) -> Option<Vec<u8>> {
    let len = read_u16(payload, cursor)? as usize;
    let bytes = payload.get(*cursor..*cursor + len)?.to_vec();
    *cursor += len;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::{ingest_inbound_snapshot, InboundSnapshot};
    use crate::session_state::{
        AppliedBlockSnapshotEnvelope, AppliedHiddenSnapshotIds, AppliedStateSnapshotCoreData,
        AppliedStateSnapshotCoreDataItem, AppliedStateSnapshotCoreDataTeam,
        AuthoritativeStateMirror, BlockSnapshotHeadProjection, BuildingProjectionUpdateKind,
        EntityFireSemanticProjection, EntityProjection, EntityPuddleSemanticProjection,
        EntitySemanticProjection, EntitySemanticProjectionEntry, EntityUnitSemanticProjection,
        EntityWeatherStateSemanticProjection, EntityWorldLabelSemanticProjection,
        GameplayStateProjection, HiddenSnapshotDeltaProjection, PayloadLifecycleCarrierProjection,
        ResourceUnitItemStack, SessionState, StateSnapshotAuthorityProjection,
        StateSnapshotBusinessProjection, TypedRuntimeEntityModel, UnitRefProjection,
    };
    use mdt_remote::HighFrequencyRemoteMethod;
    use std::collections::{BTreeMap, BTreeSet};

    fn build_state_snapshot_payload(
        wave: i32,
        enemies: i32,
        paused: bool,
        game_over: bool,
        time_data: i32,
        tps: u8,
        rand0: i64,
        rand1: i64,
        core_data: &[u8],
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&123.5f32.to_bits().to_be_bytes());
        payload.extend_from_slice(&wave.to_be_bytes());
        payload.extend_from_slice(&enemies.to_be_bytes());
        payload.push(u8::from(paused));
        payload.push(u8::from(game_over));
        payload.extend_from_slice(&time_data.to_be_bytes());
        payload.push(tps);
        payload.extend_from_slice(&rand0.to_be_bytes());
        payload.extend_from_slice(&rand1.to_be_bytes());
        payload.extend_from_slice(&(core_data.len() as u16).to_be_bytes());
        payload.extend_from_slice(core_data);
        payload
    }

    fn build_core_data_payload(teams: &[(u8, &[(u16, i32)])]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(u8::try_from(teams.len()).expect("team count fits in u8"));
        for (team_id, items) in teams {
            payload.push(*team_id);
            payload.extend_from_slice(
                &u16::try_from(items.len())
                    .expect("item count fits in u16")
                    .to_be_bytes(),
            );
            for (item_id, amount) in *items {
                payload.extend_from_slice(&item_id.to_be_bytes());
                payload.extend_from_slice(&amount.to_be_bytes());
            }
        }
        payload
    }

    #[test]
    fn state_snapshot_ingest_decodes_structured_core_data_into_session_state() {
        let core_data = [
            0x01, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x41, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x2d,
        ];
        let payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &core_data,
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(state.applied_state_snapshot_count, 1);
        assert_eq!(
            state
                .last_state_snapshot
                .as_ref()
                .map(|snapshot| snapshot.core_data.as_slice()),
            Some(core_data.as_slice())
        );
        assert_eq!(
            state.last_state_snapshot_core_data,
            Some(AppliedStateSnapshotCoreData {
                team_count: 1,
                teams: vec![AppliedStateSnapshotCoreDataTeam {
                    team_id: 1,
                    items: vec![
                        AppliedStateSnapshotCoreDataItem {
                            item_id: 0,
                            amount: 321,
                        },
                        AppliedStateSnapshotCoreDataItem {
                            item_id: 1,
                            amount: 45,
                        },
                    ],
                }],
            })
        );
        assert_eq!(
            state.last_good_state_snapshot_core_data,
            state.last_state_snapshot_core_data
        );
        assert_eq!(state.failed_state_snapshot_core_data_parse_count, 0);
        assert_eq!(state.last_state_snapshot_core_data_parse_error, None);
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error_payload_len,
            None
        );
        assert_eq!(state.last_state_snapshot_core_data_duplicate_team_count, 0);
        assert_eq!(state.last_state_snapshot_core_data_duplicate_item_count, 0);
        assert_eq!(state.state_snapshot_core_data_duplicate_team_count_total, 0);
        assert_eq!(state.state_snapshot_core_data_duplicate_item_count_total, 0);
        assert_eq!(
            state.authoritative_state_mirror,
            Some(AuthoritativeStateMirror {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 0,
                paused: false,
                game_over: false,
                net_seconds: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                gameplay_state: GameplayStateProjection::Playing,
                last_wave_advanced: true,
                wave_advance_count: 1,
                apply_count: 1,
                last_net_seconds_rollback: false,
                net_seconds_delta: 654_321,
                wave_regress_count: 0,
                core_inventory_team_count: 1,
                core_inventory_item_entry_count: 2,
                core_inventory_total_amount: 366,
                core_inventory_nonzero_item_count: 2,
                core_inventory_changed_team_count: 1,
                core_inventory_changed_team_sample: vec![1],
                core_inventory_by_team: BTreeMap::from([(
                    1,
                    BTreeMap::from([(0u16, 321), (1u16, 45)]),
                )]),
                last_core_sync_ok: true,
                core_parse_fail_count: 0,
            })
        );
        assert_eq!(
            state.state_snapshot_business_projection,
            Some(StateSnapshotBusinessProjection {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 0,
                paused: false,
                game_over: false,
                time_data: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                gameplay_state: GameplayStateProjection::Playing,
                gameplay_state_transition_count: 0,
                last_wave_advanced: true,
                last_wave_advance_from: Some(0),
                last_wave_advance_to: Some(7),
                wave_advance_count: 1,
                net_seconds_applied_count: 1,
                last_net_seconds_rollback: false,
                net_seconds_delta: 654_321,
                state_snapshot_apply_count: 1,
                state_snapshot_time_regress_count: 0,
                state_snapshot_wave_regress_count: 0,
                core_inventory_synced: true,
                core_inventory_team_count: 1,
                core_inventory_item_entry_count: 2,
                core_inventory_total_amount: 366,
                core_inventory_nonzero_item_count: 2,
                core_inventory_changed_team_count: 1,
                core_inventory_changed_team_sample: vec![1],
                core_inventory_by_team: BTreeMap::from([(
                    1,
                    BTreeMap::from([(0u16, 321), (1u16, 45)]),
                )]),
            })
        );
    }

    #[test]
    fn state_snapshot_ingest_uses_last_write_wins_core_inventory_metrics_for_duplicate_keys() {
        let core_data = build_core_data_payload(&[
            (1, &[(0, 10), (0, 20)]),
            (1, &[(1, 30)]),
            (2, &[(4, 40), (4, 0)]),
        ]);
        let payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &core_data,
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(state.failed_state_snapshot_core_data_parse_count, 0);
        assert_eq!(state.last_state_snapshot_core_data_duplicate_team_count, 1);
        assert_eq!(state.last_state_snapshot_core_data_duplicate_item_count, 2);
        assert_eq!(state.state_snapshot_core_data_duplicate_team_count_total, 1);
        assert_eq!(state.state_snapshot_core_data_duplicate_item_count_total, 2);
        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| projection.core_inventory_by_team.clone()),
            Some(BTreeMap::from([
                (1u8, BTreeMap::from([(0u16, 20), (1u16, 30)])),
                (2u8, BTreeMap::from([(4u16, 0)])),
            ]))
        );
        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| (
                    projection.core_inventory_item_entry_count,
                    projection.core_inventory_total_amount,
                    projection.core_inventory_nonzero_item_count,
                )),
            Some((3, 50, 2))
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| (
                    projection.core_inventory_item_entry_count,
                    projection.core_inventory_total_amount,
                    projection.core_inventory_nonzero_item_count,
                    projection.core_inventory_by_team.clone(),
                )),
            Some(BTreeMap::from([
                (1u8, BTreeMap::from([(0u16, 20), (1u16, 30)])),
                (2u8, BTreeMap::from([(4u16, 0)])),
            ]))
            .map(|inventory| (3, 50, 2, inventory))
        );
        assert_eq!(
            state.authoritative_state_mirror.as_ref().map(|mirror| (
                mirror.core_inventory_item_entry_count,
                mirror.core_inventory_total_amount,
                mirror.core_inventory_nonzero_item_count,
                mirror.core_inventory_by_team.clone(),
            )),
            Some((
                3,
                50,
                2,
                BTreeMap::from([
                    (1u8, BTreeMap::from([(0u16, 20), (1u16, 30)])),
                    (2u8, BTreeMap::from([(4u16, 0)])),
                ]),
            ))
        );
    }

    #[test]
    fn state_snapshot_ingest_tracks_core_data_parse_error_without_failing_state_snapshot() {
        let malformed_core_data = [0x01, 0x01, 0x00, 0x01, 0x00, 0x00];
        let payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &malformed_core_data,
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(state.applied_state_snapshot_count, 1);
        assert_eq!(state.failed_state_snapshot_parse_count, 0);
        assert_eq!(state.last_state_snapshot_core_data, None);
        assert_eq!(state.last_good_state_snapshot_core_data, None);
        assert_eq!(state.last_state_snapshot_core_data_duplicate_team_count, 0);
        assert_eq!(state.last_state_snapshot_core_data_duplicate_item_count, 0);
        assert_eq!(state.failed_state_snapshot_core_data_parse_count, 1);
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error.as_deref(),
            Some("truncated_state_snapshot_core_data")
        );
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error_payload_len,
            Some(malformed_core_data.len())
        );
        assert_eq!(
            state.state_snapshot_business_projection,
            Some(StateSnapshotBusinessProjection {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 0,
                paused: false,
                game_over: false,
                time_data: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                gameplay_state: GameplayStateProjection::Playing,
                gameplay_state_transition_count: 0,
                last_wave_advanced: true,
                last_wave_advance_from: Some(0),
                last_wave_advance_to: Some(7),
                wave_advance_count: 1,
                net_seconds_applied_count: 1,
                last_net_seconds_rollback: false,
                net_seconds_delta: 654_321,
                state_snapshot_apply_count: 1,
                state_snapshot_time_regress_count: 0,
                state_snapshot_wave_regress_count: 0,
                core_inventory_synced: false,
                core_inventory_team_count: 0,
                core_inventory_item_entry_count: 0,
                core_inventory_total_amount: 0,
                core_inventory_nonzero_item_count: 0,
                core_inventory_changed_team_count: 0,
                core_inventory_changed_team_sample: Vec::new(),
                core_inventory_by_team: BTreeMap::new(),
            })
        );
        assert_eq!(
            state
                .authoritative_state_mirror
                .as_ref()
                .map(|mirror| mirror.last_core_sync_ok),
            Some(false)
        );
        assert_eq!(
            state
                .authoritative_state_mirror
                .as_ref()
                .map(|mirror| mirror.wave),
            Some(7)
        );
    }

    #[test]
    fn state_snapshot_ingest_rejects_trailing_bytes_in_core_data_payload() {
        let malformed_core_data = [0x01, 0x01, 0x00, 0x00, 0xFF];
        let payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &malformed_core_data,
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(state.applied_state_snapshot_count, 1);
        assert_eq!(state.failed_state_snapshot_parse_count, 0);
        assert_eq!(state.last_state_snapshot_core_data, None);
        assert_eq!(state.failed_state_snapshot_core_data_parse_count, 1);
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error.as_deref(),
            Some("state_snapshot_core_data_trailing_bytes:4/5")
        );
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error_payload_len,
            Some(malformed_core_data.len())
        );
    }

    #[test]
    fn malformed_state_snapshot_with_trailing_bytes_rejects_entire_payload() {
        let mut payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &[],
        );
        payload.push(0xFF);
        let expected_error = format!(
            "state_snapshot_trailing_bytes:{}/{}",
            payload.len().saturating_sub(1),
            payload.len()
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(state.applied_state_snapshot_count, 0);
        assert_eq!(state.failed_state_snapshot_parse_count, 1);
        assert_eq!(
            state.last_state_snapshot_parse_error.as_deref(),
            Some(expected_error.as_str())
        );
        assert_eq!(
            state.last_state_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
        assert_eq!(state.last_state_snapshot, None);
        assert_eq!(state.state_snapshot_authority_projection, None);
    }

    #[test]
    fn state_snapshot_ingest_rejects_impossible_core_data_item_count() {
        let malformed_core_data = [0x01, 0x01, 0xFF, 0xFF];
        let payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &malformed_core_data,
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(state.applied_state_snapshot_count, 1);
        assert_eq!(state.failed_state_snapshot_parse_count, 0);
        assert_eq!(state.last_state_snapshot_core_data, None);
        assert_eq!(state.last_good_state_snapshot_core_data, None);
        assert_eq!(state.failed_state_snapshot_core_data_parse_count, 1);
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error.as_deref(),
            Some("truncated_state_snapshot_core_data")
        );
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error_payload_len,
            Some(malformed_core_data.len())
        );
    }

    #[test]
    fn state_snapshot_authority_applies_header_even_when_core_data_parse_fails() {
        let malformed_core_data = [0x01, 0x01, 0x00, 0x01, 0x00, 0x00];
        let payload = build_state_snapshot_payload(
            7,
            0,
            false,
            false,
            654_321,
            60,
            111_111_111,
            222_222_222,
            &malformed_core_data,
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(
            state.state_snapshot_authority_projection,
            Some(StateSnapshotAuthorityProjection {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 0,
                paused: false,
                game_over: false,
                time_data: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                gameplay_state: GameplayStateProjection::Playing,
                last_wave_advanced: true,
                wave_advance_count: 1,
                state_snapshot_apply_count: 1,
                last_net_seconds_rollback: false,
                net_seconds_delta: 654_321,
                state_snapshot_wave_regress_count: 0,
                core_inventory_team_count: 0,
                core_inventory_item_entry_count: 0,
                core_inventory_total_amount: 0,
                core_inventory_nonzero_item_count: 0,
                core_inventory_changed_team_count: 0,
                core_inventory_changed_team_sample: Vec::new(),
                core_inventory_by_team: BTreeMap::new(),
                last_core_sync_ok: false,
                core_parse_fail_count: 1,
            })
        );
        assert_eq!(
            state.authoritative_state_mirror,
            Some(AuthoritativeStateMirror {
                wave_time_bits: 123.5f32.to_bits(),
                wave: 7,
                enemies: 0,
                paused: false,
                game_over: false,
                net_seconds: 654_321,
                tps: 60,
                rand0: 111_111_111,
                rand1: 222_222_222,
                gameplay_state: GameplayStateProjection::Playing,
                last_wave_advanced: true,
                wave_advance_count: 1,
                apply_count: 1,
                last_net_seconds_rollback: false,
                net_seconds_delta: 654_321,
                wave_regress_count: 0,
                core_inventory_team_count: 0,
                core_inventory_item_entry_count: 0,
                core_inventory_total_amount: 0,
                core_inventory_nonzero_item_count: 0,
                core_inventory_changed_team_count: 0,
                core_inventory_changed_team_sample: Vec::new(),
                core_inventory_by_team: BTreeMap::new(),
                last_core_sync_ok: false,
                core_parse_fail_count: 1,
            })
        );
    }

    #[test]
    fn state_snapshot_authority_keeps_last_inventory_on_core_parse_error() {
        let initial_core_data =
            build_core_data_payload(&[(1, &[(0, 321), (1, 45)]), (4, &[(6, 9)])]);
        let malformed_core_data = [0x02, 0x01, 0x00, 0x01, 0x00];
        let initial_payload =
            build_state_snapshot_payload(7, 1, false, false, 100, 60, 1, 2, &initial_core_data);
        let malformed_payload =
            build_state_snapshot_payload(8, 2, false, false, 101, 60, 3, 4, &malformed_core_data);
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &malformed_payload,
            ),
        );

        let projection = state.state_snapshot_authority_projection.as_ref().unwrap();
        assert_eq!(projection.wave, 8);
        assert_eq!(projection.enemies, 2);
        assert_eq!(projection.wave_advance_count, 2);
        assert_eq!(projection.state_snapshot_apply_count, 2);
        assert_eq!(projection.core_inventory_team_count, 2);
        assert_eq!(projection.core_inventory_item_entry_count, 3);
        assert_eq!(projection.core_inventory_total_amount, 375);
        assert_eq!(projection.core_inventory_nonzero_item_count, 3);
        assert_eq!(projection.core_inventory_changed_team_count, 0);
        assert_eq!(
            projection.core_inventory_changed_team_sample,
            Vec::<u8>::new()
        );
        assert_eq!(
            projection.core_inventory_by_team,
            BTreeMap::from([
                (1u8, BTreeMap::from([(0u16, 321), (1u16, 45)])),
                (4u8, BTreeMap::from([(6u16, 9)])),
            ])
        );
        assert!(!projection.last_core_sync_ok);
        assert_eq!(projection.core_parse_fail_count, 1);
        assert_eq!(
            state.last_good_state_snapshot_core_data,
            Some(AppliedStateSnapshotCoreData {
                team_count: 2,
                teams: vec![
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 0,
                                amount: 321,
                            },
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 1,
                                amount: 45,
                            },
                        ],
                    },
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 4,
                        items: vec![AppliedStateSnapshotCoreDataItem {
                            item_id: 6,
                            amount: 9,
                        }],
                    },
                ],
            })
        );
    }

    #[test]
    fn state_snapshot_business_projection_keeps_last_authoritative_inventory_on_core_data_parse_error(
    ) {
        let initial_core_data =
            build_core_data_payload(&[(1, &[(0, 321), (1, 45)]), (4, &[(6, 9)])]);
        let malformed_core_data = [0x02, 0x01, 0x00, 0x01, 0x00];
        let initial_payload =
            build_state_snapshot_payload(7, 1, false, false, 100, 60, 1, 2, &initial_core_data);
        let malformed_payload =
            build_state_snapshot_payload(8, 2, false, false, 101, 60, 3, 4, &malformed_core_data);
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &malformed_payload,
            ),
        );

        assert_eq!(state.failed_state_snapshot_core_data_parse_count, 1);
        assert_eq!(state.last_state_snapshot_core_data, None);
        assert_eq!(
            state.last_good_state_snapshot_core_data,
            Some(AppliedStateSnapshotCoreData {
                team_count: 2,
                teams: vec![
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 0,
                                amount: 321,
                            },
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 1,
                                amount: 45,
                            },
                        ],
                    },
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 4,
                        items: vec![AppliedStateSnapshotCoreDataItem {
                            item_id: 6,
                            amount: 9,
                        }],
                    },
                ],
            })
        );
        assert_eq!(
            state.last_state_snapshot_core_data_parse_error.as_deref(),
            Some("truncated_state_snapshot_core_data")
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| (
                    projection.core_inventory_synced,
                    projection.core_inventory_team_count,
                    projection.core_inventory_item_entry_count,
                    projection.core_inventory_total_amount,
                    projection.core_inventory_nonzero_item_count,
                    projection.core_inventory_changed_team_count,
                    projection.core_inventory_changed_team_sample.clone(),
                    projection.core_inventory_by_team.clone(),
                )),
            Some((
                false,
                2,
                3,
                375,
                3,
                0,
                Vec::new(),
                BTreeMap::from([
                    (1u8, BTreeMap::from([(0u16, 321), (1u16, 45)])),
                    (4u8, BTreeMap::from([(6u16, 9)])),
                ]),
            ))
        );
    }

    #[test]
    fn state_snapshot_business_projection_reports_changed_team_sample() {
        let initial_core_data = build_core_data_payload(&[
            (1, &[(0, 10)]),
            (2, &[(0, 20)]),
            (3, &[(0, 30)]),
            (4, &[(0, 40)]),
            (5, &[(0, 50)]),
        ]);
        let updated_core_data = build_core_data_payload(&[
            (1, &[(0, 11)]),
            (3, &[(0, 30)]),
            (4, &[(0, 41)]),
            (5, &[(0, 0)]),
            (6, &[(0, 60)]),
        ]);
        let initial_payload =
            build_state_snapshot_payload(7, 1, false, false, 100, 60, 1, 2, &initial_core_data);
        let updated_payload =
            build_state_snapshot_payload(8, 2, false, false, 101, 60, 3, 4, &updated_core_data);
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &updated_payload,
            ),
        );

        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| (
                    projection.core_inventory_synced,
                    projection.core_inventory_changed_team_count,
                    projection.core_inventory_changed_team_sample.clone(),
                    projection.core_inventory_by_team.get(&6).cloned(),
                )),
            Some((
                true,
                5,
                vec![1, 2, 4, 5],
                Some(BTreeMap::from([(0u16, 60)])),
            ))
        );
    }

    #[test]
    fn state_snapshot_authority_wave_advance_count_only_on_increase() {
        let mut state = SessionState::default();
        let initial_payload = build_state_snapshot_payload(7, 1, false, false, 10, 60, 1, 2, &[]);
        let same_wave_payload = build_state_snapshot_payload(7, 2, true, false, 11, 61, 3, 4, &[]);
        let next_wave_payload = build_state_snapshot_payload(8, 3, false, true, 12, 62, 5, 6, &[]);
        let rollback_payload = build_state_snapshot_payload(6, 4, false, false, 5, 55, 7, 8, &[]);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &initial_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.wave_advance_count,
                        projection.state_snapshot_apply_count,
                    )
                }),
            Some((7, GameplayStateProjection::Playing, true, 1, 1))
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &same_wave_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.enemies,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.wave_advance_count,
                        projection.net_seconds_delta,
                    )
                }),
            Some((7, 2, GameplayStateProjection::Paused, false, 1, 1))
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &next_wave_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.wave_advance_count,
                    )
                }),
            Some((8, GameplayStateProjection::GameOver, true, 2))
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &rollback_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.wave_advance_count,
                        projection.last_net_seconds_rollback,
                        projection.net_seconds_delta,
                        projection.state_snapshot_wave_regress_count,
                    )
                }),
            Some((6, GameplayStateProjection::Playing, false, 2, true, -7, 1))
        );
    }

    #[test]
    fn state_snapshot_authority_gameplay_state_precedence_gameover_over_paused() {
        let payload = build_state_snapshot_payload(7, 2, true, true, 10, 60, 1, 2, &[]);
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 125, &payload),
        );

        assert_eq!(
            state
                .state_snapshot_authority_projection
                .as_ref()
                .map(|projection| projection.gameplay_state),
            Some(GameplayStateProjection::GameOver)
        );
    }

    #[test]
    fn state_snapshot_business_projection_tracks_wave_advances_only_on_increase() {
        let mut state = SessionState::default();
        let initial_payload = build_state_snapshot_payload(7, 1, false, false, 10, 60, 1, 2, &[]);
        let same_wave_payload = build_state_snapshot_payload(7, 2, true, false, 11, 61, 3, 4, &[]);
        let next_wave_payload = build_state_snapshot_payload(8, 3, false, true, 12, 62, 5, 6, &[]);
        let rollback_payload = build_state_snapshot_payload(6, 4, false, false, 5, 55, 7, 8, &[]);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &initial_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.last_wave_advance_from,
                        projection.last_wave_advance_to,
                        projection.wave_advance_count,
                        projection.net_seconds_applied_count,
                        projection.state_snapshot_apply_count,
                    )
                }),
            Some((
                7,
                GameplayStateProjection::Playing,
                true,
                Some(0),
                Some(7),
                1,
                1,
                1,
            ))
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &same_wave_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.enemies,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.wave_advance_count,
                        projection.gameplay_state_transition_count,
                        projection.net_seconds_delta,
                    )
                }),
            Some((7, 2, GameplayStateProjection::Paused, false, 1, 1, 1,))
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &next_wave_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.last_wave_advance_from,
                        projection.last_wave_advance_to,
                        projection.wave_advance_count,
                        projection.gameplay_state_transition_count,
                    )
                }),
            Some((
                8,
                GameplayStateProjection::GameOver,
                true,
                Some(7),
                Some(8),
                2,
                2,
            ))
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                125,
                &rollback_payload,
            ),
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| {
                    (
                        projection.wave,
                        projection.gameplay_state,
                        projection.last_wave_advanced,
                        projection.wave_advance_count,
                        projection.last_net_seconds_rollback,
                        projection.net_seconds_delta,
                        projection.state_snapshot_time_regress_count,
                        projection.state_snapshot_wave_regress_count,
                    )
                }),
            Some((
                6,
                GameplayStateProjection::Playing,
                false,
                2,
                true,
                -7,
                1,
                1,
            ))
        );
    }

    #[test]
    fn block_snapshot_ingest_parses_envelope_fields() {
        let payload = [
            0x00, 0x01, // amount
            0x00, 0x11, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2d, // first block id = 301
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5
            0x03, // io version = 3
            0x01, // enabled = true
            0x08, // module bitmask
            0x80, // efficiency
            0x40, // optional efficiency
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.applied_block_snapshot_count, 1);
        assert_eq!(
            state.last_block_snapshot,
            Some(AppliedBlockSnapshotEnvelope {
                amount: 1,
                data_len: 17,
                first_build_pos: Some(0x0064_0063),
                first_block_id: Some(301),
                first_health_bits: Some(0x3f800000),
                first_rotation: Some(2),
                first_team_id: Some(5),
                first_io_version: Some(3),
                first_enabled: Some(true),
                first_module_bitmask: Some(8),
                first_time_scale_bits: None,
                first_time_scale_duration_bits: None,
                first_last_disabler_pos: None,
                first_legacy_consume_connected: None,
                first_efficiency: Some(0x80),
                first_optional_efficiency: Some(0x40),
                first_visible_flags: None,
            })
        );
        assert_eq!(state.failed_block_snapshot_parse_count, 0);
        assert_eq!(state.last_block_snapshot_parse_error, None);
        assert_eq!(state.building_table_projection.by_build_pos.len(), 1);
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.block_id),
            Some(301)
        );
        assert_eq!(
            state.building_table_projection.last_update,
            Some(BuildingProjectionUpdateKind::BlockSnapshotHead)
        );
        assert_eq!(
            state
                .building_table_projection
                .block_snapshot_head_apply_count,
            1
        );
    }

    #[test]
    fn block_snapshot_ingest_rejects_negative_amount() {
        let payload = [0xFF, 0xFF];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.applied_block_snapshot_count, 0);
        assert_eq!(state.failed_block_snapshot_parse_count, 1);
        assert_eq!(
            state.last_block_snapshot_parse_error.as_deref(),
            Some("negative_block_snapshot_amount:-1")
        );
        assert_eq!(
            state.last_block_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
        assert_eq!(state.last_block_snapshot, None);
        assert_eq!(state.block_snapshot_head_projection, None);
    }

    #[test]
    fn malformed_block_snapshot_with_trailing_bytes_rejects_entire_payload() {
        let mut payload = vec![
            0x00, 0x01, // amount
            0x00, 0x11, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2d, // first block id = 301
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5
            0x03, // io version = 3
            0x01, // enabled = true
            0x08, // module bitmask
            0x80, // efficiency
            0x40, // optional efficiency
        ];
        payload.push(0xFF);
        let expected_error = format!(
            "block_snapshot_trailing_bytes:{}/{}",
            payload.len().saturating_sub(1),
            payload.len()
        );
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.applied_block_snapshot_count, 0);
        assert_eq!(state.failed_block_snapshot_parse_count, 1);
        assert_eq!(
            state.last_block_snapshot_parse_error.as_deref(),
            Some(expected_error.as_str())
        );
        assert_eq!(
            state.last_block_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
        assert_eq!(state.last_block_snapshot, None);
        assert_eq!(state.block_snapshot_head_projection, None);
    }

    #[test]
    fn malformed_block_snapshot_tracks_first_entry_header_error() {
        let payload = [0x00, 0x01, 0x00, 0x03, 0xAA, 0xBB, 0xCC];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.applied_block_snapshot_count, 0);
        assert_eq!(state.failed_block_snapshot_parse_count, 1);
        assert_eq!(
            state.last_block_snapshot_parse_error.as_deref(),
            Some("truncated_block_snapshot_first_entry_header:3")
        );
        assert_eq!(
            state.last_block_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
    }

    #[test]
    fn malformed_block_snapshot_clears_prior_head_projection_but_keeps_last_snapshot() {
        let valid_payload = [
            0x00, 0x01, // amount
            0x00, 0x11, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2d, // first block id = 301
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5
            0x03, // io version = 3
            0x01, // enabled = true
            0x08, // module bitmask
            0x80, // efficiency
            0x40, // optional efficiency
        ];
        let malformed_payload = [0x00, 0x01, 0x00, 0x03, 0xAA, 0xBB, 0xCC];
        let build_pos = 0x0064_0063;
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &valid_payload),
        );
        let last_snapshot = state.last_block_snapshot.clone();
        let applied_building = state
            .building_table_projection
            .by_build_pos
            .get(&build_pos)
            .cloned();
        assert!(state.block_snapshot_head_projection.is_some());
        assert!(applied_building.is_some());

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::BlockSnapshot,
                11,
                &malformed_payload,
            ),
        );

        assert_eq!(state.block_snapshot_head_projection, None);
        assert_eq!(state.last_block_snapshot, last_snapshot);
        assert_eq!(
            state.building_table_projection.by_build_pos.get(&build_pos),
            applied_building.as_ref()
        );
        assert_eq!(
            state.building_table_projection.block_snapshot_head_apply_count,
            1
        );
        assert_eq!(
            state.building_table_projection.last_update,
            Some(BuildingProjectionUpdateKind::BlockSnapshotHead)
        );
        assert_eq!(state.failed_block_snapshot_parse_count, 1);
        assert_eq!(
            state.last_block_snapshot_parse_error.as_deref(),
            Some("truncated_block_snapshot_first_entry_header:3")
        );
        assert_eq!(
            state.last_block_snapshot_parse_error_payload_len,
            Some(malformed_payload.len())
        );
    }

    #[test]
    fn block_snapshot_ingest_applies_and_clears_head_projection() {
        let payload = [
            0x00, 0x01, // amount
            0x00, 0x11, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2d, // first block id = 301
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5
            0x03, // io version = 3
            0x01, // enabled = true
            0x08, // module bitmask
            0x80, // efficiency
            0x40, // optional efficiency
        ];
        let clear_payload = [
            0x00, 0x00, // amount
            0x00, 0x00, // data len
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );
        assert_eq!(
            state.block_snapshot_head_projection,
            Some(BlockSnapshotHeadProjection {
                build_pos: 0x0064_0063,
                block_id: 301,
                health_bits: Some(0x3f800000),
                rotation: Some(2),
                team_id: Some(5),
                io_version: Some(3),
                enabled: Some(true),
                module_bitmask: Some(8),
                time_scale_bits: None,
                time_scale_duration_bits: None,
                last_disabler_pos: None,
                legacy_consume_connected: None,
                efficiency: Some(0x80),
                optional_efficiency: Some(0x40),
                visible_flags: None,
            })
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &clear_payload),
        );
        assert_eq!(state.block_snapshot_head_projection, None);
        assert_eq!(state.building_table_projection.by_build_pos.len(), 1);
        assert_eq!(
            state
                .building_table_projection
                .block_snapshot_head_apply_count,
            1
        );
    }

    #[test]
    fn block_snapshot_ingest_keeps_conflicting_existing_building_projection_untouched() {
        let payload = [
            0x00, 0x01, // amount
            0x00, 0x11, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2e, // first block id = 302 (conflicts with tracked 301)
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5
            0x03, // io version = 3
            0x01, // enabled = true
            0x08, // module bitmask
            0x80, // efficiency
            0x40, // optional efficiency
        ];
        let build_pos = 0x0064_0063;
        let mut state = SessionState::default();
        state.building_table_projection.apply_construct_finish(
            build_pos,
            Some(301),
            Some("message".to_string()),
            1,
            2,
            mdt_typeio::TypeIoObject::Int(7),
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.applied_block_snapshot_count, 1);
        assert_eq!(
            state
                .block_snapshot_head_projection
                .map(|head| head.block_id),
            Some(302)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&build_pos)
                .and_then(|building| building.block_id),
            Some(301)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&build_pos)
                .and_then(|building| building.team_id),
            Some(2)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&build_pos)
                .and_then(|building| building.config.clone()),
            Some(mdt_typeio::TypeIoObject::Int(7))
        );
        assert_eq!(
            state
                .building_table_projection
                .block_snapshot_head_apply_count,
            0
        );
        assert_eq!(
            state
                .building_table_projection
                .block_snapshot_head_conflict_skip_count,
            1
        );
        assert!(
            state
                .building_table_projection
                .last_block_snapshot_head_conflict
        );
    }

    #[test]
    fn malformed_block_snapshot_tracks_first_entry_prefix_error() {
        let payload = [
            0x00, 0x01, // amount
            0x00, 0x0c, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2d, // first block id = 301
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5, but missing version/enabled/module mask
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.applied_block_snapshot_count, 0);
        assert_eq!(state.failed_block_snapshot_parse_count, 1);
        assert_eq!(
            state.last_block_snapshot_parse_error.as_deref(),
            Some("truncated_block_snapshot_first_entry_prefix:12")
        );
        assert_eq!(
            state.last_block_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
    }

    #[test]
    fn malformed_block_snapshot_tracks_first_entry_base_error() {
        let payload = [
            0x00, 0x01, // amount
            0x00, 0x0f, // data len
            0x00, 0x64, 0x00, 0x63, // first build pos = pack(100, 99)
            0x01, 0x2d, // first block id = 301
            0x3f, 0x80, 0x00, 0x00, // health = 1.0
            0x82, // rotation = 2 with version marker bit
            0x05, // team = 5
            0x03, // io version = 3
            0x01, // enabled = true
            0x09, // module bitmask advertises items, but no item module body follows
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 11, &payload),
        );

        assert_eq!(state.received_block_snapshot_count, 1);
        assert_eq!(state.applied_block_snapshot_count, 0);
        assert_eq!(state.failed_block_snapshot_parse_count, 1);
        assert_eq!(
            state.last_block_snapshot_parse_error.as_deref(),
            Some("invalid_block_snapshot_first_entry_base:failed to fill whole buffer")
        );
        assert_eq!(
            state.last_block_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
    }

    #[test]
    fn hidden_snapshot_ingest_parses_count_and_first_id() {
        let payload = [
            0x00, 0x00, 0x00, 0x03, // count
            0x00, 0x00, 0x00, 0x64, // 100
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x00, 0xCA, // 202
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(state.received_hidden_snapshot_count, 1);
        assert_eq!(state.applied_hidden_snapshot_count, 1);
        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 3,
                first_id: Some(100),
                sample_ids: vec![100, 101, 202],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 3,
                added_count: 3,
                removed_count: 0,
                added_sample_ids: vec![100, 101, 202],
                removed_sample_ids: vec![],
            })
        );
        assert_eq!(
            state
                .hidden_snapshot_ids
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![100, 101, 202]
        );
        assert_eq!(state.failed_hidden_snapshot_parse_count, 0);
        assert_eq!(state.last_hidden_snapshot_parse_error, None);
    }

    #[test]
    fn hidden_snapshot_ingest_tracks_latest_hidden_id_set_and_real_delta() {
        let initial_payload = [
            0x00, 0x00, 0x00, 0x03, // count
            0x00, 0x00, 0x00, 0x64, // 100
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x00, 0xCA, // 202
        ];
        let next_payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
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

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &next_payload),
        );

        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 2,
                first_id: Some(101),
                sample_ids: vec![101, 303],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 2,
                added_count: 1,
                removed_count: 2,
                added_sample_ids: vec![303],
                removed_sample_ids: vec![100, 202],
            })
        );
        assert_eq!(
            state
                .hidden_snapshot_ids
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![101, 303]
        );
    }

    #[test]
    fn hidden_snapshot_ingest_limits_sample_ids_but_tracks_full_unique_set() {
        let payload = [
            0x00, 0x00, 0x00, 0x05, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x00, 0xCA, // 202
            0x00, 0x00, 0x01, 0x2F, // 303
            0x00, 0x00, 0x01, 0x94, // 404
            0x00, 0x00, 0x01, 0xF9, // 505
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 5,
                first_id: Some(101),
                sample_ids: vec![101, 202, 303, 404],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 5,
                added_count: 5,
                removed_count: 0,
                added_sample_ids: vec![101, 202, 303, 404],
                removed_sample_ids: vec![],
            })
        );
        assert_eq!(
            state
                .hidden_snapshot_ids
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![101, 202, 303, 404, 505]
        );
    }

    #[test]
    fn hidden_snapshot_ingest_preserves_duplicate_samples_but_deduplicates_active_set() {
        let payload = [
            0x00, 0x00, 0x00, 0x04, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x00, 0xCA, // 202
            0x00, 0x00, 0x00, 0x65, // 101
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 4,
                first_id: Some(101),
                sample_ids: vec![101, 101, 202, 101],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 2,
                added_count: 2,
                removed_count: 0,
                added_sample_ids: vec![101, 202],
                removed_sample_ids: vec![],
            })
        );
        assert_eq!(
            state
                .hidden_snapshot_ids
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![101, 202]
        );
    }

    #[test]
    fn hidden_snapshot_removes_non_local_unit_rows_but_keeps_local_player() {
        let payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
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
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 4,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 3,
                    health_bits: 4.0f32.to_bits(),
                    rotation_bits: 5.0f32.to_bits(),
                    shield_bits: 6.0f32.to_bits(),
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(state.entity_table_projection.hidden_apply_count, 1);
        assert_eq!(state.entity_table_projection.hidden_count, 1);
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);
        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&303));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert_eq!(state.hidden_lifecycle_remove_count, 1);
        assert_eq!(state.last_hidden_lifecycle_removed_ids_sample, vec![303]);
    }

    #[test]
    fn hidden_snapshot_clears_prior_local_hidden_flag_when_id_leaves_hidden_set() {
        let initial_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x00, 0x65, // 101
        ];
        let next_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &initial_payload,
            ),
        );
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &next_payload),
        );

        assert!(!state.entity_table_projection.by_entity_id[&101].hidden);
        assert_eq!(state.entity_table_projection.hidden_apply_count, 2);
        assert_eq!(state.entity_table_projection.hidden_count, 0);
        assert_eq!(
            state
                .hidden_snapshot_ids
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![303]
        );
    }

    #[test]
    fn hidden_snapshot_empty_set_clears_active_ids_and_records_empty_applied_snapshot() {
        let initial_payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let empty_payload = [0x00, 0x00, 0x00, 0x00];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &initial_payload,
            ),
        );
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &empty_payload,
            ),
        );

        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 0,
                first_id: None,
                sample_ids: vec![],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 0,
                added_count: 0,
                removed_count: 2,
                added_sample_ids: vec![],
                removed_sample_ids: vec![101, 303],
            })
        );
        assert!(state.hidden_snapshot_ids.is_empty());
        assert!(!state.entity_table_projection.by_entity_id[&101].hidden);
    }

    #[test]
    fn hidden_snapshot_clears_prior_runtime_kept_hidden_flag_when_id_leaves_hidden_set() {
        let initial_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let next_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x01, 0x94, // 404
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: 35,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 35,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 3.0f32.to_bits(),
                y_bits: 4.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 35,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::WorldLabel(
                    EntityWorldLabelSemanticProjection {
                        flags: 1,
                        font_size_bits: 12.0f32.to_bits(),
                        text: Some("first".to_string()),
                        z_bits: 0.5f32.to_bits(),
                    },
                ),
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            EntitySemanticProjectionEntry {
                class_id: 35,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::WorldLabel(
                    EntityWorldLabelSemanticProjection {
                        flags: 2,
                        font_size_bits: 13.0f32.to_bits(),
                        text: Some("second".to_string()),
                        z_bits: 0.75f32.to_bits(),
                    },
                ),
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &initial_payload,
            ),
        );
        assert!(state.entity_table_projection.by_entity_id[&303].hidden);
        assert!(!state.entity_table_projection.by_entity_id[&404].hidden);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &next_payload),
        );

        assert!(!state.entity_table_projection.by_entity_id[&303].hidden);
        assert!(state.entity_table_projection.by_entity_id[&404].hidden);
        assert!(state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert!(state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&404));
        assert_eq!(state.entity_table_projection.hidden_apply_count, 2);
        assert_eq!(state.entity_table_projection.hidden_count, 1);
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 1,
                added_count: 1,
                removed_count: 1,
                added_sample_ids: vec![404],
                removed_sample_ids: vec![303],
            })
        );
    }

    #[test]
    fn hidden_snapshot_removes_known_runtime_owned_non_unit_rows() {
        let payload = [
            0x00, 0x00, 0x00, 0x03, // count
            0x00, 0x00, 0x01, 0x2F, // 303 fire
            0x00, 0x00, 0x01, 0x94, // 404 puddle
            0x00, 0x00, 0x01, 0xF9, // 505 weather
        ];
        let mut state = SessionState::default();

        for (entity_id, class_id, x_bits, y_bits) in [
            (303, 10, 1.0f32.to_bits(), 2.0f32.to_bits()),
            (404, 13, 3.0f32.to_bits(), 4.0f32.to_bits()),
            (505, 14, 5.0f32.to_bits(), 6.0f32.to_bits()),
        ] {
            state.entity_table_projection.by_entity_id.insert(
                entity_id,
                EntityProjection {
                    class_id,
                    hidden: false,
                    is_local_player: false,
                    unit_kind: 0,
                    unit_value: 0,
                    x_bits,
                    y_bits,
                    last_seen_entity_snapshot_count: 1,
                },
            );
        }
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 10,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Fire(EntityFireSemanticProjection {
                    tile_pos: 77,
                    lifetime_bits: 8.0f32.to_bits(),
                    time_bits: 9.0f32.to_bits(),
                }),
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            404,
            EntitySemanticProjectionEntry {
                class_id: 13,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Puddle(EntityPuddleSemanticProjection {
                    tile_pos: 88,
                    liquid_id: 4,
                    amount_bits: 1.5f32.to_bits(),
                }),
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            505,
            EntitySemanticProjectionEntry {
                class_id: 14,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::WeatherState(
                    EntityWeatherStateSemanticProjection {
                        weather_id: 6,
                        intensity_bits: 0.5f32.to_bits(),
                        life_bits: 2.5f32.to_bits(),
                        opacity_bits: 0.75f32.to_bits(),
                        wind_x_bits: 1.25f32.to_bits(),
                        wind_y_bits: 1.75f32.to_bits(),
                    },
                ),
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&303));
        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&404));
        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&505));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&404));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&505));
        assert_eq!(state.hidden_lifecycle_remove_count, 3);
        assert_eq!(
            state.last_hidden_lifecycle_removed_ids_sample,
            vec![303, 404, 505]
        );
        let runtime_projection = state.runtime_typed_entity_projection();
        assert!(!runtime_projection.by_entity_id.contains_key(&303));
        assert!(!runtime_projection.by_entity_id.contains_key(&404));
        assert!(!runtime_projection.by_entity_id.contains_key(&505));
    }

    #[test]
    fn hidden_snapshot_removes_building_tether_payload_rows() {
        let payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x02, 0x7A, // 634
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            634,
            EntityProjection {
                class_id: 36,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 7.0f32.to_bits(),
                y_bits: 8.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            634,
            EntitySemanticProjectionEntry {
                class_id: 36,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 3,
                    unit_type_id: 4,
                    health_bits: 5.0f32.to_bits(),
                    rotation_bits: 6.0f32.to_bits(),
                    shield_bits: 7.0f32.to_bits(),
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&634));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&634));
        assert_eq!(state.hidden_lifecycle_remove_count, 1);
        assert_eq!(state.last_hidden_lifecycle_removed_ids_sample, vec![634]);
        assert!(!state
            .runtime_typed_entity_projection()
            .by_entity_id
            .contains_key(&634));
    }

    #[test]
    fn hidden_snapshot_keeps_non_unit_rows_when_handle_sync_hidden_is_not_known_remove() {
        let payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: 35,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 35,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::WorldLabel(
                    EntityWorldLabelSemanticProjection {
                        flags: 1,
                        font_size_bits: 12.0f32.to_bits(),
                        text: Some("hidden".to_string()),
                        z_bits: 0.5f32.to_bits(),
                    },
                ),
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(state.entity_table_projection.hidden_apply_count, 1);
        assert_eq!(state.entity_table_projection.hidden_count, 1);
        assert!(state.entity_table_projection.by_entity_id[&303].hidden);
        assert!(state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert_eq!(state.hidden_lifecycle_remove_count, 0);
        assert!(state.last_hidden_lifecycle_removed_ids_sample.is_empty());
        assert!(matches!(
            state.runtime_typed_entity_projection().entity_at(303),
            Some(TypedRuntimeEntityModel::WorldLabel(world_label))
                if world_label.base.hidden
                    && world_label.semantic.text.as_deref() == Some("hidden")
        ));
    }

    #[test]
    fn hidden_snapshot_cleans_non_local_orphan_semantic_resource_and_payload_rows() {
        let payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101 local
            0x00, 0x00, 0x01, 0x2F, // 303 hidden non-local
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            101,
            EntitySemanticProjectionEntry {
                class_id: 35,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::WorldLabel(
                    EntityWorldLabelSemanticProjection {
                        flags: 0,
                        font_size_bits: 10.0f32.to_bits(),
                        text: Some("local".to_string()),
                        z_bits: 1.0f32.to_bits(),
                    },
                ),
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 4,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 3,
                    health_bits: 4.0f32.to_bits(),
                    rotation_bits: 5.0f32.to_bits(),
                    shield_bits: 6.0f32.to_bits(),
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: Some(1),
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            },
        );
        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                101,
                ResourceUnitItemStack {
                    item_id: Some(1),
                    amount: 5,
                },
            );
        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                303,
                ResourceUnitItemStack {
                    item_id: Some(2),
                    amount: 7,
                },
            );
        state.payload_lifecycle_projection.by_carrier.insert(
            UnitRefProjection {
                kind: 2,
                value: 303,
            },
            PayloadLifecycleCarrierProjection {
                carrier: UnitRefProjection {
                    kind: 2,
                    value: 303,
                },
                target_unit: Some(UnitRefProjection {
                    kind: 2,
                    value: 404,
                }),
                target_build: None,
                drop_tile: None,
                on_ground: Some(false),
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            },
        );
        state.payload_lifecycle_projection.by_carrier.insert(
            UnitRefProjection {
                kind: 2,
                value: 505,
            },
            PayloadLifecycleCarrierProjection {
                carrier: UnitRefProjection {
                    kind: 2,
                    value: 505,
                },
                target_unit: Some(UnitRefProjection {
                    kind: 2,
                    value: 303,
                }),
                target_build: None,
                drop_tile: None,
                on_ground: Some(false),
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            },
        );
        state.payload_lifecycle_projection.by_carrier.insert(
            UnitRefProjection {
                kind: 2,
                value: 101,
            },
            PayloadLifecycleCarrierProjection {
                carrier: UnitRefProjection {
                    kind: 2,
                    value: 101,
                },
                target_unit: Some(UnitRefProjection {
                    kind: 2,
                    value: 101,
                }),
                target_build: None,
                drop_tile: None,
                on_ground: Some(false),
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            },
        );
        state.last_unit_control_target = Some(UnitRefProjection {
            kind: 2,
            value: 303,
        });
        state.last_unit_building_control_select_target = Some(UnitRefProjection {
            kind: 2,
            value: 101,
        });
        state.last_command_units_unit_target = Some(UnitRefProjection {
            kind: 2,
            value: 303,
        });
        state.last_request_unit_payload_target = Some(UnitRefProjection {
            kind: 2,
            value: 303,
        });

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert!(state.entity_table_projection.by_entity_id[&101].hidden);
        assert!(state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&101));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert!(state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .contains_key(&101));
        assert!(!state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .contains_key(&303));
        assert!(!state
            .payload_lifecycle_projection
            .by_carrier
            .contains_key(&UnitRefProjection {
                kind: 2,
                value: 303
            }));
        assert!(state
            .payload_lifecycle_projection
            .by_carrier
            .contains_key(&UnitRefProjection {
                kind: 2,
                value: 101
            }));
        assert_eq!(state.last_unit_control_target, None);
        assert_eq!(
            state.last_unit_building_control_select_target,
            Some(UnitRefProjection {
                kind: 2,
                value: 101,
            })
        );
        assert_eq!(state.last_command_units_unit_target, None);
        assert_eq!(state.last_request_unit_payload_target, None);
        assert!(
            state.payload_lifecycle_projection.by_carrier[&UnitRefProjection {
                kind: 2,
                value: 505
            }]
                .removed_target_unit
        );
        assert!(
            !state.payload_lifecycle_projection.by_carrier[&UnitRefProjection {
                kind: 2,
                value: 101
            }]
                .removed_target_unit
        );
        assert_eq!(state.hidden_lifecycle_remove_count, 0);
        assert!(state.last_hidden_lifecycle_removed_ids_sample.is_empty());
    }

    #[test]
    fn hidden_snapshot_cleans_orphan_resource_and_payload_rows_without_entity_lifecycle_remove() {
        let payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101 local
            0x00, 0x00, 0x01, 0x2F, // 303 hidden non-local
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                101,
                ResourceUnitItemStack {
                    item_id: Some(1),
                    amount: 5,
                },
            );
        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                303,
                ResourceUnitItemStack {
                    item_id: Some(2),
                    amount: 7,
                },
            );
        state.payload_lifecycle_projection.by_carrier.insert(
            UnitRefProjection {
                kind: 2,
                value: 303,
            },
            PayloadLifecycleCarrierProjection {
                carrier: UnitRefProjection {
                    kind: 2,
                    value: 303,
                },
                target_unit: Some(UnitRefProjection {
                    kind: 2,
                    value: 404,
                }),
                target_build: None,
                drop_tile: None,
                on_ground: Some(false),
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            },
        );
        state.payload_lifecycle_projection.by_carrier.insert(
            UnitRefProjection {
                kind: 2,
                value: 505,
            },
            PayloadLifecycleCarrierProjection {
                carrier: UnitRefProjection {
                    kind: 2,
                    value: 505,
                },
                target_unit: Some(UnitRefProjection {
                    kind: 2,
                    value: 303,
                }),
                target_build: None,
                drop_tile: None,
                on_ground: Some(false),
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            },
        );
        state.payload_lifecycle_projection.by_carrier.insert(
            UnitRefProjection {
                kind: 2,
                value: 101,
            },
            PayloadLifecycleCarrierProjection {
                carrier: UnitRefProjection {
                    kind: 2,
                    value: 101,
                },
                target_unit: Some(UnitRefProjection {
                    kind: 2,
                    value: 101,
                }),
                target_build: None,
                drop_tile: None,
                on_ground: Some(false),
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert!(state.entity_table_projection.by_entity_id[&101].hidden);
        assert!(state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .contains_key(&101));
        assert!(!state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .contains_key(&303));
        assert!(!state
            .payload_lifecycle_projection
            .by_carrier
            .contains_key(&UnitRefProjection {
                kind: 2,
                value: 303
            }));
        assert!(state
            .payload_lifecycle_projection
            .by_carrier
            .contains_key(&UnitRefProjection {
                kind: 2,
                value: 101
            }));
        assert!(
            state.payload_lifecycle_projection.by_carrier[&UnitRefProjection {
                kind: 2,
                value: 505
            }]
                .removed_target_unit
        );
        assert_eq!(state.hidden_lifecycle_remove_count, 0);
        assert!(state.last_hidden_lifecycle_removed_ids_sample.is_empty());
    }

    #[test]
    fn hidden_snapshot_ingest_does_not_reseed_unrelated_runtime_typed_rows() {
        let payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x00, 0xCA, // 202 hidden non-local unit
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        for entity_id in [202, 303] {
            state.entity_table_projection.by_entity_id.insert(
                entity_id,
                EntityProjection {
                    class_id: 4,
                    hidden: false,
                    is_local_player: false,
                    unit_kind: 2,
                    unit_value: entity_id as u32,
                    x_bits: (entity_id as f32).to_bits(),
                    y_bits: (entity_id as f32 + 1.0).to_bits(),
                    last_seen_entity_snapshot_count: entity_id as u64,
                },
            );
            state.entity_semantic_projection.upsert(
                entity_id,
                4,
                entity_id as u64,
                EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 55,
                    health_bits: 0x3f80_0000,
                    rotation_bits: 0x4000_0000,
                    shield_bits: 0x4040_0000,
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            );
        }
        state.refresh_runtime_typed_entity_from_tables(101);
        state.refresh_runtime_typed_entity_from_tables(202);

        assert!(state
            .runtime_typed_entity_projection()
            .by_entity_id
            .contains_key(&202));
        assert!(!state
            .runtime_typed_entity_projection()
            .by_entity_id
            .contains_key(&303));

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        let projection = state.runtime_typed_entity_projection();
        assert!(projection.by_entity_id.contains_key(&101));
        assert!(!projection.by_entity_id.contains_key(&202));
        assert!(!projection.by_entity_id.contains_key(&303));
        assert!(state
            .entity_table_projection
            .by_entity_id
            .contains_key(&303));
        assert!(state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert_eq!(projection.player_count, 1);
        assert_eq!(projection.unit_count, 0);
        assert_eq!(projection.hidden_count, 0);
    }

    #[test]
    fn malformed_hidden_snapshot_tracks_parse_error() {
        let payload = [0xFF, 0xFF, 0xFF, 0xFF];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(state.received_hidden_snapshot_count, 1);
        assert_eq!(state.applied_hidden_snapshot_count, 0);
        assert_eq!(state.failed_hidden_snapshot_parse_count, 1);
        assert_eq!(
            state.last_hidden_snapshot_parse_error.as_deref(),
            Some("negative_hidden_snapshot_count:-1")
        );
        assert_eq!(
            state.last_hidden_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
    }

    #[test]
    fn successful_hidden_snapshot_after_parse_failure_clears_parse_error_fields() {
        let malformed_payload = [0xFF, 0xFF, 0xFF, 0xFF];
        let valid_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x00, 0x65, // 101
        ];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &malformed_payload,
            ),
        );
        assert_eq!(state.failed_hidden_snapshot_parse_count, 1);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &valid_payload,
            ),
        );

        assert_eq!(state.received_hidden_snapshot_count, 2);
        assert_eq!(state.applied_hidden_snapshot_count, 1);
        assert_eq!(state.failed_hidden_snapshot_parse_count, 1);
        assert_eq!(state.last_hidden_snapshot_parse_error, None);
        assert_eq!(state.last_hidden_snapshot_parse_error_payload_len, None);
        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(101),
                sample_ids: vec![101],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 1,
                added_count: 1,
                removed_count: 0,
                added_sample_ids: vec![101],
                removed_sample_ids: vec![],
            })
        );
        assert_eq!(
            state
                .hidden_snapshot_ids
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![101]
        );
    }

    #[test]
    fn hidden_snapshot_parse_failure_preserves_hidden_state_until_valid_refresh() {
        let malformed_payload = [0xFF, 0xFF, 0xFF, 0xFF];
        let valid_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let mut state = SessionState::default();
        let seed_hidden_unit = |state: &mut SessionState| {
            state.entity_table_projection.by_entity_id.insert(
                303,
                EntityProjection {
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
            state.entity_semantic_projection.upsert(
                303,
                33,
                1,
                EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 3,
                    health_bits: 0,
                    rotation_bits: 0,
                    shield_bits: 0,
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            );
        };

        seed_hidden_unit(&mut state);
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &valid_payload),
        );

        assert_eq!(state.hidden_snapshot_ids, BTreeSet::from([303]));
        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(303),
                sample_ids: vec![303],
            })
        );
        assert_eq!(state.last_hidden_lifecycle_removed_ids_sample, vec![303]);
        let expected_hidden_snapshot = state.last_hidden_snapshot.clone();
        let expected_hidden_snapshot_ids = state.hidden_snapshot_ids.clone();
        let expected_hidden_snapshot_delta_projection =
            state.hidden_snapshot_delta_projection.clone();
        let expected_hidden_lifecycle_removed_ids_sample =
            state.last_hidden_lifecycle_removed_ids_sample.clone();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &malformed_payload,
            ),
        );

        assert_eq!(
            state.last_hidden_snapshot,
            expected_hidden_snapshot
        );
        assert_eq!(state.hidden_snapshot_ids, expected_hidden_snapshot_ids);
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            expected_hidden_snapshot_delta_projection
        );
        assert_eq!(
            state.last_hidden_lifecycle_removed_ids_sample,
            expected_hidden_lifecycle_removed_ids_sample
        );
        assert_eq!(
            state.last_hidden_snapshot_parse_error.as_deref(),
            Some("negative_hidden_snapshot_count:-1")
        );

        seed_hidden_unit(&mut state);
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &valid_payload),
        );

        assert_eq!(state.last_hidden_snapshot_parse_error, None);
        assert_eq!(state.last_hidden_snapshot_parse_error_payload_len, None);
        assert_eq!(state.hidden_snapshot_ids, BTreeSet::from([303]));
        assert_eq!(
            state.last_hidden_snapshot,
            Some(AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(303),
                sample_ids: vec![303],
            })
        );
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            Some(HiddenSnapshotDeltaProjection {
                active_count: 1,
                added_count: 0,
                removed_count: 0,
                added_sample_ids: Vec::new(),
                removed_sample_ids: Vec::new(),
            })
        );
        assert_eq!(state.last_hidden_lifecycle_removed_ids_sample, vec![303]);
    }

    #[test]
    fn hidden_snapshot_parse_error_leaves_hidden_state_unchanged() {
        let initial_payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let malformed_payload = [0xFF, 0xFF, 0xFF, 0xFF];
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
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
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 4,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 3,
                    health_bits: 4.0f32.to_bits(),
                    rotation_bits: 5.0f32.to_bits(),
                    shield_bits: 6.0f32.to_bits(),
                    mine_tile_pos: 0,
                    status_count: 0,
                    payload_count: None,
                    building_pos: None,
                    lifetime_bits: None,
                    time_bits: None,
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            },
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &initial_payload,
            ),
        );
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);
        assert_eq!(state.last_hidden_lifecycle_removed_ids_sample, vec![303]);
        let expected_hidden_snapshot = state.last_hidden_snapshot.clone();
        let expected_hidden_snapshot_ids = state.hidden_snapshot_ids.clone();
        let expected_hidden_snapshot_delta_projection =
            state.hidden_snapshot_delta_projection.clone();
        let expected_entity_hidden = state.entity_table_projection.by_entity_id[&101].hidden;
        let expected_hidden_apply_count = state.entity_table_projection.hidden_apply_count;
        let expected_hidden_count = state.entity_table_projection.hidden_count;
        let expected_hidden_lifecycle_remove_count = state.hidden_lifecycle_remove_count;
        let expected_hidden_lifecycle_removed_ids_sample =
            state.last_hidden_lifecycle_removed_ids_sample.clone();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &malformed_payload,
            ),
        );

        assert_eq!(state.received_hidden_snapshot_count, 2);
        assert_eq!(state.applied_hidden_snapshot_count, 1);
        assert_eq!(state.failed_hidden_snapshot_parse_count, 1);
        assert_eq!(
            state.last_hidden_snapshot_parse_error.as_deref(),
            Some("negative_hidden_snapshot_count:-1")
        );
        assert_eq!(
            state.last_hidden_snapshot_parse_error_payload_len,
            Some(malformed_payload.len())
        );
        assert_eq!(
            state.last_hidden_snapshot,
            expected_hidden_snapshot
        );
        assert_eq!(state.hidden_snapshot_ids, expected_hidden_snapshot_ids);
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            expected_hidden_snapshot_delta_projection
        );
        assert_eq!(
            state.entity_table_projection.by_entity_id[&101].hidden,
            expected_entity_hidden
        );
        assert_eq!(
            state.entity_table_projection.hidden_apply_count,
            expected_hidden_apply_count
        );
        assert_eq!(
            state.entity_table_projection.hidden_count,
            expected_hidden_count
        );
        assert_eq!(
            state.hidden_lifecycle_remove_count,
            expected_hidden_lifecycle_remove_count
        );
        assert_eq!(
            state.last_hidden_lifecycle_removed_ids_sample,
            expected_hidden_lifecycle_removed_ids_sample
        );
    }

    #[test]
    fn hidden_snapshot_trailing_bytes_parse_error_leaves_hidden_state_unchanged() {
        let initial_payload = [
            0x00, 0x00, 0x00, 0x02, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0x00, 0x00, 0x01, 0x2F, // 303
        ];
        let malformed_payload = [
            0x00, 0x00, 0x00, 0x01, // count
            0x00, 0x00, 0x00, 0x65, // 101
            0xFF, // trailing byte
        ];
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 100,
                x_bits: 0.0f32.to_bits(),
                y_bits: 0.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
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

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &initial_payload,
            ),
        );
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);
        assert_eq!(state.hidden_snapshot_ids.len(), 2);
        let expected_hidden_snapshot = state.last_hidden_snapshot.clone();
        let expected_hidden_snapshot_ids = state.hidden_snapshot_ids.clone();
        let expected_hidden_snapshot_delta_projection =
            state.hidden_snapshot_delta_projection.clone();
        let expected_entity_hidden = state.entity_table_projection.by_entity_id[&101].hidden;
        let expected_hidden_apply_count = state.entity_table_projection.hidden_apply_count;
        let expected_hidden_count = state.entity_table_projection.hidden_count;
        let expected_hidden_lifecycle_remove_count = state.hidden_lifecycle_remove_count;
        let expected_hidden_lifecycle_removed_ids_sample =
            state.last_hidden_lifecycle_removed_ids_sample.clone();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                49,
                &malformed_payload,
            ),
        );

        assert_eq!(state.received_hidden_snapshot_count, 2);
        assert_eq!(state.applied_hidden_snapshot_count, 1);
        assert_eq!(state.failed_hidden_snapshot_parse_count, 1);
        assert_eq!(
            state.last_hidden_snapshot_parse_error.as_deref(),
            Some("hidden_snapshot_trailing_bytes:8/9")
        );
        assert_eq!(
            state.last_hidden_snapshot_parse_error_payload_len,
            Some(malformed_payload.len())
        );
        assert_eq!(
            state.last_hidden_snapshot,
            expected_hidden_snapshot
        );
        assert_eq!(state.hidden_snapshot_ids, expected_hidden_snapshot_ids);
        assert_eq!(
            state.hidden_snapshot_delta_projection,
            expected_hidden_snapshot_delta_projection
        );
        assert_eq!(
            state.entity_table_projection.by_entity_id[&101].hidden,
            expected_entity_hidden
        );
        assert_eq!(
            state.entity_table_projection.hidden_apply_count,
            expected_hidden_apply_count
        );
        assert_eq!(
            state.entity_table_projection.hidden_count,
            expected_hidden_count
        );
        assert_eq!(
            state.hidden_lifecycle_remove_count,
            expected_hidden_lifecycle_remove_count
        );
        assert_eq!(
            state.last_hidden_lifecycle_removed_ids_sample,
            expected_hidden_lifecycle_removed_ids_sample
        );
    }

    #[test]
    fn hidden_snapshot_rejects_impossible_positive_count_before_allocating_ids() {
        let payload = i32::MAX.to_be_bytes();
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 49, &payload),
        );

        assert_eq!(state.received_hidden_snapshot_count, 1);
        assert_eq!(state.applied_hidden_snapshot_count, 0);
        assert_eq!(state.failed_hidden_snapshot_parse_count, 1);
        assert_eq!(
            state.last_hidden_snapshot_parse_error.as_deref(),
            Some("truncated_hidden_snapshot_payload")
        );
        assert_eq!(
            state.last_hidden_snapshot_parse_error_payload_len,
            Some(payload.len())
        );
    }
}
