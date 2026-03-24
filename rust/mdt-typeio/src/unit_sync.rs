use crate::{write_byte, write_float, TypeIoReadError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponMountRaw {
    pub shoot: bool,
    pub rotate: bool,
    pub aim_x: f32,
    pub aim_y: f32,
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

fn ensure_consumed(consumed: usize, total: usize) -> Result<(), TypeIoReadError> {
    if consumed == total {
        Ok(())
    } else {
        Err(TypeIoReadError::TrailingBytes { consumed, total })
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
    fn weapon_mounts_round_trip_empty_array() {
        let mut bytes = Vec::new();

        write_weapon_mounts(&mut bytes, &[]);

        assert_eq!(bytes, vec![0]);
        assert_eq!(read_weapon_mounts(&bytes).unwrap(), Vec::<WeaponMountRaw>::new());
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
}
