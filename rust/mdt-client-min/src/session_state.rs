use crate::rules_objectives_semantics::{ObjectivesProjection, RulesProjection};
use crate::state_snapshot_semantics::{
    derive_state_snapshot_core_inventory_transition, StateSnapshotCoreInventoryPrevious,
};
use mdt_remote::HighFrequencyRemoteMethod;
use mdt_typeio::TypeIoObject;
use std::collections::{BTreeMap, BTreeSet};

const ENTITY_SNAPSHOT_TOMBSTONE_TTL_SNAPSHOTS: u64 = 1;
const ENTITY_SNAPSHOT_TOMBSTONE_SKIP_SAMPLE_LIMIT: usize = 4;
const CORE_INVENTORY_CHANGED_TEAM_SAMPLE_LIMIT: usize = 4;
const HIDDEN_SNAPSHOT_SAMPLE_LIMIT: usize = 4;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AppliedStateSnapshotCoreDataItem {
    pub item_id: u16,
    pub amount: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AppliedStateSnapshotCoreDataTeam {
    pub team_id: u8,
    pub items: Vec<AppliedStateSnapshotCoreDataItem>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AppliedStateSnapshotCoreData {
    pub team_count: u8,
    pub teams: Vec<AppliedStateSnapshotCoreDataTeam>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GameplayStateProjection {
    #[default]
    Playing,
    Paused,
    GameOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTimeoutKind {
    ConnectOrLoading,
    ReadySnapshotStall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTimeoutProjection {
    pub kind: SessionTimeoutKind,
    pub idle_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionResetKind {
    Reconnect,
    WorldReload,
    Kick,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectPhaseProjection {
    #[default]
    Idle,
    Scheduled,
    Attempting,
    Succeeded,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectReasonKind {
    ConnectRedirect,
    Kick,
    Timeout,
    ManualConnect,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconnectProjection {
    pub phase: ReconnectPhaseProjection,
    pub phase_transition_count: u64,
    pub reason_kind: Option<ReconnectReasonKind>,
    pub reason_text: Option<String>,
    pub reason_ordinal: Option<i32>,
    pub hint_text: Option<String>,
}

impl ReconnectProjection {
    fn set_phase(&mut self, phase: ReconnectPhaseProjection) {
        if self.phase != phase {
            self.phase_transition_count = self.phase_transition_count.saturating_add(1);
            self.phase = phase;
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorldReloadProjection {
    pub had_loaded_world: bool,
    pub had_client_loaded: bool,
    pub was_ready_to_enter_world: bool,
    pub had_connect_confirm_sent: bool,
    pub cleared_pending_packets: usize,
    pub cleared_deferred_inbound_packets: usize,
    pub cleared_replayed_loading_events: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateSnapshotBusinessProjection {
    pub wave_time_bits: u32,
    pub wave: i32,
    pub enemies: i32,
    pub paused: bool,
    pub game_over: bool,
    pub time_data: i32,
    pub tps: u8,
    pub rand0: i64,
    pub rand1: i64,
    pub gameplay_state: GameplayStateProjection,
    pub gameplay_state_transition_count: u64,
    pub last_wave_advanced: bool,
    pub last_wave_advance_from: Option<i32>,
    pub last_wave_advance_to: Option<i32>,
    pub wave_advance_count: u64,
    pub net_seconds_applied_count: u64,
    pub last_net_seconds_rollback: bool,
    pub net_seconds_delta: i32,
    pub state_snapshot_apply_count: u64,
    pub state_snapshot_time_regress_count: u64,
    pub state_snapshot_wave_regress_count: u64,
    pub core_inventory_synced: bool,
    pub core_inventory_team_count: usize,
    pub core_inventory_item_entry_count: usize,
    pub core_inventory_total_amount: i64,
    pub core_inventory_nonzero_item_count: usize,
    pub core_inventory_changed_team_count: usize,
    pub core_inventory_changed_team_sample: Vec<u8>,
    pub core_inventory_by_team: BTreeMap<u8, BTreeMap<u16, i32>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateSnapshotAuthorityProjection {
    pub wave_time_bits: u32,
    pub wave: i32,
    pub enemies: i32,
    pub paused: bool,
    pub game_over: bool,
    pub time_data: i32,
    pub tps: u8,
    pub rand0: i64,
    pub rand1: i64,
    pub gameplay_state: GameplayStateProjection,
    pub last_wave_advanced: bool,
    pub wave_advance_count: u64,
    pub state_snapshot_apply_count: u64,
    pub last_net_seconds_rollback: bool,
    pub net_seconds_delta: i32,
    pub state_snapshot_wave_regress_count: u64,
    pub core_inventory_team_count: usize,
    pub core_inventory_item_entry_count: usize,
    pub core_inventory_total_amount: i64,
    pub core_inventory_nonzero_item_count: usize,
    pub core_inventory_changed_team_count: usize,
    pub core_inventory_changed_team_sample: Vec<u8>,
    pub core_inventory_by_team: BTreeMap<u8, BTreeMap<u16, i32>>,
    pub last_core_sync_ok: bool,
    pub core_parse_fail_count: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AuthoritativeStateMirror {
    pub wave_time_bits: u32,
    pub wave: i32,
    pub enemies: i32,
    pub paused: bool,
    pub game_over: bool,
    pub net_seconds: i32,
    pub tps: u8,
    pub rand0: i64,
    pub rand1: i64,
    pub gameplay_state: GameplayStateProjection,
    pub last_wave_advanced: bool,
    pub wave_advance_count: u64,
    pub apply_count: u64,
    pub last_net_seconds_rollback: bool,
    pub net_seconds_delta: i32,
    pub wave_regress_count: u64,
    pub core_inventory_team_count: usize,
    pub core_inventory_item_entry_count: usize,
    pub core_inventory_total_amount: i64,
    pub core_inventory_nonzero_item_count: usize,
    pub core_inventory_changed_team_count: usize,
    pub core_inventory_changed_team_sample: Vec<u8>,
    pub core_inventory_by_team: BTreeMap<u8, BTreeMap<u16, i32>>,
    pub last_core_sync_ok: bool,
    pub core_parse_fail_count: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AppliedStateSnapshot {
    pub wave_time_bits: u32,
    pub wave: i32,
    pub enemies: i32,
    pub paused: bool,
    pub game_over: bool,
    pub time_data: i32,
    pub tps: u8,
    pub rand0: i64,
    pub rand1: i64,
    pub core_data: Vec<u8>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AppliedBlockSnapshotEnvelope {
    pub amount: i16,
    pub data_len: usize,
    pub first_build_pos: Option<i32>,
    pub first_block_id: Option<i16>,
    pub first_health_bits: Option<u32>,
    pub first_rotation: Option<u8>,
    pub first_team_id: Option<u8>,
    pub first_io_version: Option<u8>,
    pub first_enabled: Option<bool>,
    pub first_module_bitmask: Option<u8>,
    pub first_time_scale_bits: Option<u32>,
    pub first_time_scale_duration_bits: Option<u32>,
    pub first_last_disabler_pos: Option<i32>,
    pub first_legacy_consume_connected: Option<bool>,
    pub first_efficiency: Option<u8>,
    pub first_optional_efficiency: Option<u8>,
    pub first_visible_flags: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSnapshotHeadProjection {
    pub build_pos: i32,
    pub block_id: i16,
    pub health_bits: Option<u32>,
    pub rotation: Option<u8>,
    pub team_id: Option<u8>,
    pub io_version: Option<u8>,
    pub enabled: Option<bool>,
    pub module_bitmask: Option<u8>,
    pub time_scale_bits: Option<u32>,
    pub time_scale_duration_bits: Option<u32>,
    pub last_disabler_pos: Option<i32>,
    pub legacy_consume_connected: Option<bool>,
    pub efficiency: Option<u8>,
    pub optional_efficiency: Option<u8>,
    pub visible_flags: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AppliedHiddenSnapshotIds {
    pub count: i32,
    pub first_id: Option<i32>,
    pub sample_ids: Vec<i32>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HiddenSnapshotDeltaProjection {
    pub active_count: usize,
    pub added_count: usize,
    pub removed_count: usize,
    pub added_sample_ids: Vec<i32>,
    pub removed_sample_ids: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectDataSemantic {
    Null,
    Int(i32),
    Long(i64),
    FloatBits(u32),
    String(Option<String>),
    ContentRaw { content_type: u8, content_id: i16 },
    IntSeqLen(usize),
    Point2 { x: i32, y: i32 },
    PackedPoint2ArrayLen(usize),
    TechNodeRaw { content_type: u8, content_id: i16 },
    Bool(bool),
    DoubleBits(u64),
    BuildingPos(i32),
    LAccess(i16),
    BytesLen(usize),
    LegacyUnitCommandNull(u8),
    BoolArrayLen(usize),
    UnitId(i32),
    Vec2ArrayLen(usize),
    Vec2 { x_bits: u32, y_bits: u32 },
    Team(u8),
    IntArrayLen(usize),
    ObjectArrayLen(usize),
    UnitCommand(u16),
    OpaqueTypeTag(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectBusinessPositionSource {
    BuildingPos,
    Point2,
    Vec2,
    EntityUnitId,
    LocalUnitId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectBusinessContentKind {
    Content,
    TechNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfiguredContentRef {
    pub content_type: u8,
    pub content_id: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectBusinessProjection {
    ContentRef {
        kind: EffectBusinessContentKind,
        content_type: u8,
        content_id: i16,
    },
    ParentRef {
        source: EffectBusinessPositionSource,
        value: i32,
        x_bits: u32,
        y_bits: u32,
    },
    WorldPosition {
        source: EffectBusinessPositionSource,
        x_bits: u32,
        y_bits: u32,
    },
    PositionTarget {
        source_x_bits: u32,
        source_y_bits: u32,
        target_x_bits: u32,
        target_y_bits: u32,
    },
    LengthRay {
        source_x_bits: u32,
        source_y_bits: u32,
        target_x_bits: u32,
        target_y_bits: u32,
        rotation_bits: u32,
        length_bits: u32,
    },
    FloatValue(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitRefProjection {
    pub kind: u8,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeItemsProjection {
    pub build_pos: Option<i32>,
    pub item_id: Option<i16>,
    pub amount: i32,
    pub to: Option<UnitRefProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferItemToProjection {
    pub unit: Option<UnitRefProjection>,
    pub item_id: Option<i16>,
    pub amount: i32,
    pub x_bits: u32,
    pub y_bits: u32,
    pub build_pos: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferItemToUnitProjection {
    pub item_id: Option<i16>,
    pub x_bits: u32,
    pub y_bits: u32,
    pub to_entity_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferItemEffectProjection {
    pub item_id: Option<i16>,
    pub x_bits: u32,
    pub y_bits: u32,
    pub to_entity_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestroyPayloadProjection {
    pub build_pos: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBulletProjection {
    pub bullet_type_id: Option<i16>,
    pub team_id: u8,
    pub x_bits: u32,
    pub y_bits: u32,
    pub angle_bits: u32,
    pub damage_bits: u32,
    pub velocity_scl_bits: u32,
    pub lifetime_scl_bits: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResourceUnitItemStack {
    pub item_id: Option<i16>,
    pub amount: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResourceDeltaProjection {
    pub take_items_count: u64,
    pub transfer_item_to_count: u64,
    pub transfer_item_to_unit_count: u64,
    pub last_kind: Option<&'static str>,
    pub last_item_id: Option<i16>,
    pub last_amount: Option<i32>,
    pub last_build_pos: Option<i32>,
    pub last_unit: Option<UnitRefProjection>,
    pub last_to_entity_id: Option<i32>,
    pub last_x_bits: Option<u32>,
    pub last_y_bits: Option<u32>,
    pub building_items_by_build: BTreeMap<i32, BTreeMap<i16, i32>>,
    pub entity_item_stack_by_entity_id: BTreeMap<i32, ResourceUnitItemStack>,
    pub authoritative_build_update_count: u64,
    pub delta_apply_count: u64,
    pub delta_skip_count: u64,
    pub delta_conflict_count: u64,
    pub last_changed_build_pos: Option<i32>,
    pub last_changed_entity_id: Option<i32>,
    pub last_changed_item_id: Option<i16>,
    pub last_changed_amount: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceDeltaOutcome {
    Applied,
    Skipped,
    Conflicted,
}

impl ResourceDeltaProjection {
    pub fn build_count(&self) -> usize {
        self.building_items_by_build.len()
    }

    pub fn build_stack_count(&self) -> usize {
        self.building_items_by_build
            .values()
            .map(BTreeMap::len)
            .sum()
    }

    pub fn entity_count(&self) -> usize {
        self.entity_item_stack_by_entity_id.len()
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }

    pub fn apply_set_item(&mut self, build_pos: Option<i32>, item_id: Option<i16>, amount: i32) {
        let (Some(build_pos), Some(item_id)) = (build_pos, item_id) else {
            return;
        };
        self.set_build_item_exact(build_pos, item_id, amount);
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.mark_build_change(build_pos, item_id, amount);
    }

    pub fn apply_set_items(&mut self, build_pos: Option<i32>, stacks: &[(Option<i16>, i32)]) {
        let Some(build_pos) = build_pos else {
            return;
        };
        let mut applied = false;
        for &(item_id, amount) in stacks {
            let Some(item_id) = item_id else {
                continue;
            };
            self.set_build_item_exact(build_pos, item_id, amount);
            self.mark_build_change(build_pos, item_id, amount);
            applied = true;
        }
        if applied {
            self.authoritative_build_update_count =
                self.authoritative_build_update_count.saturating_add(1);
        }
    }

    pub fn apply_set_tile_items(&mut self, item_id: Option<i16>, amount: i32, positions: &[i32]) {
        let Some(item_id) = item_id else {
            return;
        };
        let mut applied = false;
        for &build_pos in positions {
            self.set_build_item_exact(build_pos, item_id, amount);
            self.mark_build_change(build_pos, item_id, amount);
            applied = true;
        }
        if applied {
            self.authoritative_build_update_count =
                self.authoritative_build_update_count.saturating_add(1);
        }
    }

    pub fn clear_build_items(&mut self, build_pos: Option<i32>) {
        let Some(build_pos) = build_pos else {
            return;
        };
        self.building_items_by_build.remove(&build_pos);
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.last_changed_build_pos = Some(build_pos);
        self.last_changed_entity_id = None;
        self.last_changed_item_id = None;
        self.last_changed_amount = Some(0);
    }

    pub fn remove_building(&mut self, build_pos: Option<i32>) {
        self.clear_build_items(build_pos);
    }

    pub fn remove_standard_entity_item(&mut self, unit: Option<UnitRefProjection>) {
        let Some(entity_id) = resource_delta_standard_entity_id(unit) else {
            return;
        };
        self.entity_item_stack_by_entity_id.remove(&entity_id);
    }

    pub fn remove_entity_item_by_id(&mut self, entity_id: Option<i32>) {
        let Some(entity_id) = entity_id else {
            return;
        };
        self.entity_item_stack_by_entity_id.remove(&entity_id);
    }

    pub fn remove_hidden_entities(
        &mut self,
        hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) -> Vec<i32> {
        let removed_ids = self
            .entity_item_stack_by_entity_id
            .keys()
            .copied()
            .filter(|&entity_id| {
                hidden_lifecycle_matches_hidden_non_local_entity_id(
                    hidden_ids,
                    local_player_entity_id,
                    entity_id,
                )
            })
            .collect::<Vec<_>>();
        for entity_id in &removed_ids {
            self.entity_item_stack_by_entity_id.remove(entity_id);
        }
        removed_ids
    }

    pub fn apply_take_items(&mut self, projection: &TakeItemsProjection) {
        let Some(item_id) = projection.item_id else {
            self.delta_skip_count = self.delta_skip_count.saturating_add(1);
            return;
        };

        let mut applied = false;
        let mut skipped = false;
        let mut conflicted = false;

        match projection.build_pos {
            Some(build_pos) => {
                if self.subtract_known_build_item(build_pos, item_id, projection.amount) {
                    self.mark_build_change(build_pos, item_id, projection.amount);
                    applied = true;
                } else {
                    skipped = true;
                }
            }
            None => skipped = true,
        }

        match projection.to {
            Some(unit_ref) => match resource_delta_standard_entity_id(Some(unit_ref)) {
                Some(entity_id) => {
                    match self.add_entity_item(entity_id, item_id, projection.amount) {
                        ResourceDeltaOutcome::Applied => applied = true,
                        ResourceDeltaOutcome::Skipped => skipped = true,
                        ResourceDeltaOutcome::Conflicted => conflicted = true,
                    }
                }
                None => skipped = true,
            },
            None => skipped = true,
        }

        self.record_delta_outcome(applied, skipped, conflicted);
    }

    pub fn apply_transfer_item_to(&mut self, projection: &TransferItemToProjection) {
        let Some(item_id) = projection.item_id else {
            self.delta_skip_count = self.delta_skip_count.saturating_add(1);
            return;
        };

        let mut applied = false;
        let mut skipped = false;
        let mut conflicted = false;

        match projection.build_pos {
            Some(build_pos) => {
                if self.add_known_build_item(build_pos, item_id, projection.amount) {
                    self.mark_build_change(build_pos, item_id, projection.amount);
                    applied = true;
                } else {
                    skipped = true;
                }
            }
            None => skipped = true,
        }

        match projection.unit {
            Some(unit_ref) => match resource_delta_standard_entity_id(Some(unit_ref)) {
                Some(entity_id) => {
                    match self.subtract_entity_item(entity_id, item_id, projection.amount) {
                        ResourceDeltaOutcome::Applied => applied = true,
                        ResourceDeltaOutcome::Skipped => skipped = true,
                        ResourceDeltaOutcome::Conflicted => conflicted = true,
                    }
                }
                None => skipped = true,
            },
            None => skipped = true,
        }

        self.record_delta_outcome(applied, skipped, conflicted);
    }

    pub fn apply_transfer_item_to_unit(&mut self, projection: &TransferItemToUnitProjection) {
        let (Some(item_id), Some(entity_id)) = (projection.item_id, projection.to_entity_id) else {
            self.delta_skip_count = self.delta_skip_count.saturating_add(1);
            return;
        };

        let outcome = self.add_entity_item(entity_id, item_id, 1);
        self.record_delta_outcome(
            outcome == ResourceDeltaOutcome::Applied,
            outcome == ResourceDeltaOutcome::Skipped,
            outcome == ResourceDeltaOutcome::Conflicted,
        );
    }

    fn record_delta_outcome(&mut self, applied: bool, skipped: bool, conflicted: bool) {
        if applied {
            self.delta_apply_count = self.delta_apply_count.saturating_add(1);
        }
        if skipped {
            self.delta_skip_count = self.delta_skip_count.saturating_add(1);
        }
        if conflicted {
            self.delta_conflict_count = self.delta_conflict_count.saturating_add(1);
        }
    }

    fn set_build_item_exact(&mut self, build_pos: i32, item_id: i16, amount: i32) {
        if amount == 0 {
            let mut remove_build = false;
            if let Some(build_items) = self.building_items_by_build.get_mut(&build_pos) {
                build_items.remove(&item_id);
                remove_build = build_items.is_empty();
            }
            if remove_build {
                self.building_items_by_build.remove(&build_pos);
            }
            return;
        }

        self.building_items_by_build
            .entry(build_pos)
            .or_default()
            .insert(item_id, amount);
    }

    fn add_known_build_item(&mut self, build_pos: i32, item_id: i16, amount: i32) -> bool {
        let mut applied = false;
        let mut remove_build = false;
        if let Some(build_items) = self.building_items_by_build.get_mut(&build_pos) {
            if let Some(current) = build_items.get_mut(&item_id) {
                *current = current.saturating_add(amount);
                applied = true;
                if *current == 0 {
                    build_items.remove(&item_id);
                    remove_build = build_items.is_empty();
                }
            }
        }
        if remove_build {
            self.building_items_by_build.remove(&build_pos);
        }
        applied
    }

    fn subtract_known_build_item(&mut self, build_pos: i32, item_id: i16, amount: i32) -> bool {
        let mut applied = false;
        let mut remove_build = false;
        if let Some(build_items) = self.building_items_by_build.get_mut(&build_pos) {
            if let Some(current) = build_items.get_mut(&item_id) {
                *current = current.saturating_sub(amount);
                applied = true;
                if *current == 0 {
                    build_items.remove(&item_id);
                    remove_build = build_items.is_empty();
                }
            }
        }
        if remove_build {
            self.building_items_by_build.remove(&build_pos);
        }
        applied
    }

    fn add_entity_item(
        &mut self,
        entity_id: i32,
        item_id: i16,
        amount: i32,
    ) -> ResourceDeltaOutcome {
        if amount <= 0 {
            return ResourceDeltaOutcome::Skipped;
        }

        let Some(entry) = self.entity_item_stack_by_entity_id.get(&entity_id).cloned() else {
            self.entity_item_stack_by_entity_id.insert(
                entity_id,
                ResourceUnitItemStack {
                    item_id: Some(item_id),
                    amount,
                },
            );
            self.mark_entity_change(entity_id, item_id, amount);
            return ResourceDeltaOutcome::Applied;
        };

        if entry.amount == 0 || entry.item_id.is_none() || entry.item_id == Some(item_id) {
            self.entity_item_stack_by_entity_id.insert(
                entity_id,
                ResourceUnitItemStack {
                    item_id: Some(item_id),
                    amount: entry.amount.saturating_add(amount),
                },
            );
            self.mark_entity_change(entity_id, item_id, amount);
            return ResourceDeltaOutcome::Applied;
        }

        ResourceDeltaOutcome::Conflicted
    }

    fn subtract_entity_item(
        &mut self,
        entity_id: i32,
        item_id: i16,
        amount: i32,
    ) -> ResourceDeltaOutcome {
        if amount <= 0 {
            return ResourceDeltaOutcome::Skipped;
        }

        let Some(entry) = self.entity_item_stack_by_entity_id.get(&entity_id).cloned() else {
            return ResourceDeltaOutcome::Skipped;
        };
        if entry.item_id != Some(item_id) {
            return ResourceDeltaOutcome::Conflicted;
        }

        let next_amount = entry.amount.saturating_sub(amount);
        if next_amount == 0 {
            self.entity_item_stack_by_entity_id.remove(&entity_id);
        } else {
            self.entity_item_stack_by_entity_id.insert(
                entity_id,
                ResourceUnitItemStack {
                    item_id: Some(item_id),
                    amount: next_amount,
                },
            );
        }
        self.mark_entity_change(entity_id, item_id, amount);
        ResourceDeltaOutcome::Applied
    }

    fn mark_build_change(&mut self, build_pos: i32, item_id: i16, amount: i32) {
        self.last_changed_build_pos = Some(build_pos);
        self.last_changed_entity_id = None;
        self.last_changed_item_id = Some(item_id);
        self.last_changed_amount = Some(amount);
    }

    fn mark_entity_change(&mut self, entity_id: i32, item_id: i16, amount: i32) {
        self.last_changed_build_pos = None;
        self.last_changed_entity_id = Some(entity_id);
        self.last_changed_item_id = Some(item_id);
        self.last_changed_amount = Some(amount);
    }
}

fn resource_delta_standard_entity_id(unit: Option<UnitRefProjection>) -> Option<i32> {
    match unit {
        Some(UnitRefProjection { kind: 2, value }) => Some(value),
        _ => None,
    }
}

fn hidden_lifecycle_matches_hidden_non_local_entity_id(
    hidden_ids: &BTreeSet<i32>,
    local_player_entity_id: Option<i32>,
    entity_id: i32,
) -> bool {
    Some(entity_id) != local_player_entity_id && hidden_ids.contains(&entity_id)
}

fn hidden_lifecycle_hidden_non_local_unit_entity_id(
    unit: Option<UnitRefProjection>,
    hidden_ids: &BTreeSet<i32>,
    local_player_entity_id: Option<i32>,
) -> Option<i32> {
    let unit = unit?;
    (unit.kind == 2
        && hidden_lifecycle_matches_hidden_non_local_entity_id(
            hidden_ids,
            local_player_entity_id,
            unit.value,
        ))
    .then_some(unit.value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadDroppedProjection {
    pub unit: Option<UnitRefProjection>,
    pub x_bits: u32,
    pub y_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadLifecycleCarrierProjection {
    pub carrier: UnitRefProjection,
    pub target_unit: Option<UnitRefProjection>,
    pub target_build: Option<i32>,
    pub drop_tile: Option<i32>,
    pub on_ground: Option<bool>,
    pub removed_target_unit: bool,
    pub removed_target_build: bool,
    pub removed_carrier: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PayloadLifecycleProjection {
    pub by_carrier: BTreeMap<UnitRefProjection, PayloadLifecycleCarrierProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedBuildPayloadProjection {
    pub unit: Option<UnitRefProjection>,
    pub build_pos: Option<i32>,
    pub on_ground: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedUnitPayloadProjection {
    pub unit: Option<UnitRefProjection>,
    pub target: Option<UnitRefProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitEnteredPayloadProjection {
    pub unit: Option<UnitRefProjection>,
    pub build_pos: Option<i32>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorldBootstrapProjection {
    pub rules_sha256: String,
    pub map_locales_sha256: String,
    pub tags_sha256: String,
    pub team_count: usize,
    pub marker_count: usize,
    pub custom_chunk_count: usize,
    pub content_patch_count: usize,
    pub player_team_plan_count: usize,
    pub static_fog_team_count: usize,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TileConfigBusinessApply {
    pub business_applied: bool,
    pub cleared_pending_local: bool,
    pub was_rollback: bool,
    pub pending_local_match: Option<bool>,
    pub source: Option<TileConfigAuthoritySource>,
    pub authoritative_value: Option<TypeIoObject>,
    pub replaced_local_value: Option<TypeIoObject>,
    pub configured_block_outcome: Option<ConfiguredBlockOutcome>,
    pub configured_block_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileConfigAuthoritySource {
    TileConfigPacket,
    ConstructFinish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfiguredBlockOutcome {
    Applied,
    RejectedMissingBuilding,
    RejectedMissingBlockMetadata,
    RejectedUnsupportedBlock,
    RejectedUnsupportedConfigType,
}

impl ConfiguredBlockOutcome {
    pub fn is_applied(self) -> bool {
        matches!(self, Self::Applied)
    }

    pub fn is_rejected(self) -> bool {
        !self.is_applied()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::RejectedMissingBuilding => "missing_building",
            Self::RejectedMissingBlockMetadata => "missing_block_metadata",
            Self::RejectedUnsupportedBlock => "unsupported_block",
            Self::RejectedUnsupportedConfigType => "unsupported_config_type",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TileConfigProjection {
    pub pending_local_by_build_pos: BTreeMap<i32, TypeIoObject>,
    pub authoritative_by_build_pos: BTreeMap<i32, TypeIoObject>,
    pub queued_local_count: u64,
    pub applied_authoritative_count: u64,
    pub applied_tile_config_packet_count: u64,
    pub applied_construct_finish_count: u64,
    pub rollback_count: u64,
    pub configured_applied_count: u64,
    pub configured_rejected_count: u64,
    pub last_queued_build_pos: Option<i32>,
    pub last_queued_value: Option<TypeIoObject>,
    pub last_business_build_pos: Option<i32>,
    pub last_business_value: Option<TypeIoObject>,
    pub last_business_applied: bool,
    pub last_cleared_pending_local: bool,
    pub last_was_rollback: bool,
    pub last_pending_local_match: Option<bool>,
    pub last_business_source: Option<TileConfigAuthoritySource>,
    pub last_replaced_local_value: Option<TypeIoObject>,
    pub last_configured_block_outcome: Option<ConfiguredBlockOutcome>,
    pub last_configured_block_name: Option<String>,
}

impl TileConfigProjection {
    pub fn record_local_intent(&mut self, build_pos: i32, value: TypeIoObject) {
        self.queued_local_count = self.queued_local_count.saturating_add(1);
        self.last_queued_build_pos = Some(build_pos);
        self.last_queued_value = Some(value.clone());
        self.pending_local_by_build_pos.insert(build_pos, value);
    }

    pub fn apply_authoritative_update(
        &mut self,
        build_pos: i32,
        value: TypeIoObject,
        source: TileConfigAuthoritySource,
        configured_block_outcome: Option<ConfiguredBlockOutcome>,
        configured_block_name: Option<String>,
    ) -> TileConfigBusinessApply {
        self.applied_authoritative_count = self.applied_authoritative_count.saturating_add(1);
        match source {
            TileConfigAuthoritySource::TileConfigPacket => {
                self.applied_tile_config_packet_count =
                    self.applied_tile_config_packet_count.saturating_add(1);
            }
            TileConfigAuthoritySource::ConstructFinish => {
                self.applied_construct_finish_count =
                    self.applied_construct_finish_count.saturating_add(1);
            }
        }

        let pending_local = self.pending_local_by_build_pos.remove(&build_pos);
        let pending_local_match = pending_local.as_ref().map(|pending| pending == &value);
        let cleared_pending_local = pending_local.is_some();
        let was_rollback = pending_local_match == Some(false);
        if was_rollback {
            self.rollback_count = self.rollback_count.saturating_add(1);
        }

        self.authoritative_by_build_pos
            .insert(build_pos, value.clone());
        self.last_business_build_pos = Some(build_pos);
        self.last_business_value = Some(value.clone());
        self.last_business_applied = true;
        self.last_cleared_pending_local = cleared_pending_local;
        self.last_was_rollback = was_rollback;
        self.last_pending_local_match = pending_local_match;
        self.last_business_source = Some(source);
        self.last_replaced_local_value = pending_local.clone();
        self.last_configured_block_outcome = configured_block_outcome;
        self.last_configured_block_name = configured_block_name.clone();
        match configured_block_outcome {
            Some(outcome) if outcome.is_applied() => {
                self.configured_applied_count = self.configured_applied_count.saturating_add(1);
            }
            Some(outcome) if outcome.is_rejected() => {
                self.configured_rejected_count = self.configured_rejected_count.saturating_add(1);
            }
            _ => {}
        }

        TileConfigBusinessApply {
            business_applied: true,
            cleared_pending_local,
            was_rollback,
            pending_local_match,
            source: Some(source),
            authoritative_value: Some(value),
            replaced_local_value: pending_local,
            configured_block_outcome,
            configured_block_name,
        }
    }

    pub fn mark_packet_without_business_apply(&mut self) {
        self.last_business_build_pos = None;
        self.last_business_value = None;
        self.last_business_applied = false;
        self.last_cleared_pending_local = false;
        self.last_was_rollback = false;
        self.last_pending_local_match = None;
        self.last_business_source = None;
        self.last_replaced_local_value = None;
        self.last_configured_block_outcome = None;
        self.last_configured_block_name = None;
    }

    pub fn seed_authoritative_state(&mut self, build_pos: i32, value: TypeIoObject) {
        self.pending_local_by_build_pos.remove(&build_pos);
        self.authoritative_by_build_pos.insert(build_pos, value);
    }

    pub fn clear_pending_local_without_business_apply(
        &mut self,
        build_pos: Option<i32>,
    ) -> TileConfigBusinessApply {
        let pending_local =
            build_pos.and_then(|value| self.pending_local_by_build_pos.remove(&value));
        let cleared_pending_local = pending_local.is_some();
        self.last_business_build_pos = None;
        self.last_business_value = None;
        self.last_business_applied = false;
        self.last_cleared_pending_local = cleared_pending_local;
        self.last_was_rollback = false;
        self.last_pending_local_match = None;
        self.last_business_source = None;
        self.last_replaced_local_value = pending_local.clone();
        self.last_configured_block_outcome = None;
        self.last_configured_block_name = None;
        TileConfigBusinessApply {
            business_applied: false,
            cleared_pending_local,
            was_rollback: false,
            pending_local_match: None,
            source: None,
            authoritative_value: None,
            replaced_local_value: pending_local,
            configured_block_outcome: None,
            configured_block_name: None,
        }
    }

    pub fn fallback_rollback_to_known_authority(
        &mut self,
        build_pos: Option<i32>,
        source: TileConfigAuthoritySource,
    ) -> TileConfigBusinessApply {
        let Some(build_pos) = build_pos else {
            return self.clear_pending_local_without_business_apply(None);
        };
        let pending_local = self.pending_local_by_build_pos.remove(&build_pos);
        let authoritative_value = self.authoritative_by_build_pos.get(&build_pos).cloned();
        let cleared_pending_local = pending_local.is_some();

        if pending_local.is_none() || authoritative_value.is_none() {
            self.last_business_build_pos = None;
            self.last_business_value = None;
            self.last_business_applied = false;
            self.last_cleared_pending_local = cleared_pending_local;
            self.last_was_rollback = false;
            self.last_pending_local_match = None;
            self.last_business_source = None;
            self.last_replaced_local_value = pending_local.clone();
            self.last_configured_block_outcome = None;
            self.last_configured_block_name = None;
            return TileConfigBusinessApply {
                business_applied: false,
                cleared_pending_local,
                was_rollback: false,
                pending_local_match: None,
                source: None,
                authoritative_value: None,
                replaced_local_value: pending_local,
                configured_block_outcome: None,
                configured_block_name: None,
            };
        }

        let authoritative_value = authoritative_value.unwrap();
        let pending_local_match = pending_local
            .as_ref()
            .map(|pending| pending == &authoritative_value);
        let was_rollback = pending_local_match == Some(false);
        if was_rollback {
            self.rollback_count = self.rollback_count.saturating_add(1);
        }

        self.last_business_build_pos = Some(build_pos);
        self.last_business_value = Some(authoritative_value.clone());
        self.last_business_applied = true;
        self.last_cleared_pending_local = true;
        self.last_was_rollback = was_rollback;
        self.last_pending_local_match = pending_local_match;
        self.last_business_source = Some(source);
        self.last_replaced_local_value = pending_local.clone();
        self.last_configured_block_outcome = None;
        self.last_configured_block_name = None;
        TileConfigBusinessApply {
            business_applied: true,
            cleared_pending_local: true,
            was_rollback,
            pending_local_match,
            source: Some(source),
            authoritative_value: Some(authoritative_value),
            replaced_local_value: pending_local,
            configured_block_outcome: None,
            configured_block_name: None,
        }
    }

    pub fn remove_building_state(&mut self, build_pos: i32) {
        self.pending_local_by_build_pos.remove(&build_pos);
        self.authoritative_by_build_pos.remove(&build_pos);
    }

    pub fn clear_for_world_reload(&mut self) {
        self.pending_local_by_build_pos.clear();
        self.authoritative_by_build_pos.clear();
        self.last_queued_build_pos = None;
        self.last_queued_value = None;
        self.mark_packet_without_business_apply();
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfiguredBlockProjection {
    pub unit_cargo_unload_point_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub item_source_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub liquid_source_liquid_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub landing_pad_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub sorter_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub inverted_sorter_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub switch_enabled_by_build_pos: BTreeMap<i32, Option<bool>>,
    pub door_open_by_build_pos: BTreeMap<i32, Option<bool>>,
    pub message_text_by_build_pos: BTreeMap<i32, String>,
    pub constructor_recipe_block_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub light_color_by_build_pos: BTreeMap<i32, i32>,
    pub payload_source_content_by_build_pos: BTreeMap<i32, Option<ConfiguredContentRef>>,
    pub payload_router_sorted_content_by_build_pos: BTreeMap<i32, Option<ConfiguredContentRef>>,
    pub item_bridge_link_by_build_pos: BTreeMap<i32, Option<i32>>,
    pub unloader_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub duct_unloader_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub duct_router_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub mass_driver_link_by_build_pos: BTreeMap<i32, Option<i32>>,
    pub payload_mass_driver_link_by_build_pos: BTreeMap<i32, Option<i32>>,
    pub power_node_links_by_build_pos: BTreeMap<i32, BTreeSet<i32>>,
    pub reconstructor_command_by_build_pos: BTreeMap<i32, Option<u16>>,
    pub canvas_bytes_by_build_pos: BTreeMap<i32, Vec<u8>>,
}

impl ConfiguredBlockProjection {
    pub fn apply_unit_cargo_unload_point_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.unit_cargo_unload_point_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_item_source_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.item_source_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_liquid_source_liquid(&mut self, build_pos: i32, liquid_id: Option<i16>) {
        self.liquid_source_liquid_by_build_pos
            .insert(build_pos, liquid_id);
    }

    pub fn apply_landing_pad_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.landing_pad_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_sorter_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.sorter_item_by_build_pos.insert(build_pos, item_id);
    }

    pub fn apply_inverted_sorter_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.inverted_sorter_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_switch_enabled(&mut self, build_pos: i32, enabled: Option<bool>) {
        self.switch_enabled_by_build_pos.insert(build_pos, enabled);
    }

    pub fn apply_door_open(&mut self, build_pos: i32, open: Option<bool>) {
        self.door_open_by_build_pos.insert(build_pos, open);
    }

    pub fn apply_message_text(&mut self, build_pos: i32, text: String) {
        self.message_text_by_build_pos.insert(build_pos, text);
    }

    pub fn apply_constructor_recipe_block(&mut self, build_pos: i32, block_id: Option<i16>) {
        self.constructor_recipe_block_by_build_pos
            .insert(build_pos, block_id);
    }

    pub fn apply_light_color(&mut self, build_pos: i32, color: i32) {
        self.light_color_by_build_pos.insert(build_pos, color);
    }

    pub fn apply_payload_source_content(
        &mut self,
        build_pos: i32,
        content: Option<ConfiguredContentRef>,
    ) {
        self.payload_source_content_by_build_pos
            .insert(build_pos, content);
    }

    pub fn apply_payload_router_sorted_content(
        &mut self,
        build_pos: i32,
        content: Option<ConfiguredContentRef>,
    ) {
        self.payload_router_sorted_content_by_build_pos
            .insert(build_pos, content);
    }

    pub fn apply_item_bridge_link(&mut self, build_pos: i32, link: Option<i32>) {
        self.item_bridge_link_by_build_pos.insert(build_pos, link);
    }

    pub fn apply_unloader_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.unloader_item_by_build_pos.insert(build_pos, item_id);
    }

    pub fn apply_duct_unloader_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.duct_unloader_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_duct_router_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.duct_router_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_mass_driver_link(&mut self, build_pos: i32, link: Option<i32>) {
        self.mass_driver_link_by_build_pos.insert(build_pos, link);
    }

    pub fn apply_payload_mass_driver_link(&mut self, build_pos: i32, link: Option<i32>) {
        self.payload_mass_driver_link_by_build_pos
            .insert(build_pos, link);
    }

    pub fn apply_power_node_link_toggle(&mut self, build_pos: i32, target_pos: i32) {
        let links = self
            .power_node_links_by_build_pos
            .entry(build_pos)
            .or_default();
        if !links.remove(&target_pos) {
            links.insert(target_pos);
        }
    }

    pub fn apply_power_node_links_full_replace(&mut self, build_pos: i32, targets: BTreeSet<i32>) {
        self.power_node_links_by_build_pos
            .insert(build_pos, targets);
    }

    pub fn apply_reconstructor_command(&mut self, build_pos: i32, command_id: Option<u16>) {
        self.reconstructor_command_by_build_pos
            .insert(build_pos, command_id);
    }

    pub fn apply_canvas_bytes(&mut self, build_pos: i32, bytes: Vec<u8>) {
        self.canvas_bytes_by_build_pos.insert(build_pos, bytes);
    }

    pub fn clear_building_state(&mut self, build_pos: i32) {
        self.unit_cargo_unload_point_item_by_build_pos
            .remove(&build_pos);
        self.item_source_item_by_build_pos.remove(&build_pos);
        self.liquid_source_liquid_by_build_pos.remove(&build_pos);
        self.landing_pad_item_by_build_pos.remove(&build_pos);
        self.sorter_item_by_build_pos.remove(&build_pos);
        self.inverted_sorter_item_by_build_pos.remove(&build_pos);
        self.switch_enabled_by_build_pos.remove(&build_pos);
        self.door_open_by_build_pos.remove(&build_pos);
        self.message_text_by_build_pos.remove(&build_pos);
        self.constructor_recipe_block_by_build_pos
            .remove(&build_pos);
        self.light_color_by_build_pos.remove(&build_pos);
        self.payload_source_content_by_build_pos.remove(&build_pos);
        self.payload_router_sorted_content_by_build_pos
            .remove(&build_pos);
        self.item_bridge_link_by_build_pos.remove(&build_pos);
        self.unloader_item_by_build_pos.remove(&build_pos);
        self.duct_unloader_item_by_build_pos.remove(&build_pos);
        self.duct_router_item_by_build_pos.remove(&build_pos);
        self.mass_driver_link_by_build_pos.remove(&build_pos);
        self.payload_mass_driver_link_by_build_pos
            .remove(&build_pos);
        self.power_node_links_by_build_pos.remove(&build_pos);
        self.reconstructor_command_by_build_pos.remove(&build_pos);
        self.canvas_bytes_by_build_pos.remove(&build_pos);
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingProjectionUpdateKind {
    WorldBaseline,
    BlockSnapshotHead,
    ConstructFinish,
    TileConfig,
    DeconstructFinish,
    BuildHealthUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildingProjection {
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub team_id: Option<u8>,
    pub io_version: Option<u8>,
    pub module_bitmask: Option<u8>,
    pub time_scale_bits: Option<u32>,
    pub time_scale_duration_bits: Option<u32>,
    pub last_disabler_pos: Option<i32>,
    pub legacy_consume_connected: Option<bool>,
    pub config: Option<TypeIoObject>,
    pub health_bits: Option<u32>,
    pub enabled: Option<bool>,
    pub efficiency: Option<u8>,
    pub optional_efficiency: Option<u8>,
    pub visible_flags: Option<u64>,
    pub build_turret_rotation_bits: Option<u32>,
    pub build_turret_plans_present: Option<bool>,
    pub build_turret_plan_count: Option<u16>,
    pub last_update: BuildingProjectionUpdateKind,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BuildingTableProjection {
    pub by_build_pos: BTreeMap<i32, BuildingProjection>,
    pub block_known_count: usize,
    pub configured_count: usize,
    pub block_snapshot_head_apply_count: u64,
    pub block_snapshot_head_conflict_skip_count: u64,
    pub construct_finish_apply_count: u64,
    pub tile_config_apply_count: u64,
    pub deconstruct_finish_apply_count: u64,
    pub build_health_apply_count: u64,
    pub last_build_pos: Option<i32>,
    pub last_block_id: Option<i16>,
    pub last_rotation: Option<u8>,
    pub last_team_id: Option<u8>,
    pub last_io_version: Option<u8>,
    pub last_module_bitmask: Option<u8>,
    pub last_time_scale_bits: Option<u32>,
    pub last_time_scale_duration_bits: Option<u32>,
    pub last_last_disabler_pos: Option<i32>,
    pub last_legacy_consume_connected: Option<bool>,
    pub last_config: Option<TypeIoObject>,
    pub last_health_bits: Option<u32>,
    pub last_enabled: Option<bool>,
    pub last_efficiency: Option<u8>,
    pub last_optional_efficiency: Option<u8>,
    pub last_visible_flags: Option<u64>,
    pub last_build_turret_rotation_bits: Option<u32>,
    pub last_build_turret_plans_present: Option<bool>,
    pub last_build_turret_plan_count: Option<u16>,
    pub last_update: Option<BuildingProjectionUpdateKind>,
    pub last_removed: bool,
    pub last_block_snapshot_head_conflict: bool,
}

impl BuildingTableProjection {
    pub fn seed_world_baseline(
        &mut self,
        build_pos: i32,
        block_id: i16,
        rotation: u8,
        team_id: u8,
        io_version: Option<u8>,
        module_bitmask: Option<u8>,
        time_scale_bits: Option<u32>,
        time_scale_duration_bits: Option<u32>,
        last_disabler_pos: Option<i32>,
        legacy_consume_connected: Option<bool>,
        health_bits: u32,
        enabled: Option<bool>,
        efficiency: Option<u8>,
        optional_efficiency: Option<u8>,
        visible_flags: Option<u64>,
    ) {
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id: Some(block_id),
                rotation: Some(rotation),
                team_id: Some(team_id),
                io_version: io_version
                    .or_else(|| previous.as_ref().and_then(|building| building.io_version)),
                module_bitmask: module_bitmask.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.module_bitmask)
                }),
                time_scale_bits: time_scale_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.time_scale_bits)
                }),
                time_scale_duration_bits: time_scale_duration_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.time_scale_duration_bits)
                }),
                last_disabler_pos: last_disabler_pos.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.last_disabler_pos)
                }),
                legacy_consume_connected: legacy_consume_connected.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.legacy_consume_connected)
                }),
                config: previous
                    .as_ref()
                    .and_then(|building| building.config.clone()),
                health_bits: Some(health_bits),
                enabled: enabled
                    .or_else(|| previous.as_ref().and_then(|building| building.enabled)),
                efficiency: efficiency
                    .or_else(|| previous.as_ref().and_then(|building| building.efficiency)),
                optional_efficiency: optional_efficiency.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.optional_efficiency)
                }),
                visible_flags: visible_flags.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.visible_flags)
                }),
                build_turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_rotation_bits),
                build_turret_plans_present: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plans_present),
                build_turret_plan_count: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plan_count),
                last_update: BuildingProjectionUpdateKind::WorldBaseline,
            },
        );
        self.last_block_snapshot_head_conflict = false;
        self.sync_last_mirror_for_apply(
            build_pos,
            BuildingProjectionUpdateKind::WorldBaseline,
            None,
            Some(health_bits),
        );
        self.recount();
    }

    pub fn apply_block_snapshot_head(
        &mut self,
        build_pos: i32,
        block_id: i16,
        rotation: Option<u8>,
        team_id: Option<u8>,
        io_version: Option<u8>,
        module_bitmask: Option<u8>,
        time_scale_bits: Option<u32>,
        time_scale_duration_bits: Option<u32>,
        last_disabler_pos: Option<i32>,
        legacy_consume_connected: Option<bool>,
        health_bits: Option<u32>,
        enabled: Option<bool>,
        efficiency: Option<u8>,
        optional_efficiency: Option<u8>,
        visible_flags: Option<u64>,
        build_turret_rotation_bits: Option<u32>,
        build_turret_plans_present: Option<bool>,
        build_turret_plan_count: Option<u16>,
    ) {
        if self.by_build_pos.get(&build_pos).is_some_and(|existing| {
            existing
                .block_id
                .is_some_and(|previous| previous != block_id)
                || match (existing.team_id, team_id) {
                    (Some(previous), Some(current)) => previous != current,
                    _ => false,
                }
                || match (existing.io_version, io_version) {
                    (Some(previous), Some(current)) => previous != current,
                    _ => false,
                }
        }) {
            self.block_snapshot_head_conflict_skip_count = self
                .block_snapshot_head_conflict_skip_count
                .saturating_add(1);
            self.last_block_snapshot_head_conflict = true;
            self.last_build_pos = Some(build_pos);
            self.last_block_id = Some(block_id);
            self.last_rotation = rotation;
            self.last_team_id = team_id;
            self.last_io_version = io_version;
            self.last_module_bitmask = module_bitmask;
            self.last_time_scale_bits = time_scale_bits;
            self.last_time_scale_duration_bits = time_scale_duration_bits;
            self.last_last_disabler_pos = last_disabler_pos;
            self.last_legacy_consume_connected = legacy_consume_connected;
            self.last_health_bits = health_bits;
            self.last_enabled = enabled;
            self.last_efficiency = efficiency;
            self.last_optional_efficiency = optional_efficiency;
            self.last_visible_flags = visible_flags;
            self.last_build_turret_rotation_bits = build_turret_rotation_bits;
            self.last_build_turret_plans_present = build_turret_plans_present;
            self.last_build_turret_plan_count = build_turret_plan_count;
            self.last_removed = false;
            return;
        }
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id: Some(block_id),
                rotation: rotation
                    .or_else(|| previous.as_ref().and_then(|building| building.rotation)),
                team_id: team_id
                    .or_else(|| previous.as_ref().and_then(|building| building.team_id)),
                io_version: io_version
                    .or_else(|| previous.as_ref().and_then(|building| building.io_version)),
                module_bitmask: module_bitmask.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.module_bitmask)
                }),
                time_scale_bits: time_scale_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.time_scale_bits)
                }),
                time_scale_duration_bits: time_scale_duration_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.time_scale_duration_bits)
                }),
                last_disabler_pos: last_disabler_pos.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.last_disabler_pos)
                }),
                legacy_consume_connected: legacy_consume_connected.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.legacy_consume_connected)
                }),
                config: previous
                    .as_ref()
                    .and_then(|building| building.config.clone()),
                health_bits: health_bits
                    .or_else(|| previous.as_ref().and_then(|building| building.health_bits)),
                enabled: enabled
                    .or_else(|| previous.as_ref().and_then(|building| building.enabled)),
                efficiency: efficiency
                    .or_else(|| previous.as_ref().and_then(|building| building.efficiency)),
                optional_efficiency: optional_efficiency.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.optional_efficiency)
                }),
                visible_flags: visible_flags.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.visible_flags)
                }),
                build_turret_rotation_bits: build_turret_rotation_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.build_turret_rotation_bits)
                }),
                build_turret_plans_present: build_turret_plans_present.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.build_turret_plans_present)
                }),
                build_turret_plan_count: build_turret_plan_count.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.build_turret_plan_count)
                }),
                last_update: BuildingProjectionUpdateKind::BlockSnapshotHead,
            },
        );
        self.last_block_snapshot_head_conflict = false;
        self.block_snapshot_head_apply_count =
            self.block_snapshot_head_apply_count.saturating_add(1);
        self.sync_last_mirror_for_apply(
            build_pos,
            BuildingProjectionUpdateKind::BlockSnapshotHead,
            None,
            None,
        );
        self.recount();
    }

    pub fn apply_construct_finish(
        &mut self,
        build_pos: i32,
        block_id: Option<i16>,
        rotation: u8,
        team_id: u8,
        config: TypeIoObject,
    ) {
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id,
                rotation: Some(rotation),
                team_id: Some(team_id),
                io_version: previous.as_ref().and_then(|building| building.io_version),
                module_bitmask: previous
                    .as_ref()
                    .and_then(|building| building.module_bitmask),
                time_scale_bits: previous
                    .as_ref()
                    .and_then(|building| building.time_scale_bits),
                time_scale_duration_bits: previous
                    .as_ref()
                    .and_then(|building| building.time_scale_duration_bits),
                last_disabler_pos: previous
                    .as_ref()
                    .and_then(|building| building.last_disabler_pos),
                legacy_consume_connected: previous
                    .as_ref()
                    .and_then(|building| building.legacy_consume_connected),
                config: Some(config.clone()),
                health_bits: previous.as_ref().and_then(|building| building.health_bits),
                enabled: previous.as_ref().and_then(|building| building.enabled),
                efficiency: previous.as_ref().and_then(|building| building.efficiency),
                optional_efficiency: previous
                    .as_ref()
                    .and_then(|building| building.optional_efficiency),
                visible_flags: previous
                    .as_ref()
                    .and_then(|building| building.visible_flags),
                build_turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_rotation_bits),
                build_turret_plans_present: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plans_present),
                build_turret_plan_count: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plan_count),
                last_update: BuildingProjectionUpdateKind::ConstructFinish,
            },
        );
        self.construct_finish_apply_count = self.construct_finish_apply_count.saturating_add(1);
        self.sync_last_mirror_for_apply(
            build_pos,
            BuildingProjectionUpdateKind::ConstructFinish,
            Some(config),
            None,
        );
        self.recount();
    }

    pub fn apply_tile_config(&mut self, build_pos: i32, config: TypeIoObject) {
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id: previous.as_ref().and_then(|building| building.block_id),
                rotation: previous.as_ref().and_then(|building| building.rotation),
                team_id: previous.as_ref().and_then(|building| building.team_id),
                io_version: previous.as_ref().and_then(|building| building.io_version),
                module_bitmask: previous
                    .as_ref()
                    .and_then(|building| building.module_bitmask),
                time_scale_bits: previous
                    .as_ref()
                    .and_then(|building| building.time_scale_bits),
                time_scale_duration_bits: previous
                    .as_ref()
                    .and_then(|building| building.time_scale_duration_bits),
                last_disabler_pos: previous
                    .as_ref()
                    .and_then(|building| building.last_disabler_pos),
                legacy_consume_connected: previous
                    .as_ref()
                    .and_then(|building| building.legacy_consume_connected),
                config: Some(config.clone()),
                health_bits: previous.as_ref().and_then(|building| building.health_bits),
                enabled: previous.as_ref().and_then(|building| building.enabled),
                efficiency: previous.as_ref().and_then(|building| building.efficiency),
                optional_efficiency: previous
                    .as_ref()
                    .and_then(|building| building.optional_efficiency),
                visible_flags: previous
                    .as_ref()
                    .and_then(|building| building.visible_flags),
                build_turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_rotation_bits),
                build_turret_plans_present: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plans_present),
                build_turret_plan_count: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plan_count),
                last_update: BuildingProjectionUpdateKind::TileConfig,
            },
        );
        self.tile_config_apply_count = self.tile_config_apply_count.saturating_add(1);
        self.sync_last_mirror_for_apply(
            build_pos,
            BuildingProjectionUpdateKind::TileConfig,
            Some(config),
            None,
        );
        self.recount();
    }

    pub fn apply_deconstruct_finish(&mut self, build_pos: i32, block_id: Option<i16>) {
        let previous = self.by_build_pos.remove(&build_pos);
        self.deconstruct_finish_apply_count = self.deconstruct_finish_apply_count.saturating_add(1);
        self.sync_last_mirror_for_removed(
            build_pos,
            BuildingProjectionUpdateKind::DeconstructFinish,
            block_id,
            previous.as_ref(),
        );
        self.recount();
    }

    pub fn apply_build_health(&mut self, build_pos: i32, health_bits: u32) {
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id: previous.as_ref().and_then(|building| building.block_id),
                rotation: previous.as_ref().and_then(|building| building.rotation),
                team_id: previous.as_ref().and_then(|building| building.team_id),
                io_version: previous.as_ref().and_then(|building| building.io_version),
                module_bitmask: previous
                    .as_ref()
                    .and_then(|building| building.module_bitmask),
                time_scale_bits: previous
                    .as_ref()
                    .and_then(|building| building.time_scale_bits),
                time_scale_duration_bits: previous
                    .as_ref()
                    .and_then(|building| building.time_scale_duration_bits),
                last_disabler_pos: previous
                    .as_ref()
                    .and_then(|building| building.last_disabler_pos),
                legacy_consume_connected: previous
                    .as_ref()
                    .and_then(|building| building.legacy_consume_connected),
                config: previous
                    .as_ref()
                    .and_then(|building| building.config.clone()),
                health_bits: Some(health_bits),
                enabled: previous.as_ref().and_then(|building| building.enabled),
                efficiency: previous.as_ref().and_then(|building| building.efficiency),
                optional_efficiency: previous
                    .as_ref()
                    .and_then(|building| building.optional_efficiency),
                visible_flags: previous
                    .as_ref()
                    .and_then(|building| building.visible_flags),
                build_turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_rotation_bits),
                build_turret_plans_present: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plans_present),
                build_turret_plan_count: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plan_count),
                last_update: BuildingProjectionUpdateKind::BuildHealthUpdate,
            },
        );
        self.build_health_apply_count = self.build_health_apply_count.saturating_add(1);
        self.sync_last_mirror_for_apply(
            build_pos,
            BuildingProjectionUpdateKind::BuildHealthUpdate,
            None,
            Some(health_bits),
        );
        self.recount();
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }

    fn sync_last_mirror_for_apply(
        &mut self,
        build_pos: i32,
        last_update: BuildingProjectionUpdateKind,
        config_override: Option<TypeIoObject>,
        health_bits_override: Option<u32>,
    ) {
        let building = self.by_build_pos.get(&build_pos);
        self.last_build_pos = Some(build_pos);
        self.last_block_id = building.and_then(|building| building.block_id);
        self.last_rotation = building.and_then(|building| building.rotation);
        self.last_team_id = building.and_then(|building| building.team_id);
        self.last_io_version = building.and_then(|building| building.io_version);
        self.last_module_bitmask = building.and_then(|building| building.module_bitmask);
        self.last_time_scale_bits = building.and_then(|building| building.time_scale_bits);
        self.last_time_scale_duration_bits =
            building.and_then(|building| building.time_scale_duration_bits);
        self.last_last_disabler_pos = building.and_then(|building| building.last_disabler_pos);
        self.last_legacy_consume_connected =
            building.and_then(|building| building.legacy_consume_connected);
        self.last_config =
            config_override.or_else(|| building.and_then(|building| building.config.clone()));
        self.last_health_bits =
            health_bits_override.or_else(|| building.and_then(|building| building.health_bits));
        self.last_enabled = building.and_then(|building| building.enabled);
        self.last_efficiency = building.and_then(|building| building.efficiency);
        self.last_optional_efficiency = building.and_then(|building| building.optional_efficiency);
        self.last_visible_flags = building.and_then(|building| building.visible_flags);
        self.last_build_turret_rotation_bits =
            building.and_then(|building| building.build_turret_rotation_bits);
        self.last_build_turret_plans_present =
            building.and_then(|building| building.build_turret_plans_present);
        self.last_build_turret_plan_count =
            building.and_then(|building| building.build_turret_plan_count);
        self.last_update = Some(last_update);
        self.last_removed = false;
    }

    fn sync_last_mirror_for_removed(
        &mut self,
        build_pos: i32,
        last_update: BuildingProjectionUpdateKind,
        block_id_override: Option<i16>,
        previous: Option<&BuildingProjection>,
    ) {
        self.last_build_pos = Some(build_pos);
        self.last_block_id =
            block_id_override.or_else(|| previous.and_then(|building| building.block_id));
        self.last_rotation = previous.and_then(|building| building.rotation);
        self.last_team_id = previous.and_then(|building| building.team_id);
        self.last_io_version = previous.and_then(|building| building.io_version);
        self.last_module_bitmask = previous.and_then(|building| building.module_bitmask);
        self.last_time_scale_bits = previous.and_then(|building| building.time_scale_bits);
        self.last_time_scale_duration_bits =
            previous.and_then(|building| building.time_scale_duration_bits);
        self.last_last_disabler_pos = previous.and_then(|building| building.last_disabler_pos);
        self.last_legacy_consume_connected =
            previous.and_then(|building| building.legacy_consume_connected);
        self.last_config = previous.and_then(|building| building.config.clone());
        self.last_health_bits = previous.and_then(|building| building.health_bits);
        self.last_enabled = previous.and_then(|building| building.enabled);
        self.last_efficiency = previous.and_then(|building| building.efficiency);
        self.last_optional_efficiency = previous.and_then(|building| building.optional_efficiency);
        self.last_visible_flags = previous.and_then(|building| building.visible_flags);
        self.last_build_turret_rotation_bits =
            previous.and_then(|building| building.build_turret_rotation_bits);
        self.last_build_turret_plans_present =
            previous.and_then(|building| building.build_turret_plans_present);
        self.last_build_turret_plan_count =
            previous.and_then(|building| building.build_turret_plan_count);
        self.last_update = Some(last_update);
        self.last_removed = true;
    }

    fn recount(&mut self) {
        self.block_known_count = self
            .by_build_pos
            .values()
            .filter(|building| building.block_id.is_some())
            .count();
        self.configured_count = self
            .by_build_pos
            .values()
            .filter(|building| building.config.is_some())
            .count();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderQueueEntryObservation {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderPlanStage {
    Queued,
    InFlight,
    Finished,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderPlanProjection {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub team_id: Option<u8>,
    pub builder_kind: Option<u8>,
    pub builder_value: Option<i32>,
    pub stage: BuilderPlanStage,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BuilderQueueProjection {
    pub active_by_tile: BTreeMap<(i32, i32), BuilderPlanProjection>,
    pub ordered_tiles: Vec<(i32, i32)>,
    pub queued_count: usize,
    pub inflight_count: usize,
    pub finished_count: u64,
    pub removed_count: u64,
    pub orphan_authoritative_count: u64,
    pub head_x: Option<i32>,
    pub head_y: Option<i32>,
    pub head_breaking: Option<bool>,
    pub head_block_id: Option<i16>,
    pub head_rotation: Option<u8>,
    pub head_stage: Option<BuilderPlanStage>,
    pub last_stage: Option<BuilderPlanStage>,
    pub last_x: Option<i32>,
    pub last_y: Option<i32>,
    pub last_breaking: Option<bool>,
    pub last_block_id: Option<i16>,
    pub last_rotation: Option<u8>,
    pub last_team_id: Option<u8>,
    pub last_builder_kind: Option<u8>,
    pub last_builder_value: Option<i32>,
    pub last_removed_local_plan: bool,
    pub last_orphan_authoritative: bool,
}

impl BuilderQueueProjection {
    pub fn sync_local_queue_entries<I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = BuilderQueueEntryObservation>,
    {
        let mut next = BTreeMap::new();
        let mut next_order = Vec::new();
        for entry in entries {
            let key = (entry.x, entry.y);
            let previous = self
                .active_by_tile
                .get(&key)
                .filter(|plan| plan.breaking == entry.breaking);
            let stage = if previous.is_some_and(|plan| plan.stage == BuilderPlanStage::InFlight) {
                BuilderPlanStage::InFlight
            } else {
                BuilderPlanStage::Queued
            };
            next.insert(
                key,
                BuilderPlanProjection {
                    x: entry.x,
                    y: entry.y,
                    breaking: entry.breaking,
                    block_id: entry
                        .block_id
                        .or_else(|| previous.and_then(|plan| plan.block_id)),
                    rotation: Some(entry.rotation),
                    team_id: previous.and_then(|plan| plan.team_id),
                    builder_kind: previous.and_then(|plan| plan.builder_kind),
                    builder_value: previous.and_then(|plan| plan.builder_value),
                    stage,
                },
            );
            next_order.retain(|tile| *tile != key);
            next_order.push(key);
        }
        self.active_by_tile = next;
        self.ordered_tiles = next_order;
        self.recount_active();
    }

    pub fn mark_begin_place(
        &mut self,
        x: i32,
        y: i32,
        block_id: Option<i16>,
        rotation: u8,
        team_id: u8,
        builder_kind: u8,
        builder_value: i32,
    ) {
        let key = (x, y);
        self.active_by_tile.insert(
            key,
            BuilderPlanProjection {
                x,
                y,
                breaking: false,
                block_id,
                rotation: Some(rotation),
                team_id: Some(team_id),
                builder_kind: Some(builder_kind),
                builder_value: Some(builder_value),
                stage: BuilderPlanStage::InFlight,
            },
        );
        self.last_stage = Some(BuilderPlanStage::InFlight);
        self.last_x = Some(x);
        self.last_y = Some(y);
        self.last_breaking = Some(false);
        self.last_block_id = block_id;
        self.last_rotation = Some(rotation);
        self.last_team_id = Some(team_id);
        self.last_builder_kind = Some(builder_kind);
        self.last_builder_value = Some(builder_value);
        self.last_removed_local_plan = false;
        self.last_orphan_authoritative = false;
        self.promote_tile_to_front(key);
        self.recount_active();
    }

    pub fn mark_begin_break(
        &mut self,
        x: i32,
        y: i32,
        team_id: u8,
        builder_kind: u8,
        builder_value: i32,
    ) {
        let key = (x, y);
        let previous = self.active_by_tile.get(&key).filter(|plan| plan.breaking);
        let previous_block_id = previous.and_then(|plan| plan.block_id);
        let previous_rotation = previous.and_then(|plan| plan.rotation);
        self.active_by_tile.insert(
            key,
            BuilderPlanProjection {
                x,
                y,
                breaking: true,
                block_id: previous_block_id,
                rotation: previous_rotation,
                team_id: Some(team_id),
                builder_kind: Some(builder_kind),
                builder_value: Some(builder_value),
                stage: BuilderPlanStage::InFlight,
            },
        );
        self.last_stage = Some(BuilderPlanStage::InFlight);
        self.last_x = Some(x);
        self.last_y = Some(y);
        self.last_breaking = Some(true);
        self.last_block_id = previous_block_id;
        self.last_rotation = previous_rotation;
        self.last_team_id = Some(team_id);
        self.last_builder_kind = Some(builder_kind);
        self.last_builder_value = Some(builder_value);
        self.last_removed_local_plan = false;
        self.last_orphan_authoritative = false;
        self.promote_tile_to_front(key);
        self.recount_active();
    }

    pub fn mark_remove_queue_block(
        &mut self,
        x: i32,
        y: i32,
        breaking: bool,
        removed_local_plan: bool,
    ) {
        let previous = self.remove_matching_plan(x, y, breaking);
        let orphan_authoritative = previous.is_none() && !removed_local_plan;
        if orphan_authoritative {
            self.orphan_authoritative_count = self.orphan_authoritative_count.saturating_add(1);
        }
        self.removed_count = self.removed_count.saturating_add(1);
        self.last_stage = Some(BuilderPlanStage::Removed);
        self.last_x = Some(x);
        self.last_y = Some(y);
        self.last_breaking = Some(breaking);
        self.last_block_id = previous.as_ref().and_then(|plan| plan.block_id);
        self.last_rotation = previous.as_ref().and_then(|plan| plan.rotation);
        self.last_team_id = previous.as_ref().and_then(|plan| plan.team_id);
        self.last_builder_kind = previous.as_ref().and_then(|plan| plan.builder_kind);
        self.last_builder_value = previous.as_ref().and_then(|plan| plan.builder_value);
        self.last_removed_local_plan = removed_local_plan;
        self.last_orphan_authoritative = orphan_authoritative;
        self.recount_active();
    }

    pub fn mark_construct_finish(
        &mut self,
        x: i32,
        y: i32,
        block_id: Option<i16>,
        rotation: u8,
        team_id: u8,
        builder_kind: u8,
        builder_value: i32,
        removed_local_plan: bool,
    ) {
        let previous = self.remove_matching_plan(x, y, false);
        let orphan_authoritative = previous.is_none() && !removed_local_plan;
        if orphan_authoritative {
            self.orphan_authoritative_count = self.orphan_authoritative_count.saturating_add(1);
        }
        self.finished_count = self.finished_count.saturating_add(1);
        self.last_stage = Some(BuilderPlanStage::Finished);
        self.last_x = Some(x);
        self.last_y = Some(y);
        self.last_breaking = Some(false);
        self.last_block_id = block_id.or_else(|| previous.as_ref().and_then(|plan| plan.block_id));
        self.last_rotation = Some(rotation);
        self.last_team_id = Some(team_id);
        self.last_builder_kind = Some(builder_kind);
        self.last_builder_value = Some(builder_value);
        self.last_removed_local_plan = removed_local_plan;
        self.last_orphan_authoritative = orphan_authoritative;
        self.recount_active();
    }

    pub fn mark_deconstruct_finish(
        &mut self,
        x: i32,
        y: i32,
        block_id: Option<i16>,
        builder_kind: u8,
        builder_value: i32,
        removed_local_plan: bool,
    ) {
        let previous = self.remove_matching_plan(x, y, true);
        let orphan_authoritative = previous.is_none() && !removed_local_plan;
        if orphan_authoritative {
            self.orphan_authoritative_count = self.orphan_authoritative_count.saturating_add(1);
        }
        self.finished_count = self.finished_count.saturating_add(1);
        self.last_stage = Some(BuilderPlanStage::Finished);
        self.last_x = Some(x);
        self.last_y = Some(y);
        self.last_breaking = Some(true);
        self.last_block_id = block_id.or_else(|| previous.as_ref().and_then(|plan| plan.block_id));
        self.last_rotation = previous.as_ref().and_then(|plan| plan.rotation);
        self.last_team_id = previous.as_ref().and_then(|plan| plan.team_id);
        self.last_builder_kind = Some(builder_kind);
        self.last_builder_value = Some(builder_value);
        self.last_removed_local_plan = removed_local_plan;
        self.last_orphan_authoritative = orphan_authoritative;
        self.recount_active();
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }

    fn remove_matching_plan(
        &mut self,
        x: i32,
        y: i32,
        breaking: bool,
    ) -> Option<BuilderPlanProjection> {
        let key = (x, y);
        if self
            .active_by_tile
            .get(&key)
            .is_some_and(|plan| plan.breaking == breaking)
        {
            self.remove_tile_from_order(key);
            self.active_by_tile.remove(&key)
        } else {
            None
        }
    }

    fn promote_tile_to_front(&mut self, key: (i32, i32)) {
        self.remove_tile_from_order(key);
        self.ordered_tiles.insert(0, key);
    }

    fn remove_tile_from_order(&mut self, key: (i32, i32)) {
        self.ordered_tiles.retain(|tile| *tile != key);
    }

    fn refresh_head_projection(&mut self) {
        self.ordered_tiles
            .retain(|tile| self.active_by_tile.contains_key(tile));
        let head = self
            .ordered_tiles
            .first()
            .and_then(|tile| self.active_by_tile.get(tile));
        self.head_x = head.map(|plan| plan.x);
        self.head_y = head.map(|plan| plan.y);
        self.head_breaking = head.map(|plan| plan.breaking);
        self.head_block_id = head.and_then(|plan| plan.block_id);
        self.head_rotation = head.and_then(|plan| plan.rotation);
        self.head_stage = head.map(|plan| plan.stage);
    }

    fn recount_active(&mut self) {
        self.queued_count = self
            .active_by_tile
            .values()
            .filter(|plan| plan.stage == BuilderPlanStage::Queued)
            .count();
        self.inflight_count = self
            .active_by_tile
            .values()
            .filter(|plan| plan.stage == BuilderPlanStage::InFlight)
            .count();
        self.refresh_head_projection();
    }
}

#[cfg(test)]
mod builder_queue_projection_tests {
    use super::{BuilderPlanStage, BuilderQueueEntryObservation, BuilderQueueProjection};

    #[test]
    fn sync_local_queue_entries_dedupes_by_tile_with_tail_wins() {
        let mut projection = BuilderQueueProjection::default();

        projection.sync_local_queue_entries([
            BuilderQueueEntryObservation {
                x: 12,
                y: 34,
                breaking: false,
                block_id: Some(5),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 12,
                y: 34,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        assert_eq!(projection.active_by_tile.len(), 1);
        assert_eq!(projection.ordered_tiles, vec![(12, 34)]);
        assert_eq!(projection.queued_count, 1);
        assert_eq!(projection.inflight_count, 0);
        assert_eq!(projection.head_x, Some(12));
        assert_eq!(projection.head_y, Some(34));
        assert_eq!(projection.head_breaking, Some(true));
        assert_eq!(projection.head_stage, Some(BuilderPlanStage::Queued));
        assert_eq!(
            projection
                .active_by_tile
                .get(&(12, 34))
                .map(|plan| plan.breaking),
            Some(true)
        );
        assert_eq!(
            projection
                .active_by_tile
                .get(&(12, 34))
                .map(|plan| plan.stage),
            Some(BuilderPlanStage::Queued)
        );
    }

    #[test]
    fn begin_break_replaces_existing_place_on_same_tile() {
        let mut projection = BuilderQueueProjection::default();
        projection.mark_begin_place(7, 8, Some(9), 2, 3, 4, 55);

        projection.mark_begin_break(7, 8, 5, 6, 77);

        assert_eq!(projection.active_by_tile.len(), 1);
        assert_eq!(projection.ordered_tiles, vec![(7, 8)]);
        assert_eq!(projection.queued_count, 0);
        assert_eq!(projection.inflight_count, 1);
        assert_eq!(projection.head_x, Some(7));
        assert_eq!(projection.head_y, Some(8));
        assert_eq!(projection.head_breaking, Some(true));
        assert_eq!(projection.head_stage, Some(BuilderPlanStage::InFlight));
        assert_eq!(
            projection
                .active_by_tile
                .get(&(7, 8))
                .map(|plan| plan.breaking),
            Some(true)
        );
        assert_eq!(
            projection
                .active_by_tile
                .get(&(7, 8))
                .and_then(|plan| plan.team_id),
            Some(5)
        );
        assert_eq!(
            projection
                .active_by_tile
                .get(&(7, 8))
                .and_then(|plan| plan.block_id),
            None
        );
    }

    #[test]
    fn remove_queue_block_keeps_opposite_plan_on_same_tile() {
        let mut projection = BuilderQueueProjection::default();
        projection.mark_begin_place(20, 21, Some(22), 1, 2, 3, 44);

        projection.mark_remove_queue_block(20, 21, true, false);

        assert_eq!(projection.active_by_tile.len(), 1);
        assert_eq!(projection.ordered_tiles, vec![(20, 21)]);
        assert_eq!(projection.inflight_count, 1);
        assert_eq!(projection.removed_count, 1);
        assert_eq!(projection.orphan_authoritative_count, 1);
        assert_eq!(projection.head_x, Some(20));
        assert_eq!(projection.head_y, Some(21));
        assert_eq!(projection.head_breaking, Some(false));
        assert_eq!(
            projection
                .active_by_tile
                .get(&(20, 21))
                .map(|plan| plan.breaking),
            Some(false)
        );
        assert!(projection.last_orphan_authoritative);
    }

    #[test]
    fn sync_local_queue_entries_keeps_tail_wins_queue_order_across_tiles() {
        let mut projection = BuilderQueueProjection::default();

        projection.sync_local_queue_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(20),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        assert_eq!(projection.ordered_tiles, vec![(2, 2), (1, 1)]);
        assert_eq!(projection.head_x, Some(2));
        assert_eq!(projection.head_y, Some(2));
        assert_eq!(projection.head_breaking, Some(false));
        assert_eq!(projection.head_block_id, Some(20));
    }

    #[test]
    fn begin_place_promotes_existing_tile_to_queue_head() {
        let mut projection = BuilderQueueProjection::default();
        projection.sync_local_queue_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(20),
                rotation: 1,
            },
        ]);

        projection.mark_begin_place(2, 2, Some(20), 1, 3, 4, 5);

        assert_eq!(projection.ordered_tiles, vec![(2, 2), (1, 1)]);
        assert_eq!(projection.head_x, Some(2));
        assert_eq!(projection.head_y, Some(2));
        assert_eq!(projection.head_breaking, Some(false));
        assert_eq!(projection.head_stage, Some(BuilderPlanStage::InFlight));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityProjection {
    pub class_id: u8,
    pub hidden: bool,
    pub is_local_player: bool,
    pub unit_kind: u8,
    pub unit_value: u32,
    pub x_bits: u32,
    pub y_bits: u32,
    pub last_seen_entity_snapshot_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitySemanticProjection {
    Unit(EntityUnitSemanticProjection),
    Fire(EntityFireSemanticProjection),
    Puddle(EntityPuddleSemanticProjection),
    WeatherState(EntityWeatherStateSemanticProjection),
    WorldLabel(EntityWorldLabelSemanticProjection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUnitSemanticProjection {
    pub team_id: u8,
    pub unit_type_id: i16,
    pub health_bits: u32,
    pub rotation_bits: u32,
    pub shield_bits: u32,
    pub mine_tile_pos: i32,
    pub status_count: i32,
    pub payload_count: Option<i32>,
    pub building_pos: Option<i32>,
    pub lifetime_bits: Option<u32>,
    pub time_bits: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFireSemanticProjection {
    pub tile_pos: i32,
    pub lifetime_bits: u32,
    pub time_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityPuddleSemanticProjection {
    pub tile_pos: i32,
    pub liquid_id: i16,
    pub amount_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityWeatherStateSemanticProjection {
    pub weather_id: i16,
    pub intensity_bits: u32,
    pub life_bits: u32,
    pub opacity_bits: u32,
    pub wind_x_bits: u32,
    pub wind_y_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityWorldLabelSemanticProjection {
    pub flags: u8,
    pub font_size_bits: u32,
    pub text: Option<String>,
    pub z_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySemanticProjectionEntry {
    pub class_id: u8,
    pub last_seen_entity_snapshot_count: u64,
    pub projection: EntitySemanticProjection,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntitySemanticProjectionTable {
    pub by_entity_id: BTreeMap<i32, EntitySemanticProjectionEntry>,
}

impl EntitySemanticProjectionTable {
    pub fn upsert(
        &mut self,
        entity_id: i32,
        class_id: u8,
        last_seen_entity_snapshot_count: u64,
        projection: EntitySemanticProjection,
    ) {
        self.by_entity_id.insert(
            entity_id,
            EntitySemanticProjectionEntry {
                class_id,
                last_seen_entity_snapshot_count,
                projection,
            },
        );
    }

    pub fn remove_entity(&mut self, entity_id: i32) -> bool {
        self.by_entity_id.remove(&entity_id).is_some()
    }

    pub fn remove_entities<'a>(&mut self, entity_ids: impl IntoIterator<Item = &'a i32>) {
        for entity_id in entity_ids {
            self.by_entity_id.remove(entity_id);
        }
    }

    pub fn remove_hidden_entities(
        &mut self,
        hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) -> Vec<i32> {
        let removed_ids = self
            .by_entity_id
            .keys()
            .copied()
            .filter(|&entity_id| {
                hidden_lifecycle_matches_hidden_non_local_entity_id(
                    hidden_ids,
                    local_player_entity_id,
                    entity_id,
                )
            })
            .collect::<Vec<_>>();
        for entity_id in &removed_ids {
            self.by_entity_id.remove(entity_id);
        }
        removed_ids
    }

    pub fn clear_for_world_reload(&mut self) {
        self.by_entity_id.clear();
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityTableProjection {
    pub by_entity_id: BTreeMap<i32, EntityProjection>,
    pub local_player_entity_id: Option<i32>,
    pub hidden_count: usize,
    pub applied_local_player_count: u64,
    pub hidden_apply_count: u64,
}

impl EntityTableProjection {
    pub const LOCAL_PLAYER_CLASS_ID: u8 = 12;

    pub fn upsert_entity(
        &mut self,
        entity_id: i32,
        class_id: u8,
        is_local_player: bool,
        unit_kind: u8,
        unit_value: u32,
        x_bits: u32,
        y_bits: u32,
        hidden: bool,
        last_seen_entity_snapshot_count: u64,
    ) {
        let is_local_player = is_local_player
            || self
                .by_entity_id
                .get(&entity_id)
                .is_some_and(|entity| entity.is_local_player);
        self.by_entity_id.insert(
            entity_id,
            EntityProjection {
                class_id,
                hidden,
                is_local_player,
                unit_kind,
                unit_value,
                x_bits,
                y_bits,
                last_seen_entity_snapshot_count,
            },
        );
        if is_local_player {
            self.local_player_entity_id = Some(entity_id);
        }
        self.recount_hidden();
    }

    pub fn upsert_player_entity(
        &mut self,
        entity_id: i32,
        is_local_player: bool,
        unit_kind: u8,
        unit_value: u32,
        x_bits: u32,
        y_bits: u32,
        hidden: bool,
        last_seen_entity_snapshot_count: u64,
    ) {
        self.upsert_entity(
            entity_id,
            Self::LOCAL_PLAYER_CLASS_ID,
            is_local_player,
            unit_kind,
            unit_value,
            x_bits,
            y_bits,
            hidden,
            last_seen_entity_snapshot_count,
        );
    }

    pub fn upsert_local_player(
        &mut self,
        entity_id: i32,
        unit_kind: u8,
        unit_value: u32,
        x_bits: u32,
        y_bits: u32,
        hidden: bool,
        last_seen_entity_snapshot_count: u64,
    ) {
        self.upsert_entity(
            entity_id,
            Self::LOCAL_PLAYER_CLASS_ID,
            true,
            unit_kind,
            unit_value,
            x_bits,
            y_bits,
            hidden,
            last_seen_entity_snapshot_count,
        );
        self.applied_local_player_count = self.applied_local_player_count.saturating_add(1);
    }

    pub fn upsert_bootstrap_local_player(
        &mut self,
        entity_id: i32,
        unit_kind: u8,
        unit_value: u32,
        x_bits: u32,
        y_bits: u32,
        hidden: bool,
    ) {
        self.upsert_entity(
            entity_id,
            Self::LOCAL_PLAYER_CLASS_ID,
            true,
            unit_kind,
            unit_value,
            x_bits,
            y_bits,
            hidden,
            0,
        );
    }

    pub fn update_local_player_position(
        &mut self,
        entity_id: i32,
        x_bits: u32,
        y_bits: u32,
        hidden: bool,
    ) {
        let existing = self.by_entity_id.get(&entity_id).cloned();
        self.by_entity_id.insert(
            entity_id,
            EntityProjection {
                class_id: Self::LOCAL_PLAYER_CLASS_ID,
                hidden,
                is_local_player: true,
                unit_kind: existing.as_ref().map_or(0, |entity| entity.unit_kind),
                unit_value: existing.as_ref().map_or(0, |entity| entity.unit_value),
                x_bits,
                y_bits,
                last_seen_entity_snapshot_count: existing
                    .as_ref()
                    .map_or(0, |entity| entity.last_seen_entity_snapshot_count),
            },
        );
        self.local_player_entity_id = Some(entity_id);
        self.recount_hidden();
    }

    pub fn apply_hidden_ids(&mut self, hidden_ids: &BTreeSet<i32>) {
        for entity_id in hidden_ids {
            if let Some(entity) = self.by_entity_id.get_mut(entity_id) {
                entity.hidden = true;
            }
        }
        self.hidden_apply_count = self.hidden_apply_count.saturating_add(1);
        self.recount_hidden();
    }

    pub fn remove_hidden_entities(&mut self, hidden_ids: &BTreeSet<i32>) -> Vec<i32> {
        let removed_ids = self
            .by_entity_id
            .iter()
            .filter_map(|(&entity_id, entity)| {
                if !entity.is_local_player && hidden_ids.contains(&entity_id) {
                    Some(entity_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for entity_id in &removed_ids {
            self.by_entity_id.remove(entity_id);
        }
        self.recount_hidden();
        removed_ids
    }

    pub fn remove_entity(&mut self, entity_id: i32) -> bool {
        let removed = self.by_entity_id.remove(&entity_id).is_some();
        if self.local_player_entity_id == Some(entity_id) {
            self.local_player_entity_id = None;
        }
        self.recount_hidden();
        removed
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }

    fn recount_hidden(&mut self) {
        self.hidden_count = self
            .by_entity_id
            .values()
            .filter(|entity| entity.hidden)
            .count();
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SessionState {
    pub session_id: Option<u64>,
    pub last_applied_tick: u64,
    pub connect_packet_sent: bool,
    pub connect_payload_len: usize,
    pub connect_packet_len: usize,
    pub client_loaded: bool,
    pub connect_confirm_sent: bool,
    pub last_connect_confirm_at_ms: Option<u64>,
    pub bootstrap_stream_id: Option<i32>,
    pub world_stream_expected_len: usize,
    pub world_stream_received_len: usize,
    pub world_stream_loaded: bool,
    pub world_stream_compressed_len: usize,
    pub world_stream_inflated_len: usize,
    pub world_map_width: usize,
    pub world_map_height: usize,
    pub world_player_id: Option<i32>,
    pub world_player_unit_kind: Option<u8>,
    pub world_player_unit_value: Option<u32>,
    pub world_player_x_bits: Option<u32>,
    pub world_player_y_bits: Option<u32>,
    pub last_camera_x_bits: Option<u32>,
    pub last_camera_y_bits: Option<u32>,
    pub world_display_title: Option<String>,
    pub world_bootstrap_projection: Option<WorldBootstrapProjection>,
    pub ready_to_enter_world: bool,
    pub deferred_inbound_packet_count: u64,
    pub replayed_inbound_packet_count: u64,
    pub dropped_loading_low_priority_packet_count: u64,
    pub dropped_loading_deferred_overflow_count: u64,
    pub last_deferred_packet_id: Option<u8>,
    pub last_deferred_packet_method: Option<String>,
    pub last_replayed_packet_id: Option<u8>,
    pub last_replayed_packet_method: Option<String>,
    pub last_dropped_loading_packet_id: Option<u8>,
    pub last_dropped_loading_packet_method: Option<String>,
    pub last_dropped_loading_deferred_overflow_packet_id: Option<u8>,
    pub last_dropped_loading_deferred_overflow_packet_method: Option<String>,
    pub received_connect_redirect_count: u64,
    pub last_connect_redirect_ip: Option<String>,
    pub last_connect_redirect_port: Option<i32>,
    pub last_inbound_at_ms: Option<u64>,
    pub last_ready_inbound_liveness_anchor_at_ms: Option<u64>,
    pub ready_inbound_liveness_anchor_count: u64,
    pub last_outbound_at_ms: Option<u64>,
    pub last_keepalive_at_ms: Option<u64>,
    pub last_client_snapshot_at_ms: Option<u64>,
    pub sent_keepalive_count: u64,
    pub sent_client_snapshot_count: u64,
    pub last_sent_client_snapshot_id: Option<i32>,
    pub connection_timed_out: bool,
    pub timeout_count: u64,
    pub connect_or_loading_timeout_count: u64,
    pub ready_snapshot_timeout_count: u64,
    pub last_timeout: Option<SessionTimeoutProjection>,
    pub reset_count: u64,
    pub reconnect_reset_count: u64,
    pub world_reload_count: u64,
    pub kick_reset_count: u64,
    pub last_reset_kind: Option<SessionResetKind>,
    pub last_world_reload: Option<WorldReloadProjection>,
    pub reconnect_projection: ReconnectProjection,
    pub received_snapshot_count: u64,
    pub last_snapshot_packet_id: Option<u8>,
    pub last_snapshot_method: Option<HighFrequencyRemoteMethod>,
    pub last_snapshot_payload_len: usize,
    pub applied_state_snapshot_count: u64,
    pub last_state_snapshot: Option<AppliedStateSnapshot>,
    pub last_state_snapshot_core_data: Option<AppliedStateSnapshotCoreData>,
    pub last_good_state_snapshot_core_data: Option<AppliedStateSnapshotCoreData>,
    pub last_state_snapshot_core_data_duplicate_team_count: usize,
    pub last_state_snapshot_core_data_duplicate_item_count: usize,
    pub state_snapshot_core_data_duplicate_team_count_total: u64,
    pub state_snapshot_core_data_duplicate_item_count_total: u64,
    pub authoritative_state_mirror: Option<AuthoritativeStateMirror>,
    pub state_snapshot_authority_projection: Option<StateSnapshotAuthorityProjection>,
    pub state_snapshot_business_projection: Option<StateSnapshotBusinessProjection>,
    pub failed_state_snapshot_core_data_parse_count: u64,
    pub last_state_snapshot_core_data_parse_error: Option<String>,
    pub last_state_snapshot_core_data_parse_error_payload_len: Option<usize>,
    pub failed_state_snapshot_parse_count: u64,
    pub last_state_snapshot_parse_error: Option<String>,
    pub last_state_snapshot_parse_error_payload_len: Option<usize>,
    pub received_server_message_count: u64,
    pub last_server_message: Option<String>,
    pub received_chat_message_count: u64,
    pub last_chat_message: Option<String>,
    pub last_chat_unformatted: Option<String>,
    pub last_chat_sender_entity_id: Option<i32>,
    pub received_sound_count: u64,
    pub last_sound_id: Option<i16>,
    pub last_sound_volume_bits: Option<u32>,
    pub last_sound_pitch_bits: Option<u32>,
    pub last_sound_pan_bits: Option<u32>,
    pub failed_sound_parse_count: u64,
    pub last_sound_parse_error_payload_len: Option<usize>,
    pub received_sound_at_count: u64,
    pub last_sound_at_id: Option<i16>,
    pub last_sound_at_x_bits: Option<u32>,
    pub last_sound_at_y_bits: Option<u32>,
    pub last_sound_at_volume_bits: Option<u32>,
    pub last_sound_at_pitch_bits: Option<u32>,
    pub failed_sound_at_parse_count: u64,
    pub last_sound_at_parse_error_payload_len: Option<usize>,
    pub received_take_items_count: u64,
    pub last_take_items: Option<TakeItemsProjection>,
    pub received_transfer_item_to_count: u64,
    pub last_transfer_item_to: Option<TransferItemToProjection>,
    pub received_transfer_item_to_unit_count: u64,
    pub last_transfer_item_to_unit: Option<TransferItemToUnitProjection>,
    pub received_transfer_item_effect_count: u64,
    pub last_transfer_item_effect: Option<TransferItemEffectProjection>,
    pub resource_delta_projection: ResourceDeltaProjection,
    pub received_payload_dropped_count: u64,
    pub last_payload_dropped: Option<PayloadDroppedProjection>,
    pub received_picked_build_payload_count: u64,
    pub last_picked_build_payload: Option<PickedBuildPayloadProjection>,
    pub received_picked_unit_payload_count: u64,
    pub last_picked_unit_payload: Option<PickedUnitPayloadProjection>,
    pub received_destroy_payload_count: u64,
    pub last_destroy_payload: Option<DestroyPayloadProjection>,
    pub payload_lifecycle_projection: PayloadLifecycleProjection,
    pub received_unit_entered_payload_count: u64,
    pub last_unit_entered_payload: Option<UnitEnteredPayloadProjection>,
    pub received_build_destroyed_count: u64,
    pub last_build_destroyed_build_pos: Option<i32>,
    pub received_unit_despawn_count: u64,
    pub last_unit_despawn: Option<UnitRefProjection>,
    pub received_unit_death_count: u64,
    pub last_unit_death_id: Option<i32>,
    pub received_unit_destroy_count: u64,
    pub last_unit_destroy_id: Option<i32>,
    pub received_unit_env_death_count: u64,
    pub last_unit_env_death: Option<UnitRefProjection>,
    pub received_unit_safe_death_count: u64,
    pub last_unit_safe_death: Option<UnitRefProjection>,
    pub received_unit_cap_death_count: u64,
    pub last_unit_cap_death: Option<UnitRefProjection>,
    pub received_create_weather_count: u64,
    pub last_create_weather_id: Option<i16>,
    pub last_create_weather_intensity_bits: Option<u32>,
    pub last_create_weather_duration_bits: Option<u32>,
    pub last_create_weather_wind_x_bits: Option<u32>,
    pub last_create_weather_wind_y_bits: Option<u32>,
    pub received_create_bullet_count: u64,
    pub last_create_bullet: Option<CreateBulletProjection>,
    pub received_spawn_effect_count: u64,
    pub last_spawn_effect_x_bits: Option<u32>,
    pub last_spawn_effect_y_bits: Option<u32>,
    pub last_spawn_effect_rotation_bits: Option<u32>,
    pub last_spawn_effect_unit_type_id: Option<i16>,
    pub received_logic_explosion_count: u64,
    pub last_logic_explosion_team_id: Option<u8>,
    pub last_logic_explosion_x_bits: Option<u32>,
    pub last_logic_explosion_y_bits: Option<u32>,
    pub last_logic_explosion_radius_bits: Option<u32>,
    pub last_logic_explosion_damage_bits: Option<u32>,
    pub last_logic_explosion_air: Option<bool>,
    pub last_logic_explosion_ground: Option<bool>,
    pub last_logic_explosion_pierce: Option<bool>,
    pub last_logic_explosion_effect: Option<bool>,
    pub received_auto_door_toggle_count: u64,
    pub last_auto_door_toggle_tile_pos: Option<i32>,
    pub last_auto_door_toggle_open: Option<bool>,
    pub received_landing_pad_landed_count: u64,
    pub last_landing_pad_landed_tile_pos: Option<i32>,
    pub received_assembler_drone_spawned_count: u64,
    pub last_assembler_drone_spawned_tile_pos: Option<i32>,
    pub last_assembler_drone_spawned_unit_id: Option<i32>,
    pub received_assembler_unit_spawned_count: u64,
    pub last_assembler_unit_spawned_tile_pos: Option<i32>,
    pub received_unit_spawn_count: u64,
    pub last_unit_spawn_id: Option<i32>,
    pub last_unit_spawn_class_id: Option<u8>,
    pub last_unit_spawn_payload_len: Option<usize>,
    pub last_unit_spawn_consumed_bytes: Option<usize>,
    pub last_unit_spawn_trailing_bytes: Option<usize>,
    pub received_unit_block_spawn_count: u64,
    pub last_unit_block_spawn_tile_pos: Option<i32>,
    pub received_unit_tether_block_spawned_count: u64,
    pub last_unit_tether_block_spawned_tile_pos: Option<i32>,
    pub last_unit_tether_block_spawned_id: Option<i32>,
    pub received_effect_count: u64,
    pub last_effect_id: Option<i16>,
    pub last_effect_x_bits: Option<u32>,
    pub last_effect_y_bits: Option<u32>,
    pub last_effect_rotation_bits: Option<u32>,
    pub last_effect_color_rgba: Option<u32>,
    pub last_effect_data_len: Option<usize>,
    pub last_effect_data_type_tag: Option<u8>,
    pub last_effect_data_kind: Option<String>,
    pub last_effect_contract_name: Option<String>,
    pub last_effect_data_consumed_len: Option<usize>,
    pub last_effect_data_object: Option<TypeIoObject>,
    pub last_effect_data_semantic: Option<EffectDataSemantic>,
    pub last_effect_business_projection: Option<EffectBusinessProjection>,
    pub last_effect_business_path: Option<Vec<usize>>,
    pub last_effect_data_parse_failed: bool,
    pub failed_effect_data_parse_count: u64,
    pub last_effect_data_parse_error: Option<String>,
    pub received_effect_reliable_count: u64,
    pub last_effect_reliable_id: Option<i16>,
    pub last_effect_reliable_contract_name: Option<String>,
    pub last_effect_reliable_x_bits: Option<u32>,
    pub last_effect_reliable_y_bits: Option<u32>,
    pub last_effect_reliable_rotation_bits: Option<u32>,
    pub last_effect_reliable_color_rgba: Option<u32>,
    pub received_trace_info_count: u64,
    pub last_trace_info_player_id: Option<i32>,
    pub last_trace_info_ip: Option<String>,
    pub last_trace_info_uuid: Option<String>,
    pub last_trace_info_locale: Option<String>,
    pub last_trace_info_modded: Option<bool>,
    pub last_trace_info_mobile: Option<bool>,
    pub last_trace_info_times_joined: Option<i32>,
    pub last_trace_info_times_kicked: Option<i32>,
    pub last_trace_info_ips: Option<Vec<String>>,
    pub last_trace_info_names: Option<Vec<String>>,
    pub failed_trace_info_parse_count: u64,
    pub last_trace_info_parse_error_payload_len: Option<usize>,
    pub received_debug_status_client_count: u64,
    pub received_debug_status_client_unreliable_count: u64,
    pub last_debug_status_reliable: Option<bool>,
    pub last_debug_status_value: Option<i32>,
    pub last_debug_status_last_client_snapshot: Option<i32>,
    pub last_debug_status_snapshots_sent: Option<i32>,
    pub failed_debug_status_client_parse_count: u64,
    pub last_debug_status_client_parse_error_payload_len: Option<usize>,
    pub failed_debug_status_client_unreliable_parse_count: u64,
    pub last_debug_status_client_unreliable_parse_error_payload_len: Option<usize>,
    pub received_client_packet_reliable_count: u64,
    pub received_client_packet_unreliable_count: u64,
    pub last_client_packet_reliable_type: Option<String>,
    pub last_client_packet_reliable_contents: Option<String>,
    pub last_client_packet_unreliable_type: Option<String>,
    pub last_client_packet_unreliable_contents: Option<String>,
    pub received_server_packet_reliable_count: u64,
    pub received_server_packet_unreliable_count: u64,
    pub last_server_packet_reliable_type: Option<String>,
    pub last_server_packet_reliable_contents: Option<String>,
    pub last_server_packet_unreliable_type: Option<String>,
    pub last_server_packet_unreliable_contents: Option<String>,
    pub received_client_binary_packet_reliable_count: u64,
    pub received_client_binary_packet_unreliable_count: u64,
    pub last_client_binary_packet_reliable_type: Option<String>,
    pub last_client_binary_packet_reliable_contents: Option<Vec<u8>>,
    pub last_client_binary_packet_unreliable_type: Option<String>,
    pub last_client_binary_packet_unreliable_contents: Option<Vec<u8>>,
    pub received_server_binary_packet_reliable_count: u64,
    pub received_server_binary_packet_unreliable_count: u64,
    pub last_server_binary_packet_reliable_type: Option<String>,
    pub last_server_binary_packet_reliable_contents: Option<Vec<u8>>,
    pub last_server_binary_packet_unreliable_type: Option<String>,
    pub last_server_binary_packet_unreliable_contents: Option<Vec<u8>>,
    pub received_client_logic_data_reliable_count: u64,
    pub received_client_logic_data_unreliable_count: u64,
    pub last_client_logic_data_reliable_channel: Option<String>,
    pub last_client_logic_data_reliable_value: Option<TypeIoObject>,
    pub last_client_logic_data_unreliable_channel: Option<String>,
    pub last_client_logic_data_unreliable_value: Option<TypeIoObject>,
    pub received_set_camera_position_count: u64,
    pub received_set_rules_count: u64,
    pub last_set_rules_json_data: Option<String>,
    pub failed_set_rules_parse_count: u64,
    pub last_set_rules_parse_error: Option<String>,
    pub last_set_rules_parse_error_payload_len: Option<usize>,
    pub rules_projection: RulesProjection,
    pub received_set_objectives_count: u64,
    pub last_set_objectives_json_data: Option<String>,
    pub failed_set_objectives_parse_count: u64,
    pub last_set_objectives_parse_error: Option<String>,
    pub last_set_objectives_parse_error_payload_len: Option<usize>,
    pub objectives_projection: ObjectivesProjection,
    pub received_set_rule_count: u64,
    pub last_set_rule_name: Option<String>,
    pub last_set_rule_json_data: Option<String>,
    pub failed_set_rule_parse_count: u64,
    pub last_set_rule_parse_error: Option<String>,
    pub last_set_rule_parse_error_payload_len: Option<usize>,
    pub received_clear_objectives_count: u64,
    pub received_complete_objective_count: u64,
    pub last_complete_objective_index: Option<i32>,
    pub received_set_hud_text_count: u64,
    pub last_set_hud_text_message: Option<String>,
    pub received_set_hud_text_reliable_count: u64,
    pub last_set_hud_text_reliable_message: Option<String>,
    pub received_hide_hud_text_count: u64,
    pub received_announce_count: u64,
    pub last_announce_message: Option<String>,
    pub received_set_flag_count: u64,
    pub last_set_flag: Option<String>,
    pub last_set_flag_add: Option<bool>,
    pub received_game_over_count: u64,
    pub last_game_over_winner_team_id: Option<u8>,
    pub received_update_game_over_count: u64,
    pub last_update_game_over_winner_team_id: Option<u8>,
    pub received_sector_capture_count: u64,
    pub received_researched_count: u64,
    pub last_researched_content_type: Option<u8>,
    pub last_researched_content_id: Option<i16>,
    pub received_world_label_count: u64,
    pub received_world_label_reliable_count: u64,
    pub last_world_label_reliable: Option<bool>,
    pub last_world_label_id: Option<i32>,
    pub last_world_label_message: Option<String>,
    pub last_world_label_duration_bits: Option<u32>,
    pub last_world_label_world_x_bits: Option<u32>,
    pub last_world_label_world_y_bits: Option<u32>,
    pub received_remove_world_label_count: u64,
    pub last_remove_world_label_id: Option<i32>,
    pub received_create_marker_count: u64,
    pub received_remove_marker_count: u64,
    pub received_update_marker_count: u64,
    pub received_update_marker_text_count: u64,
    pub received_update_marker_texture_count: u64,
    pub failed_marker_decode_count: u64,
    pub last_failed_marker_method: Option<String>,
    pub last_failed_marker_payload_len: Option<usize>,
    pub last_marker_id: Option<i32>,
    pub last_marker_json_len: Option<usize>,
    pub last_marker_control: Option<u8>,
    pub last_marker_control_name: Option<String>,
    pub last_marker_p1_bits: Option<u64>,
    pub last_marker_p2_bits: Option<u64>,
    pub last_marker_p3_bits: Option<u64>,
    pub last_marker_fetch: Option<bool>,
    pub last_marker_text: Option<String>,
    pub last_marker_texture_kind: Option<u8>,
    pub last_marker_texture_kind_name: Option<String>,
    pub received_info_message_count: u64,
    pub last_info_message: Option<String>,
    pub received_info_popup_count: u64,
    pub received_info_popup_reliable_count: u64,
    pub last_info_popup_reliable: Option<bool>,
    pub last_info_popup_id: Option<String>,
    pub last_info_popup_message: Option<String>,
    pub last_info_popup_duration_bits: Option<u32>,
    pub last_info_popup_align: Option<i32>,
    pub last_info_popup_top: Option<i32>,
    pub last_info_popup_left: Option<i32>,
    pub last_info_popup_bottom: Option<i32>,
    pub last_info_popup_right: Option<i32>,
    pub received_info_toast_count: u64,
    pub last_info_toast_message: Option<String>,
    pub last_info_toast_duration_bits: Option<u32>,
    pub received_warning_toast_count: u64,
    pub last_warning_toast_unicode: Option<i32>,
    pub last_warning_toast_text: Option<String>,
    pub received_menu_open_count: u64,
    pub last_menu_open_id: Option<i32>,
    pub last_menu_open_title: Option<String>,
    pub last_menu_open_message: Option<String>,
    pub last_menu_open_option_rows: usize,
    pub last_menu_open_first_row_len: usize,
    pub received_follow_up_menu_open_count: u64,
    pub last_follow_up_menu_open_id: Option<i32>,
    pub last_follow_up_menu_open_title: Option<String>,
    pub last_follow_up_menu_open_message: Option<String>,
    pub last_follow_up_menu_open_option_rows: usize,
    pub last_follow_up_menu_open_first_row_len: usize,
    pub received_hide_follow_up_menu_count: u64,
    pub last_hide_follow_up_menu_id: Option<i32>,
    pub received_copy_to_clipboard_count: u64,
    pub last_copy_to_clipboard_text: Option<String>,
    pub received_open_uri_count: u64,
    pub last_open_uri: Option<String>,
    pub received_text_input_count: u64,
    pub last_text_input_id: Option<i32>,
    pub last_text_input_title: Option<String>,
    pub last_text_input_message: Option<String>,
    pub last_text_input_length: Option<i32>,
    pub last_text_input_default_text: Option<String>,
    pub last_text_input_numeric: Option<bool>,
    pub last_text_input_allow_empty: Option<bool>,
    pub received_set_item_count: u64,
    pub last_set_item_build_pos: Option<i32>,
    pub last_set_item_item_id: Option<i16>,
    pub last_set_item_amount: Option<i32>,
    pub received_set_items_count: u64,
    pub last_set_items_build_pos: Option<i32>,
    pub last_set_items_count: usize,
    pub last_set_items_first_item_id: Option<i16>,
    pub last_set_items_first_amount: Option<i32>,
    pub received_set_liquid_count: u64,
    pub last_set_liquid_build_pos: Option<i32>,
    pub last_set_liquid_liquid_id: Option<i16>,
    pub last_set_liquid_amount_bits: Option<u32>,
    pub received_set_liquids_count: u64,
    pub last_set_liquids_build_pos: Option<i32>,
    pub last_set_liquids_count: usize,
    pub last_set_liquids_first_liquid_id: Option<i16>,
    pub last_set_liquids_first_amount_bits: Option<u32>,
    pub received_clear_items_count: u64,
    pub last_clear_items_build_pos: Option<i32>,
    pub received_clear_liquids_count: u64,
    pub last_clear_liquids_build_pos: Option<i32>,
    pub received_set_floor_count: u64,
    pub last_set_floor_tile_pos: Option<i32>,
    pub last_set_floor_floor_id: Option<i16>,
    pub last_set_floor_overlay_id: Option<i16>,
    pub received_set_overlay_count: u64,
    pub last_set_overlay_tile_pos: Option<i32>,
    pub last_set_overlay_block_id: Option<i16>,
    pub received_set_map_area_count: u64,
    pub last_set_map_area_x: Option<i32>,
    pub last_set_map_area_y: Option<i32>,
    pub last_set_map_area_w: Option<i32>,
    pub last_set_map_area_h: Option<i32>,
    pub received_set_team_count: u64,
    pub last_set_team_build_pos: Option<i32>,
    pub last_set_team_id: Option<u8>,
    pub received_remove_tile_count: u64,
    pub last_remove_tile_pos: Option<i32>,
    pub received_set_tile_count: u64,
    pub last_set_tile_pos: Option<i32>,
    pub last_set_tile_block_id: Option<i16>,
    pub last_set_tile_team_id: Option<u8>,
    pub last_set_tile_rotation: Option<i32>,
    pub received_set_tile_blocks_count: u64,
    pub last_set_tile_blocks_block_id: Option<i16>,
    pub last_set_tile_blocks_team_id: Option<u8>,
    pub last_set_tile_blocks_count: usize,
    pub last_set_tile_blocks_first_position: Option<i32>,
    pub received_set_tile_floors_count: u64,
    pub last_set_tile_floors_block_id: Option<i16>,
    pub last_set_tile_floors_count: usize,
    pub last_set_tile_floors_first_position: Option<i32>,
    pub received_set_tile_items_count: u64,
    pub last_set_tile_items_item_id: Option<i16>,
    pub last_set_tile_items_amount: Option<i32>,
    pub last_set_tile_items_count: usize,
    pub last_set_tile_items_first_position: Option<i32>,
    pub received_set_tile_liquids_count: u64,
    pub last_set_tile_liquids_liquid_id: Option<i16>,
    pub last_set_tile_liquids_amount_bits: Option<u32>,
    pub last_set_tile_liquids_count: usize,
    pub last_set_tile_liquids_first_position: Option<i32>,
    pub received_set_tile_overlays_count: u64,
    pub last_set_tile_overlays_block_id: Option<i16>,
    pub last_set_tile_overlays_count: usize,
    pub last_set_tile_overlays_first_position: Option<i32>,
    pub received_set_teams_count: u64,
    pub last_set_teams_team_id: Option<u8>,
    pub last_set_teams_count: usize,
    pub last_set_teams_first_position: Option<i32>,
    pub received_sync_variable_count: u64,
    pub last_sync_variable_build_pos: Option<i32>,
    pub last_sync_variable_index: Option<i32>,
    pub last_sync_variable_value_kind: Option<u8>,
    pub last_sync_variable_value_kind_name: Option<String>,
    pub received_set_player_team_editor_count: u64,
    pub last_set_player_team_editor_team_id: Option<u8>,
    pub received_menu_choose_count: u64,
    pub last_menu_choose_menu_id: Option<i32>,
    pub last_menu_choose_option: Option<i32>,
    pub received_text_input_result_count: u64,
    pub last_text_input_result_id: Option<i32>,
    pub last_text_input_result_text: Option<String>,
    pub received_building_control_select_count: u64,
    pub last_building_control_select_build_pos: Option<i32>,
    pub received_unit_clear_count: u64,
    pub received_unit_control_count: u64,
    pub last_unit_control_target: Option<UnitRefProjection>,
    pub received_unit_building_control_select_count: u64,
    pub last_unit_building_control_select_target: Option<UnitRefProjection>,
    pub last_unit_building_control_select_build_pos: Option<i32>,
    pub received_command_building_count: u64,
    pub last_command_building_count: usize,
    pub last_command_building_first_build_pos: Option<i32>,
    pub last_command_building_x_bits: Option<u32>,
    pub last_command_building_y_bits: Option<u32>,
    pub received_command_units_count: u64,
    pub last_command_units_count: usize,
    pub last_command_units_first_unit_id: Option<i32>,
    pub last_command_units_build_target: Option<i32>,
    pub last_command_units_unit_target: Option<UnitRefProjection>,
    pub last_command_units_x_bits: Option<u32>,
    pub last_command_units_y_bits: Option<u32>,
    pub last_command_units_queue: Option<bool>,
    pub last_command_units_final_batch: Option<bool>,
    pub received_set_unit_command_count: u64,
    pub last_set_unit_command_count: usize,
    pub last_set_unit_command_first_unit_id: Option<i32>,
    pub last_set_unit_command_id: Option<u8>,
    pub received_set_unit_stance_count: u64,
    pub last_set_unit_stance_count: usize,
    pub last_set_unit_stance_first_unit_id: Option<i32>,
    pub last_set_unit_stance_id: Option<u8>,
    pub last_set_unit_stance_enable: Option<bool>,
    pub received_rotate_block_count: u64,
    pub last_rotate_block_build_pos: Option<i32>,
    pub last_rotate_block_direction: Option<bool>,
    pub received_transfer_inventory_count: u64,
    pub last_transfer_inventory_build_pos: Option<i32>,
    pub received_request_item_count: u64,
    pub last_request_item_build_pos: Option<i32>,
    pub last_request_item_item_id: Option<i16>,
    pub last_request_item_amount: Option<i32>,
    pub received_request_build_payload_count: u64,
    pub last_request_build_payload_build_pos: Option<i32>,
    pub received_request_drop_payload_count: u64,
    pub last_request_drop_payload_x_bits: Option<u32>,
    pub last_request_drop_payload_y_bits: Option<u32>,
    pub received_request_unit_payload_count: u64,
    pub last_request_unit_payload_target: Option<UnitRefProjection>,
    pub received_drop_item_count: u64,
    pub last_drop_item_angle_bits: Option<u32>,
    pub received_delete_plans_count: u64,
    pub last_delete_plans_count: usize,
    pub last_delete_plans_first_pos: Option<i32>,
    pub received_tile_tap_count: u64,
    pub last_tile_tap_pos: Option<i32>,
    pub received_begin_place_count: u64,
    pub last_begin_place_x: Option<i32>,
    pub last_begin_place_y: Option<i32>,
    pub last_begin_place_block_id: Option<i16>,
    pub last_begin_place_rotation: Option<i32>,
    pub last_begin_place_team_id: Option<u8>,
    pub last_begin_place_config_kind: Option<u8>,
    pub last_begin_place_config_kind_name: Option<String>,
    pub last_begin_place_config_consumed_len: Option<usize>,
    pub last_begin_place_config_object: Option<TypeIoObject>,
    pub received_begin_break_count: u64,
    pub last_begin_break_x: Option<i32>,
    pub last_begin_break_y: Option<i32>,
    pub last_begin_break_team_id: Option<u8>,
    pub received_remove_queue_block_count: u64,
    pub last_remove_queue_block_x: Option<i32>,
    pub last_remove_queue_block_y: Option<i32>,
    pub last_remove_queue_block_breaking: Option<bool>,
    pub last_remove_queue_block_removed_local_plan: bool,
    pub received_tile_config_count: u64,
    pub last_tile_config_build_pos: Option<i32>,
    pub last_tile_config_kind: Option<u8>,
    pub last_tile_config_kind_name: Option<String>,
    pub last_tile_config_consumed_len: Option<usize>,
    pub last_tile_config_object: Option<TypeIoObject>,
    pub last_tile_config_parse_failed: bool,
    pub failed_tile_config_parse_count: u64,
    pub last_tile_config_parse_error: Option<String>,
    pub tile_config_projection: TileConfigProjection,
    pub configured_block_projection: ConfiguredBlockProjection,
    pub building_table_projection: BuildingTableProjection,
    pub received_construct_finish_count: u64,
    pub last_construct_finish_tile_pos: Option<i32>,
    pub last_construct_finish_block_id: Option<i16>,
    pub last_construct_finish_config_kind: Option<u8>,
    pub last_construct_finish_config_kind_name: Option<String>,
    pub last_construct_finish_config_consumed_len: Option<usize>,
    pub last_construct_finish_config_object: Option<TypeIoObject>,
    pub last_construct_finish_removed_local_plan: bool,
    pub received_deconstruct_finish_count: u64,
    pub last_deconstruct_finish_tile_pos: Option<i32>,
    pub last_deconstruct_finish_block_id: Option<i16>,
    pub last_deconstruct_finish_removed_local_plan: bool,
    pub builder_queue_projection: BuilderQueueProjection,
    pub received_build_health_update_count: u64,
    pub received_build_health_update_pair_count: u64,
    pub last_build_health_update_pair_count: usize,
    pub last_build_health_update_first_build_pos: Option<i32>,
    pub last_build_health_update_first_health_bits: Option<u32>,
    pub seen_state_snapshot: bool,
    pub seen_entity_snapshot: bool,
    pub received_entity_snapshot_count: u64,
    pub last_entity_snapshot_amount: Option<u16>,
    pub last_entity_snapshot_body_len: Option<usize>,
    pub entity_snapshot_with_local_target_count: u64,
    pub missed_local_player_sync_from_entity_snapshot_count: u64,
    pub applied_local_player_sync_from_entity_snapshot_count: u64,
    pub applied_local_player_sync_from_entity_snapshot_fallback_count: u64,
    pub ambiguous_local_player_sync_from_entity_snapshot_count: u64,
    pub last_entity_snapshot_target_player_id: Option<i32>,
    pub last_entity_snapshot_used_projection_fallback: bool,
    pub last_entity_snapshot_local_player_sync_applied: bool,
    pub last_entity_snapshot_local_player_sync_ambiguous: bool,
    pub last_entity_snapshot_local_player_sync_match_count: usize,
    pub failed_entity_snapshot_parse_count: u64,
    pub last_entity_snapshot_parse_error: Option<String>,
    pub entity_snapshot_tombstones: BTreeMap<i32, u64>,
    pub entity_snapshot_tombstone_skip_count: u64,
    pub last_entity_snapshot_tombstone_skipped_ids_sample: Vec<i32>,
    pub entity_snapshot_hidden_skip_count: u64,
    pub last_entity_snapshot_hidden_skipped_ids_sample: Vec<i32>,
    pub seen_block_snapshot: bool,
    pub seen_hidden_snapshot: bool,
    pub received_block_snapshot_count: u64,
    pub last_block_snapshot_payload_len: Option<usize>,
    pub applied_block_snapshot_count: u64,
    pub last_block_snapshot: Option<AppliedBlockSnapshotEnvelope>,
    pub block_snapshot_head_projection: Option<BlockSnapshotHeadProjection>,
    pub applied_loaded_world_block_snapshot_extra_entry_count: u64,
    pub last_loaded_world_block_snapshot_extra_entry_count: usize,
    pub failed_loaded_world_block_snapshot_extra_entry_parse_count: u64,
    pub last_loaded_world_block_snapshot_extra_entry_parse_error: Option<String>,
    pub failed_block_snapshot_parse_count: u64,
    pub last_block_snapshot_parse_error: Option<String>,
    pub last_block_snapshot_parse_error_payload_len: Option<usize>,
    pub received_hidden_snapshot_count: u64,
    pub last_hidden_snapshot_payload_len: Option<usize>,
    pub applied_hidden_snapshot_count: u64,
    pub last_hidden_snapshot: Option<AppliedHiddenSnapshotIds>,
    pub hidden_snapshot_ids: BTreeSet<i32>,
    pub hidden_snapshot_delta_projection: Option<HiddenSnapshotDeltaProjection>,
    pub hidden_lifecycle_remove_count: u64,
    pub last_hidden_lifecycle_removed_ids_sample: Vec<i32>,
    pub failed_hidden_snapshot_parse_count: u64,
    pub last_hidden_snapshot_parse_error: Option<String>,
    pub last_hidden_snapshot_parse_error_payload_len: Option<usize>,
    pub entity_table_projection: EntityTableProjection,
    pub entity_semantic_projection: EntitySemanticProjectionTable,
}

impl SessionState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_reconnect_phase(&mut self, phase: ReconnectPhaseProjection) {
        self.reconnect_projection.set_phase(phase);
    }

    pub fn record_reconnect_projection(
        &mut self,
        phase: ReconnectPhaseProjection,
        reason_kind: Option<ReconnectReasonKind>,
        reason_text: Option<String>,
        reason_ordinal: Option<i32>,
        hint_text: Option<String>,
    ) {
        self.reconnect_projection.set_phase(phase);
        self.reconnect_projection.reason_kind = reason_kind;
        self.reconnect_projection.reason_text = reason_text;
        self.reconnect_projection.reason_ordinal = reason_ordinal;
        self.reconnect_projection.hint_text = hint_text;
    }

    pub fn apply_state_snapshot_runtime(
        &mut self,
        snapshot: &AppliedStateSnapshot,
        core_data: Option<&AppliedStateSnapshotCoreData>,
        core_data_parse_failed: bool,
    ) {
        let previous = self.authoritative_state_mirror.as_ref();
        let previous_wave = previous.map(|mirror| mirror.wave).unwrap_or_default();
        let previous_net_seconds = previous
            .map(|mirror| mirror.net_seconds)
            .unwrap_or_default();
        let last_wave_advanced = snapshot.wave > previous_wave;
        let wave_advance_count = previous
            .map(|mirror| mirror.wave_advance_count)
            .unwrap_or_default()
            .saturating_add(u64::from(last_wave_advanced));
        let last_net_seconds_rollback = snapshot.time_data < previous_net_seconds;
        let net_seconds_delta_i64 = i64::from(snapshot.time_data) - i64::from(previous_net_seconds);
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

        self.authoritative_state_mirror = Some(AuthoritativeStateMirror {
            wave_time_bits: snapshot.wave_time_bits,
            wave: snapshot.wave,
            enemies: snapshot.enemies,
            paused: snapshot.paused,
            game_over: snapshot.game_over,
            net_seconds: snapshot.time_data,
            tps: snapshot.tps,
            rand0: snapshot.rand0,
            rand1: snapshot.rand1,
            gameplay_state,
            last_wave_advanced,
            wave_advance_count,
            apply_count: previous
                .map(|mirror| mirror.apply_count)
                .unwrap_or_default()
                .saturating_add(1),
            last_net_seconds_rollback,
            net_seconds_delta,
            wave_regress_count: previous
                .map(|mirror| mirror.wave_regress_count)
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
                .map(|mirror| mirror.core_parse_fail_count)
                .unwrap_or_default()
                .saturating_add(u64::from(core_data_parse_failed)),
        });
    }

    pub fn prune_entity_snapshot_tombstones(&mut self) {
        let current_snapshot_count = self.received_entity_snapshot_count;
        self.entity_snapshot_tombstones
            .retain(|_, removed_at_snapshot_count| {
                current_snapshot_count.saturating_sub(*removed_at_snapshot_count)
                    <= ENTITY_SNAPSHOT_TOMBSTONE_TTL_SNAPSHOTS
            });
    }

    pub fn clear_entity_snapshot_tombstones(&mut self) {
        self.entity_snapshot_tombstones.clear();
        self.last_entity_snapshot_tombstone_skipped_ids_sample
            .clear();
    }

    pub fn record_entity_snapshot_tombstone(&mut self, entity_id: i32) {
        self.entity_snapshot_tombstones
            .insert(entity_id, self.received_entity_snapshot_count);
    }

    pub fn entity_snapshot_tombstone_blocks_upsert(&self, entity_id: i32) -> bool {
        self.entity_snapshot_tombstones
            .get(&entity_id)
            .is_some_and(|removed_at_snapshot_count| {
                self.received_entity_snapshot_count
                    .saturating_sub(*removed_at_snapshot_count)
                    <= ENTITY_SNAPSHOT_TOMBSTONE_TTL_SNAPSHOTS
            })
    }

    pub fn record_entity_snapshot_tombstone_skip(&mut self, entity_id: i32) {
        self.entity_snapshot_tombstone_skip_count =
            self.entity_snapshot_tombstone_skip_count.saturating_add(1);
        if self.last_entity_snapshot_tombstone_skipped_ids_sample.len()
            < ENTITY_SNAPSHOT_TOMBSTONE_SKIP_SAMPLE_LIMIT
            && !self
                .last_entity_snapshot_tombstone_skipped_ids_sample
                .contains(&entity_id)
        {
            self.last_entity_snapshot_tombstone_skipped_ids_sample
                .push(entity_id);
        }
    }

    pub fn entity_snapshot_hidden_blocks_upsert(
        &self,
        entity_id: i32,
        is_local_player: bool,
    ) -> bool {
        !is_local_player && self.hidden_snapshot_ids.contains(&entity_id)
    }

    pub fn record_entity_snapshot_hidden_skip(&mut self, entity_id: i32) {
        self.entity_snapshot_hidden_skip_count =
            self.entity_snapshot_hidden_skip_count.saturating_add(1);
        if self.last_entity_snapshot_hidden_skipped_ids_sample.len()
            < ENTITY_SNAPSHOT_TOMBSTONE_SKIP_SAMPLE_LIMIT
            && !self
                .last_entity_snapshot_hidden_skipped_ids_sample
                .contains(&entity_id)
        {
            self.last_entity_snapshot_hidden_skipped_ids_sample
                .push(entity_id);
        }
    }

    pub fn apply_hidden_snapshot(
        &mut self,
        applied: AppliedHiddenSnapshotIds,
        trigger_hidden_ids: BTreeSet<i32>,
    ) {
        let previous_hidden_ids = std::mem::take(&mut self.hidden_snapshot_ids);
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

        self.applied_hidden_snapshot_count = self.applied_hidden_snapshot_count.saturating_add(1);
        self.last_hidden_snapshot = Some(applied);
        self.entity_table_projection
            .apply_hidden_ids(&trigger_hidden_ids);
        let local_player_entity_id = self.entity_table_projection.local_player_entity_id;
        let hidden_removed_ids = self
            .entity_table_projection
            .remove_hidden_entities(&trigger_hidden_ids);
        self.entity_semantic_projection
            .remove_hidden_entities(&trigger_hidden_ids, local_player_entity_id);
        self.resource_delta_projection
            .remove_hidden_entities(&trigger_hidden_ids, local_player_entity_id);
        self.payload_lifecycle_projection
            .remove_hidden_entities(&trigger_hidden_ids, local_player_entity_id);
        self.hidden_lifecycle_remove_count = self
            .hidden_lifecycle_remove_count
            .saturating_add(hidden_removed_ids.len() as u64);
        self.last_hidden_lifecycle_removed_ids_sample = hidden_removed_ids
            .into_iter()
            .take(HIDDEN_SNAPSHOT_SAMPLE_LIMIT)
            .collect();
        self.hidden_snapshot_ids = trigger_hidden_ids;
        self.hidden_snapshot_delta_projection = Some(HiddenSnapshotDeltaProjection {
            active_count: trigger_count,
            added_count: added_ids.len(),
            removed_count: removed_ids.len(),
            added_sample_ids,
            removed_sample_ids,
        });
    }

    pub fn record_payload_lifecycle_drop(
        &mut self,
        carrier: Option<UnitRefProjection>,
        drop_tile: Option<i32>,
    ) {
        let Some(carrier) = carrier else {
            return;
        };
        let entry = self.payload_lifecycle_projection.entry_mut(carrier);
        entry.drop_tile = drop_tile;
        entry.on_ground = Some(true);
        entry.removed_carrier = false;
        if entry.target_unit.is_some() {
            entry.removed_target_unit = false;
        }
        if entry.target_build.is_some() {
            entry.removed_target_build = false;
        }
    }

    pub fn record_picked_build_payload_lifecycle(
        &mut self,
        carrier: Option<UnitRefProjection>,
        target_build: Option<i32>,
        on_ground: bool,
    ) {
        let Some(carrier) = carrier else {
            return;
        };
        let entry = self.payload_lifecycle_projection.entry_mut(carrier);
        entry.target_unit = None;
        entry.target_build = target_build;
        entry.drop_tile = None;
        entry.on_ground = Some(on_ground);
        entry.removed_target_unit = false;
        entry.removed_target_build = target_build.is_some();
        entry.removed_carrier = false;
    }

    pub fn record_picked_unit_payload_lifecycle(
        &mut self,
        carrier: Option<UnitRefProjection>,
        target_unit: Option<UnitRefProjection>,
    ) {
        let Some(carrier) = carrier else {
            return;
        };
        let entry = self.payload_lifecycle_projection.entry_mut(carrier);
        entry.target_unit = target_unit;
        entry.target_build = None;
        entry.drop_tile = None;
        entry.on_ground = Some(false);
        entry.removed_target_unit = target_unit.is_some();
        entry.removed_target_build = false;
        entry.removed_carrier = false;
    }

    pub fn mark_payload_lifecycle_unit_despawn(&mut self, unit: Option<UnitRefProjection>) {
        let Some(unit) = unit else {
            return;
        };
        for entry in self.payload_lifecycle_projection.by_carrier.values_mut() {
            if entry.carrier == unit {
                entry.removed_carrier = true;
            }
            if entry.target_unit == Some(unit) {
                entry.removed_target_unit = true;
            }
        }
    }

    pub fn record_set_item_resource_delta(
        &mut self,
        build_pos: Option<i32>,
        item_id: Option<i16>,
        amount: i32,
    ) {
        self.resource_delta_projection
            .apply_set_item(build_pos, item_id, amount);
    }

    pub fn record_set_items_resource_delta(
        &mut self,
        build_pos: Option<i32>,
        stacks: &[(Option<i16>, i32)],
    ) {
        self.resource_delta_projection
            .apply_set_items(build_pos, stacks);
    }

    pub fn record_set_tile_items_resource_delta(
        &mut self,
        item_id: Option<i16>,
        amount: i32,
        positions: &[i32],
    ) {
        self.resource_delta_projection
            .apply_set_tile_items(item_id, amount, positions);
    }

    pub fn record_clear_items_resource_delta(&mut self, build_pos: Option<i32>) {
        self.resource_delta_projection.clear_build_items(build_pos);
    }

    pub fn record_remove_building_resource_delta(&mut self, build_pos: Option<i32>) {
        self.resource_delta_projection.remove_building(build_pos);
    }

    pub fn record_remove_resource_delta_entity(&mut self, unit: Option<UnitRefProjection>) {
        self.resource_delta_projection
            .remove_standard_entity_item(unit);
    }

    pub fn record_remove_resource_delta_entity_by_id(&mut self, entity_id: Option<i32>) {
        self.resource_delta_projection
            .remove_entity_item_by_id(entity_id);
    }

    pub fn record_take_items_resource_delta(&mut self, projection: &TakeItemsProjection) {
        self.resource_delta_projection.take_items_count = self
            .resource_delta_projection
            .take_items_count
            .saturating_add(1);
        self.resource_delta_projection.last_kind = Some("take");
        self.resource_delta_projection.last_item_id = projection.item_id;
        self.resource_delta_projection.last_amount = Some(projection.amount);
        self.resource_delta_projection.last_build_pos = projection.build_pos;
        self.resource_delta_projection.last_unit = projection.to;
        self.resource_delta_projection.last_to_entity_id = None;
        self.resource_delta_projection.last_x_bits = None;
        self.resource_delta_projection.last_y_bits = None;
        self.resource_delta_projection.apply_take_items(projection);
    }

    pub fn record_transfer_item_to_resource_delta(
        &mut self,
        projection: &TransferItemToProjection,
    ) {
        self.resource_delta_projection.transfer_item_to_count = self
            .resource_delta_projection
            .transfer_item_to_count
            .saturating_add(1);
        self.resource_delta_projection.last_kind = Some("to_build");
        self.resource_delta_projection.last_item_id = projection.item_id;
        self.resource_delta_projection.last_amount = Some(projection.amount);
        self.resource_delta_projection.last_build_pos = projection.build_pos;
        self.resource_delta_projection.last_unit = projection.unit;
        self.resource_delta_projection.last_to_entity_id = None;
        self.resource_delta_projection.last_x_bits = Some(projection.x_bits);
        self.resource_delta_projection.last_y_bits = Some(projection.y_bits);
        self.resource_delta_projection
            .apply_transfer_item_to(projection);
    }

    pub fn record_transfer_item_to_unit_resource_delta(
        &mut self,
        projection: &TransferItemToUnitProjection,
    ) {
        self.resource_delta_projection.transfer_item_to_unit_count = self
            .resource_delta_projection
            .transfer_item_to_unit_count
            .saturating_add(1);
        self.resource_delta_projection.last_kind = Some("to_unit");
        self.resource_delta_projection.last_item_id = projection.item_id;
        self.resource_delta_projection.last_amount = None;
        self.resource_delta_projection.last_build_pos = None;
        self.resource_delta_projection.last_unit = None;
        self.resource_delta_projection.last_to_entity_id = projection.to_entity_id;
        self.resource_delta_projection.last_x_bits = Some(projection.x_bits);
        self.resource_delta_projection.last_y_bits = Some(projection.y_bits);
        self.resource_delta_projection
            .apply_transfer_item_to_unit(projection);
    }
}

impl PayloadLifecycleProjection {
    pub fn remove_hidden_entities(
        &mut self,
        hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) {
        let removed_carriers = self
            .by_carrier
            .keys()
            .copied()
            .filter(|carrier| {
                hidden_lifecycle_hidden_non_local_unit_entity_id(
                    Some(*carrier),
                    hidden_ids,
                    local_player_entity_id,
                )
                .is_some()
            })
            .collect::<Vec<_>>();
        for carrier in removed_carriers {
            self.by_carrier.remove(&carrier);
        }

        for entry in self.by_carrier.values_mut() {
            if hidden_lifecycle_hidden_non_local_unit_entity_id(
                entry.target_unit,
                hidden_ids,
                local_player_entity_id,
            )
            .is_some()
            {
                entry.removed_target_unit = true;
            }
        }
    }

    fn entry_mut(&mut self, carrier: UnitRefProjection) -> &mut PayloadLifecycleCarrierProjection {
        self.by_carrier
            .entry(carrier)
            .or_insert_with(|| PayloadLifecycleCarrierProjection {
                carrier,
                target_unit: None,
                target_build: None,
                drop_tile: None,
                on_ground: None,
                removed_target_unit: false,
                removed_target_build: false,
                removed_carrier: false,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_projection_counts_only_distinct_phase_transitions() {
        let mut state = SessionState::default();

        assert_eq!(
            state.reconnect_projection.phase,
            ReconnectPhaseProjection::Idle
        );
        assert_eq!(state.reconnect_projection.phase_transition_count, 0);

        state.record_reconnect_projection(
            ReconnectPhaseProjection::Scheduled,
            Some(ReconnectReasonKind::ConnectRedirect),
            Some("connectRedirect".to_string()),
            None,
            Some("server requested redirect".to_string()),
        );
        state.set_reconnect_phase(ReconnectPhaseProjection::Scheduled);
        state.set_reconnect_phase(ReconnectPhaseProjection::Attempting);
        state.record_reconnect_projection(
            ReconnectPhaseProjection::Aborted,
            Some(ReconnectReasonKind::Timeout),
            Some("connectOrLoadingTimeout".to_string()),
            None,
            Some("session timed out".to_string()),
        );

        assert_eq!(
            state.reconnect_projection.phase,
            ReconnectPhaseProjection::Aborted
        );
        assert_eq!(state.reconnect_projection.phase_transition_count, 3);
        assert_eq!(
            state.reconnect_projection.reason_kind,
            Some(ReconnectReasonKind::Timeout)
        );
        assert_eq!(
            state.reconnect_projection.reason_text.as_deref(),
            Some("connectOrLoadingTimeout")
        );
        assert_eq!(
            state.reconnect_projection.hint_text.as_deref(),
            Some("session timed out")
        );
    }

    #[test]
    fn block_snapshot_head_stores_build_turret_plan_summary_and_construct_finish_preserves_it() {
        let mut table = BuildingTableProjection::default();
        let build_pos = 0x0012_0034i32;
        table.apply_block_snapshot_head(
            build_pos,
            300,
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(123),
            Some(true),
            Some(0x4000_0000),
            Some(true),
            Some(0x40),
            Some(0x20),
            Some(99),
            Some(0x4260_0000),
            Some(true),
            Some(7),
        );

        let building = table.by_build_pos.get(&build_pos).unwrap();
        assert_eq!(building.build_turret_rotation_bits, Some(0x4260_0000));
        assert_eq!(building.build_turret_plans_present, Some(true));
        assert_eq!(building.build_turret_plan_count, Some(7));
        assert_eq!(table.last_build_turret_rotation_bits, Some(0x4260_0000));
        assert_eq!(table.last_build_turret_plans_present, Some(true));
        assert_eq!(table.last_build_turret_plan_count, Some(7));

        table.apply_construct_finish(build_pos, Some(300), 1, 2, TypeIoObject::Int(9));
        let building_after_construct = table.by_build_pos.get(&build_pos).unwrap();
        assert_eq!(
            building_after_construct.build_turret_rotation_bits,
            Some(0x4260_0000)
        );
        assert_eq!(
            building_after_construct.build_turret_plans_present,
            Some(true)
        );
        assert_eq!(building_after_construct.build_turret_plan_count, Some(7));
    }
}
