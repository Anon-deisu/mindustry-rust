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

impl BuilderQueueStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InFlight => "in-flight",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderQueueEntry {
    pub x: i32,
    pub y: i32,
    pub breaking: bool,
    pub block_id: Option<i16>,
    pub rotation: Option<u8>,
    pub progress_permyriad: Option<u16>,
    pub stage: BuilderQueueStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueTransition {
    Started,
    Rejected,
    Finished,
}

impl BuilderQueueTransition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Rejected => "rejected",
            Self::Finished => "finished",
        }
    }
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
pub enum BuilderQueueSkipReason {
    ObservationMissing,
    OutOfRange,
    RequestedSkip,
}

impl BuilderQueueSkipReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::ObservationMissing => "observation-missing",
            Self::OutOfRange => "out-of-range",
            Self::RequestedSkip => "requested-skip",
        }
    }
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

impl BuilderQueueActivityState {
    pub fn skip_reason(&self) -> Option<BuilderQueueSkipReason> {
        if self.head_tile.is_none() {
            None
        } else if self.head_selection == BuilderQueueHeadSelection::ObservationMissing {
            Some(BuilderQueueSkipReason::ObservationMissing)
        } else if self.head_should_skip {
            Some(BuilderQueueSkipReason::RequestedSkip)
        } else if !self.head_in_range {
            Some(BuilderQueueSkipReason::OutOfRange)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderQueueLocalStepResult {
    pub validation: BuilderQueueValidationResult,
    pub activity: BuilderQueueActivityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueHeadExecutionObservation {
    OutOfRange,
    PendingBegin,
    ActiveConstruct,
    BlockedByUnit,
    InvalidPlan,
    ConstructMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueHeadExecutionAction {
    QueueEmpty,
    OutOfRange,
    BeginPlace,
    BeginBreak,
    ContinueConstruct,
    DeferredBlockedByUnit,
    RemovedInvalidHead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderQueueHeadExecutionResult {
    pub action: BuilderQueueHeadExecutionAction,
    pub head_tile_before: Option<(i32, i32)>,
    pub head_tile_after: Option<(i32, i32)>,
    pub removed_entry: Option<BuilderQueueEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueBuildSelectionSource {
    Head,
    Fallback,
}

impl BuilderQueueBuildSelectionSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderQueueBuildSelection {
    pub building: bool,
    pub selected_tile: Option<(i32, i32)>,
    pub selected_block_id: Option<i16>,
    pub selected_rotation: u8,
    pub selection_source: Option<BuilderQueueBuildSelectionSource>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueValidationRemovalReason {
    BreakAlreadyAir,
    PlaceAlreadyMatchesRotation,
    PlaceAlreadyMatchesIgnoringRotation,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueReconcileOutcome {
    #[default]
    Unchanged,
    RemovedNonHead,
    AdvancedHead,
    ClearedQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderQueueFrontPromotion {
    EnqueueFront,
    BeginInFlight,
    ExplicitMoveToFront,
    ExecutionDeferredToTail,
    ActivityReorderedToReachable,
    ActivityClosestInRangeFallback,
    ValidationAdvancedHead,
}

impl BuilderQueueFrontPromotion {
    pub fn label(self) -> &'static str {
        match self {
            Self::EnqueueFront => "enqueue-front",
            Self::BeginInFlight => "begin-in-flight",
            Self::ExplicitMoveToFront => "explicit-move-to-front",
            Self::ExecutionDeferredToTail => "execution-deferred-to-tail",
            Self::ActivityReorderedToReachable => "activity-reordered-to-reachable",
            Self::ActivityClosestInRangeFallback => "activity-closest-in-range-fallback",
            Self::ValidationAdvancedHead => "validation-advanced-head",
        }
    }
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
    pub last_skip_reason: Option<BuilderQueueSkipReason>,
    pub last_front_promotion: Option<BuilderQueueFrontPromotion>,
    pub last_validation_removal_reasons: BTreeMap<(i32, i32), BuilderQueueValidationRemovalReason>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuilderQueueProfile {
    pub head_tile: Option<(i32, i32)>,
    pub head_breaking: Option<bool>,
    pub head_block_id: Option<i16>,
    pub head_rotation: Option<u8>,
    pub head_progress_permyriad: Option<u16>,
    pub head_stage: Option<BuilderQueueStage>,
    pub queued_count: usize,
    pub inflight_count: usize,
    pub finished_count: u64,
    pub rejected_count: u64,
    pub orphan_authoritative_count: u64,
    pub last_transition: Option<BuilderQueueTransition>,
    pub last_removed_local_plan: bool,
    pub last_orphan_authoritative: bool,
    pub last_skip_reason: Option<BuilderQueueSkipReason>,
    pub last_front_promotion: Option<BuilderQueueFrontPromotion>,
}

impl BuilderQueueProfile {
    pub fn active_count(self) -> usize {
        self.queued_count + self.inflight_count
    }

    pub fn has_head(self) -> bool {
        self.head_tile.is_some()
    }

    pub fn state_label(self) -> &'static str {
        if self.active_count() == 0 {
            "idle"
        } else if self.queued_count > 0 && self.inflight_count == 0 {
            "queued"
        } else if self.queued_count == 0 && self.inflight_count > 0 {
            "in-flight"
        } else {
            "mixed"
        }
    }

    pub fn head_kind_label(self) -> &'static str {
        match self.head_breaking {
            Some(true) => "break",
            Some(false) => "place",
            None => "none",
        }
    }

    pub fn head_target_label(self) -> String {
        match self.head_breaking {
            Some(true) => "air".to_string(),
            Some(false) => {
                let block_label = self
                    .head_block_id
                    .map_or_else(|| "?".to_string(), |block_id| block_id.to_string());
                let rotation_label = self
                    .head_rotation
                    .map_or_else(|| "?".to_string(), |rotation| rotation.to_string());
                format!("{block_label}@r{rotation_label}")
            }
            None => "none".to_string(),
        }
    }

    pub fn head_progress_label(self) -> String {
        self.head_progress_permyriad
            .map_or_else(|| "none".to_string(), |progress| progress.to_string())
    }

    pub fn last_outcome_label(self) -> &'static str {
        if self.last_orphan_authoritative {
            "orphan-authoritative"
        } else if self.last_removed_local_plan {
            "removed-local-plan"
        } else {
            "none"
        }
    }

    pub fn summary_label(self) -> String {
        let head_label = self
            .head_tile
            .map_or_else(|| "none".to_string(), |(x, y)| format!("{x}:{y}"));
        let head_kind_label = self.head_kind_label();
        let head_target_label = self.head_target_label();
        let progress_label = self.head_progress_label();
        let stage_label = self.head_stage.map_or("none", BuilderQueueStage::label);
        let last_transition_label = self
            .last_transition
            .map_or("none", BuilderQueueTransition::label);
        let outcome_label = self.last_outcome_label();
        let skip_label = self
            .last_skip_reason
            .map_or("none", BuilderQueueSkipReason::label);
        let promotion_label = self
            .last_front_promotion
            .map_or("none", BuilderQueueFrontPromotion::label);

        format!(
            "state={} head={} kind={} target={} progress={} stage={} q={} i={} done={} rej={} orphan={} last={} outcome={} skip={} promo={}",
            self.state_label(),
            head_label,
            head_kind_label,
            head_target_label,
            progress_label,
            stage_label,
            self.queued_count,
            self.inflight_count,
            self.finished_count,
            self.rejected_count,
            self.orphan_authoritative_count,
            last_transition_label,
            outcome_label,
            skip_label,
            promotion_label,
        )
    }
}

impl BuilderQueueStateMachine {
    pub fn enqueue_local(
        &mut self,
        entry: BuilderQueueEntryObservation,
        tail: bool,
    ) -> Option<BuilderQueueEntry> {
        self.enqueue_local_with_progress(entry, tail, None)
    }

    pub fn enqueue_local_with_progress(
        &mut self,
        entry: BuilderQueueEntryObservation,
        tail: bool,
        progress_permyriad: Option<u16>,
    ) -> Option<BuilderQueueEntry> {
        let key = (entry.x, entry.y);
        let was_head = self.head_tile == Some(key);
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
                progress_permyriad: Self::resolve_enqueue_progress(
                    progress_permyriad,
                    previous.as_ref(),
                    entry.breaking,
                ),
                stage: BuilderQueueStage::Queued,
            },
        );
        if tail {
            self.ordered_tiles.push(key);
        } else {
            self.ordered_tiles.insert(0, key);
        }
        self.last_skip_reason = None;
        self.last_validation_removal_reasons.clear();
        self.last_front_promotion = (!tail && !was_head)
            .then_some(BuilderQueueFrontPromotion::EnqueueFront);
        self.recount();
        previous
    }

    pub fn sync_local_entries<I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = BuilderQueueEntryObservation>,
    {
        let mut next = BTreeMap::new();
        let mut incoming_counts = BTreeMap::new();
        let mut incoming_last_index = BTreeMap::new();
        let mut incoming_order = Vec::new();
        for (incoming_index, entry) in entries.into_iter().enumerate() {
            let key = (entry.x, entry.y);
            let previous = Self::matching_entry(self.active_by_tile.get(&key), entry.breaking);
            incoming_counts
                .entry(key)
                .and_modify(|count| *count += 1)
                .or_insert(1usize);
            incoming_last_index.insert(key, incoming_index);
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
                    progress_permyriad: Self::resolve_progress(previous, entry.breaking),
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
                let insert_at = if incoming_counts.get(&key).copied().unwrap_or_default() > 1 {
                    let key_last_index =
                        incoming_last_index.get(&key).copied().unwrap_or(usize::MAX);
                    next_order
                        .iter()
                        .position(|existing| {
                            incoming_last_index
                                .get(existing)
                                .copied()
                                .unwrap_or(usize::MAX)
                                > key_last_index
                        })
                        .unwrap_or(next_order.len())
                } else {
                    next_order.len()
                };
                next_order.insert(insert_at, key);
            }
        }

        self.active_by_tile = next;
        self.ordered_tiles = next_order;
        self.last_skip_reason = None;
        self.last_front_promotion = None;
        self.last_validation_removal_reasons.clear();
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
                progress_permyriad: Self::resolve_progress(previous, breaking),
                stage: BuilderQueueStage::InFlight,
            },
        );
        self.promote_to_front(key);
        self.last_transition = Some(BuilderQueueTransition::Started);
        self.last_removed_local_plan = false;
        self.last_orphan_authoritative = false;
        self.last_skip_reason = None;
        self.last_validation_removal_reasons.clear();
        self.last_front_promotion = Some(BuilderQueueFrontPromotion::BeginInFlight);
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
        self.last_skip_reason = None;
        self.last_front_promotion = None;
        self.last_validation_removal_reasons.clear();
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
        self.last_skip_reason = None;
        self.last_front_promotion = None;
        self.last_validation_removal_reasons.clear();
        self.recount();
    }

    pub fn move_to_front(&mut self, x: i32, y: i32, breaking: bool) -> bool {
        let key = (x, y);
        let was_head = self.head_tile == Some(key);
        if self
            .active_by_tile
            .get(&key)
            .is_some_and(|entry| entry.breaking == breaking)
        {
            if !was_head {
                self.promote_to_front(key);
            }
            self.last_skip_reason = None;
            self.last_validation_removal_reasons.clear();
            self.last_front_promotion = (!was_head)
                .then_some(BuilderQueueFrontPromotion::ExplicitMoveToFront);
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
            self.last_skip_reason = None;
            self.last_front_promotion = None;
            self.last_validation_removal_reasons.clear();
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

    pub fn snapshot(&self) -> BuilderQueueProfile {
        let head_entry = self.head_entry();
        BuilderQueueProfile {
            head_tile: self.head_tile,
            head_breaking: head_entry.map(|entry| entry.breaking),
            head_block_id: head_entry.and_then(|entry| entry.block_id),
            head_rotation: head_entry.and_then(|entry| entry.rotation),
            head_progress_permyriad: head_entry.and_then(|entry| entry.progress_permyriad),
            head_stage: head_entry.map(|entry| entry.stage),
            queued_count: self.queued_count,
            inflight_count: self.inflight_count,
            finished_count: self.finished_count,
            rejected_count: self.rejected_count,
            orphan_authoritative_count: self.orphan_authoritative_count,
            last_transition: self.last_transition,
            last_removed_local_plan: self.last_removed_local_plan,
            last_orphan_authoritative: self.last_orphan_authoritative,
            last_skip_reason: self.last_skip_reason,
            last_front_promotion: self.last_front_promotion,
        }
    }

    pub fn profile(&self) -> BuilderQueueProfile {
        self.snapshot()
    }

    pub fn head_entry(&self) -> Option<&BuilderQueueEntry> {
        self.head_tile
            .and_then(|tile| self.active_by_tile.get(&tile))
    }

    pub fn active_non_breaking_entry(&self) -> Option<&BuilderQueueEntry> {
        self.ordered_tiles.iter().find_map(|tile| {
            self.active_by_tile
                .get(tile)
                .filter(|entry| !entry.breaking && entry.block_id.is_some())
        })
    }

    pub fn build_selection(&self) -> BuilderQueueBuildSelection {
        let head_tile = self.head_tile;
        let selection_entry = self
            .head_entry()
            .filter(|entry| !entry.breaking && entry.block_id.is_some())
            .or_else(|| self.active_non_breaking_entry());
        let selection_source = selection_entry.map(|entry| {
            if Some((entry.x, entry.y)) == head_tile {
                BuilderQueueBuildSelectionSource::Head
            } else {
                BuilderQueueBuildSelectionSource::Fallback
            }
        });

        BuilderQueueBuildSelection {
            building: head_tile.is_some(),
            selected_tile: selection_entry.map(|entry| (entry.x, entry.y)),
            selected_block_id: selection_entry.and_then(|entry| entry.block_id),
            selected_rotation: selection_entry
                .and_then(|entry| entry.rotation)
                .unwrap_or(0),
            selection_source,
        }
    }

    pub fn observe_progress(
        &mut self,
        x: i32,
        y: i32,
        breaking: bool,
        progress_permyriad: u16,
    ) -> bool {
        let key = (x, y);
        let Some(entry) = self.active_by_tile.get_mut(&key) else {
            return false;
        };
        if entry.breaking != breaking {
            return false;
        }
        entry.progress_permyriad = Some(progress_permyriad.min(10_000));
        true
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
        let original_head = self.head_tile;
        let mut reordered = false;
        let mut used_closest_in_range_fallback = false;
        let mut missing_head_observation = false;
        let mut encountered_incomplete_observation = false;

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
            let original_head_should_skip = self
                .head_entry()
                .and_then(|entry| Self::activity_observation(&observations_by_key, entry))
                .is_some_and(|observation| observation.should_skip);
            let can_reorder_to_first_unskipped_in_range =
                !original_head_in_range || original_head_should_skip;

            while total < size {
                let Some(tile) = self.ordered_tiles.first().copied() else {
                    break;
                };
                let Some(entry) = self.active_by_tile.get(&tile) else {
                    break;
                };
                let Some(observation) = Self::activity_observation(&observations_by_key, entry)
                else {
                    self.ordered_tiles = original_order.clone();
                    encountered_incomplete_observation = true;
                    if Some(tile) == original_head {
                        missing_head_observation = true;
                    }
                    break;
                };

                if observation.in_range && !observation.should_skip {
                    found_valid_head = true;
                    reordered = total > 0 && can_reorder_to_first_unskipped_in_range;
                    if !reordered {
                        self.ordered_tiles = original_order.clone();
                    }
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
                if !encountered_incomplete_observation {
                    if let Some(best_index) = best_index.filter(|index| *index > 0) {
                        if !original_head_in_range {
                            self.ordered_tiles.rotate_left(best_index);
                            reordered = true;
                            used_closest_in_range_fallback = true;
                        }
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
        } else if missing_head_observation || head_observation.is_none() {
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
        let activity = BuilderQueueActivityState {
            head_tile: self.head_tile,
            actively_building: head_in_range,
            head_in_range,
            head_should_skip,
            reordered,
            used_closest_in_range_fallback,
            head_selection,
        };
        self.last_skip_reason = activity.skip_reason();
        self.last_validation_removal_reasons.clear();
        self.last_front_promotion = if used_closest_in_range_fallback {
            Some(BuilderQueueFrontPromotion::ActivityClosestInRangeFallback)
        } else if reordered {
            Some(BuilderQueueFrontPromotion::ActivityReorderedToReachable)
        } else {
            None
        };
        activity
    }

    /// Mirrors the low-risk Java `BuilderComp.updateBuildLogic()` ordering:
    /// prune completed plans first, then evaluate the new head against local activity.
    pub fn apply_local_builder_step<T, U>(
        &mut self,
        tile_state_observations: T,
        activity_observations: U,
    ) -> BuilderQueueLocalStepResult
    where
        T: IntoIterator<Item = BuilderQueueTileStateObservation>,
        U: IntoIterator<Item = BuilderQueueActivityObservation>,
    {
        let validation = self.validate_against_tile_states(tile_state_observations);
        let validation_removal_reasons = self.last_validation_removal_reasons.clone();
        let validation_front_promotion = (self.last_front_promotion
            == Some(BuilderQueueFrontPromotion::ValidationAdvancedHead))
        .then_some(BuilderQueueFrontPromotion::ValidationAdvancedHead);
        let activity = self.update_local_activity(activity_observations);
        if !validation_removal_reasons.is_empty() {
            self.last_validation_removal_reasons = validation_removal_reasons;
        }
        if self.last_front_promotion.is_none() {
            self.last_front_promotion = validation_front_promotion;
        }
        BuilderQueueLocalStepResult {
            validation,
            activity,
        }
    }

    /// Mirrors the low-risk `BuilderComp.updateBuildLogic()` head-resolution cases after
    /// validation/reorder:
    /// - keep an out-of-range head in place,
    /// - emit a `beginPlace` / `beginBreak` intent when the tile is ready,
    /// - defer a temporarily blocked place plan to queue tail,
    /// - drop a head that is no longer buildable or whose construct state diverged.
    pub fn apply_head_execution_observation(
        &mut self,
        observation: BuilderQueueHeadExecutionObservation,
    ) -> BuilderQueueHeadExecutionResult {
        let head_tile_before = self.head_tile;
        let head_entry = self.head_entry().cloned();
        let mut removed_entry = None;

        let action = match (head_entry, observation) {
            (None, _) => BuilderQueueHeadExecutionAction::QueueEmpty,
            (Some(_), BuilderQueueHeadExecutionObservation::OutOfRange) => {
                BuilderQueueHeadExecutionAction::OutOfRange
            }
            (Some(entry), BuilderQueueHeadExecutionObservation::PendingBegin) => {
                if entry.stage == BuilderQueueStage::InFlight {
                    BuilderQueueHeadExecutionAction::ContinueConstruct
                } else if entry.breaking {
                    BuilderQueueHeadExecutionAction::BeginBreak
                } else if entry.block_id.is_some() {
                    BuilderQueueHeadExecutionAction::BeginPlace
                } else {
                    removed_entry = self.remove_matching_entry(entry.x, entry.y, entry.breaking);
                    self.recount();
                    BuilderQueueHeadExecutionAction::RemovedInvalidHead
                }
            }
            (Some(entry), BuilderQueueHeadExecutionObservation::ActiveConstruct) => {
                if let Some(active_entry) = self.active_by_tile.get_mut(&(entry.x, entry.y)) {
                    active_entry.stage = BuilderQueueStage::InFlight;
                }
                self.recount();
                BuilderQueueHeadExecutionAction::ContinueConstruct
            }
            (Some(_), BuilderQueueHeadExecutionObservation::BlockedByUnit) => {
                self.defer_head_to_tail();
                self.recount();
                BuilderQueueHeadExecutionAction::DeferredBlockedByUnit
            }
            (Some(entry), BuilderQueueHeadExecutionObservation::InvalidPlan)
            | (Some(entry), BuilderQueueHeadExecutionObservation::ConstructMismatch) => {
                removed_entry = self.remove_matching_entry(entry.x, entry.y, entry.breaking);
                self.recount();
                BuilderQueueHeadExecutionAction::RemovedInvalidHead
            }
        };

        if matches!(
            action,
            BuilderQueueHeadExecutionAction::BeginPlace
                | BuilderQueueHeadExecutionAction::BeginBreak
                | BuilderQueueHeadExecutionAction::ContinueConstruct
        ) {
            self.last_transition = Some(BuilderQueueTransition::Started);
        }
        self.last_skip_reason = None;
        self.last_validation_removal_reasons.clear();
        self.last_front_promotion = (action
            == BuilderQueueHeadExecutionAction::DeferredBlockedByUnit)
            .then_some(BuilderQueueFrontPromotion::ExecutionDeferredToTail);

        BuilderQueueHeadExecutionResult {
            action,
            head_tile_before,
            head_tile_after: self.head_tile,
            removed_entry,
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
        let mut removed_reasons = BTreeMap::new();

        self.ordered_tiles.retain(|tile| {
            let Some(entry) = self.active_by_tile.get(tile) else {
                return false;
            };
            let Some(observation) = observations_by_tile.get(tile) else {
                return true;
            };
            let removal_reason = Self::entry_removal_reason(entry, observation);
            if let Some(reason) = removal_reason {
                removed_tiles.push(*tile);
                removed_reasons.insert(*tile, reason);
            }
            removal_reason.is_none()
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
        self.last_skip_reason = None;
        self.last_validation_removal_reasons = removed_reasons;
        self.last_front_promotion = (reconcile_outcome
            == BuilderQueueReconcileOutcome::AdvancedHead)
            .then_some(BuilderQueueFrontPromotion::ValidationAdvancedHead);
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

    fn defer_head_to_tail(&mut self) {
        if let Some(head) = self.head_tile {
            self.ordered_tiles.retain(|tile| *tile != head);
            self.ordered_tiles.push(head);
        }
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

    fn resolve_progress(previous: Option<&BuilderQueueEntry>, breaking: bool) -> Option<u16> {
        Self::matching_entry(previous, breaking).and_then(|entry| entry.progress_permyriad)
    }

    fn resolve_enqueue_progress(
        progress_permyriad: Option<u16>,
        previous: Option<&BuilderQueueEntry>,
        breaking: bool,
    ) -> Option<u16> {
        progress_permyriad
            .map(|progress| progress.min(10_000))
            .or_else(|| Self::resolve_progress(previous, breaking))
    }

    fn activity_observation<'a>(
        observations_by_key: &'a BTreeMap<(i32, i32, bool), BuilderQueueActivityObservation>,
        entry: &BuilderQueueEntry,
    ) -> Option<&'a BuilderQueueActivityObservation> {
        observations_by_key.get(&(entry.x, entry.y, entry.breaking))
    }

    fn entry_removal_reason(
        entry: &BuilderQueueEntry,
        observation: &BuilderQueueTileStateObservation,
    ) -> Option<BuilderQueueValidationRemovalReason> {
        if entry.breaking {
            observation
                .block_id
                .is_none()
                .then_some(BuilderQueueValidationRemovalReason::BreakAlreadyAir)
        } else {
            let same_block = observation.block_id == entry.block_id;
            if !same_block {
                return None;
            }
            if !observation.requires_rotation_match {
                return Some(
                    BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesIgnoringRotation,
                );
            }
            observation
                .rotation
                .zip(entry.rotation)
                .is_some_and(|(lhs, rhs)| lhs == rhs)
                .then_some(BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesRotation)
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
        BuilderQueueActivityObservation, BuilderQueueActivityState, BuilderQueueBuildSelection,
        BuilderQueueBuildSelectionSource, BuilderQueueEntry, BuilderQueueEntryObservation,
        BuilderQueueFrontPromotion, BuilderQueueHeadExecutionAction,
        BuilderQueueHeadExecutionObservation, BuilderQueueHeadExecutionResult,
        BuilderQueueHeadSelection, BuilderQueueLocalStepResult, BuilderQueueProfile,
        BuilderQueueReconcileOutcome, BuilderQueueSkipReason, BuilderQueueStage,
        BuilderQueueStateMachine, BuilderQueueTileStateObservation, BuilderQueueTransition,
        BuilderQueueValidationRemovalReason, BuilderQueueValidationResult,
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
                        progress_permyriad: None,
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
                        progress_permyriad: None,
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
        assert!(queue.observe_progress(6, 6, false, 3_300));

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
                progress_permyriad: None,
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
        assert!(queue.observe_progress(7, 7, false, 6_700));

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
                progress_permyriad: Some(6_700),
                stage: BuilderQueueStage::InFlight,
            })
        );
        assert_eq!(queue.queued_count, 0);
        assert_eq!(queue.inflight_count, 1);
    }

    #[test]
    fn builder_queue_snapshot_tracks_head_counts_and_high_signal_summary() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(7),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
        ]);

        let profile = queue.snapshot();
        assert_eq!(
            profile,
            BuilderQueueProfile {
                head_tile: Some((2, 2)),
                head_breaking: Some(false),
                head_block_id: Some(7),
                head_rotation: Some(1),
                head_progress_permyriad: None,
                head_stage: Some(BuilderQueueStage::Queued),
                queued_count: 2,
                inflight_count: 0,
                finished_count: 0,
                rejected_count: 0,
                orphan_authoritative_count: 0,
                last_transition: None,
                last_removed_local_plan: false,
                last_orphan_authoritative: false,
                last_skip_reason: None,
                last_front_promotion: None,
            }
        );
        assert_eq!(profile.active_count(), 2);
        assert!(profile.has_head());
        assert_eq!(profile.state_label(), "queued");
        assert_eq!(profile.head_kind_label(), "place");
        assert_eq!(profile.head_target_label(), "7@r1");
        assert_eq!(profile.head_progress_label(), "none");
        assert_eq!(profile.last_outcome_label(), "none");
        assert_eq!(
            profile.summary_label(),
            "state=queued head=2:2 kind=place target=7@r1 progress=none stage=queued q=2 i=0 done=0 rej=0 orphan=0 last=none outcome=none skip=none promo=none"
        );

        let activity = queue.update_local_activity([BuilderQueueActivityObservation {
            x: 4,
            y: 4,
            breaking: false,
            in_range: true,
            should_skip: false,
            distance_sq: 0,
        }]);
        assert_eq!(
            activity.head_selection,
            BuilderQueueHeadSelection::ObservationMissing
        );
        let profile = queue.profile();
        assert_eq!(
            profile.last_skip_reason,
            Some(BuilderQueueSkipReason::ObservationMissing)
        );
        assert_eq!(profile.last_front_promotion, None);
        assert_eq!(profile.head_tile, Some((2, 2)));
        assert_eq!(
            profile.summary_label(),
            "state=queued head=2:2 kind=place target=7@r1 progress=none stage=queued q=2 i=0 done=0 rej=0 orphan=0 last=none outcome=none skip=observation-missing promo=none"
        );

        assert!(queue.move_to_front(4, 4, true));
        let profile = queue.profile();
        assert_eq!(profile.head_tile, Some((4, 4)));
        assert_eq!(profile.head_breaking, Some(true));
        assert_eq!(profile.head_kind_label(), "break");
        assert_eq!(profile.head_target_label(), "air");
        assert_eq!(
            profile.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ExplicitMoveToFront)
        );
        assert_eq!(profile.last_skip_reason, None);
        assert_eq!(
            profile.summary_label(),
            "state=queued head=4:4 kind=break target=air progress=none stage=queued q=2 i=0 done=0 rej=0 orphan=0 last=none outcome=none skip=none promo=explicit-move-to-front"
        );

        queue.mark_begin(4, 4, true, None, 0);
        assert!(queue.observe_progress(4, 4, true, 4_200));
        let profile = queue.profile();
        assert_eq!(profile.head_tile, Some((4, 4)));
        assert_eq!(profile.head_stage, Some(BuilderQueueStage::InFlight));
        assert_eq!(profile.head_progress_permyriad, Some(4_200));
        assert_eq!(profile.queued_count, 1);
        assert_eq!(profile.inflight_count, 1);
        assert_eq!(
            profile.last_transition,
            Some(BuilderQueueTransition::Started)
        );
        assert_eq!(
            profile.last_front_promotion,
            Some(BuilderQueueFrontPromotion::BeginInFlight)
        );
        assert_eq!(profile.state_label(), "mixed");
        assert_eq!(
            profile.summary_label(),
            "state=mixed head=4:4 kind=break target=air progress=4200 stage=in-flight q=1 i=1 done=0 rej=0 orphan=0 last=started outcome=none skip=none promo=begin-in-flight"
        );
    }

    #[test]
    fn builder_queue_summary_tracks_transition_outcomes_and_cumulative_counts() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 1,
            y: 1,
            breaking: false,
            block_id: Some(11),
            rotation: 2,
        }]);

        queue.mark_reject(99, 99, false, false);

        let profile = queue.profile();
        assert_eq!(profile.rejected_count, 1);
        assert_eq!(profile.orphan_authoritative_count, 1);
        assert!(!profile.last_removed_local_plan);
        assert!(profile.last_orphan_authoritative);
        assert_eq!(profile.last_outcome_label(), "orphan-authoritative");
        assert_eq!(
            profile.summary_label(),
            "state=queued head=1:1 kind=place target=11@r2 progress=none stage=queued q=1 i=0 done=0 rej=1 orphan=1 last=rejected outcome=orphan-authoritative skip=none promo=none"
        );

        queue.mark_finish(1, 1, false, true);

        let profile = queue.profile();
        assert_eq!(profile.finished_count, 1);
        assert_eq!(profile.rejected_count, 1);
        assert_eq!(profile.orphan_authoritative_count, 1);
        assert!(profile.last_removed_local_plan);
        assert!(!profile.last_orphan_authoritative);
        assert_eq!(profile.last_outcome_label(), "removed-local-plan");
        assert_eq!(
            profile.summary_label(),
            "state=idle head=none kind=none target=none progress=none stage=none q=0 i=0 done=1 rej=1 orphan=1 last=finished outcome=removed-local-plan skip=none promo=none"
        );
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
        assert!(queue.observe_progress(9, 9, false, 4_200));

        queue.mark_begin(9, 9, true, None, 3);

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 9,
                y: 9,
                breaking: true,
                block_id: None,
                rotation: Some(3),
                progress_permyriad: None,
                stage: BuilderQueueStage::InFlight,
            })
        );
        assert_eq!(queue.queued_count, 0);
        assert_eq!(queue.inflight_count, 1);
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
    }

    #[test]
    fn mark_begin_preserves_known_progress_for_same_break_mode() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 14,
            y: 14,
            breaking: false,
            block_id: Some(140),
            rotation: 1,
        }]);
        assert!(queue.observe_progress(14, 14, false, 5_400));

        queue.mark_begin(14, 14, false, Some(141), 3);

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 14,
                y: 14,
                breaking: false,
                block_id: Some(141),
                rotation: Some(3),
                progress_permyriad: Some(5_400),
                stage: BuilderQueueStage::InFlight,
            })
        );
        assert_eq!(queue.queued_count, 0);
        assert_eq!(queue.inflight_count, 1);
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
    fn sync_local_entries_keeps_duplicate_tile_ahead_of_preserved_unique_tiles() {
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

        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: false,
                block_id: Some(10),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: true,
                block_id: None,
                rotation: 3,
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

        assert_eq!(queue.ordered_tiles, vec![(1, 1), (2, 2), (3, 3)]);
        assert_eq!(queue.head_tile, Some((1, 1)));
        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 1,
                y: 1,
                breaking: true,
                block_id: None,
                rotation: Some(3),
                progress_permyriad: None,
                stage: BuilderQueueStage::Queued,
            })
        );
    }

    #[test]
    fn sync_local_entries_reinserts_duplicate_tile_by_last_occurrence_between_unique_tiles() {
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
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(40),
                rotation: 3,
            },
        ]);

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
                rotation: 2,
            },
            BuilderQueueEntryObservation {
                x: 4,
                y: 4,
                breaking: false,
                block_id: Some(40),
                rotation: 3,
            },
        ]);

        assert_eq!(queue.ordered_tiles, vec![(1, 1), (2, 2), (3, 3), (4, 4)]);
        assert_eq!(queue.head_tile, Some((1, 1)));
        assert_eq!(
            queue.active_by_tile.get(&(2, 2)),
            Some(&BuilderQueueEntry {
                x: 2,
                y: 2,
                breaking: true,
                block_id: None,
                rotation: Some(0),
                progress_permyriad: None,
                stage: BuilderQueueStage::Queued,
            })
        );
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
                progress_permyriad: None,
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
                progress_permyriad: None,
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
                progress_permyriad: None,
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
    fn enqueue_local_preserves_known_progress_across_same_tile_replacement() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 15,
                y: 15,
                breaking: false,
                block_id: Some(150),
                rotation: 0,
            },
            true,
        );
        assert!(queue.observe_progress(15, 15, false, 4_500));

        let replaced = queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 15,
                y: 15,
                breaking: false,
                block_id: Some(151),
                rotation: 2,
            },
            false,
        );

        assert_eq!(
            replaced,
            Some(BuilderQueueEntry {
                x: 15,
                y: 15,
                breaking: false,
                block_id: Some(150),
                rotation: Some(0),
                progress_permyriad: Some(4_500),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 15,
                y: 15,
                breaking: false,
                block_id: Some(151),
                rotation: Some(2),
                progress_permyriad: Some(4_500),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(15, 15)]);
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn enqueue_local_with_progress_seeds_fresh_entry_and_clamps_progress() {
        let mut queue = BuilderQueueStateMachine::default();

        queue.enqueue_local_with_progress(
            BuilderQueueEntryObservation {
                x: 17,
                y: 17,
                breaking: false,
                block_id: Some(170),
                rotation: 1,
            },
            true,
            Some(12_345),
        );

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 17,
                y: 17,
                breaking: false,
                block_id: Some(170),
                rotation: Some(1),
                progress_permyriad: Some(10_000),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(17, 17)]);
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn enqueue_local_with_progress_overrides_stale_same_mode_progress() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 18,
                y: 18,
                breaking: false,
                block_id: Some(180),
                rotation: 0,
            },
            true,
        );
        assert!(queue.observe_progress(18, 18, false, 4_500));

        queue.enqueue_local_with_progress(
            BuilderQueueEntryObservation {
                x: 18,
                y: 18,
                breaking: false,
                block_id: Some(181),
                rotation: 2,
            },
            false,
            Some(6_200),
        );

        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 18,
                y: 18,
                breaking: false,
                block_id: Some(181),
                rotation: Some(2),
                progress_permyriad: Some(6_200),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(18, 18)]);
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn enqueue_local_with_progress_seeds_mode_switch_from_live_construct_progress() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 19,
                y: 19,
                breaking: false,
                block_id: Some(190),
                rotation: 3,
            },
            true,
        );
        assert!(queue.observe_progress(19, 19, false, 3_300));

        let replaced = queue.enqueue_local_with_progress(
            BuilderQueueEntryObservation {
                x: 19,
                y: 19,
                breaking: true,
                block_id: None,
                rotation: 1,
            },
            false,
            Some(7_700),
        );

        assert_eq!(
            replaced,
            Some(BuilderQueueEntry {
                x: 19,
                y: 19,
                breaking: false,
                block_id: Some(190),
                rotation: Some(3),
                progress_permyriad: Some(3_300),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(
            queue.head_entry(),
            Some(&BuilderQueueEntry {
                x: 19,
                y: 19,
                breaking: true,
                block_id: None,
                rotation: Some(1),
                progress_permyriad: Some(7_700),
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(19, 19)]);
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
    }

    #[test]
    fn enqueue_local_with_progress_replacing_head_does_not_report_front_promotion() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 20,
                y: 20,
                breaking: false,
                block_id: Some(200),
                rotation: 1,
            },
            true,
        );

        let replaced = queue.enqueue_local_with_progress(
            BuilderQueueEntryObservation {
                x: 20,
                y: 20,
                breaking: false,
                block_id: Some(201),
                rotation: 2,
            },
            false,
            Some(8_800),
        );

        assert_eq!(
            replaced,
            Some(BuilderQueueEntry {
                x: 20,
                y: 20,
                breaking: false,
                block_id: Some(200),
                rotation: Some(1),
                progress_permyriad: None,
                stage: BuilderQueueStage::Queued,
            })
        );
        assert_eq!(queue.ordered_tiles, vec![(20, 20)]);
        assert_eq!(queue.head_tile, Some((20, 20)));
        assert_eq!(queue.head_entry().map(|entry| entry.progress_permyriad), Some(Some(8_800)));
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn move_to_front_on_current_head_is_a_noop_without_promotion() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 21,
                y: 21,
                breaking: false,
                block_id: Some(210),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 22,
                y: 22,
                breaking: false,
                block_id: Some(220),
                rotation: 1,
            },
        ]);

        assert!(queue.move_to_front(21, 21, false));
        assert_eq!(queue.ordered_tiles, vec![(21, 21), (22, 22)]);
        assert_eq!(queue.head_tile, Some((21, 21)));
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn observe_progress_requires_exact_breaking_match_and_clamps_value() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 16,
            y: 16,
            breaking: false,
            block_id: Some(160),
            rotation: 1,
        }]);

        assert!(!queue.observe_progress(16, 16, true, 500));
        assert!(queue.observe_progress(16, 16, false, 12_345));
        assert_eq!(
            queue
                .head_entry()
                .and_then(|entry| entry.progress_permyriad),
            Some(10_000)
        );
        assert_eq!(queue.ordered_tiles, vec![(16, 16)]);
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
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
                progress_permyriad: None,
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
    fn update_local_activity_reorders_past_in_range_skipped_head_to_first_unskipped_plan() {
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
                should_skip: false,
                distance_sq: 4,
            },
            BuilderQueueActivityObservation {
                x: 3,
                y: 3,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 1,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((2, 2)),
                actively_building: true,
                head_in_range: true,
                head_should_skip: false,
                reordered: true,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::ReorderedToInRange,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(2, 2), (3, 3), (1, 1)]);
        assert_eq!(queue.head_tile, Some((2, 2)));
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
    fn update_local_activity_keeps_head_out_of_range_when_non_head_observation_is_missing() {
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
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: false,
                block_id: Some(60),
                rotation: 2,
            },
        ]);
        let expected_order = queue.ordered_tiles.clone();

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 4,
                y: 4,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 64,
            },
            BuilderQueueActivityObservation {
                x: 6,
                y: 6,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 4,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((4, 4)),
                actively_building: false,
                head_in_range: false,
                head_should_skip: false,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::HeadOutOfRange,
            }
        );
        assert_eq!(queue.ordered_tiles, expected_order);
        assert_eq!(queue.head_tile, Some((4, 4)));
    }

    #[test]
    fn update_local_activity_keeps_skipped_head_when_non_head_observation_is_missing() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 7,
                y: 7,
                breaking: false,
                block_id: Some(70),
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 8,
                y: 8,
                breaking: false,
                block_id: Some(80),
                rotation: 1,
            },
            BuilderQueueEntryObservation {
                x: 9,
                y: 9,
                breaking: false,
                block_id: Some(90),
                rotation: 2,
            },
        ]);
        let expected_order = queue.ordered_tiles.clone();

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 7,
                y: 7,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 25,
            },
            BuilderQueueActivityObservation {
                x: 9,
                y: 9,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 1,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((7, 7)),
                actively_building: true,
                head_in_range: true,
                head_should_skip: true,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::SkippedInRange,
            }
        );
        assert_eq!(queue.ordered_tiles, expected_order);
        assert_eq!(queue.head_tile, Some((7, 7)));
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
    fn update_local_activity_does_not_fallback_when_scan_is_incomplete() {
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
        let expected_order = queue.ordered_tiles.clone();

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 1,
                y: 1,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 49,
            },
            BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 4,
            },
        ]);

        assert_eq!(
            activity,
            BuilderQueueActivityState {
                head_tile: Some((1, 1)),
                actively_building: false,
                head_in_range: false,
                head_should_skip: false,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::HeadOutOfRange,
            }
        );
        assert_eq!(queue.ordered_tiles, expected_order);
        assert_eq!(queue.head_tile, Some((1, 1)));
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
    fn validate_against_tile_states_advances_same_tile_replacement_head_without_touching_counts() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: false,
                block_id: Some(50),
                rotation: 0,
            },
            true,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: false,
                block_id: Some(60),
                rotation: 1,
            },
            true,
        );
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 5,
                y: 5,
                breaking: true,
                block_id: None,
                rotation: 2,
            },
            false,
        );
        queue.rejected_count = 3;
        queue.finished_count = 5;
        queue.orphan_authoritative_count = 7;

        let validation = queue.validate_against_tile_states([BuilderQueueTileStateObservation {
            x: 5,
            y: 5,
            block_id: None,
            rotation: None,
            requires_rotation_match: false,
        }]);

        assert_eq!(
            validation,
            BuilderQueueValidationResult {
                removed_count: 1,
                removed_head: true,
                removed_tiles: vec![(5, 5)],
                head_tile_before: Some((5, 5)),
                head_tile_after: Some((6, 6)),
                reconcile_outcome: BuilderQueueReconcileOutcome::AdvancedHead,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(6, 6)]);
        assert_eq!(queue.head_tile, Some((6, 6)));
        assert_eq!(queue.rejected_count, 3);
        assert_eq!(queue.finished_count, 5);
        assert_eq!(queue.orphan_authoritative_count, 7);
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

        let validation = queue.validate_against_tile_states([BuilderQueueTileStateObservation {
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

        let validation = queue.validate_against_tile_states([BuilderQueueTileStateObservation {
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

    #[test]
    fn apply_local_builder_step_validates_head_before_reordering_activity() {
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

        let step = queue.apply_local_builder_step(
            [BuilderQueueTileStateObservation {
                x: 1,
                y: 1,
                block_id: Some(10),
                rotation: Some(0),
                requires_rotation_match: true,
            }],
            [
                BuilderQueueActivityObservation {
                    x: 2,
                    y: 2,
                    breaking: false,
                    in_range: false,
                    should_skip: false,
                    distance_sq: 25,
                },
                BuilderQueueActivityObservation {
                    x: 3,
                    y: 3,
                    breaking: false,
                    in_range: true,
                    should_skip: false,
                    distance_sq: 4,
                },
            ],
        );

        assert_eq!(
            step,
            BuilderQueueLocalStepResult {
                validation: BuilderQueueValidationResult {
                    removed_count: 1,
                    removed_head: true,
                    removed_tiles: vec![(1, 1)],
                    head_tile_before: Some((1, 1)),
                    head_tile_after: Some((2, 2)),
                    reconcile_outcome: BuilderQueueReconcileOutcome::AdvancedHead,
                },
                activity: BuilderQueueActivityState {
                    head_tile: Some((3, 3)),
                    actively_building: true,
                    head_in_range: true,
                    head_should_skip: false,
                    reordered: true,
                    used_closest_in_range_fallback: false,
                    head_selection: BuilderQueueHeadSelection::ReorderedToInRange,
                },
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(3, 3), (2, 2)]);
        assert_eq!(queue.head_tile, Some((3, 3)));
        assert_eq!(
            queue.last_validation_removal_reasons.get(&(1, 1)),
            Some(&BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesRotation)
        );
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ActivityReorderedToReachable)
        );
    }

    #[test]
    fn apply_local_builder_step_reports_queue_empty_after_last_head_is_removed() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 9,
            y: 9,
            breaking: false,
            block_id: Some(90),
            rotation: 2,
        }]);

        let step = queue.apply_local_builder_step(
            [BuilderQueueTileStateObservation {
                x: 9,
                y: 9,
                block_id: Some(90),
                rotation: Some(2),
                requires_rotation_match: true,
            }],
            [BuilderQueueActivityObservation {
                x: 9,
                y: 9,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 0,
            }],
        );

        assert_eq!(
            step,
            BuilderQueueLocalStepResult {
                validation: BuilderQueueValidationResult {
                    removed_count: 1,
                    removed_head: true,
                    removed_tiles: vec![(9, 9)],
                    head_tile_before: Some((9, 9)),
                    head_tile_after: None,
                    reconcile_outcome: BuilderQueueReconcileOutcome::ClearedQueue,
                },
                activity: BuilderQueueActivityState {
                    head_tile: None,
                    actively_building: false,
                    head_in_range: false,
                    head_should_skip: false,
                    reordered: false,
                    used_closest_in_range_fallback: false,
                    head_selection: BuilderQueueHeadSelection::QueueEmpty,
                },
            }
        );
        assert!(queue.ordered_tiles.is_empty());
        assert_eq!(queue.head_tile, None);
        assert_eq!(
            queue.last_validation_removal_reasons.get(&(9, 9)),
            Some(&BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesRotation)
        );
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn apply_local_builder_step_preserves_authoritative_counts_across_multi_tick_head_advances() {
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
        queue.rejected_count = 11;
        queue.finished_count = 13;
        queue.orphan_authoritative_count = 17;

        let first_tick = queue.apply_local_builder_step(
            [BuilderQueueTileStateObservation {
                x: 1,
                y: 1,
                block_id: Some(10),
                rotation: Some(0),
                requires_rotation_match: true,
            }],
            [
                BuilderQueueActivityObservation {
                    x: 2,
                    y: 2,
                    breaking: false,
                    in_range: false,
                    should_skip: false,
                    distance_sq: 16,
                },
                BuilderQueueActivityObservation {
                    x: 3,
                    y: 3,
                    breaking: false,
                    in_range: true,
                    should_skip: false,
                    distance_sq: 1,
                },
            ],
        );

        assert_eq!(
            first_tick,
            BuilderQueueLocalStepResult {
                validation: BuilderQueueValidationResult {
                    removed_count: 1,
                    removed_head: true,
                    removed_tiles: vec![(1, 1)],
                    head_tile_before: Some((1, 1)),
                    head_tile_after: Some((2, 2)),
                    reconcile_outcome: BuilderQueueReconcileOutcome::AdvancedHead,
                },
                activity: BuilderQueueActivityState {
                    head_tile: Some((3, 3)),
                    actively_building: true,
                    head_in_range: true,
                    head_should_skip: false,
                    reordered: true,
                    used_closest_in_range_fallback: false,
                    head_selection: BuilderQueueHeadSelection::ReorderedToInRange,
                },
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(3, 3), (2, 2)]);
        assert_eq!(queue.head_tile, Some((3, 3)));
        assert_eq!(queue.rejected_count, 11);
        assert_eq!(queue.finished_count, 13);
        assert_eq!(queue.orphan_authoritative_count, 17);

        let second_tick = queue.apply_local_builder_step(
            [BuilderQueueTileStateObservation {
                x: 3,
                y: 3,
                block_id: Some(30),
                rotation: Some(2),
                requires_rotation_match: true,
            }],
            [BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 1,
            }],
        );

        assert_eq!(
            second_tick,
            BuilderQueueLocalStepResult {
                validation: BuilderQueueValidationResult {
                    removed_count: 1,
                    removed_head: true,
                    removed_tiles: vec![(3, 3)],
                    head_tile_before: Some((3, 3)),
                    head_tile_after: Some((2, 2)),
                    reconcile_outcome: BuilderQueueReconcileOutcome::AdvancedHead,
                },
                activity: BuilderQueueActivityState {
                    head_tile: Some((2, 2)),
                    actively_building: true,
                    head_in_range: true,
                    head_should_skip: false,
                    reordered: false,
                    used_closest_in_range_fallback: false,
                    head_selection: BuilderQueueHeadSelection::HeadInRange,
                },
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(2, 2)]);
        assert_eq!(queue.head_tile, Some((2, 2)));
        assert_eq!(queue.rejected_count, 11);
        assert_eq!(queue.finished_count, 13);
        assert_eq!(queue.orphan_authoritative_count, 17);
    }

    #[test]
    fn apply_local_builder_step_keeps_validation_promotion_when_new_head_is_skipped() {
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
        ]);

        let step = queue.apply_local_builder_step(
            [BuilderQueueTileStateObservation {
                x: 1,
                y: 1,
                block_id: Some(10),
                rotation: Some(0),
                requires_rotation_match: true,
            }],
            [BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: true,
                should_skip: true,
                distance_sq: 4,
            }],
        );

        assert_eq!(
            step.activity,
            BuilderQueueActivityState {
                head_tile: Some((2, 2)),
                actively_building: true,
                head_in_range: true,
                head_should_skip: true,
                reordered: false,
                used_closest_in_range_fallback: false,
                head_selection: BuilderQueueHeadSelection::SkippedInRange,
            }
        );
        assert_eq!(
            queue.last_skip_reason,
            Some(BuilderQueueSkipReason::RequestedSkip)
        );
        assert_eq!(
            queue.last_validation_removal_reasons.get(&(1, 1)),
            Some(&BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesRotation)
        );
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ValidationAdvancedHead)
        );
    }

    #[test]
    fn apply_local_builder_step_matches_separate_validation_and_activity_calls() {
        let mut sequential_queue = BuilderQueueStateMachine::default();
        sequential_queue.sync_local_entries([
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

        let mut combined_queue = sequential_queue.clone();
        let tile_state_observations = [
            BuilderQueueTileStateObservation {
                x: 1,
                y: 1,
                block_id: Some(10),
                rotation: Some(0),
                requires_rotation_match: true,
            },
            BuilderQueueTileStateObservation {
                x: 3,
                y: 3,
                block_id: Some(31),
                rotation: Some(2),
                requires_rotation_match: true,
            },
        ];
        let activity_observations = [
            BuilderQueueActivityObservation {
                x: 2,
                y: 2,
                breaking: false,
                in_range: false,
                should_skip: false,
                distance_sq: 16,
            },
            BuilderQueueActivityObservation {
                x: 3,
                y: 3,
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 1,
            },
        ];

        let validation = sequential_queue.validate_against_tile_states(tile_state_observations);
        let activity = sequential_queue.update_local_activity(activity_observations);
        let combined =
            combined_queue.apply_local_builder_step(tile_state_observations, activity_observations);

        assert_eq!(
            combined,
            BuilderQueueLocalStepResult {
                validation,
                activity,
            }
        );
        assert_eq!(
            combined_queue.active_by_tile,
            sequential_queue.active_by_tile
        );
        assert_eq!(combined_queue.ordered_tiles, vec![(3, 3), (2, 2)]);
        assert_eq!(combined_queue.head_tile, Some((3, 3)));
        assert_eq!(combined_queue.queued_count, sequential_queue.queued_count);
        assert_eq!(
            combined_queue.inflight_count,
            sequential_queue.inflight_count
        );
        assert_eq!(
            combined_queue.last_skip_reason,
            sequential_queue.last_skip_reason
        );
        assert_eq!(
            combined_queue.last_front_promotion,
            sequential_queue.last_front_promotion
        );
        assert_eq!(
            combined_queue.last_validation_removal_reasons.get(&(1, 1)),
            Some(&BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesRotation)
        );
        assert!(sequential_queue.last_validation_removal_reasons.is_empty());
    }

    #[test]
    fn apply_head_execution_observation_emits_begin_place_for_build_head() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 12,
                y: 12,
                breaking: false,
                block_id: Some(120),
                rotation: 3,
            },
            true,
        );

        let result = queue
            .apply_head_execution_observation(BuilderQueueHeadExecutionObservation::PendingBegin);

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::BeginPlace,
                head_tile_before: Some((12, 12)),
                head_tile_after: Some((12, 12)),
                removed_entry: None,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(12, 12)]);
        assert_eq!(queue.head_tile, Some((12, 12)));
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn apply_head_execution_observation_emits_begin_break_for_breaking_head() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 13,
                y: 13,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
            true,
        );

        let result = queue
            .apply_head_execution_observation(BuilderQueueHeadExecutionObservation::PendingBegin);

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::BeginBreak,
                head_tile_before: Some((13, 13)),
                head_tile_after: Some((13, 13)),
                removed_entry: None,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(13, 13)]);
        assert_eq!(queue.head_tile, Some((13, 13)));
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
    }

    #[test]
    fn apply_head_execution_observation_defers_blocked_head_to_queue_tail() {
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

        let result = queue
            .apply_head_execution_observation(BuilderQueueHeadExecutionObservation::BlockedByUnit);

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::DeferredBlockedByUnit,
                head_tile_before: Some((1, 1)),
                head_tile_after: Some((2, 2)),
                removed_entry: None,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(2, 2), (3, 3), (1, 1)]);
        assert_eq!(queue.head_tile, Some((2, 2)));
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ExecutionDeferredToTail)
        );
    }

    #[test]
    fn apply_head_execution_observation_marks_active_construct_head_inflight() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 6,
                y: 6,
                breaking: false,
                block_id: Some(60),
                rotation: 2,
            },
            BuilderQueueEntryObservation {
                x: 7,
                y: 7,
                breaking: true,
                block_id: None,
                rotation: 1,
            },
        ]);

        let result = queue.apply_head_execution_observation(
            BuilderQueueHeadExecutionObservation::ActiveConstruct,
        );

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::ContinueConstruct,
                head_tile_before: Some((6, 6)),
                head_tile_after: Some((6, 6)),
                removed_entry: None,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(6, 6), (7, 7)]);
        assert_eq!(queue.head_tile, Some((6, 6)));
        assert_eq!(
            queue.head_entry().map(|entry| entry.stage),
            Some(BuilderQueueStage::InFlight)
        );
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 1);
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn apply_head_execution_observation_does_not_rebegin_inflight_head() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 8,
                y: 8,
                breaking: false,
                block_id: Some(80),
                rotation: 0,
            },
            true,
        );
        queue
            .active_by_tile
            .get_mut(&(8, 8))
            .expect("head entry")
            .stage = BuilderQueueStage::InFlight;
        queue.recount();

        let result = queue
            .apply_head_execution_observation(BuilderQueueHeadExecutionObservation::PendingBegin);

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::ContinueConstruct,
                head_tile_before: Some((8, 8)),
                head_tile_after: Some((8, 8)),
                removed_entry: None,
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(8, 8)]);
        assert_eq!(queue.head_tile, Some((8, 8)));
        assert_eq!(
            queue.head_entry().map(|entry| entry.stage),
            Some(BuilderQueueStage::InFlight)
        );
        assert_eq!(queue.queued_count, 0);
        assert_eq!(queue.inflight_count, 1);
        assert_eq!(queue.last_transition, Some(BuilderQueueTransition::Started));
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn apply_head_execution_observation_removes_invalid_head_and_advances_queue() {
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
                breaking: true,
                block_id: None,
                rotation: 1,
            },
        ]);

        let result = queue
            .apply_head_execution_observation(BuilderQueueHeadExecutionObservation::InvalidPlan);

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::RemovedInvalidHead,
                head_tile_before: Some((4, 4)),
                head_tile_after: Some((5, 5)),
                removed_entry: Some(BuilderQueueEntry {
                    x: 4,
                    y: 4,
                    breaking: false,
                    block_id: Some(40),
                    rotation: Some(0),
                    progress_permyriad: None,
                    stage: BuilderQueueStage::Queued,
                }),
            }
        );
        assert_eq!(queue.ordered_tiles, vec![(5, 5)]);
        assert_eq!(queue.head_tile, Some((5, 5)));
        assert_eq!(queue.queued_count, 1);
        assert_eq!(queue.inflight_count, 0);
        assert_eq!(queue.last_front_promotion, None);
    }

    #[test]
    fn apply_head_execution_observation_drops_blockless_place_head_instead_of_emitting_begin() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.enqueue_local(
            BuilderQueueEntryObservation {
                x: 14,
                y: 14,
                breaking: false,
                block_id: None,
                rotation: 2,
            },
            true,
        );

        let result = queue
            .apply_head_execution_observation(BuilderQueueHeadExecutionObservation::PendingBegin);

        assert_eq!(
            result,
            BuilderQueueHeadExecutionResult {
                action: BuilderQueueHeadExecutionAction::RemovedInvalidHead,
                head_tile_before: Some((14, 14)),
                head_tile_after: None,
                removed_entry: Some(BuilderQueueEntry {
                    x: 14,
                    y: 14,
                    breaking: false,
                    block_id: None,
                    rotation: Some(2),
                    progress_permyriad: None,
                    stage: BuilderQueueStage::Queued,
                }),
            }
        );
        assert!(queue.ordered_tiles.is_empty());
        assert_eq!(queue.head_tile, None);
        assert_eq!(queue.queued_count, 0);
    }

    #[test]
    fn build_selection_prefers_non_breaking_head_and_falls_back_when_head_is_breaking() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([
            BuilderQueueEntryObservation {
                x: 1,
                y: 1,
                breaking: true,
                block_id: None,
                rotation: 0,
            },
            BuilderQueueEntryObservation {
                x: 2,
                y: 2,
                breaking: false,
                block_id: Some(22),
                rotation: 3,
            },
        ]);

        assert_eq!(
            queue.build_selection(),
            BuilderQueueBuildSelection {
                building: true,
                selected_tile: Some((2, 2)),
                selected_block_id: Some(22),
                selected_rotation: 3,
                selection_source: Some(BuilderQueueBuildSelectionSource::Fallback),
            }
        );

        assert!(queue.move_to_front(2, 2, false));
        assert_eq!(
            queue.build_selection(),
            BuilderQueueBuildSelection {
                building: true,
                selected_tile: Some((2, 2)),
                selected_block_id: Some(22),
                selected_rotation: 3,
                selection_source: Some(BuilderQueueBuildSelectionSource::Head),
            }
        );
    }

    #[test]
    fn build_selection_marks_head_source_for_single_place_plan() {
        let mut queue = BuilderQueueStateMachine::default();
        queue.sync_local_entries([BuilderQueueEntryObservation {
            x: 4,
            y: 5,
            breaking: false,
            block_id: Some(33),
            rotation: 2,
        }]);

        assert_eq!(
            queue.build_selection(),
            BuilderQueueBuildSelection {
                building: true,
                selected_tile: Some((4, 5)),
                selected_block_id: Some(33),
                selected_rotation: 2,
                selection_source: Some(BuilderQueueBuildSelectionSource::Head),
            }
        );
    }

    #[test]
    fn update_local_activity_records_skip_reason_and_reorder_promotion() {
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

        let activity = queue.update_local_activity([
            BuilderQueueActivityObservation {
                x: 1,
                y: 1,
                breaking: false,
                in_range: false,
                should_skip: false,
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
                breaking: false,
                in_range: true,
                should_skip: false,
                distance_sq: 1,
            },
        ]);

        assert_eq!(activity.skip_reason(), None);
        assert_eq!(queue.last_skip_reason, None);
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ActivityReorderedToReachable)
        );
    }

    #[test]
    fn update_local_activity_records_requested_skip_reason_for_fallback_head() {
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
            activity.skip_reason(),
            Some(BuilderQueueSkipReason::RequestedSkip)
        );
        assert_eq!(
            queue.last_skip_reason,
            Some(BuilderQueueSkipReason::RequestedSkip)
        );
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ActivityClosestInRangeFallback)
        );
    }

    #[test]
    fn validate_against_tile_states_records_removal_reason_and_head_advance_promotion() {
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
        ]);

        let validation = queue.validate_against_tile_states([BuilderQueueTileStateObservation {
            x: 1,
            y: 1,
            block_id: Some(10),
            rotation: Some(0),
            requires_rotation_match: true,
        }]);

        assert_eq!(
            validation.reconcile_outcome,
            BuilderQueueReconcileOutcome::AdvancedHead
        );
        assert_eq!(
            queue.last_validation_removal_reasons.get(&(1, 1)),
            Some(&BuilderQueueValidationRemovalReason::PlaceAlreadyMatchesRotation)
        );
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ValidationAdvancedHead)
        );
    }

    #[test]
    fn mark_begin_and_move_to_front_record_explicit_front_promotions() {
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

        queue.mark_begin(2, 2, false, Some(20), 1);
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::BeginInFlight)
        );

        assert!(queue.move_to_front(1, 1, false));
        assert_eq!(
            queue.last_front_promotion,
            Some(BuilderQueueFrontPromotion::ExplicitMoveToFront)
        );
    }
}
