use crate::effect_data_runtime::EffectDataBusinessHint;
use crate::entity_snapshot_families::{
    ALPHA_SHAPE_ENTITY_CLASS_IDS, BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS, FIRE_ENTITY_CLASS_IDS,
    MECH_SHAPE_ENTITY_CLASS_IDS, MISSILE_SHAPE_ENTITY_CLASS_IDS, PAYLOAD_SHAPE_ENTITY_CLASS_IDS,
    PUDDLE_ENTITY_CLASS_IDS, WEATHER_STATE_ENTITY_CLASS_IDS,
};
use crate::rules_objectives_semantics::{ObjectivesProjection, RulesProjection};
use crate::state_snapshot_semantics::{
    derive_state_snapshot_core_inventory_transition, StateSnapshotCoreInventoryPrevious,
};
use mdt_remote::HighFrequencyRemoteMethod;
use mdt_typeio::TypeIoObject;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[path = "runtime_entity_ownership.rs"]
mod runtime_entity_ownership;

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
pub struct FinishConnectingProjection {
    pub committed_at_ms: u64,
    pub replayed_loading_packet_count: u64,
    pub total_replayed_loading_packet_count: u64,
    pub ready_to_enter_world: bool,
    pub client_loaded: bool,
    pub connect_confirm_queued: bool,
    pub connect_confirm_flushed: bool,
    pub snapshot_watchdog_armed_at_ms: Option<u64>,
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
pub enum EffectRuntimeBindingState {
    ParentFollow,
    BindingRejected,
    UnresolvedFallback,
}

impl EffectRuntimeBindingState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParentFollow => "follow",
            Self::BindingRejected => "reject",
            Self::UnresolvedFallback => "fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfiguredContentRef {
    pub content_type: u8,
    pub content_id: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorRuntimeProjection {
    pub progress_bits: u32,
    pub payload_present: bool,
    pub pay_rotation_bits: u32,
    pub payload_build_block_id: Option<i16>,
    pub payload_unit_class_id: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadLoaderRuntimeProjection {
    pub exporting: bool,
    pub payload_present: bool,
    pub payload_type: Option<u8>,
    pub pay_rotation_bits: u32,
    pub payload_build_block_id: Option<i16>,
    pub payload_build_revision: Option<u8>,
    pub payload_unit_class_id: Option<u8>,
    pub payload_unit_payload_len: Option<usize>,
    pub payload_unit_payload_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MassDriverRuntimeProjection {
    pub rotation_bits: u32,
    pub state_ordinal: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadMassDriverRuntimeProjection {
    pub turret_rotation_bits: u32,
    pub state_ordinal: u8,
    pub reload_counter_bits: u32,
    pub charge_bits: u32,
    pub loaded: bool,
    pub charging: bool,
    pub payload_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorterRuntimeProjection {
    pub legacy: bool,
    pub non_empty_side_mask: u8,
    pub buffered_item_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemBridgeBufferRuntimeProjection {
    pub index: i8,
    pub capacity: usize,
    pub normalized_index: i32,
    pub entry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemBridgeRuntimeProjection {
    pub warmup_bits: u32,
    pub incoming_count: usize,
    pub moved: bool,
    pub buffer: Option<ItemBridgeBufferRuntimeProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuctUnloaderRuntimeProjection {
    pub offset: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadSourceRuntimeProjection {
    pub command_pos: Option<(u32, u32)>,
    pub pay_vector_x_bits: u32,
    pub pay_vector_y_bits: u32,
    pub pay_rotation_bits: u32,
    pub payload_present: bool,
    pub payload_type: Option<u8>,
    pub payload_build_block_id: Option<i16>,
    pub payload_build_revision: Option<u8>,
    pub payload_unit_class_id: Option<u8>,
    pub payload_unit_payload_len: Option<usize>,
    pub payload_unit_payload_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadRouterPayloadKind {
    Null,
    Build,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadRouterRuntimeProjection {
    pub progress_bits: u32,
    pub item_rotation_bits: u32,
    pub payload_present: bool,
    pub payload_type: Option<u8>,
    pub payload_kind: Option<PayloadRouterPayloadKind>,
    pub payload_build_block_id: Option<i16>,
    pub payload_build_revision: Option<u8>,
    pub payload_unit_class_id: Option<u8>,
    pub payload_unit_revision: Option<i16>,
    pub payload_serialized_len: usize,
    pub payload_serialized_sha256: String,
    pub rec_dir: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitFactoryRuntimeProjection {
    pub progress_bits: u32,
    pub command_pos: Option<(u32, u32)>,
    pub command_id: Option<u8>,
    pub payload_present: bool,
    pub pay_rotation_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAssemblerRuntimeProjection {
    pub progress_bits: u32,
    pub unit_ids: Vec<i32>,
    pub block_entry_count: usize,
    pub block_sample: Option<ConfiguredContentRef>,
    pub command_pos: Option<(u32, u32)>,
    pub payload_present: bool,
    pub pay_rotation_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructorRuntimeProjection {
    pub progress_bits: u32,
    pub command_pos: Option<(u32, u32)>,
    pub payload_present: bool,
    pub pay_rotation_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreInventoryRuntimeBindingKind {
    FirstCorePerTeamApproximation,
}

impl CoreInventoryRuntimeBindingKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstCorePerTeamApproximation => "first-core-per-team",
        }
    }
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
    PayloadTargetContent {
        source_x_bits: u32,
        source_y_bits: u32,
        target_x_bits: u32,
        target_y_bits: u32,
        content_type: u8,
        content_id: i16,
    },
    LengthRay {
        source_x_bits: u32,
        source_y_bits: u32,
        target_x_bits: u32,
        target_y_bits: u32,
        rotation_bits: u32,
        length_bits: u32,
    },
    LightningPath {
        points: Vec<(u32, u32)>,
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
    pub building_liquids_by_build: BTreeMap<i32, BTreeMap<i16, u32>>,
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

    pub fn apply_set_liquid(
        &mut self,
        build_pos: Option<i32>,
        liquid_id: Option<i16>,
        amount_bits: u32,
    ) {
        let (Some(build_pos), Some(liquid_id)) = (build_pos, liquid_id) else {
            return;
        };
        self.set_build_liquid_exact(build_pos, liquid_id, amount_bits);
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.mark_build_liquid_change(build_pos);
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

    pub fn apply_set_liquids(&mut self, build_pos: Option<i32>, stacks: &[(Option<i16>, u32)]) {
        let Some(build_pos) = build_pos else {
            return;
        };
        let mut applied = false;
        for &(liquid_id, amount_bits) in stacks {
            let Some(liquid_id) = liquid_id else {
                continue;
            };
            self.set_build_liquid_exact(build_pos, liquid_id, amount_bits);
            applied = true;
        }
        if applied {
            self.authoritative_build_update_count =
                self.authoritative_build_update_count.saturating_add(1);
            self.mark_build_liquid_change(build_pos);
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

    pub fn apply_set_tile_liquids(
        &mut self,
        liquid_id: Option<i16>,
        amount_bits: u32,
        positions: &[i32],
    ) {
        let Some(liquid_id) = liquid_id else {
            return;
        };
        let mut applied = false;
        for &build_pos in positions {
            self.set_build_liquid_exact(build_pos, liquid_id, amount_bits);
            self.mark_build_liquid_change(build_pos);
            applied = true;
        }
        if applied {
            self.authoritative_build_update_count =
                self.authoritative_build_update_count.saturating_add(1);
        }
    }

    pub fn seed_world_build_items(&mut self, build_pos: i32, stacks: &[(i16, i32)]) {
        let mut build_items = BTreeMap::new();
        for &(item_id, amount) in stacks {
            if amount != 0 {
                build_items.insert(item_id, amount);
            }
        }
        if build_items.is_empty() {
            self.building_items_by_build.remove(&build_pos);
        } else {
            self.building_items_by_build.insert(build_pos, build_items);
        }
    }

    pub fn seed_world_build_liquids(&mut self, build_pos: i32, stacks: &[(i16, u32)]) {
        let mut build_liquids = BTreeMap::new();
        for &(liquid_id, amount_bits) in stacks {
            if !liquid_amount_bits_is_zero(amount_bits) {
                build_liquids.insert(liquid_id, amount_bits);
            }
        }
        if build_liquids.is_empty() {
            self.building_liquids_by_build.remove(&build_pos);
        } else {
            self.building_liquids_by_build
                .insert(build_pos, build_liquids);
        }
    }

    pub fn replace_build_items_exact(&mut self, build_pos: Option<i32>, stacks: &[(i16, i32)]) {
        let Some(build_pos) = build_pos else {
            return;
        };
        let mut build_items = BTreeMap::new();
        for &(item_id, amount) in stacks {
            if amount != 0 {
                build_items.insert(item_id, amount);
            }
        }
        let (last_item_id, last_amount) = match build_items.iter().next_back() {
            Some((&item_id, &amount)) => (Some(item_id), Some(amount)),
            None => (None, Some(0)),
        };
        if build_items.is_empty() {
            self.building_items_by_build.remove(&build_pos);
        } else {
            self.building_items_by_build.insert(build_pos, build_items);
        }
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.last_changed_build_pos = Some(build_pos);
        self.last_changed_entity_id = None;
        self.last_changed_item_id = last_item_id;
        self.last_changed_amount = last_amount;
    }

    pub fn replace_build_liquids_exact(&mut self, build_pos: Option<i32>, stacks: &[(i16, u32)]) {
        let Some(build_pos) = build_pos else {
            return;
        };
        let mut build_liquids = BTreeMap::new();
        for &(liquid_id, amount_bits) in stacks {
            if !liquid_amount_bits_is_zero(amount_bits) {
                build_liquids.insert(liquid_id, amount_bits);
            }
        }
        if build_liquids.is_empty() {
            self.building_liquids_by_build.remove(&build_pos);
        } else {
            self.building_liquids_by_build
                .insert(build_pos, build_liquids);
        }
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.mark_build_liquid_change(build_pos);
    }

    pub fn replace_entity_item_stack_exact(
        &mut self,
        entity_id: Option<i32>,
        stack_item_id: i16,
        stack_amount: i32,
    ) {
        let Some(entity_id) = entity_id else {
            return;
        };
        if stack_amount > 0 && stack_item_id >= 0 {
            self.entity_item_stack_by_entity_id.insert(
                entity_id,
                ResourceUnitItemStack {
                    item_id: Some(stack_item_id),
                    amount: stack_amount,
                },
            );
            self.last_changed_build_pos = None;
            self.last_changed_entity_id = Some(entity_id);
            self.last_changed_item_id = Some(stack_item_id);
            self.last_changed_amount = Some(stack_amount);
            return;
        }

        self.entity_item_stack_by_entity_id.remove(&entity_id);
        self.last_changed_build_pos = None;
        self.last_changed_entity_id = Some(entity_id);
        self.last_changed_item_id = None;
        self.last_changed_amount = Some(0);
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

    pub fn clear_build_liquids(&mut self, build_pos: Option<i32>) {
        let Some(build_pos) = build_pos else {
            return;
        };
        self.building_liquids_by_build.remove(&build_pos);
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.last_changed_build_pos = Some(build_pos);
        self.last_changed_entity_id = None;
        self.last_changed_item_id = None;
        self.last_changed_amount = Some(0);
    }

    pub fn remove_building(&mut self, build_pos: Option<i32>) {
        let Some(build_pos) = build_pos else {
            return;
        };
        self.building_items_by_build.remove(&build_pos);
        self.building_liquids_by_build.remove(&build_pos);
        self.authoritative_build_update_count =
            self.authoritative_build_update_count.saturating_add(1);
        self.last_changed_build_pos = Some(build_pos);
        self.last_changed_entity_id = None;
        self.last_changed_item_id = None;
        self.last_changed_amount = Some(0);
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

    pub fn clear_hidden_entity_refs(
        &mut self,
        hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) {
        clear_hidden_non_local_unit_ref(&mut self.last_unit, hidden_ids, local_player_entity_id);
        clear_hidden_non_local_entity_id(
            &mut self.last_to_entity_id,
            hidden_ids,
            local_player_entity_id,
        );
        clear_hidden_non_local_entity_id(
            &mut self.last_changed_entity_id,
            hidden_ids,
            local_player_entity_id,
        );
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

    fn set_build_liquid_exact(&mut self, build_pos: i32, liquid_id: i16, amount_bits: u32) {
        if liquid_amount_bits_is_zero(amount_bits) {
            let mut remove_build = false;
            if let Some(build_liquids) = self.building_liquids_by_build.get_mut(&build_pos) {
                build_liquids.remove(&liquid_id);
                remove_build = build_liquids.is_empty();
            }
            if remove_build {
                self.building_liquids_by_build.remove(&build_pos);
            }
            return;
        }

        self.building_liquids_by_build
            .entry(build_pos)
            .or_default()
            .insert(liquid_id, amount_bits);
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

    fn mark_build_liquid_change(&mut self, build_pos: i32) {
        self.last_changed_build_pos = Some(build_pos);
        self.last_changed_entity_id = None;
        self.last_changed_item_id = None;
        self.last_changed_amount = None;
    }

    fn mark_entity_change(&mut self, entity_id: i32, item_id: i16, amount: i32) {
        self.last_changed_build_pos = None;
        self.last_changed_entity_id = Some(entity_id);
        self.last_changed_item_id = Some(item_id);
        self.last_changed_amount = Some(amount);
    }
}

fn liquid_amount_bits_is_zero(amount_bits: u32) -> bool {
    f32::from_bits(amount_bits) == 0.0
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

fn clear_hidden_non_local_entity_id(
    entity_id: &mut Option<i32>,
    hidden_ids: &BTreeSet<i32>,
    local_player_entity_id: Option<i32>,
) {
    if entity_id.is_some_and(|entity_id| {
        hidden_lifecycle_matches_hidden_non_local_entity_id(
            hidden_ids,
            local_player_entity_id,
            entity_id,
        )
    }) {
        *entity_id = None;
    }
}

fn clear_hidden_non_local_unit_ref(
    unit: &mut Option<UnitRefProjection>,
    hidden_ids: &BTreeSet<i32>,
    local_player_entity_id: Option<i32>,
) {
    if hidden_lifecycle_hidden_non_local_unit_entity_id(*unit, hidden_ids, local_player_entity_id)
        .is_some()
    {
        *unit = None;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum HiddenSnapshotTypedPolicy {
    #[default]
    KeepHidden,
    RemoveLikeJavaUnitHandleSyncHidden,
}

impl HiddenSnapshotTypedPolicy {
    fn merges_remove(self, other: Self) -> Self {
        if matches!(self, Self::RemoveLikeJavaUnitHandleSyncHidden)
            || matches!(other, Self::RemoveLikeJavaUnitHandleSyncHidden)
        {
            Self::RemoveLikeJavaUnitHandleSyncHidden
        } else {
            Self::KeepHidden
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum HiddenSnapshotRuntimePolicy {
    #[default]
    KeepHidden,
    RemoveKnownRuntimeOwned,
}

impl HiddenSnapshotRuntimePolicy {
    fn merges_remove(self, other: Self) -> Self {
        if matches!(self, Self::RemoveKnownRuntimeOwned)
            || matches!(other, Self::RemoveKnownRuntimeOwned)
        {
            Self::RemoveKnownRuntimeOwned
        } else {
            Self::KeepHidden
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct HiddenSnapshotEntityPolicy {
    typed: HiddenSnapshotTypedPolicy,
    runtime: HiddenSnapshotRuntimePolicy,
}

impl HiddenSnapshotEntityPolicy {
    fn should_remove_known_runtime_owned(self) -> bool {
        matches!(
            self.runtime,
            HiddenSnapshotRuntimePolicy::RemoveKnownRuntimeOwned
        )
    }
}

fn class_id_matches_java_unit_handle_sync_hidden_remove(class_id: u8) -> bool {
    ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id)
        || MECH_SHAPE_ENTITY_CLASS_IDS.contains(&class_id)
        || MISSILE_SHAPE_ENTITY_CLASS_IDS.contains(&class_id)
        || PAYLOAD_SHAPE_ENTITY_CLASS_IDS.contains(&class_id)
        || BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS.contains(&class_id)
}

fn class_id_matches_known_runtime_owned_hidden_remove(class_id: u8) -> bool {
    FIRE_ENTITY_CLASS_IDS.contains(&class_id)
        || PUDDLE_ENTITY_CLASS_IDS.contains(&class_id)
        || WEATHER_STATE_ENTITY_CLASS_IDS.contains(&class_id)
}

fn resolve_hidden_snapshot_entity_policy(
    entity: Option<&EntityProjection>,
    semantic: Option<&EntitySemanticProjection>,
) -> HiddenSnapshotEntityPolicy {
    let typed = entity
        .map(EntityProjection::hidden_snapshot_typed_policy)
        .unwrap_or_default()
        .merges_remove(
            semantic
                .map(EntitySemanticProjection::hidden_snapshot_typed_policy)
                .unwrap_or_default(),
        );
    let runtime = entity
        .map(EntityProjection::hidden_snapshot_runtime_policy)
        .unwrap_or_default()
        .merges_remove(
            semantic
                .map(EntitySemanticProjection::hidden_snapshot_runtime_policy)
                .unwrap_or_default(),
        );

    HiddenSnapshotEntityPolicy { typed, runtime }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct HiddenSnapshotRuntimeTransition {
    auxiliary_cleanup_ids: BTreeSet<i32>,
    unit_handle_sync_hidden_remove_ids: BTreeSet<i32>,
    runtime_owned_cleanup_remove_ids: BTreeSet<i32>,
}

impl HiddenSnapshotRuntimeTransition {
    fn lifecycle_remove_ids(&self) -> BTreeSet<i32> {
        self.unit_handle_sync_hidden_remove_ids
            .union(&self.runtime_owned_cleanup_remove_ids)
            .copied()
            .collect()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct HiddenSnapshotTypedRuntimeTransition {
    refresh_ids: BTreeSet<i32>,
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
    pub pending_local_request_queue_by_build_pos: BTreeMap<i32, VecDeque<TypeIoObject>>,
    pub authoritative_by_build_pos: BTreeMap<i32, TypeIoObject>,
    pub canonical_authoritative_by_build_pos: BTreeMap<i32, TypeIoObject>,
    pub queued_local_count: u64,
    pub applied_authoritative_count: u64,
    pub applied_tile_config_packet_count: u64,
    pub applied_construct_finish_count: u64,
    pub rollback_count: u64,
    pub fallback_missing_authority_count: u64,
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
        self.pending_local_request_queue_by_build_pos
            .entry(build_pos)
            .or_default()
            .push_back(value.clone());
        self.pending_local_by_build_pos.insert(build_pos, value);
    }

    fn pop_next_pending_local_request(&mut self, build_pos: i32) -> Option<TypeIoObject> {
        let pending_local = self
            .pending_local_request_queue_by_build_pos
            .get_mut(&build_pos)
            .and_then(VecDeque::pop_front);
        self.sync_pending_local_latest_value(build_pos);
        pending_local
    }

    fn clear_all_pending_local_requests(&mut self, build_pos: i32) {
        self.pending_local_request_queue_by_build_pos
            .remove(&build_pos);
        self.pending_local_by_build_pos.remove(&build_pos);
    }

    fn sync_pending_local_latest_value(&mut self, build_pos: i32) {
        match self
            .pending_local_request_queue_by_build_pos
            .get(&build_pos)
            .and_then(|queue| queue.back())
            .cloned()
        {
            Some(value) => {
                self.pending_local_by_build_pos.insert(build_pos, value);
            }
            None => {
                self.pending_local_request_queue_by_build_pos
                    .remove(&build_pos);
                self.pending_local_by_build_pos.remove(&build_pos);
            }
        }
    }

    pub fn apply_authoritative_update(
        &mut self,
        build_pos: i32,
        value: TypeIoObject,
        canonical_authoritative_value: Option<TypeIoObject>,
        source: TileConfigAuthoritySource,
        configured_block_outcome: Option<ConfiguredBlockOutcome>,
        configured_block_name: Option<String>,
    ) -> TileConfigBusinessApply {
        self.apply_authoritative_update_with_match(
            build_pos,
            value,
            canonical_authoritative_value,
            source,
            configured_block_outcome,
            configured_block_name,
            |pending, authoritative| pending == authoritative,
        )
    }

    pub fn apply_authoritative_update_with_match<F>(
        &mut self,
        build_pos: i32,
        value: TypeIoObject,
        canonical_authoritative_value: Option<TypeIoObject>,
        source: TileConfigAuthoritySource,
        configured_block_outcome: Option<ConfiguredBlockOutcome>,
        configured_block_name: Option<String>,
        pending_local_matches: F,
    ) -> TileConfigBusinessApply
    where
        F: FnOnce(&TypeIoObject, &TypeIoObject) -> bool,
    {
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

        let canonical_authoritative_value =
            canonical_authoritative_value.unwrap_or_else(|| value.clone());
        let pending_local = self.pop_next_pending_local_request(build_pos);
        let pending_local_match = pending_local
            .as_ref()
            .map(|pending| pending_local_matches(pending, &canonical_authoritative_value));
        let cleared_pending_local = pending_local.is_some();
        let was_rollback = pending_local_match == Some(false);
        if was_rollback {
            self.rollback_count = self.rollback_count.saturating_add(1);
        }

        self.authoritative_by_build_pos
            .insert(build_pos, value.clone());
        self.canonical_authoritative_by_build_pos
            .insert(build_pos, canonical_authoritative_value);
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
        self.clear_all_pending_local_requests(build_pos);
        self.authoritative_by_build_pos
            .insert(build_pos, value.clone());
        self.canonical_authoritative_by_build_pos
            .insert(build_pos, value);
    }

    pub fn clear_pending_local_without_business_apply(
        &mut self,
        build_pos: Option<i32>,
    ) -> TileConfigBusinessApply {
        let pending_local = build_pos.and_then(|value| self.pop_next_pending_local_request(value));
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
        configured_block_outcome: Option<ConfiguredBlockOutcome>,
        configured_block_name: Option<String>,
    ) -> TileConfigBusinessApply {
        self.fallback_rollback_to_known_authority_with_match(
            build_pos,
            source,
            configured_block_outcome,
            configured_block_name,
            |pending, authoritative| pending == authoritative,
        )
    }

    pub fn fallback_rollback_to_known_authority_with_match<F>(
        &mut self,
        build_pos: Option<i32>,
        source: TileConfigAuthoritySource,
        configured_block_outcome: Option<ConfiguredBlockOutcome>,
        configured_block_name: Option<String>,
        pending_local_matches: F,
    ) -> TileConfigBusinessApply
    where
        F: FnOnce(&TypeIoObject, &TypeIoObject) -> bool,
    {
        let Some(build_pos) = build_pos else {
            return self.clear_pending_local_without_business_apply(None);
        };
        let pending_local = self.pop_next_pending_local_request(build_pos);
        let authoritative_value = self
            .canonical_authoritative_by_build_pos
            .get(&build_pos)
            .cloned()
            .or_else(|| self.authoritative_by_build_pos.get(&build_pos).cloned());
        let cleared_pending_local = pending_local.is_some();

        if pending_local.is_none() || authoritative_value.is_none() {
            if pending_local.is_some() && authoritative_value.is_none() {
                self.fallback_missing_authority_count =
                    self.fallback_missing_authority_count.saturating_add(1);
            }
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
            .map(|pending| pending_local_matches(pending, &authoritative_value));
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
            cleared_pending_local: true,
            was_rollback,
            pending_local_match,
            source: Some(source),
            authoritative_value: Some(authoritative_value),
            replaced_local_value: pending_local,
            configured_block_outcome,
            configured_block_name,
        }
    }

    pub fn remove_building_state(&mut self, build_pos: i32) {
        self.clear_all_pending_local_requests(build_pos);
        self.authoritative_by_build_pos.remove(&build_pos);
        self.canonical_authoritative_by_build_pos.remove(&build_pos);
    }

    pub fn clear_for_world_reload(&mut self) {
        self.pending_local_request_queue_by_build_pos.clear();
        self.pending_local_by_build_pos.clear();
        self.authoritative_by_build_pos.clear();
        self.canonical_authoritative_by_build_pos.clear();
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
    pub sorter_runtime_by_build_pos: BTreeMap<i32, SorterRuntimeProjection>,
    pub inverted_sorter_runtime_by_build_pos: BTreeMap<i32, SorterRuntimeProjection>,
    pub switch_enabled_by_build_pos: BTreeMap<i32, Option<bool>>,
    pub door_open_by_build_pos: BTreeMap<i32, Option<bool>>,
    pub message_text_by_build_pos: BTreeMap<i32, String>,
    pub constructor_recipe_block_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub constructor_runtime_by_build_pos: BTreeMap<i32, ConstructorRuntimeProjection>,
    pub light_color_by_build_pos: BTreeMap<i32, i32>,
    pub payload_loader_runtime_by_build_pos: BTreeMap<i32, PayloadLoaderRuntimeProjection>,
    pub payload_source_content_by_build_pos: BTreeMap<i32, Option<ConfiguredContentRef>>,
    pub payload_source_runtime_by_build_pos: BTreeMap<i32, PayloadSourceRuntimeProjection>,
    pub payload_router_sorted_content_by_build_pos: BTreeMap<i32, Option<ConfiguredContentRef>>,
    pub payload_router_runtime_by_build_pos: BTreeMap<i32, PayloadRouterRuntimeProjection>,
    pub item_bridge_link_by_build_pos: BTreeMap<i32, Option<i32>>,
    pub item_bridge_runtime_by_build_pos: BTreeMap<i32, ItemBridgeRuntimeProjection>,
    pub unloader_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub directional_unloader_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub duct_unloader_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub duct_unloader_runtime_by_build_pos: BTreeMap<i32, DuctUnloaderRuntimeProjection>,
    pub duct_router_item_by_build_pos: BTreeMap<i32, Option<i16>>,
    pub mass_driver_link_by_build_pos: BTreeMap<i32, Option<i32>>,
    pub mass_driver_runtime_by_build_pos: BTreeMap<i32, MassDriverRuntimeProjection>,
    pub payload_mass_driver_link_by_build_pos: BTreeMap<i32, Option<i32>>,
    pub payload_mass_driver_runtime_by_build_pos: BTreeMap<i32, PayloadMassDriverRuntimeProjection>,
    pub unit_factory_current_plan_by_build_pos: BTreeMap<i32, i16>,
    pub unit_factory_runtime_by_build_pos: BTreeMap<i32, UnitFactoryRuntimeProjection>,
    pub power_node_links_by_build_pos: BTreeMap<i32, BTreeSet<i32>>,
    pub reconstructor_command_by_build_pos: BTreeMap<i32, Option<u16>>,
    pub reconstructor_runtime_by_build_pos: BTreeMap<i32, ReconstructorRuntimeProjection>,
    pub memory_values_bits_by_build_pos: BTreeMap<i32, Vec<u64>>,
    pub canvas_bytes_by_build_pos: BTreeMap<i32, Vec<u8>>,
    pub unit_assembler_by_build_pos: BTreeMap<i32, UnitAssemblerRuntimeProjection>,
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

    pub fn apply_sorter_runtime(&mut self, build_pos: i32, projection: SorterRuntimeProjection) {
        self.sorter_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_inverted_sorter_runtime(
        &mut self,
        build_pos: i32,
        projection: SorterRuntimeProjection,
    ) {
        self.inverted_sorter_runtime_by_build_pos
            .insert(build_pos, projection);
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

    pub fn apply_constructor_runtime(
        &mut self,
        build_pos: i32,
        projection: ConstructorRuntimeProjection,
    ) {
        self.constructor_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_light_color(&mut self, build_pos: i32, color: i32) {
        self.light_color_by_build_pos.insert(build_pos, color);
    }

    pub fn apply_payload_loader_runtime(
        &mut self,
        build_pos: i32,
        projection: PayloadLoaderRuntimeProjection,
    ) {
        self.payload_loader_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_payload_source_content(
        &mut self,
        build_pos: i32,
        content: Option<ConfiguredContentRef>,
    ) {
        self.payload_source_content_by_build_pos
            .insert(build_pos, content);
    }

    pub fn apply_payload_source_runtime(
        &mut self,
        build_pos: i32,
        projection: PayloadSourceRuntimeProjection,
    ) {
        self.payload_source_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_payload_router_sorted_content(
        &mut self,
        build_pos: i32,
        content: Option<ConfiguredContentRef>,
    ) {
        self.payload_router_sorted_content_by_build_pos
            .insert(build_pos, content);
    }

    pub fn apply_payload_router_runtime(
        &mut self,
        build_pos: i32,
        projection: PayloadRouterRuntimeProjection,
    ) {
        self.payload_router_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_item_bridge_link(&mut self, build_pos: i32, link: Option<i32>) {
        self.item_bridge_link_by_build_pos.insert(build_pos, link);
    }

    pub fn apply_item_bridge_runtime(
        &mut self,
        build_pos: i32,
        projection: ItemBridgeRuntimeProjection,
    ) {
        self.item_bridge_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_unloader_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.unloader_item_by_build_pos.insert(build_pos, item_id);
    }

    pub fn apply_directional_unloader_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.directional_unloader_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_duct_unloader_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.duct_unloader_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_duct_unloader_runtime(
        &mut self,
        build_pos: i32,
        projection: DuctUnloaderRuntimeProjection,
    ) {
        self.duct_unloader_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_duct_router_item(&mut self, build_pos: i32, item_id: Option<i16>) {
        self.duct_router_item_by_build_pos
            .insert(build_pos, item_id);
    }

    pub fn apply_mass_driver_link(&mut self, build_pos: i32, link: Option<i32>) {
        self.mass_driver_link_by_build_pos.insert(build_pos, link);
    }

    pub fn apply_mass_driver_runtime(
        &mut self,
        build_pos: i32,
        projection: MassDriverRuntimeProjection,
    ) {
        self.mass_driver_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_payload_mass_driver_link(&mut self, build_pos: i32, link: Option<i32>) {
        self.payload_mass_driver_link_by_build_pos
            .insert(build_pos, link);
    }

    pub fn apply_payload_mass_driver_runtime(
        &mut self,
        build_pos: i32,
        projection: PayloadMassDriverRuntimeProjection,
    ) {
        self.payload_mass_driver_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_unit_factory_current_plan(&mut self, build_pos: i32, current_plan: i16) {
        self.unit_factory_current_plan_by_build_pos
            .insert(build_pos, current_plan);
    }

    pub fn apply_unit_factory_runtime(
        &mut self,
        build_pos: i32,
        projection: UnitFactoryRuntimeProjection,
    ) {
        self.unit_factory_runtime_by_build_pos
            .insert(build_pos, projection);
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

    pub fn apply_reconstructor_runtime(
        &mut self,
        build_pos: i32,
        projection: ReconstructorRuntimeProjection,
    ) {
        self.reconstructor_runtime_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn apply_memory_values_bits(&mut self, build_pos: i32, values_bits: Vec<u64>) {
        self.memory_values_bits_by_build_pos
            .insert(build_pos, values_bits);
    }

    pub fn apply_canvas_bytes(&mut self, build_pos: i32, bytes: Vec<u8>) {
        self.canvas_bytes_by_build_pos.insert(build_pos, bytes);
    }

    pub fn apply_unit_assembler(
        &mut self,
        build_pos: i32,
        projection: UnitAssemblerRuntimeProjection,
    ) {
        self.unit_assembler_by_build_pos
            .insert(build_pos, projection);
    }

    pub fn clear_building_state(&mut self, build_pos: i32) {
        self.unit_cargo_unload_point_item_by_build_pos
            .remove(&build_pos);
        self.item_source_item_by_build_pos.remove(&build_pos);
        self.liquid_source_liquid_by_build_pos.remove(&build_pos);
        self.landing_pad_item_by_build_pos.remove(&build_pos);
        self.sorter_item_by_build_pos.remove(&build_pos);
        self.inverted_sorter_item_by_build_pos.remove(&build_pos);
        self.sorter_runtime_by_build_pos.remove(&build_pos);
        self.inverted_sorter_runtime_by_build_pos.remove(&build_pos);
        self.switch_enabled_by_build_pos.remove(&build_pos);
        self.door_open_by_build_pos.remove(&build_pos);
        self.message_text_by_build_pos.remove(&build_pos);
        self.constructor_recipe_block_by_build_pos
            .remove(&build_pos);
        self.constructor_runtime_by_build_pos.remove(&build_pos);
        self.light_color_by_build_pos.remove(&build_pos);
        self.payload_loader_runtime_by_build_pos.remove(&build_pos);
        self.payload_source_content_by_build_pos.remove(&build_pos);
        self.payload_source_runtime_by_build_pos.remove(&build_pos);
        self.payload_router_sorted_content_by_build_pos
            .remove(&build_pos);
        self.payload_router_runtime_by_build_pos.remove(&build_pos);
        self.item_bridge_link_by_build_pos.remove(&build_pos);
        self.item_bridge_runtime_by_build_pos.remove(&build_pos);
        self.unloader_item_by_build_pos.remove(&build_pos);
        self.directional_unloader_item_by_build_pos
            .remove(&build_pos);
        self.duct_unloader_item_by_build_pos.remove(&build_pos);
        self.duct_unloader_runtime_by_build_pos.remove(&build_pos);
        self.duct_router_item_by_build_pos.remove(&build_pos);
        self.mass_driver_link_by_build_pos.remove(&build_pos);
        self.mass_driver_runtime_by_build_pos.remove(&build_pos);
        self.payload_mass_driver_link_by_build_pos
            .remove(&build_pos);
        self.payload_mass_driver_runtime_by_build_pos
            .remove(&build_pos);
        self.unit_factory_current_plan_by_build_pos
            .remove(&build_pos);
        self.unit_factory_runtime_by_build_pos.remove(&build_pos);
        self.power_node_links_by_build_pos.remove(&build_pos);
        self.reconstructor_command_by_build_pos.remove(&build_pos);
        self.reconstructor_runtime_by_build_pos.remove(&build_pos);
        self.memory_values_bits_by_build_pos.remove(&build_pos);
        self.canvas_bytes_by_build_pos.remove(&build_pos);
        self.unit_assembler_by_build_pos.remove(&build_pos);
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BuildingTailSummaryProjection {
    pub turret_reload_counter_bits: Option<u32>,
    pub turret_rotation_bits: Option<u32>,
    pub item_turret_ammo_count: Option<u16>,
    pub continuous_turret_last_length_bits: Option<u32>,
    pub build_turret_rotation_bits: Option<u32>,
    pub build_turret_plans_present: Option<bool>,
    pub build_turret_plan_count: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildingProjection {
    pub block_id: Option<i16>,
    pub block_name: Option<String>,
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
    pub turret_reload_counter_bits: Option<u32>,
    pub turret_rotation_bits: Option<u32>,
    pub item_turret_ammo_count: Option<u16>,
    pub continuous_turret_last_length_bits: Option<u32>,
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
    pub last_block_name: Option<String>,
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
    pub last_turret_reload_counter_bits: Option<u32>,
    pub last_turret_rotation_bits: Option<u32>,
    pub last_item_turret_ammo_count: Option<u16>,
    pub last_continuous_turret_last_length_bits: Option<u32>,
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
        block_name: Option<String>,
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
                block_name: preserve_matching_block_name(
                    previous.as_ref(),
                    Some(block_id),
                    block_name,
                ),
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
                turret_reload_counter_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_reload_counter_bits),
                turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_rotation_bits),
                item_turret_ammo_count: previous
                    .as_ref()
                    .and_then(|building| building.item_turret_ammo_count),
                continuous_turret_last_length_bits: previous
                    .as_ref()
                    .and_then(|building| building.continuous_turret_last_length_bits),
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
        block_name: Option<String>,
        rotation: Option<u8>,
        team_id: Option<u8>,
        io_version: Option<u8>,
        module_bitmask: Option<u8>,
        time_scale_bits: Option<u32>,
        time_scale_duration_bits: Option<u32>,
        last_disabler_pos: Option<i32>,
        legacy_consume_connected: Option<bool>,
        config: Option<TypeIoObject>,
        health_bits: Option<u32>,
        enabled: Option<bool>,
        efficiency: Option<u8>,
        optional_efficiency: Option<u8>,
        visible_flags: Option<u64>,
        build_turret_rotation_bits: Option<u32>,
        build_turret_plans_present: Option<bool>,
        build_turret_plan_count: Option<u16>,
    ) {
        self.apply_block_snapshot_head_with_tail_summary(
            build_pos,
            block_id,
            block_name,
            rotation,
            team_id,
            io_version,
            module_bitmask,
            time_scale_bits,
            time_scale_duration_bits,
            last_disabler_pos,
            legacy_consume_connected,
            config,
            health_bits,
            enabled,
            efficiency,
            optional_efficiency,
            visible_flags,
            BuildingTailSummaryProjection {
                build_turret_rotation_bits,
                build_turret_plans_present,
                build_turret_plan_count,
                ..BuildingTailSummaryProjection::default()
            },
        );
    }

    pub fn apply_remote_tile_authority(
        &mut self,
        build_pos: i32,
        block_id: i16,
        block_name: Option<String>,
        rotation: Option<u8>,
        team_id: Option<u8>,
    ) {
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id: Some(block_id),
                block_name: preserve_matching_block_name(
                    previous.as_ref(),
                    Some(block_id),
                    block_name,
                ),
                rotation: rotation
                    .or_else(|| previous.as_ref().and_then(|building| building.rotation)),
                team_id: team_id
                    .or_else(|| previous.as_ref().and_then(|building| building.team_id)),
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
                health_bits: previous.as_ref().and_then(|building| building.health_bits),
                enabled: previous.as_ref().and_then(|building| building.enabled),
                efficiency: previous.as_ref().and_then(|building| building.efficiency),
                optional_efficiency: previous
                    .as_ref()
                    .and_then(|building| building.optional_efficiency),
                visible_flags: previous
                    .as_ref()
                    .and_then(|building| building.visible_flags),
                turret_reload_counter_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_reload_counter_bits),
                turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_rotation_bits),
                item_turret_ammo_count: previous
                    .as_ref()
                    .and_then(|building| building.item_turret_ammo_count),
                continuous_turret_last_length_bits: previous
                    .as_ref()
                    .and_then(|building| building.continuous_turret_last_length_bits),
                build_turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_rotation_bits),
                build_turret_plans_present: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plans_present),
                build_turret_plan_count: previous
                    .as_ref()
                    .and_then(|building| building.build_turret_plan_count),
                last_update: BuildingProjectionUpdateKind::BlockSnapshotHead,
            },
        );
        self.last_block_snapshot_head_conflict = false;
        self.sync_last_mirror_for_apply(
            build_pos,
            BuildingProjectionUpdateKind::BlockSnapshotHead,
            None,
            None,
        );
        self.recount();
    }

    pub fn apply_block_snapshot_head_with_tail_summary(
        &mut self,
        build_pos: i32,
        block_id: i16,
        block_name: Option<String>,
        rotation: Option<u8>,
        team_id: Option<u8>,
        io_version: Option<u8>,
        module_bitmask: Option<u8>,
        time_scale_bits: Option<u32>,
        time_scale_duration_bits: Option<u32>,
        last_disabler_pos: Option<i32>,
        legacy_consume_connected: Option<bool>,
        config: Option<TypeIoObject>,
        health_bits: Option<u32>,
        enabled: Option<bool>,
        efficiency: Option<u8>,
        optional_efficiency: Option<u8>,
        visible_flags: Option<u64>,
        tail_summary: BuildingTailSummaryProjection,
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
            self.last_turret_reload_counter_bits = tail_summary.turret_reload_counter_bits;
            self.last_turret_rotation_bits = tail_summary.turret_rotation_bits;
            self.last_item_turret_ammo_count = tail_summary.item_turret_ammo_count;
            self.last_continuous_turret_last_length_bits =
                tail_summary.continuous_turret_last_length_bits;
            self.last_build_turret_rotation_bits = tail_summary.build_turret_rotation_bits;
            self.last_build_turret_plans_present = tail_summary.build_turret_plans_present;
            self.last_build_turret_plan_count = tail_summary.build_turret_plan_count;
            self.last_removed = false;
            return;
        }
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id: Some(block_id),
                block_name: preserve_matching_block_name(
                    previous.as_ref(),
                    Some(block_id),
                    block_name,
                ),
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
                config: config.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.config.clone())
                }),
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
                turret_reload_counter_bits: tail_summary.turret_reload_counter_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.turret_reload_counter_bits)
                }),
                turret_rotation_bits: tail_summary.turret_rotation_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.turret_rotation_bits)
                }),
                item_turret_ammo_count: tail_summary.item_turret_ammo_count.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.item_turret_ammo_count)
                }),
                continuous_turret_last_length_bits: tail_summary
                    .continuous_turret_last_length_bits
                    .or_else(|| {
                        previous
                            .as_ref()
                            .and_then(|building| building.continuous_turret_last_length_bits)
                    }),
                build_turret_rotation_bits: tail_summary.build_turret_rotation_bits.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.build_turret_rotation_bits)
                }),
                build_turret_plans_present: tail_summary.build_turret_plans_present.or_else(|| {
                    previous
                        .as_ref()
                        .and_then(|building| building.build_turret_plans_present)
                }),
                build_turret_plan_count: tail_summary.build_turret_plan_count.or_else(|| {
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
        block_name: Option<String>,
        rotation: u8,
        team_id: u8,
        config: TypeIoObject,
    ) {
        let previous = self.by_build_pos.get(&build_pos).cloned();
        self.by_build_pos.insert(
            build_pos,
            BuildingProjection {
                block_id,
                block_name: preserve_matching_block_name(previous.as_ref(), block_id, block_name),
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
                turret_reload_counter_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_reload_counter_bits),
                turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_rotation_bits),
                item_turret_ammo_count: previous
                    .as_ref()
                    .and_then(|building| building.item_turret_ammo_count),
                continuous_turret_last_length_bits: previous
                    .as_ref()
                    .and_then(|building| building.continuous_turret_last_length_bits),
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
                block_name: previous
                    .as_ref()
                    .and_then(|building| building.block_name.clone()),
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
                turret_reload_counter_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_reload_counter_bits),
                turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_rotation_bits),
                item_turret_ammo_count: previous
                    .as_ref()
                    .and_then(|building| building.item_turret_ammo_count),
                continuous_turret_last_length_bits: previous
                    .as_ref()
                    .and_then(|building| building.continuous_turret_last_length_bits),
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

    pub fn apply_deconstruct_finish(
        &mut self,
        build_pos: i32,
        block_id: Option<i16>,
        block_name: Option<String>,
    ) {
        let previous = self.by_build_pos.remove(&build_pos);
        self.deconstruct_finish_apply_count = self.deconstruct_finish_apply_count.saturating_add(1);
        self.sync_last_mirror_for_removed(
            build_pos,
            BuildingProjectionUpdateKind::DeconstructFinish,
            block_id,
            block_name,
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
                block_name: previous
                    .as_ref()
                    .and_then(|building| building.block_name.clone()),
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
                turret_reload_counter_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_reload_counter_bits),
                turret_rotation_bits: previous
                    .as_ref()
                    .and_then(|building| building.turret_rotation_bits),
                item_turret_ammo_count: previous
                    .as_ref()
                    .and_then(|building| building.item_turret_ammo_count),
                continuous_turret_last_length_bits: previous
                    .as_ref()
                    .and_then(|building| building.continuous_turret_last_length_bits),
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
        self.last_block_name = building.and_then(|building| building.block_name.clone());
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
        self.last_turret_reload_counter_bits =
            building.and_then(|building| building.turret_reload_counter_bits);
        self.last_turret_rotation_bits =
            building.and_then(|building| building.turret_rotation_bits);
        self.last_item_turret_ammo_count =
            building.and_then(|building| building.item_turret_ammo_count);
        self.last_continuous_turret_last_length_bits =
            building.and_then(|building| building.continuous_turret_last_length_bits);
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
        block_name_override: Option<String>,
        previous: Option<&BuildingProjection>,
    ) {
        self.last_build_pos = Some(build_pos);
        self.last_block_id =
            block_id_override.or_else(|| previous.and_then(|building| building.block_id));
        self.last_block_name = block_name_override
            .or_else(|| previous.and_then(|building| building.block_name.clone()));
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
        self.last_turret_reload_counter_bits =
            previous.and_then(|building| building.turret_reload_counter_bits);
        self.last_turret_rotation_bits =
            previous.and_then(|building| building.turret_rotation_bits);
        self.last_item_turret_ammo_count =
            previous.and_then(|building| building.item_turret_ammo_count);
        self.last_continuous_turret_last_length_bits =
            previous.and_then(|building| building.continuous_turret_last_length_bits);
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

    pub fn typed_runtime_building_at(
        &self,
        build_pos: i32,
        configured: &ConfiguredBlockProjection,
        resource_delta: &ResourceDeltaProjection,
    ) -> Option<TypedBuildingRuntimeModel> {
        let building = self.by_build_pos.get(&build_pos)?;
        typed_runtime_building_model(build_pos, building, configured, resource_delta)
    }

    pub fn typed_runtime_buildings(
        &self,
        configured: &ConfiguredBlockProjection,
        resource_delta: &ResourceDeltaProjection,
    ) -> Vec<TypedBuildingRuntimeModel> {
        self.by_build_pos
            .iter()
            .filter_map(|(build_pos, building)| {
                typed_runtime_building_model(*build_pos, building, configured, resource_delta)
            })
            .collect()
    }
}

fn preserve_matching_block_name(
    previous: Option<&BuildingProjection>,
    block_id: Option<i16>,
    block_name: Option<String>,
) -> Option<String> {
    block_name.or_else(|| match (previous, block_id) {
        (Some(previous), Some(block_id)) if previous.block_id == Some(block_id) => {
            previous.block_name.clone()
        }
        (Some(previous), None) if previous.block_id.is_none() => previous.block_name.clone(),
        _ => None,
    })
}

pub(crate) fn merge_building_projection_with_anchor<F>(
    anchor: &BuildingProjection,
    projection: &BuildingProjection,
    mut resolve_block_name: F,
) -> BuildingProjection
where
    F: FnMut(i16) -> Option<String>,
{
    let block_id = projection.block_id.or(anchor.block_id);
    let block_name = projection.block_name.clone().or_else(|| {
        block_id.and_then(|block_id| {
            if anchor.block_id == Some(block_id) {
                anchor.block_name.clone()
            } else {
                resolve_block_name(block_id)
            }
        })
    });
    BuildingProjection {
        block_id,
        block_name,
        rotation: projection.rotation.or(anchor.rotation),
        team_id: projection.team_id.or(anchor.team_id),
        io_version: projection.io_version.or(anchor.io_version),
        module_bitmask: projection.module_bitmask.or(anchor.module_bitmask),
        time_scale_bits: projection.time_scale_bits.or(anchor.time_scale_bits),
        time_scale_duration_bits: projection
            .time_scale_duration_bits
            .or(anchor.time_scale_duration_bits),
        last_disabler_pos: projection.last_disabler_pos.or(anchor.last_disabler_pos),
        legacy_consume_connected: projection
            .legacy_consume_connected
            .or(anchor.legacy_consume_connected),
        config: projection.config.clone(),
        health_bits: projection.health_bits.or(anchor.health_bits),
        enabled: projection.enabled.or(anchor.enabled),
        efficiency: projection.efficiency.or(anchor.efficiency),
        optional_efficiency: projection
            .optional_efficiency
            .or(anchor.optional_efficiency),
        visible_flags: projection.visible_flags.or(anchor.visible_flags),
        turret_reload_counter_bits: projection
            .turret_reload_counter_bits
            .or(anchor.turret_reload_counter_bits),
        turret_rotation_bits: projection
            .turret_rotation_bits
            .or(anchor.turret_rotation_bits),
        item_turret_ammo_count: projection
            .item_turret_ammo_count
            .or(anchor.item_turret_ammo_count),
        continuous_turret_last_length_bits: projection
            .continuous_turret_last_length_bits
            .or(anchor.continuous_turret_last_length_bits),
        build_turret_rotation_bits: projection
            .build_turret_rotation_bits
            .or(anchor.build_turret_rotation_bits),
        build_turret_plans_present: projection
            .build_turret_plans_present
            .or(anchor.build_turret_plans_present),
        build_turret_plan_count: projection
            .build_turret_plan_count
            .or(anchor.build_turret_plan_count),
        last_update: projection.last_update,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypedBuildingRuntimeKind {
    Core,
    UnitCargoUnloadPoint,
    ItemSource,
    LiquidSource,
    Storage,
    ItemBuffer,
    LandingPad,
    Sorter,
    InvertedSorter,
    Switch,
    Door,
    Processor,
    Message,
    Constructor,
    Illuminator,
    PayloadLoader,
    PayloadSource,
    PayloadRouter,
    ItemBridge,
    Unloader,
    DirectionalUnloader,
    DuctUnloader,
    DuctRouter,
    MassDriver,
    PayloadMassDriver,
    UnitFactory,
    UnitAssembler,
    PowerNode,
    Reconstructor,
    Turret,
    ItemTurret,
    ContinuousTurret,
    BuildTower,
    Memory,
    Canvas,
}

impl TypedBuildingRuntimeKind {
    pub fn family_name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::UnitCargoUnloadPoint => "unit-cargo-unload-point",
            Self::ItemSource => "item-source",
            Self::LiquidSource => "liquid-source",
            Self::Storage => "storage",
            Self::ItemBuffer => "item-buffer",
            Self::LandingPad => "landing-pad",
            Self::Sorter => "sorter",
            Self::InvertedSorter => "inverted-sorter",
            Self::Switch => "switch",
            Self::Door => "door",
            Self::Processor => "processor",
            Self::Message => "message",
            Self::Constructor => "constructor",
            Self::Illuminator => "illuminator",
            Self::PayloadLoader => "payload-loader",
            Self::PayloadSource => "payload-source",
            Self::PayloadRouter => "payload-router",
            Self::ItemBridge => "item-bridge",
            Self::Unloader => "unloader",
            Self::DirectionalUnloader => "directional-unloader",
            Self::DuctUnloader => "duct-unloader",
            Self::DuctRouter => "duct-router",
            Self::MassDriver => "mass-driver",
            Self::PayloadMassDriver => "payload-mass-driver",
            Self::UnitFactory => "unit-factory",
            Self::UnitAssembler => "unit-assembler",
            Self::PowerNode => "power-node",
            Self::Reconstructor => "reconstructor",
            Self::Turret => "turret",
            Self::ItemTurret => "item-turret",
            Self::ContinuousTurret => "continuous-turret",
            Self::BuildTower => "build-tower",
            Self::Memory => "memory",
            Self::Canvas => "canvas",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedBuildingRuntimeValue {
    Core,
    Item(Option<i16>),
    Liquid(Option<i16>),
    Bool(Option<bool>),
    Text(String),
    Constructor {
        recipe_block_id: Option<i16>,
        progress_bits: Option<u32>,
        payload_present: Option<bool>,
        pay_rotation_bits: Option<u32>,
        payload_build_block_id: Option<i16>,
        payload_unit_class_id: Option<u8>,
    },
    PayloadLoader {
        exporting: Option<bool>,
        payload_present: Option<bool>,
        payload_type: Option<u8>,
        pay_rotation_bits: Option<u32>,
        payload_build_block_id: Option<i16>,
        payload_build_revision: Option<u8>,
        payload_unit_class_id: Option<u8>,
        payload_unit_payload_len: Option<usize>,
        payload_unit_payload_sha256: Option<String>,
    },
    PayloadSource {
        configured_content: Option<ConfiguredContentRef>,
        command_pos: Option<(u32, u32)>,
        pay_vector_x_bits: Option<u32>,
        pay_vector_y_bits: Option<u32>,
        pay_rotation_bits: Option<u32>,
        payload_present: Option<bool>,
        payload_type: Option<u8>,
        payload_build_block_id: Option<i16>,
        payload_build_revision: Option<u8>,
        payload_unit_class_id: Option<u8>,
        payload_unit_payload_len: Option<usize>,
        payload_unit_payload_sha256: Option<String>,
    },
    PayloadRouter {
        sorted_content: Option<ConfiguredContentRef>,
        progress_bits: Option<u32>,
        item_rotation_bits: Option<u32>,
        payload_present: Option<bool>,
        payload_type: Option<u8>,
        payload_kind: Option<PayloadRouterPayloadKind>,
        payload_build_block_id: Option<i16>,
        payload_build_revision: Option<u8>,
        payload_unit_class_id: Option<u8>,
        payload_unit_revision: Option<i16>,
        payload_serialized_len: Option<usize>,
        payload_serialized_sha256: Option<String>,
        rec_dir: Option<u8>,
    },
    MassDriver {
        link: Option<i32>,
        rotation_bits: Option<u32>,
        state_ordinal: Option<u8>,
    },
    PayloadMassDriver {
        link: Option<i32>,
        turret_rotation_bits: Option<u32>,
        state_ordinal: Option<u8>,
        reload_counter_bits: Option<u32>,
        charge_bits: Option<u32>,
        loaded: Option<bool>,
        charging: Option<bool>,
        payload_present: Option<bool>,
    },
    Sorter {
        item_id: Option<i16>,
        legacy: Option<bool>,
        non_empty_side_mask: Option<u8>,
        buffered_item_count: Option<u16>,
    },
    ItemBridge {
        link: Option<i32>,
        warmup_bits: Option<u32>,
        incoming_count: Option<usize>,
        moved: Option<bool>,
        buffer_index: Option<i8>,
        buffer_capacity: Option<usize>,
        buffer_normalized_index: Option<i32>,
        buffer_entry_count: Option<usize>,
    },
    DuctUnloader {
        item_id: Option<i16>,
        offset: Option<i16>,
    },
    Block(Option<i16>),
    Color(i32),
    Content(Option<ConfiguredContentRef>),
    Link(Option<i32>),
    Links(BTreeSet<i32>),
    UnitFactory {
        current_plan: Option<i16>,
        progress_bits: Option<u32>,
        command_pos: Option<(u32, u32)>,
        command_id: Option<u8>,
        payload_present: Option<bool>,
        pay_rotation_bits: Option<u32>,
    },
    Reconstructor {
        command_id: Option<u16>,
        progress_bits: Option<u32>,
        command_pos: Option<(u32, u32)>,
        payload_present: Option<bool>,
        pay_rotation_bits: Option<u32>,
    },
    UnitAssembler {
        progress_bits: u32,
        unit_count: usize,
        block_count: usize,
        block_sample: Option<ConfiguredContentRef>,
        command_pos: Option<(u32, u32)>,
        payload_present: bool,
        pay_rotation_bits: u32,
    },
    Turret {
        reload_counter_bits: Option<u32>,
        rotation_bits: Option<u32>,
    },
    ItemTurret {
        reload_counter_bits: Option<u32>,
        rotation_bits: Option<u32>,
        ammo_count: Option<u16>,
    },
    ContinuousTurret {
        reload_counter_bits: Option<u32>,
        rotation_bits: Option<u32>,
        last_length_bits: Option<u32>,
    },
    BuildTower {
        rotation_bits: Option<u32>,
        plans_present: Option<bool>,
        plan_count: Option<u16>,
    },
    Memory(Vec<u64>),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedBuildingRuntimeModel {
    pub build_pos: i32,
    pub block_id: Option<i16>,
    pub block_name: String,
    pub kind: TypedBuildingRuntimeKind,
    pub value: TypedBuildingRuntimeValue,
    pub inventory_item_stacks: Vec<(i16, i32)>,
    pub inventory_liquid_stacks: Vec<(i16, u32)>,
    pub rotation: Option<u8>,
    pub team_id: Option<u8>,
    pub io_version: Option<u8>,
    pub module_bitmask: Option<u8>,
    pub time_scale_bits: Option<u32>,
    pub time_scale_duration_bits: Option<u32>,
    pub last_disabler_pos: Option<i32>,
    pub legacy_consume_connected: Option<bool>,
    pub health_bits: Option<u32>,
    pub enabled: Option<bool>,
    pub efficiency: Option<u8>,
    pub optional_efficiency: Option<u8>,
    pub visible_flags: Option<u64>,
    pub turret_reload_counter_bits: Option<u32>,
    pub turret_rotation_bits: Option<u32>,
    pub item_turret_ammo_count: Option<u16>,
    pub continuous_turret_last_length_bits: Option<u32>,
    pub build_turret_rotation_bits: Option<u32>,
    pub build_turret_plans_present: Option<bool>,
    pub build_turret_plan_count: Option<u16>,
    pub last_update: BuildingProjectionUpdateKind,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypedBuildingRuntimeProjection {
    pub by_build_pos: BTreeMap<i32, TypedBuildingRuntimeModel>,
}

fn build_typed_runtime_model(
    build_pos: i32,
    block_id: Option<i16>,
    block_name: String,
    kind: TypedBuildingRuntimeKind,
    value: TypedBuildingRuntimeValue,
    inventory_item_stacks: Vec<(i16, i32)>,
    inventory_liquid_stacks: Vec<(i16, u32)>,
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
    turret_reload_counter_bits: Option<u32>,
    turret_rotation_bits: Option<u32>,
    item_turret_ammo_count: Option<u16>,
    continuous_turret_last_length_bits: Option<u32>,
    build_turret_rotation_bits: Option<u32>,
    build_turret_plans_present: Option<bool>,
    build_turret_plan_count: Option<u16>,
    last_update: BuildingProjectionUpdateKind,
) -> TypedBuildingRuntimeModel {
    TypedBuildingRuntimeModel {
        build_pos,
        block_id,
        block_name,
        kind,
        value,
        inventory_item_stacks,
        inventory_liquid_stacks,
        rotation,
        team_id,
        io_version,
        module_bitmask,
        time_scale_bits,
        time_scale_duration_bits,
        last_disabler_pos,
        legacy_consume_connected,
        health_bits,
        enabled,
        efficiency,
        optional_efficiency,
        visible_flags,
        turret_reload_counter_bits,
        turret_rotation_bits,
        item_turret_ammo_count,
        continuous_turret_last_length_bits,
        build_turret_rotation_bits,
        build_turret_plans_present,
        build_turret_plan_count,
        last_update,
    }
}

impl TypedBuildingRuntimeProjection {
    pub fn building_at(&self, build_pos: i32) -> Option<&TypedBuildingRuntimeModel> {
        self.by_build_pos.get(&build_pos)
    }

    pub fn buildings(&self) -> impl Iterator<Item = &TypedBuildingRuntimeModel> {
        self.by_build_pos.values()
    }

    pub fn upsert_runtime_building(&mut self, model: TypedBuildingRuntimeModel) {
        self.by_build_pos.insert(model.build_pos, model);
    }

    pub fn remove_runtime_building(&mut self, build_pos: i32) -> bool {
        self.by_build_pos.remove(&build_pos).is_some()
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }
}

fn typed_runtime_building_model(
    build_pos: i32,
    building: &BuildingProjection,
    configured: &ConfiguredBlockProjection,
    resource_delta: &ResourceDeltaProjection,
) -> Option<TypedBuildingRuntimeModel> {
    let block_name = building.block_name.as_deref()?;
    let inventory_item_stacks =
        typed_runtime_building_inventory_item_stacks(build_pos, resource_delta);
    let inventory_liquid_stacks =
        typed_runtime_building_inventory_liquid_stacks(build_pos, resource_delta);
    let (kind, value) = match block_name {
        block_name if block_name.starts_with("core-") => (
            TypedBuildingRuntimeKind::Core,
            TypedBuildingRuntimeValue::Core,
        ),
        "unit-cargo-unload-point" => (
            TypedBuildingRuntimeKind::UnitCargoUnloadPoint,
            TypedBuildingRuntimeValue::Item(
                configured
                    .unit_cargo_unload_point_item_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "item-source" => (
            TypedBuildingRuntimeKind::ItemSource,
            TypedBuildingRuntimeValue::Item(
                configured
                    .item_source_item_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "liquid-source" => (
            TypedBuildingRuntimeKind::LiquidSource,
            TypedBuildingRuntimeValue::Liquid(
                configured
                    .liquid_source_liquid_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "liquid-router"
        | "liquid-junction"
        | "reinforced-liquid-router"
        | "reinforced-liquid-junction"
        | "liquid-container"
        | "liquid-tank"
        | "reinforced-liquid-container"
        | "reinforced-liquid-tank" => (
            TypedBuildingRuntimeKind::LiquidSource,
            TypedBuildingRuntimeValue::Liquid(
                inventory_liquid_stacks
                    .first()
                    .map(|(liquid_id, _)| *liquid_id),
            ),
        ),
        "container" | "vault" | "reinforced-container" | "reinforced-vault" => (
            TypedBuildingRuntimeKind::Storage,
            TypedBuildingRuntimeValue::Item(
                inventory_item_stacks.first().map(|(item_id, _)| *item_id),
            ),
        ),
        "junction" | "router" | "distributor" | "overflow-gate" | "underflow-gate"
        | "surge-router" => (
            TypedBuildingRuntimeKind::ItemBuffer,
            TypedBuildingRuntimeValue::Item(
                inventory_item_stacks.first().map(|(item_id, _)| *item_id),
            ),
        ),
        "landing-pad" => (
            TypedBuildingRuntimeKind::LandingPad,
            TypedBuildingRuntimeValue::Item(
                configured
                    .landing_pad_item_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "sorter" => {
            let runtime = configured.sorter_runtime_by_build_pos.get(&build_pos);
            (
                TypedBuildingRuntimeKind::Sorter,
                TypedBuildingRuntimeValue::Sorter {
                    item_id: configured
                        .sorter_item_by_build_pos
                        .get(&build_pos)
                        .copied()?,
                    legacy: runtime.map(|projection| projection.legacy),
                    non_empty_side_mask: runtime.map(|projection| projection.non_empty_side_mask),
                    buffered_item_count: runtime.map(|projection| projection.buffered_item_count),
                },
            )
        }
        "inverted-sorter" => {
            let runtime = configured
                .inverted_sorter_runtime_by_build_pos
                .get(&build_pos);
            (
                TypedBuildingRuntimeKind::InvertedSorter,
                TypedBuildingRuntimeValue::Sorter {
                    item_id: configured
                        .inverted_sorter_item_by_build_pos
                        .get(&build_pos)
                        .copied()?,
                    legacy: runtime.map(|projection| projection.legacy),
                    non_empty_side_mask: runtime.map(|projection| projection.non_empty_side_mask),
                    buffered_item_count: runtime.map(|projection| projection.buffered_item_count),
                },
            )
        }
        "switch" | "world-switch" => (
            TypedBuildingRuntimeKind::Switch,
            TypedBuildingRuntimeValue::Bool(
                configured
                    .switch_enabled_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "door" | "door-large" => (
            TypedBuildingRuntimeKind::Door,
            TypedBuildingRuntimeValue::Bool(
                configured.door_open_by_build_pos.get(&build_pos).copied()?,
            ),
        ),
        "micro-processor" | "logic-processor" | "hyper-processor" => (
            TypedBuildingRuntimeKind::Processor,
            TypedBuildingRuntimeValue::Text(processor_config_text(building)),
        ),
        "message" | "reinforced-message" | "world-message" => (
            TypedBuildingRuntimeKind::Message,
            TypedBuildingRuntimeValue::Text(
                configured
                    .message_text_by_build_pos
                    .get(&build_pos)
                    .cloned()
                    .unwrap_or_default(),
            ),
        ),
        "constructor" | "large-constructor" => (
            TypedBuildingRuntimeKind::Constructor,
            TypedBuildingRuntimeValue::Constructor {
                recipe_block_id: configured
                    .constructor_recipe_block_by_build_pos
                    .get(&build_pos)
                    .copied()?,
                progress_bits: configured
                    .constructor_runtime_by_build_pos
                    .get(&build_pos)
                    .map(|projection| projection.progress_bits),
                payload_present: configured
                    .constructor_runtime_by_build_pos
                    .get(&build_pos)
                    .map(|projection| projection.payload_present),
                pay_rotation_bits: configured
                    .constructor_runtime_by_build_pos
                    .get(&build_pos)
                    .map(|projection| projection.pay_rotation_bits),
                payload_build_block_id: configured
                    .constructor_runtime_by_build_pos
                    .get(&build_pos)
                    .and_then(|projection| projection.payload_build_block_id),
                payload_unit_class_id: configured
                    .constructor_runtime_by_build_pos
                    .get(&build_pos)
                    .and_then(|projection| projection.payload_unit_class_id),
            },
        ),
        "payload-loader" | "payload-unloader" => {
            let runtime = configured
                .payload_loader_runtime_by_build_pos
                .get(&build_pos);
            (
                TypedBuildingRuntimeKind::PayloadLoader,
                TypedBuildingRuntimeValue::PayloadLoader {
                    exporting: runtime.map(|projection| projection.exporting).or_else(|| {
                        match block_name {
                            "payload-loader" => Some(true),
                            "payload-unloader" => Some(false),
                            _ => None,
                        }
                    }),
                    payload_present: runtime.map(|projection| projection.payload_present),
                    payload_type: runtime.and_then(|projection| projection.payload_type),
                    pay_rotation_bits: runtime.map(|projection| projection.pay_rotation_bits),
                    payload_build_block_id: runtime
                        .and_then(|projection| projection.payload_build_block_id),
                    payload_build_revision: runtime
                        .and_then(|projection| projection.payload_build_revision),
                    payload_unit_class_id: runtime
                        .and_then(|projection| projection.payload_unit_class_id),
                    payload_unit_payload_len: runtime
                        .and_then(|projection| projection.payload_unit_payload_len),
                    payload_unit_payload_sha256: runtime
                        .and_then(|projection| projection.payload_unit_payload_sha256.clone()),
                },
            )
        }
        "illuminator" => (
            TypedBuildingRuntimeKind::Illuminator,
            TypedBuildingRuntimeValue::Color(
                configured
                    .light_color_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "payload-source" => {
            let runtime = configured
                .payload_source_runtime_by_build_pos
                .get(&build_pos);
            (
                TypedBuildingRuntimeKind::PayloadSource,
                TypedBuildingRuntimeValue::PayloadSource {
                    configured_content: configured
                        .payload_source_content_by_build_pos
                        .get(&build_pos)
                        .copied()
                        .flatten(),
                    command_pos: runtime.and_then(|projection| projection.command_pos),
                    pay_vector_x_bits: runtime.map(|projection| projection.pay_vector_x_bits),
                    pay_vector_y_bits: runtime.map(|projection| projection.pay_vector_y_bits),
                    pay_rotation_bits: runtime.map(|projection| projection.pay_rotation_bits),
                    payload_present: runtime.map(|projection| projection.payload_present),
                    payload_type: runtime.and_then(|projection| projection.payload_type),
                    payload_build_block_id: runtime
                        .and_then(|projection| projection.payload_build_block_id),
                    payload_build_revision: runtime
                        .and_then(|projection| projection.payload_build_revision),
                    payload_unit_class_id: runtime
                        .and_then(|projection| projection.payload_unit_class_id),
                    payload_unit_payload_len: runtime
                        .and_then(|projection| projection.payload_unit_payload_len),
                    payload_unit_payload_sha256: runtime
                        .and_then(|projection| projection.payload_unit_payload_sha256.clone()),
                },
            )
        }
        "payload-router" | "reinforced-payload-router" => {
            let runtime = configured
                .payload_router_runtime_by_build_pos
                .get(&build_pos);
            (
                TypedBuildingRuntimeKind::PayloadRouter,
                TypedBuildingRuntimeValue::PayloadRouter {
                    sorted_content: configured
                        .payload_router_sorted_content_by_build_pos
                        .get(&build_pos)
                        .copied()
                        .flatten(),
                    progress_bits: runtime.map(|projection| projection.progress_bits),
                    item_rotation_bits: runtime.map(|projection| projection.item_rotation_bits),
                    payload_present: runtime.map(|projection| projection.payload_present),
                    payload_type: runtime.and_then(|projection| projection.payload_type),
                    payload_kind: runtime.and_then(|projection| projection.payload_kind.clone()),
                    payload_build_block_id: runtime
                        .and_then(|projection| projection.payload_build_block_id),
                    payload_build_revision: runtime
                        .and_then(|projection| projection.payload_build_revision),
                    payload_unit_class_id: runtime
                        .and_then(|projection| projection.payload_unit_class_id),
                    payload_unit_revision: runtime
                        .and_then(|projection| projection.payload_unit_revision),
                    payload_serialized_len: runtime
                        .map(|projection| projection.payload_serialized_len),
                    payload_serialized_sha256: runtime
                        .and_then(|projection| Some(projection.payload_serialized_sha256.clone())),
                    rec_dir: runtime.map(|projection| projection.rec_dir),
                },
            )
        }
        "bridge-conveyor" | "phase-conveyor" | "bridge-conduit" | "phase-conduit" => {
            let link = configured
                .item_bridge_link_by_build_pos
                .get(&build_pos)
                .copied();
            let runtime = configured.item_bridge_runtime_by_build_pos.get(&build_pos);
            if link.is_none() && runtime.is_none() {
                return None;
            }

            (
                TypedBuildingRuntimeKind::ItemBridge,
                TypedBuildingRuntimeValue::ItemBridge {
                    link: link.flatten(),
                    warmup_bits: runtime.map(|projection| projection.warmup_bits),
                    incoming_count: runtime.map(|projection| projection.incoming_count),
                    moved: runtime.map(|projection| projection.moved),
                    buffer_index: runtime.and_then(|projection| {
                        projection.buffer.as_ref().map(|buffer| buffer.index)
                    }),
                    buffer_capacity: runtime.and_then(|projection| {
                        projection.buffer.as_ref().map(|buffer| buffer.capacity)
                    }),
                    buffer_normalized_index: runtime.and_then(|projection| {
                        projection
                            .buffer
                            .as_ref()
                            .map(|buffer| buffer.normalized_index)
                    }),
                    buffer_entry_count: runtime.and_then(|projection| {
                        projection.buffer.as_ref().map(|buffer| buffer.entry_count)
                    }),
                },
            )
        }
        "unloader" => (
            TypedBuildingRuntimeKind::Unloader,
            TypedBuildingRuntimeValue::Item(
                configured
                    .unloader_item_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "directional-unloader" => (
            TypedBuildingRuntimeKind::DirectionalUnloader,
            TypedBuildingRuntimeValue::Item(
                configured
                    .directional_unloader_item_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "duct-unloader" => {
            let item_id = configured
                .duct_unloader_item_by_build_pos
                .get(&build_pos)
                .copied();
            let runtime = configured
                .duct_unloader_runtime_by_build_pos
                .get(&build_pos);
            if item_id.is_none() && runtime.is_none() {
                return None;
            }

            (
                TypedBuildingRuntimeKind::DuctUnloader,
                TypedBuildingRuntimeValue::DuctUnloader {
                    item_id: item_id.flatten(),
                    offset: runtime.map(|projection| projection.offset),
                },
            )
        }
        "duct-router" => (
            TypedBuildingRuntimeKind::DuctRouter,
            TypedBuildingRuntimeValue::Item(
                configured
                    .duct_router_item_by_build_pos
                    .get(&build_pos)
                    .copied()?,
            ),
        ),
        "mass-driver" => {
            let runtime = configured.mass_driver_runtime_by_build_pos.get(&build_pos);
            (
                TypedBuildingRuntimeKind::MassDriver,
                TypedBuildingRuntimeValue::MassDriver {
                    link: configured
                        .mass_driver_link_by_build_pos
                        .get(&build_pos)
                        .copied()?,
                    rotation_bits: runtime.map(|projection| projection.rotation_bits),
                    state_ordinal: runtime.map(|projection| projection.state_ordinal),
                },
            )
        }
        "payload-mass-driver" | "large-payload-mass-driver" => {
            let runtime = configured
                .payload_mass_driver_runtime_by_build_pos
                .get(&build_pos);
            (
                TypedBuildingRuntimeKind::PayloadMassDriver,
                TypedBuildingRuntimeValue::PayloadMassDriver {
                    link: configured
                        .payload_mass_driver_link_by_build_pos
                        .get(&build_pos)
                        .copied()?,
                    turret_rotation_bits: runtime.map(|projection| projection.turret_rotation_bits),
                    state_ordinal: runtime.map(|projection| projection.state_ordinal),
                    reload_counter_bits: runtime.map(|projection| projection.reload_counter_bits),
                    charge_bits: runtime.map(|projection| projection.charge_bits),
                    loaded: runtime.map(|projection| projection.loaded),
                    charging: runtime.map(|projection| projection.charging),
                    payload_present: runtime.map(|projection| projection.payload_present),
                },
            )
        }
        "ground-factory" | "air-factory" | "naval-factory" | "tank-fabricator"
        | "ship-fabricator" | "mech-fabricator" => {
            let runtime = configured.unit_factory_runtime_by_build_pos.get(&build_pos);
            (
                TypedBuildingRuntimeKind::UnitFactory,
                TypedBuildingRuntimeValue::UnitFactory {
                    current_plan: configured
                        .unit_factory_current_plan_by_build_pos
                        .get(&build_pos)
                        .copied(),
                    progress_bits: runtime.map(|projection| projection.progress_bits),
                    command_pos: runtime.and_then(|projection| projection.command_pos),
                    command_id: runtime.and_then(|projection| projection.command_id),
                    payload_present: runtime.map(|projection| projection.payload_present),
                    pay_rotation_bits: runtime.map(|projection| projection.pay_rotation_bits),
                },
            )
        }
        "tank-assembler" | "ship-assembler" | "mech-assembler" => {
            let assembler = configured.unit_assembler_by_build_pos.get(&build_pos)?;
            (
                TypedBuildingRuntimeKind::UnitAssembler,
                TypedBuildingRuntimeValue::UnitAssembler {
                    progress_bits: assembler.progress_bits,
                    unit_count: assembler.unit_ids.len(),
                    block_count: assembler.block_entry_count,
                    block_sample: assembler.block_sample,
                    command_pos: assembler.command_pos,
                    payload_present: assembler.payload_present,
                    pay_rotation_bits: assembler.pay_rotation_bits,
                },
            )
        }
        "power-node" | "power-node-large" | "surge-tower" | "beam-link" => (
            TypedBuildingRuntimeKind::PowerNode,
            TypedBuildingRuntimeValue::Links(
                configured
                    .power_node_links_by_build_pos
                    .get(&build_pos)
                    .cloned()?,
            ),
        ),
        "additive-reconstructor"
        | "multiplicative-reconstructor"
        | "exponential-reconstructor"
        | "tetrative-reconstructor"
        | "tank-refabricator"
        | "ship-refabricator"
        | "mech-refabricator"
        | "prime-refabricator" => {
            let command_id = configured
                .reconstructor_command_by_build_pos
                .get(&build_pos)
                .copied();
            let runtime = configured
                .reconstructor_runtime_by_build_pos
                .get(&build_pos);
            (
                TypedBuildingRuntimeKind::Reconstructor,
                TypedBuildingRuntimeValue::Reconstructor {
                    command_id: command_id.flatten(),
                    progress_bits: runtime.map(|projection| projection.progress_bits),
                    command_pos: runtime.and_then(|projection| projection.command_pos),
                    payload_present: runtime.map(|projection| projection.payload_present),
                    pay_rotation_bits: runtime.map(|projection| projection.pay_rotation_bits),
                },
            )
        }
        "wave" | "tsunami" | "lancer" | "arc" | "meltdown" | "afflict" | "malign" => (
            TypedBuildingRuntimeKind::Turret,
            TypedBuildingRuntimeValue::Turret {
                reload_counter_bits: building.turret_reload_counter_bits,
                rotation_bits: building.turret_rotation_bits,
            },
        ),
        "duo" | "scatter" | "scorch" | "hail" | "swarmer" | "salvo" | "fuse" | "ripple"
        | "cyclone" | "foreshadow" | "spectre" | "breach" | "diffuse" | "titan" | "disperse"
        | "scathe" | "smite" => (
            TypedBuildingRuntimeKind::ItemTurret,
            TypedBuildingRuntimeValue::ItemTurret {
                reload_counter_bits: building.turret_reload_counter_bits,
                rotation_bits: building.turret_rotation_bits,
                ammo_count: building.item_turret_ammo_count,
            },
        ),
        "lustre" | "sublimate" => (
            TypedBuildingRuntimeKind::ContinuousTurret,
            TypedBuildingRuntimeValue::ContinuousTurret {
                reload_counter_bits: building.turret_reload_counter_bits,
                rotation_bits: building.turret_rotation_bits,
                last_length_bits: building.continuous_turret_last_length_bits,
            },
        ),
        "build-tower" => (
            TypedBuildingRuntimeKind::BuildTower,
            TypedBuildingRuntimeValue::BuildTower {
                rotation_bits: building.build_turret_rotation_bits,
                plans_present: building.build_turret_plans_present,
                plan_count: building.build_turret_plan_count,
            },
        ),
        "memory-cell" | "memory-bank" => (
            TypedBuildingRuntimeKind::Memory,
            TypedBuildingRuntimeValue::Memory(
                configured
                    .memory_values_bits_by_build_pos
                    .get(&build_pos)
                    .cloned()?,
            ),
        ),
        "canvas" | "large-canvas" => (
            TypedBuildingRuntimeKind::Canvas,
            TypedBuildingRuntimeValue::Bytes(
                configured
                    .canvas_bytes_by_build_pos
                    .get(&build_pos)
                    .cloned()?,
            ),
        ),
        _ => return None,
    };
    Some(build_typed_runtime_model(
        build_pos,
        building.block_id,
        block_name.to_string(),
        kind,
        value,
        inventory_item_stacks,
        inventory_liquid_stacks,
        building.rotation,
        building.team_id,
        building.io_version,
        building.module_bitmask,
        building.time_scale_bits,
        building.time_scale_duration_bits,
        building.last_disabler_pos,
        building.legacy_consume_connected,
        building.health_bits,
        building.enabled,
        building.efficiency,
        building.optional_efficiency,
        building.visible_flags,
        building.turret_reload_counter_bits,
        building.turret_rotation_bits,
        building.item_turret_ammo_count,
        building.continuous_turret_last_length_bits,
        building.build_turret_rotation_bits,
        building.build_turret_plans_present,
        building.build_turret_plan_count,
        building.last_update,
    ))
}

fn processor_config_text(building: &BuildingProjection) -> String {
    match building.config.as_ref() {
        Some(TypeIoObject::String(Some(text))) => text.clone(),
        Some(TypeIoObject::String(None)) | None => String::new(),
        _ => String::new(),
    }
}

fn typed_runtime_building_inventory_item_stacks(
    build_pos: i32,
    resource_delta: &ResourceDeltaProjection,
) -> Vec<(i16, i32)> {
    resource_delta
        .building_items_by_build
        .get(&build_pos)
        .map(|items| {
            items
                .iter()
                .map(|(&item_id, &amount)| (item_id, amount))
                .collect()
        })
        .unwrap_or_default()
}

fn typed_runtime_building_inventory_liquid_stacks(
    build_pos: i32,
    resource_delta: &ResourceDeltaProjection,
) -> Vec<(i16, u32)> {
    resource_delta
        .building_liquids_by_build
        .get(&build_pos)
        .map(|liquids| {
            liquids
                .iter()
                .map(|(&liquid_id, &amount_bits)| (liquid_id, amount_bits))
                .collect()
        })
        .unwrap_or_default()
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

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePlanSnapshotFirstPlanProjection {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: u8,
    pub config: TypeIoObject,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RemotePlanSnapshotProjection {
    pub received_count: u64,
    pub last_player_id: Option<i32>,
    pub last_group_id: Option<i32>,
    pub last_plan_count: Option<usize>,
    pub last_first_plan: Option<RemotePlanSnapshotFirstPlanProjection>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PingLocationProjection {
    pub received_count: u64,
    pub last_player_id: Option<i32>,
    pub last_x_bits: Option<u32>,
    pub last_y_bits: Option<u32>,
    pub last_text: Option<String>,
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityPlayerSemanticProjection {
    pub admin: bool,
    pub boosting: bool,
    pub color_rgba: u32,
    pub mouse_x_bits: u32,
    pub mouse_y_bits: u32,
    pub name: Option<String>,
    pub selected_block_id: u16,
    pub selected_rotation: u32,
    pub shooting: bool,
    pub team_id: u8,
    pub typing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitySemanticProjection {
    Unit(EntityUnitSemanticProjection),
    Fire(EntityFireSemanticProjection),
    Puddle(EntityPuddleSemanticProjection),
    WeatherState(EntityWeatherStateSemanticProjection),
    WorldLabel(EntityWorldLabelSemanticProjection),
}

impl EntitySemanticProjection {
    fn hidden_snapshot_typed_policy(&self) -> HiddenSnapshotTypedPolicy {
        match self {
            Self::Unit(_) => HiddenSnapshotTypedPolicy::RemoveLikeJavaUnitHandleSyncHidden,
            Self::Fire(_) | Self::Puddle(_) | Self::WeatherState(_) | Self::WorldLabel(_) => {
                HiddenSnapshotTypedPolicy::KeepHidden
            }
        }
    }

    fn hidden_snapshot_runtime_policy(&self) -> HiddenSnapshotRuntimePolicy {
        match self {
            Self::Fire(_) | Self::Puddle(_) | Self::WeatherState(_) => {
                HiddenSnapshotRuntimePolicy::RemoveKnownRuntimeOwned
            }
            Self::Unit(_) | Self::WorldLabel(_) => HiddenSnapshotRuntimePolicy::KeepHidden,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUnitRuntimeSyncProjection {
    pub ammo_bits: u32,
    pub elevation_bits: u32,
    pub flag_bits: u64,
    pub base_rotation_bits: Option<u32>,
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
    pub runtime_sync: Option<EntityUnitRuntimeSyncProjection>,
    pub controller_type: u8,
    pub controller_value: Option<i32>,
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
pub struct PlayerSemanticProjectionTable {
    pub by_entity_id: BTreeMap<i32, EntityPlayerSemanticProjection>,
}

impl PlayerSemanticProjectionTable {
    pub fn upsert(&mut self, entity_id: i32, projection: EntityPlayerSemanticProjection) {
        self.by_entity_id.insert(entity_id, projection);
    }

    pub fn remove_entity(&mut self, entity_id: i32) -> bool {
        self.by_entity_id.remove(&entity_id).is_some()
    }

    pub fn remove_entities<'a>(&mut self, entity_ids: impl IntoIterator<Item = &'a i32>) {
        for entity_id in entity_ids {
            self.by_entity_id.remove(entity_id);
        }
    }

    pub fn clear_for_world_reload(&mut self) {
        self.by_entity_id.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRuntimeEntityBase {
    pub entity_id: i32,
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
pub struct TypedRuntimePlayerEntity {
    pub base: TypedRuntimeEntityBase,
    pub semantic: EntityPlayerSemanticProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRuntimeUnitEntity {
    pub base: TypedRuntimeEntityBase,
    pub semantic: EntityUnitSemanticProjection,
    pub carried_item_stack: Option<ResourceUnitItemStack>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRuntimeFireEntity {
    pub base: TypedRuntimeEntityBase,
    pub semantic: EntityFireSemanticProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRuntimePuddleEntity {
    pub base: TypedRuntimeEntityBase,
    pub semantic: EntityPuddleSemanticProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRuntimeWeatherStateEntity {
    pub base: TypedRuntimeEntityBase,
    pub semantic: EntityWeatherStateSemanticProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRuntimeWorldLabelEntity {
    pub base: TypedRuntimeEntityBase,
    pub semantic: EntityWorldLabelSemanticProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedRuntimeEntityModel {
    Player(TypedRuntimePlayerEntity),
    Unit(TypedRuntimeUnitEntity),
    Fire(TypedRuntimeFireEntity),
    Puddle(TypedRuntimePuddleEntity),
    WeatherState(TypedRuntimeWeatherStateEntity),
    WorldLabel(TypedRuntimeWorldLabelEntity),
}

impl TypedRuntimeEntityModel {
    pub fn base(&self) -> &TypedRuntimeEntityBase {
        match self {
            Self::Player(player) => &player.base,
            Self::Unit(unit) => &unit.base,
            Self::Fire(fire) => &fire.base,
            Self::Puddle(puddle) => &puddle.base,
            Self::WeatherState(weather) => &weather.base,
            Self::WorldLabel(world_label) => &world_label.base,
        }
    }

    pub fn entity_id(&self) -> i32 {
        self.base().entity_id
    }

    pub fn kind(&self) -> TypedRuntimeEntityKind {
        match self {
            Self::Player(_) => TypedRuntimeEntityKind::Player,
            Self::Unit(_) => TypedRuntimeEntityKind::Unit,
            Self::Fire(_) => TypedRuntimeEntityKind::Fire,
            Self::Puddle(_) => TypedRuntimeEntityKind::Puddle,
            Self::WeatherState(_) => TypedRuntimeEntityKind::WeatherState,
            Self::WorldLabel(_) => TypedRuntimeEntityKind::WorldLabel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedRuntimeEntityKind {
    Player,
    Unit,
    Fire,
    Puddle,
    WeatherState,
    WorldLabel,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypedRuntimeEntityProjection {
    pub by_entity_id: BTreeMap<i32, TypedRuntimeEntityModel>,
    pub local_player_entity_id: Option<i32>,
    pub local_player_owned_unit_entity_id: Option<i32>,
    pub player_count: usize,
    pub unit_count: usize,
    pub hidden_count: usize,
    pub player_with_owned_unit_count: usize,
    pub owned_unit_count: usize,
    pub ownership_conflict_count: usize,
    pub ownership_conflict_unit_sample: Vec<i32>,
    pub player_owned_unit_by_player_entity_id: BTreeMap<i32, i32>,
    pub unit_owner_player_by_unit_entity_id: BTreeMap<i32, i32>,
    pub last_entity_id: Option<i32>,
    pub last_player_entity_id: Option<i32>,
    pub last_unit_entity_id: Option<i32>,
}

impl TypedRuntimeEntityProjection {
    pub fn entity_at(&self, entity_id: i32) -> Option<&TypedRuntimeEntityModel> {
        self.by_entity_id.get(&entity_id)
    }

    pub fn local_player(&self) -> Option<&TypedRuntimePlayerEntity> {
        let entity_id = self.local_player_entity_id?;
        match self.by_entity_id.get(&entity_id)? {
            TypedRuntimeEntityModel::Player(player) => Some(player),
            TypedRuntimeEntityModel::Unit(_)
            | TypedRuntimeEntityModel::Fire(_)
            | TypedRuntimeEntityModel::Puddle(_)
            | TypedRuntimeEntityModel::WeatherState(_)
            | TypedRuntimeEntityModel::WorldLabel(_) => None,
        }
    }

    pub fn owned_unit_entity_id_for_player(&self, player_entity_id: i32) -> Option<i32> {
        self.player_owned_unit_by_player_entity_id
            .get(&player_entity_id)
            .copied()
    }

    pub fn owner_player_entity_id_for_unit(&self, unit_entity_id: i32) -> Option<i32> {
        self.unit_owner_player_by_unit_entity_id
            .get(&unit_entity_id)
            .copied()
    }

    pub fn upsert_runtime_entity(&mut self, model: TypedRuntimeEntityModel) {
        self.by_entity_id.insert(model.entity_id(), model);
        self.rebuild_summary();
    }

    pub fn remove_runtime_entity(&mut self, entity_id: i32) -> bool {
        let removed = self.by_entity_id.remove(&entity_id).is_some();
        if removed {
            self.rebuild_summary();
        }
        removed
    }

    pub fn clear_for_world_reload(&mut self) {
        *self = Self::default();
    }

    fn rebuild_summary(&mut self) {
        let mut player_count = 0usize;
        let mut unit_count = 0usize;
        let mut hidden_count = 0usize;
        let mut last_entity = None::<(u64, i32)>;
        let mut last_player = None::<(u64, i32)>;
        let mut last_unit = None::<(u64, i32)>;
        let mut local_player = None::<(u64, i32)>;

        for (&entity_id, model) in &self.by_entity_id {
            let base = model.base();
            let priority = (base.last_seen_entity_snapshot_count, entity_id);
            if base.hidden {
                hidden_count = hidden_count.saturating_add(1);
            }
            if last_entity.is_none_or(|current| priority > current) {
                last_entity = Some(priority);
            }
            match model.kind() {
                TypedRuntimeEntityKind::Player => {
                    player_count = player_count.saturating_add(1);
                    if last_player.is_none_or(|current| priority > current) {
                        last_player = Some(priority);
                    }
                    if base.is_local_player && local_player.is_none_or(|current| priority > current)
                    {
                        local_player = Some(priority);
                    }
                }
                TypedRuntimeEntityKind::Unit => {
                    unit_count = unit_count.saturating_add(1);
                    if last_unit.is_none_or(|current| priority > current) {
                        last_unit = Some(priority);
                    }
                }
                TypedRuntimeEntityKind::Fire
                | TypedRuntimeEntityKind::Puddle
                | TypedRuntimeEntityKind::WeatherState
                | TypedRuntimeEntityKind::WorldLabel => {}
            }
        }

        self.player_count = player_count;
        self.unit_count = unit_count;
        self.hidden_count = hidden_count;
        self.local_player_entity_id = local_player.map(|(_, entity_id)| entity_id);
        let ownership =
            runtime_entity_ownership::resolve_typed_runtime_entity_ownership(&self.by_entity_id);
        self.local_player_owned_unit_entity_id = self
            .local_player_entity_id
            .and_then(|entity_id| {
                ownership
                    .player_owned_unit_by_player_entity_id
                    .get(&entity_id)
            })
            .copied();
        self.player_with_owned_unit_count = ownership.player_owned_unit_by_player_entity_id.len();
        self.owned_unit_count = ownership.unit_owner_player_by_unit_entity_id.len();
        self.ownership_conflict_count = ownership.ownership_conflict_count;
        self.ownership_conflict_unit_sample = ownership.ownership_conflict_unit_sample;
        self.player_owned_unit_by_player_entity_id =
            ownership.player_owned_unit_by_player_entity_id;
        self.unit_owner_player_by_unit_entity_id = ownership.unit_owner_player_by_unit_entity_id;
        self.last_entity_id = last_entity.map(|(_, entity_id)| entity_id);
        self.last_player_entity_id = last_player.map(|(_, entity_id)| entity_id);
        self.last_unit_entity_id = last_unit.map(|(_, entity_id)| entity_id);
    }
}

fn typed_runtime_entity_base(entity_id: i32, entity: &EntityProjection) -> TypedRuntimeEntityBase {
    TypedRuntimeEntityBase {
        entity_id,
        class_id: entity.class_id,
        hidden: entity.hidden,
        is_local_player: entity.is_local_player,
        unit_kind: entity.unit_kind,
        unit_value: entity.unit_value,
        x_bits: entity.x_bits,
        y_bits: entity.y_bits,
        last_seen_entity_snapshot_count: entity.last_seen_entity_snapshot_count,
    }
}

fn typed_runtime_entity_model(
    entity_id: i32,
    entity: &EntityProjection,
    semantic: Option<&EntitySemanticProjectionEntry>,
    player_semantic: Option<&EntityPlayerSemanticProjection>,
    resource_delta: &ResourceDeltaProjection,
) -> Option<TypedRuntimeEntityModel> {
    let base = typed_runtime_entity_base(entity_id, entity);
    match semantic.map(|entry| &entry.projection) {
        Some(EntitySemanticProjection::Unit(unit)) => {
            Some(TypedRuntimeEntityModel::Unit(TypedRuntimeUnitEntity {
                base,
                semantic: unit.clone(),
                carried_item_stack: resource_delta
                    .entity_item_stack_by_entity_id
                    .get(&entity_id)
                    .cloned(),
            }))
        }
        Some(EntitySemanticProjection::Fire(fire)) => {
            Some(TypedRuntimeEntityModel::Fire(TypedRuntimeFireEntity {
                base,
                semantic: fire.clone(),
            }))
        }
        Some(EntitySemanticProjection::Puddle(puddle)) => {
            Some(TypedRuntimeEntityModel::Puddle(TypedRuntimePuddleEntity {
                base,
                semantic: puddle.clone(),
            }))
        }
        Some(EntitySemanticProjection::WeatherState(weather)) => Some(
            TypedRuntimeEntityModel::WeatherState(TypedRuntimeWeatherStateEntity {
                base,
                semantic: weather.clone(),
            }),
        ),
        Some(EntitySemanticProjection::WorldLabel(world_label)) => Some(
            TypedRuntimeEntityModel::WorldLabel(TypedRuntimeWorldLabelEntity {
                base,
                semantic: world_label.clone(),
            }),
        ),
        _ if entity.class_id == EntityTableProjection::LOCAL_PLAYER_CLASS_ID => {
            Some(TypedRuntimeEntityModel::Player(TypedRuntimePlayerEntity {
                base,
                semantic: player_semantic.cloned().unwrap_or_default(),
            }))
        }
        _ => None,
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
        for (&entity_id, entity) in &mut self.by_entity_id {
            entity.hidden = hidden_ids.contains(&entity_id);
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

impl EntityProjection {
    fn hidden_snapshot_typed_policy(&self) -> HiddenSnapshotTypedPolicy {
        if self.is_local_player {
            HiddenSnapshotTypedPolicy::KeepHidden
        } else if class_id_matches_java_unit_handle_sync_hidden_remove(self.class_id) {
            HiddenSnapshotTypedPolicy::RemoveLikeJavaUnitHandleSyncHidden
        } else {
            HiddenSnapshotTypedPolicy::KeepHidden
        }
    }

    fn hidden_snapshot_runtime_policy(&self) -> HiddenSnapshotRuntimePolicy {
        if class_id_matches_known_runtime_owned_hidden_remove(self.class_id) {
            HiddenSnapshotRuntimePolicy::RemoveKnownRuntimeOwned
        } else {
            HiddenSnapshotRuntimePolicy::KeepHidden
        }
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
    pub connect_confirm_flushed: bool,
    pub last_connect_confirm_at_ms: Option<u64>,
    pub last_connect_confirm_flushed_at_ms: Option<u64>,
    pub finish_connecting_commit_count: u64,
    pub last_finish_connecting: Option<FinishConnectingProjection>,
    pub bootstrap_stream_id: Option<i32>,
    pub world_stream_expected_len: usize,
    pub world_stream_received_len: usize,
    pub world_stream_loaded: bool,
    pub world_stream_compressed_len: usize,
    pub world_stream_inflated_len: usize,
    pub world_map_width: usize,
    pub world_map_height: usize,
    pub world_player_id: Option<i32>,
    pub world_player_semantic_projection: Option<EntityPlayerSemanticProjection>,
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
    pub core_inventory_runtime_binding_kind: Option<CoreInventoryRuntimeBindingKind>,
    pub core_inventory_runtime_ambiguous_team_count: usize,
    pub core_inventory_runtime_ambiguous_team_sample: Vec<u8>,
    pub core_inventory_runtime_missing_team_count: usize,
    pub core_inventory_runtime_missing_team_sample: Vec<u8>,
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
    pub last_effect_data_business_hint: Option<EffectDataBusinessHint>,
    pub last_effect_business_projection: Option<EffectBusinessProjection>,
    pub last_effect_business_path: Option<Vec<usize>>,
    pub last_effect_runtime_binding_state: Option<EffectRuntimeBindingState>,
    pub last_effect_runtime_source_binding_state: Option<EffectRuntimeBindingState>,
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
    pub received_wave_advance_signal_count: u64,
    pub last_wave_advance_signal_from: Option<i32>,
    pub last_wave_advance_signal_to: Option<i32>,
    pub last_wave_advance_signal_apply_count: Option<u64>,
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
    pub client_plan_snapshot_projection: RemotePlanSnapshotProjection,
    pub client_plan_snapshot_received_projection: RemotePlanSnapshotProjection,
    pub ping_location_projection: PingLocationProjection,
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
    pub suppress_block_snapshot_head_table_apply: bool,
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
    pub player_semantic_projection: PlayerSemanticProjectionTable,
    pub runtime_typed_entity_apply_projection: TypedRuntimeEntityProjection,
    pub runtime_typed_building_apply_projection: TypedBuildingRuntimeProjection,
}

impl SessionState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn typed_runtime_entity_at(&self, entity_id: i32) -> Option<TypedRuntimeEntityModel> {
        let entity = self.entity_table_projection.by_entity_id.get(&entity_id)?;
        typed_runtime_entity_model(
            entity_id,
            entity,
            self.entity_semantic_projection.by_entity_id.get(&entity_id),
            self.player_semantic_projection.by_entity_id.get(&entity_id),
            &self.resource_delta_projection,
        )
    }

    pub fn typed_runtime_entities(&self) -> Vec<TypedRuntimeEntityModel> {
        self.entity_table_projection
            .by_entity_id
            .iter()
            .filter_map(|(entity_id, entity)| {
                typed_runtime_entity_model(
                    *entity_id,
                    entity,
                    self.entity_semantic_projection.by_entity_id.get(entity_id),
                    self.player_semantic_projection.by_entity_id.get(entity_id),
                    &self.resource_delta_projection,
                )
            })
            .collect()
    }

    pub fn typed_runtime_building_at(&self, build_pos: i32) -> Option<TypedBuildingRuntimeModel> {
        let building = self
            .building_table_projection
            .by_build_pos
            .get(&build_pos)?;
        typed_runtime_building_model(
            build_pos,
            building,
            &self.configured_block_projection,
            &self.resource_delta_projection,
        )
    }

    pub fn typed_runtime_buildings(&self) -> Vec<TypedBuildingRuntimeModel> {
        self.building_table_projection
            .by_build_pos
            .iter()
            .filter_map(|(build_pos, building)| {
                typed_runtime_building_model(
                    *build_pos,
                    building,
                    &self.configured_block_projection,
                    &self.resource_delta_projection,
                )
            })
            .collect()
    }

    pub fn typed_runtime_building_from_projection(
        &self,
        build_pos: i32,
        building: &BuildingProjection,
    ) -> Option<TypedBuildingRuntimeModel> {
        typed_runtime_building_model(
            build_pos,
            building,
            &self.configured_block_projection,
            &self.resource_delta_projection,
        )
    }

    pub fn typed_runtime_entity_projection(&self) -> TypedRuntimeEntityProjection {
        let mut projection = TypedRuntimeEntityProjection::default();
        for model in self.typed_runtime_entities() {
            projection.by_entity_id.insert(model.entity_id(), model);
        }
        projection.rebuild_summary();
        projection
    }

    pub fn typed_runtime_building_projection(&self) -> TypedBuildingRuntimeProjection {
        let mut projection = TypedBuildingRuntimeProjection::default();
        for model in self.typed_runtime_buildings() {
            projection.by_build_pos.insert(model.build_pos, model);
        }
        projection
    }

    pub fn runtime_typed_entity_projection(&self) -> TypedRuntimeEntityProjection {
        if self
            .runtime_typed_entity_apply_projection
            .by_entity_id
            .is_empty()
            && !self.entity_table_projection.by_entity_id.is_empty()
        {
            self.typed_runtime_entity_projection()
        } else {
            self.runtime_typed_entity_apply_projection.clone()
        }
    }

    pub fn runtime_typed_building_projection(&self) -> TypedBuildingRuntimeProjection {
        if self
            .runtime_typed_building_apply_projection
            .by_build_pos
            .is_empty()
            && !self.building_table_projection.by_build_pos.is_empty()
        {
            self.typed_runtime_building_projection()
        } else {
            self.runtime_typed_building_apply_projection.clone()
        }
    }

    pub fn refresh_runtime_typed_entity_from_tables(&mut self, entity_id: i32) {
        let model = self
            .entity_table_projection
            .by_entity_id
            .get(&entity_id)
            .and_then(|entity| {
                typed_runtime_entity_model(
                    entity_id,
                    entity,
                    self.entity_semantic_projection.by_entity_id.get(&entity_id),
                    self.player_semantic_projection.by_entity_id.get(&entity_id),
                    &self.resource_delta_projection,
                )
            });
        match model {
            Some(model) => self
                .runtime_typed_entity_apply_projection
                .upsert_runtime_entity(model),
            None => {
                self.runtime_typed_entity_apply_projection
                    .remove_runtime_entity(entity_id);
            }
        }
    }

    pub fn refresh_runtime_typed_building_from_tables(&mut self, build_pos: i32) {
        let model = self
            .building_table_projection
            .by_build_pos
            .get(&build_pos)
            .and_then(|building| {
                typed_runtime_building_model(
                    build_pos,
                    building,
                    &self.configured_block_projection,
                    &self.resource_delta_projection,
                )
            });
        match model {
            Some(model) => self
                .runtime_typed_building_apply_projection
                .upsert_runtime_building(model),
            None => {
                self.runtime_typed_building_apply_projection
                    .remove_runtime_building(build_pos);
            }
        }
    }

    pub fn rebuild_runtime_typed_entity_projection_from_tables(&mut self) {
        let mut projection = TypedRuntimeEntityProjection::default();
        for (&entity_id, entity) in &self.entity_table_projection.by_entity_id {
            let model = typed_runtime_entity_model(
                entity_id,
                entity,
                self.entity_semantic_projection.by_entity_id.get(&entity_id),
                self.player_semantic_projection.by_entity_id.get(&entity_id),
                &self.resource_delta_projection,
            );
            if let Some(model) = model {
                projection.by_entity_id.insert(entity_id, model);
            }
        }
        projection.rebuild_summary();
        self.runtime_typed_entity_apply_projection = projection;
    }

    pub fn rebuild_runtime_typed_building_projection_from_tables(&mut self) {
        let mut projection = TypedBuildingRuntimeProjection::default();
        for model in self.typed_runtime_buildings() {
            projection.by_build_pos.insert(model.build_pos, model);
        }
        self.runtime_typed_building_apply_projection = projection;
    }

    pub fn remove_runtime_typed_entity(&mut self, entity_id: i32) -> bool {
        self.runtime_typed_entity_apply_projection
            .remove_runtime_entity(entity_id)
    }

    pub fn remove_runtime_typed_building(&mut self, build_pos: i32) -> bool {
        self.runtime_typed_building_apply_projection
            .remove_runtime_building(build_pos)
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

    pub fn record_finish_connecting(&mut self, projection: FinishConnectingProjection) {
        self.finish_connecting_commit_count = self.finish_connecting_commit_count.saturating_add(1);
        self.last_finish_connecting = Some(projection);
    }

    pub fn clear_wave_advance_signal(&mut self) {
        self.received_wave_advance_signal_count = 0;
        self.last_wave_advance_signal_from = None;
        self.last_wave_advance_signal_to = None;
        self.last_wave_advance_signal_apply_count = None;
    }

    pub fn clear_runtime_ui_transients_for_world_reload(&mut self) {
        self.received_server_message_count = 0;
        self.last_server_message = None;
        self.received_chat_message_count = 0;
        self.last_chat_message = None;
        self.last_chat_unformatted = None;
        self.last_chat_sender_entity_id = None;

        self.received_set_hud_text_count = 0;
        self.last_set_hud_text_message = None;
        self.received_set_hud_text_reliable_count = 0;
        self.last_set_hud_text_reliable_message = None;
        self.received_hide_hud_text_count = 0;
        self.received_announce_count = 0;
        self.last_announce_message = None;

        self.received_world_label_count = 0;
        self.received_world_label_reliable_count = 0;
        self.last_world_label_reliable = None;
        self.last_world_label_id = None;
        self.last_world_label_message = None;
        self.last_world_label_duration_bits = None;
        self.last_world_label_world_x_bits = None;
        self.last_world_label_world_y_bits = None;
        self.received_remove_world_label_count = 0;
        self.last_remove_world_label_id = None;

        self.received_create_marker_count = 0;
        self.received_remove_marker_count = 0;
        self.received_update_marker_count = 0;
        self.received_update_marker_text_count = 0;
        self.received_update_marker_texture_count = 0;
        self.failed_marker_decode_count = 0;
        self.last_failed_marker_method = None;
        self.last_failed_marker_payload_len = None;
        self.last_marker_id = None;
        self.last_marker_json_len = None;
        self.last_marker_control = None;
        self.last_marker_control_name = None;
        self.last_marker_p1_bits = None;
        self.last_marker_p2_bits = None;
        self.last_marker_p3_bits = None;
        self.last_marker_fetch = None;
        self.last_marker_text = None;
        self.last_marker_texture_kind = None;
        self.last_marker_texture_kind_name = None;

        self.received_info_message_count = 0;
        self.last_info_message = None;
        self.received_info_popup_count = 0;
        self.received_info_popup_reliable_count = 0;
        self.last_info_popup_reliable = None;
        self.last_info_popup_id = None;
        self.last_info_popup_message = None;
        self.last_info_popup_duration_bits = None;
        self.last_info_popup_align = None;
        self.last_info_popup_top = None;
        self.last_info_popup_left = None;
        self.last_info_popup_bottom = None;
        self.last_info_popup_right = None;
        self.received_info_toast_count = 0;
        self.last_info_toast_message = None;
        self.last_info_toast_duration_bits = None;
        self.received_warning_toast_count = 0;
        self.last_warning_toast_unicode = None;
        self.last_warning_toast_text = None;

        self.received_menu_open_count = 0;
        self.last_menu_open_id = None;
        self.last_menu_open_title = None;
        self.last_menu_open_message = None;
        self.last_menu_open_option_rows = 0;
        self.last_menu_open_first_row_len = 0;
        self.received_follow_up_menu_open_count = 0;
        self.last_follow_up_menu_open_id = None;
        self.last_follow_up_menu_open_title = None;
        self.last_follow_up_menu_open_message = None;
        self.last_follow_up_menu_open_option_rows = 0;
        self.last_follow_up_menu_open_first_row_len = 0;
        self.received_hide_follow_up_menu_count = 0;
        self.last_hide_follow_up_menu_id = None;
        self.received_copy_to_clipboard_count = 0;
        self.last_copy_to_clipboard_text = None;
        self.received_open_uri_count = 0;
        self.last_open_uri = None;
        self.received_text_input_count = 0;
        self.last_text_input_id = None;
        self.last_text_input_title = None;
        self.last_text_input_message = None;
        self.last_text_input_length = None;
        self.last_text_input_default_text = None;
        self.last_text_input_numeric = None;
        self.last_text_input_allow_empty = None;
    }

    pub fn record_wave_advance_signal(
        &mut self,
        from: Option<i32>,
        to: Option<i32>,
        apply_count: u64,
    ) {
        self.received_wave_advance_signal_count =
            self.received_wave_advance_signal_count.saturating_add(1);
        self.last_wave_advance_signal_from = from;
        self.last_wave_advance_signal_to = to;
        self.last_wave_advance_signal_apply_count = Some(apply_count);
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

    pub fn clear_entity_snapshot_tombstone(&mut self, entity_id: i32) -> bool {
        self.entity_snapshot_tombstones.remove(&entity_id).is_some()
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
        let typed_runtime_transition = HiddenSnapshotTypedRuntimeTransition {
            refresh_ids: trigger_hidden_ids
                .iter()
                .chain(removed_ids.iter())
                .copied()
                .collect(),
        };

        self.applied_hidden_snapshot_count = self.applied_hidden_snapshot_count.saturating_add(1);
        self.last_hidden_snapshot = Some(applied);
        self.entity_table_projection
            .apply_hidden_ids(&trigger_hidden_ids);
        let local_player_entity_id = self.entity_table_projection.local_player_entity_id;
        let runtime_transition =
            self.hidden_snapshot_runtime_transition(&trigger_hidden_ids, local_player_entity_id);
        let hidden_removed_ids = self
            .apply_hidden_snapshot_runtime_transition(&runtime_transition, local_player_entity_id);
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
        self.apply_hidden_snapshot_typed_runtime_transition(&typed_runtime_transition);
    }

    fn hidden_snapshot_runtime_transition(
        &self,
        trigger_hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) -> HiddenSnapshotRuntimeTransition {
        HiddenSnapshotRuntimeTransition {
            auxiliary_cleanup_ids: self
                .hidden_snapshot_auxiliary_cleanup_ids(trigger_hidden_ids, local_player_entity_id),
            unit_handle_sync_hidden_remove_ids: self
                .hidden_snapshot_unit_handle_sync_hidden_remove_ids(
                    trigger_hidden_ids,
                    local_player_entity_id,
                ),
            runtime_owned_cleanup_remove_ids: self
                .hidden_snapshot_runtime_owned_cleanup_remove_ids(
                    trigger_hidden_ids,
                    local_player_entity_id,
                ),
        }
    }

    fn apply_hidden_snapshot_runtime_transition(
        &mut self,
        transition: &HiddenSnapshotRuntimeTransition,
        local_player_entity_id: Option<i32>,
    ) -> Vec<i32> {
        let mut hidden_removed_ids = self
            .entity_table_projection
            .remove_hidden_entities(&transition.unit_handle_sync_hidden_remove_ids);
        hidden_removed_ids.extend(
            self.entity_table_projection
                .remove_hidden_entities(&transition.runtime_owned_cleanup_remove_ids),
        );
        hidden_removed_ids.sort_unstable();
        hidden_removed_ids.dedup();
        let lifecycle_remove_ids = transition.lifecycle_remove_ids();
        self.entity_semantic_projection
            .remove_hidden_entities(&lifecycle_remove_ids, local_player_entity_id);
        self.player_semantic_projection
            .remove_entities(&hidden_removed_ids);
        self.resource_delta_projection
            .remove_hidden_entities(&transition.auxiliary_cleanup_ids, local_player_entity_id);
        self.resource_delta_projection
            .clear_hidden_entity_refs(&transition.auxiliary_cleanup_ids, local_player_entity_id);
        self.payload_lifecycle_projection
            .remove_hidden_entities(&transition.auxiliary_cleanup_ids, local_player_entity_id);
        self.clear_hidden_resource_and_payload_event_refs(
            &transition.auxiliary_cleanup_ids,
            local_player_entity_id,
        );
        for entity_id in &hidden_removed_ids {
            self.clear_entity_snapshot_tombstone(*entity_id);
        }
        hidden_removed_ids
    }

    fn apply_hidden_snapshot_typed_runtime_transition(
        &mut self,
        transition: &HiddenSnapshotTypedRuntimeTransition,
    ) {
        // Reassert the full current hidden-id set so repeated hidden snapshots can suppress stale
        // runtime-owned rows that were reintroduced by other apply paths.
        self.seed_runtime_typed_entity_apply_projection_from_tables_if_empty();
        for entity_id in &transition.refresh_ids {
            self.refresh_runtime_typed_entity_from_tables(*entity_id);
        }
    }

    fn seed_runtime_typed_entity_apply_projection_from_tables_if_empty(&mut self) {
        if self
            .runtime_typed_entity_apply_projection
            .by_entity_id
            .is_empty()
            && !self.entity_table_projection.by_entity_id.is_empty()
        {
            self.rebuild_runtime_typed_entity_projection_from_tables();
        }
    }

    fn hidden_snapshot_auxiliary_cleanup_ids(
        &self,
        trigger_hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) -> BTreeSet<i32> {
        trigger_hidden_ids
            .iter()
            .copied()
            .filter(|entity_id| Some(*entity_id) != local_player_entity_id)
            .collect()
    }

    fn hidden_snapshot_unit_handle_sync_hidden_remove_ids(
        &self,
        trigger_hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) -> BTreeSet<i32> {
        trigger_hidden_ids
            .iter()
            .copied()
            .filter(|entity_id| {
                Some(*entity_id) != local_player_entity_id
                    && self.hidden_snapshot_matches_java_unit_handle_sync_hidden(*entity_id)
            })
            .collect()
    }

    fn hidden_snapshot_runtime_owned_cleanup_remove_ids(
        &self,
        trigger_hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) -> BTreeSet<i32> {
        trigger_hidden_ids
            .iter()
            .copied()
            .filter(|entity_id| {
                Some(*entity_id) != local_player_entity_id
                    && resolve_hidden_snapshot_entity_policy(
                        self.entity_table_projection.by_entity_id.get(entity_id),
                        self.entity_semantic_projection
                            .by_entity_id
                            .get(entity_id)
                            .map(|entry| &entry.projection),
                    )
                    .should_remove_known_runtime_owned()
            })
            .collect()
    }

    fn hidden_snapshot_matches_java_unit_handle_sync_hidden(&self, entity_id: i32) -> bool {
        self.entity_table_projection
            .by_entity_id
            .get(&entity_id)
            .is_some_and(|entity| {
                class_id_matches_java_unit_handle_sync_hidden_remove(entity.class_id)
            })
            || self
                .entity_semantic_projection
                .by_entity_id
                .get(&entity_id)
                .is_some_and(|entry| {
                    class_id_matches_java_unit_handle_sync_hidden_remove(entry.class_id)
                })
    }

    fn clear_hidden_resource_and_payload_event_refs(
        &mut self,
        hidden_ids: &BTreeSet<i32>,
        local_player_entity_id: Option<i32>,
    ) {
        if let Some(projection) = self.last_take_items.as_mut() {
            clear_hidden_non_local_unit_ref(&mut projection.to, hidden_ids, local_player_entity_id);
        }
        if let Some(projection) = self.last_transfer_item_to.as_mut() {
            clear_hidden_non_local_unit_ref(
                &mut projection.unit,
                hidden_ids,
                local_player_entity_id,
            );
        }
        if let Some(projection) = self.last_transfer_item_to_unit.as_mut() {
            clear_hidden_non_local_entity_id(
                &mut projection.to_entity_id,
                hidden_ids,
                local_player_entity_id,
            );
        }
        if let Some(projection) = self.last_transfer_item_effect.as_mut() {
            clear_hidden_non_local_entity_id(
                &mut projection.to_entity_id,
                hidden_ids,
                local_player_entity_id,
            );
        }
        if let Some(projection) = self.last_payload_dropped.as_mut() {
            clear_hidden_non_local_unit_ref(
                &mut projection.unit,
                hidden_ids,
                local_player_entity_id,
            );
        }
        if let Some(projection) = self.last_picked_build_payload.as_mut() {
            clear_hidden_non_local_unit_ref(
                &mut projection.unit,
                hidden_ids,
                local_player_entity_id,
            );
        }
        if let Some(projection) = self.last_picked_unit_payload.as_mut() {
            clear_hidden_non_local_unit_ref(
                &mut projection.unit,
                hidden_ids,
                local_player_entity_id,
            );
            clear_hidden_non_local_unit_ref(
                &mut projection.target,
                hidden_ids,
                local_player_entity_id,
            );
        }
        if let Some(projection) = self.last_unit_entered_payload.as_mut() {
            clear_hidden_non_local_unit_ref(
                &mut projection.unit,
                hidden_ids,
                local_player_entity_id,
            );
        }
        clear_hidden_non_local_unit_ref(
            &mut self.last_unit_control_target,
            hidden_ids,
            local_player_entity_id,
        );
        clear_hidden_non_local_unit_ref(
            &mut self.last_unit_building_control_select_target,
            hidden_ids,
            local_player_entity_id,
        );
        clear_hidden_non_local_unit_ref(
            &mut self.last_command_units_unit_target,
            hidden_ids,
            local_player_entity_id,
        );
        clear_hidden_non_local_unit_ref(
            &mut self.last_request_unit_payload_target,
            hidden_ids,
            local_player_entity_id,
        );
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

    pub fn record_set_liquid_resource_delta(
        &mut self,
        build_pos: Option<i32>,
        liquid_id: Option<i16>,
        amount_bits: u32,
    ) {
        self.resource_delta_projection
            .apply_set_liquid(build_pos, liquid_id, amount_bits);
    }

    pub fn record_set_liquids_resource_delta(
        &mut self,
        build_pos: Option<i32>,
        stacks: &[(Option<i16>, u32)],
    ) {
        self.resource_delta_projection
            .apply_set_liquids(build_pos, stacks);
    }

    pub fn record_replace_build_items_resource_delta(
        &mut self,
        build_pos: Option<i32>,
        stacks: &[(i16, i32)],
    ) {
        self.resource_delta_projection
            .replace_build_items_exact(build_pos, stacks);
    }

    pub fn record_replace_build_liquids_resource_delta(
        &mut self,
        build_pos: Option<i32>,
        stacks: &[(i16, u32)],
    ) {
        self.resource_delta_projection
            .replace_build_liquids_exact(build_pos, stacks);
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

    pub fn record_set_tile_liquids_resource_delta(
        &mut self,
        liquid_id: Option<i16>,
        amount_bits: u32,
        positions: &[i32],
    ) {
        self.resource_delta_projection
            .apply_set_tile_liquids(liquid_id, amount_bits, positions);
    }

    pub fn record_clear_items_resource_delta(&mut self, build_pos: Option<i32>) {
        self.resource_delta_projection.clear_build_items(build_pos);
    }

    pub fn record_clear_liquids_resource_delta(&mut self, build_pos: Option<i32>) {
        self.resource_delta_projection
            .clear_build_liquids(build_pos);
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
    use mdt_typeio::pack_point2;

    fn expected_typed_runtime_building(
        build_pos: i32,
        block_id: i16,
        block_name: &str,
        kind: TypedBuildingRuntimeKind,
        value: TypedBuildingRuntimeValue,
        inventory_item_stacks: Vec<(i16, i32)>,
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
        turret_reload_counter_bits: Option<u32>,
        turret_rotation_bits: Option<u32>,
        item_turret_ammo_count: Option<u16>,
        continuous_turret_last_length_bits: Option<u32>,
        build_turret_rotation_bits: Option<u32>,
        build_turret_plans_present: Option<bool>,
        build_turret_plan_count: Option<u16>,
        last_update: BuildingProjectionUpdateKind,
    ) -> TypedBuildingRuntimeModel {
        build_typed_runtime_model(
            build_pos,
            Some(block_id),
            block_name.to_string(),
            kind,
            value,
            inventory_item_stacks,
            Vec::new(),
            rotation,
            team_id,
            io_version,
            module_bitmask,
            time_scale_bits,
            time_scale_duration_bits,
            last_disabler_pos,
            legacy_consume_connected,
            health_bits,
            enabled,
            efficiency,
            optional_efficiency,
            visible_flags,
            turret_reload_counter_bits,
            turret_rotation_bits,
            item_turret_ammo_count,
            continuous_turret_last_length_bits,
            build_turret_rotation_bits,
            build_turret_plans_present,
            build_turret_plan_count,
            last_update,
        )
    }

    fn test_building_projection(last_update: BuildingProjectionUpdateKind) -> BuildingProjection {
        BuildingProjection {
            block_id: None,
            block_name: None,
            rotation: None,
            team_id: None,
            io_version: None,
            module_bitmask: None,
            time_scale_bits: None,
            time_scale_duration_bits: None,
            last_disabler_pos: None,
            legacy_consume_connected: None,
            config: None,
            health_bits: None,
            enabled: None,
            efficiency: None,
            optional_efficiency: None,
            visible_flags: None,
            turret_reload_counter_bits: None,
            turret_rotation_bits: None,
            item_turret_ammo_count: None,
            continuous_turret_last_length_bits: None,
            build_turret_rotation_bits: None,
            build_turret_plans_present: None,
            build_turret_plan_count: None,
            last_update,
        }
    }

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
            Some("payload-router".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(123),
            Some(true),
            Some(TypeIoObject::Bool(true)),
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
        assert_eq!(building.config, Some(TypeIoObject::Bool(true)));
        assert_eq!(table.last_build_turret_rotation_bits, Some(0x4260_0000));
        assert_eq!(table.last_build_turret_plans_present, Some(true));
        assert_eq!(table.last_build_turret_plan_count, Some(7));
        assert_eq!(table.last_config, Some(TypeIoObject::Bool(true)));
        assert_eq!(building.block_name.as_deref(), Some("payload-router"));
        assert_eq!(table.last_block_name.as_deref(), Some("payload-router"));

        table.apply_construct_finish(build_pos, Some(300), None, 1, 2, TypeIoObject::Int(9));
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
        assert_eq!(
            building_after_construct.block_name.as_deref(),
            Some("payload-router")
        );
    }

    #[test]
    fn building_projection_merge_helper_prefers_live_fields_over_anchor() {
        let anchor = BuildingProjection {
            block_id: Some(10),
            block_name: Some("anchor-block".to_string()),
            rotation: Some(1),
            team_id: Some(2),
            io_version: Some(3),
            module_bitmask: Some(4),
            time_scale_bits: Some(5),
            time_scale_duration_bits: Some(6),
            last_disabler_pos: Some(7),
            legacy_consume_connected: Some(false),
            config: Some(TypeIoObject::Int(1)),
            health_bits: Some(8),
            enabled: Some(false),
            efficiency: Some(9),
            optional_efficiency: Some(10),
            visible_flags: Some(11),
            turret_reload_counter_bits: Some(12),
            turret_rotation_bits: Some(13),
            item_turret_ammo_count: Some(14),
            continuous_turret_last_length_bits: Some(15),
            build_turret_rotation_bits: Some(16),
            build_turret_plans_present: Some(false),
            build_turret_plan_count: Some(17),
            last_update: BuildingProjectionUpdateKind::WorldBaseline,
        };
        let live = BuildingProjection {
            block_id: Some(20),
            block_name: None,
            rotation: Some(21),
            team_id: Some(22),
            io_version: Some(23),
            module_bitmask: Some(24),
            time_scale_bits: Some(25),
            time_scale_duration_bits: Some(26),
            last_disabler_pos: Some(27),
            legacy_consume_connected: Some(true),
            config: Some(TypeIoObject::Int(2)),
            health_bits: Some(28),
            enabled: Some(true),
            efficiency: Some(29),
            optional_efficiency: Some(30),
            visible_flags: Some(31),
            turret_reload_counter_bits: Some(32),
            turret_rotation_bits: Some(33),
            item_turret_ammo_count: Some(34),
            continuous_turret_last_length_bits: Some(35),
            build_turret_rotation_bits: Some(36),
            build_turret_plans_present: Some(true),
            build_turret_plan_count: Some(37),
            last_update: BuildingProjectionUpdateKind::TileConfig,
        };

        let merged = merge_building_projection_with_anchor(&anchor, &live, |block_id| {
            (block_id == 20).then(|| "resolved-live-block".to_string())
        });

        assert_eq!(merged.block_id, Some(20));
        assert_eq!(merged.block_name.as_deref(), Some("resolved-live-block"));
        assert_eq!(merged.rotation, Some(21));
        assert_eq!(merged.team_id, Some(22));
        assert_eq!(merged.io_version, Some(23));
        assert_eq!(merged.module_bitmask, Some(24));
        assert_eq!(merged.time_scale_bits, Some(25));
        assert_eq!(merged.time_scale_duration_bits, Some(26));
        assert_eq!(merged.last_disabler_pos, Some(27));
        assert_eq!(merged.legacy_consume_connected, Some(true));
        assert_eq!(merged.config, Some(TypeIoObject::Int(2)));
        assert_eq!(merged.health_bits, Some(28));
        assert_eq!(merged.enabled, Some(true));
        assert_eq!(merged.efficiency, Some(29));
        assert_eq!(merged.optional_efficiency, Some(30));
        assert_eq!(merged.visible_flags, Some(31));
        assert_eq!(merged.turret_reload_counter_bits, Some(32));
        assert_eq!(merged.turret_rotation_bits, Some(33));
        assert_eq!(merged.item_turret_ammo_count, Some(34));
        assert_eq!(merged.continuous_turret_last_length_bits, Some(35));
        assert_eq!(merged.build_turret_rotation_bits, Some(36));
        assert_eq!(merged.build_turret_plans_present, Some(true));
        assert_eq!(merged.build_turret_plan_count, Some(37));
        assert_eq!(merged.last_update, BuildingProjectionUpdateKind::TileConfig);
    }

    #[test]
    fn building_projection_merge_helper_falls_back_to_anchor_for_missing_live_fields() {
        let anchor = BuildingProjection {
            block_id: Some(10),
            block_name: Some("anchor-block".to_string()),
            rotation: Some(1),
            team_id: Some(2),
            io_version: Some(3),
            module_bitmask: Some(4),
            time_scale_bits: Some(5),
            time_scale_duration_bits: Some(6),
            last_disabler_pos: Some(7),
            legacy_consume_connected: Some(true),
            config: Some(TypeIoObject::Int(1)),
            health_bits: Some(8),
            enabled: Some(false),
            efficiency: Some(9),
            optional_efficiency: Some(10),
            visible_flags: Some(11),
            turret_reload_counter_bits: Some(12),
            turret_rotation_bits: Some(13),
            item_turret_ammo_count: Some(14),
            continuous_turret_last_length_bits: Some(15),
            build_turret_rotation_bits: Some(16),
            build_turret_plans_present: Some(true),
            build_turret_plan_count: Some(17),
            last_update: BuildingProjectionUpdateKind::WorldBaseline,
        };
        let mut live = test_building_projection(BuildingProjectionUpdateKind::BuildHealthUpdate);
        live.block_id = Some(10);

        let merged = merge_building_projection_with_anchor(&anchor, &live, |_| {
            panic!("resolver should not run when anchor block metadata is reusable")
        });

        assert_eq!(merged.block_id, Some(10));
        assert_eq!(merged.block_name.as_deref(), Some("anchor-block"));
        assert_eq!(merged.rotation, Some(1));
        assert_eq!(merged.team_id, Some(2));
        assert_eq!(merged.io_version, Some(3));
        assert_eq!(merged.module_bitmask, Some(4));
        assert_eq!(merged.time_scale_bits, Some(5));
        assert_eq!(merged.time_scale_duration_bits, Some(6));
        assert_eq!(merged.last_disabler_pos, Some(7));
        assert_eq!(merged.legacy_consume_connected, Some(true));
        assert_eq!(merged.config, None);
        assert_eq!(merged.health_bits, Some(8));
        assert_eq!(merged.enabled, Some(false));
        assert_eq!(merged.efficiency, Some(9));
        assert_eq!(merged.optional_efficiency, Some(10));
        assert_eq!(merged.visible_flags, Some(11));
        assert_eq!(merged.turret_reload_counter_bits, Some(12));
        assert_eq!(merged.turret_rotation_bits, Some(13));
        assert_eq!(merged.item_turret_ammo_count, Some(14));
        assert_eq!(merged.continuous_turret_last_length_bits, Some(15));
        assert_eq!(merged.build_turret_rotation_bits, Some(16));
        assert_eq!(merged.build_turret_plans_present, Some(true));
        assert_eq!(merged.build_turret_plan_count, Some(17));
        assert_eq!(
            merged.last_update,
            BuildingProjectionUpdateKind::BuildHealthUpdate
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_tracks_base_tail_fields() {
        let mut state = SessionState::default();
        let build_pos = 0x0005_0007i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            300,
            Some("message".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(123),
            Some(true),
            Some(TypeIoObject::String(Some("ignored".to_string()))),
            Some(0x4000_0000),
            Some(false),
            Some(0x40),
            Some(0x20),
            Some(99),
            Some(0x4260_0000),
            Some(true),
            Some(7),
        );
        state
            .configured_block_projection
            .apply_message_text(build_pos, "hello".to_string());
        state
            .resource_delta_projection
            .seed_world_build_items(build_pos, &[(4, 12), (6, 0), (7, 3)]);
        state
            .resource_delta_projection
            .seed_world_build_liquids(build_pos, &[(5, 1.25f32.to_bits()), (8, 0.0f32.to_bits())]);

        let mut expected = expected_typed_runtime_building(
            build_pos,
            300,
            "message",
            TypedBuildingRuntimeKind::Message,
            TypedBuildingRuntimeValue::Text("hello".to_string()),
            vec![(4, 12), (7, 3)],
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(123),
            Some(true),
            Some(0x4000_0000),
            Some(false),
            Some(0x40),
            Some(0x20),
            Some(99),
            None,
            None,
            None,
            None,
            Some(0x4260_0000),
            Some(true),
            Some(7),
            BuildingProjectionUpdateKind::BlockSnapshotHead,
        );
        expected.inventory_liquid_stacks = vec![(5, 1.25f32.to_bits())];
        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected.clone())
        );
        assert_eq!(
            state
                .runtime_typed_building_projection()
                .building_at(build_pos),
            Some(&expected)
        );

        state.refresh_runtime_typed_building_from_tables(build_pos);
        assert_eq!(
            state
                .runtime_typed_building_apply_projection
                .building_at(build_pos),
            Some(&expected)
        );

        state
            .configured_block_projection
            .clear_building_state(build_pos);
        state.refresh_runtime_typed_building_from_tables(build_pos);
        let mut expected_shell = expected.clone();
        expected_shell.value = TypedBuildingRuntimeValue::Text(String::new());
        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_shell.clone())
        );
        assert_eq!(
            state
                .runtime_typed_building_apply_projection
                .building_at(build_pos),
            Some(&expected_shell)
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_processor_family_shells() {
        for (build_pos, block_name, code) in [
            (0x0005_0012i32, "micro-processor", "print(\"micro\")"),
            (0x0005_0013i32, "logic-processor", "print(\"logic\")"),
            (0x0005_0014i32, "hyper-processor", "print(\"hyper\")"),
        ] {
            let mut state = SessionState::default();
            state.building_table_projection.apply_block_snapshot_head(
                build_pos,
                350,
                Some(block_name.to_string()),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(TypeIoObject::String(Some(code.to_string()))),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
            );

            assert_eq!(
                state.typed_runtime_building_at(build_pos),
                Some(expected_typed_runtime_building(
                    build_pos,
                    350,
                    block_name,
                    TypedBuildingRuntimeKind::Processor,
                    TypedBuildingRuntimeValue::Text(code.to_string()),
                    Vec::new(),
                    Some(1),
                    Some(2),
                    Some(3),
                    Some(4),
                    Some(0x3f80_0000),
                    Some(0x3f00_0000),
                    Some(126),
                    Some(false),
                    Some(0x40a0_0000),
                    Some(true),
                    Some(0x50),
                    Some(0x28),
                    Some(66),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    BuildingProjectionUpdateKind::BlockSnapshotHead,
                ))
            );
        }
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_message_family_empty_string_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0005_0017i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            353,
            Some("message".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(127),
            Some(true),
            None,
            Some(0x40a0_0000),
            Some(false),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                353,
                "message",
                TypedBuildingRuntimeKind::Message,
                TypedBuildingRuntimeValue::Text(String::new()),
                Vec::new(),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(127),
                Some(true),
                Some(0x40a0_0000),
                Some(false),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_processors_empty_string_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0005_0015i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            351,
            Some("logic-processor".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            None,
            None,
            None,
            None,
            Some(TypeIoObject::String(None)),
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                351,
                "logic-processor",
                TypedBuildingRuntimeKind::Processor,
                TypedBuildingRuntimeValue::Text(String::new()),
                Vec::new(),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
                None,
                None,
                None,
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_refresh_runtime_typed_building_updates_processor_text_from_building_config() {
        let mut state = SessionState::default();
        let build_pos = 0x0005_0016i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            352,
            Some("micro-processor".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            None,
            None,
            None,
            None,
            Some(TypeIoObject::String(Some("print(\"old\")".to_string()))),
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state.refresh_runtime_typed_building_from_tables(build_pos);
        assert_eq!(
            state
                .runtime_typed_building_apply_projection
                .building_at(build_pos),
            Some(&expected_typed_runtime_building(
                build_pos,
                352,
                "micro-processor",
                TypedBuildingRuntimeKind::Processor,
                TypedBuildingRuntimeValue::Text("print(\"old\")".to_string()),
                Vec::new(),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
                None,
                None,
                None,
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );

        state.building_table_projection.apply_tile_config(
            build_pos,
            TypeIoObject::String(Some("print(\"new\")".to_string())),
        );
        state.refresh_runtime_typed_building_from_tables(build_pos);

        assert_eq!(
            state
                .runtime_typed_building_apply_projection
                .building_at(build_pos),
            Some(&expected_typed_runtime_building(
                build_pos,
                352,
                "micro-processor",
                TypedBuildingRuntimeKind::Processor,
                TypedBuildingRuntimeValue::Text("print(\"new\")".to_string()),
                Vec::new(),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
                None,
                None,
                None,
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::TileConfig,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_reconstructor_family_empty_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000bi32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            304,
            Some("prime-refabricator".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                304,
                "prime-refabricator",
                TypedBuildingRuntimeKind::Reconstructor,
                TypedBuildingRuntimeValue::Reconstructor {
                    command_id: None,
                    progress_bits: None,
                    command_pos: None,
                    payload_present: None,
                    pay_rotation_bits: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_liquid_source_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0005_0008i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            300,
            Some("liquid-source".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(122),
            Some(true),
            Some(TypeIoObject::Null),
            Some(0x4000_0000),
            Some(false),
            Some(0x40),
            Some(0x20),
            Some(98),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_liquid_source_liquid(build_pos, Some(9));
        state
            .resource_delta_projection
            .seed_world_build_liquids(build_pos, &[(6, 0.5f32.to_bits()), (7, 0.0f32.to_bits())]);

        let mut expected = expected_typed_runtime_building(
            build_pos,
            300,
            "liquid-source",
            TypedBuildingRuntimeKind::LiquidSource,
            TypedBuildingRuntimeValue::Liquid(Some(9)),
            Vec::new(),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(122),
            Some(true),
            Some(0x4000_0000),
            Some(false),
            Some(0x40),
            Some(0x20),
            Some(98),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            BuildingProjectionUpdateKind::BlockSnapshotHead,
        );
        expected.inventory_liquid_stacks = vec![(6, 0.5f32.to_bits())];

        assert_eq!(state.typed_runtime_building_at(build_pos), Some(expected));
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_liquid_bridge_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0006_0008i32;
        let target_pos = 0x0006_000ci32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            301,
            Some("phase-conduit".to_string()),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(124),
            Some(false),
            Some(TypeIoObject::Null),
            Some(0x4040_0000),
            Some(true),
            Some(0x30),
            Some(0x10),
            Some(88),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_item_bridge_link(build_pos, Some(target_pos));

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                301,
                "phase-conduit",
                TypedBuildingRuntimeKind::ItemBridge,
                TypedBuildingRuntimeValue::ItemBridge {
                    link: Some(target_pos),
                    warmup_bits: None,
                    incoming_count: None,
                    moved: None,
                    buffer_index: None,
                    buffer_capacity: None,
                    buffer_normalized_index: None,
                    buffer_entry_count: None,
                },
                Vec::new(),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(124),
                Some(false),
                Some(0x4040_0000),
                Some(true),
                Some(0x30),
                Some(0x10),
                Some(88),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_buffered_item_bridge_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0006_0010i32;
        let target_pos = 0x0006_0016i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            302,
            Some("bridge-conveyor".to_string()),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f40_0000),
            Some(0x3f10_0000),
            Some(126),
            Some(true),
            Some(TypeIoObject::Null),
            Some(0x4080_0000),
            Some(false),
            Some(0x31),
            Some(0x11),
            Some(89),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_item_bridge_link(build_pos, Some(target_pos));
        state.configured_block_projection.apply_item_bridge_runtime(
            build_pos,
            ItemBridgeRuntimeProjection {
                warmup_bits: 0x3f00_0000,
                incoming_count: 2,
                moved: true,
                buffer: Some(ItemBridgeBufferRuntimeProjection {
                    index: 1,
                    capacity: 4,
                    normalized_index: 1,
                    entry_count: 2,
                }),
            },
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                302,
                "bridge-conveyor",
                TypedBuildingRuntimeKind::ItemBridge,
                TypedBuildingRuntimeValue::ItemBridge {
                    link: Some(target_pos),
                    warmup_bits: Some(0x3f00_0000),
                    incoming_count: Some(2),
                    moved: Some(true),
                    buffer_index: Some(1),
                    buffer_capacity: Some(4),
                    buffer_normalized_index: Some(1),
                    buffer_entry_count: Some(2),
                },
                Vec::new(),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f40_0000),
                Some(0x3f10_0000),
                Some(126),
                Some(true),
                Some(0x4080_0000),
                Some(false),
                Some(0x31),
                Some(0x11),
                Some(89),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_sorter_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0006_0020i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            305,
            Some("sorter".to_string()),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f80_0000),
            Some(0x3f20_0000),
            Some(127),
            Some(false),
            Some(TypeIoObject::Null),
            Some(0x4080_0000),
            Some(true),
            Some(0x44),
            Some(0x12),
            Some(85),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_sorter_item(build_pos, Some(31));
        state.configured_block_projection.apply_sorter_runtime(
            build_pos,
            SorterRuntimeProjection {
                legacy: true,
                non_empty_side_mask: 0x05,
                buffered_item_count: 3,
            },
        );

        assert_eq!(
            state
                .typed_runtime_building_at(build_pos)
                .map(|building| (building.kind, building.value.clone())),
            Some((
                TypedBuildingRuntimeKind::Sorter,
                TypedBuildingRuntimeValue::Sorter {
                    item_id: Some(31),
                    legacy: Some(true),
                    non_empty_side_mask: Some(0x05),
                    buffered_item_count: Some(3),
                },
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_inverted_sorter_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0006_0021i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            306,
            Some("inverted-sorter".to_string()),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f80_0000),
            Some(0x3f20_0000),
            Some(127),
            Some(false),
            Some(TypeIoObject::Null),
            Some(0x4080_0000),
            Some(true),
            Some(0x44),
            Some(0x12),
            Some(85),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_inverted_sorter_item(build_pos, None);
        state
            .configured_block_projection
            .apply_inverted_sorter_runtime(
                build_pos,
                SorterRuntimeProjection {
                    legacy: true,
                    non_empty_side_mask: 0x02,
                    buffered_item_count: 1,
                },
            );

        assert_eq!(
            state
                .typed_runtime_building_at(build_pos)
                .map(|building| (building.kind, building.value.clone())),
            Some((
                TypedBuildingRuntimeKind::InvertedSorter,
                TypedBuildingRuntimeValue::Sorter {
                    item_id: None,
                    legacy: Some(true),
                    non_empty_side_mask: Some(0x02),
                    buffered_item_count: Some(1),
                },
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_duct_unloader_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0006_0022i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            307,
            Some("duct-unloader".to_string()),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f80_0000),
            Some(0x3f20_0000),
            Some(128),
            Some(true),
            Some(TypeIoObject::Null),
            Some(0x4080_0000),
            Some(false),
            Some(0x45),
            Some(0x13),
            Some(84),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_duct_unloader_item(build_pos, Some(41));
        state
            .configured_block_projection
            .apply_duct_unloader_runtime(build_pos, DuctUnloaderRuntimeProjection { offset: 11 });

        assert_eq!(
            state
                .typed_runtime_building_at(build_pos)
                .map(|building| (building.kind, building.value.clone())),
            Some((
                TypedBuildingRuntimeKind::DuctUnloader,
                TypedBuildingRuntimeValue::DuctUnloader {
                    item_id: Some(41),
                    offset: Some(11),
                },
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_liquid_family_shells() {
        for (build_pos, block_name, liquid_id) in [
            (0x0006_000ei32, "liquid-router", 11),
            (0x0006_000fi32, "liquid-junction", 12),
            (0x0006_0010i32, "reinforced-liquid-router", 13),
            (0x0006_0011i32, "reinforced-liquid-junction", 14),
            (0x0006_0012i32, "liquid-container", 15),
            (0x0006_0013i32, "liquid-tank", 16),
            (0x0006_0014i32, "reinforced-liquid-container", 17),
            (0x0006_0015i32, "reinforced-liquid-tank", 18),
        ] {
            let mut state = SessionState::default();
            state.building_table_projection.apply_block_snapshot_head(
                build_pos,
                302,
                Some(block_name.to_string()),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f20_0000),
                Some(125),
                Some(false),
                Some(TypeIoObject::Null),
                Some(0x4080_0000),
                Some(true),
                Some(0x44),
                Some(0x12),
                Some(87),
                None,
                None,
                None,
            );
            state
                .resource_delta_projection
                .seed_world_build_liquids(build_pos, &[(liquid_id, 0.5f32.to_bits())]);

            let mut expected = expected_typed_runtime_building(
                build_pos,
                302,
                block_name,
                TypedBuildingRuntimeKind::LiquidSource,
                TypedBuildingRuntimeValue::Liquid(Some(liquid_id)),
                Vec::new(),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f20_0000),
                Some(125),
                Some(false),
                Some(0x4080_0000),
                Some(true),
                Some(0x44),
                Some(0x12),
                Some(87),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            );
            expected.inventory_liquid_stacks = vec![(liquid_id, 0.5f32.to_bits())];

            assert_eq!(state.typed_runtime_building_at(build_pos), Some(expected));
        }
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_storage_family_shells() {
        for (build_pos, block_name, item_stacks, first_item_id) in [
            (
                0x0006_0016i32,
                "container",
                vec![(21, 7), (22, 1)],
                Some(21),
            ),
            (0x0006_0017i32, "vault", vec![(23, 9)], Some(23)),
            (0x0006_0018i32, "reinforced-container", Vec::new(), None),
            (0x0006_0019i32, "reinforced-vault", vec![(24, 11)], Some(24)),
        ] {
            let mut state = SessionState::default();
            state.building_table_projection.apply_block_snapshot_head(
                build_pos,
                303,
                Some(block_name.to_string()),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f20_0000),
                Some(126),
                Some(false),
                Some(TypeIoObject::Null),
                Some(0x4080_0000),
                Some(true),
                Some(0x44),
                Some(0x12),
                Some(86),
                None,
                None,
                None,
            );
            state
                .resource_delta_projection
                .seed_world_build_items(build_pos, &item_stacks);

            let expected = expected_typed_runtime_building(
                build_pos,
                303,
                block_name,
                TypedBuildingRuntimeKind::Storage,
                TypedBuildingRuntimeValue::Item(first_item_id),
                item_stacks,
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f20_0000),
                Some(126),
                Some(false),
                Some(0x4080_0000),
                Some(true),
                Some(0x44),
                Some(0x12),
                Some(86),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            );

            assert_eq!(state.typed_runtime_building_at(build_pos), Some(expected));
        }
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_item_buffer_family_shells() {
        for (build_pos, block_name, item_stacks, first_item_id) in [
            (0x0006_001ai32, "junction", vec![(31, 2)], Some(31)),
            (0x0006_001bi32, "router", vec![(32, 4), (33, 1)], Some(32)),
            (0x0006_001ci32, "distributor", Vec::new(), None),
            (0x0006_001di32, "overflow-gate", vec![(34, 3)], Some(34)),
            (0x0006_001ei32, "underflow-gate", vec![(35, 5)], Some(35)),
            (0x0006_001fi32, "surge-router", vec![(36, 6)], Some(36)),
        ] {
            let mut state = SessionState::default();
            state.building_table_projection.apply_block_snapshot_head(
                build_pos,
                304,
                Some(block_name.to_string()),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f20_0000),
                Some(127),
                Some(false),
                Some(TypeIoObject::Null),
                Some(0x4080_0000),
                Some(true),
                Some(0x44),
                Some(0x12),
                Some(85),
                None,
                None,
                None,
            );
            state
                .resource_delta_projection
                .seed_world_build_items(build_pos, &item_stacks);

            let expected = expected_typed_runtime_building(
                build_pos,
                304,
                block_name,
                TypedBuildingRuntimeKind::ItemBuffer,
                TypedBuildingRuntimeValue::Item(first_item_id),
                item_stacks,
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f20_0000),
                Some(127),
                Some(false),
                Some(0x4080_0000),
                Some(true),
                Some(0x44),
                Some(0x12),
                Some(85),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            );

            assert_eq!(state.typed_runtime_building_at(build_pos), Some(expected));
        }
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_power_node_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0006_0009i32;
        let links = BTreeSet::from([0x0006_000di32, 0x0007_0009i32]);
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            302,
            Some("power-node".to_string()),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f80_0000),
            Some(0x3f20_0000),
            Some(125),
            Some(true),
            Some(TypeIoObject::Null),
            Some(0x4080_0000),
            Some(false),
            Some(0x44),
            Some(0x12),
            Some(87),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_power_node_links_full_replace(build_pos, links.clone());

        let expected = expected_typed_runtime_building(
            build_pos,
            302,
            "power-node",
            TypedBuildingRuntimeKind::PowerNode,
            TypedBuildingRuntimeValue::Links(links),
            Vec::new(),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f80_0000),
            Some(0x3f20_0000),
            Some(125),
            Some(true),
            Some(0x4080_0000),
            Some(false),
            Some(0x44),
            Some(0x12),
            Some(87),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            BuildingProjectionUpdateKind::BlockSnapshotHead,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected.clone())
        );
        state.refresh_runtime_typed_building_from_tables(build_pos);
        assert_eq!(
            state
                .runtime_typed_building_apply_projection
                .building_at(build_pos),
            Some(&expected)
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_memory_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0007_0009i32;
        let values_bits = vec![1.5f64.to_bits(), (-2.25f64).to_bits(), f64::NAN.to_bits()];
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            302,
            Some("memory-cell".to_string()),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(125),
            Some(true),
            None,
            Some(0x4080_0000),
            Some(true),
            Some(0x20),
            Some(0x10),
            Some(77),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_memory_values_bits(build_pos, values_bits.clone());

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                302,
                "memory-cell",
                TypedBuildingRuntimeKind::Memory,
                TypedBuildingRuntimeValue::Memory(values_bits),
                Vec::new(),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(125),
                Some(true),
                Some(0x4080_0000),
                Some(true),
                Some(0x20),
                Some(0x10),
                Some(77),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_constructor_family_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000ai32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            303,
            Some("constructor".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_constructor_recipe_block(build_pos, Some(7));
        state.configured_block_projection.apply_constructor_runtime(
            build_pos,
            ConstructorRuntimeProjection {
                progress_bits: 0x3f40_0000,
                payload_present: true,
                pay_rotation_bits: 0x4000_0000,
                payload_build_block_id: Some(11),
                payload_unit_class_id: None,
            },
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                303,
                "constructor",
                TypedBuildingRuntimeKind::Constructor,
                TypedBuildingRuntimeValue::Constructor {
                    recipe_block_id: Some(7),
                    progress_bits: Some(0x3f40_0000),
                    payload_present: Some(true),
                    pay_rotation_bits: Some(0x4000_0000),
                    payload_build_block_id: Some(11),
                    payload_unit_class_id: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_payload_loader_family_empty_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000ci32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            304,
            Some("payload-unloader".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                304,
                "payload-unloader",
                TypedBuildingRuntimeKind::PayloadLoader,
                TypedBuildingRuntimeValue::PayloadLoader {
                    exporting: Some(false),
                    payload_present: None,
                    payload_type: None,
                    pay_rotation_bits: None,
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: None,
                    payload_unit_payload_len: None,
                    payload_unit_payload_sha256: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_payload_loader_family_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000di32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            305,
            Some("payload-loader".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_payload_loader_runtime(
                build_pos,
                PayloadLoaderRuntimeProjection {
                    exporting: true,
                    payload_present: true,
                    payload_type: Some(1),
                    pay_rotation_bits: 0x4000_0000,
                    payload_build_block_id: Some(11),
                    payload_build_revision: Some(2),
                    payload_unit_class_id: None,
                    payload_unit_payload_len: None,
                    payload_unit_payload_sha256: None,
                },
            );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                305,
                "payload-loader",
                TypedBuildingRuntimeKind::PayloadLoader,
                TypedBuildingRuntimeValue::PayloadLoader {
                    exporting: Some(true),
                    payload_present: Some(true),
                    payload_type: Some(1),
                    pay_rotation_bits: Some(0x4000_0000),
                    payload_build_block_id: Some(11),
                    payload_build_revision: Some(2),
                    payload_unit_class_id: None,
                    payload_unit_payload_len: None,
                    payload_unit_payload_sha256: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_mass_driver_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000ei32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            306,
            Some("mass-driver".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_mass_driver_link(build_pos, Some(11));
        state.configured_block_projection.apply_mass_driver_runtime(
            build_pos,
            MassDriverRuntimeProjection {
                rotation_bits: 0x4120_0000,
                state_ordinal: 2,
            },
        );

        assert_eq!(
            state
                .typed_runtime_building_at(build_pos)
                .map(|building| (building.kind, building.value.clone())),
            Some((
                TypedBuildingRuntimeKind::MassDriver,
                TypedBuildingRuntimeValue::MassDriver {
                    link: Some(11),
                    rotation_bits: Some(0x4120_0000),
                    state_ordinal: Some(2),
                },
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_payload_mass_driver_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000fi32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            307,
            Some("large-payload-mass-driver".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_payload_mass_driver_link(build_pos, Some(13));
        state
            .configured_block_projection
            .apply_payload_mass_driver_runtime(
                build_pos,
                PayloadMassDriverRuntimeProjection {
                    turret_rotation_bits: 0x4140_0000,
                    state_ordinal: 3,
                    reload_counter_bits: 0x3f20_0000,
                    charge_bits: 0x3f40_0000,
                    loaded: true,
                    charging: false,
                    payload_present: true,
                },
            );

        assert_eq!(
            state
                .typed_runtime_building_at(build_pos)
                .map(|building| (building.kind, building.value.clone())),
            Some((
                TypedBuildingRuntimeKind::PayloadMassDriver,
                TypedBuildingRuntimeValue::PayloadMassDriver {
                    link: Some(13),
                    turret_rotation_bits: Some(0x4140_0000),
                    state_ordinal: Some(3),
                    reload_counter_bits: Some(0x3f20_0000),
                    charge_bits: Some(0x3f40_0000),
                    loaded: Some(true),
                    charging: Some(false),
                    payload_present: Some(true),
                },
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_payload_source_family_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_0012i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            308,
            Some("payload-source".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                306,
                "payload-source",
                TypedBuildingRuntimeKind::PayloadSource,
                TypedBuildingRuntimeValue::PayloadSource {
                    configured_content: None,
                    command_pos: None,
                    pay_vector_x_bits: None,
                    pay_vector_y_bits: None,
                    pay_rotation_bits: None,
                    payload_present: None,
                    payload_type: None,
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: None,
                    payload_unit_payload_len: None,
                    payload_unit_payload_sha256: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_payload_source_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_0013i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            309,
            Some("payload-source".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_payload_source_content(
                build_pos,
                Some(ConfiguredContentRef {
                    content_type: 1,
                    content_id: 11,
                }),
            );
        state
            .configured_block_projection
            .apply_payload_source_runtime(
                build_pos,
                PayloadSourceRuntimeProjection {
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    pay_vector_x_bits: 0x4120_0000,
                    pay_vector_y_bits: 0x41a0_0000,
                    pay_rotation_bits: 0x4000_0000,
                    payload_present: true,
                    payload_type: Some(1),
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: Some(9),
                    payload_unit_payload_len: Some(128),
                    payload_unit_payload_sha256: Some("abc123".to_string()),
                },
            );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                307,
                "payload-source",
                TypedBuildingRuntimeKind::PayloadSource,
                TypedBuildingRuntimeValue::PayloadSource {
                    configured_content: Some(ConfiguredContentRef {
                        content_type: 1,
                        content_id: 11,
                    }),
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    pay_vector_x_bits: Some(0x4120_0000),
                    pay_vector_y_bits: Some(0x41a0_0000),
                    pay_rotation_bits: Some(0x4000_0000),
                    payload_present: Some(true),
                    payload_type: Some(1),
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: Some(9),
                    payload_unit_payload_len: Some(128),
                    payload_unit_payload_sha256: Some("abc123".to_string()),
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_payload_router_family_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_0010i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            308,
            Some("payload-router".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                308,
                "payload-router",
                TypedBuildingRuntimeKind::PayloadRouter,
                TypedBuildingRuntimeValue::PayloadRouter {
                    sorted_content: None,
                    progress_bits: None,
                    item_rotation_bits: None,
                    payload_present: None,
                    payload_type: None,
                    payload_kind: None,
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: None,
                    payload_unit_revision: None,
                    payload_serialized_len: None,
                    payload_serialized_sha256: None,
                    rec_dir: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_payload_router_runtime() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_0011i32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            309,
            Some("reinforced-payload-router".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_payload_router_sorted_content(
                build_pos,
                Some(ConfiguredContentRef {
                    content_type: 1,
                    content_id: 11,
                }),
            );
        state
            .configured_block_projection
            .apply_payload_router_runtime(
                build_pos,
                PayloadRouterRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    item_rotation_bits: 0x4040_0000,
                    payload_present: true,
                    payload_type: Some(0),
                    payload_kind: Some(PayloadRouterPayloadKind::Unit),
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: Some(9),
                    payload_unit_revision: Some(2),
                    payload_serialized_len: 5,
                    payload_serialized_sha256: "0123456789abcdef".to_string(),
                    rec_dir: 3,
                },
            );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                309,
                "reinforced-payload-router",
                TypedBuildingRuntimeKind::PayloadRouter,
                TypedBuildingRuntimeValue::PayloadRouter {
                    sorted_content: Some(ConfiguredContentRef {
                        content_type: 1,
                        content_id: 11,
                    }),
                    progress_bits: Some(0x3f40_0000),
                    item_rotation_bits: Some(0x4040_0000),
                    payload_present: Some(true),
                    payload_type: Some(0),
                    payload_kind: Some(PayloadRouterPayloadKind::Unit),
                    payload_build_block_id: None,
                    payload_build_revision: None,
                    payload_unit_class_id: Some(9),
                    payload_unit_revision: Some(2),
                    payload_serialized_len: Some(5),
                    payload_serialized_sha256: Some("0123456789abcdef".to_string()),
                    rec_dir: Some(3),
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_gives_unit_factory_family_empty_shell() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000ci32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            305,
            Some("ground-factory".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                305,
                "ground-factory",
                TypedBuildingRuntimeKind::UnitFactory,
                TypedBuildingRuntimeValue::UnitFactory {
                    current_plan: None,
                    progress_bits: None,
                    command_pos: None,
                    command_id: None,
                    payload_present: None,
                    pay_rotation_bits: None,
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_reconstructor_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000ai32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            303,
            Some("additive-reconstructor".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_reconstructor_command(build_pos, Some(7));
        state
            .configured_block_projection
            .apply_reconstructor_runtime(
                build_pos,
                ReconstructorRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    payload_present: true,
                    pay_rotation_bits: 0x4000_0000,
                },
            );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                303,
                "additive-reconstructor",
                TypedBuildingRuntimeKind::Reconstructor,
                TypedBuildingRuntimeValue::Reconstructor {
                    command_id: Some(7),
                    progress_bits: Some(0x3f40_0000),
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    payload_present: Some(true),
                    pay_rotation_bits: Some(0x4000_0000),
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_unit_factory_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000di32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            306,
            Some("ground-factory".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            None,
            None,
            None,
        );
        state
            .configured_block_projection
            .apply_unit_factory_current_plan(build_pos, 7);
        state
            .configured_block_projection
            .apply_unit_factory_runtime(
                build_pos,
                UnitFactoryRuntimeProjection {
                    progress_bits: 0x3f40_0000,
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    command_id: Some(9),
                    payload_present: true,
                    pay_rotation_bits: 0x4000_0000,
                },
            );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                306,
                "ground-factory",
                TypedBuildingRuntimeKind::UnitFactory,
                TypedBuildingRuntimeValue::UnitFactory {
                    current_plan: Some(7),
                    progress_bits: Some(0x3f40_0000),
                    command_pos: Some((12.5f32.to_bits(), 18.0f32.to_bits())),
                    command_id: Some(9),
                    payload_present: Some(true),
                    pay_rotation_bits: Some(0x4000_0000),
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_build_tower_family() {
        let mut state = SessionState::default();
        let build_pos = 0x0008_000ai32;
        state.building_table_projection.apply_block_snapshot_head(
            build_pos,
            303,
            Some("build-tower".to_string()),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(0x3f80_0000),
            Some(0x3f00_0000),
            Some(126),
            Some(false),
            None,
            Some(0x40a0_0000),
            Some(true),
            Some(0x50),
            Some(0x28),
            Some(66),
            Some(0x4210_0000),
            Some(true),
            Some(5),
        );

        assert_eq!(
            state.typed_runtime_building_at(build_pos),
            Some(expected_typed_runtime_building(
                build_pos,
                303,
                "build-tower",
                TypedBuildingRuntimeKind::BuildTower,
                TypedBuildingRuntimeValue::BuildTower {
                    rotation_bits: Some(0x4210_0000),
                    plans_present: Some(true),
                    plan_count: Some(5),
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(0x3f80_0000),
                Some(0x3f00_0000),
                Some(126),
                Some(false),
                Some(0x40a0_0000),
                Some(true),
                Some(0x50),
                Some(0x28),
                Some(66),
                None,
                None,
                None,
                None,
                Some(0x4210_0000),
                Some(true),
                Some(5),
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn session_state_runtime_typed_building_projection_supports_turret_family_variants() {
        let mut state = SessionState::default();

        let turret_pos = 0x0009_000bi32;
        state
            .building_table_projection
            .apply_block_snapshot_head_with_tail_summary(
                turret_pos,
                304,
                Some("lancer".to_string()),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
                None,
                None,
                None,
                None,
                Some(0x40b0_0000),
                Some(true),
                Some(0x10),
                Some(0x08),
                Some(11),
                BuildingTailSummaryProjection {
                    turret_reload_counter_bits: Some(0x3f80_0000),
                    turret_rotation_bits: Some(0x4120_0000),
                    ..BuildingTailSummaryProjection::default()
                },
            );
        assert_eq!(
            state.typed_runtime_building_at(turret_pos),
            Some(expected_typed_runtime_building(
                turret_pos,
                304,
                "lancer",
                TypedBuildingRuntimeKind::Turret,
                TypedBuildingRuntimeValue::Turret {
                    reload_counter_bits: Some(0x3f80_0000),
                    rotation_bits: Some(0x4120_0000),
                },
                Vec::new(),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
                None,
                None,
                None,
                Some(0x40b0_0000),
                Some(true),
                Some(0x10),
                Some(0x08),
                Some(11),
                Some(0x3f80_0000),
                Some(0x4120_0000),
                None,
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );

        let item_turret_pos = 0x000a_000ci32;
        state
            .building_table_projection
            .apply_block_snapshot_head_with_tail_summary(
                item_turret_pos,
                305,
                Some("duo".to_string()),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                None,
                None,
                None,
                None,
                None,
                Some(0x40c0_0000),
                Some(true),
                Some(0x20),
                Some(0x10),
                Some(12),
                BuildingTailSummaryProjection {
                    turret_reload_counter_bits: Some(0x4000_0000),
                    turret_rotation_bits: Some(0x4130_0000),
                    item_turret_ammo_count: Some(7),
                    ..BuildingTailSummaryProjection::default()
                },
            );
        assert_eq!(
            state.typed_runtime_building_at(item_turret_pos),
            Some(expected_typed_runtime_building(
                item_turret_pos,
                305,
                "duo",
                TypedBuildingRuntimeKind::ItemTurret,
                TypedBuildingRuntimeValue::ItemTurret {
                    reload_counter_bits: Some(0x4000_0000),
                    rotation_bits: Some(0x4130_0000),
                    ammo_count: Some(7),
                },
                Vec::new(),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                None,
                None,
                None,
                None,
                Some(0x40c0_0000),
                Some(true),
                Some(0x20),
                Some(0x10),
                Some(12),
                Some(0x4000_0000),
                Some(0x4130_0000),
                Some(7),
                None,
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );

        let continuous_turret_pos = 0x000b_000di32;
        state
            .building_table_projection
            .apply_block_snapshot_head_with_tail_summary(
                continuous_turret_pos,
                306,
                Some("lustre".to_string()),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                None,
                None,
                None,
                None,
                None,
                Some(0x40d0_0000),
                Some(true),
                Some(0x30),
                Some(0x18),
                Some(13),
                BuildingTailSummaryProjection {
                    turret_reload_counter_bits: Some(0x4040_0000),
                    turret_rotation_bits: Some(0x4140_0000),
                    continuous_turret_last_length_bits: Some(0x40c0_0000),
                    ..BuildingTailSummaryProjection::default()
                },
            );
        assert_eq!(
            state.typed_runtime_building_at(continuous_turret_pos),
            Some(expected_typed_runtime_building(
                continuous_turret_pos,
                306,
                "lustre",
                TypedBuildingRuntimeKind::ContinuousTurret,
                TypedBuildingRuntimeValue::ContinuousTurret {
                    reload_counter_bits: Some(0x4040_0000),
                    rotation_bits: Some(0x4140_0000),
                    last_length_bits: Some(0x40c0_0000),
                },
                Vec::new(),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                None,
                None,
                None,
                None,
                Some(0x40d0_0000),
                Some(true),
                Some(0x30),
                Some(0x18),
                Some(13),
                Some(0x4040_0000),
                Some(0x4140_0000),
                None,
                Some(0x40c0_0000),
                None,
                None,
                None,
                BuildingProjectionUpdateKind::BlockSnapshotHead,
            ))
        );
    }

    #[test]
    fn hidden_snapshot_policy_removes_known_unit_class_rows_without_semantic_projection() {
        let mut state = SessionState::default();
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

        let remove_ids =
            state.hidden_snapshot_unit_handle_sync_hidden_remove_ids(&BTreeSet::from([303]), None);

        assert_eq!(remove_ids, BTreeSet::from([303]));
    }

    #[test]
    fn hidden_snapshot_policy_removes_known_runtime_owned_class_rows_without_semantic_projection() {
        let mut state = SessionState::default();
        for (entity_id, class_id) in [(303, 10), (404, 13), (505, 14), (606, 35)] {
            state.entity_table_projection.by_entity_id.insert(
                entity_id,
                EntityProjection {
                    class_id,
                    hidden: false,
                    is_local_player: false,
                    unit_kind: 0,
                    unit_value: 0,
                    x_bits: 0,
                    y_bits: 0,
                    last_seen_entity_snapshot_count: 1,
                },
            );
        }

        let remove_ids = state.hidden_snapshot_runtime_owned_cleanup_remove_ids(
            &BTreeSet::from([303, 404, 505, 606]),
            None,
        );

        assert_eq!(remove_ids, BTreeSet::from([303, 404, 505]));
    }

    #[test]
    fn resource_delta_projection_seed_world_build_items_sets_baseline_without_counter_drift() {
        let mut projection = ResourceDeltaProjection::default();
        projection.authoritative_build_update_count = 7;
        projection.last_changed_build_pos = Some(pack_point2(1, 1));
        projection.last_changed_item_id = Some(5);
        projection.last_changed_amount = Some(9);

        let build_pos = pack_point2(8, 9);
        projection.seed_world_build_items(build_pos, &[(4, 12), (6, 0), (7, 3)]);

        assert_eq!(
            projection.building_items_by_build.get(&build_pos).cloned(),
            Some(BTreeMap::from([(4, 12), (7, 3)]))
        );
        assert_eq!(projection.authoritative_build_update_count, 7);
        assert_eq!(projection.last_changed_build_pos, Some(pack_point2(1, 1)));
        assert_eq!(projection.last_changed_item_id, Some(5));
        assert_eq!(projection.last_changed_amount, Some(9));

        projection.seed_world_build_items(build_pos, &[]);
        assert!(!projection.building_items_by_build.contains_key(&build_pos));
        assert_eq!(projection.authoritative_build_update_count, 7);
    }

    #[test]
    fn resource_delta_projection_seed_world_build_liquids_sets_baseline_without_counter_drift() {
        let mut projection = ResourceDeltaProjection::default();
        projection.authoritative_build_update_count = 7;
        projection.last_changed_build_pos = Some(pack_point2(1, 1));
        projection.last_changed_item_id = Some(5);
        projection.last_changed_amount = Some(9);

        let build_pos = pack_point2(8, 9);
        projection.seed_world_build_liquids(
            build_pos,
            &[
                (4, 1.25f32.to_bits()),
                (6, 0.0f32.to_bits()),
                (7, 3.5f32.to_bits()),
            ],
        );

        assert_eq!(
            projection
                .building_liquids_by_build
                .get(&build_pos)
                .cloned(),
            Some(BTreeMap::from([
                (4, 1.25f32.to_bits()),
                (7, 3.5f32.to_bits())
            ]))
        );
        assert_eq!(projection.authoritative_build_update_count, 7);
        assert_eq!(projection.last_changed_build_pos, Some(pack_point2(1, 1)));
        assert_eq!(projection.last_changed_item_id, Some(5));
        assert_eq!(projection.last_changed_amount, Some(9));

        projection.seed_world_build_liquids(build_pos, &[]);
        assert!(!projection
            .building_liquids_by_build
            .contains_key(&build_pos));
        assert_eq!(projection.authoritative_build_update_count, 7);
    }

    #[test]
    fn resource_delta_projection_replace_build_items_exact_full_replaces_and_counts_once() {
        let mut projection = ResourceDeltaProjection::default();
        let build_pos = pack_point2(8, 9);
        projection
            .building_items_by_build
            .insert(build_pos, BTreeMap::from([(1, 4), (3, 7)]));
        projection.authoritative_build_update_count = 2;

        projection.replace_build_items_exact(Some(build_pos), &[(4, 12), (6, 0)]);

        assert_eq!(
            projection.building_items_by_build.get(&build_pos).cloned(),
            Some(BTreeMap::from([(4, 12)]))
        );
        assert_eq!(projection.authoritative_build_update_count, 3);
        assert_eq!(projection.last_changed_build_pos, Some(build_pos));
        assert_eq!(projection.last_changed_item_id, Some(4));
        assert_eq!(projection.last_changed_amount, Some(12));

        projection.replace_build_items_exact(Some(build_pos), &[]);

        assert!(!projection.building_items_by_build.contains_key(&build_pos));
        assert_eq!(projection.authoritative_build_update_count, 4);
        assert_eq!(projection.last_changed_build_pos, Some(build_pos));
        assert_eq!(projection.last_changed_item_id, None);
        assert_eq!(projection.last_changed_amount, Some(0));
    }

    #[test]
    fn resource_delta_projection_replace_build_liquids_exact_full_replaces_and_counts_once() {
        let mut projection = ResourceDeltaProjection::default();
        let build_pos = pack_point2(8, 9);
        projection.building_liquids_by_build.insert(
            build_pos,
            BTreeMap::from([(1, 0.25f32.to_bits()), (3, 0.75f32.to_bits())]),
        );
        projection.authoritative_build_update_count = 2;

        projection.replace_build_liquids_exact(
            Some(build_pos),
            &[(4, 1.5f32.to_bits()), (6, 0.0f32.to_bits())],
        );

        assert_eq!(
            projection
                .building_liquids_by_build
                .get(&build_pos)
                .cloned(),
            Some(BTreeMap::from([(4, 1.5f32.to_bits())]))
        );
        assert_eq!(projection.authoritative_build_update_count, 3);
        assert_eq!(projection.last_changed_build_pos, Some(build_pos));
        assert_eq!(projection.last_changed_item_id, None);
        assert_eq!(projection.last_changed_amount, None);

        projection.replace_build_liquids_exact(Some(build_pos), &[]);

        assert!(!projection
            .building_liquids_by_build
            .contains_key(&build_pos));
        assert_eq!(projection.authoritative_build_update_count, 4);
        assert_eq!(projection.last_changed_build_pos, Some(build_pos));
        assert_eq!(projection.last_changed_item_id, None);
        assert_eq!(projection.last_changed_amount, None);
    }

    #[test]
    fn resource_delta_projection_replace_entity_item_stack_exact_overwrites_and_clears() {
        let mut projection = ResourceDeltaProjection::default();
        projection.entity_item_stack_by_entity_id.insert(
            44,
            ResourceUnitItemStack {
                item_id: Some(1),
                amount: 3,
            },
        );

        projection.replace_entity_item_stack_exact(Some(44), 6, 9);

        assert_eq!(
            projection.entity_item_stack_by_entity_id.get(&44).cloned(),
            Some(ResourceUnitItemStack {
                item_id: Some(6),
                amount: 9,
            })
        );
        assert_eq!(projection.last_changed_build_pos, None);
        assert_eq!(projection.last_changed_entity_id, Some(44));
        assert_eq!(projection.last_changed_item_id, Some(6));
        assert_eq!(projection.last_changed_amount, Some(9));
        assert_eq!(projection.authoritative_build_update_count, 0);
        assert_eq!(projection.delta_apply_count, 0);

        projection.replace_entity_item_stack_exact(Some(44), 6, 0);

        assert!(!projection.entity_item_stack_by_entity_id.contains_key(&44));
        assert_eq!(projection.last_changed_build_pos, None);
        assert_eq!(projection.last_changed_entity_id, Some(44));
        assert_eq!(projection.last_changed_item_id, None);
        assert_eq!(projection.last_changed_amount, Some(0));
        assert_eq!(projection.authoritative_build_update_count, 0);
        assert_eq!(projection.delta_apply_count, 0);
    }

    #[test]
    fn hidden_snapshot_runtime_transition_separates_unit_handle_sync_hidden_from_runtime_cleanup() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 1,
            },
        );
        for (entity_id, class_id) in [(202, 33), (404, 10)] {
            state.entity_table_projection.by_entity_id.insert(
                entity_id,
                EntityProjection {
                    class_id,
                    hidden: false,
                    is_local_player: false,
                    unit_kind: 0,
                    unit_value: 0,
                    x_bits: 0,
                    y_bits: 0,
                    last_seen_entity_snapshot_count: 1,
                },
            );
        }
        state.entity_table_projection.by_entity_id.insert(
            505,
            EntityProjection {
                class_id: 99,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 0,
                y_bits: 0,
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
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            505,
            EntitySemanticProjectionEntry {
                class_id: 99,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
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
            },
        );

        let transition = state.hidden_snapshot_runtime_transition(
            &BTreeSet::from([101, 202, 303, 404, 505]),
            Some(101),
        );

        assert_eq!(
            transition.auxiliary_cleanup_ids,
            BTreeSet::from([202, 303, 404, 505])
        );
        assert_eq!(
            transition.unit_handle_sync_hidden_remove_ids,
            BTreeSet::from([202, 303])
        );
        assert_eq!(
            transition.runtime_owned_cleanup_remove_ids,
            BTreeSet::from([404])
        );
        assert_eq!(
            transition.lifecycle_remove_ids(),
            BTreeSet::from([202, 303, 404])
        );
    }

    #[test]
    fn hidden_snapshot_lifecycle_remove_clears_matching_entity_tombstones() {
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: 33,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_semantic_projection.by_entity_id.insert(
            303,
            EntitySemanticProjectionEntry {
                class_id: 33,
                last_seen_entity_snapshot_count: 1,
                projection: EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
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
            },
        );
        state.record_entity_snapshot_tombstone(303);

        assert!(state.entity_snapshot_tombstone_blocks_upsert(303));

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(303),
                sample_ids: vec![303],
            },
            BTreeSet::from([303]),
        );

        assert!(!state
            .entity_table_projection
            .by_entity_id
            .contains_key(&303));
        assert!(!state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert!(!state.entity_snapshot_tombstones.contains_key(&303));
        assert!(!state.entity_snapshot_tombstone_blocks_upsert(303));
    }

    #[test]
    fn hidden_snapshot_clears_non_local_resource_event_refs_without_touching_local_refs() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.resource_delta_projection.last_unit = Some(UnitRefProjection {
            kind: 2,
            value: 202,
        });
        state.resource_delta_projection.last_to_entity_id = Some(202);
        state.resource_delta_projection.last_changed_entity_id = Some(202);
        state.last_take_items = Some(TakeItemsProjection {
            build_pos: Some(pack_point2(1, 1)),
            item_id: Some(4),
            amount: 3,
            to: Some(UnitRefProjection {
                kind: 2,
                value: 101,
            }),
        });
        state.last_transfer_item_to = Some(TransferItemToProjection {
            unit: Some(UnitRefProjection {
                kind: 2,
                value: 202,
            }),
            item_id: Some(4),
            amount: 1,
            x_bits: 0,
            y_bits: 0,
            build_pos: Some(pack_point2(2, 2)),
        });
        state.last_transfer_item_to_unit = Some(TransferItemToUnitProjection {
            item_id: Some(4),
            x_bits: 0,
            y_bits: 0,
            to_entity_id: Some(202),
        });
        state.last_transfer_item_effect = Some(TransferItemEffectProjection {
            item_id: Some(4),
            x_bits: 0,
            y_bits: 0,
            to_entity_id: Some(101),
        });

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(202),
                sample_ids: vec![202],
            },
            BTreeSet::from([202]),
        );

        assert_eq!(state.resource_delta_projection.last_unit, None);
        assert_eq!(state.resource_delta_projection.last_to_entity_id, None);
        assert_eq!(state.resource_delta_projection.last_changed_entity_id, None);
        assert_eq!(
            state
                .last_take_items
                .as_ref()
                .and_then(|projection| projection.to),
            Some(UnitRefProjection {
                kind: 2,
                value: 101,
            })
        );
        assert_eq!(
            state
                .last_transfer_item_to
                .as_ref()
                .and_then(|projection| projection.unit),
            None
        );
        assert_eq!(
            state
                .last_transfer_item_to_unit
                .as_ref()
                .and_then(|projection| projection.to_entity_id),
            None
        );
        assert_eq!(
            state
                .last_transfer_item_effect
                .as_ref()
                .and_then(|projection| projection.to_entity_id),
            Some(101)
        );
    }

    #[test]
    fn hidden_snapshot_clears_non_local_payload_event_refs_without_touching_local_refs() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.last_payload_dropped = Some(PayloadDroppedProjection {
            unit: Some(UnitRefProjection {
                kind: 2,
                value: 202,
            }),
            x_bits: 0,
            y_bits: 0,
        });
        state.last_picked_build_payload = Some(PickedBuildPayloadProjection {
            unit: Some(UnitRefProjection {
                kind: 2,
                value: 101,
            }),
            build_pos: Some(pack_point2(3, 3)),
            on_ground: false,
        });
        state.last_picked_unit_payload = Some(PickedUnitPayloadProjection {
            unit: Some(UnitRefProjection {
                kind: 2,
                value: 101,
            }),
            target: Some(UnitRefProjection {
                kind: 2,
                value: 202,
            }),
        });
        state.last_unit_entered_payload = Some(UnitEnteredPayloadProjection {
            unit: Some(UnitRefProjection {
                kind: 2,
                value: 202,
            }),
            build_pos: Some(pack_point2(4, 4)),
        });

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(202),
                sample_ids: vec![202],
            },
            BTreeSet::from([202]),
        );

        assert_eq!(
            state
                .last_payload_dropped
                .as_ref()
                .and_then(|projection| projection.unit),
            None
        );
        assert_eq!(
            state
                .last_picked_build_payload
                .as_ref()
                .and_then(|projection| projection.unit),
            Some(UnitRefProjection {
                kind: 2,
                value: 101,
            })
        );
        assert_eq!(
            state
                .last_picked_unit_payload
                .as_ref()
                .and_then(|projection| projection.unit),
            Some(UnitRefProjection {
                kind: 2,
                value: 101,
            })
        );
        assert_eq!(
            state
                .last_picked_unit_payload
                .as_ref()
                .and_then(|projection| projection.target),
            None
        );
        assert_eq!(
            state
                .last_unit_entered_payload
                .as_ref()
                .and_then(|projection| projection.unit),
            None
        );
    }

    #[test]
    fn session_state_typed_runtime_entity_at_surfaces_player_without_semantic() {
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 9001,
                x_bits: 1.5f32.to_bits(),
                y_bits: 2.5f32.to_bits(),
                last_seen_entity_snapshot_count: 7,
            },
        );

        assert_eq!(
            state.typed_runtime_entity_at(101),
            Some(TypedRuntimeEntityModel::Player(TypedRuntimePlayerEntity {
                base: TypedRuntimeEntityBase {
                    entity_id: 101,
                    class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                    hidden: false,
                    is_local_player: false,
                    unit_kind: 2,
                    unit_value: 9001,
                    x_bits: 1.5f32.to_bits(),
                    y_bits: 2.5f32.to_bits(),
                    last_seen_entity_snapshot_count: 7,
                },
                semantic: EntityPlayerSemanticProjection::default(),
            }))
        );
        assert_eq!(state.typed_runtime_entities().len(), 1);
    }

    #[test]
    fn session_state_typed_runtime_entity_at_joins_unit_semantic_projection() {
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: true,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 3.5f32.to_bits(),
                y_bits: 4.5f32.to_bits(),
                last_seen_entity_snapshot_count: 9,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            9,
            EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                team_id: 2,
                unit_type_id: 55,
                health_bits: 0x3f80_0000,
                rotation_bits: 0x4000_0000,
                shield_bits: 0x4040_0000,
                mine_tile_pos: 77,
                status_count: 3,
                payload_count: Some(1),
                building_pos: Some(88),
                lifetime_bits: Some(0x4080_0000),
                time_bits: Some(0x40a0_0000),
                runtime_sync: None,
                controller_type: 0,
                controller_value: None,
            }),
        );
        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                202,
                ResourceUnitItemStack {
                    item_id: Some(4),
                    amount: 7,
                },
            );

        assert_eq!(
            state.typed_runtime_entity_at(202),
            Some(TypedRuntimeEntityModel::Unit(TypedRuntimeUnitEntity {
                base: TypedRuntimeEntityBase {
                    entity_id: 202,
                    class_id: 4,
                    hidden: true,
                    is_local_player: false,
                    unit_kind: 2,
                    unit_value: 202,
                    x_bits: 3.5f32.to_bits(),
                    y_bits: 4.5f32.to_bits(),
                    last_seen_entity_snapshot_count: 9,
                },
                semantic: EntityUnitSemanticProjection {
                    team_id: 2,
                    unit_type_id: 55,
                    health_bits: 0x3f80_0000,
                    rotation_bits: 0x4000_0000,
                    shield_bits: 0x4040_0000,
                    mine_tile_pos: 77,
                    status_count: 3,
                    payload_count: Some(1),
                    building_pos: Some(88),
                    lifetime_bits: Some(0x4080_0000),
                    time_bits: Some(0x40a0_0000),
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                },
                carried_item_stack: Some(ResourceUnitItemStack {
                    item_id: Some(4),
                    amount: 7,
                }),
            }))
        );
        assert_eq!(state.typed_runtime_entities().len(), 1);
    }

    #[test]
    fn session_state_typed_runtime_entity_at_joins_world_label_semantic_projection() {
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: 35,
                hidden: true,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 5.5f32.to_bits(),
                y_bits: 6.5f32.to_bits(),
                last_seen_entity_snapshot_count: 11,
            },
        );
        state.entity_semantic_projection.upsert(
            303,
            35,
            11,
            EntitySemanticProjection::WorldLabel(EntityWorldLabelSemanticProjection {
                flags: 3,
                font_size_bits: 1.5f32.to_bits(),
                text: Some("hello world".to_string()),
                z_bits: 120.0f32.to_bits(),
            }),
        );

        assert_eq!(
            state.typed_runtime_entity_at(303),
            Some(TypedRuntimeEntityModel::WorldLabel(
                TypedRuntimeWorldLabelEntity {
                    base: TypedRuntimeEntityBase {
                        entity_id: 303,
                        class_id: 35,
                        hidden: true,
                        is_local_player: false,
                        unit_kind: 0,
                        unit_value: 0,
                        x_bits: 5.5f32.to_bits(),
                        y_bits: 6.5f32.to_bits(),
                        last_seen_entity_snapshot_count: 11,
                    },
                    semantic: EntityWorldLabelSemanticProjection {
                        flags: 3,
                        font_size_bits: 1.5f32.to_bits(),
                        text: Some("hello world".to_string()),
                        z_bits: 120.0f32.to_bits(),
                    },
                }
            ))
        );
        assert_eq!(state.typed_runtime_entities().len(), 1);
    }

    #[test]
    fn entity_table_apply_hidden_ids_clears_stale_hidden_flags_for_removed_ids() {
        let mut table = EntityTableProjection::default();
        table.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: 12,
                hidden: true,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 1,
            },
        );
        table.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: 35,
                hidden: true,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 1,
            },
        );
        table.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 35,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 1,
            },
        );
        table.hidden_count = 2;

        table.apply_hidden_ids(&BTreeSet::from([404]));

        assert!(!table.by_entity_id[&101].hidden);
        assert!(!table.by_entity_id[&303].hidden);
        assert!(table.by_entity_id[&404].hidden);
        assert_eq!(table.hidden_apply_count, 1);
        assert_eq!(table.hidden_count, 1);
    }

    #[test]
    fn session_state_typed_runtime_entity_projection_summarizes_players_and_units() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 1001,
                x_bits: 10.0f32.to_bits(),
                y_bits: 20.0f32.to_bits(),
                last_seen_entity_snapshot_count: 7,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            102,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: true,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 1002,
                x_bits: 11.0f32.to_bits(),
                y_bits: 21.0f32.to_bits(),
                last_seen_entity_snapshot_count: 8,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 30.0f32.to_bits(),
                y_bits: 40.0f32.to_bits(),
                last_seen_entity_snapshot_count: 9,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: 35,
                hidden: false,
                is_local_player: false,
                unit_kind: 0,
                unit_value: 0,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count: 10,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            9,
            EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                team_id: 2,
                unit_type_id: 55,
                health_bits: 0x3f80_0000,
                rotation_bits: 0x4000_0000,
                shield_bits: 0x4040_0000,
                mine_tile_pos: 77,
                status_count: 3,
                payload_count: Some(1),
                building_pos: Some(88),
                lifetime_bits: Some(0x4080_0000),
                time_bits: Some(0x40a0_0000),
                runtime_sync: None,
                controller_type: 0,
                controller_value: None,
            }),
        );
        state.entity_semantic_projection.upsert(
            303,
            35,
            10,
            EntitySemanticProjection::WorldLabel(EntityWorldLabelSemanticProjection {
                flags: 1,
                font_size_bits: 12.0f32.to_bits(),
                text: Some("world".to_string()),
                z_bits: 0.5f32.to_bits(),
            }),
        );

        let projection = state.typed_runtime_entity_projection();

        assert_eq!(projection.player_count, 2);
        assert_eq!(projection.unit_count, 1);
        assert_eq!(projection.hidden_count, 1);
        assert_eq!(projection.local_player_entity_id, Some(101));
        assert_eq!(projection.last_entity_id, Some(303));
        assert_eq!(projection.last_player_entity_id, Some(102));
        assert_eq!(projection.last_unit_entity_id, Some(202));
        assert!(matches!(
            projection.entity_at(102),
            Some(TypedRuntimeEntityModel::Player(player))
                if player.base.hidden && player.base.unit_value == 1002
        ));
        assert!(matches!(
            projection.entity_at(202),
            Some(TypedRuntimeEntityModel::Unit(unit))
                if unit.semantic.unit_type_id == 55
                    && unit.semantic.payload_count == Some(1)
        ));
        assert!(matches!(
            projection.entity_at(303),
            Some(TypedRuntimeEntityModel::WorldLabel(world_label))
                if world_label.base.hidden == false
                    && world_label.semantic.text.as_deref() == Some("world")
        ));
        assert_eq!(
            projection
                .local_player()
                .map(|player| player.base.entity_id),
            Some(101)
        );
        assert_eq!(projection.local_player_owned_unit_entity_id, None);
        assert_eq!(projection.player_with_owned_unit_count, 0);
        assert_eq!(projection.owned_unit_count, 0);
    }

    #[test]
    fn session_state_typed_runtime_entity_projection_resolves_player_unit_ownership() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 10.0f32.to_bits(),
                y_bits: 20.0f32.to_bits(),
                last_seen_entity_snapshot_count: 7,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            102,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 11.0f32.to_bits(),
                y_bits: 21.0f32.to_bits(),
                last_seen_entity_snapshot_count: 8,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 30.0f32.to_bits(),
                y_bits: 40.0f32.to_bits(),
                last_seen_entity_snapshot_count: 9,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            303,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 404,
                x_bits: 12.0f32.to_bits(),
                y_bits: 22.0f32.to_bits(),
                last_seen_entity_snapshot_count: 10,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            404,
            EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 404,
                x_bits: 50.0f32.to_bits(),
                y_bits: 60.0f32.to_bits(),
                last_seen_entity_snapshot_count: 11,
            },
        );
        for entity_id in [202, 404] {
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
                    mine_tile_pos: 77,
                    status_count: 3,
                    payload_count: Some(1),
                    building_pos: Some(88),
                    lifetime_bits: Some(0x4080_0000),
                    time_bits: Some(0x40a0_0000),
                    runtime_sync: None,
                    controller_type: 0,
                    controller_value: None,
                }),
            );
        }

        let projection = state.typed_runtime_entity_projection();

        assert_eq!(projection.local_player_owned_unit_entity_id, None);
        assert_eq!(projection.player_with_owned_unit_count, 1);
        assert_eq!(projection.owned_unit_count, 1);
        assert_eq!(projection.ownership_conflict_count, 1);
        assert_eq!(projection.ownership_conflict_unit_sample, vec![202]);
        assert_eq!(projection.owned_unit_entity_id_for_player(303), Some(404));
        assert_eq!(projection.owner_player_entity_id_for_unit(404), Some(303));
        assert_eq!(projection.owned_unit_entity_id_for_player(101), None);
        assert_eq!(projection.owner_player_entity_id_for_unit(202), None);
    }

    #[test]
    fn session_state_runtime_typed_entity_projection_rebuilds_from_tables() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 1001,
                x_bits: 10.0f32.to_bits(),
                y_bits: 20.0f32.to_bits(),
                last_seen_entity_snapshot_count: 7,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: true,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 30.0f32.to_bits(),
                y_bits: 40.0f32.to_bits(),
                last_seen_entity_snapshot_count: 9,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            9,
            EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                team_id: 2,
                unit_type_id: 55,
                health_bits: 0x3f80_0000,
                rotation_bits: 0x4000_0000,
                shield_bits: 0x4040_0000,
                mine_tile_pos: 77,
                status_count: 3,
                payload_count: Some(1),
                building_pos: Some(88),
                lifetime_bits: Some(0x4080_0000),
                time_bits: Some(0x40a0_0000),
                runtime_sync: None,
                controller_type: 0,
                controller_value: None,
            }),
        );

        state.rebuild_runtime_typed_entity_projection_from_tables();
        let projection = state.runtime_typed_entity_projection();

        assert_eq!(projection.player_count, 1);
        assert_eq!(projection.unit_count, 1);
        assert_eq!(projection.hidden_count, 1);
        assert_eq!(projection.local_player_entity_id, Some(101));
        assert_eq!(projection.local_player_owned_unit_entity_id, None);
        assert_eq!(projection.player_with_owned_unit_count, 0);
        assert_eq!(projection.owned_unit_count, 0);
        assert_eq!(projection.last_entity_id, Some(202));
        assert_eq!(projection.last_player_entity_id, Some(101));
        assert_eq!(projection.last_unit_entity_id, Some(202));
        assert!(matches!(
            projection.entity_at(101),
            Some(TypedRuntimeEntityModel::Player(player))
                if player.base.is_local_player && player.base.unit_value == 1001
        ));
        assert!(matches!(
            projection.entity_at(202),
            Some(TypedRuntimeEntityModel::Unit(unit))
                if unit.base.hidden && unit.semantic.unit_type_id == 55
        ));
    }

    #[test]
    fn session_state_refresh_runtime_typed_entity_tracks_unit_carried_item_stack() {
        let mut state = SessionState::default();
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 30.0f32.to_bits(),
                y_bits: 40.0f32.to_bits(),
                last_seen_entity_snapshot_count: 9,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            9,
            EntitySemanticProjection::Unit(EntityUnitSemanticProjection {
                team_id: 2,
                unit_type_id: 55,
                health_bits: 0x3f80_0000,
                rotation_bits: 0x4000_0000,
                shield_bits: 0x4040_0000,
                mine_tile_pos: 77,
                status_count: 3,
                payload_count: Some(1),
                building_pos: Some(88),
                lifetime_bits: Some(0x4080_0000),
                time_bits: Some(0x40a0_0000),
                runtime_sync: None,
                controller_type: 0,
                controller_value: None,
            }),
        );

        state.refresh_runtime_typed_entity_from_tables(202);
        assert!(matches!(
            state.runtime_typed_entity_projection().entity_at(202),
            Some(TypedRuntimeEntityModel::Unit(unit)) if unit.carried_item_stack.is_none()
        ));

        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .insert(
                202,
                ResourceUnitItemStack {
                    item_id: Some(6),
                    amount: 4,
                },
            );
        state.refresh_runtime_typed_entity_from_tables(202);
        assert!(matches!(
            state.runtime_typed_entity_projection().entity_at(202),
            Some(TypedRuntimeEntityModel::Unit(unit))
                if unit.carried_item_stack
                    == Some(ResourceUnitItemStack { item_id: Some(6), amount: 4 })
        ));

        state
            .resource_delta_projection
            .entity_item_stack_by_entity_id
            .remove(&202);
        state.refresh_runtime_typed_entity_from_tables(202);
        assert!(matches!(
            state.runtime_typed_entity_projection().entity_at(202),
            Some(TypedRuntimeEntityModel::Unit(unit)) if unit.carried_item_stack.is_none()
        ));
    }

    #[test]
    fn hidden_snapshot_rebuilds_runtime_typed_entity_apply_projection() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 3.0f32.to_bits(),
                y_bits: 4.0f32.to_bits(),
                last_seen_entity_snapshot_count: 2,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            2,
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
        state.rebuild_runtime_typed_entity_projection_from_tables();

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(202),
                sample_ids: vec![202],
            },
            BTreeSet::from([202]),
        );

        let projection = state.runtime_typed_entity_projection();
        assert_eq!(projection.player_count, 1);
        assert_eq!(projection.unit_count, 0);
        assert_eq!(projection.hidden_count, 0);
        assert_eq!(projection.local_player_entity_id, Some(101));
        assert!(projection.by_entity_id.contains_key(&101));
        assert!(!projection.by_entity_id.contains_key(&202));
    }

    #[test]
    fn hidden_snapshot_runtime_typed_transition_does_not_reseed_unrelated_table_rows() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
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

        let before = state.runtime_typed_entity_projection();
        assert!(before.by_entity_id.contains_key(&101));
        assert!(before.by_entity_id.contains_key(&202));
        assert!(!before.by_entity_id.contains_key(&303));

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(202),
                sample_ids: vec![202],
            },
            BTreeSet::from([202]),
        );

        let projection = state.runtime_typed_entity_projection();
        assert!(state
            .entity_table_projection
            .by_entity_id
            .contains_key(&303));
        assert!(state
            .entity_semantic_projection
            .by_entity_id
            .contains_key(&303));
        assert!(projection.by_entity_id.contains_key(&101));
        assert!(!projection.by_entity_id.contains_key(&202));
        assert!(!projection.by_entity_id.contains_key(&303));
        assert_eq!(projection.player_count, 1);
        assert_eq!(projection.unit_count, 0);
        assert_eq!(projection.hidden_count, 0);
    }

    #[test]
    fn repeated_hidden_snapshot_reasserts_runtime_typed_entity_suppression() {
        let mut state = SessionState::default();
        state.entity_table_projection.local_player_entity_id = Some(101);
        state.entity_table_projection.by_entity_id.insert(
            101,
            EntityProjection {
                class_id: EntityTableProjection::LOCAL_PLAYER_CLASS_ID,
                hidden: false,
                is_local_player: true,
                unit_kind: 2,
                unit_value: 101,
                x_bits: 1.0f32.to_bits(),
                y_bits: 2.0f32.to_bits(),
                last_seen_entity_snapshot_count: 1,
            },
        );
        state.entity_table_projection.by_entity_id.insert(
            202,
            EntityProjection {
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: 202,
                x_bits: 3.0f32.to_bits(),
                y_bits: 4.0f32.to_bits(),
                last_seen_entity_snapshot_count: 2,
            },
        );
        state.entity_semantic_projection.upsert(
            202,
            4,
            2,
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
        state.rebuild_runtime_typed_entity_projection_from_tables();
        let stale_model = state
            .runtime_typed_entity_projection()
            .entity_at(202)
            .cloned()
            .expect("expected runtime entity before hidden snapshot");

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(202),
                sample_ids: vec![202],
            },
            BTreeSet::from([202]),
        );
        assert!(!state
            .runtime_typed_entity_projection()
            .by_entity_id
            .contains_key(&202));

        state
            .runtime_typed_entity_apply_projection
            .upsert_runtime_entity(stale_model);
        assert!(state
            .runtime_typed_entity_projection()
            .by_entity_id
            .contains_key(&202));

        state.apply_hidden_snapshot(
            AppliedHiddenSnapshotIds {
                count: 1,
                first_id: Some(202),
                sample_ids: vec![202],
            },
            BTreeSet::from([202]),
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
        let projection = state.runtime_typed_entity_projection();
        assert!(projection.by_entity_id.contains_key(&101));
        assert!(!projection.by_entity_id.contains_key(&202));
    }
}
