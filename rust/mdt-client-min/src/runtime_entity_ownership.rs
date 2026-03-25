use super::{TypedRuntimeEntityKind, TypedRuntimeEntityModel};
use std::collections::BTreeMap;

const OWNERSHIP_CONFLICT_SAMPLE_LIMIT: usize = 4;

#[derive(Debug, Default)]
pub(super) struct TypedRuntimeEntityOwnershipResolution {
    pub player_owned_unit_by_player_entity_id: BTreeMap<i32, i32>,
    pub unit_owner_player_by_unit_entity_id: BTreeMap<i32, i32>,
    pub ownership_conflict_count: usize,
    pub ownership_conflict_unit_sample: Vec<i32>,
}

pub(super) fn resolve_typed_runtime_entity_ownership(
    by_entity_id: &BTreeMap<i32, TypedRuntimeEntityModel>,
) -> TypedRuntimeEntityOwnershipResolution {
    let mut ownership_claims_by_unit_id = BTreeMap::<i32, Vec<(u64, i32)>>::new();

    for (&player_entity_id, model) in by_entity_id {
        let TypedRuntimeEntityModel::Player(player) = model else {
            continue;
        };
        if player.base.unit_kind != 2 {
            continue;
        }
        let Ok(unit_entity_id) = i32::try_from(player.base.unit_value) else {
            continue;
        };
        if unit_entity_id <= 0 {
            continue;
        }
        let Some(unit_model) = by_entity_id.get(&unit_entity_id) else {
            continue;
        };
        if unit_model.kind() != TypedRuntimeEntityKind::Unit {
            continue;
        }
        ownership_claims_by_unit_id
            .entry(unit_entity_id)
            .or_default()
            .push((
                player.base.last_seen_entity_snapshot_count,
                player_entity_id,
            ));
    }

    let mut resolution = TypedRuntimeEntityOwnershipResolution::default();
    for (unit_entity_id, mut claimants) in ownership_claims_by_unit_id {
        if claimants.len() != 1 {
            resolution.ownership_conflict_count =
                resolution.ownership_conflict_count.saturating_add(1);
            if resolution.ownership_conflict_unit_sample.len() < OWNERSHIP_CONFLICT_SAMPLE_LIMIT {
                resolution
                    .ownership_conflict_unit_sample
                    .push(unit_entity_id);
            }
            continue;
        }
        let (_, player_entity_id) = claimants.pop().unwrap();
        resolution
            .player_owned_unit_by_player_entity_id
            .insert(player_entity_id, unit_entity_id);
        resolution
            .unit_owner_player_by_unit_entity_id
            .insert(unit_entity_id, player_entity_id);
    }
    resolution
}
