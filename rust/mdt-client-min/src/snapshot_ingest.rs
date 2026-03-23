use crate::session_state::{
    AppliedBlockSnapshotEnvelope, AppliedHiddenSnapshotIds, AppliedStateSnapshot,
    AppliedStateSnapshotCoreData, AppliedStateSnapshotCoreDataItem,
    AppliedStateSnapshotCoreDataTeam, BlockSnapshotHeadProjection, GameplayStateProjection,
    HiddenSnapshotDeltaProjection, SessionState, StateSnapshotAuthorityProjection,
    StateSnapshotBusinessProjection,
};
use mdt_remote::HighFrequencyRemoteMethod;
use mdt_world::parse_building_base_snapshot_bytes;
use std::collections::{BTreeMap, BTreeSet};
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
                            let (duplicate_team_count, duplicate_item_count) =
                                count_state_snapshot_core_data_duplicates(&core_data);
                            state.last_state_snapshot_core_data = Some(core_data.clone());
                            state.last_good_state_snapshot_core_data = Some(core_data);
                            state.last_state_snapshot_core_data_duplicate_team_count =
                                duplicate_team_count;
                            state.last_state_snapshot_core_data_duplicate_item_count =
                                duplicate_item_count;
                            state.state_snapshot_core_data_duplicate_team_count_total = state
                                .state_snapshot_core_data_duplicate_team_count_total
                                .saturating_add(duplicate_team_count as u64);
                            state.state_snapshot_core_data_duplicate_item_count_total = state
                                .state_snapshot_core_data_duplicate_item_count_total
                                .saturating_add(duplicate_item_count as u64);
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
                    state.block_snapshot_head_projection = parsed
                        .first_build_pos
                        .zip(parsed.first_block_id)
                        .map(|(build_pos, block_id)| {
                            state.building_table_projection.apply_block_snapshot_head(
                                build_pos,
                                block_id,
                                parsed.first_rotation,
                                parsed.first_team_id,
                                parsed.first_io_version,
                                parsed.first_module_bitmask,
                                parsed.first_time_scale_bits,
                                parsed.first_time_scale_duration_bits,
                                parsed.first_last_disabler_pos,
                                parsed.first_legacy_consume_connected,
                                parsed.first_health_bits,
                                parsed.first_enabled,
                                parsed.first_efficiency,
                                parsed.first_optional_efficiency,
                                parsed.first_visible_flags,
                            );
                            BlockSnapshotHeadProjection {
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
                            }
                        });
                    state.last_block_snapshot = Some(parsed);
                    state.last_block_snapshot_parse_error = None;
                    state.last_block_snapshot_parse_error_payload_len = None;
                }
                Err(error) => {
                    state.failed_block_snapshot_parse_count =
                        state.failed_block_snapshot_parse_count.saturating_add(1);
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
                    let previous_hidden_ids = state.hidden_snapshot_ids.clone();
                    let trigger_hidden_ids = parsed.ids.iter().copied().collect::<BTreeSet<_>>();
                    let trigger_count = trigger_hidden_ids.len();
                    let added_ids = trigger_hidden_ids
                        .difference(&previous_hidden_ids)
                        .copied()
                        .collect::<Vec<_>>();
                    let removed_ids = previous_hidden_ids
                        .difference(&trigger_hidden_ids)
                        .copied()
                        .collect::<Vec<_>>();
                    let added_sample_ids = added_ids
                        .iter()
                        .take(HIDDEN_SNAPSHOT_SAMPLE_LIMIT)
                        .copied()
                        .collect();
                    let removed_sample_ids = removed_ids
                        .iter()
                        .take(HIDDEN_SNAPSHOT_SAMPLE_LIMIT)
                        .copied()
                        .collect();
                    state.applied_hidden_snapshot_count =
                        state.applied_hidden_snapshot_count.saturating_add(1);
                    state.last_hidden_snapshot = Some(parsed.applied);
                    state
                        .entity_table_projection
                        .apply_hidden_ids(&trigger_hidden_ids);
                    let hidden_removed_ids = state
                        .entity_table_projection
                        .remove_hidden_entities(&trigger_hidden_ids);
                    for entity_id in &hidden_removed_ids {
                        state.record_entity_snapshot_tombstone(*entity_id);
                    }
                    state.hidden_lifecycle_remove_count = state
                        .hidden_lifecycle_remove_count
                        .saturating_add(hidden_removed_ids.len() as u64);
                    state.last_hidden_lifecycle_removed_ids_sample = hidden_removed_ids
                        .into_iter()
                        .take(HIDDEN_SNAPSHOT_SAMPLE_LIMIT)
                        .collect();
                    state.hidden_snapshot_ids = trigger_hidden_ids;
                    state.hidden_snapshot_delta_projection = Some(HiddenSnapshotDeltaProjection {
                        active_count: trigger_count,
                        added_count: added_ids.len(),
                        removed_count: removed_ids.len(),
                        added_sample_ids,
                        removed_sample_ids,
                    });
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
    let mut next_core_inventory_by_team = BTreeMap::new();
    let mut next_core_inventory_item_entry_count = 0usize;
    let mut next_core_inventory_total_amount = 0i64;
    let mut next_core_inventory_nonzero_item_count = 0usize;

    if let Some(core_data) = core_data {
        for team in &core_data.teams {
            let mut items = BTreeMap::new();
            for item in &team.items {
                items.insert(item.item_id, item.amount);
                next_core_inventory_total_amount =
                    next_core_inventory_total_amount.saturating_add(i64::from(item.amount));
                if item.amount != 0 {
                    next_core_inventory_nonzero_item_count =
                        next_core_inventory_nonzero_item_count.saturating_add(1);
                }
            }
            next_core_inventory_item_entry_count =
                next_core_inventory_item_entry_count.saturating_add(items.len());
            next_core_inventory_by_team.insert(team.team_id, items);
        }
    } else if let Some(previous) = previous {
        next_core_inventory_by_team = previous.core_inventory_by_team.clone();
        next_core_inventory_item_entry_count = previous.core_inventory_item_entry_count;
        next_core_inventory_total_amount = previous.core_inventory_total_amount;
        next_core_inventory_nonzero_item_count = previous.core_inventory_nonzero_item_count;
    }

    let mut changed_team_ids = BTreeSet::new();
    if core_data.is_some() {
        if let Some(previous) = previous {
            for team_id in previous
                .core_inventory_by_team
                .keys()
                .chain(next_core_inventory_by_team.keys())
            {
                if previous.core_inventory_by_team.get(team_id)
                    != next_core_inventory_by_team.get(team_id)
                {
                    changed_team_ids.insert(*team_id);
                }
            }
        } else {
            changed_team_ids.extend(next_core_inventory_by_team.keys().copied());
        }
    }
    let core_inventory_changed_team_sample = changed_team_ids
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
        core_inventory_team_count: next_core_inventory_by_team.len(),
        core_inventory_item_entry_count: next_core_inventory_item_entry_count,
        core_inventory_total_amount: next_core_inventory_total_amount,
        core_inventory_nonzero_item_count: next_core_inventory_nonzero_item_count,
        core_inventory_changed_team_count: changed_team_ids.len(),
        core_inventory_changed_team_sample,
        core_inventory_by_team: next_core_inventory_by_team,
        last_core_sync_ok: core_data.is_some(),
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
    let mut next_core_inventory_by_team = BTreeMap::new();
    let mut next_core_inventory_item_entry_count = 0usize;
    let mut next_core_inventory_total_amount = 0i64;
    let mut next_core_inventory_nonzero_item_count = 0usize;

    if let Some(core_data) = core_data {
        for team in &core_data.teams {
            let mut items = BTreeMap::new();
            for item in &team.items {
                items.insert(item.item_id, item.amount);
                next_core_inventory_total_amount =
                    next_core_inventory_total_amount.saturating_add(i64::from(item.amount));
                if item.amount != 0 {
                    next_core_inventory_nonzero_item_count =
                        next_core_inventory_nonzero_item_count.saturating_add(1);
                }
            }
            next_core_inventory_item_entry_count =
                next_core_inventory_item_entry_count.saturating_add(items.len());
            next_core_inventory_by_team.insert(team.team_id, items);
        }
    } else if let Some(previous) = previous {
        next_core_inventory_by_team = previous.core_inventory_by_team.clone();
        next_core_inventory_item_entry_count = previous.core_inventory_item_entry_count;
        next_core_inventory_total_amount = previous.core_inventory_total_amount;
        next_core_inventory_nonzero_item_count = previous.core_inventory_nonzero_item_count;
    }

    let mut changed_team_ids = BTreeSet::new();
    if core_data.is_some() {
        if let Some(previous) = previous {
            for team_id in previous
                .core_inventory_by_team
                .keys()
                .chain(next_core_inventory_by_team.keys())
            {
                if previous.core_inventory_by_team.get(team_id)
                    != next_core_inventory_by_team.get(team_id)
                {
                    changed_team_ids.insert(*team_id);
                }
            }
        } else {
            changed_team_ids.extend(next_core_inventory_by_team.keys().copied());
        }
    }
    let core_inventory_changed_team_sample = changed_team_ids
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
        core_inventory_synced: core_data.is_some(),
        core_inventory_team_count: next_core_inventory_by_team.len(),
        core_inventory_item_entry_count: next_core_inventory_item_entry_count,
        core_inventory_total_amount: next_core_inventory_total_amount,
        core_inventory_nonzero_item_count: next_core_inventory_nonzero_item_count,
        core_inventory_changed_team_count: changed_team_ids.len(),
        core_inventory_changed_team_sample,
        core_inventory_by_team: next_core_inventory_by_team,
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

fn count_state_snapshot_core_data_duplicates(
    core_data: &AppliedStateSnapshotCoreData,
) -> (usize, usize) {
    let mut duplicate_team_count = 0usize;
    let mut duplicate_item_count = 0usize;
    let mut seen_team_ids = BTreeSet::new();
    let mut seen_item_ids_by_team = BTreeMap::<u8, BTreeSet<u16>>::new();

    for team in &core_data.teams {
        if !seen_team_ids.insert(team.team_id) {
            duplicate_team_count = duplicate_team_count.saturating_add(1);
        }
        let seen_item_ids = seen_item_ids_by_team.entry(team.team_id).or_default();
        for item in &team.items {
            if !seen_item_ids.insert(item.item_id) {
                duplicate_item_count = duplicate_item_count.saturating_add(1);
            }
        }
    }

    (duplicate_team_count, duplicate_item_count)
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
        BlockSnapshotHeadProjection, EntityProjection, GameplayStateProjection,
        HiddenSnapshotDeltaProjection, SessionState, StateSnapshotAuthorityProjection,
        StateSnapshotBusinessProjection,
    };
    use mdt_remote::HighFrequencyRemoteMethod;
    use std::collections::BTreeMap;

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
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 122, &payload),
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
    fn state_snapshot_ingest_tracks_duplicate_core_data_keys_without_changing_fold_behavior() {
        let core_data = build_core_data_payload(&[
            (1, &[(0, 10), (0, 20)]),
            (1, &[(1, 30)]),
            (2, &[(4, 40), (4, 50)]),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 122, &payload),
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
                (1u8, BTreeMap::from([(1u16, 30)])),
                (2u8, BTreeMap::from([(4u16, 50)])),
            ]))
        );
        assert_eq!(
            state
                .state_snapshot_business_projection
                .as_ref()
                .map(|projection| projection.core_inventory_by_team.clone()),
            Some(BTreeMap::from([
                (1u8, BTreeMap::from([(1u16, 30)])),
                (2u8, BTreeMap::from([(4u16, 50)])),
            ]))
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 122, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 122, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 122, &payload),
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
                122,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                122,
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
                122,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                122,
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
                122,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::StateSnapshot,
                122,
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
                122,
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
                122,
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
                122,
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
                122,
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::StateSnapshot, 122, &payload),
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
                122,
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
                122,
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
                122,
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
                122,
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &payload),
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
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.rotation),
            Some(2)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.team_id),
            Some(5)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.health_bits),
            Some(0x3f800000)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.enabled),
            Some(true)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.efficiency),
            Some(0x80)
        );
        assert_eq!(
            state
                .building_table_projection
                .by_build_pos
                .get(&0x0064_0063)
                .and_then(|building| building.optional_efficiency),
            Some(0x40)
        );
        assert_eq!(
            state.building_table_projection.last_update,
            Some(crate::session_state::BuildingProjectionUpdateKind::BlockSnapshotHead)
        );
    }

    #[test]
    fn malformed_block_snapshot_tracks_first_entry_header_error() {
        let payload = [0x00, 0x01, 0x00, 0x03, 0xAA, 0xBB, 0xCC];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &clear_payload),
        );
        assert_eq!(state.block_snapshot_head_projection, None);
        assert_eq!(state.building_table_projection.by_build_pos.len(), 1);
    }

    #[test]
    fn block_snapshot_ingest_skips_conflicting_head_over_existing_building_projection() {
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
            1,
            2,
            mdt_typeio::TypeIoObject::Int(7),
        );

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::BlockSnapshot, 47, &payload),
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
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 11, &payload),
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

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(
                HighFrequencyRemoteMethod::HiddenSnapshot,
                11,
                &initial_payload,
            ),
        );
        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 11, &next_payload),
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
    fn hidden_snapshot_removes_non_local_entity_rows_but_keeps_local_player() {
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

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 11, &payload),
        );

        assert_eq!(state.entity_table_projection.hidden_apply_count, 1);
        assert_eq!(state.entity_table_projection.hidden_count, 1);
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);
        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&303));
        assert_eq!(state.hidden_lifecycle_remove_count, 1);
        assert_eq!(state.last_hidden_lifecycle_removed_ids_sample, vec![303]);
    }

    #[test]
    fn hidden_snapshot_clears_stale_hidden_flag_for_tracked_local_entity() {
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
                11,
                &initial_payload,
            ),
        );
        assert!(state.entity_table_projection.by_entity_id[&101].hidden);

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 11, &next_payload),
        );

        assert!(!state.entity_table_projection.by_entity_id[&101].hidden);
        assert_eq!(state.entity_table_projection.hidden_apply_count, 2);
        assert_eq!(state.entity_table_projection.hidden_count, 0);
    }

    #[test]
    fn malformed_hidden_snapshot_tracks_parse_error() {
        let payload = [0xFF, 0xFF, 0xFF, 0xFF];
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 11, &payload),
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
    fn hidden_snapshot_rejects_impossible_positive_count_before_allocating_ids() {
        let payload = i32::MAX.to_be_bytes();
        let mut state = SessionState::default();

        ingest_inbound_snapshot(
            &mut state,
            InboundSnapshot::new(HighFrequencyRemoteMethod::HiddenSnapshot, 11, &payload),
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
