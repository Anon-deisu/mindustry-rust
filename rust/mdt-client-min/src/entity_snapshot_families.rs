pub(crate) const ALPHA_SHAPE_ENTITY_CLASS_IDS: [u8; 17] = [
    0, 2, 3, 16, 18, 20, 21, 24, 29, 30, 31, 33, 40, 43, 44, 45, 46,
];
#[cfg(test)]
pub(crate) const ALPHA_SHAPE_CURRENT_VANILLA_ENTITY_CLASS_IDS: [u8; 15] =
    [0, 2, 3, 16, 18, 20, 21, 24, 29, 30, 31, 33, 43, 45, 46];
#[cfg(test)]
pub(crate) const ALPHA_SHAPE_LEGACY_ALIAS_ENTITY_CLASS_IDS: [u8; 2] = [40, 44];
pub(crate) const BUILDING_ENTITY_CLASS_IDS: [u8; 1] = [6];
pub(crate) const MECH_SHAPE_ENTITY_CLASS_IDS: [u8; 4] = [4, 17, 19, 32];
pub(crate) const MISSILE_SHAPE_ENTITY_CLASS_IDS: [u8; 1] = [39];
pub(crate) const PAYLOAD_SHAPE_ENTITY_CLASS_IDS: [u8; 3] = [5, 23, 26];
pub(crate) const BUILDING_TETHER_PAYLOAD_ENTITY_CLASS_IDS: [u8; 1] = [36];
pub(crate) const FIRE_ENTITY_CLASS_IDS: [u8; 1] = [10];
pub(crate) const PUDDLE_ENTITY_CLASS_IDS: [u8; 1] = [13];
pub(crate) const WEATHER_STATE_ENTITY_CLASS_IDS: [u8; 1] = [14];
pub(crate) const WORLD_LABEL_ENTITY_CLASS_IDS: [u8; 1] = [35];

pub(crate) fn is_building_entity_class_id(class_id: u8) -> bool {
    BUILDING_ENTITY_CLASS_IDS.contains(&class_id)
}

#[cfg(test)]
pub(crate) fn is_current_vanilla_alpha_shape_entity_class_id(class_id: u8) -> bool {
    ALPHA_SHAPE_CURRENT_VANILLA_ENTITY_CLASS_IDS.contains(&class_id)
}

#[cfg(test)]
pub(crate) fn is_runtime_compatible_alpha_shape_entity_class_id(class_id: u8) -> bool {
    ALPHA_SHAPE_ENTITY_CLASS_IDS.contains(&class_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_building_entity_class_id_matches_only_building_class_ids() {
        assert!(is_building_entity_class_id(BUILDING_ENTITY_CLASS_IDS[0]));
        assert!(!is_building_entity_class_id(0));
        assert!(!is_building_entity_class_id(5));
        assert!(!is_building_entity_class_id(7));
        assert!(!is_building_entity_class_id(255));
    }

    #[test]
    fn current_vanilla_alpha_shape_entity_class_ids_exclude_legacy_aliases() {
        for legacy_alias in ALPHA_SHAPE_LEGACY_ALIAS_ENTITY_CLASS_IDS {
            assert!(!ALPHA_SHAPE_CURRENT_VANILLA_ENTITY_CLASS_IDS.contains(&legacy_alias));
        }
        assert_eq!(ALPHA_SHAPE_CURRENT_VANILLA_ENTITY_CLASS_IDS.len(), 15);
    }

    #[test]
    fn runtime_compatible_alpha_shape_entity_class_ids_still_accept_legacy_aliases() {
        assert!(is_runtime_compatible_alpha_shape_entity_class_id(
            ALPHA_SHAPE_LEGACY_ALIAS_ENTITY_CLASS_IDS[0]
        ));
        assert!(is_runtime_compatible_alpha_shape_entity_class_id(
            ALPHA_SHAPE_LEGACY_ALIAS_ENTITY_CLASS_IDS[1]
        ));
        assert!(is_current_vanilla_alpha_shape_entity_class_id(43));
        assert!(!is_current_vanilla_alpha_shape_entity_class_id(
            ALPHA_SHAPE_LEGACY_ALIAS_ENTITY_CLASS_IDS[0]
        ));
        assert!(!is_current_vanilla_alpha_shape_entity_class_id(
            ALPHA_SHAPE_LEGACY_ALIAS_ENTITY_CLASS_IDS[1]
        ));
    }

    #[test]
    fn runtime_compatible_alpha_shape_entity_class_id_accepts_all_listed_ids_and_rejects_neighbors() {
        for class_id in ALPHA_SHAPE_ENTITY_CLASS_IDS {
            assert!(is_runtime_compatible_alpha_shape_entity_class_id(class_id));
        }

        assert!(!is_runtime_compatible_alpha_shape_entity_class_id(1));
        assert!(!is_runtime_compatible_alpha_shape_entity_class_id(47));

        assert_eq!(BUILDING_ENTITY_CLASS_IDS, [6]);
        assert!(is_building_entity_class_id(BUILDING_ENTITY_CLASS_IDS[0]));
        assert!(!is_building_entity_class_id(5));
        assert!(!is_building_entity_class_id(7));
    }
}
