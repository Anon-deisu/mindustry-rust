use crate::session_state::{
    AppliedStateSnapshot, AppliedStateSnapshotCoreData, GameplayStateProjection,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateSnapshotCoreInventorySemantics {
    pub inventory_by_team: BTreeMap<u8, BTreeMap<u16, i32>>,
    pub item_entry_count: usize,
    pub total_amount: i64,
    pub nonzero_item_count: usize,
    pub duplicate_team_count: usize,
    pub duplicate_item_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct StateSnapshotCoreInventoryPrevious<'a> {
    pub inventory_by_team: &'a BTreeMap<u8, BTreeMap<u16, i32>>,
    pub item_entry_count: usize,
    pub total_amount: i64,
    pub nonzero_item_count: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateSnapshotCoreInventoryTransition {
    pub inventory: StateSnapshotCoreInventorySemantics,
    pub changed_team_ids: BTreeSet<u8>,
    pub synced: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateSnapshotHeadPrevious {
    pub wave: i32,
    pub time_data: i32,
    pub gameplay_state: GameplayStateProjection,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateSnapshotHeadTransition {
    pub gameplay_state: GameplayStateProjection,
    pub gameplay_state_changed: bool,
    pub last_wave_advanced: bool,
    pub last_wave_advance_from: Option<i32>,
    pub last_wave_advance_to: Option<i32>,
    pub last_net_seconds_rollback: bool,
    pub net_seconds_delta: i32,
    pub wave_regressed: bool,
    pub time_regressed: bool,
}

impl StateSnapshotCoreInventorySemantics {
    pub fn from_core_data(core_data: &AppliedStateSnapshotCoreData) -> Self {
        let mut inventory_by_team = BTreeMap::<u8, BTreeMap<u16, i32>>::new();
        let mut seen_team_ids = BTreeSet::new();
        let mut seen_item_ids_by_team = BTreeMap::<u8, BTreeSet<u16>>::new();
        let mut duplicate_team_count = 0usize;
        let mut duplicate_item_count = 0usize;

        for team in &core_data.teams {
            if !seen_team_ids.insert(team.team_id) {
                duplicate_team_count = duplicate_team_count.saturating_add(1);
            }

            let items = inventory_by_team.entry(team.team_id).or_default();
            let seen_item_ids = seen_item_ids_by_team.entry(team.team_id).or_default();

            for item in &team.items {
                if !seen_item_ids.insert(item.item_id) {
                    duplicate_item_count = duplicate_item_count.saturating_add(1);
                }
                items.insert(item.item_id, item.amount);
            }
        }

        let mut item_entry_count = 0usize;
        let mut total_amount = 0i64;
        let mut nonzero_item_count = 0usize;
        for items in inventory_by_team.values() {
            item_entry_count = item_entry_count.saturating_add(items.len());
            for amount in items.values() {
                total_amount = total_amount.saturating_add(i64::from(*amount));
                if *amount != 0 {
                    nonzero_item_count = nonzero_item_count.saturating_add(1);
                }
            }
        }

        Self {
            inventory_by_team,
            item_entry_count,
            total_amount,
            nonzero_item_count,
            duplicate_team_count,
            duplicate_item_count,
        }
    }

    pub fn from_previous(previous: StateSnapshotCoreInventoryPrevious<'_>) -> Self {
        Self {
            inventory_by_team: previous.inventory_by_team.clone(),
            item_entry_count: previous.item_entry_count,
            total_amount: previous.total_amount,
            nonzero_item_count: previous.nonzero_item_count,
            duplicate_team_count: 0,
            duplicate_item_count: 0,
        }
    }

    pub fn changed_team_ids_since(
        &self,
        previous_by_team: Option<&BTreeMap<u8, BTreeMap<u16, i32>>>,
    ) -> BTreeSet<u8> {
        let Some(previous_by_team) = previous_by_team else {
            return self.inventory_by_team.keys().copied().collect();
        };

        previous_by_team
            .keys()
            .chain(self.inventory_by_team.keys())
            .filter(|team_id| previous_by_team.get(team_id) != self.inventory_by_team.get(team_id))
            .copied()
            .collect()
    }
}

pub fn derive_state_snapshot_core_inventory_transition(
    previous: Option<StateSnapshotCoreInventoryPrevious<'_>>,
    core_data: Option<&AppliedStateSnapshotCoreData>,
) -> StateSnapshotCoreInventoryTransition {
    let inventory = match core_data {
        Some(core_data) => StateSnapshotCoreInventorySemantics::from_core_data(core_data),
        None => previous
            .map(StateSnapshotCoreInventorySemantics::from_previous)
            .unwrap_or_default(),
    };
    let changed_team_ids = if core_data.is_some() {
        inventory.changed_team_ids_since(previous.map(|previous| previous.inventory_by_team))
    } else {
        BTreeSet::new()
    };

    StateSnapshotCoreInventoryTransition {
        inventory,
        changed_team_ids,
        synced: core_data.is_some(),
    }
}

pub const fn derive_gameplay_state_projection(
    paused: bool,
    game_over: bool,
) -> GameplayStateProjection {
    if game_over {
        GameplayStateProjection::GameOver
    } else if paused {
        GameplayStateProjection::Paused
    } else {
        GameplayStateProjection::Playing
    }
}

pub fn derive_state_snapshot_head_transition(
    previous: Option<StateSnapshotHeadPrevious>,
    snapshot: &AppliedStateSnapshot,
) -> StateSnapshotHeadTransition {
    let previous_wave = previous.map(|previous| previous.wave).unwrap_or_default();
    let previous_time_data = previous
        .map(|previous| previous.time_data)
        .unwrap_or_default();
    let gameplay_state = derive_gameplay_state_projection(snapshot.paused, snapshot.game_over);
    let last_wave_advanced = snapshot.wave > previous_wave;
    let last_net_seconds_rollback = snapshot.time_data < previous_time_data;
    let net_seconds_delta_i64 = i64::from(snapshot.time_data) - i64::from(previous_time_data);

    StateSnapshotHeadTransition {
        gameplay_state,
        gameplay_state_changed: previous
            .map(|previous| previous.gameplay_state != gameplay_state)
            .unwrap_or(false),
        last_wave_advanced,
        last_wave_advance_from: last_wave_advanced.then_some(previous_wave),
        last_wave_advance_to: last_wave_advanced.then_some(snapshot.wave),
        last_net_seconds_rollback,
        net_seconds_delta: net_seconds_delta_i64.clamp(i64::from(i32::MIN), i64::from(i32::MAX))
            as i32,
        wave_regressed: snapshot.wave < previous_wave,
        time_regressed: last_net_seconds_rollback,
    }
}

pub fn sample_changed_team_ids(changed_team_ids: &BTreeSet<u8>, limit: usize) -> Vec<u8> {
    changed_team_ids.iter().take(limit).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::{
        derive_gameplay_state_projection, derive_state_snapshot_core_inventory_transition,
        derive_state_snapshot_head_transition, sample_changed_team_ids,
        StateSnapshotCoreInventoryPrevious, StateSnapshotCoreInventorySemantics,
        StateSnapshotHeadPrevious,
    };
    use crate::session_state::{
        AppliedStateSnapshot, AppliedStateSnapshotCoreData, AppliedStateSnapshotCoreDataItem,
        AppliedStateSnapshotCoreDataTeam, GameplayStateProjection,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn inventory_map(teams: &[(u8, &[(u16, i32)])]) -> BTreeMap<u8, BTreeMap<u16, i32>> {
        teams
            .iter()
            .map(|&(team_id, items)| (team_id, BTreeMap::from_iter(items.iter().copied())))
            .collect()
    }

    fn inventory_previous(
        inventory_by_team: &BTreeMap<u8, BTreeMap<u16, i32>>,
        item_entry_count: usize,
        total_amount: i64,
        nonzero_item_count: usize,
    ) -> StateSnapshotCoreInventoryPrevious<'_> {
        StateSnapshotCoreInventoryPrevious {
            inventory_by_team,
            item_entry_count,
            total_amount,
            nonzero_item_count,
        }
    }

    fn core_team(team_id: u8, items: &[(u16, i32)]) -> AppliedStateSnapshotCoreDataTeam {
        AppliedStateSnapshotCoreDataTeam {
            team_id,
            items: items
                .iter()
                .map(|&(item_id, amount)| AppliedStateSnapshotCoreDataItem { item_id, amount })
                .collect(),
        }
    }

    fn core_data(teams: Vec<AppliedStateSnapshotCoreDataTeam>) -> AppliedStateSnapshotCoreData {
        AppliedStateSnapshotCoreData {
            team_count: u8::try_from(teams.len())
                .expect("test core_data team count should fit in u8"),
            teams,
        }
    }

    fn assert_inventory_totals(
        semantics: &StateSnapshotCoreInventorySemantics,
        item_entry_count: usize,
        total_amount: i64,
        nonzero_item_count: usize,
    ) {
        assert_eq!(semantics.item_entry_count, item_entry_count);
        assert_eq!(semantics.total_amount, total_amount);
        assert_eq!(semantics.nonzero_item_count, nonzero_item_count);
    }

    fn head_previous(
        wave: i32,
        time_data: i32,
        gameplay_state: GameplayStateProjection,
    ) -> StateSnapshotHeadPrevious {
        StateSnapshotHeadPrevious {
            wave,
            time_data,
            gameplay_state,
        }
    }

    fn snapshot_head(
        wave: i32,
        time_data: i32,
        paused: bool,
        game_over: bool,
    ) -> AppliedStateSnapshot {
        AppliedStateSnapshot {
            wave,
            time_data,
            paused,
            game_over,
            ..AppliedStateSnapshot::default()
        }
    }

    #[test]
    fn fold_core_inventory_uses_last_write_wins_for_duplicate_teams_and_items() {
        let semantics = StateSnapshotCoreInventorySemantics::from_core_data(&core_data(vec![
            core_team(1, &[(0, 10), (0, 20)]),
            core_team(1, &[(1, 30)]),
            core_team(2, &[(4, 40), (4, 0)]),
        ]));

        assert_eq!(semantics.duplicate_team_count, 1);
        assert_eq!(semantics.duplicate_item_count, 2);
        assert_eq!(
            semantics.inventory_by_team,
            inventory_map(&[(1u8, &[(0u16, 20), (1u16, 30)]), (2u8, &[(4u16, 0)])])
        );
        assert_inventory_totals(&semantics, 3, 50, 2);
    }

    #[test]
    fn derive_transition_reports_changed_teams_from_folded_inventory() {
        let previous_inventory = inventory_map(&[(1u8, &[(0u16, 10)]), (2u8, &[(4u16, 40)])]);
        let transition = derive_state_snapshot_core_inventory_transition(
            Some(inventory_previous(&previous_inventory, 2, 50, 2)),
            Some(&core_data(vec![
                core_team(1, &[(0, 11)]),
                core_team(3, &[(9, 90)]),
            ])),
        );

        assert!(transition.synced);
        assert_eq!(transition.changed_team_ids, BTreeSet::from([1u8, 2u8, 3u8]));
        assert_inventory_totals(&transition.inventory, 2, 101, 2);
    }

    #[test]
    fn derive_transition_without_core_data_reuses_previous_inventory_without_duplicates() {
        let previous_inventory =
            inventory_map(&[(1u8, &[(0u16, 10), (2u16, 0)]), (3u8, &[(4u16, 40)])]);

        let transition = derive_state_snapshot_core_inventory_transition(
            Some(inventory_previous(&previous_inventory, 3, 50, 2)),
            None,
        );

        assert!(!transition.synced);
        assert!(transition.changed_team_ids.is_empty());
        assert_eq!(transition.inventory.inventory_by_team, previous_inventory);
        assert_inventory_totals(&transition.inventory, 3, 50, 2);
        assert_eq!(transition.inventory.duplicate_team_count, 0);
        assert_eq!(transition.inventory.duplicate_item_count, 0);
    }

    #[test]
    fn derive_transition_without_previous_or_core_data_yields_empty_unsynced_inventory() {
        let transition = derive_state_snapshot_core_inventory_transition(None, None);

        assert!(!transition.synced);
        assert!(transition.changed_team_ids.is_empty());
        assert_eq!(
            transition.inventory,
            StateSnapshotCoreInventorySemantics::default()
        );
    }

    #[test]
    fn derive_transition_with_empty_core_data_marks_synced_without_changed_teams() {
        let transition =
            derive_state_snapshot_core_inventory_transition(None, Some(&core_data(Vec::new())));

        assert!(transition.synced);
        assert!(transition.changed_team_ids.is_empty());
        assert_eq!(
            transition.inventory,
            StateSnapshotCoreInventorySemantics::default()
        );
    }

    #[test]
    fn derive_state_snapshot_head_transition_prioritizes_gameover_over_paused() {
        for (paused, game_over, expected) in [
            (true, true, GameplayStateProjection::GameOver),
            (true, false, GameplayStateProjection::Paused),
            (false, false, GameplayStateProjection::Playing),
        ] {
            assert_eq!(
                derive_gameplay_state_projection(paused, game_over),
                expected
            );
        }
    }

    #[test]
    fn derive_state_snapshot_head_transition_marks_wave_advance_and_regressions() {
        let advanced = derive_state_snapshot_head_transition(
            Some(head_previous(6, 10, GameplayStateProjection::Playing)),
            &snapshot_head(7, 12, false, false),
        );
        assert!(advanced.last_wave_advanced);
        assert_eq!(advanced.last_wave_advance_from, Some(6));
        assert_eq!(advanced.last_wave_advance_to, Some(7));
        assert!(!advanced.wave_regressed);
        assert!(!advanced.time_regressed);

        let regressed = derive_state_snapshot_head_transition(
            Some(head_previous(7, 12, GameplayStateProjection::Paused)),
            &snapshot_head(5, 11, false, false),
        );
        assert!(!regressed.last_wave_advanced);
        assert_eq!(regressed.last_wave_advance_from, None);
        assert_eq!(regressed.last_wave_advance_to, None);
        assert!(regressed.wave_regressed);
        assert!(regressed.time_regressed);
    }

    #[test]
    fn derive_state_snapshot_head_transition_clamps_net_seconds_delta_and_marks_time_rollback() {
        let transition = derive_state_snapshot_head_transition(
            Some(head_previous(1, i32::MAX, GameplayStateProjection::Paused)),
            &snapshot_head(0, i32::MIN, false, false),
        );

        assert_eq!(transition.gameplay_state, GameplayStateProjection::Playing);
        assert!(transition.gameplay_state_changed);
        assert!(transition.last_net_seconds_rollback);
        assert!(transition.time_regressed);
        assert_eq!(transition.net_seconds_delta, i32::MIN);
    }

    #[test]
    fn sample_changed_team_ids_respects_btree_order_and_limit() {
        assert_eq!(
            sample_changed_team_ids(&BTreeSet::from([9u8, 1u8, 7u8]), 2),
            vec![1u8, 7u8]
        );
    }
}
