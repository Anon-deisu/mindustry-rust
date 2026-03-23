use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION_PROPERTIES: &[u8] = include_bytes!("../assets/version.properties");
const BASE64_ENCODE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
static GENERATED_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectPacketSpec {
    pub version: i32,
    pub version_type: String,
    pub name: String,
    pub locale: String,
    pub usid: String,
    pub uuid: String,
    pub mobile: bool,
    pub color: i32,
    pub mods: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectCompatibilityWarningCode {
    BuildUnknown,
    VersionTypeCustomLike,
}

impl ConnectCompatibilityWarningCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BuildUnknown => "build_unknown",
            Self::VersionTypeCustomLike => "version_type_custom_like",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectCompatibilityWarning {
    pub code: ConnectCompatibilityWarningCode,
    pub message: &'static str,
}

impl ConnectPacketSpec {
    pub fn new_default(locale: impl Into<String>) -> Self {
        Self {
            version: default_connect_build(),
            version_type: default_connect_version_type().to_string(),
            name: "mdt-client-min".to_string(),
            locale: locale.into(),
            usid: generate_connect_identity_base64(),
            uuid: generate_connect_identity_base64(),
            mobile: false,
            color: -1,
            mods: Vec::new(),
        }
    }

    pub fn encode_payload(&self) -> Result<Vec<u8>, ConnectPacketEncodeError> {
        let raw_uuid = self.preflight_and_decode_uuid()?;
        let mut out = Vec::new();
        out.extend_from_slice(&self.version.to_be_bytes());
        write_typeio_string(&mut out, "versionType", &self.version_type)?;
        write_typeio_string(&mut out, "name", &self.name)?;
        write_typeio_string(&mut out, "locale", &self.locale)?;
        write_typeio_string(&mut out, "usid", &self.usid)?;
        out.extend_from_slice(&raw_uuid);
        let crc = crc32(&raw_uuid) as u64;
        out.extend_from_slice(&crc.to_be_bytes());

        out.push(u8::from(self.mobile));
        out.extend_from_slice(&self.color.to_be_bytes());

        let mod_count = u8::try_from(self.mods.len())
            .map_err(|_| ConnectPacketEncodeError::TooManyMods(self.mods.len()))?;
        out.push(mod_count);
        for entry in &self.mods {
            write_typeio_string(&mut out, "mods", entry)?;
        }

        Ok(out)
    }

    pub fn server_observed_uuid(&self) -> Result<String, ConnectPacketEncodeError> {
        let raw_uuid = self.preflight_and_decode_uuid()?;
        let mut combined = Vec::with_capacity(raw_uuid.len() + 8);
        combined.extend_from_slice(&raw_uuid);
        combined.extend_from_slice(&(crc32(&raw_uuid) as u64).to_be_bytes());
        Ok(encode_base64(&combined))
    }

    pub fn compatibility_warnings(&self) -> Vec<ConnectCompatibilityWarning> {
        let mut warnings = Vec::new();
        if self.version < 0 {
            warnings.push(ConnectCompatibilityWarning {
                code: ConnectCompatibilityWarningCode::BuildUnknown,
                message: "connect build is negative; strict servers may reject custom/non-release clients. Consider setting --build to a concrete release build number.",
            });
        }

        let normalized_version_type = self.version_type.trim();
        if normalized_version_type.eq_ignore_ascii_case("custom")
            || normalized_version_type.eq_ignore_ascii_case("custom build")
        {
            warnings.push(ConnectCompatibilityWarning {
                code: ConnectCompatibilityWarningCode::VersionTypeCustomLike,
                message: "connect version-type is custom-like; strict servers may reject custom clients. Consider setting --version-type to the server-accepted value.",
            });
        }

        warnings
    }

    fn preflight_and_decode_uuid(&self) -> Result<Vec<u8>, ConnectPacketEncodeError> {
        require_non_empty_connect_field("usid", &self.usid)?;
        require_non_empty_connect_field("uuid", &self.uuid)?;
        for (index, entry) in self.mods.iter().enumerate() {
            if entry.trim().is_empty() {
                return Err(ConnectPacketEncodeError::InvalidModEntry {
                    index,
                    reason: "must not be empty",
                });
            }
        }

        let raw_uuid = decode_base64(&self.uuid)?;
        if raw_uuid.is_empty() {
            return Err(ConnectPacketEncodeError::InvalidUuidLength(0));
        }
        Ok(raw_uuid)
    }
}

pub fn default_connect_build() -> i32 {
    version_properties_value("build")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(-1)
}

pub fn default_connect_version_type() -> &'static str {
    version_properties_value("type").unwrap_or("official")
}

pub fn generate_connect_identity_base64() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = u128::from(GENERATED_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
    let pid = u128::from(std::process::id());
    let mut mixed = now ^ (pid << 32) ^ counter;

    // A small local mixer is enough here; we only need collision-resistant IDs per run.
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xff51afd7ed558ccd_u128);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xc4ceb9fe1a85ec53_u128);
    mixed ^= mixed >> 33;

    encode_base64(&(mixed as u64).to_be_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectPacketEncodeError {
    InvalidBase64Length(usize),
    InvalidBase64Char { ch: char, index: usize },
    EmptyField(&'static str),
    InvalidUuidLength(usize),
    InvalidModEntry { index: usize, reason: &'static str },
    StringTooLong { field: &'static str, utf_len: usize },
    TooManyMods(usize),
}

impl fmt::Display for ConnectPacketEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase64Length(len) => {
                write!(f, "invalid base64 length for connect identity: {len}")
            }
            Self::InvalidBase64Char { ch, index } => {
                write!(f, "invalid base64 character '{ch}' at index {index}")
            }
            Self::EmptyField(field) => write!(f, "connect field {field} must not be empty"),
            Self::InvalidUuidLength(len) => {
                write!(f, "decoded connect uuid must not be empty, got {len} bytes")
            }
            Self::InvalidModEntry { index, reason } => {
                write!(f, "invalid connect mod entry at index {index}: {reason}")
            }
            Self::StringTooLong { field, utf_len } => {
                write!(
                    f,
                    "connect field {field} exceeds Java UTF limit: {utf_len} bytes"
                )
            }
            Self::TooManyMods(count) => write!(f, "too many connect mods: {count}"),
        }
    }
}

impl std::error::Error for ConnectPacketEncodeError {}

fn require_non_empty_connect_field(
    field: &'static str,
    value: &str,
) -> Result<(), ConnectPacketEncodeError> {
    if value.trim().is_empty() {
        return Err(ConnectPacketEncodeError::EmptyField(field));
    }
    Ok(())
}

fn version_properties_value(key: &str) -> Option<&'static str> {
    let key_bytes = key.as_bytes();
    for line in VERSION_PROPERTIES.split(|byte| *byte == b'\n') {
        let line = match line.last() {
            Some(b'\r') => &line[..line.len() - 1],
            _ => line,
        };
        let Some(eq_index) = line.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let found_key = trim_ascii(&line[..eq_index]);
        if found_key != key_bytes {
            continue;
        }

        let value = trim_ascii(&line[eq_index + 1..]);
        return std::str::from_utf8(value).ok();
    }
    None
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while let Some(first) = bytes.first() {
        if first.is_ascii_whitespace() {
            bytes = &bytes[1..];
        } else {
            break;
        }
    }
    while let Some(last) = bytes.last() {
        if last.is_ascii_whitespace() {
            bytes = &bytes[..bytes.len() - 1];
        } else {
            break;
        }
    }
    bytes
}

fn write_typeio_string(
    out: &mut Vec<u8>,
    field: &'static str,
    value: &str,
) -> Result<(), ConnectPacketEncodeError> {
    out.push(1);
    write_java_modified_utf(out, field, value)
}

fn write_java_modified_utf(
    out: &mut Vec<u8>,
    field: &'static str,
    value: &str,
) -> Result<(), ConnectPacketEncodeError> {
    let utf_len = modified_utf8_len(value);
    let utf_len_u16 = u16::try_from(utf_len)
        .map_err(|_| ConnectPacketEncodeError::StringTooLong { field, utf_len })?;
    out.extend_from_slice(&utf_len_u16.to_be_bytes());

    for unit in value.encode_utf16() {
        match unit {
            0x0001..=0x007f => out.push(unit as u8),
            0x0000 | 0x0080..=0x07ff => {
                out.push((0xc0 | ((unit >> 6) & 0x1f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
            _ => {
                out.push((0xe0 | ((unit >> 12) & 0x0f)) as u8);
                out.push((0x80 | ((unit >> 6) & 0x3f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
        }
    }

    Ok(())
}

fn modified_utf8_len(value: &str) -> usize {
    value
        .encode_utf16()
        .map(|unit| match unit {
            0x0001..=0x007f => 1,
            0x0000 | 0x0080..=0x07ff => 2,
            _ => 3,
        })
        .sum()
}

fn decode_base64(input: &str) -> Result<Vec<u8>, ConnectPacketEncodeError> {
    let cleaned = input
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<Vec<_>>();
    if cleaned.len() % 4 != 0 {
        return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
    }

    let mut effective_len = cleaned.len();
    while effective_len > 0 && cleaned[effective_len - 1] == '=' {
        effective_len -= 1;
    }

    let output_len = (effective_len * 3) / 4;
    let mut output = Vec::with_capacity(output_len);
    let mut index = 0usize;
    while index < effective_len {
        let i0 = cleaned[index];
        let i1 = cleaned[index + 1];
        let i2 = if index + 2 < effective_len {
            cleaned[index + 2]
        } else {
            'A'
        };
        let i3 = if index + 3 < effective_len {
            cleaned[index + 3]
        } else {
            'A'
        };

        let b0 = decode_base64_value(i0, index)?;
        let b1 = decode_base64_value(i1, index + 1)?;
        let b2 = decode_base64_value(i2, index + 2)?;
        let b3 = decode_base64_value(i3, index + 3)?;

        output.push((b0 << 2) | (b1 >> 4));
        if output.len() < output_len {
            output.push(((b1 & 0x0f) << 4) | (b2 >> 2));
        }
        if output.len() < output_len {
            output.push(((b2 & 0x03) << 6) | b3);
        }

        index += 4;
    }

    Ok(output)
}

fn decode_base64_value(ch: char, index: usize) -> Result<u8, ConnectPacketEncodeError> {
    match ch {
        'A'..='Z' => Ok((ch as u8) - b'A'),
        'a'..='z' => Ok((ch as u8) - b'a' + 26),
        '0'..='9' => Ok((ch as u8) - b'0' + 52),
        '+' | '-' => Ok(62),
        '/' | '_' => Ok(63),
        _ => Err(ConnectPacketEncodeError::InvalidBase64Char { ch, index }),
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    let output_len = bytes.len().div_ceil(3) * 4;
    let mut output = String::with_capacity(output_len);
    let mut index = 0usize;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied().unwrap_or(0);
        let b2 = bytes.get(index + 2).copied().unwrap_or(0);

        output.push(BASE64_ENCODE[(b0 >> 2) as usize] as char);
        output.push(BASE64_ENCODE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if index + 1 < bytes.len() {
            output.push(BASE64_ENCODE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if index + 2 < bytes.len() {
            output.push(BASE64_ENCODE[(b2 & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }

        index += 3;
    }
    output
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = if crc & 1 == 0 { 0 } else { 0xedb8_8320 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex_text(text: &str) -> Vec<u8> {
        let cleaned = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn encodes_java_golden_connect_packet_bytes() {
        let expected = decode_hex_text(include_str!(
            "../../../tests/src/test/resources/connect-packet.hex"
        ));
        let spec = ConnectPacketSpec {
            version: 123,
            version_type: "golden-type".to_string(),
            name: "golden-user".to_string(),
            locale: "en_US".to_string(),
            usid: "golden-usid".to_string(),
            uuid: "AAECAwQFBgcICQoLDA0ODw==".to_string(),
            mobile: true,
            color: 0x1122_3344,
            mods: vec!["mod-a:1".to_string(), "mod-b:2".to_string()],
        };

        let actual = spec.encode_payload().unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn generated_identity_roundtrips_to_eight_bytes() {
        let encoded = generate_connect_identity_base64();
        let decoded = decode_base64(&encoded).unwrap();

        assert_eq!(decoded.len(), 8);
    }

    #[test]
    fn server_observed_uuid_appends_crc32_bytes() {
        let spec = ConnectPacketSpec {
            version: -1,
            version_type: "official".to_string(),
            name: "tester".to_string(),
            locale: "en_US".to_string(),
            usid: "AAAAAAAAAAA=".to_string(),
            uuid: "AAECAwQFBgc=".to_string(),
            mobile: false,
            color: -1,
            mods: Vec::new(),
        };

        let raw_uuid = decode_base64(&spec.uuid).unwrap();
        let observed = decode_base64(&spec.server_observed_uuid().unwrap()).unwrap();

        assert_eq!(&observed[..raw_uuid.len()], raw_uuid.as_slice());
        assert_eq!(observed.len(), raw_uuid.len() + 8);
    }

    #[test]
    fn encode_payload_rejects_empty_usid_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.usid = "   ".to_string();

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::EmptyField("usid"));
    }

    #[test]
    fn encode_payload_rejects_empty_uuid_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.uuid = "".to_string();

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::EmptyField("uuid"));
    }

    #[test]
    fn encode_payload_rejects_empty_uuid_bytes_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.uuid = "====".to_string();

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::InvalidUuidLength(0));
    }

    #[test]
    fn encode_payload_rejects_empty_mod_entry_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.mods = vec!["mod-a:1".to_string(), "  ".to_string()];

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(
            err,
            ConnectPacketEncodeError::InvalidModEntry {
                index: 1,
                reason: "must not be empty",
            }
        );
    }

    #[test]
    fn compatibility_warnings_flag_negative_build() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.version = -1;
        spec.version_type = "official".to_string();

        let warnings = spec
            .compatibility_warnings()
            .iter()
            .map(|warning| warning.code)
            .collect::<Vec<_>>();

        assert_eq!(
            warnings,
            vec![ConnectCompatibilityWarningCode::BuildUnknown]
        );
    }

    #[test]
    fn compatibility_warnings_flag_custom_version_type() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.version = 123;
        spec.version_type = "custom build".to_string();

        let warnings = spec
            .compatibility_warnings()
            .iter()
            .map(|warning| warning.code)
            .collect::<Vec<_>>();

        assert_eq!(
            warnings,
            vec![ConnectCompatibilityWarningCode::VersionTypeCustomLike]
        );
    }

    #[test]
    fn compatibility_warnings_are_empty_for_release_like_values() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.version = 123;
        spec.version_type = "official".to_string();

        assert!(spec.compatibility_warnings().is_empty());
    }
}
