pub(crate) const ALPHA_SHAPE_ENTITY_CLASS_IDS: [u8; 17] = [
    0, 2, 3, 16, 18, 20, 21, 24, 29, 30, 31, 33, 40, 43, 44, 45, 46,
];
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
