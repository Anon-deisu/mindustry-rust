use crate::session_state::AppliedStateSnapshotCoreData;
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

#[cfg(test)]
mod tests {
    use super::{
        derive_state_snapshot_core_inventory_transition, StateSnapshotCoreInventoryPrevious,
        StateSnapshotCoreInventorySemantics,
    };
    use crate::session_state::{
        AppliedStateSnapshotCoreData, AppliedStateSnapshotCoreDataItem,
        AppliedStateSnapshotCoreDataTeam,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn fold_core_inventory_uses_last_write_wins_for_duplicate_teams_and_items() {
        let semantics =
            StateSnapshotCoreInventorySemantics::from_core_data(&AppliedStateSnapshotCoreData {
                team_count: 3,
                teams: vec![
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 0,
                                amount: 10,
                            },
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 0,
                                amount: 20,
                            },
                        ],
                    },
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![AppliedStateSnapshotCoreDataItem {
                            item_id: 1,
                            amount: 30,
                        }],
                    },
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 2,
                        items: vec![
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 4,
                                amount: 40,
                            },
                            AppliedStateSnapshotCoreDataItem {
                                item_id: 4,
                                amount: 0,
                            },
                        ],
                    },
                ],
            });

        assert_eq!(semantics.duplicate_team_count, 1);
        assert_eq!(semantics.duplicate_item_count, 2);
        assert_eq!(
            semantics.inventory_by_team,
            BTreeMap::from([
                (1u8, BTreeMap::from([(0u16, 20), (1u16, 30)])),
                (2u8, BTreeMap::from([(4u16, 0)])),
            ])
        );
        assert_eq!(semantics.item_entry_count, 3);
        assert_eq!(semantics.total_amount, 50);
        assert_eq!(semantics.nonzero_item_count, 2);
    }

    #[test]
    fn derive_transition_reports_changed_teams_from_folded_inventory() {
        let transition = derive_state_snapshot_core_inventory_transition(
            Some(StateSnapshotCoreInventoryPrevious {
                inventory_by_team: &BTreeMap::from([
                    (1u8, BTreeMap::from([(0u16, 10)])),
                    (2u8, BTreeMap::from([(4u16, 40)])),
                ]),
                item_entry_count: 2,
                total_amount: 50,
                nonzero_item_count: 2,
            }),
            Some(&AppliedStateSnapshotCoreData {
                team_count: 3,
                teams: vec![
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 1,
                        items: vec![AppliedStateSnapshotCoreDataItem {
                            item_id: 0,
                            amount: 11,
                        }],
                    },
                    AppliedStateSnapshotCoreDataTeam {
                        team_id: 3,
                        items: vec![AppliedStateSnapshotCoreDataItem {
                            item_id: 9,
                            amount: 90,
                        }],
                    },
                ],
            }),
        );

        assert!(transition.synced);
        assert_eq!(transition.changed_team_ids, BTreeSet::from([1u8, 2u8, 3u8]));
        assert_eq!(transition.inventory.item_entry_count, 2);
        assert_eq!(transition.inventory.total_amount, 101);
        assert_eq!(transition.inventory.nonzero_item_count, 2);
    }
}
