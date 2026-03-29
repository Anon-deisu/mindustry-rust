use crate::{write_byte, write_float, write_short, TypeIoReadError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityRaw {
    pub data: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponMountRaw {
    pub shoot: bool,
    pub rotate: bool,
    pub aim_x: f32,
    pub aim_y: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StatusDynamicFieldsRaw {
    pub damage_multiplier: Option<f32>,
    pub health_multiplier: Option<f32>,
    pub speed_multiplier: Option<f32>,
    pub reload_multiplier: Option<f32>,
    pub build_speed_multiplier: Option<f32>,
    pub drag_multiplier: Option<f32>,
    pub armor_override: Option<f32>,
}

impl StatusDynamicFieldsRaw {
    fn flags(self) -> u8 {
        u8::from(self.damage_multiplier.is_some())
            | (u8::from(self.health_multiplier.is_some()) << 1)
            | (u8::from(self.speed_multiplier.is_some()) << 2)
            | (u8::from(self.reload_multiplier.is_some()) << 3)
            | (u8::from(self.build_speed_multiplier.is_some()) << 4)
            | (u8::from(self.drag_multiplier.is_some()) << 5)
            | (u8::from(self.armor_override.is_some()) << 6)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusEntryRaw {
    pub status_id: i16,
    pub time: f32,
    pub dynamic_fields: Option<StatusDynamicFieldsRaw>,
}

impl WeaponMountRaw {
    fn state_byte(self) -> u8 {
        u8::from(self.shoot) | (u8::from(self.rotate) << 1)
    }

    fn from_state_byte(state: u8, aim_x: f32, aim_y: f32) -> Self {
        Self {
            shoot: (state & 1) != 0,
            rotate: (state & 2) != 0,
            aim_x,
            aim_y,
        }
    }
}

pub fn write_status_entry(out: &mut Vec<u8>, entry: &StatusEntryRaw) {
    write_short(out, entry.status_id);
    write_float(out, entry.time);
    if let Some(dynamic_fields) = entry.dynamic_fields {
        write_byte(out, dynamic_fields.flags());
        for value in [
            dynamic_fields.damage_multiplier,
            dynamic_fields.health_multiplier,
            dynamic_fields.speed_multiplier,
            dynamic_fields.reload_multiplier,
            dynamic_fields.build_speed_multiplier,
            dynamic_fields.drag_multiplier,
            dynamic_fields.armor_override,
        ]
        .into_iter()
        .flatten()
        {
            write_float(out, value);
        }
    }
}

pub fn write_status_entries(out: &mut Vec<u8>, entries: &[StatusEntryRaw], dynamic: bool) {
    let len: u8 = entries
        .len()
        .try_into()
        .expect("status entry count exceeds wire byte capacity");
    write_byte(out, len);
    for entry in entries {
        if dynamic {
            write_short(out, entry.status_id);
            write_float(out, entry.time);
            if let Some(dynamic_fields) = entry.dynamic_fields {
                write_byte(out, dynamic_fields.flags());
                for value in [
                    dynamic_fields.damage_multiplier,
                    dynamic_fields.health_multiplier,
                    dynamic_fields.speed_multiplier,
                    dynamic_fields.reload_multiplier,
                    dynamic_fields.build_speed_multiplier,
                    dynamic_fields.drag_multiplier,
                    dynamic_fields.armor_override,
                ]
                .into_iter()
                .flatten()
                {
                    write_float(out, value);
                }
            } else {
                write_byte(out, 0);
            }
        } else {
            write_status_entry(out, entry);
        }
    }
}

pub fn read_status_entry(bytes: &[u8], dynamic: bool) -> Result<StatusEntryRaw, TypeIoReadError> {
    let (entry, consumed) = read_status_entry_prefix(bytes, dynamic)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(entry)
}

pub fn read_status_entry_prefix(
    bytes: &[u8],
    dynamic: bool,
) -> Result<(StatusEntryRaw, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let status_id = reader.read_i16()?;
    let time = reader.read_f32()?;
    let dynamic_fields = if dynamic {
        let flags_position = reader.position();
        let flags = reader.read_u8()?;
        if (flags & !0b0111_1111) != 0 {
            return Err(TypeIoReadError::UnsupportedPayloadType {
                type_id: flags,
                position: flags_position,
            });
        }
        Some(StatusDynamicFieldsRaw {
            damage_multiplier: read_flagged_f32(&mut reader, flags, 0)?,
            health_multiplier: read_flagged_f32(&mut reader, flags, 1)?,
            speed_multiplier: read_flagged_f32(&mut reader, flags, 2)?,
            reload_multiplier: read_flagged_f32(&mut reader, flags, 3)?,
            build_speed_multiplier: read_flagged_f32(&mut reader, flags, 4)?,
            drag_multiplier: read_flagged_f32(&mut reader, flags, 5)?,
            armor_override: read_flagged_f32(&mut reader, flags, 6)?,
        })
    } else {
        None
    };
    Ok((
        StatusEntryRaw {
            status_id,
            time,
            dynamic_fields,
        },
        reader.position(),
    ))
}

pub fn read_status_entries(
    bytes: &[u8],
    dynamic: bool,
) -> Result<Vec<StatusEntryRaw>, TypeIoReadError> {
    let (entries, consumed) = read_status_entries_prefix(bytes, dynamic)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(entries)
}

pub fn read_status_entries_prefix(
    bytes: &[u8],
    dynamic: bool,
) -> Result<(Vec<StatusEntryRaw>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let len = reader.read_u8()? as usize;
    let mut entries = Vec::with_capacity(len);
    for _ in 0..len {
        let (entry, consumed) =
            read_status_entry_prefix(&reader.bytes[reader.position()..], dynamic)?;
        let _ = reader.read_exact(consumed)?;
        entries.push(entry);
    }
    Ok((entries, reader.position()))
}

pub fn status_name_uses_dynamic_fields(status_name: Option<&str>) -> bool {
    matches!(status_name, Some("dynamic"))
}

pub fn status_id_uses_dynamic_fields<'a, F>(status_id: i16, resolve_status_name: F) -> bool
where
    F: FnOnce(u16) -> Option<&'a str>,
{
    (status_id >= 0)
        .then_some(status_id as u16)
        .and_then(resolve_status_name)
        .is_some_and(|status_name| status_name_uses_dynamic_fields(Some(status_name)))
}

pub fn write_abilities(out: &mut Vec<u8>, abilities: &[AbilityRaw]) {
    let len: u8 = abilities
        .len()
        .try_into()
        .expect("ability count exceeds wire byte capacity");
    write_byte(out, len);
    for ability in abilities {
        write_float(out, ability.data);
    }
}

pub fn read_abilities(bytes: &[u8]) -> Result<Vec<AbilityRaw>, TypeIoReadError> {
    let (abilities, consumed) = read_abilities_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(abilities)
}

pub fn read_abilities_into(
    bytes: &[u8],
    abilities: &mut Vec<AbilityRaw>,
) -> Result<(), TypeIoReadError> {
    let (parsed, consumed) = read_abilities_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    abilities.clear();
    abilities.extend(parsed);
    Ok(())
}

pub fn read_abilities_prefix(bytes: &[u8]) -> Result<(Vec<AbilityRaw>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let len = reader.read_u8()? as usize;
    let mut abilities = Vec::with_capacity(len);
    for _ in 0..len {
        abilities.push(AbilityRaw {
            data: reader.read_f32()?,
        });
    }
    Ok((abilities, reader.position()))
}

pub fn read_abilities_into_prefix(
    bytes: &[u8],
    abilities: &mut Vec<AbilityRaw>,
) -> Result<usize, TypeIoReadError> {
    let (parsed, consumed) = read_abilities_prefix(bytes)?;
    abilities.clear();
    abilities.extend(parsed);
    Ok(consumed)
}

pub fn write_weapon_mounts(out: &mut Vec<u8>, mounts: &[WeaponMountRaw]) {
    let len: u8 = mounts
        .len()
        .try_into()
        .expect("weapon mount count exceeds wire byte capacity");
    write_byte(out, len);
    for mount in mounts {
        write_byte(out, mount.state_byte());
        write_float(out, mount.aim_x);
        write_float(out, mount.aim_y);
    }
}

pub fn read_weapon_mounts(bytes: &[u8]) -> Result<Vec<WeaponMountRaw>, TypeIoReadError> {
    let (mounts, consumed) = read_weapon_mounts_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    Ok(mounts)
}

pub fn read_weapon_mounts_into(
    bytes: &[u8],
    mounts: &mut Vec<WeaponMountRaw>,
) -> Result<(), TypeIoReadError> {
    let (parsed, consumed) = read_weapon_mounts_prefix(bytes)?;
    ensure_consumed(consumed, bytes.len())?;
    mounts.clear();
    mounts.extend(parsed);
    Ok(())
}

pub fn read_weapon_mounts_prefix(
    bytes: &[u8],
) -> Result<(Vec<WeaponMountRaw>, usize), TypeIoReadError> {
    let mut reader = PrimitiveReader::new(bytes);
    let len = reader.read_u8()? as usize;
    let mut mounts = Vec::with_capacity(len);
    for _ in 0..len {
        let state = reader.read_u8()?;
        let aim_x = reader.read_f32()?;
        let aim_y = reader.read_f32()?;
        mounts.push(WeaponMountRaw::from_state_byte(state, aim_x, aim_y));
    }
    Ok((mounts, reader.position()))
}

pub fn read_weapon_mounts_into_prefix(
    bytes: &[u8],
    mounts: &mut Vec<WeaponMountRaw>,
) -> Result<usize, TypeIoReadError> {
    let (parsed, consumed) = read_weapon_mounts_prefix(bytes)?;
    mounts.clear();
    mounts.extend(parsed);
    Ok(consumed)
}

fn ensure_consumed(consumed: usize, total: usize) -> Result<(), TypeIoReadError> {
    if consumed == total {
        Ok(())
    } else {
        Err(TypeIoReadError::TrailingBytes { consumed, total })
    }
}

fn read_flagged_f32(
    reader: &mut PrimitiveReader<'_>,
    flags: u8,
    bit: u8,
) -> Result<Option<f32>, TypeIoReadError> {
    if (flags & (1 << bit)) != 0 {
        Ok(Some(reader.read_f32()?))
    } else {
        Ok(None)
    }
}

struct PrimitiveReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> PrimitiveReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn read_u8(&mut self) -> Result<u8, TypeIoReadError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_f32(&mut self) -> Result<f32, TypeIoReadError> {
        let bytes = self.read_exact(4)?;
        Ok(f32::from_bits(u32::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ])))
    }

    fn read_i16(&mut self) -> Result<i16, TypeIoReadError> {
        let bytes = self.read_exact(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], TypeIoReadError> {
        let remaining = self.remaining();
        if remaining < len {
            return Err(TypeIoReadError::UnexpectedEof {
                position: self.position,
                needed: len,
                remaining,
            });
        }
        let start = self.position;
        self.position += len;
        Ok(&self.bytes[start..self.position])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abilities_round_trip_two_entries() {
        let abilities = vec![AbilityRaw { data: 12.5 }, AbilityRaw { data: -3.25 }];
        let mut bytes = Vec::new();

        write_abilities(&mut bytes, &abilities);

        assert_eq!(read_abilities(&bytes).unwrap(), abilities);
    }

    #[test]
    fn abilities_into_round_trip_two_entries() {
        let abilities = vec![AbilityRaw { data: 12.5 }, AbilityRaw { data: -3.25 }];
        let mut bytes = Vec::new();
        let mut target = vec![AbilityRaw { data: 99.0 }];

        write_abilities(&mut bytes, &abilities);
        read_abilities_into(&bytes, &mut target).unwrap();

        assert_eq!(target, abilities);
    }

    #[test]
    fn abilities_round_trip_empty_array() {
        let mut bytes = Vec::new();

        write_abilities(&mut bytes, &[]);

        assert_eq!(bytes, vec![0]);
        assert_eq!(read_abilities(&bytes).unwrap(), Vec::<AbilityRaw>::new());
    }

    #[test]
    fn abilities_reader_reports_truncated_payload() {
        let bytes = vec![1, 0x41, 0x48, 0x00];

        assert!(matches!(
            read_abilities(&bytes),
            Err(TypeIoReadError::UnexpectedEof {
                position: 1,
                needed: 4,
                remaining: 3,
            })
        ));
    }

    #[test]
    #[should_panic(expected = "status entry count exceeds wire byte capacity")]
    fn write_status_entries_rejects_counts_outside_u8_range() {
        let entries = vec![
            StatusEntryRaw {
                status_id: 1,
                time: 1.0,
                dynamic_fields: None,
            };
            u8::MAX as usize + 1
        ];
        let mut bytes = Vec::new();

        write_status_entries(&mut bytes, &entries, false);
    }

    #[test]
    fn abilities_into_prefix_overwrites_existing_entries() {
        let bytes = vec![2, 0x41, 0x48, 0x00, 0x00, 0xc0, 0x50, 0x00, 0x00];
        let mut abilities = vec![AbilityRaw { data: 99.0 }];

        let consumed = read_abilities_into_prefix(&bytes, &mut abilities).unwrap();

        assert_eq!(consumed, bytes.len());
        assert_eq!(
            abilities,
            vec![AbilityRaw { data: 12.5 }, AbilityRaw { data: -3.25 }]
        );
    }

    #[test]
    fn abilities_into_rejects_trailing_payload() {
        let bytes = vec![1, 0x41, 0x48, 0x00, 0x00, 0xff];
        let mut abilities = vec![AbilityRaw { data: 99.0 }];

        assert!(matches!(
            read_abilities_into(&bytes, &mut abilities),
            Err(TypeIoReadError::TrailingBytes {
                consumed: 5,
                total: 6,
            })
        ));
        assert_eq!(abilities, vec![AbilityRaw { data: 99.0 }]);
    }

    #[test]
    #[should_panic(expected = "ability count exceeds wire byte capacity")]
    fn write_abilities_rejects_counts_outside_u8_range() {
        let abilities = vec![AbilityRaw { data: 0.0 }; u8::MAX as usize + 1];
        let mut bytes = Vec::new();

        write_abilities(&mut bytes, &abilities);
    }

    #[test]
    fn weapon_mounts_round_trip_two_entries() {
        let mounts = vec![
            WeaponMountRaw {
                shoot: true,
                rotate: false,
                aim_x: 12.5,
                aim_y: -3.25,
            },
            WeaponMountRaw {
                shoot: false,
                rotate: true,
                aim_x: -8.0,
                aim_y: 64.5,
            },
        ];
        let mut bytes = Vec::new();

        write_weapon_mounts(&mut bytes, &mounts);

        assert_eq!(read_weapon_mounts(&bytes).unwrap(), mounts);
    }

    #[test]
    fn weapon_mounts_into_round_trip_two_entries() {
        let mounts = vec![
            WeaponMountRaw {
                shoot: true,
                rotate: false,
                aim_x: 12.5,
                aim_y: -3.25,
            },
            WeaponMountRaw {
                shoot: false,
                rotate: true,
                aim_x: -8.0,
                aim_y: 64.5,
            },
        ];
        let mut bytes = Vec::new();
        let mut target = vec![WeaponMountRaw {
            shoot: false,
            rotate: false,
            aim_x: 99.0,
            aim_y: 99.0,
        }];

        write_weapon_mounts(&mut bytes, &mounts);
        read_weapon_mounts_into(&bytes, &mut target).unwrap();

        assert_eq!(target, mounts);
    }

    #[test]
    fn weapon_mounts_round_trip_empty_array() {
        let mut bytes = Vec::new();

        write_weapon_mounts(&mut bytes, &[]);

        assert_eq!(bytes, vec![0]);
        assert_eq!(
            read_weapon_mounts(&bytes).unwrap(),
            Vec::<WeaponMountRaw>::new()
        );
    }

    #[test]
    fn weapon_mounts_reader_reports_truncated_payload() {
        let bytes = vec![1, 3, 0x41, 0x48, 0x00];

        assert!(matches!(
            read_weapon_mounts(&bytes),
            Err(TypeIoReadError::UnexpectedEof {
                position: 2,
                needed: 4,
                remaining: 3,
            })
        ));
    }

    #[test]
    fn weapon_mounts_into_prefix_overwrites_existing_entries() {
        let bytes = vec![
            1,
            0b0000_0011,
            0x41,
            0x48,
            0x00,
            0x00,
            0xc0,
            0x50,
            0x00,
            0x00,
        ];
        let mut mounts = vec![WeaponMountRaw {
            shoot: false,
            rotate: false,
            aim_x: 99.0,
            aim_y: 99.0,
        }];

        let consumed = read_weapon_mounts_into_prefix(&bytes, &mut mounts).unwrap();

        assert_eq!(consumed, bytes.len());
        assert_eq!(
            mounts,
            vec![WeaponMountRaw {
                shoot: true,
                rotate: true,
                aim_x: 12.5,
                aim_y: -3.25,
            }]
        );
    }

    #[test]
    fn weapon_mounts_into_rejects_trailing_payload() {
        let bytes = vec![
            1,
            0b0000_0011,
            0x41,
            0x48,
            0x00,
            0x00,
            0xc0,
            0x50,
            0x00,
            0x00,
            0xff,
        ];
        let mut mounts = vec![WeaponMountRaw {
            shoot: false,
            rotate: false,
            aim_x: 99.0,
            aim_y: 99.0,
        }];

        assert!(matches!(
            read_weapon_mounts_into(&bytes, &mut mounts),
            Err(TypeIoReadError::TrailingBytes {
                consumed: 10,
                total: 11,
            })
        ));
        assert_eq!(
            mounts,
            vec![WeaponMountRaw {
                shoot: false,
                rotate: false,
                aim_x: 99.0,
                aim_y: 99.0,
            }]
        );
    }

    #[test]
    #[should_panic(expected = "weapon mount count exceeds wire byte capacity")]
    fn write_weapon_mounts_rejects_counts_outside_u8_range() {
        let mounts = vec![
            WeaponMountRaw {
                shoot: false,
                rotate: false,
                aim_x: 0.0,
                aim_y: 0.0,
            };
            u8::MAX as usize + 1
        ];
        let mut bytes = Vec::new();

        write_weapon_mounts(&mut bytes, &mounts);
    }

    #[test]
    fn status_entry_round_trip_without_dynamic_fields() {
        let entry = StatusEntryRaw {
            status_id: 27,
            time: 45.5,
            dynamic_fields: None,
        };
        let mut bytes = Vec::new();

        write_status_entry(&mut bytes, &entry);

        assert_eq!(read_status_entry(&bytes, false).unwrap(), entry);
    }

    #[test]
    fn status_entry_round_trip_with_sparse_dynamic_fields() {
        let entry = StatusEntryRaw {
            status_id: 91,
            time: 12.25,
            dynamic_fields: Some(StatusDynamicFieldsRaw {
                damage_multiplier: Some(1.5),
                speed_multiplier: Some(0.75),
                armor_override: Some(6.0),
                ..StatusDynamicFieldsRaw::default()
            }),
        };
        let mut bytes = Vec::new();

        write_status_entry(&mut bytes, &entry);

        assert_eq!(read_status_entry(&bytes, true).unwrap(), entry);
    }

    #[test]
    fn status_entries_round_trip_two_entries() {
        let entries = vec![
            StatusEntryRaw {
                status_id: 27,
                time: 45.5,
                dynamic_fields: Some(StatusDynamicFieldsRaw::default()),
            },
            StatusEntryRaw {
                status_id: 91,
                time: 12.25,
                dynamic_fields: Some(StatusDynamicFieldsRaw {
                    damage_multiplier: Some(1.5),
                    speed_multiplier: Some(0.75),
                    armor_override: Some(6.0),
                    ..StatusDynamicFieldsRaw::default()
                }),
            },
        ];
        let mut bytes = Vec::new();

        write_status_entries(&mut bytes, &entries, true);

        assert_eq!(read_status_entries(&bytes, true).unwrap(), entries);
    }

    #[test]
    fn status_entries_round_trip_empty_array() {
        let mut bytes = Vec::new();

        write_status_entries(&mut bytes, &[], false);

        assert_eq!(bytes, vec![0]);
        assert_eq!(
            read_status_entries(&bytes, false).unwrap(),
            Vec::<StatusEntryRaw>::new()
        );
    }

    #[test]
    fn status_entries_reader_reports_truncated_payload() {
        let bytes = vec![1, 0, 27, 0x42];

        assert!(matches!(
            read_status_entries(&bytes, false),
            Err(TypeIoReadError::UnexpectedEof {
                position: 2,
                needed: 4,
                remaining: 1,
            })
        ));
    }

    #[test]
    fn status_name_uses_dynamic_fields_only_for_dynamic_effect_name() {
        assert!(status_name_uses_dynamic_fields(Some("dynamic")));
        assert!(!status_name_uses_dynamic_fields(None));
        assert!(!status_name_uses_dynamic_fields(Some("freeze")));
    }

    #[test]
    fn status_id_uses_dynamic_fields_resolves_via_lookup_closure() {
        assert!(status_id_uses_dynamic_fields(7, |status_id| {
            (status_id == 7).then_some("dynamic")
        }));
        assert!(!status_id_uses_dynamic_fields(7, |status_id| {
            (status_id == 7).then_some("freeze")
        }));
        assert!(!status_id_uses_dynamic_fields(-1, |_| Some("dynamic")));
    }

    #[test]
    fn status_entry_reader_reports_truncated_dynamic_payload() {
        let bytes = vec![0, 5, 0x41, 0x40, 0x00, 0x00, 0b0000_0101, 0x3f];

        assert!(matches!(
            read_status_entry(&bytes, true),
            Err(TypeIoReadError::UnexpectedEof {
                position: 7,
                needed: 4,
                remaining: 1,
            })
        ));
    }

    #[test]
    fn status_entry_reader_rejects_reserved_dynamic_flag_bits() {
        let bytes = vec![0, 5, 0x41, 0x40, 0x00, 0x00, 0b1000_0000];

        assert!(matches!(
            read_status_entry(&bytes, true),
            Err(TypeIoReadError::UnsupportedPayloadType {
                type_id: 0b1000_0000,
                position: 6,
            })
        ));
    }

    #[test]
    fn status_entry_reader_reports_trailing_payload() {
        let entry = StatusEntryRaw {
            status_id: 27,
            time: 45.5,
            dynamic_fields: None,
        };
        let mut bytes = Vec::new();

        write_status_entry(&mut bytes, &entry);
        bytes.push(0xff);

        assert!(matches!(
            read_status_entry(&bytes, false),
            Err(TypeIoReadError::TrailingBytes {
                consumed: 6,
                total: 7,
            })
        ));
    }
}
