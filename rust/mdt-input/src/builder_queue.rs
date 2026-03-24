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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderQueueActivityObservation {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub in_range: bool,
    pub should_skip: bool,
    pub distance_sq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueHeadSelection {
    QueueEmpty,
    HeadInRange,
    ReorderedToInRange,
    FallbackToClosestInRange,
    SkippedInRange,
    HeadOutOfRange,
    ObservationMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderQueueActivityState {
    pub head_tile: Option<(i32, i32)>,
    pub actively_building: bool,
    pub head_in_range: bool,
    pub head_should_skip: bool,
    pub reordered: bool,
    pub used_closest_in_range_fallback: bool,
    pub head_selection: BuilderQueueHeadSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderQueueTileStateObservation {
    pub x: i32,
    pub y: i32,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub requires_rotation_match: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BuilderQueueValidationResult {
    pub removed_count: usize,
    pub removed_head: bool,
    pub removed_tiles: Vec<(i32, i32)>,
    pub head_tile_before: Option<(i32, i32)>,
    pub head_tile_after: Option<(i32, i32)>,
    pub reconcile_outcome: BuilderQueueReconcileOutcome,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueReconcileOutcome {
    #[default]
    Unchanged,
    RemovedNonHead,
    AdvancedHead,
    ClearedQueue,
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
                block_id: Self::resolve_block_id(entry.block_id, previous.as_ref(), entry.breaking),
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
        let mut incoming_counts = BTreeMap::new();
        let mut incoming_order = Vec::new();
        for entry in entries {
            let key = (entry.x, entry.y);
            let previous = Self::matching_entry(self.active_by_tile.get(&key), entry.breaking);
            incoming_counts
                .entry(key)
                .and_modify(|count| *count += 1)
                .or_insert(1usize);
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
                    block_id: Self::resolve_block_id(entry.block_id, previous, entry.breaking),
                    rotation: Some(entry.rotation),
                    stage,
                },
            );
            incoming_order.retain(|tile| *tile != key);
            incoming_order.push(key);
        }

        let mut next_order = self
            .ordered_tiles
            .iter()
            .copied()
            .filter(|tile| next.contains_key(tile) && incoming_counts.get(tile) == Some(&1usize))
            .collect::<Vec<_>>();
        for key in incoming_order {
            if !next_order.contains(&key) {
                next_order.push(key);
            }
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
        let previous = Self::matching_entry(self.active_by_tile.get(&key), breaking);
        self.active_by_tile.insert(
            key,
            BuilderQueueEntry {
                x,
                y,
                breaking,
                block_id: Self::resolve_block_id(block_id, previous, breaking),
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

    pub fn update_local_activity<I>(&mut self, observations: I) -> BuilderQueueActivityState
    where
        I: IntoIterator<Item = BuilderQueueActivityObservation>,
    {
        let observations_by_key = observations
            .into_iter()
            .map(|observation| {
                (
                    (observation.x, observation.y, observation.breaking),
                    observation,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let original_order = self.ordered_tiles.clone();
        let mut reordered = false;
        let mut used_closest_in_range_fallback = false;
        let mut missing_observation = false;

        if self.ordered_tiles.len() > 1 {
            let mut total = 0usize;
            let size = self.ordered_tiles.len();
            let mut best_index = None;
            let mut best_distance = u64::MAX;
            let mut found_valid_head = false;
            let original_head_in_range = self
                .head_entry()
                .and_then(|entry| Self::activity_observation(&observations_by_key, entry))
                .is_some_and(|observation| observation.in_range);

            while total < size {
                let Some(tile) = self.ordered_tiles.first().copied() else {
                    break;
                };
                let Some(entry) = self.active_by_tile.get(&tile) else {
                    break;
                };
                let Some(observation) = Self::activity_observation(&observations_by_key, entry)
                else {
                    missing_observation = true;
                    self.ordered_tiles = original_order.clone();
                    break;
                };

                if observation.in_range && !observation.should_skip {
                    found_valid_head = true;
                    reordered = total > 0;
                    break;
                }
                if observation.in_range && observation.distance_sq < best_distance {
                    best_distance = observation.distance_sq;
                    best_index = Some(total);
                }

                self.ordered_tiles.rotate_left(1);
                total += 1;
            }

            if !found_valid_head {
                self.ordered_tiles = original_order;
                if let Some(best_index) = best_index.filter(|index| *index > 0) {
                    if !original_head_in_range {
                        self.ordered_tiles.rotate_left(best_index);
                        reordered = true;
                        used_closest_in_range_fallback = true;
                    }
                }
            }
        }

        self.recount();
        let head_observation = self
            .head_entry()
            .and_then(|entry| Self::activity_observation(&observations_by_key, entry));
        let head_in_range = head_observation.is_some_and(|observation| observation.in_range);
        let head_should_skip = head_observation.is_some_and(|observation| observation.should_skip);
        let head_selection = if self.head_tile.is_none() {
            BuilderQueueHeadSelection::QueueEmpty
        } else if missing_observation || head_observation.is_none() {
            BuilderQueueHeadSelection::ObservationMissing
        } else if used_closest_in_range_fallback {
            BuilderQueueHeadSelection::FallbackToClosestInRange
        } else if reordered {
            BuilderQueueHeadSelection::ReorderedToInRange
        } else if head_in_range && !head_should_skip {
            BuilderQueueHeadSelection::HeadInRange
        } else if head_in_range {
            BuilderQueueHeadSelection::SkippedInRange
        } else {
            BuilderQueueHeadSelection::HeadOutOfRange
        };
        BuilderQueueActivityState {
            head_tile: self.head_tile,
            actively_building: head_in_range,
            head_in_range,
            head_should_skip,
            reordered,
            used_closest_in_range_fallback,
            head_selection,
        }
    }

    pub fn validate_against_tile_states<I>(
        &mut self,
        observations: I,
    ) -> BuilderQueueValidationResult
    where
        I: IntoIterator<Item = BuilderQueueTileStateObservation>,
    {
        let observations_by_tile = observations
            .into_iter()
            .map(|observation| ((observation.x, observation.y), observation))
            .collect::<BTreeMap<_, _>>();
        let original_head = self.head_tile;
        let mut removed_tiles = Vec::new();

        self.ordered_tiles.retain(|tile| {
            let Some(entry) = self.active_by_tile.get(tile) else {
                return false;
            };
            let Some(observation) = observations_by_tile.get(tile) else {
                return true;
            };
            let should_remove = Self::entry_matches_tile_state(entry, observation);
            if should_remove {
                removed_tiles.push(*tile);
            }
            !should_remove
        });

        for tile in &removed_tiles {
            self.active_by_tile.remove(tile);
        }

        self.recount();
        let head_tile_after = self.head_tile;
        let removed_head = original_head.is_some_and(|head| removed_tiles.contains(&head));
        let reconcile_outcome = if removed_tiles.is_empty() {
            BuilderQueueReconcileOutcome::Unchanged
        } else if removed_head {
            if head_tile_after.is_none() {
                BuilderQueueReconcileOutcome::ClearedQueue
            } else {
                BuilderQueueReconcileOutcome::AdvancedHead
            }
        } else {
            BuilderQueueReconcileOutcome::RemovedNonHead
        };
        BuilderQueueValidationResult {
            removed_count: removed_tiles.len(),
            removed_head,
            removed_tiles,
            head_tile_before: original_head,
            head_tile_after,
            reconcile_outcome,
        }
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

    fn matching_entry<'a>(
        entry: Option<&'a BuilderQueueEntry>,
        breaking: bool,
    ) -> Option<&'a BuilderQueueEntry> {
        entry.filter(|entry| entry.breaking == breaking)
    }

    fn resolve_block_id(
        block_id: Option<i16>,
        previous: Option<&BuilderQueueEntry>,
        breaking: bool,
    ) -> Option<i16> {
        block_id
            .or_else(|| Self::matching_entry(previous, breaking).and_then(|entry| entry.block_id))
    }

    fn activity_observation<'a>(
        observations_by_key: &'a BTreeMap<(i32, i32, bool), BuilderQueueActivityObservation>,
        entry: &BuilderQueueEntry,
    ) -> Option<&'a BuilderQueueActivityObservation> {
        observations_by_key.get(&(entry.x, entry.y, entry.breaking))
    }

    fn entry_matches_tile_state(
        entry: &BuilderQueueEntry,
        observation: &BuilderQueueTileStateObservation,
    ) -> bool {
        if entry.breaking {
            observation.block_id.is_none()
        } else {
            observation.block_id == entry.block_id
                && (!observation.requires_rotation_match
                    || observation
                        .rotation
                        .zip(entry.rotation)
                        .is_some_and(|(lhs, rhs)| lhs == rhs))
        }
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
        BuilderQueueActivityObservation, BuilderQueueActivityState, BuilderQueueEntry,
        BuilderQueueEntryObservation, BuilderQueueHeadSelection, BuilderQueueReconcileOutcome,
        BuilderQueueStage, BuilderQueueStateMachine, BuilderQueueTileStateObservation,
        BuilderQueueTransition, BuilderQueueValidationResult,
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
    fn sync_local_entries_does_not_leak_inflight_stage_or_block_id_across_break_mode_switch() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.mark_begin(6, 6, false, Some(42), 1);

        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 6,
            y: 6,
            breaking: true,
            block_id: None,
            rotation: 0,
        }]);

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 6,
                y: 6,
                breaking: true,
                block_id: None,
                rotation: Some(0),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn sync_local_entries_preserves_inflight_stage_and_block_id_for_same_break_mode() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.mark_begin(7, 7, false, Some(70), 2);

        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 7,
            y: 7,
            breaking: false,
            block_id: None,
            rotation: 3,
        }]);

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 7,
                y: 7,
                breaking: false,
                block_id: Some(70),
                rotation: Some(3),
                stage: BuilderQueueStage::InFlight,
            })
        );
        assert_eq!(queue.queued_count, 0);
        assert_eq!(queue.inflight_count, 1);
    }

    #[test]
    fn mark_begin_does_not_leak_block_id_across_break_mode_switch() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 9,
            y: 9,
            breaking: false,
            block_id: Some(90),
            rotation: 1,
        }]);

        queue.mark_begin(9, 9, true, None, 3);

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 9,
                y: 9,
                breaking: true,
                block_id: None,
                rotation: Some(3),
                stage: BuilderQueueStage::InFlight,
            })
        );
        assert_eq!(queue.queued_count, 0);
        assert_eq!(queue.inflight_count, 1);
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
    }

    #[test]
    fn sync_local_entries_preserves_existing_relative_order_for_unique_tiles() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(11),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(22),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 3,
                y: 3,
                breaking: true,
                block_id: None,
                rotation: 2,
            },
        ]);
        queue.move_to_front(3, 3, true);

        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(22),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(11),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 3,
                y: 3,
                breaking: true,
                block_id: None,
                rotation: 2,
            },
        ]);

        assert_eq!(queue.ordered_tiles, vec![(3, 3), (1, 1), (2, 2)]);
        assert_eq!(queue.head_tile, Some((3, 3)));
    }

    #[test]
    fn sync_local_entries_appends_new_tiles_without_reordering_existing_head() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(44),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: true,
                block_id: None,
                rotation: 1,
            },
        ]);
        queue.move_to_front(5, 5, true);

        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(44),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: false,
                block_id: Some(66),
                rotation: 2,
            },
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: true,
                block_id: None,
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 7,
                y: 7,
                breaking: false,
                block_id: Some(77),
                rotation: 3,
            },
        ]);

        assert_eq!(queue.ordered_tiles, vec![(5, 5), (4, 4), (6, 6), (7, 7)]);
        assert_eq!(queue.head_tile, Some((5, 5)));
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

    #[test]
    fn update_local_activity_rotates_to_first_in_range_unskipped_plan() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
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
                x: 3,
                y: 3,
                breaking: false,
                block_id: Some(30),
                rotation: 2,
            },
        ]);
        queue.last_transition = Some(BuilderQueueTransition::Started);

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 1,
                y: 1,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 81,
            },
            BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 25,
            },
            BuilderQueueActivityObservation {
                x: 3,
                y: 3,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 9,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((3, 3)),
                actively_building: true,
                head_in_range: true,
                head_should_skip: false,
                reordered: true,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::ReorderedToInRange,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(3, 3), (1, 1), (2, 2)]);
        assert_eq!(queue.head_tile, Some((3, 3)));
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.queued_count, 3);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn update_local_activity_falls_back_to_closest_in_range_plan_when_all_are_skipped() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
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
                x: 3,
                y: 3,
                breaking: true,
                block_id: None,
                rotation: 2,
            },
        ]);

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 1,
                y: 1,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 100,
            },
            BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 16,
            },
            BuilderQueueActivityObservation {
                x: 3,
                y: 3,
                breaking: true,
                in_range: true,
                should_skip: true,
                distance_sq: 4,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((3, 3)),
                actively_building: true,
                head_in_range: true,
                head_should_skip: true,
                reordered: true,
                used_closest_in_range_fallback: true,
                head_selection: BuilderQueueHeadSelection::FallbackToClosestInRange,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(3, 3), (1, 1), (2, 2)]);
        assert_eq!(queue.head_tile, Some((3, 3)));
    }

    #[test]
    fn update_local_activity_keeps_in_range_skipped_head_without_closest_fallback() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
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
                x: 3,
                y: 3,
                breaking: true,
                block_id: None,
                rotation: 2,
            },
        ]);

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 1,
                y: 1,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 36,
            },
            BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 4,
            },
            BuilderQueueActivityObservation {
                x: 3,
                y: 3,
                breaking: true,
                in_range: false,
                should_skip: false,
                distance_sq: 100,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((1, 1)),
                actively_building: true,
                head_in_range: true,
                head_should_skip: true,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::SkippedInRange,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(1, 1), (2, 2), (3, 3)]);
        assert_eq!(queue.head_tile, Some((1, 1)));
    }

    #[test]
    fn update_local_activity_keeps_order_stable_when_observations_are_incomplete() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(40),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: false,
                block_id: Some(50),
                rotation: 1,
            },
        ]);
        let expected_order = queue.ordered_tiles.clone();

        let activity = queue.update_local_activity([BuilderQueueActivityObservation {
            x: 5,
            y: 5,
            breaking: false,
            in_range: true,
            should_skip: false,
            distance_sq: 1,
        }]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((4, 4)),
                actively_building: false,
                head_in_range: false,
                head_should_skip: false,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::ObservationMissing,
            }
        );
        assert_eq!(queue.ordered_tiles, expected_order);
        assert_eq!(queue.head_tile, Some((4, 4)));
        assert_eq!(queue.queued_count, 2);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn update_local_activity_reports_head_out_of_range_without_reordering() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 8,
                y: 8,
                breaking: false,
                block_id: Some(80),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 9,
                y: 9,
                breaking: false,
                block_id: Some(90),
                rotation: 1,
            },
        ]);

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 8,
                y: 8,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 64,
            },
            BuilderQueueActivityObservation {
                x: 9,
                y: 9,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 16,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((8, 8)),
                actively_building: false,
                head_in_range: false,
                head_should_skip: false,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::HeadOutOfRange,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(8, 8), (9, 9)]);
    }

    #[test]
    fn validate_against_tile_states_removes_matching_place_and_break_entries() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: 2,
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
                block_id: Some(30),
                rotation: 1,
            },
        ]);

        let validation = queue.validate_against_tile_states([
            BuilderQueueTileStateObservation {
                x: 1,
                y: 1,
                block_id: Some(10),
                rotation: Some(2),
                requires_rotation_match: true,
            },
            BuilderQueueTileStateObservation {
                x: 2,
                y: 2,
                block_id: None,
                rotation: None,
                requires_rotation_match: false,
            },
            BuilderQueueTileStateObservation {
                x: 3,
                y: 3,
                block_id: Some(30),
                rotation: Some(0),
                requires_rotation_match: true,
            },
        ]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 2,
                removed_head: true,
                removed_tiles: vec![(1, 1), (2, 2)],
                head_tile_before: Some((1, 1)),
                head_tile_after: Some((3, 3)),
                reconcile_outcome: BuilderQueueReconcileOutcome::AdvancedHead,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(3, 3)]);
        assert_eq!(queue.head_tile, Some((3, 3)));
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn validate_against_tile_states_keeps_place_entry_when_rotation_is_unknown_or_mismatched() {
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
                rotation: 3,
            },
        ]);

        let validation = queue.validate_against_tile_states([
            BuilderQueueTileStateObservation {
                x: 4,
                y: 4,
                block_id: Some(40),
                rotation: None,
                requires_rotation_match: true,
            },
            BuilderQueueTileStateObservation {
                x: 5,
                y: 5,
                block_id: Some(50),
                rotation: Some(2),
                requires_rotation_match: true,
            },
        ]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 0,
                removed_head: false,
                removed_tiles: Vec::new(),
                head_tile_before: Some((4, 4)),
                head_tile_after: Some((4, 4)),
                reconcile_outcome: BuilderQueueReconcileOutcome::Unchanged,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(4, 4), (5, 5)]);
        assert_eq!(queue.head_tile, Some((4, 4)));
        assert_eq!(queue.queued_count, 2);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn validate_against_tile_states_removes_place_entry_when_rotation_match_is_not_required() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: false,
                block_id: Some(60),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 7,
                y: 7,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        let validation = queue.validate_against_tile_states([BuilderQueueTileStateObservation {
            x: 6,
            y: 6,
            block_id: Some(60),
            rotation: None,
            requires_rotation_match: false,
        }]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 1,
                removed_head: true,
                removed_tiles: vec![(6, 6)],
                head_tile_before: Some((6, 6)),
                head_tile_after: Some((7, 7)),
                reconcile_outcome: BuilderQueueReconcileOutcome::AdvancedHead,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(7, 7)]);
        assert_eq!(queue.head_tile, Some((7, 7)));
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn validate_against_tile_states_reports_removed_non_head_when_head_stays_put() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 11,
                y: 11,
                breaking: false,
                block_id: Some(110),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 12,
                y: 12,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        let validation =
            queue.validate_against_tile_states([BuilderQueueTileStateObservation {
                x: 12,
                y: 12,
                block_id: None,
                rotation: None,
                requires_rotation_match: false,
            }]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 1,
                removed_head: false,
                removed_tiles: vec![(12, 12)],
                head_tile_before: Some((11, 11)),
                head_tile_after: Some((11, 11)),
                reconcile_outcome: BuilderQueueReconcileOutcome::RemovedNonHead,
            }
        );
        assert_eq!(queue.head_tile, Some((11, 11)));
        assert_eq!(queue.ordered_tiles, vec![(11, 11)]);
    }

    #[test]
    fn validate_against_tile_states_reports_cleared_queue_when_last_head_is_removed() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 13,
            y: 13,
            breaking: false,
            block_id: Some(130),
            rotation: 2,
        }]);

        let validation =
            queue.validate_against_tile_states([BuilderQueueTileStateObservation {
                x: 13,
                y: 13,
                block_id: Some(130),
                rotation: Some(2),
                requires_rotation_match: true,
            }]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 1,
                removed_head: true,
                removed_tiles: vec![(13, 13)],
                head_tile_before: Some((13, 13)),
                head_tile_after: None,
                reconcile_outcome: BuilderQueueReconcileOutcome::ClearedQueue,
            }
        );
        assert_eq!(queue.head_tile, None);
        assert!(queue.ordered_tiles.is_empty());
    }

    #[test]
    fn validate_against_tile_states_ignores_tiles_without_observation() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: false,
                block_id: Some(60),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 7,
                y: 7,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        let validation = queue.validate_against_tile_states([BuilderQueueTileStateObservation {
            x: 6,
            y: 6,
            block_id: Some(99),
            rotation: Some(0),
            requires_rotation_match: true,
        }]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 0,
                removed_head: false,
                removed_tiles: Vec::new(),
                head_tile_before: Some((6, 6)),
                head_tile_after: Some((6, 6)),
                reconcile_outcome: BuilderQueueReconcileOutcome::Unchanged,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(6, 6), (7, 7)]);
        assert_eq!(queue.head_tile, Some((6, 6)));
        assert_eq!(queue.queued_count, 2);
        assert_eq!(queue.inflight_count, 0);
    }
}
