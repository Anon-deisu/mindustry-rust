use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderQueueEntryObservation {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueStage {
    Queued,
    InFlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderQueueEntry {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub stage: BuilderQueueStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueTransition {
    Started,
    Rejected,
    Finished,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BuilderQueueStateMachine {
    pub active_by_tile: BTreeMap<(i32, i32), BuilderQueueEntry>,
    pub ordered_tiles: Vec<(i32, i32)>,
    pub queued_count: usize,
    pub inflight_count: usize,
    pub finished_count: u64,
    pub rejected_count: u64,
    pub orphan_authoritative_count: u64,
    pub head_tile: Option<(i32, i32)>,
    pub last_transition: Option<BuilderQueueTransition>,
    pub last_removed_local_plan: bool,
    pub last_orphan_authoritative: bool,
}

impl BuilderQueueStateMachine {
    pub fn enqueue_local(
        &mut self,
        entry: BuilderQueueEntryObservation,
        tail: bool,
    ) -> Option<BuilderQueueEntry> {
        let key = (entry.x, entry.y);
        let previous = self.active_by_tile.remove(&key);
        self.ordered_tiles.retain(|tile| *tile != key);
        self.active_by_tile.insert(
            key,
            BuilderQueueEntry {
                x: entry.x,
                y: entry.y,
                breaking: entry.breaking,
                block_id: entry.block_id.or_else(|| {
                    previous
                        .as_ref()
                        .filter(|plan| plan.breaking == entry.breaking)
                        .and_then(|plan| plan.block_id)
                }),
                rotation: Some(entry.rotation),
                stage: BuilderQueueStage::Queued,
            },
        );
        if tail {
            self.ordered_tiles.push(key);
        } else {
            self.ordered_tiles.insert(0, key);
        }
        self.recount();
        previous
    }

    pub fn sync_local_entries<I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = BuilderQueueEntryObservation>,
    {
        let mut next = BTreeMap::new();
        let mut next_order = Vec::new();
        for entry in entries {
            let key = (entry.x, entry.y);
            let previous = self.active_by_tile.get(&key);
            let stage = if previous.is_some_and(|plan| plan.stage == BuilderQueueStage::InFlight) {
                BuilderQueueStage::InFlight
            } else {
                BuilderQueueStage::Queued
            };
            next.insert(
                key,
                BuilderQueueEntry {
                    x: entry.x,
                    y: entry.y,
                    breaking: entry.breaking,
                    block_id: entry
                        .block_id
                        .or_else(|| previous.and_then(|plan| plan.block_id)),
                    rotation: Some(entry.rotation),
                    stage,
                },
            );
            next_order.retain(|tile| *tile != key);
            next_order.push(key);
        }

        self.active_by_tile = next;
        self.ordered_tiles = next_order;
        self.recount();
    }

    pub fn mark_begin(
        &mut self,
        x: i32,
        y: i32,
        breaking: bool,
        block_id: Option<i16>,
        rotation: u8,
    ) {
        let key = (x, y);
        let previous = self
            .active_by_tile
            .get(&key)
            .filter(|entry| entry.breaking == breaking);
        self.active_by_tile.insert(
            key,
            BuilderQueueEntry {
                x,
                y,
                breaking,
                block_id: block_id.or_else(|| previous.and_then(|entry| entry.block_id)),
                rotation: Some(rotation),
                stage: BuilderQueueStage::InFlight,
            },
        );
        self.promote_to_front(key);
        self.last_transition = Some(BuilderQueueTransition::Started);
        self.last_removed_local_plan = false;
        self.last_orphan_authoritative = false;
        self.recount();
    }

    pub fn mark_reject(&mut self, x: i32, y: i32, breaking: bool, removed_local_plan: bool) {
        let previous = self.remove_matching_entry(x, y, breaking);
        let orphan_authoritative = previous.is_none() && !removed_local_plan;
        if orphan_authoritative {
            self.orphan_authoritative_count = self.orphan_authoritative_count.saturating_add(1);
        }
        self.rejected_count = self.rejected_count.saturating_add(1);
        self.last_transition = Some(BuilderQueueTransition::Rejected);
        self.last_removed_local_plan = removed_local_plan;
        self.last_orphan_authoritative = orphan_authoritative;
        self.recount();
    }

    pub fn mark_finish(&mut self, x: i32, y: i32, breaking: bool, removed_local_plan: bool) {
        let previous = self.remove_matching_entry(x, y, breaking);
        let orphan_authoritative = previous.is_none() && !removed_local_plan;
        if orphan_authoritative {
            self.orphan_authoritative_count = self.orphan_authoritative_count.saturating_add(1);
        }
        self.finished_count = self.finished_count.saturating_add(1);
        self.last_transition = Some(BuilderQueueTransition::Finished);
        self.last_removed_local_plan = removed_local_plan;
        self.last_orphan_authoritative = orphan_authoritative;
        self.recount();
    }

    pub fn move_to_front(&mut self, x: i32, y: i32, breaking: bool) -> bool {
        let key = (x, y);
        if self
            .active_by_tile
            .get(&key)
            .is_some_and(|entry| entry.breaking == breaking)
        {
            self.promote_to_front(key);
            self.recount();
            true
        } else {
            false
        }
    }

    pub fn remove_local_entry(
        &mut self,
        x: i32,
        y: i32,
        breaking: bool,
    ) -> Option<BuilderQueueEntry> {
        let removed = self.remove_matching_entry(x, y, breaking);
        if removed.is_some() {
            self.recount();
        }
        removed
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_building(&self) -> bool {
        !self.active_by_tile.is_empty()
    }

    pub fn head_entry(&self) -> Option<&BuilderQueueEntry> {
        self.head_tile
            .and_then(|tile| self.active_by_tile.get(&tile))
    }

    fn remove_matching_entry(
        &mut self,
        x: i32,
        y: i32,
        breaking: bool,
    ) -> Option<BuilderQueueEntry> {
        let key = (x, y);
        if self
            .active_by_tile
            .get(&key)
            .is_some_and(|entry| entry.breaking == breaking)
        {
            self.ordered_tiles.retain(|tile| *tile != key);
            self.active_by_tile.remove(&key)
        } else {
            None
        }
    }

    fn promote_to_front(&mut self, key: (i32, i32)) {
        self.ordered_tiles.retain(|tile| *tile != key);
        self.ordered_tiles.insert(0, key);
    }

    fn recount(&mut self) {
        self.queued_count = self
            .active_by_tile
            .values()
            .filter(|entry| entry.stage == BuilderQueueStage::Queued)
            .count();
        self.inflight_count = self
            .active_by_tile
            .values()
            .filter(|entry| entry.stage == BuilderQueueStage::InFlight)
            .count();
        self.ordered_tiles
            .retain(|tile| self.active_by_tile.contains_key(tile));
        self.head_tile = self.ordered_tiles.first().copied();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuilderQueueEntry, BuilderQueueEntryObservation, BuilderQueueStage,
        BuilderQueueStateMachine, BuilderQueueTransition,
    };
    use std::collections::BTreeMap;

    #[test]
    fn sync_local_entries_dedupes_same_tile_with_tail_wins() {
        let mut queue = BuilderQueueStateMachine::default();

        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(5),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(9),
                rotation: 2,
            },
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        assert_eq!(
            queue.active_by_tile,
            BTreeMap::from([
                (
                    (1, 1),
                    BuilderQueueEntry {
                        x: 1,
                        y: 1,
                        breaking: false,
                        block_id: Some(9),
                        rotation: Some(2),
                        stage: BuilderQueueStage::Queued,
                    },
                ),
                (
                    (4, 4),
                    BuilderQueueEntry {
                        x: 4,
                        y: 4,
                        breaking: true,
                        block_id: None,
                        rotation: Some(0),
                        stage: BuilderQueueStage::Queued,
                    },
                ),
            ])
        );
        assert_eq!(queue.ordered_tiles, vec![(1, 1), (4, 4)]);
        assert_eq!(queue.head_tile, Some((1, 1)));
        assert_eq!(queue.queued_count, 2);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn start_reject_finish_sequence_is_stable() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 10,
                y: 10,
                breaking: false,
                block_id: Some(1),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 20,
                y: 20,
                breaking: false,
                block_id: Some(2),
                rotation: 1,
            },
        ]);

        queue.mark_begin(20, 20, false, Some(7), 3);
        assert_eq!(queue.head_tile, Some((20, 20)));
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.inflight_count, 1);

        queue.mark_reject(20, 20, false, true);
        assert_eq!(queue.head_tile, Some((10, 10)));
        assert_eq!(
            queue.last_transition,
            Some(BuilderQueueTransition::Rejected)
        );
        assert_eq!(queue.rejected_count, 1);
        assert!(!queue.last_orphan_authoritative);
        assert_eq!(queue.inflight_count, 0);

        queue.mark_begin(10, 10, false, Some(9), 2);
        assert_eq!(queue.head_tile, Some((10, 10)));
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.inflight_count, 1);

        queue.mark_finish(10, 10, false, true);
        assert_eq!(queue.head_tile, None);
        assert_eq!(
            queue.last_transition,
            Some(BuilderQueueTransition::Finished)
        );
        assert_eq!(queue.finished_count, 1);
        assert_eq!(queue.inflight_count, 0);
        assert_eq!(queue.queued_count, 0);
    }

    #[test]
    fn remove_orphan_authoritative_does_not_corrupt_head_order() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(1),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 3,
                y: 3,
                breaking: false,
                block_id: Some(3),
                rotation: 1,
            },
        ]);
        let expected_order = queue.ordered_tiles.clone();
        let expected_head = queue.head_tile;
        let expected_active = queue.active_by_tile.clone();

        queue.mark_reject(99, 99, false, false);

        assert_eq!(queue.ordered_tiles, expected_order);
        assert_eq!(queue.head_tile, expected_head);
        assert_eq!(queue.active_by_tile, expected_active);
        assert_eq!(queue.rejected_count, 1);
        assert_eq!(queue.orphan_authoritative_count, 1);
        assert_eq!(
            queue.last_transition,
            Some(BuilderQueueTransition::Rejected)
        );
        assert!(queue.last_orphan_authoritative);
        assert!(!queue.last_removed_local_plan);
    }

    #[test]
    fn move_to_front_promotes_exact_matching_local_entry_without_touching_counters() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(1),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: true,
                block_id: None,
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 3,
                y: 3,
                breaking: false,
                block_id: Some(3),
                rotation: 2,
            },
        ]);
        queue.last_transition = Some(BuilderQueueTransition::Started);
        queue.rejected_count = 5;
        queue.finished_count = 7;

        assert!(queue.move_to_front(2, 2, true));

        assert_eq!(queue.ordered_tiles, vec![(2, 2), (1, 1), (3, 3)]);
        assert_eq!(queue.head_tile, Some((2, 2)));
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.rejected_count, 5);
        assert_eq!(queue.finished_count, 7);
        assert_eq!(queue.queued_count, 3);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn move_to_front_requires_exact_breaking_match() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 4,
            y: 4,
            breaking: false,
            block_id: Some(9),
            rotation: 2,
        }]);
        let expected_order = queue.ordered_tiles.clone();

        assert!(!queue.move_to_front(4, 4, true));

        assert_eq!(queue.ordered_tiles, expected_order);
        assert_eq!(queue.head_tile, Some((4, 4)));
    }

    #[test]
    fn remove_local_entry_removes_exact_matching_plan_without_affecting_authoritative_counts() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: false,
                block_id: Some(11),
                rotation: 3,
            },
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);
        queue.last_transition = Some(BuilderQueueTransition::Finished);
        queue.orphan_authoritative_count = 4;

        let removed = queue.remove_local_entry(6, 6, true);

        assert_eq!(
            removed,
            Some(BuilderQueueEntry {
                x: 6,
                y: 6,
                breaking: true,
                block_id: None,
                rotation: Some(0),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(5, 5)]);
        assert_eq!(queue.head_tile, Some((5, 5)));
        assert_eq!(queue.queued_count, 1);
        assert_eq!(
            queue.last_transition,
            Some(BuilderQueueTransition::Finished)
        );
        assert_eq!(queue.orphan_authoritative_count, 4);
    }

    #[test]
    fn remove_local_entry_keeps_opposite_breaking_plan_on_same_tile() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 8,
            y: 8,
            breaking: false,
            block_id: Some(13),
            rotation: 1,
        }]);

        assert_eq!(queue.remove_local_entry(8, 8, true), None);

        assert_eq!(queue.ordered_tiles, vec![(8, 8)]);
        assert_eq!(queue.head_tile, Some((8, 8)));
        assert_eq!(queue.queued_count, 1);
    }

    #[test]
    fn enqueue_local_tail_replaces_same_tile_and_appends_to_queue_tail() {
        let mut queue = BuilderQueueStateMachine::default();

        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: 0,
            },
            true,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(20),
                rotation: 1,
            },
            true,
        );
        let replaced = queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: true,
                block_id: None,
                rotation: 3,
            },
            true,
        );

        assert_eq!(
            replaced,
            Some(BuilderQueueEntry {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: Some(0),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(2, 2), (1, 1)]);
        assert_eq!(queue.head_tile, Some((2, 2)));
        assert_eq!(queue.head_entry().map(|entry| entry.breaking), Some(false));
        assert_eq!(
            queue
                .active_by_tile
                .get(&(1, 1))
                .and_then(|entry| entry.block_id),
            None
        );
        assert!(queue.is_building());
        assert_eq!(queue.queued_count, 2);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn enqueue_local_front_replaces_inflight_tile_and_downgrades_to_queued_head() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(40),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: false,
                block_id: Some(50),
                rotation: 2,
            },
        ]);
        queue.mark_begin(5, 5, false, Some(55), 3);

        let replaced = queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: false,
                block_id: Some(51),
                rotation: 0,
            },
            false,
        );

        assert_eq!(
            replaced,
            Some(BuilderQueueEntry {
                x: 5,
                y: 5,
                breaking: false,
                block_id: Some(55),
                rotation: Some(3),
                stage: BuilderQueueStage::InFlight,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(5, 5), (4, 4)]);
        assert_eq!(queue.head_tile, Some((5, 5)));
        assert_eq!(
            queue.head_entry().map(|entry| entry.stage),
            Some(BuilderQueueStage::Queued)
        );
        assert_eq!(
            queue.head_entry().and_then(|entry| entry.block_id),
            Some(51)
        );
        assert_eq!(queue.queued_count, 2);
        assert_eq!(queue.inflight_count, 0);
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
    }

    #[test]
    fn enqueue_local_sequence_keeps_head_selection_deterministic_across_mixed_front_and_tail_ops() {
        let mut queue = BuilderQueueStateMachine::default();

        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: 0,
            },
            true,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(20),
                rotation: 1,
            },
            true,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: true,
                block_id: None,
                rotation: 2,
            },
            true,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 3,
                y: 3,
                breaking: false,
                block_id: Some(30),
                rotation: 3,
            },
            false,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
            false,
        );

        assert_eq!(queue.ordered_tiles, vec![(2, 2), (3, 3), (1, 1)]);
        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 2,
                y: 2,
                breaking: true,
                block_id: None,
                rotation: Some(0),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.queued_count, 3);
        assert_eq!(queue.inflight_count, 0);
    }
}
