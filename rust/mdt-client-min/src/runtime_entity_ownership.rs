use super::{TypedRuntimeEntityKind, TypedRuntimeEntityModel, TypedRuntimeUnitEntity};
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
    let mut resolution = TypedRuntimeEntityOwnershipResolution::default();
    let mut authoritative_claims_by_player_id = BTreeMap::<i32, Vec<(u64, i32)>>::new();

    for (&unit_entity_id, model) in by_entity_id {
        let TypedRuntimeEntityModel::Unit(unit) = model else {
            continue;
        };
        let Some(player_entity_id) = authoritative_player_controller_entity_id(unit, by_entity_id)
        else {
            continue;
        };
        authoritative_claims_by_player_id
            .entry(player_entity_id)
            .or_default()
            .push((unit.base.last_seen_entity_snapshot_count, unit_entity_id));
    }

    for (player_entity_id, claimants) in authoritative_claims_by_player_id {
        let Some(unit_entity_id) = unique_latest_claim_entity_id(&claimants) else {
            record_conflict_units(
                &mut resolution,
                claimants
                    .into_iter()
                    .map(|(_, unit_entity_id)| unit_entity_id),
            );
            continue;
        };
        resolution
            .player_owned_unit_by_player_entity_id
            .insert(player_entity_id, unit_entity_id);
        resolution
            .unit_owner_player_by_unit_entity_id
            .insert(unit_entity_id, player_entity_id);
    }

    let mut ownership_claims_by_unit_id = BTreeMap::<i32, Vec<(u64, i32)>>::new();

    for (&player_entity_id, model) in by_entity_id {
        let TypedRuntimeEntityModel::Player(player) = model else {
            continue;
        };
        if resolution
            .player_owned_unit_by_player_entity_id
            .contains_key(&player_entity_id)
        {
            continue;
        }
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
        if resolution
            .unit_owner_player_by_unit_entity_id
            .contains_key(&unit_entity_id)
        {
            continue;
        }
        let TypedRuntimeEntityModel::Unit(unit_model) = unit_model else {
            continue;
        };
        if !unit_allows_heuristic_player_ownership(unit_model) {
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

    for (unit_entity_id, claimants) in ownership_claims_by_unit_id {
        let Some(player_entity_id) = unique_latest_claim_entity_id(&claimants) else {
            record_conflict_units(&mut resolution, [unit_entity_id]);
            continue;
        };
        resolution
            .player_owned_unit_by_player_entity_id
            .insert(player_entity_id, unit_entity_id);
        resolution
            .unit_owner_player_by_unit_entity_id
            .insert(unit_entity_id, player_entity_id);
    }
    resolution
}

fn unique_latest_claim_entity_id(claimants: &[(u64, i32)]) -> Option<i32> {
    let mut max_snapshot_count = None::<u64>;
    let mut latest_entity_id = None::<i32>;
    let mut max_count_claimants = 0usize;

    for &(snapshot_count, entity_id) in claimants {
        match max_snapshot_count {
            None => {
                max_snapshot_count = Some(snapshot_count);
                latest_entity_id = Some(entity_id);
                max_count_claimants = 1;
            }
            Some(current) if snapshot_count > current => {
                max_snapshot_count = Some(snapshot_count);
                latest_entity_id = Some(entity_id);
                max_count_claimants = 1;
            }
            Some(current) if snapshot_count == current => {
                max_count_claimants = max_count_claimants.saturating_add(1);
            }
            Some(_) => {}
        }
    }

    if max_count_claimants == 1 {
        latest_entity_id
    } else {
        None
    }
}

fn record_conflict_units(
    resolution: &mut TypedRuntimeEntityOwnershipResolution,
    conflicted_unit_ids: impl IntoIterator<Item = i32>,
) {
    for unit_entity_id in conflicted_unit_ids {
        resolution.ownership_conflict_count = resolution.ownership_conflict_count.saturating_add(1);
        if resolution.ownership_conflict_unit_sample.len() < OWNERSHIP_CONFLICT_SAMPLE_LIMIT {
            resolution
                .ownership_conflict_unit_sample
                .push(unit_entity_id);
        }
    }
}

fn authoritative_player_controller_entity_id(
    unit: &TypedRuntimeUnitEntity,
    by_entity_id: &BTreeMap<i32, TypedRuntimeEntityModel>,
) -> Option<i32> {
    if unit.semantic.controller_type != 0 {
        return None;
    }
    let player_entity_id = unit.semantic.controller_value?;
    matches!(
        by_entity_id.get(&player_entity_id),
        Some(TypedRuntimeEntityModel::Player(_))
    )
    .then_some(player_entity_id)
}

fn unit_allows_heuristic_player_ownership(unit: &TypedRuntimeUnitEntity) -> bool {
    unit.semantic.controller_type == 0
        && unit.semantic.controller_value.is_none()
        && unit.unit_payload.is_none()
}

#[cfg(test)]
mod tests {
    use super::{
        record_conflict_units, resolve_typed_runtime_entity_ownership,
        unique_latest_claim_entity_id,
    };
    use crate::session_state::{
        EntityPlayerSemanticProjection, EntityUnitSemanticProjection, TypedRuntimeEntityBase,
        TypedRuntimeEntityModel, TypedRuntimePlayerEntity, TypedRuntimeUnitEntity,
    };
    use std::collections::BTreeMap;

    fn player(
        entity_id: i32,
        unit_value: u32,
        last_seen_entity_snapshot_count: u64,
    ) -> TypedRuntimeEntityModel {
        TypedRuntimeEntityModel::Player(TypedRuntimePlayerEntity {
            base: TypedRuntimeEntityBase {
                entity_id,
                class_id: 12,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count,
            },
            semantic: EntityPlayerSemanticProjection::default(),
        })
    }

    fn unit(
        entity_id: i32,
        controller_type: u8,
        controller_value: Option<i32>,
        last_seen_entity_snapshot_count: u64,
    ) -> TypedRuntimeEntityModel {
        TypedRuntimeEntityModel::Unit(TypedRuntimeUnitEntity {
            base: TypedRuntimeEntityBase {
                entity_id,
                class_id: 4,
                hidden: false,
                is_local_player: false,
                unit_kind: 2,
                unit_value: entity_id as u32,
                x_bits: 0,
                y_bits: 0,
                last_seen_entity_snapshot_count,
            },
            semantic: EntityUnitSemanticProjection {
                team_id: 1,
                unit_type_id: 55,
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
                controller_type,
                controller_value,
            },
            unit_payload: None,
            carried_item_stack: None,
        })
    }

    fn payload_unit(
        entity_id: i32,
        controller_type: u8,
        controller_value: Option<i32>,
        last_seen_entity_snapshot_count: u64,
    ) -> TypedRuntimeEntityModel {
        let TypedRuntimeEntityModel::Unit(mut unit_model) = unit(
            entity_id,
            controller_type,
            controller_value,
            last_seen_entity_snapshot_count,
        ) else {
            unreachable!("unit helper must return unit model");
        };
        unit_model.unit_payload = Some(mdt_world::UnitPayloadSnapshot {
            class_id: 5,
            revision: 7,
            body_len: 12,
            body_sha256:
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            nested_unit_payloads: Vec::new(),
        });
        TypedRuntimeEntityModel::Unit(unit_model)
    }

    #[test]
    fn unique_latest_claim_entity_id_prefers_single_latest_claim_and_rejects_ties() {
        assert_eq!(unique_latest_claim_entity_id(&[]), None);
        assert_eq!(unique_latest_claim_entity_id(&[(3, 11)]), Some(11));
        assert_eq!(
            unique_latest_claim_entity_id(&[(2, 7), (5, 9), (4, 3)]),
            Some(9)
        );
        assert_eq!(unique_latest_claim_entity_id(&[(5, 7), (5, 9)]), None);
        assert_eq!(
            unique_latest_claim_entity_id(&[(1, 4), (3, 8), (3, 12), (2, 6)]),
            None
        );
    }

    #[test]
    fn controller_backed_ownership_wins_over_heuristic_claims() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 0, 7)),
            (102, player(102, 202, 8)),
            (202, unit(202, 0, Some(101), 1)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            Some(&202)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            Some(&101)
        );
        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&102),
            None
        );
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn heuristic_fallback_still_works_without_controller() {
        let by_entity_id =
            BTreeMap::from([(101, player(101, 202, 7)), (202, unit(202, 0, None, 1))]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            Some(&202)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            Some(&101)
        );
        assert_eq!(resolution.ownership_conflict_count, 0);
    }

    #[test]
    fn heuristic_fallback_does_not_claim_payload_carrying_unit() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 7)),
            (202, payload_unit(202, 0, None, 1)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            None
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            None
        );
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn newer_heuristic_player_claim_wins_for_same_unit() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 7)),
            (102, player(102, 202, 9)),
            (202, unit(202, 0, None, 1)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            None
        );
        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&102),
            Some(&202)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            Some(&102)
        );
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn equal_heuristic_player_claims_for_same_unit_record_conflict_without_blocking_other_units() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 7)),
            (102, player(102, 202, 7)),
            (103, player(103, 303, 8)),
            (202, unit(202, 0, None, 1)),
            (303, unit(303, 0, None, 1)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            None
        );
        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&102),
            None
        );
        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&103),
            Some(&303)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            None
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&303),
            Some(&103)
        );
        assert_eq!(resolution.ownership_conflict_count, 1);
        assert_eq!(resolution.ownership_conflict_unit_sample, vec![202]);
    }

    #[test]
    fn non_player_controller_does_not_create_player_ownership() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 7)),
            (202, unit(202, 0, Some(303), 1)),
            (303, unit(303, 0, None, 1)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert!(resolution.player_owned_unit_by_player_entity_id.is_empty());
        assert!(resolution.unit_owner_player_by_unit_entity_id.is_empty());
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn newer_authoritative_unit_claim_wins_for_same_player() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 9)),
            (202, unit(202, 0, Some(101), 7)),
            (303, unit(303, 0, Some(101), 9)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            Some(&303)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&303),
            Some(&101)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            None
        );
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn equal_authoritative_unit_claims_for_same_player_record_conflict() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 9)),
            (202, unit(202, 0, Some(101), 9)),
            (303, unit(303, 0, Some(101), 9)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            None
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            None
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&303),
            None
        );
        assert_eq!(resolution.ownership_conflict_count, 2);
        assert_eq!(resolution.ownership_conflict_unit_sample, vec![202, 303]);
    }

    #[test]
    fn record_conflict_units_caps_sample_and_preserves_order() {
        let mut resolution = Default::default();

        record_conflict_units(&mut resolution, [42, 7, 19, 23, 88, 91]);

        assert_eq!(resolution.ownership_conflict_count, 6);
        assert_eq!(resolution.ownership_conflict_unit_sample.len(), 4);
        assert_eq!(
            resolution.ownership_conflict_unit_sample,
            vec![42, 7, 19, 23]
        );
    }

    #[test]
    fn heuristic_claim_does_not_override_authoritative_owner_for_same_player() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 303, 9)),
            (202, unit(202, 0, Some(101), 9)),
            (303, unit(303, 0, None, 9)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert_eq!(
            resolution.player_owned_unit_by_player_entity_id.get(&101),
            Some(&202)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&202),
            Some(&101)
        );
        assert_eq!(
            resolution.unit_owner_player_by_unit_entity_id.get(&303),
            None
        );
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn nonzero_controller_type_blocks_heuristic_fallback() {
        let by_entity_id =
            BTreeMap::from([(101, player(101, 202, 7)), (202, unit(202, 1, None, 1))]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert!(resolution.player_owned_unit_by_player_entity_id.is_empty());
        assert!(resolution.unit_owner_player_by_unit_entity_id.is_empty());
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }

    #[test]
    fn invalid_controller_value_blocks_heuristic_fallback() {
        let by_entity_id = BTreeMap::from([
            (101, player(101, 202, 7)),
            (202, unit(202, 0, Some(999), 1)),
            (999, unit(999, 0, None, 1)),
        ]);

        let resolution = resolve_typed_runtime_entity_ownership(&by_entity_id);

        assert!(resolution.player_owned_unit_by_player_entity_id.is_empty());
        assert!(resolution.unit_owner_player_by_unit_entity_id.is_empty());
        assert_eq!(resolution.ownership_conflict_count, 0);
        assert!(resolution.ownership_conflict_unit_sample.is_empty());
    }
}
